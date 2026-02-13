use crate::calculator::*;

// ---------------------------------------------------------------------------
// evaluate() — basic arithmetic
// ---------------------------------------------------------------------------

#[test]
fn calculator_addition() {
    let result = evaluate("2+3");
    assert_eq!(result, CalcResult::Number(5.0));
}

#[test]
fn calculator_subtraction() {
    let result = evaluate("10-4");
    assert_eq!(result, CalcResult::Number(6.0));
}

#[test]
fn calculator_multiplication() {
    let result = evaluate("3*7");
    assert_eq!(result, CalcResult::Number(21.0));
}

#[test]
fn calculator_division() {
    let result = evaluate("20/4");
    assert_eq!(result, CalcResult::Number(5.0));
}

// ---------------------------------------------------------------------------
// evaluate() — operator precedence
// ---------------------------------------------------------------------------

#[test]
fn calculator_operator_precedence() {
    // Multiplication binds tighter than addition: 2 + 3*4 = 2 + 12 = 14
    let result = evaluate("2+3*4");
    assert_eq!(result, CalcResult::Number(14.0));
}

#[test]
fn calculator_operator_precedence_subtraction() {
    // 10 - 2*3 = 10 - 6 = 4
    let result = evaluate("10-2*3");
    assert_eq!(result, CalcResult::Number(4.0));
}

// ---------------------------------------------------------------------------
// evaluate() — parentheses
// ---------------------------------------------------------------------------

#[test]
fn calculator_parentheses() {
    let result = evaluate("(2+3)*4");
    assert_eq!(result, CalcResult::Number(20.0));
}

#[test]
fn calculator_nested_parentheses() {
    // ((1+2)*(3+4)) = 3 * 7 = 21
    let result = evaluate("((1+2)*(3+4))");
    assert_eq!(result, CalcResult::Number(21.0));
}

// ---------------------------------------------------------------------------
// evaluate() — power operator (right-associative)
// ---------------------------------------------------------------------------

#[test]
fn calculator_power() {
    let result = evaluate("2^3");
    assert_eq!(result, CalcResult::Number(8.0));
}

#[test]
fn calculator_power_right_associative() {
    // 2^3^2 = 2^(3^2) = 2^9 = 512
    let result = evaluate("2^3^2");
    assert_eq!(result, CalcResult::Number(512.0));
}

// ---------------------------------------------------------------------------
// evaluate() — modulo
// ---------------------------------------------------------------------------

#[test]
fn calculator_modulo() {
    let result = evaluate("10%3");
    assert_eq!(result, CalcResult::Number(1.0));
}

// ---------------------------------------------------------------------------
// tokenize()
// ---------------------------------------------------------------------------

#[test]
fn tokenize_simple_expression() {
    let tokens = tokenize("2+3").unwrap();
    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens[0], CalcToken::Number(2.0));
    assert_eq!(tokens[1], CalcToken::Op('+'));
    assert_eq!(tokens[2], CalcToken::Number(3.0));
}

#[test]
fn tokenize_whitespace_ignored() {
    let tokens = tokenize("  2  +  3  ").unwrap();
    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens[0], CalcToken::Number(2.0));
    assert_eq!(tokens[1], CalcToken::Op('+'));
    assert_eq!(tokens[2], CalcToken::Number(3.0));
}

#[test]
fn tokenize_parentheses() {
    let tokens = tokenize("(1+2)").unwrap();
    assert_eq!(tokens.len(), 5);
    assert_eq!(tokens[0], CalcToken::LParen);
    assert_eq!(tokens[4], CalcToken::RParen);
}

#[test]
fn tokenize_function() {
    let tokens = tokenize("sqrt(9)").unwrap();
    assert_eq!(tokens.len(), 4);
    assert_eq!(tokens[0], CalcToken::Func("sqrt".into()));
    assert_eq!(tokens[1], CalcToken::LParen);
    assert_eq!(tokens[2], CalcToken::Number(9.0));
    assert_eq!(tokens[3], CalcToken::RParen);
}

#[test]
fn tokenize_all_operators() {
    let tokens = tokenize("+-*/%^").unwrap();
    assert_eq!(tokens.len(), 6);
    assert_eq!(tokens[0], CalcToken::Op('+'));
    assert_eq!(tokens[1], CalcToken::Op('-'));
    assert_eq!(tokens[2], CalcToken::Op('*'));
    assert_eq!(tokens[3], CalcToken::Op('/'));
    assert_eq!(tokens[4], CalcToken::Op('%'));
    assert_eq!(tokens[5], CalcToken::Op('^'));
}

// ---------------------------------------------------------------------------
// Functions
// ---------------------------------------------------------------------------

#[test]
fn calculator_sqrt() {
    let result = evaluate("sqrt(9)");
    assert_eq!(result, CalcResult::Number(3.0));
}

#[test]
fn calculator_sin_zero() {
    if let CalcResult::Number(val) = evaluate("sin(0)") {
        assert!((val - 0.0).abs() < 1e-6, "sin(0) should be 0, got {val}");
    } else {
        panic!("expected Number result");
    }
}

#[test]
fn calculator_cos_zero() {
    if let CalcResult::Number(val) = evaluate("cos(0)") {
        assert!((val - 1.0).abs() < 1e-6, "cos(0) should be 1, got {val}");
    } else {
        panic!("expected Number result");
    }
}

#[test]
fn calculator_tan_zero() {
    if let CalcResult::Number(val) = evaluate("tan(0)") {
        assert!((val - 0.0).abs() < 1e-6, "tan(0) should be 0, got {val}");
    } else {
        panic!("expected Number result");
    }
}

#[test]
fn calculator_log_100() {
    if let CalcResult::Number(val) = evaluate("log(100)") {
        assert!((val - 2.0).abs() < 1e-6, "log(100) should be 2, got {val}");
    } else {
        panic!("expected Number result");
    }
}

#[test]
fn calculator_ln_e() {
    // ln(e^1) = 1; we approximate e as 2.718281828
    if let CalcResult::Number(val) = evaluate("ln(2.718281828)") {
        assert!(
            (val - 1.0).abs() < 1e-6,
            "ln(e) should be approximately 1, got {val}"
        );
    } else {
        panic!("expected Number result");
    }
}

#[test]
fn calculator_abs_negative() {
    let result = evaluate("abs(-5)");
    assert_eq!(result, CalcResult::Number(5.0));
}

#[test]
fn calculator_unknown_function() {
    let result = evaluate("foobar(1)");
    match result {
        CalcResult::Error(msg) => assert!(
            msg.contains("unknown function"),
            "expected 'unknown function' error, got: {msg}"
        ),
        other => panic!("expected Error, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Unary operators
// ---------------------------------------------------------------------------

#[test]
fn calculator_unary_minus() {
    let result = evaluate("-5");
    assert_eq!(result, CalcResult::Number(-5.0));
}

#[test]
fn calculator_unary_plus() {
    let result = evaluate("+5");
    assert_eq!(result, CalcResult::Number(5.0));
}

#[test]
fn calculator_double_negate() {
    // -(-3) = 3
    let result = evaluate("-(-3)");
    assert_eq!(result, CalcResult::Number(3.0));
}

// ---------------------------------------------------------------------------
// convert_units()
// ---------------------------------------------------------------------------

#[test]
fn convert_f_to_c_freezing() {
    let result = convert_units(32.0, "F", "C").unwrap();
    assert!((result - 0.0).abs() < 1e-6, "32F should be 0C, got {result}");
}

#[test]
fn convert_f_to_c_boiling() {
    let result = convert_units(212.0, "F", "C").unwrap();
    assert!(
        (result - 100.0).abs() < 1e-6,
        "212F should be 100C, got {result}"
    );
}

#[test]
fn convert_c_to_f() {
    let result = convert_units(100.0, "C", "F").unwrap();
    assert!(
        (result - 212.0).abs() < 1e-6,
        "100C should be 212F, got {result}"
    );
}

#[test]
fn convert_c_to_k() {
    let result = convert_units(0.0, "C", "K").unwrap();
    assert!(
        (result - 273.15).abs() < 1e-6,
        "0C should be 273.15K, got {result}"
    );
}

#[test]
fn convert_k_to_c() {
    let result = convert_units(273.15, "K", "C").unwrap();
    assert!(
        (result - 0.0).abs() < 1e-6,
        "273.15K should be 0C, got {result}"
    );
}

#[test]
fn convert_km_to_mi() {
    let result = convert_units(1.0, "km", "mi").unwrap();
    assert!(
        (result - 0.621371).abs() < 1e-4,
        "1km should be ~0.621mi, got {result}"
    );
}

#[test]
fn convert_mi_to_km() {
    let result = convert_units(1.0, "mi", "km").unwrap();
    assert!(
        (result - 1.609344).abs() < 1e-4,
        "1mi should be ~1.609km, got {result}"
    );
}

#[test]
fn convert_kg_to_lb() {
    let result = convert_units(1.0, "kg", "lb").unwrap();
    assert!(
        (result - 2.204623).abs() < 1e-4,
        "1kg should be ~2.205lb, got {result}"
    );
}

#[test]
fn convert_lb_to_kg() {
    let result = convert_units(1.0, "lb", "kg").unwrap();
    assert!(
        (result - 0.453592).abs() < 1e-4,
        "1lb should be ~0.454kg, got {result}"
    );
}

#[test]
fn convert_unsupported_pair_returns_none() {
    assert!(convert_units(1.0, "m", "ft").is_none());
}

// ---------------------------------------------------------------------------
// Error cases
// ---------------------------------------------------------------------------

#[test]
fn calculator_division_by_zero() {
    let result = evaluate("1/0");
    match result {
        CalcResult::Error(msg) => assert!(
            msg.contains("division by zero"),
            "expected 'division by zero', got: {msg}"
        ),
        other => panic!("expected Error, got {other:?}"),
    }
}

#[test]
fn calculator_modulo_by_zero() {
    let result = evaluate("5%0");
    match result {
        CalcResult::Error(msg) => assert!(
            msg.contains("modulo by zero"),
            "expected 'modulo by zero', got: {msg}"
        ),
        other => panic!("expected Error, got {other:?}"),
    }
}

#[test]
fn calculator_empty_expression() {
    let result = evaluate("");
    match result {
        CalcResult::Error(msg) => assert!(
            msg.contains("empty expression"),
            "expected 'empty expression', got: {msg}"
        ),
        other => panic!("expected Error, got {other:?}"),
    }
}

#[test]
fn calculator_missing_closing_paren() {
    let result = evaluate("(2+3");
    match result {
        CalcResult::Error(msg) => assert!(
            msg.contains("missing closing parenthesis"),
            "expected 'missing closing parenthesis', got: {msg}"
        ),
        other => panic!("expected Error, got {other:?}"),
    }
}

#[test]
fn calculator_unexpected_token() {
    let result = evaluate("2+3)");
    match result {
        CalcResult::Error(msg) => assert!(
            msg.contains("unexpected token"),
            "expected 'unexpected token', got: {msg}"
        ),
        other => panic!("expected Error, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// CalcResult Display impl
// ---------------------------------------------------------------------------

#[test]
fn calc_result_display_number() {
    let r = CalcResult::Number(42.0);
    assert_eq!(format!("{r}"), "42");
}

#[test]
fn calc_result_display_conversion() {
    let r = CalcResult::Conversion {
        value: 32.0,
        from_unit: "F".into(),
        to_unit: "C".into(),
        result: 0.0,
    };
    assert_eq!(format!("{r}"), "32 F = 0 C");
}

#[test]
fn calc_result_display_error() {
    let r = CalcResult::Error("test error".into());
    assert_eq!(format!("{r}"), "Error: test error");
}
