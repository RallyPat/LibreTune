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
