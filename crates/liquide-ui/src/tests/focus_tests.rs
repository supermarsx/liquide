//! Tests for focus chain navigation.

use crate::focus::{FocusChain, FocusDirection};
use crate::widget::WidgetId;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn w(id: u64) -> WidgetId {
    WidgetId(id)
}

fn chain_of_3() -> FocusChain {
    let mut chain = FocusChain::new();
    chain.add(w(1));
    chain.add(w(2));
    chain.add(w(3));
    chain
}

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

#[test]
fn test_focus_chain_new_is_empty() {
    let chain = FocusChain::new();
    assert!(chain.is_empty());
    assert_eq!(chain.len(), 0);
    assert!(chain.current().is_none());
}

#[test]
fn test_focus_chain_add() {
    let mut chain = FocusChain::new();
    chain.add(w(1));
    chain.add(w(2));
    assert_eq!(chain.len(), 2);
    assert!(chain.contains(&w(1)));
    assert!(chain.contains(&w(2)));
}

#[test]
fn test_focus_chain_add_duplicate_ignored() {
    let mut chain = FocusChain::new();
    chain.add(w(1));
    chain.add(w(1));
    assert_eq!(chain.len(), 1);
}

// ---------------------------------------------------------------------------
// Navigation - Next/Previous
// ---------------------------------------------------------------------------

#[test]
fn test_focus_next_from_none() {
    let mut chain = chain_of_3();
    let focused = chain.move_focus(FocusDirection::Next);
    assert_eq!(focused, Some(w(1)));
    assert_eq!(chain.current(), Some(w(1)));
}

#[test]
fn test_focus_next_cycles() {
    let mut chain = chain_of_3();
    chain.move_focus(FocusDirection::Next); // -> 1
    chain.move_focus(FocusDirection::Next); // -> 2
    chain.move_focus(FocusDirection::Next); // -> 3
    let focused = chain.move_focus(FocusDirection::Next); // wrap -> 1
    assert_eq!(focused, Some(w(1)));
}

#[test]
fn test_focus_previous_from_none() {
    let mut chain = chain_of_3();
    let focused = chain.move_focus(FocusDirection::Previous);
    assert_eq!(focused, Some(w(3)));
}

#[test]
fn test_focus_previous_cycles() {
    let mut chain = chain_of_3();
    chain.set_focus(w(1));
    let focused = chain.move_focus(FocusDirection::Previous); // wrap -> 3
    assert_eq!(focused, Some(w(3)));
}

#[test]
fn test_focus_next_sequential() {
    let mut chain = chain_of_3();
    assert_eq!(chain.move_focus(FocusDirection::Next), Some(w(1)));
    assert_eq!(chain.move_focus(FocusDirection::Next), Some(w(2)));
    assert_eq!(chain.move_focus(FocusDirection::Next), Some(w(3)));
}

// ---------------------------------------------------------------------------
// Navigation - Directional
// ---------------------------------------------------------------------------

#[test]
fn test_focus_down_is_forward() {
    let mut chain = chain_of_3();
    chain.set_focus(w(1));
    assert_eq!(chain.move_focus(FocusDirection::Down), Some(w(2)));
}

#[test]
fn test_focus_right_is_forward() {
    let mut chain = chain_of_3();
    chain.set_focus(w(1));
    assert_eq!(chain.move_focus(FocusDirection::Right), Some(w(2)));
}

#[test]
fn test_focus_up_is_backward() {
    let mut chain = chain_of_3();
    chain.set_focus(w(2));
    assert_eq!(chain.move_focus(FocusDirection::Up), Some(w(1)));
}

#[test]
fn test_focus_left_is_backward() {
    let mut chain = chain_of_3();
    chain.set_focus(w(2));
    assert_eq!(chain.move_focus(FocusDirection::Left), Some(w(1)));
}

// ---------------------------------------------------------------------------
// Set focus
// ---------------------------------------------------------------------------

#[test]
fn test_set_focus_existing() {
    let mut chain = chain_of_3();
    assert!(chain.set_focus(w(2)));
    assert_eq!(chain.current(), Some(w(2)));
}

#[test]
fn test_set_focus_not_in_chain() {
    let mut chain = chain_of_3();
    assert!(!chain.set_focus(w(99)));
    assert!(chain.current().is_none());
}

// ---------------------------------------------------------------------------
// Clear focus
// ---------------------------------------------------------------------------

#[test]
fn test_clear_focus() {
    let mut chain = chain_of_3();
    chain.set_focus(w(1));
    chain.clear_focus();
    assert!(chain.current().is_none());
}

// ---------------------------------------------------------------------------
// Remove
// ---------------------------------------------------------------------------

#[test]
fn test_remove_non_focused() {
    let mut chain = chain_of_3();
    chain.set_focus(w(1));
    chain.remove(&w(3));
    assert_eq!(chain.len(), 2);
    assert_eq!(chain.current(), Some(w(1)));
}

#[test]
fn test_remove_focused_adjusts() {
    let mut chain = chain_of_3();
    chain.set_focus(w(2));
    chain.remove(&w(2));
    assert_eq!(chain.len(), 2);
    // Focus stays at the same index (now w(3)), or wraps.
    let current = chain.current();
    assert!(current.is_some());
}

#[test]
fn test_remove_last_item_clears_focus() {
    let mut chain = FocusChain::new();
    chain.add(w(1));
    chain.set_focus(w(1));
    chain.remove(&w(1));
    assert!(chain.is_empty());
    assert!(chain.current().is_none());
}

#[test]
fn test_remove_not_in_chain_is_noop() {
    let mut chain = chain_of_3();
    chain.remove(&w(99));
    assert_eq!(chain.len(), 3);
}

// ---------------------------------------------------------------------------
// Contains
// ---------------------------------------------------------------------------

#[test]
fn test_contains_present() {
    let chain = chain_of_3();
    assert!(chain.contains(&w(1)));
    assert!(chain.contains(&w(2)));
    assert!(chain.contains(&w(3)));
}

#[test]
fn test_contains_absent() {
    let chain = chain_of_3();
    assert!(!chain.contains(&w(99)));
}

// ---------------------------------------------------------------------------
// Empty chain navigation
// ---------------------------------------------------------------------------

#[test]
fn test_move_focus_empty_chain() {
    let mut chain = FocusChain::new();
    assert_eq!(chain.move_focus(FocusDirection::Next), None);
    assert_eq!(chain.move_focus(FocusDirection::Previous), None);
}
