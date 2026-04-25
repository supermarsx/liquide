//! Inline calculator search provider.
//!
//! Detects math expressions in the search query and evaluates them using a
//! recursive-descent parser.  Supports basic arithmetic (`+`, `-`, `*`, `/`,
//! `%`, `^`), parenthesised sub-expressions, unary minus, and the constants
//! `pi`, `e`, and `tau`.

use crate::provider::{
    SearchCategory, SearchProvider, SearchResult, SearchResultAction, clamp_score,
};

// ---------------------------------------------------------------------------
// CalculatorProvider
// ---------------------------------------------------------------------------

/// Search provider that evaluates math expressions inline.
pub struct CalculatorProvider;

impl CalculatorProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CalculatorProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchProvider for CalculatorProvider {
    fn id(&self) -> &str {
        "calculator"
    }
    fn name(&self) -> &str {
        "Calculator"
    }
    fn icon(&self) -> &str {
        "accessories-calculator"
    }
    fn priority(&self) -> u32 {
        95
    }

    fn search(&self, query: &str, _max_results: usize) -> Vec<SearchResult> {
        let expr = query.trim();
        if !looks_like_math(expr) {
            return Vec::new();
        }

        // Strip leading `=` if present.
        let expr = expr.strip_prefix('=').unwrap_or(expr).trim();
        if expr.is_empty() {
            return Vec::new();
        }

        match evaluate(expr) {
            Ok(value) => {
                let display = format_value(value);
                vec![SearchResult {
                    id: "calc-result".into(),
                    title: format!("{} = {}", expr, display),
                    description: "Press Enter to copy to clipboard".into(),
                    icon: "accessories-calculator".into(),
                    category: SearchCategory::Calculator,
                    relevance_score: clamp_score(1.0),
                    action: SearchResultAction::Custom(display),
                }]
            }
            Err(_) => Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Expression detection
// ---------------------------------------------------------------------------

/// Heuristic: does this query look like a math expression?
fn looks_like_math(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() {
        return false;
    }

    // Starts with `=`.
    if s.starts_with('=') {
        return true;
    }

    // Starts with a digit, `(`, `.`, or unary minus followed by digit.
    let first = s.chars().next().unwrap();
    if first.is_ascii_digit() || first == '(' || first == '.' {
        // Must also contain an operator or be a lone number with constants.
        return s.chars().any(|c| "+-*/%^()".contains(c)) || s.contains("pi") || s.contains("tau");
    }

    // Starts with a known constant name.
    if s.starts_with("pi") || s.starts_with("tau") || s.starts_with("e ") {
        return true;
    }

    false
}

// ---------------------------------------------------------------------------
// Evaluation
// ---------------------------------------------------------------------------

/// Calculation error.
#[derive(Debug, Clone, PartialEq)]
pub enum CalcError {
    UnexpectedChar(char),
    UnexpectedEnd,
    DivisionByZero,
    UnbalancedParens,
    InvalidExpression,
}

impl std::fmt::Display for CalcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnexpectedChar(c) => write!(f, "unexpected character: '{}'", c),
            Self::UnexpectedEnd => write!(f, "unexpected end of expression"),
            Self::DivisionByZero => write!(f, "division by zero"),
            Self::UnbalancedParens => write!(f, "unbalanced parentheses"),
            Self::InvalidExpression => write!(f, "invalid expression"),
        }
    }
}

/// Evaluate a math expression string and return its value.
///
/// Grammar (precedence low → high):
/// ```text
/// expr   = term (('+' | '-') term)*
/// term   = power (('*' | '/' | '%') power)*
/// power  = unary ('^' unary)*
/// unary  = '-' unary | atom
/// atom   = NUMBER | CONST | '(' expr ')'
/// ```
pub fn evaluate(input: &str) -> Result<f64, CalcError> {
    let tokens = tokenize(input)?;
    let mut pos = 0;
    let result = parse_expr(&tokens, &mut pos)?;
    if pos != tokens.len() {
        return Err(CalcError::InvalidExpression);
    }
    Ok(result)
}

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Num(f64),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    LParen,
    RParen,
}

fn tokenize(input: &str) -> Result<Vec<Token>, CalcError> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            ' ' | '\t' => {
                i += 1;
            }
            '+' => {
                tokens.push(Token::Plus);
                i += 1;
            }
            '-' => {
                tokens.push(Token::Minus);
                i += 1;
            }
            '*' => {
                tokens.push(Token::Star);
                i += 1;
            }
            '/' => {
                tokens.push(Token::Slash);
                i += 1;
            }
            '%' => {
                tokens.push(Token::Percent);
                i += 1;
            }
            '^' => {
                tokens.push(Token::Caret);
                i += 1;
            }
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            c if c.is_ascii_digit() || c == '.' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let s: String = chars[start..i].iter().collect();
                let num: f64 = s.parse().map_err(|_| CalcError::InvalidExpression)?;
                tokens.push(Token::Num(num));
            }
            c if c.is_ascii_alphabetic() => {
                let start = i;
                while i < chars.len() && chars[i].is_ascii_alphabetic() {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                match word.to_lowercase().as_str() {
                    "pi" => tokens.push(Token::Num(std::f64::consts::PI)),
                    "e" => tokens.push(Token::Num(std::f64::consts::E)),
                    "tau" => tokens.push(Token::Num(std::f64::consts::TAU)),
                    _ => return Err(CalcError::UnexpectedChar(chars[start])),
                }
            }
            c => return Err(CalcError::UnexpectedChar(c)),
        }
    }

    Ok(tokens)
}

// ---------------------------------------------------------------------------
// Recursive descent parser
// ---------------------------------------------------------------------------

fn parse_expr(tokens: &[Token], pos: &mut usize) -> Result<f64, CalcError> {
    let mut left = parse_term(tokens, pos)?;
    while *pos < tokens.len() {
        match tokens[*pos] {
            Token::Plus => {
                *pos += 1;
                left += parse_term(tokens, pos)?;
            }
            Token::Minus => {
                *pos += 1;
                left -= parse_term(tokens, pos)?;
            }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_term(tokens: &[Token], pos: &mut usize) -> Result<f64, CalcError> {
    let mut left = parse_power(tokens, pos)?;
    while *pos < tokens.len() {
        match tokens[*pos] {
            Token::Star => {
                *pos += 1;
                left *= parse_power(tokens, pos)?;
            }
            Token::Slash => {
                *pos += 1;
                let right = parse_power(tokens, pos)?;
                if right == 0.0 {
                    return Err(CalcError::DivisionByZero);
                }
                left /= right;
            }
            Token::Percent => {
                *pos += 1;
                let right = parse_power(tokens, pos)?;
                if right == 0.0 {
                    return Err(CalcError::DivisionByZero);
                }
                left %= right;
            }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_power(tokens: &[Token], pos: &mut usize) -> Result<f64, CalcError> {
    let base = parse_unary(tokens, pos)?;
    if *pos < tokens.len() && tokens[*pos] == Token::Caret {
        *pos += 1;
        let exp = parse_unary(tokens, pos)?;
        Ok(base.powf(exp))
    } else {
        Ok(base)
    }
}

fn parse_unary(tokens: &[Token], pos: &mut usize) -> Result<f64, CalcError> {
    if *pos < tokens.len() && tokens[*pos] == Token::Minus {
        *pos += 1;
        let val = parse_unary(tokens, pos)?;
        Ok(-val)
    } else {
        parse_atom(tokens, pos)
    }
}

fn parse_atom(tokens: &[Token], pos: &mut usize) -> Result<f64, CalcError> {
    if *pos >= tokens.len() {
        return Err(CalcError::UnexpectedEnd);
    }
    match &tokens[*pos] {
        Token::Num(n) => {
            let v = *n;
            *pos += 1;
            Ok(v)
        }
        Token::LParen => {
            *pos += 1;
            let val = parse_expr(tokens, pos)?;
            if *pos >= tokens.len() || tokens[*pos] != Token::RParen {
                return Err(CalcError::UnbalancedParens);
            }
            *pos += 1;
            Ok(val)
        }
        _ => Err(CalcError::InvalidExpression),
    }
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

fn format_value(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        // Up to 10 decimal digits, trimming trailing zeros.
        let s = format!("{:.10}", v);
        let s = s.trim_end_matches('0');
        let s = s.trim_end_matches('.');
        s.to_string()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- looks_like_math ------------------------------------------------------

    #[test]
    fn detect_digit_with_op() {
        assert!(looks_like_math("2+3"));
        assert!(looks_like_math("42*10"));
    }

    #[test]
    fn detect_equals_prefix() {
        assert!(looks_like_math("=2+3"));
        assert!(looks_like_math("= 100 / 4"));
    }

    #[test]
    fn detect_parens() {
        assert!(looks_like_math("(1+2)*3"));
    }

    #[test]
    fn detect_constant() {
        assert!(looks_like_math("pi*2"));
        assert!(looks_like_math("tau+1"));
    }

    #[test]
    fn reject_plain_text() {
        assert!(!looks_like_math("hello world"));
        assert!(!looks_like_math("firefox"));
    }

    #[test]
    fn reject_empty() {
        assert!(!looks_like_math(""));
    }

    // -- evaluate: basic arithmetic -------------------------------------------

    #[test]
    fn eval_addition() {
        assert_eq!(evaluate("2+3").unwrap(), 5.0);
    }

    #[test]
    fn eval_subtraction() {
        assert_eq!(evaluate("10-4").unwrap(), 6.0);
    }

    #[test]
    fn eval_multiplication() {
        assert_eq!(evaluate("3*7").unwrap(), 21.0);
    }

    #[test]
    fn eval_division() {
        assert_eq!(evaluate("20/4").unwrap(), 5.0);
    }

    #[test]
    fn eval_modulo() {
        assert_eq!(evaluate("10%3").unwrap(), 1.0);
    }

    #[test]
    fn eval_exponent() {
        assert_eq!(evaluate("2^10").unwrap(), 1024.0);
    }

    // -- precedence -----------------------------------------------------------

    #[test]
    fn eval_precedence_mul_add() {
        assert_eq!(evaluate("2+3*4").unwrap(), 14.0);
    }

    #[test]
    fn eval_precedence_parens() {
        assert_eq!(evaluate("(2+3)*4").unwrap(), 20.0);
    }

    #[test]
    fn eval_nested_parens() {
        assert_eq!(evaluate("((2+3)*(4-1))").unwrap(), 15.0);
    }

    // -- unary minus ----------------------------------------------------------

    #[test]
    fn eval_unary_minus() {
        assert_eq!(evaluate("-5").unwrap(), -5.0);
    }

    #[test]
    fn eval_double_negation() {
        assert_eq!(evaluate("--5").unwrap(), 5.0);
    }

    #[test]
    fn eval_unary_in_expr() {
        assert_eq!(evaluate("3 + -2").unwrap(), 1.0);
    }

    // -- constants ------------------------------------------------------------

    #[test]
    fn eval_pi() {
        let val = evaluate("pi").unwrap();
        assert!((val - std::f64::consts::PI).abs() < 1e-10);
    }

    #[test]
    fn eval_e() {
        let val = evaluate("e").unwrap();
        assert!((val - std::f64::consts::E).abs() < 1e-10);
    }

    #[test]
    fn eval_tau() {
        let val = evaluate("tau").unwrap();
        assert!((val - std::f64::consts::TAU).abs() < 1e-10);
    }

    #[test]
    fn eval_pi_times_two() {
        let val = evaluate("pi * 2").unwrap();
        assert!((val - std::f64::consts::TAU).abs() < 1e-10);
    }

    // -- decimals -------------------------------------------------------------

    #[test]
    fn eval_decimal() {
        assert_eq!(evaluate("1.5 + 2.5").unwrap(), 4.0);
    }

    #[test]
    fn eval_leading_dot() {
        assert_eq!(evaluate(".5 + .5").unwrap(), 1.0);
    }

    // -- whitespace -----------------------------------------------------------

    #[test]
    fn eval_whitespace() {
        assert_eq!(evaluate("  2  +  3  ").unwrap(), 5.0);
    }

    // -- error cases ----------------------------------------------------------

    #[test]
    fn eval_division_by_zero() {
        assert_eq!(evaluate("1/0"), Err(CalcError::DivisionByZero));
    }

    #[test]
    fn eval_modulo_by_zero() {
        assert_eq!(evaluate("5%0"), Err(CalcError::DivisionByZero));
    }

    #[test]
    fn eval_unbalanced_parens() {
        assert!(evaluate("(2+3").is_err());
    }

    #[test]
    fn eval_unexpected_char() {
        assert!(evaluate("2 & 3").is_err());
    }

    #[test]
    fn eval_empty() {
        assert!(evaluate("").is_err());
    }

    #[test]
    fn eval_unknown_word() {
        assert!(evaluate("foo + 1").is_err());
    }

    // -- format_value ---------------------------------------------------------

    #[test]
    fn format_integer() {
        assert_eq!(format_value(42.0), "42");
    }

    #[test]
    fn format_decimal() {
        assert_eq!(format_value(3.14), "3.14");
    }

    #[test]
    fn format_trailing_zeros_trimmed() {
        assert_eq!(format_value(1.50), "1.5");
    }

    #[test]
    fn format_negative() {
        assert_eq!(format_value(-7.0), "-7");
    }

    // -- CalculatorProvider integration ---------------------------------------

    #[test]
    fn provider_metadata() {
        let p = CalculatorProvider::new();
        assert_eq!(p.id(), "calculator");
        assert_eq!(p.priority(), 95);
    }

    #[test]
    fn provider_evaluates_expression() {
        let p = CalculatorProvider::new();
        let r = p.search("2+2", 5);
        assert_eq!(r.len(), 1);
        assert!(r[0].title.contains("4"));
        assert_eq!(r[0].category, SearchCategory::Calculator);
    }

    #[test]
    fn provider_equals_prefix() {
        let p = CalculatorProvider::new();
        let r = p.search("=10*5", 5);
        assert_eq!(r.len(), 1);
        assert!(r[0].title.contains("50"));
    }

    #[test]
    fn provider_rejects_plain_text() {
        let p = CalculatorProvider::new();
        assert!(p.search("firefox", 5).is_empty());
    }

    #[test]
    fn provider_rejects_invalid_expr() {
        let p = CalculatorProvider::new();
        assert!(p.search("=abc", 5).is_empty());
    }

    #[test]
    fn provider_empty_query() {
        let p = CalculatorProvider::new();
        assert!(p.search("", 5).is_empty());
    }
}
