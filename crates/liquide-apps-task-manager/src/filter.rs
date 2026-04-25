//! Search and filter types for the task manager.
//!
//! Provides filter expression AST, comparison operators, quick filters,
//! filter presets, and a simple parser for textual filter expressions.
//! Corresponds to spec section 4.7.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A composable filter expression tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterExpr {
    /// All sub-expressions must match.
    And(Vec<FilterExpr>),
    /// At least one sub-expression must match.
    Or(Vec<FilterExpr>),
    /// Negates the inner expression.
    Not(Box<FilterExpr>),
    /// A field-level comparison.
    Comparison {
        /// Column key to compare (e.g. `"cpu_percent"`).
        field: String,
        /// Comparison operator.
        op: CompareOp,
        /// Value to compare against.
        value: FilterValue,
    },
    /// Free-text search matching across name, PID, command line, path, user.
    FreeText(String),
}

/// Comparison operators for filter expressions.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompareOp {
    /// Equal.
    Eq,
    /// Not equal.
    NotEq,
    /// Greater than.
    Gt,
    /// Greater than or equal.
    Gte,
    /// Less than.
    Lt,
    /// Less than or equal.
    Lte,
    /// String contains substring.
    Contains,
    /// String does not contain substring.
    NotContains,
    /// String starts with prefix.
    StartsWith,
    /// String ends with suffix.
    EndsWith,
}

impl CompareOp {
    /// Return a human-readable name for this operator.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Eq => "Eq",
            Self::NotEq => "NotEq",
            Self::Gt => "Gt",
            Self::Gte => "Gte",
            Self::Lt => "Lt",
            Self::Lte => "Lte",
            Self::Contains => "Contains",
            Self::NotContains => "NotContains",
            Self::StartsWith => "StartsWith",
            Self::EndsWith => "EndsWith",
        }
    }
}

impl fmt::Display for CompareOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A typed value used on the right-hand side of a comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterValue {
    /// A text string.
    Text(String),
    /// A numeric value.
    Number(f64),
    /// A boolean value.
    Bool(bool),
}

/// Built-in quick-filter presets toggled via the UI toolbar.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuickFilter {
    /// Show only application processes.
    Apps,
    /// Show only background processes.
    Background,
    /// Show only system processes.
    System,
    /// Show only elevated / administrator processes.
    Elevated,
    /// Show only processes that are not responding.
    NotResponding,
}

impl QuickFilter {
    /// Return a human-readable name for this quick filter.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Apps => "Apps",
            Self::Background => "Background",
            Self::System => "System",
            Self::Elevated => "Elevated",
            Self::NotResponding => "Not Responding",
        }
    }
}

impl fmt::Display for QuickFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A named, saveable filter preset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterPreset {
    /// Human-readable preset name.
    pub name: String,
    /// The filter expression this preset applies.
    pub expression: FilterExpr,
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Parse a textual filter expression into a [`FilterExpr`].
///
/// Supported syntax:
/// - Simple comparison: `cpu_percent > 10`
/// - Boolean combinators: `cpu_percent > 10 AND user = "admin"`
/// - Negation: `NOT status = "zombie"`
/// - Free text (a plain string without operators): `firefox`
///
/// Returns `Err` for empty or malformed input.
pub fn parse_filter(input: &str) -> Result<FilterExpr, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("empty filter expression".to_string());
    }
    parse_or(trimmed)
}

/// Parse OR-separated sub-expressions.
fn parse_or(input: &str) -> Result<FilterExpr, String> {
    let parts = split_keyword(input, "OR");
    if parts.len() == 1 {
        return parse_and(parts[0].trim());
    }
    let mut exprs = Vec::new();
    for part in &parts {
        exprs.push(parse_and(part.trim())?);
    }
    Ok(FilterExpr::Or(exprs))
}

/// Parse AND-separated sub-expressions.
fn parse_and(input: &str) -> Result<FilterExpr, String> {
    let parts = split_keyword(input, "AND");
    if parts.len() == 1 {
        return parse_unary(parts[0].trim());
    }
    let mut exprs = Vec::new();
    for part in &parts {
        exprs.push(parse_unary(part.trim())?);
    }
    Ok(FilterExpr::And(exprs))
}

/// Parse a NOT prefix or a primary expression.
fn parse_unary(input: &str) -> Result<FilterExpr, String> {
    if let Some(rest) = strip_keyword_prefix(input, "NOT") {
        let inner = parse_unary(rest.trim())?;
        return Ok(FilterExpr::Not(Box::new(inner)));
    }
    parse_primary(input)
}

/// Parse a single comparison or free-text term.
fn parse_primary(input: &str) -> Result<FilterExpr, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("empty sub-expression".to_string());
    }

    // Try to find a comparison operator in the input.
    // Order matters: check multi-char operators before single-char ones.
    let operators: &[(&str, CompareOp)] = &[
        ("!=", CompareOp::NotEq),
        (">=", CompareOp::Gte),
        ("<=", CompareOp::Lte),
        (">", CompareOp::Gt),
        ("<", CompareOp::Lt),
        ("=", CompareOp::Eq),
    ];

    for &(token, ref op) in operators {
        if let Some(pos) = find_operator(trimmed, token) {
            let field = trimmed[..pos].trim().to_string();
            let raw_value = trimmed[pos + token.len()..].trim();
            if field.is_empty() {
                return Err(format!("missing field name before '{token}'"));
            }
            if raw_value.is_empty() {
                return Err(format!("missing value after '{token}'"));
            }
            let value = parse_value(raw_value)?;
            return Ok(FilterExpr::Comparison {
                field,
                op: op.clone(),
                value,
            });
        }
    }

    // No operator found — treat as free text.
    Ok(FilterExpr::FreeText(trimmed.to_string()))
}

/// Parse a right-hand-side value.
fn parse_value(raw: &str) -> Result<FilterValue, String> {
    // Quoted string.
    if raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2 {
        let inner = &raw[1..raw.len() - 1];
        return Ok(FilterValue::Text(inner.to_string()));
    }

    // Boolean.
    if raw.eq_ignore_ascii_case("true") {
        return Ok(FilterValue::Bool(true));
    }
    if raw.eq_ignore_ascii_case("false") {
        return Ok(FilterValue::Bool(false));
    }

    // Number.
    if let Ok(n) = raw.parse::<f64>() {
        return Ok(FilterValue::Number(n));
    }

    // Fall back to unquoted text.
    Ok(FilterValue::Text(raw.to_string()))
}

/// Split `input` on a keyword boundary (the keyword must be surrounded by
/// whitespace to avoid splitting inside identifiers).
fn split_keyword<'a>(input: &'a str, keyword: &str) -> Vec<&'a str> {
    let mut parts = Vec::new();
    let mut remaining = input;

    loop {
        if let Some(pos) = find_keyword(remaining, keyword) {
            parts.push(&remaining[..pos]);
            remaining = &remaining[pos + keyword.len()..];
        } else {
            parts.push(remaining);
            break;
        }
    }

    parts
}

/// Find the byte offset of a keyword that is surrounded by whitespace.
fn find_keyword(input: &str, keyword: &str) -> Option<usize> {
    let mut start = 0;
    while let Some(pos) = input[start..].find(keyword) {
        let abs = start + pos;
        let before_ok = abs == 0 || input.as_bytes()[abs - 1].is_ascii_whitespace();
        let after = abs + keyword.len();
        let after_ok = after >= input.len() || input.as_bytes()[after].is_ascii_whitespace();
        if before_ok && after_ok {
            return Some(abs);
        }
        start = abs + 1;
    }
    None
}

/// Strip a keyword prefix if it appears at the start followed by whitespace.
fn strip_keyword_prefix<'a>(input: &'a str, keyword: &str) -> Option<&'a str> {
    if input.len() > keyword.len()
        && input[..keyword.len()].eq_ignore_ascii_case(keyword)
        && input.as_bytes()[keyword.len()].is_ascii_whitespace()
    {
        Some(&input[keyword.len()..])
    } else {
        None
    }
}

/// Find the position of a comparison operator, ignoring occurrences inside
/// quoted strings.
fn find_operator(input: &str, op: &str) -> Option<usize> {
    let bytes = input.as_bytes();
    let op_bytes = op.as_bytes();
    let mut in_quotes = false;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            in_quotes = !in_quotes;
            i += 1;
            continue;
        }
        if !in_quotes
            && i + op_bytes.len() <= bytes.len()
            && &bytes[i..i + op_bytes.len()] == op_bytes
        {
            return Some(i);
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_free_text() {
        let expr = parse_filter("firefox").unwrap();
        match expr {
            FilterExpr::FreeText(s) => assert_eq!(s, "firefox"),
            other => panic!("expected FreeText, got {:?}", other),
        }
    }

    #[test]
    fn test_simple_comparison_number() {
        let expr = parse_filter("cpu_percent > 10").unwrap();
        match expr {
            FilterExpr::Comparison { field, op, value } => {
                assert_eq!(field, "cpu_percent");
                assert_eq!(op, CompareOp::Gt);
                match value {
                    FilterValue::Number(n) => assert!((n - 10.0).abs() < f64::EPSILON),
                    other => panic!("expected Number, got {:?}", other),
                }
            }
            other => panic!("expected Comparison, got {:?}", other),
        }
    }

    #[test]
    fn test_comparison_quoted_string() {
        let expr = parse_filter("user = \"admin\"").unwrap();
        match expr {
            FilterExpr::Comparison { field, op, value } => {
                assert_eq!(field, "user");
                assert_eq!(op, CompareOp::Eq);
                match value {
                    FilterValue::Text(s) => assert_eq!(s, "admin"),
                    other => panic!("expected Text, got {:?}", other),
                }
            }
            other => panic!("expected Comparison, got {:?}", other),
        }
    }

    #[test]
    fn test_and_expression() {
        let expr = parse_filter("cpu_percent > 10 AND user = \"admin\"").unwrap();
        match expr {
            FilterExpr::And(parts) => assert_eq!(parts.len(), 2),
            other => panic!("expected And, got {:?}", other),
        }
    }

    #[test]
    fn test_or_expression() {
        let expr = parse_filter("status = \"running\" OR status = \"sleeping\"").unwrap();
        match expr {
            FilterExpr::Or(parts) => assert_eq!(parts.len(), 2),
            other => panic!("expected Or, got {:?}", other),
        }
    }

    #[test]
    fn test_not_expression() {
        let expr = parse_filter("NOT status = \"zombie\"").unwrap();
        match expr {
            FilterExpr::Not(inner) => match *inner {
                FilterExpr::Comparison {
                    ref field, ref op, ..
                } => {
                    assert_eq!(field, "status");
                    assert_eq!(*op, CompareOp::Eq);
                }
                other => panic!("expected Comparison inside Not, got {:?}", other),
            },
            other => panic!("expected Not, got {:?}", other),
        }
    }

    #[test]
    fn test_bool_value() {
        let expr = parse_filter("elevated = true").unwrap();
        match expr {
            FilterExpr::Comparison { value, .. } => match value {
                FilterValue::Bool(b) => assert!(b),
                other => panic!("expected Bool, got {:?}", other),
            },
            other => panic!("expected Comparison, got {:?}", other),
        }
    }

    #[test]
    fn test_empty_input() {
        assert!(parse_filter("").is_err());
        assert!(parse_filter("   ").is_err());
    }
}
