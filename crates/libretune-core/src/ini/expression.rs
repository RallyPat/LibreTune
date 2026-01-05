//! INI Expression Parser and Evaluator
//! 
//! This module implements a parser and evaluator for expressions used in ECU INI files
//! for conditional visibility, computed channels, and indicators.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

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
            Value::Bool(b) => if *b { 1.0 } else { 0.0 },
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
    Add, Sub, Mul, Div, Mod,
    Eq, Ne, Lt, Gt, Le, Ge,
    And, Or,
    BitAnd, BitOr, BitXor, Shl, Shr,
}

/// Unary operators
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum UnaryOp {
    Neg, Not, BitNot,
}

/// Expression AST
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    Literal(Value),
    Variable(String),
    Binary(Box<Expr>, BinOp, Box<Expr>),
    Unary(UnaryOp, Box<Expr>),
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
    Plus, Minus, Star, Slash, Percent,
    EqEq, Ne, Lt, Gt, Le, Ge,
    AmpAmp, PipePipe, Bang,
    Amp, Pipe, Caret, Tilde,
    Shl, Shr,
    LParen, RParen,
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
        if self.match_token(Token::EqEq) { Some(BinOp::Eq) }
        else if self.match_token(Token::Ne) { Some(BinOp::Ne) }
        else { None }
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
        if self.match_token(Token::Lt) { Some(BinOp::Lt) }
        else if self.match_token(Token::Gt) { Some(BinOp::Gt) }
        else if self.match_token(Token::Le) { Some(BinOp::Le) }
        else if self.match_token(Token::Ge) { Some(BinOp::Ge) }
        else { None }
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
        if self.match_token(Token::Shl) { Some(BinOp::Shl) }
        else if self.match_token(Token::Shr) { Some(BinOp::Shr) }
        else { None }
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
        if self.match_token(Token::Plus) { Some(BinOp::Add) }
        else if self.match_token(Token::Minus) { Some(BinOp::Sub) }
        else { None }
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
        if self.match_token(Token::Star) { Some(BinOp::Mul) }
        else if self.match_token(Token::Slash) { Some(BinOp::Div) }
        else if self.match_token(Token::Percent) { Some(BinOp::Mod) }
        else { None }
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
        match self.advance() {
            Some(Token::Number(n)) => Ok(Expr::Literal(Value::Number(*n))),
            Some(Token::Ident(s)) => {
                if s == "true" { Ok(Expr::Literal(Value::Bool(true))) }
                else if s == "false" { Ok(Expr::Literal(Value::Bool(false))) }
                else { Ok(Expr::Variable(s.clone())) }
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
                    if next_ch == '"' { break; }
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

/// Evaluates an expression against a context
pub fn evaluate(expr: &Expr, context: &HashMap<String, f64>) -> Result<Value, String> {
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
        Expr::Unary(op, inner) => {
            let val = evaluate(inner, context)?;
            match op {
                UnaryOp::Neg => Ok(Value::Number(-val.as_f64())),
                UnaryOp::Not => Ok(Value::Bool(!val.as_bool())),
                UnaryOp::BitNot => Ok(Value::Number(!(val.as_f64() as i64) as f64)),
            }
        }
        Expr::Binary(left, op, right) => {
            let l = evaluate(left, context)?;
            let r = evaluate(right, context)?;

            match op {
                BinOp::Add => Ok(Value::Number(l.as_f64() + r.as_f64())),
                BinOp::Sub => Ok(Value::Number(l.as_f64() - r.as_f64())),
                BinOp::Mul => Ok(Value::Number(l.as_f64() * r.as_f64())),
                BinOp::Div => {
                    let rv = r.as_f64();
                    if rv == 0.0 { Ok(Value::Number(0.0)) }
                    else { Ok(Value::Number(l.as_f64() / rv)) }
                }
                BinOp::Mod => Ok(Value::Number(l.as_f64() % r.as_f64())),
                BinOp::Eq => Ok(Value::Bool(l.as_f64() == r.as_f64())),
                BinOp::Ne => Ok(Value::Bool(l.as_f64() != r.as_f64())),
                BinOp::Lt => Ok(Value::Bool(l.as_f64() < r.as_f64())),
                BinOp::Gt => Ok(Value::Bool(l.as_f64() > r.as_f64())),
                BinOp::Le => Ok(Value::Bool(l.as_f64() <= r.as_f64())),
                BinOp::Ge => Ok(Value::Bool(l.as_f64() >= r.as_f64())),
                BinOp::And => Ok(Value::Bool(l.as_bool() && r.as_bool())),
                BinOp::Or => Ok(Value::Bool(l.as_bool() || r.as_bool())),
                BinOp::BitAnd => Ok(Value::Number(((l.as_f64() as i64) & (r.as_f64() as i64)) as f64)),
                BinOp::BitOr => Ok(Value::Number(((l.as_f64() as i64) | (r.as_f64() as i64)) as f64)),
                BinOp::BitXor => Ok(Value::Number(((l.as_f64() as i64) ^ (r.as_f64() as i64)) as f64)),
                BinOp::Shl => Ok(Value::Number(((l.as_f64() as i64) << (r.as_f64() as i32)) as f64)),
                BinOp::Shr => Ok(Value::Number(((l.as_f64() as i64) >> (r.as_f64() as i32)) as f64)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arithmetic() {
        let mut p = Parser::new("1 + 2 * 3");
        let expr = p.parse().unwrap();
        let context = HashMap::new();
        assert_eq!(evaluate(&expr, &context).unwrap(), Value::Number(7.0));
    }

    #[test]
    fn test_logical() {
        let mut p = Parser::new("true && false || 1 == 1");
        let expr = p.parse().unwrap();
        let context = HashMap::new();
        assert_eq!(evaluate(&expr, &context).unwrap(), Value::Bool(true));
    }

    #[test]
    fn test_variable() {
        let mut p = Parser::new("rpm > 1000");
        let expr = p.parse().unwrap();
        let mut context = HashMap::new();
        context.insert("rpm".to_string(), 1500.0);
        assert_eq!(evaluate(&expr, &context).unwrap(), Value::Bool(true));
        
        context.insert("rpm".to_string(), 500.0);
        assert_eq!(evaluate(&expr, &context).unwrap(), Value::Bool(false));
    }

    #[test]
    fn test_bitwise() {
        let mut p = Parser::new("(flags & 4) == 4");
        let expr = p.parse().unwrap();
        let mut context = HashMap::new();
        context.insert("flags".to_string(), 5.0); // binary 101
        assert_eq!(evaluate(&expr, &context).unwrap(), Value::Bool(true));
        
        context.insert("flags".to_string(), 3.0); // binary 011
        assert_eq!(evaluate(&expr, &context).unwrap(), Value::Bool(false));
    }
}
