//! CSS math expression parsing — `calc()`, `min()`, `max()`, `clamp()`.
//!
//! Parses CSS math functions into a `CssMathExpr` AST with correct operator
//! precedence (additive < multiplicative) and parenthesised sub-expressions.

use crate::value::PropertyValue;

use super::ThemeParser;

impl ThemeParser {
    /// Parse a CSS math expression string into a `CssMathExpr`.
    pub(crate) fn parse_math_expr(&self, s: &str) -> Option<crate::value::CssMathExpr> {
        let s = s.trim();
        if let Some(inner) = Self::strip_function(s, "calc") {
            return self.parse_calc_expr(inner);
        }
        if let Some(inner) = Self::strip_function(s, "min") {
            let exprs = self.parse_function_expr_args(inner)?;
            return Some(crate::value::CssMathExpr::Min(exprs));
        }
        if let Some(inner) = Self::strip_function(s, "max") {
            let exprs = self.parse_function_expr_args(inner)?;
            return Some(crate::value::CssMathExpr::Max(exprs));
        }
        if let Some(inner) = Self::strip_function(s, "clamp") {
            let exprs = self.parse_function_expr_args(inner)?;
            if exprs.len() == 3 {
                let mut exprs = exprs.into_iter();
                let min = exprs.next()?;
                let pref = exprs.next()?;
                let max = exprs.next()?;
                return Some(crate::value::CssMathExpr::Clamp {
                    min: Box::new(min),
                    preferred: Box::new(pref),
                    max: Box::new(max),
                });
            }
        }
        None
    }

    /// Parse the inside of a `calc(...)` expression (supports +, -, *, /).
    fn parse_calc_expr(&self, s: &str) -> Option<crate::value::CssMathExpr> {
        let s = s.trim();
        // Try to split on + or - at the top level (outside parens).
        // Addition/subtraction are the lowest precedence operators.
        if let Some((left, op, right)) = Self::split_additive(s) {
            let lhs = self.parse_calc_term(left.trim())?;
            let rhs = self.parse_calc_term(right.trim())?;
            return Some(if op == '+' {
                crate::value::CssMathExpr::Add(Box::new(lhs), Box::new(rhs))
            } else {
                crate::value::CssMathExpr::Sub(Box::new(lhs), Box::new(rhs))
            });
        }
        self.parse_calc_term(s)
    }

    /// Parse a multiplicative term (handles * and /).
    fn parse_calc_term(&self, s: &str) -> Option<crate::value::CssMathExpr> {
        let s = s.trim();
        if let Some((left, op, right)) = Self::split_multiplicative(s) {
            let lhs = self.parse_calc_atom(left.trim())?;
            let rhs = self.parse_calc_atom(right.trim())?;
            return Some(if op == '*' {
                crate::value::CssMathExpr::Mul(Box::new(lhs), Box::new(rhs))
            } else {
                crate::value::CssMathExpr::Div(Box::new(lhs), Box::new(rhs))
            });
        }
        self.parse_calc_atom(s)
    }

    /// Parse a calc atom: a number, length, parenthesized sub-expression, or nested function.
    fn parse_calc_atom(&self, s: &str) -> Option<crate::value::CssMathExpr> {
        let s = s.trim();
        // Nested function (calc, min, max, clamp)
        if s.starts_with("calc(")
            || s.starts_with("min(")
            || s.starts_with("max(")
            || s.starts_with("clamp(")
        {
            return self.parse_math_expr(s);
        }
        // Parenthesized sub-expression
        if s.starts_with('(') && s.ends_with(')') {
            return self.parse_calc_expr(&s[1..s.len() - 1]);
        }
        // Try as length
        if let Some(pv) = self.parse_explicit_length_value(s) {
            if let PropertyValue::Length(lu) = pv {
                return Some(crate::value::CssMathExpr::Value(lu));
            }
        }
        // Try as bare number
        if let Ok(n) = s.parse::<f32>() {
            return Some(crate::value::CssMathExpr::Number(n));
        }
        None
    }

    /// Strip a function wrapper: e.g. `calc(100% - 20px)` → `100% - 20px`.
    pub(crate) fn strip_function<'a>(s: &'a str, name: &str) -> Option<&'a str> {
        let s = s.trim();
        if s.len() > name.len() + 1
            && s[..name.len()].eq_ignore_ascii_case(name)
            && s[name.len()..].starts_with('(')
            && s.ends_with(')')
        {
            Some(&s[name.len() + 1..s.len() - 1])
        } else {
            None
        }
    }

    fn parse_function_expr_args(&self, s: &str) -> Option<Vec<crate::value::CssMathExpr>> {
        let args = Self::split_function_args(s);
        if args.is_empty() || args.iter().any(|arg| arg.trim().is_empty()) {
            return None;
        }

        args.into_iter()
            .map(|arg| self.parse_calc_expr(arg.trim()))
            .collect()
    }

    /// Split function arguments by commas at the top level (respecting nested parens).
    fn split_function_args(s: &str) -> Vec<String> {
        let mut args = Vec::new();
        let mut current = String::new();
        let mut depth = 0;
        for ch in s.chars() {
            match ch {
                '(' => {
                    depth += 1;
                    current.push(ch);
                }
                ')' => {
                    depth -= 1;
                    current.push(ch);
                }
                ',' if depth == 0 => {
                    args.push(std::mem::take(&mut current));
                }
                _ => current.push(ch),
            }
        }
        if !current.is_empty() {
            args.push(current);
        }
        args
    }

    /// Split on the *last* top-level `+` or `-` (lowest precedence, left-associative).
    /// We scan right-to-left, but the `-` must be preceded by a space to differentiate
    /// from negative numbers (e.g. `-20px`).
    fn split_additive(s: &str) -> Option<(&str, char, &str)> {
        let bytes = s.as_bytes();
        let mut depth: i32 = 0;
        // Scan right to left
        let mut i = bytes.len();
        while i > 0 {
            i -= 1;
            match bytes[i] {
                b')' => depth += 1,
                b'(' => depth -= 1,
                b'+' if depth == 0 && i > 0 => {
                    // Require whitespace around operator for calc
                    return Some((&s[..i].trim_end(), '+', &s[i + 1..]));
                }
                b'-' if depth == 0 && i > 0 && bytes[i - 1] == b' ' => {
                    return Some((&s[..i].trim_end(), '-', &s[i + 1..]));
                }
                _ => {}
            }
        }
        None
    }

    /// Split on the *last* top-level `*` or `/`.
    fn split_multiplicative(s: &str) -> Option<(&str, char, &str)> {
        let bytes = s.as_bytes();
        let mut depth: i32 = 0;
        let mut i = bytes.len();
        while i > 0 {
            i -= 1;
            match bytes[i] {
                b')' => depth += 1,
                b'(' => depth -= 1,
                b'*' if depth == 0 => {
                    return Some((&s[..i], '*', &s[i + 1..]));
                }
                b'/' if depth == 0 => {
                    return Some((&s[..i], '/', &s[i + 1..]));
                }
                _ => {}
            }
        }
        None
    }
}
