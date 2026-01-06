//! INI Expression Parser and Evaluator
//!
//! This module implements a parser and evaluator for expressions used in ECU INI files
//! for conditional visibility, computed channels, and indicators.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Values supported in expressions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Number(f64),
    Bool(bool),
    String(String),
}

impl Value {
    pub fn as_f64(&self) -> f64 {
        match self {
            Value::Number(n) => *n,
            Value::Bool(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            Value::String(_) => 0.0,
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            Value::Number(n) => *n != 0.0,
            Value::Bool(b) => *b,
            Value::String(s) => !s.is_empty(),
        }
    }
}

/// Binary operators
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

/// Unary operators
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum UnaryOp {
    Neg,
    Not,
    BitNot,
}

/// Expression AST
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    Literal(Value),
    Variable(String),
    Binary(Box<Expr>, BinOp, Box<Expr>),
    Unary(UnaryOp, Box<Expr>),
    FunctionCall(String, Vec<Expr>), // function name, arguments
}

/// Parser for expressions
pub struct Parser<'a> {
    tokens: Vec<Token>,
    pos: usize,
    _input: &'a str,
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Ident(String),
    String(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    EqEq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    AmpAmp,
    PipePipe,
    Bang,
    Amp,
    Pipe,
    Caret,
    Tilde,
    Shl,
    Shr,
    LParen,
    RParen,
    Comma,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        let tokens = lex(input);
        Self {
            tokens,
            pos: 0,
            _input: input,
        }
    }

    pub fn parse(&mut self) -> Result<Expr, String> {
        self.parse_logical_or()
    }

    fn parse_logical_or(&mut self) -> Result<Expr, String> {
        let mut node = self.parse_logical_and()?;
        while self.match_token(Token::PipePipe) {
            let right = self.parse_logical_and()?;
            node = Expr::Binary(Box::new(node), BinOp::Or, Box::new(right));
        }
        Ok(node)
    }

    fn parse_logical_and(&mut self) -> Result<Expr, String> {
        let mut node = self.parse_bitwise_or()?;
        while self.match_token(Token::AmpAmp) {
            let right = self.parse_bitwise_or()?;
            node = Expr::Binary(Box::new(node), BinOp::And, Box::new(right));
        }
        Ok(node)
    }

    fn parse_bitwise_or(&mut self) -> Result<Expr, String> {
        let mut node = self.parse_bitwise_xor()?;
        while self.match_token(Token::Pipe) {
            let right = self.parse_bitwise_xor()?;
            node = Expr::Binary(Box::new(node), BinOp::BitOr, Box::new(right));
        }
        Ok(node)
    }

    fn parse_bitwise_xor(&mut self) -> Result<Expr, String> {
        let mut node = self.parse_bitwise_and()?;
        while self.match_token(Token::Caret) {
            let right = self.parse_bitwise_and()?;
            node = Expr::Binary(Box::new(node), BinOp::BitXor, Box::new(right));
        }
        Ok(node)
    }

    fn parse_bitwise_and(&mut self) -> Result<Expr, String> {
        let mut node = self.parse_equality()?;
        while self.match_token(Token::Amp) {
            let right = self.parse_equality()?;
            node = Expr::Binary(Box::new(node), BinOp::BitAnd, Box::new(right));
        }
        Ok(node)
    }

    fn parse_equality(&mut self) -> Result<Expr, String> {
        let mut node = self.parse_comparison()?;
        while let Some(op) = self.match_equality_op() {
            let right = self.parse_comparison()?;
            node = Expr::Binary(Box::new(node), op, Box::new(right));
        }
        Ok(node)
    }

    fn match_equality_op(&mut self) -> Option<BinOp> {
        if self.match_token(Token::EqEq) {
            Some(BinOp::Eq)
        } else if self.match_token(Token::Ne) {
            Some(BinOp::Ne)
        } else {
            None
        }
    }

    fn parse_comparison(&mut self) -> Result<Expr, String> {
        let mut node = self.parse_shift()?;
        while let Some(op) = self.match_comparison_op() {
            let right = self.parse_shift()?;
            node = Expr::Binary(Box::new(node), op, Box::new(right));
        }
        Ok(node)
    }

    fn match_comparison_op(&mut self) -> Option<BinOp> {
        if self.match_token(Token::Lt) {
            Some(BinOp::Lt)
        } else if self.match_token(Token::Gt) {
            Some(BinOp::Gt)
        } else if self.match_token(Token::Le) {
            Some(BinOp::Le)
        } else if self.match_token(Token::Ge) {
            Some(BinOp::Ge)
        } else {
            None
        }
    }

    fn parse_shift(&mut self) -> Result<Expr, String> {
        let mut node = self.parse_additive()?;
        while let Some(op) = self.match_shift_op() {
            let right = self.parse_additive()?;
            node = Expr::Binary(Box::new(node), op, Box::new(right));
        }
        Ok(node)
    }

    fn match_shift_op(&mut self) -> Option<BinOp> {
        if self.match_token(Token::Shl) {
            Some(BinOp::Shl)
        } else if self.match_token(Token::Shr) {
            Some(BinOp::Shr)
        } else {
            None
        }
    }

    fn parse_additive(&mut self) -> Result<Expr, String> {
        let mut node = self.parse_multiplicative()?;
        while let Some(op) = self.match_additive_op() {
            let right = self.parse_multiplicative()?;
            node = Expr::Binary(Box::new(node), op, Box::new(right));
        }
        Ok(node)
    }

    fn match_additive_op(&mut self) -> Option<BinOp> {
        if self.match_token(Token::Plus) {
            Some(BinOp::Add)
        } else if self.match_token(Token::Minus) {
            Some(BinOp::Sub)
        } else {
            None
        }
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, String> {
        let mut node = self.parse_unary()?;
        while let Some(op) = self.match_multiplicative_op() {
            let right = self.parse_unary()?;
            node = Expr::Binary(Box::new(node), op, Box::new(right));
        }
        Ok(node)
    }

    fn match_multiplicative_op(&mut self) -> Option<BinOp> {
        if self.match_token(Token::Star) {
            Some(BinOp::Mul)
        } else if self.match_token(Token::Slash) {
            Some(BinOp::Div)
        } else if self.match_token(Token::Percent) {
            Some(BinOp::Mod)
        } else {
            None
        }
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        if self.match_token(Token::Minus) {
            Ok(Expr::Unary(UnaryOp::Neg, Box::new(self.parse_unary()?)))
        } else if self.match_token(Token::Bang) {
            Ok(Expr::Unary(UnaryOp::Not, Box::new(self.parse_unary()?)))
        } else if self.match_token(Token::Tilde) {
            Ok(Expr::Unary(UnaryOp::BitNot, Box::new(self.parse_unary()?)))
        } else {
            self.parse_primary()
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        let token = self.advance();
        match token {
            Some(Token::Number(n)) => Ok(Expr::Literal(Value::Number(*n))),
            Some(Token::Ident(s)) => {
                let s_clone = s.clone();
                if s == "true" {
                    Ok(Expr::Literal(Value::Bool(true)))
                } else if s == "false" {
                    Ok(Expr::Literal(Value::Bool(false)))
                } else if self.match_token(Token::LParen) {
                    // Function call: name(arg1, arg2, ...)
                    let mut args = Vec::new();
                    if !self.match_token(Token::RParen) {
                        loop {
                            args.push(self.parse()?);
                            if self.match_token(Token::RParen) {
                                break;
                            }
                            if !self.match_token(Token::Comma) {
                                return Err("Expected ',' or ')'".to_string());
                            }
                        }
                    }
                    Ok(Expr::FunctionCall(s_clone, args))
                } else {
                    Ok(Expr::Variable(s_clone))
                }
            }
            Some(Token::String(s)) => Ok(Expr::Literal(Value::String(s.clone()))),
            Some(Token::LParen) => {
                let expr = self.parse()?;
                if !self.match_token(Token::RParen) {
                    return Err("Expected ')'".to_string());
                }
                Ok(expr)
            }
            _ => Err("Unexpected token".to_string()),
        }
    }

    fn advance(&mut self) -> Option<&Token> {
        if self.pos < self.tokens.len() {
            let token = &self.tokens[self.pos];
            self.pos += 1;
            Some(token)
        } else {
            None
        }
    }

    fn match_token(&mut self, token: Token) -> bool {
        if let Some(t) = self.tokens.get(self.pos) {
            if *t == token {
                self.pos += 1;
                return true;
            }
        }
        false
    }
}

fn lex(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            ' ' | '\t' | '\r' | '\n' => continue,
            '(' => tokens.push(Token::LParen),
            ')' => tokens.push(Token::RParen),
            ',' => tokens.push(Token::Comma),
            '+' => tokens.push(Token::Plus),
            '-' => tokens.push(Token::Minus),
            '*' => tokens.push(Token::Star),
            '/' => tokens.push(Token::Slash),
            '%' => tokens.push(Token::Percent),
            '~' => tokens.push(Token::Tilde),
            '^' => tokens.push(Token::Caret),
            '!' => {
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Ne);
                } else {
                    tokens.push(Token::Bang);
                }
            }
            '=' => {
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::EqEq);
                }
            }
            '<' => {
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Le);
                } else if chars.peek() == Some(&'<') {
                    chars.next();
                    tokens.push(Token::Shl);
                } else {
                    tokens.push(Token::Lt);
                }
            }
            '>' => {
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Ge);
                } else if chars.peek() == Some(&'>') {
                    chars.next();
                    tokens.push(Token::Shr);
                } else {
                    tokens.push(Token::Gt);
                }
            }
            '&' => {
                if chars.peek() == Some(&'&') {
                    chars.next();
                    tokens.push(Token::AmpAmp);
                } else {
                    tokens.push(Token::Amp);
                }
            }
            '|' => {
                if chars.peek() == Some(&'|') {
                    chars.next();
                    tokens.push(Token::PipePipe);
                } else {
                    tokens.push(Token::Pipe);
                }
            }
            '"' => {
                let mut s = String::new();
                for next_ch in chars.by_ref() {
                    if next_ch == '"' {
                        break;
                    }
                    s.push(next_ch);
                }
                tokens.push(Token::String(s));
            }
            ch if ch.is_ascii_digit() => {
                let mut s = String::new();
                s.push(ch);
                while let Some(&next_ch) = chars.peek() {
                    if next_ch.is_ascii_digit() || next_ch == '.' {
                        s.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                if let Ok(n) = s.parse::<f64>() {
                    tokens.push(Token::Number(n));
                }
            }
            '$' => {
                // Path functions start with $
                let mut s = String::new();
                s.push('$');
                while let Some(&next_ch) = chars.peek() {
                    if next_ch.is_alphanumeric() || next_ch == '_' {
                        s.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Ident(s));
            }
            ch if ch.is_alphabetic() || ch == '_' => {
                let mut s = String::new();
                s.push(ch);
                while let Some(&next_ch) = chars.peek() {
                    if next_ch.is_alphanumeric() || next_ch == '_' {
                        s.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Ident(s));
            }
            _ => {}
        }
    }
    tokens
}

/// Context for string function evaluation
#[derive(Default)]
pub struct StringContext {
    /// Function to get string value of a constant
    pub get_string_value: Option<Box<dyn Fn(&str) -> Option<String> + Send + Sync>>,
    /// Function to get bit options for a constant
    pub get_bit_options: Option<Box<dyn Fn(&str) -> Option<Vec<String>> + Send + Sync>>,
    /// Function to get projects directory path
    pub get_projects_dir: Option<Box<dyn Fn() -> String + Send + Sync>>,
    /// Function to get working directory path
    pub get_working_dir: Option<Box<dyn Fn() -> String + Send + Sync>>,
}


/// Evaluates a function call
fn evaluate_function(
    name: &str,
    args: &[Expr],
    context: &HashMap<String, f64>,
    string_context: Option<&StringContext>,
) -> Result<Value, String> {
    let name_lower = name.to_lowercase();

    match name_lower.as_str() {
        // Math functions (single argument)
        "abs" | "round" | "floor" | "ceil" | "sqrt" | "log" | "exp" | "sin" | "cos" | "tan"
        | "asin" | "acos" | "atan" => {
            if args.len() != 1 {
                return Err(format!(
                    "Function {} requires 1 argument, got {}",
                    name,
                    args.len()
                ));
            }
            let arg = evaluate(&args[0], context, string_context)?;
            let x = arg.as_f64();

            match name_lower.as_str() {
                "abs" => Ok(Value::Number(x.abs())),
                "round" => Ok(Value::Number(x.round())),
                "floor" => Ok(Value::Number(x.floor())),
                "ceil" => Ok(Value::Number(x.ceil())),
                "sqrt" => Ok(Value::Number(x.sqrt())),
                "log" => Ok(Value::Number(x.ln())),
                "exp" => Ok(Value::Number(x.exp())),
                "sin" => Ok(Value::Number(x.sin())),
                "cos" => Ok(Value::Number(x.cos())),
                "tan" => Ok(Value::Number(x.tan())),
                "asin" => Ok(Value::Number(x.asin())),
                "acos" => Ok(Value::Number(x.acos())),
                "atan" => Ok(Value::Number(x.atan())),
                _ => unreachable!(),
            }
        }
        // Math functions (two arguments)
        "pow" | "atan2" => {
            if args.len() != 2 {
                return Err(format!(
                    "Function {} requires 2 arguments, got {}",
                    name,
                    args.len()
                ));
            }
            let arg1 = evaluate(&args[0], context, string_context)?;
            let arg2 = evaluate(&args[1], context, string_context)?;
            let x = arg1.as_f64();
            let y = arg2.as_f64();

            match name_lower.as_str() {
                "pow" => Ok(Value::Number(x.powf(y))),
                "atan2" => Ok(Value::Number(x.atan2(y))),
                _ => unreachable!(),
            }
        }
        // Variadic functions (2+ arguments)
        "min" => {
            if args.len() < 2 {
                return Err(format!(
                    "Function min requires at least 2 arguments, got {}",
                    args.len()
                ));
            }
            let mut min_val = evaluate(&args[0], context, string_context)?.as_f64();
            for arg in &args[1..] {
                min_val = min_val.min(evaluate(arg, context, string_context)?.as_f64());
            }
            Ok(Value::Number(min_val))
        }
        "max" => {
            if args.len() < 2 {
                return Err(format!(
                    "Function max requires at least 2 arguments, got {}",
                    args.len()
                ));
            }
            let mut max_val = evaluate(&args[0], context, string_context)?.as_f64();
            for arg in &args[1..] {
                max_val = max_val.max(evaluate(arg, context, string_context)?.as_f64());
            }
            Ok(Value::Number(max_val))
        }
        // Special functions
        "isnan" => {
            if args.len() != 1 {
                return Err(format!(
                    "Function isNaN requires 1 argument, got {}",
                    args.len()
                ));
            }
            let arg = evaluate(&args[0], context, string_context)?;
            Ok(Value::Bool(arg.as_f64().is_nan()))
        }
        "isadvancedmathavailable" => {
            // Always return true (we support advanced math)
            Ok(Value::Bool(true))
        }
        // String functions
        "bitstringvalue" => {
            // bitStringValue(bitOptionsConstant, indexValue)
            // Returns the string value at index in bit_options array
            if args.len() != 2 {
                return Err(format!(
                    "Function bitStringValue requires 2 arguments, got {}",
                    args.len()
                ));
            }

            // First arg is constant name (variable), second is index
            let constant_name = match &args[0] {
                Expr::Variable(name) => name.clone(),
                _ => {
                    return Err("bitStringValue first argument must be a constant name".to_string())
                }
            };

            let index_val = evaluate(&args[1], context, string_context)?.as_f64();
            let index = index_val as usize;

            if let Some(ctx) = string_context {
                if let Some(get_bit_options) = &ctx.get_bit_options {
                    if let Some(options) = get_bit_options(&constant_name) {
                        if index < options.len() {
                            return Ok(Value::String(options[index].clone()));
                        }
                    }
                }
            }

            Ok(Value::String(format!("INVALID[{}]", index)))
        }
        "stringvalue" => {
            // stringValue(constantName)
            // Returns the string value of a string constant
            if args.len() != 1 {
                return Err(format!(
                    "Function stringValue requires 1 argument, got {}",
                    args.len()
                ));
            }

            let constant_name = match &args[0] {
                Expr::Variable(name) => name.clone(),
                Expr::Literal(Value::String(s)) => s.clone(),
                _ => {
                    return Err("stringValue argument must be a constant name or string".to_string())
                }
            };

            if let Some(ctx) = string_context {
                if let Some(get_string_value) = &ctx.get_string_value {
                    if let Some(value) = get_string_value(&constant_name) {
                        return Ok(Value::String(value));
                    }
                }
            }

            Ok(Value::String(String::new()))
        }
        _ => {
            // Check for path functions (start with $)
            if name.starts_with('$') {
                match name_lower.as_str() {
                    "$getprojectsdirpath" | "$getprojectsdir" => {
                        if let Some(ctx) = string_context {
                            if let Some(get_projects_dir) = &ctx.get_projects_dir {
                                return Ok(Value::String(get_projects_dir()));
                            }
                        }
                        Ok(Value::String(String::new()))
                    }
                    "$getworkingdirpath" | "$getworkingdir" => {
                        if let Some(ctx) = string_context {
                            if let Some(get_working_dir) = &ctx.get_working_dir {
                                return Ok(Value::String(get_working_dir()));
                            }
                        }
                        Ok(Value::String(String::new()))
                    }
                    _ => Err(format!("Unknown function: {}", name)),
                }
            } else {
                Err(format!("Unknown function: {}", name))
            }
        }
    }
}

/// Evaluates an expression against a context
pub fn evaluate(
    expr: &Expr,
    context: &HashMap<String, f64>,
    string_context: Option<&StringContext>,
) -> Result<Value, String> {
    match expr {
        Expr::Literal(v) => Ok(v.clone()),
        Expr::Variable(name) => {
            if let Some(val) = context.get(name) {
                Ok(Value::Number(*val))
            } else {
                // Default to 0 for unknown variables (common in INIs)
                Ok(Value::Number(0.0))
            }
        }
        Expr::FunctionCall(name, args) => evaluate_function(name, args, context, string_context),
        Expr::Unary(op, inner) => {
            let val = evaluate(inner, context, string_context)?;
            match op {
                UnaryOp::Neg => Ok(Value::Number(-val.as_f64())),
                UnaryOp::Not => Ok(Value::Bool(!val.as_bool())),
                UnaryOp::BitNot => Ok(Value::Number(!(val.as_f64() as i64) as f64)),
            }
        }
        Expr::Binary(left, op, right) => {
            let l = evaluate(left, context, string_context)?;
            let r = evaluate(right, context, string_context)?;

            match op {
                BinOp::Add => {
                    // String concatenation if both are strings
                    if let (Value::String(ref ls), Value::String(ref rs)) = (&l, &r) {
                        Ok(Value::String(format!("{}{}", ls, rs)))
                    } else {
                        Ok(Value::Number(l.as_f64() + r.as_f64()))
                    }
                }
                BinOp::Sub => Ok(Value::Number(l.as_f64() - r.as_f64())),
                BinOp::Mul => Ok(Value::Number(l.as_f64() * r.as_f64())),
                BinOp::Div => {
                    let rv = r.as_f64();
                    if rv == 0.0 {
                        Ok(Value::Number(0.0))
                    } else {
                        Ok(Value::Number(l.as_f64() / rv))
                    }
                }
                BinOp::Mod => Ok(Value::Number(l.as_f64() % r.as_f64())),
                BinOp::Eq => {
                    // String comparison if both are strings
                    if let (Value::String(ref ls), Value::String(ref rs)) = (&l, &r) {
                        Ok(Value::Bool(ls == rs))
                    } else {
                        Ok(Value::Bool(l.as_f64() == r.as_f64()))
                    }
                }
                BinOp::Ne => {
                    // String comparison if both are strings
                    if let (Value::String(ref ls), Value::String(ref rs)) = (&l, &r) {
                        Ok(Value::Bool(ls != rs))
                    } else {
                        Ok(Value::Bool(l.as_f64() != r.as_f64()))
                    }
                }
                BinOp::Lt => Ok(Value::Bool(l.as_f64() < r.as_f64())),
                BinOp::Gt => Ok(Value::Bool(l.as_f64() > r.as_f64())),
                BinOp::Le => Ok(Value::Bool(l.as_f64() <= r.as_f64())),
                BinOp::Ge => Ok(Value::Bool(l.as_f64() >= r.as_f64())),
                BinOp::And => Ok(Value::Bool(l.as_bool() && r.as_bool())),
                BinOp::Or => Ok(Value::Bool(l.as_bool() || r.as_bool())),
                BinOp::BitAnd => Ok(Value::Number(
                    ((l.as_f64() as i64) & (r.as_f64() as i64)) as f64,
                )),
                BinOp::BitOr => Ok(Value::Number(
                    ((l.as_f64() as i64) | (r.as_f64() as i64)) as f64,
                )),
                BinOp::BitXor => Ok(Value::Number(
                    ((l.as_f64() as i64) ^ (r.as_f64() as i64)) as f64,
                )),
                BinOp::Shl => Ok(Value::Number(
                    ((l.as_f64() as i64) << (r.as_f64() as i32)) as f64,
                )),
                BinOp::Shr => Ok(Value::Number(
                    ((l.as_f64() as i64) >> (r.as_f64() as i32)) as f64,
                )),
            }
        }
    }
}

/// Convenience function for backward compatibility (no string context)
pub fn evaluate_simple(expr: &Expr, context: &HashMap<String, f64>) -> Result<Value, String> {
    evaluate(expr, context, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arithmetic() {
        let mut p = Parser::new("1 + 2 * 3");
        let expr = p.parse().unwrap();
        let context = HashMap::new();
        assert_eq!(
            evaluate_simple(&expr, &context).unwrap(),
            Value::Number(7.0)
        );
    }

    #[test]
    fn test_logical() {
        let mut p = Parser::new("true && false || 1 == 1");
        let expr = p.parse().unwrap();
        let context = HashMap::new();
        assert_eq!(evaluate_simple(&expr, &context).unwrap(), Value::Bool(true));
    }

    #[test]
    fn test_variable() {
        let mut p = Parser::new("rpm > 1000");
        let expr = p.parse().unwrap();
        let mut context = HashMap::new();
        context.insert("rpm".to_string(), 1500.0);
        assert_eq!(evaluate_simple(&expr, &context).unwrap(), Value::Bool(true));

        context.insert("rpm".to_string(), 500.0);
        assert_eq!(
            evaluate_simple(&expr, &context).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn test_bitwise() {
        let mut p = Parser::new("(flags & 4) == 4");
        let expr = p.parse().unwrap();
        let mut context = HashMap::new();
        context.insert("flags".to_string(), 5.0); // binary 101
        assert_eq!(evaluate_simple(&expr, &context).unwrap(), Value::Bool(true));

        context.insert("flags".to_string(), 3.0); // binary 011
        assert_eq!(
            evaluate_simple(&expr, &context).unwrap(),
            Value::Bool(false)
        );
    }
}
