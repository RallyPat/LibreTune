use crate::ini::expression::{Expr, Parser};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMathChannel {
    pub name: String,
    pub units: String,
    pub expression: String, // The source string

    #[serde(skip)]
    pub cached_ast: Option<Expr>,
}

impl UserMathChannel {
    pub fn new(name: String, units: String, expression: String) -> Self {
        Self {
            name,
            units,
            expression,
            cached_ast: None,
        }
    }

    pub fn compile(&mut self) -> Result<(), String> {
        let mut parser = Parser::new(&self.expression);
        match parser.parse() {
            Ok(expr) => {
                self.cached_ast = Some(expr);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}

/// Collect every `Expr::Variable` name referenced by an expression.
fn collect_variables(expr: &Expr, out: &mut std::collections::HashSet<String>) {
    match expr {
        Expr::Literal(_) => {}
        Expr::Variable(name) => {
            out.insert(name.clone());
        }
        Expr::Binary(a, _, b) => {
            collect_variables(a, out);
            collect_variables(b, out);
        }
        Expr::Ternary(a, b, c) => {
            collect_variables(a, out);
            collect_variables(b, out);
            collect_variables(c, out);
        }
        Expr::Unary(_, a) => collect_variables(a, out),
        Expr::FunctionCall(_, args) => {
            for a in args {
                collect_variables(a, out);
            }
        }
    }
}

/// Dependency-aware evaluation order for user math channels.
///
/// Channels are evaluated by inserting each result into the shared value map,
/// so a channel can only see the channels evaluated before it. Iterating in
/// stored (creation) order makes a forward reference — a channel referencing
/// another math channel created after it — silently read 0 forever (issue
/// #127). This returns indices in dependency order (Kahn's algorithm, stable
/// with respect to stored order among ready channels), so creation order no
/// longer matters. Channels involved in a reference cycle can't be ordered;
/// they are appended in stored order, which matches the old behavior for
/// exactly that unresolvable case.
///
/// Compiles any channel whose AST isn't cached yet (same as the evaluation
/// loops did); channels that fail to compile have no dependencies.
pub fn math_channel_evaluation_order(channels: &mut [UserMathChannel]) -> Vec<usize> {
    use std::collections::{HashMap, HashSet};

    let n = channels.len();
    if n <= 1 {
        return (0..n).collect();
    }

    for ch in channels.iter_mut() {
        if ch.cached_ast.is_none() {
            let _ = ch.compile();
        }
    }

    let index_by_name: HashMap<&str, usize> = channels
        .iter()
        .enumerate()
        .map(|(i, c)| (c.name.as_str(), i))
        .collect();

    // deps[i] = indices of math channels that channel i references.
    let mut deps: Vec<HashSet<usize>> = vec![HashSet::new(); n];
    for (i, ch) in channels.iter().enumerate() {
        if let Some(expr) = &ch.cached_ast {
            let mut vars = HashSet::new();
            collect_variables(expr, &mut vars);
            for v in vars {
                if let Some(&j) = index_by_name.get(v.as_str()) {
                    if j != i {
                        deps[i].insert(j);
                    }
                }
            }
        }
    }

    // Kahn's algorithm, scanning in stored order each round so the result is
    // deterministic and matches stored order whenever dependencies allow.
    let mut ordered = Vec::with_capacity(n);
    let mut placed = vec![false; n];
    loop {
        let mut progressed = false;
        for i in 0..n {
            if !placed[i] && deps[i].iter().all(|&j| placed[j]) {
                placed[i] = true;
                ordered.push(i);
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }

    // Anything left is part of a reference cycle: append in stored order.
    for (i, done) in placed.iter().enumerate() {
        if !done {
            ordered.push(i);
        }
    }

    ordered
}

pub fn save_math_channels(path: &Path, channels: &[UserMathChannel]) -> Result<(), String> {
    let json = serde_json::to_string_pretty(channels).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}

pub fn load_math_channels(path: &Path) -> Result<Vec<UserMathChannel>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut channels: Vec<UserMathChannel> =
        serde_json::from_str(&content).map_err(|e| e.to_string())?;

    // Compile them after loading
    for channel in &mut channels {
        // We suppress errors here - invalid channels will fail at runtime
        // or be flagged in the UI, but shouldn't prevent loading
        let _ = channel.compile();
    }

    Ok(channels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_math_channel_compile() {
        let mut ch = UserMathChannel::new(
            "test".to_string(),
            "psi".to_string(),
            "(map - 100) * 0.1".to_string(),
        );
        assert!(ch.cached_ast.is_none());
        assert!(ch.compile().is_ok());
        assert!(ch.cached_ast.is_some());
    }

    #[test]
    fn test_invalid_expression() {
        let mut ch = UserMathChannel::new("bad".to_string(), "".to_string(), "map + ".to_string());
        assert!(ch.compile().is_err());
    }

    fn ch(name: &str, expr: &str) -> UserMathChannel {
        UserMathChannel::new(name.to_string(), String::new(), expr.to_string())
    }

    #[test]
    fn forward_reference_evaluates_dependency_first() {
        // The issue #127 repro: Boost_Warn created BEFORE Boost_PSI, but it
        // references Boost_PSI — so Boost_PSI must be evaluated first.
        let mut channels = vec![
            ch("Boost_Warn", "Boost_PSI > 5 ? 1 : 0"),
            ch("Boost_PSI", "(map - 100) * 0.145"),
        ];
        let order = math_channel_evaluation_order(&mut channels);
        assert_eq!(order, vec![1, 0]);

        // And evaluating in that order actually produces the right values.
        let mut data = std::collections::HashMap::from([("map".to_string(), 150.0)]);
        for &i in &order {
            let expr = channels[i].cached_ast.as_ref().unwrap();
            let v = crate::ini::expression::evaluate_simple(expr, &data)
                .unwrap()
                .as_f64();
            data.insert(channels[i].name.clone(), v);
        }
        assert!((data["Boost_PSI"] - 7.25).abs() < 1e-9);
        assert_eq!(data["Boost_Warn"], 1.0);
    }

    #[test]
    fn chain_orders_transitively_and_independents_keep_stored_order() {
        // c -> b -> a defined in reverse; x is independent and stays put
        // relative to its dependency-free peers.
        let mut channels = vec![
            ch("c", "b + 1"),
            ch("x", "map * 2"),
            ch("b", "a + 1"),
            ch("a", "rpm / 100"),
        ];
        let order = math_channel_evaluation_order(&mut channels);
        let pos = |name: &str| {
            order
                .iter()
                .position(|&i| channels[i].name == name)
                .unwrap()
        };
        assert!(pos("a") < pos("b"));
        assert!(pos("b") < pos("c"));
        // x has no deps: it must come before the channels that wait on others.
        assert!(pos("x") < pos("b"));
    }

    #[test]
    fn cycle_falls_back_to_stored_order_without_losing_channels() {
        let mut channels = vec![ch("p", "q + 1"), ch("q", "p + 1"), ch("solo", "rpm")];
        let order = math_channel_evaluation_order(&mut channels);
        // Every channel appears exactly once.
        let mut seen = order.clone();
        seen.sort_unstable();
        assert_eq!(seen, vec![0, 1, 2]);
        // solo is orderable and comes first; the cycle keeps stored order.
        assert_eq!(order, vec![2, 0, 1]);
    }

    #[test]
    fn invalid_expression_still_included_in_order() {
        let mut channels = vec![ch("bad", "map + "), ch("good", "rpm * 2")];
        let order = math_channel_evaluation_order(&mut channels);
        let mut seen = order.clone();
        seen.sort_unstable();
        assert_eq!(seen, vec![0, 1]);
    }

    #[test]
    fn test_persistence() {
        let temp_dir = std::env::temp_dir().join("libretune_test_math");
        fs::create_dir_all(&temp_dir).unwrap();
        let file_path = temp_dir.join("math_channels.json");

        let channels = vec![
            UserMathChannel::new("A".to_string(), "u".to_string(), "1+1".to_string()),
            UserMathChannel::new("B".to_string(), "v".to_string(), "2*2".to_string()),
        ];

        assert!(save_math_channels(&file_path, &channels).is_ok());

        let loaded = load_math_channels(&file_path).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].name, "A");
        // Check if expression was preserved
        assert_eq!(loaded[0].expression, "1+1");
        // Check if it was compiled during load
        assert!(loaded[0].cached_ast.is_some());

        fs::remove_dir_all(temp_dir).unwrap();
    }
}
