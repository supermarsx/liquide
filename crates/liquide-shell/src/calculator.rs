//! Expression evaluator for the launcher's quick-answer feature.
//!
//! Implements a recursive-descent parser with correct operator precedence:
//! `^` (power) > `*`, `/`, `%` > `+`, `-`.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A single lexical token produced by [`tokenize`].
#[derive(Debug, Clone, PartialEq)]
pub enum CalcToken {
    Number(f64),
    Op(char),
    LParen,
    RParen,
    Func(String),
}

/// Result of an [`evaluate`] call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CalcResult {
    Number(f64),
    Conversion {
        value: f64,
        from_unit: String,
        to_unit: String,
        result: f64,
    },
    Error(String),
}

impl fmt::Display for CalcResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(n) => write!(f, "{n}"),
            Self::Conversion {
                value,
                from_unit,
                to_unit,
                result,
            } => write!(f, "{value} {from_unit} = {result} {to_unit}"),
            Self::Error(e) => write!(f, "Error: {e}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Evaluate a mathematical expression and return the result.
#[must_use]
pub fn evaluate(expr: &str) -> CalcResult {
    let tokens = match tokenize(expr) {
        Ok(t) => t,
        Err(e) => return CalcResult::Error(e),
    };
    if tokens.is_empty() {
        return CalcResult::Error("empty expression".into());
    }
    let mut parser = Parser::new(tokens);
    match parser.parse_expr() {
        Ok(val) if parser.at_end() => CalcResult::Number(val),
        Ok(_) => CalcResult::Error("unexpected token after expression".into()),
        Err(e) => CalcResult::Error(e),
    }
}

/// Tokenize a mathematical expression into a sequence of [`CalcToken`]s.
pub fn tokenize(expr: &str) -> Result<Vec<CalcToken>, String> {
    let chars: Vec<char> = expr.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            ' ' | '\t' => i += 1,
            '(' => {
                tokens.push(CalcToken::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(CalcToken::RParen);
                i += 1;
            }
            '+' | '-' | '*' | '/' | '%' | '^' => {
                tokens.push(CalcToken::Op(chars[i]));
                i += 1;
            }
            c if c.is_ascii_digit() || c == '.' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let s: String = chars[start..i].iter().collect();
                let n = s.parse::<f64>().map_err(|_| format!("invalid number: {s}"))?;
                tokens.push(CalcToken::Number(n));
            }
            c if c.is_ascii_alphabetic() => {
                let start = i;
                while i < chars.len() && chars[i].is_ascii_alphabetic() {
                    i += 1;
                }
                let name: String = chars[start..i].iter().collect();
                tokens.push(CalcToken::Func(name));
            }
            c => return Err(format!("unexpected character: {c}")),
        }
    }
    Ok(tokens)
}

/// Convert `value` between two units, returning `None` for unsupported pairs.
///
/// Supported conversions:
/// - Temperature: `F`, `C`, `K`
/// - Distance: `km`, `mi`
/// - Weight: `kg`, `lb`
#[must_use]
pub fn convert_units(value: f64, from: &str, to: &str) -> Option<f64> {
    match (from, to) {
        // Temperature
        ("F", "C") => Some((value - 32.0) * 5.0 / 9.0),
        ("C", "F") => Some(value * 9.0 / 5.0 + 32.0),
        ("F", "K") => Some((value - 32.0) * 5.0 / 9.0 + 273.15),
        ("K", "F") => Some((value - 273.15) * 9.0 / 5.0 + 32.0),
        ("C", "K") => Some(value + 273.15),
        ("K", "C") => Some(value - 273.15),
        // Distance
        ("km", "mi") => Some(value * 0.621_371),
        ("mi", "km") => Some(value * 1.609_344),
        // Weight
        ("kg", "lb") => Some(value * 2.204_623),
        ("lb", "kg") => Some(value * 0.453_592),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Recursive-descent parser
// ---------------------------------------------------------------------------

struct Parser {
    tokens: Vec<CalcToken>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<CalcToken>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&CalcToken> {
        self.tokens.get(self.pos)
    }

    fn at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    /// `expr = term (('+' | '-') term)*`
    fn parse_expr(&mut self) -> Result<f64, String> {
        let mut left = self.parse_term()?;
        loop {
            match self.peek() {
                Some(CalcToken::Op('+')) => {
                    self.pos += 1;
                    left += self.parse_term()?;
                }
                Some(CalcToken::Op('-')) => {
                    self.pos += 1;
                    left -= self.parse_term()?;
                }
                _ => break,
            }
        }
        Ok(left)
    }

    /// `term = factor (('*' | '/' | '%') factor)*`
    fn parse_term(&mut self) -> Result<f64, String> {
        let mut left = self.parse_factor()?;
        loop {
            match self.peek() {
                Some(CalcToken::Op('*')) => {
                    self.pos += 1;
                    left *= self.parse_factor()?;
                }
                Some(CalcToken::Op('/')) => {
                    self.pos += 1;
                    let right = self.parse_factor()?;
                    if right == 0.0 {
                        return Err("division by zero".into());
                    }
                    left /= right;
                }
                Some(CalcToken::Op('%')) => {
                    self.pos += 1;
                    let right = self.parse_factor()?;
                    if right == 0.0 {
                        return Err("modulo by zero".into());
                    }
                    left %= right;
                }
                _ => break,
            }
        }
        Ok(left)
    }

    /// `factor = primary ('^' factor)?`   (right-associative)
    fn parse_factor(&mut self) -> Result<f64, String> {
        let base = self.parse_primary()?;
        if matches!(self.peek(), Some(CalcToken::Op('^'))) {
            self.pos += 1;
            let exp = self.parse_factor()?;
            Ok(base.powf(exp))
        } else {
            Ok(base)
        }
    }

    /// `primary = NUMBER | '(' expr ')' | FUNC '(' expr ')' | ('-' | '+') primary`
    fn parse_primary(&mut self) -> Result<f64, String> {
        match self.peek().cloned() {
            Some(CalcToken::Op('-')) => {
                self.pos += 1;
                Ok(-self.parse_primary()?)
            }
            Some(CalcToken::Op('+')) => {
                self.pos += 1;
                self.parse_primary()
            }
            Some(CalcToken::Number(n)) => {
                self.pos += 1;
                Ok(n)
            }
            Some(CalcToken::LParen) => {
                self.pos += 1;
                let val = self.parse_expr()?;
                if matches!(self.peek(), Some(CalcToken::RParen)) {
                    self.pos += 1;
                    Ok(val)
                } else {
                    Err("missing closing parenthesis".into())
                }
            }
            Some(CalcToken::Func(name)) => {
                self.pos += 1;
                if !matches!(self.peek(), Some(CalcToken::LParen)) {
                    return Err(format!("expected '(' after function {name}"));
                }
                self.pos += 1;
                let arg = self.parse_expr()?;
                if !matches!(self.peek(), Some(CalcToken::RParen)) {
                    return Err("missing ')' after function argument".into());
                }
                self.pos += 1;
                match name.as_str() {
                    "sqrt" => Ok(arg.sqrt()),
                    "sin" => Ok(arg.sin()),
                    "cos" => Ok(arg.cos()),
                    "tan" => Ok(arg.tan()),
                    "log" => Ok(arg.log10()),
                    "ln" => Ok(arg.ln()),
                    "abs" => Ok(arg.abs()),
                    _ => Err(format!("unknown function: {name}")),
                }
            }
            Some(tok) => Err(format!("unexpected token: {tok:?}")),
            None => Err("unexpected end of expression".into()),
        }
    }
}
