//! Tests for `filter` module types.

use liquide_apps_task_manager::filter::*;

// ---------------------------------------------------------------------------
// CompareOp
// ---------------------------------------------------------------------------

#[test]
fn compare_op_all_variants() {
    let variants = [
        CompareOp::Eq,
        CompareOp::NotEq,
        CompareOp::Gt,
        CompareOp::Gte,
        CompareOp::Lt,
        CompareOp::Lte,
        CompareOp::Contains,
        CompareOp::NotContains,
        CompareOp::StartsWith,
        CompareOp::EndsWith,
    ];
    assert_eq!(variants.len(), 10);
}

#[test]
fn compare_op_display() {
    assert_eq!(CompareOp::Eq.to_string(), "Eq");
    assert_eq!(CompareOp::NotEq.to_string(), "NotEq");
    assert_eq!(CompareOp::Gt.to_string(), "Gt");
    assert_eq!(CompareOp::Contains.to_string(), "Contains");
}

#[test]
fn compare_op_serde_roundtrip() {
    let val = CompareOp::Gte;
    let json = serde_json::to_string(&val).unwrap();
    let back: CompareOp = serde_json::from_str(&json).unwrap();
    assert_eq!(back, val);
}

// ---------------------------------------------------------------------------
// QuickFilter
// ---------------------------------------------------------------------------

#[test]
fn quick_filter_all_variants() {
    let variants = [
        QuickFilter::Apps,
        QuickFilter::Background,
        QuickFilter::System,
        QuickFilter::Elevated,
        QuickFilter::NotResponding,
    ];
    assert_eq!(variants.len(), 5);
}

#[test]
fn quick_filter_display() {
    assert_eq!(QuickFilter::Apps.to_string(), "Apps");
    assert_eq!(QuickFilter::Background.to_string(), "Background");
    assert_eq!(QuickFilter::NotResponding.to_string(), "Not Responding");
}

// ---------------------------------------------------------------------------
// FilterValue
// ---------------------------------------------------------------------------

#[test]
fn filter_value_text() {
    let v = FilterValue::Text("hello".into());
    let json = serde_json::to_string(&v).unwrap();
    let back: FilterValue = serde_json::from_str(&json).unwrap();
    match back {
        FilterValue::Text(s) => assert_eq!(s, "hello"),
        _ => panic!("expected Text"),
    }
}

#[test]
fn filter_value_number() {
    let v = FilterValue::Number(42.5);
    let json = serde_json::to_string(&v).unwrap();
    let back: FilterValue = serde_json::from_str(&json).unwrap();
    match back {
        FilterValue::Number(n) => assert!((n - 42.5).abs() < f64::EPSILON),
        _ => panic!("expected Number"),
    }
}

#[test]
fn filter_value_bool() {
    let v = FilterValue::Bool(true);
    let json = serde_json::to_string(&v).unwrap();
    let back: FilterValue = serde_json::from_str(&json).unwrap();
    match back {
        FilterValue::Bool(b) => assert!(b),
        _ => panic!("expected Bool"),
    }
}

// ---------------------------------------------------------------------------
// FilterExpr – construction
// ---------------------------------------------------------------------------

#[test]
fn filter_expr_free_text() {
    let expr = FilterExpr::FreeText("firefox".into());
    let json = serde_json::to_string(&expr).unwrap();
    let back: FilterExpr = serde_json::from_str(&json).unwrap();
    match back {
        FilterExpr::FreeText(s) => assert_eq!(s, "firefox"),
        _ => panic!("expected FreeText"),
    }
}

#[test]
fn filter_expr_comparison() {
    let expr = FilterExpr::Comparison {
        field: "cpu_percent".into(),
        op: CompareOp::Gt,
        value: FilterValue::Number(10.0),
    };
    let json = serde_json::to_string(&expr).unwrap();
    let _back: FilterExpr = serde_json::from_str(&json).unwrap();
}

#[test]
fn filter_expr_and() {
    let expr = FilterExpr::And(vec![
        FilterExpr::FreeText("test".into()),
        FilterExpr::FreeText("other".into()),
    ]);
    match expr {
        FilterExpr::And(parts) => assert_eq!(parts.len(), 2),
        _ => panic!("expected And"),
    }
}

#[test]
fn filter_expr_or() {
    let expr = FilterExpr::Or(vec![
        FilterExpr::FreeText("a".into()),
        FilterExpr::FreeText("b".into()),
    ]);
    match expr {
        FilterExpr::Or(parts) => assert_eq!(parts.len(), 2),
        _ => panic!("expected Or"),
    }
}

#[test]
fn filter_expr_not() {
    let inner = FilterExpr::FreeText("test".into());
    let expr = FilterExpr::Not(Box::new(inner));
    match expr {
        FilterExpr::Not(_) => {}
        _ => panic!("expected Not"),
    }
}

// ---------------------------------------------------------------------------
// parse_filter – free text
// ---------------------------------------------------------------------------

#[test]
fn parse_free_text() {
    let expr = parse_filter("firefox").unwrap();
    match expr {
        FilterExpr::FreeText(s) => assert_eq!(s, "firefox"),
        other => panic!("expected FreeText, got {:?}", other),
    }
}

#[test]
fn parse_free_text_with_spaces() {
    let expr = parse_filter("my process").unwrap();
    match expr {
        FilterExpr::FreeText(s) => assert_eq!(s, "my process"),
        other => panic!("expected FreeText, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// parse_filter – comparisons
// ---------------------------------------------------------------------------

#[test]
fn parse_comparison_gt_number() {
    let expr = parse_filter("cpu_percent > 10").unwrap();
    match expr {
        FilterExpr::Comparison { field, op, value } => {
            assert_eq!(field, "cpu_percent");
            assert_eq!(op, CompareOp::Gt);
            match value {
                FilterValue::Number(n) => assert!((n - 10.0).abs() < f64::EPSILON),
                _ => panic!("expected Number"),
            }
        }
        other => panic!("expected Comparison, got {:?}", other),
    }
}

#[test]
fn parse_comparison_eq_string() {
    let expr = parse_filter("user = \"admin\"").unwrap();
    match expr {
        FilterExpr::Comparison { field, op, value } => {
            assert_eq!(field, "user");
            assert_eq!(op, CompareOp::Eq);
            match value {
                FilterValue::Text(s) => assert_eq!(s, "admin"),
                _ => panic!("expected Text"),
            }
        }
        other => panic!("expected Comparison, got {:?}", other),
    }
}

#[test]
fn parse_comparison_ne() {
    let expr = parse_filter("status != \"zombie\"").unwrap();
    match expr {
        FilterExpr::Comparison { op, .. } => assert_eq!(op, CompareOp::NotEq),
        other => panic!("expected Comparison, got {:?}", other),
    }
}

#[test]
fn parse_comparison_gte() {
    let expr = parse_filter("memory >= 1024").unwrap();
    match expr {
        FilterExpr::Comparison { op, .. } => assert_eq!(op, CompareOp::Gte),
        other => panic!("expected Comparison, got {:?}", other),
    }
}

#[test]
fn parse_comparison_lte() {
    let expr = parse_filter("pid <= 100").unwrap();
    match expr {
        FilterExpr::Comparison { op, .. } => assert_eq!(op, CompareOp::Lte),
        other => panic!("expected Comparison, got {:?}", other),
    }
}

#[test]
fn parse_comparison_lt() {
    let expr = parse_filter("threads < 5").unwrap();
    match expr {
        FilterExpr::Comparison { op, .. } => assert_eq!(op, CompareOp::Lt),
        other => panic!("expected Comparison, got {:?}", other),
    }
}

#[test]
fn parse_comparison_bool_true() {
    let expr = parse_filter("elevated = true").unwrap();
    match expr {
        FilterExpr::Comparison { value, .. } => match value {
            FilterValue::Bool(b) => assert!(b),
            _ => panic!("expected Bool"),
        },
        other => panic!("expected Comparison, got {:?}", other),
    }
}

#[test]
fn parse_comparison_bool_false() {
    let expr = parse_filter("elevated = false").unwrap();
    match expr {
        FilterExpr::Comparison { value, .. } => match value {
            FilterValue::Bool(b) => assert!(!b),
            _ => panic!("expected Bool"),
        },
        other => panic!("expected Comparison, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// parse_filter – combinators
// ---------------------------------------------------------------------------

#[test]
fn parse_and_expression() {
    let expr = parse_filter("cpu_percent > 10 AND user = \"admin\"").unwrap();
    match expr {
        FilterExpr::And(parts) => {
            assert_eq!(parts.len(), 2);
        }
        other => panic!("expected And, got {:?}", other),
    }
}

#[test]
fn parse_or_expression() {
    let expr = parse_filter("status = \"running\" OR status = \"sleeping\"").unwrap();
    match expr {
        FilterExpr::Or(parts) => {
            assert_eq!(parts.len(), 2);
        }
        other => panic!("expected Or, got {:?}", other),
    }
}

#[test]
fn parse_not_expression() {
    let expr = parse_filter("NOT status = \"zombie\"").unwrap();
    match expr {
        FilterExpr::Not(inner) => match *inner {
            FilterExpr::Comparison { ref field, ref op, .. } => {
                assert_eq!(field, "status");
                assert_eq!(*op, CompareOp::Eq);
            }
            other => panic!("expected Comparison inside Not, got {:?}", other),
        },
        other => panic!("expected Not, got {:?}", other),
    }
}

#[test]
fn parse_complex_and_or() {
    let expr = parse_filter("cpu > 10 OR mem > 50 AND elevated = true").unwrap();
    // OR binds lower than AND, so this is: cpu > 10 OR (mem > 50 AND elevated = true)
    match expr {
        FilterExpr::Or(parts) => {
            assert_eq!(parts.len(), 2);
        }
        other => panic!("expected Or, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// parse_filter – error cases
// ---------------------------------------------------------------------------

#[test]
fn parse_empty_input_error() {
    assert!(parse_filter("").is_err());
}

#[test]
fn parse_whitespace_only_error() {
    assert!(parse_filter("   ").is_err());
}

// ---------------------------------------------------------------------------
// FilterPreset
// ---------------------------------------------------------------------------

#[test]
fn filter_preset_construction() {
    let preset = FilterPreset {
        name: "High CPU".into(),
        expression: FilterExpr::Comparison {
            field: "cpu_percent".into(),
            op: CompareOp::Gt,
            value: FilterValue::Number(50.0),
        },
    };
    assert_eq!(preset.name, "High CPU");
}

#[test]
fn filter_preset_serde_roundtrip() {
    let preset = FilterPreset {
        name: "Admin Only".into(),
        expression: FilterExpr::Comparison {
            field: "user".into(),
            op: CompareOp::Eq,
            value: FilterValue::Text("admin".into()),
        },
    };
    let json = serde_json::to_string(&preset).unwrap();
    let back: FilterPreset = serde_json::from_str(&json).unwrap();
    assert_eq!(back.name, "Admin Only");
}
