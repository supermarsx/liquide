//! Tests for the focus / activation protocol.

use crate::chain::FocusChain;
use crate::dispatch::{Dispatcher, FnHandler, MessageResult};
use crate::error::FocusError;
use crate::events::ActivationEvent;
use crate::history::{ActivationHistory, ActivationRecord};
use crate::hooks::{FnHook, HookChain, HookResult};
use crate::manager::{FocusManager, WindowInfo};
use crate::message::{
    MessagePriority, MessageTarget, MinMaxInfo, Modifiers, MouseButton, WindowMessage,
};
use crate::modal::ModalState;
use crate::queue::MessageQueue;
use crate::state::ActivationState;
use crate::timer::TimerManager;
use crate::types::{ActivateReason, WindowId};

// ---------------------------------------------------------------
// Helper: create a default WindowInfo
// ---------------------------------------------------------------

fn win_info(pid: u32, thread_id: u32) -> WindowInfo {
    WindowInfo {
        pid,
        thread_id,
        enabled: true,
        minimized: false,
        visible: true,
        parent: None,
    }
}

fn disabled_info(pid: u32) -> WindowInfo {
    WindowInfo {
        pid,
        thread_id: 1,
        enabled: false,
        minimized: false,
        visible: true,
        parent: None,
    }
}

fn minimized_info(pid: u32) -> WindowInfo {
    WindowInfo {
        pid,
        thread_id: 1,
        enabled: true,
        minimized: true,
        visible: true,
        parent: None,
    }
}

// ---------------------------------------------------------------
// ActivationState
// ---------------------------------------------------------------

#[test]
fn state_defaults_to_empty() {
    let s = ActivationState::new();
    assert_eq!(s.foreground_window, None);
    assert_eq!(s.active_window, None);
    assert_eq!(s.focus_window, None);
    assert_eq!(s.last_active, None);
    assert_eq!(s.foreground_lock_pid, None);
    assert_eq!(s.foreground_lock_timeout_ms, 200_000);
    assert_eq!(s.capture_window, None);
}

#[test]
fn state_foreground_lock_expiry() {
    let mut s = ActivationState::new();
    s.foreground_lock_pid = Some(42);
    s.foreground_lock_timestamp_us = 1000;
    s.foreground_lock_timeout_ms = 500;

    // Still within timeout.
    assert!(s.is_foreground_locked(1400));
    // Exactly at timeout boundary.
    assert!(!s.is_foreground_locked(1500));
    // Past timeout.
    assert!(!s.is_foreground_locked(2000));
}

#[test]
fn state_not_locked_when_no_pid() {
    let s = ActivationState::new();
    assert!(!s.is_foreground_locked(0));
    assert!(!s.is_foreground_locked(999_999));
}

// ---------------------------------------------------------------
// FocusManager: basic activation
// ---------------------------------------------------------------

#[test]
fn activate_first_window() {
    let mut fm = FocusManager::new();
    let w1 = WindowId(1);
    fm.register_window(w1, win_info(100, 1));

    let events = fm.activate_window(w1, ActivateReason::Click);

    // Should produce: NcActivate(true), Activate, ActivateApp(true), FocusGained
    // No old window → no deactivate/cancel/focus-lost events.
    // ActivateApp fires because old_pid (None) != new_pid (Some(100)).
    assert_eq!(events.len(), 4);
    assert!(matches!(
        &events[0],
        ActivationEvent::NcActivate { window, active: true } if *window == w1
    ));
    assert!(matches!(
        &events[1],
        ActivationEvent::Activate { window, reason: ActivateReason::Click } if *window == w1
    ));
    assert!(matches!(
        &events[2],
        ActivationEvent::ActivateApp { window, activating: true, .. } if *window == w1
    ));
    assert!(matches!(
        &events[3],
        ActivationEvent::FocusGained { window } if *window == w1
    ));

    assert_eq!(fm.active_window(), Some(w1));
    assert_eq!(fm.focus_window(), Some(w1));
    assert_eq!(fm.foreground_window(), Some(w1));
}

#[test]
fn activate_same_window_is_noop() {
    let mut fm = FocusManager::new();
    let w1 = WindowId(1);
    fm.register_window(w1, win_info(100, 1));
    fm.activate_window(w1, ActivateReason::Click);

    let events = fm.activate_window(w1, ActivateReason::Keyboard);
    assert!(events.is_empty());
}

#[test]
fn activate_second_window_full_sequence() {
    let mut fm = FocusManager::new();
    let w1 = WindowId(1);
    let w2 = WindowId(2);
    fm.register_window(w1, win_info(100, 1));
    fm.register_window(w2, win_info(200, 2));

    fm.activate_window(w1, ActivateReason::Click);
    let events = fm.activate_window(w2, ActivateReason::Keyboard);

    // Full NT sequence with process change:
    // NcActivate(w1, false), Deactivate(w1),
    // NcActivate(w2, true), Activate(w2),
    // ActivateApp(w1, false), ActivateApp(w2, true),
    // FocusLost(w1), FocusGained(w2)
    assert_eq!(events.len(), 8);

    assert!(matches!(
        &events[0],
        ActivationEvent::NcActivate { window, active: false } if *window == w1
    ));
    assert!(matches!(&events[1], ActivationEvent::Deactivate { window } if *window == w1));
    assert!(matches!(
        &events[2],
        ActivationEvent::NcActivate { window, active: true } if *window == w2
    ));
    assert!(matches!(
        &events[3],
        ActivationEvent::Activate { window, reason: ActivateReason::Keyboard } if *window == w2
    ));
    assert!(matches!(
        &events[4],
        ActivationEvent::ActivateApp { window, activating: false, .. } if *window == w1
    ));
    assert!(matches!(
        &events[5],
        ActivationEvent::ActivateApp { window, activating: true, .. } if *window == w2
    ));
    assert!(matches!(&events[6], ActivationEvent::FocusLost { window } if *window == w1));
    assert!(matches!(&events[7], ActivationEvent::FocusGained { window } if *window == w2));

    assert_eq!(fm.active_window(), Some(w2));
    assert_eq!(fm.last_active(), Some(w1));
}

#[test]
fn activate_same_process_no_activate_app() {
    let mut fm = FocusManager::new();
    let w1 = WindowId(1);
    let w2 = WindowId(2);
    // Same PID.
    fm.register_window(w1, win_info(100, 1));
    fm.register_window(w2, win_info(100, 2));

    fm.activate_window(w1, ActivateReason::Click);
    let events = fm.activate_window(w2, ActivateReason::Click);

    // No ActivateApp events when same process.
    for e in &events {
        assert!(!matches!(e, ActivationEvent::ActivateApp { .. }));
    }
}

#[test]
fn cancel_mode_sent_to_capture_window() {
    let mut fm = FocusManager::new();
    let w1 = WindowId(1);
    let w2 = WindowId(2);
    let capture = WindowId(99);
    fm.register_window(w1, win_info(100, 1));
    fm.register_window(w2, win_info(200, 2));

    fm.activate_window(w1, ActivateReason::Click);
    fm.set_capture(Some(capture));

    let events = fm.activate_window(w2, ActivateReason::Click);

    assert!(matches!(
        &events[0],
        ActivationEvent::CancelMode { window } if *window == capture
    ));
    // Capture should be cleared.
    assert_eq!(fm.state.capture_window, None);
}

// ---------------------------------------------------------------
// FocusManager: set_focus
// ---------------------------------------------------------------

#[test]
fn set_focus_within_active() {
    let mut fm = FocusManager::new();
    let w1 = WindowId(1);
    let child = WindowId(10);
    fm.register_window(w1, win_info(100, 1));
    fm.register_window(child, win_info(100, 1));
    fm.activate_window(w1, ActivateReason::Click);

    let events = fm.set_focus(child).unwrap();
    assert_eq!(events.len(), 2);
    assert!(matches!(&events[0], ActivationEvent::FocusLost { window } if *window == w1));
    assert!(matches!(&events[1], ActivationEvent::FocusGained { window } if *window == child));
    assert_eq!(fm.focus_window(), Some(child));
}

#[test]
fn set_focus_same_window_noop() {
    let mut fm = FocusManager::new();
    let w1 = WindowId(1);
    fm.register_window(w1, win_info(100, 1));
    fm.activate_window(w1, ActivateReason::Click);

    let events = fm.set_focus(w1).unwrap();
    assert!(events.is_empty());
}

#[test]
fn set_focus_unknown_window() {
    let mut fm = FocusManager::new();
    let unknown = WindowId(999);
    let err = fm.set_focus(unknown).unwrap_err();
    assert_eq!(err, FocusError::WindowNotFound(unknown));
}

#[test]
fn set_focus_disabled_window() {
    let mut fm = FocusManager::new();
    let w1 = WindowId(1);
    fm.register_window(w1, disabled_info(100));

    let err = fm.set_focus(w1).unwrap_err();
    assert_eq!(err, FocusError::WindowDisabled(w1));
}

// ---------------------------------------------------------------
// FocusManager: set_foreground
// ---------------------------------------------------------------

#[test]
fn set_foreground_no_lock() {
    let mut fm = FocusManager::new();
    let w1 = WindowId(1);
    fm.register_window(w1, win_info(100, 1));

    let events = fm.set_foreground(w1, 100, 0).unwrap();
    assert!(!events.is_empty());
    assert_eq!(fm.foreground_window(), Some(w1));
}

#[test]
fn set_foreground_locked_denied() {
    let mut fm = FocusManager::new();
    let w1 = WindowId(1);
    let w2 = WindowId(2);
    fm.register_window(w1, win_info(100, 1));
    fm.register_window(w2, win_info(200, 2));

    fm.activate_window(w1, ActivateReason::Click);
    fm.lock_foreground(100, 0);

    // PID 200 tries to steal foreground.
    let err = fm.set_foreground(w2, 200, 50).unwrap_err();
    assert!(matches!(err, FocusError::ForegroundLocked { flash_window } if flash_window == w2));
}

#[test]
fn set_foreground_lock_owner_allowed() {
    let mut fm = FocusManager::new();
    let w1 = WindowId(1);
    let w2 = WindowId(2);
    fm.register_window(w1, win_info(100, 1));
    fm.register_window(w2, win_info(100, 1));

    fm.activate_window(w1, ActivateReason::Click);
    fm.lock_foreground(100, 0);

    // The locking process can still set foreground.
    let events = fm.set_foreground(w2, 100, 50).unwrap();
    assert!(!events.is_empty());
}

#[test]
fn set_foreground_allowed_pid() {
    let mut fm = FocusManager::new();
    let w1 = WindowId(1);
    let w2 = WindowId(2);
    fm.register_window(w1, win_info(100, 1));
    fm.register_window(w2, win_info(200, 2));

    fm.activate_window(w1, ActivateReason::Click);
    fm.lock_foreground(100, 0);
    fm.allow_set_foreground(200);

    // PID 200 was explicitly allowed.
    let events = fm.set_foreground(w2, 200, 50).unwrap();
    assert!(!events.is_empty());

    // One-shot: allowance consumed.  Use a third process (PID 300) to
    // verify that the one-shot for PID 200 was consumed and doesn't
    // carry over.
    let w3 = WindowId(3);
    fm.register_window(w3, win_info(300, 3));
    fm.allow_set_foreground(300);
    let events = fm.set_foreground(w3, 300, 60).unwrap();
    assert!(!events.is_empty());

    // Now PID 300's allowance should be consumed.
    let w4 = WindowId(4);
    fm.register_window(w4, win_info(400, 4));
    let err = fm.set_foreground(w4, 400, 70).unwrap_err();
    assert!(matches!(err, FocusError::ForegroundLocked { .. }));
}

#[test]
fn set_foreground_input_awakened() {
    let mut fm = FocusManager::new();
    let w1 = WindowId(1);
    let w2 = WindowId(2);
    fm.register_window(w1, win_info(100, 1));
    fm.register_window(w2, win_info(200, 2));

    fm.activate_window(w1, ActivateReason::Click);
    fm.lock_foreground(100, 0);
    fm.mark_input_awakened(200);

    let events = fm.set_foreground(w2, 200, 50).unwrap();
    assert!(!events.is_empty());
}

#[test]
fn set_foreground_lock_expired() {
    let mut fm = FocusManager::new();
    let w1 = WindowId(1);
    let w2 = WindowId(2);
    fm.register_window(w1, win_info(100, 1));
    fm.register_window(w2, win_info(200, 2));

    fm.activate_window(w1, ActivateReason::Click);
    fm.lock_foreground(100, 0);

    // After timeout, anyone can set foreground.
    let events = fm.set_foreground(w2, 200, 200_001).unwrap();
    assert!(!events.is_empty());
}

#[test]
fn set_foreground_disabled_window() {
    let mut fm = FocusManager::new();
    let w1 = WindowId(1);
    fm.register_window(w1, disabled_info(100));

    let err = fm.set_foreground(w1, 100, 0).unwrap_err();
    assert_eq!(err, FocusError::WindowDisabled(w1));
}

#[test]
fn set_foreground_minimized_window() {
    let mut fm = FocusManager::new();
    let w1 = WindowId(1);
    fm.register_window(w1, minimized_info(100));

    let err = fm.set_foreground(w1, 100, 0).unwrap_err();
    assert_eq!(err, FocusError::WindowMinimized(w1));
}

#[test]
fn set_foreground_unknown_window() {
    let mut fm = FocusManager::new();
    let err = fm.set_foreground(WindowId(1), 100, 0).unwrap_err();
    assert!(matches!(err, FocusError::WindowNotFound(_)));
}

// ---------------------------------------------------------------
// FocusManager: unregister_window
// ---------------------------------------------------------------

#[test]
fn unregister_active_window_transfers_focus() {
    let mut fm = FocusManager::new();
    let w1 = WindowId(1);
    let w2 = WindowId(2);
    fm.register_window(w1, win_info(100, 1));
    fm.register_window(w2, win_info(200, 2));

    fm.activate_window(w1, ActivateReason::Click);
    fm.activate_window(w2, ActivateReason::Click);
    // w2 is active, w1 is on the focus chain.

    let events = fm.unregister_window(w2);
    // Should deactivate w2 and activate w1 from chain.
    assert!(events.iter().any(|e| matches!(e, ActivationEvent::Deactivate { window } if *window == w2)));
    assert_eq!(fm.active_window(), Some(w1));
}

#[test]
fn unregister_non_active_window() {
    let mut fm = FocusManager::new();
    let w1 = WindowId(1);
    let w2 = WindowId(2);
    fm.register_window(w1, win_info(100, 1));
    fm.register_window(w2, win_info(200, 2));

    fm.activate_window(w1, ActivateReason::Click);

    let events = fm.unregister_window(w2);
    assert!(events.is_empty());
    assert_eq!(fm.active_window(), Some(w1));
}

// ---------------------------------------------------------------
// FocusManager: unlock_foreground
// ---------------------------------------------------------------

#[test]
fn unlock_foreground_wrong_pid() {
    let mut fm = FocusManager::new();
    fm.lock_foreground(100, 0);
    fm.unlock_foreground(999);
    // Lock should still be held.
    assert!(fm.state.is_foreground_locked(50));
}

#[test]
fn unlock_foreground_correct_pid() {
    let mut fm = FocusManager::new();
    fm.lock_foreground(100, 0);
    fm.unlock_foreground(100);
    assert!(!fm.state.is_foreground_locked(50));
}

#[test]
fn unlock_foreground_pid_zero() {
    let mut fm = FocusManager::new();
    fm.lock_foreground(100, 0);
    fm.unlock_foreground(0);
    assert!(!fm.state.is_foreground_locked(50));
}

// ---------------------------------------------------------------
// FocusChain
// ---------------------------------------------------------------

#[test]
fn chain_push_pop() {
    let mut chain = FocusChain::new(32);
    chain.push_focus(WindowId(1));
    chain.push_focus(WindowId(2));
    chain.push_focus(WindowId(3));

    assert_eq!(chain.len(), 3);
    assert_eq!(chain.pop_focus(), Some(WindowId(3)));
    assert_eq!(chain.pop_focus(), Some(WindowId(2)));
    assert_eq!(chain.pop_focus(), Some(WindowId(1)));
    assert_eq!(chain.pop_focus(), None);
}

#[test]
fn chain_dedup() {
    let mut chain = FocusChain::new(32);
    chain.push_focus(WindowId(1));
    chain.push_focus(WindowId(2));
    chain.push_focus(WindowId(1)); // duplicate

    assert_eq!(chain.len(), 2);
    assert_eq!(chain.pop_focus(), Some(WindowId(1)));
    assert_eq!(chain.pop_focus(), Some(WindowId(2)));
}

#[test]
fn chain_bounded() {
    let mut chain = FocusChain::new(3);
    chain.push_focus(WindowId(1));
    chain.push_focus(WindowId(2));
    chain.push_focus(WindowId(3));
    chain.push_focus(WindowId(4)); // oldest (1) should be evicted

    assert_eq!(chain.len(), 3);
    let items: Vec<_> = chain.iter().collect();
    assert_eq!(items, vec![WindowId(4), WindowId(3), WindowId(2)]);
}

#[test]
fn chain_remove() {
    let mut chain = FocusChain::new(32);
    chain.push_focus(WindowId(1));
    chain.push_focus(WindowId(2));
    chain.push_focus(WindowId(3));

    chain.remove(WindowId(2));
    assert_eq!(chain.len(), 2);
    assert_eq!(chain.pop_focus(), Some(WindowId(3)));
    assert_eq!(chain.pop_focus(), Some(WindowId(1)));
}

#[test]
fn chain_peek() {
    let mut chain = FocusChain::new(32);
    assert_eq!(chain.peek(), None);
    chain.push_focus(WindowId(5));
    assert_eq!(chain.peek(), Some(WindowId(5)));
    chain.push_focus(WindowId(6));
    assert_eq!(chain.peek(), Some(WindowId(6)));
}

#[test]
fn chain_clear() {
    let mut chain = FocusChain::new(32);
    chain.push_focus(WindowId(1));
    chain.push_focus(WindowId(2));
    chain.clear();
    assert!(chain.is_empty());
}

// ---------------------------------------------------------------
// ActivationHistory
// ---------------------------------------------------------------

#[test]
fn history_push_and_recent() {
    let mut h = ActivationHistory::new(4);
    h.push(ActivationRecord {
        window_id: WindowId(1),
        timestamp_ms: 100,
        reason: ActivateReason::Click,
    });
    h.push(ActivationRecord {
        window_id: WindowId(2),
        timestamp_ms: 200,
        reason: ActivateReason::Keyboard,
    });
    h.push(ActivationRecord {
        window_id: WindowId(3),
        timestamp_ms: 300,
        reason: ActivateReason::Api,
    });

    let recent = h.recent(2);
    assert_eq!(recent.len(), 2);
    assert_eq!(recent[0].window_id, WindowId(3)); // newest
    assert_eq!(recent[1].window_id, WindowId(2));
}

#[test]
fn history_ring_buffer_wraps() {
    let mut h = ActivationHistory::new(3);
    for i in 1..=5 {
        h.push(ActivationRecord {
            window_id: WindowId(i),
            timestamp_ms: i * 100,
            reason: ActivateReason::Click,
        });
    }

    assert_eq!(h.len(), 3);
    let recent = h.recent(3);
    assert_eq!(recent[0].window_id, WindowId(5));
    assert_eq!(recent[1].window_id, WindowId(4));
    assert_eq!(recent[2].window_id, WindowId(3));
}

#[test]
fn history_was_recently_active() {
    let mut h = ActivationHistory::new(16);
    h.push(ActivationRecord {
        window_id: WindowId(1),
        timestamp_ms: 100,
        reason: ActivateReason::Click,
    });
    h.push(ActivationRecord {
        window_id: WindowId(2),
        timestamp_ms: 500,
        reason: ActivateReason::Click,
    });

    // Window 1 was active at t=100, checking at t=600 with window=500ms.
    assert!(h.was_recently_active(WindowId(1), 600, 600));
    assert!(!h.was_recently_active(WindowId(1), 400, 600));
    assert!(h.was_recently_active(WindowId(2), 200, 600));
}

#[test]
fn history_empty() {
    let h = ActivationHistory::new(4);
    assert!(h.is_empty());
    assert_eq!(h.len(), 0);
    assert!(h.recent(10).is_empty());
    assert!(!h.was_recently_active(WindowId(1), 1000, 1000));
}

#[test]
fn history_clear() {
    let mut h = ActivationHistory::new(4);
    h.push(ActivationRecord {
        window_id: WindowId(1),
        timestamp_ms: 100,
        reason: ActivateReason::Click,
    });
    h.clear();
    assert!(h.is_empty());
}

// ---------------------------------------------------------------
// ModalState
// ---------------------------------------------------------------

#[test]
fn modal_push_pop() {
    let mut m = ModalState::new();
    let dialog = WindowId(10);
    let owner = WindowId(1);

    m.push_modal(dialog, owner);
    assert!(m.is_modal(dialog));
    assert!(!m.is_modal(owner));
    assert_eq!(m.modal_owner(dialog), Some(owner));
    assert_eq!(m.depth(), 1);

    m.pop_modal(dialog);
    assert!(!m.is_modal(dialog));
    assert_eq!(m.depth(), 0);
}

#[test]
fn modal_blocks_activation() {
    let mut m = ModalState::new();
    let dialog = WindowId(10);
    let owner = WindowId(1);
    let other = WindowId(2);

    m.push_modal(dialog, owner);

    // The modal itself is NOT blocked.
    assert!(!m.should_block_activation(dialog));
    // Everything else IS blocked.
    assert!(m.should_block_activation(owner));
    assert!(m.should_block_activation(other));
}

#[test]
fn modal_no_block_when_empty() {
    let m = ModalState::new();
    assert!(!m.should_block_activation(WindowId(1)));
    assert!(!m.has_modal());
}

#[test]
fn modal_stacking() {
    let mut m = ModalState::new();
    let d1 = WindowId(10);
    let d2 = WindowId(20);
    let owner = WindowId(1);

    m.push_modal(d1, owner);
    m.push_modal(d2, d1);

    assert_eq!(m.topmost_modal(), Some(d2));
    assert_eq!(m.depth(), 2);

    // Only topmost modal can be activated.
    assert!(m.should_block_activation(d1));
    assert!(!m.should_block_activation(d2));

    m.pop_modal(d2);
    assert_eq!(m.topmost_modal(), Some(d1));
    assert!(!m.should_block_activation(d1));
}

#[test]
fn modal_duplicate_push_is_noop() {
    let mut m = ModalState::new();
    let d = WindowId(10);
    let owner = WindowId(1);

    m.push_modal(d, owner);
    m.push_modal(d, owner);
    assert_eq!(m.depth(), 1);
}

#[test]
fn modal_remove_window() {
    let mut m = ModalState::new();
    let d = WindowId(10);
    let owner = WindowId(1);

    m.push_modal(d, owner);
    m.remove_window(d);
    assert!(!m.has_modal());
}

#[test]
fn modal_remove_owner_clears() {
    let mut m = ModalState::new();
    let d = WindowId(10);
    let owner = WindowId(1);

    m.push_modal(d, owner);
    m.remove_window(owner);
    assert!(!m.has_modal());
}

// ---------------------------------------------------------------
// FocusManager: modal integration
// ---------------------------------------------------------------

#[test]
fn set_foreground_blocked_by_modal() {
    let mut fm = FocusManager::new();
    let w1 = WindowId(1);
    let w2 = WindowId(2);
    let dialog = WindowId(10);
    fm.register_window(w1, win_info(100, 1));
    fm.register_window(w2, win_info(200, 2));
    fm.register_window(dialog, win_info(100, 1));

    fm.activate_window(w1, ActivateReason::Click);
    fm.modal.push_modal(dialog, w1);

    // Trying to activate w2 through set_foreground should be blocked.
    let err = fm.set_foreground(w2, 200, 0).unwrap_err();
    assert!(matches!(err, FocusError::ModalBlocked { modal_window } if modal_window == dialog));

    // But the modal itself can be set foreground.
    let events = fm.set_foreground(dialog, 100, 0).unwrap();
    assert!(!events.is_empty());
}

// ---------------------------------------------------------------
// FocusManager: activate_and_record
// ---------------------------------------------------------------

#[test]
fn activate_and_record_adds_to_history() {
    let mut fm = FocusManager::new();
    let w1 = WindowId(1);
    fm.register_window(w1, win_info(100, 1));

    fm.activate_and_record(w1, ActivateReason::Click, 1000);

    let recent = fm.history.recent(1);
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].window_id, w1);
    assert_eq!(recent[0].timestamp_ms, 1000);
}

// ---------------------------------------------------------------
// FocusError Display
// ---------------------------------------------------------------

#[test]
fn focus_error_display() {
    let e = FocusError::WindowNotFound(WindowId(42));
    assert!(e.to_string().contains("42"));
    assert!(e.to_string().contains("not found"));

    let e = FocusError::ForegroundLocked {
        flash_window: WindowId(7),
    };
    assert!(e.to_string().contains("locked"));
    assert!(e.to_string().contains("7"));

    let e = FocusError::ModalBlocked {
        modal_window: WindowId(10),
    };
    assert!(e.to_string().contains("modal"));
}

// ---------------------------------------------------------------
// WindowId Display
// ---------------------------------------------------------------

#[test]
fn window_id_display() {
    let w = WindowId(123);
    assert_eq!(format!("{}", w), "WindowId(123)");
}

// ---------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------

#[test]
fn activate_three_windows_chain_order() {
    let mut fm = FocusManager::new();
    let w1 = WindowId(1);
    let w2 = WindowId(2);
    let w3 = WindowId(3);
    fm.register_window(w1, win_info(100, 1));
    fm.register_window(w2, win_info(100, 2));
    fm.register_window(w3, win_info(100, 3));

    fm.activate_window(w1, ActivateReason::Click);
    fm.activate_window(w2, ActivateReason::Click);
    fm.activate_window(w3, ActivateReason::Click);

    // Chain should have w1, w2 (w3 is active, not on chain).
    assert_eq!(fm.chain.peek(), Some(w2));
    assert_eq!(fm.chain.pop_focus(), Some(w2));
    assert_eq!(fm.chain.pop_focus(), Some(w1));
}

#[test]
fn foreground_process_can_always_set_foreground() {
    let mut fm = FocusManager::new();
    let w1 = WindowId(1);
    let w2 = WindowId(2);
    fm.register_window(w1, win_info(100, 1));
    fm.register_window(w2, win_info(200, 2));

    fm.activate_window(w1, ActivateReason::Click);
    // Lock by a different process.
    fm.lock_foreground(300, 0);

    // PID 100 is the foreground process (owns w1 which is foreground).
    // It should be allowed to switch to w2 ... wait, w2 is PID 200.
    // But PID 100 is calling set_foreground, and PID 100 is the
    // foreground process, so it should be allowed.
    let events = fm.set_foreground(w2, 100, 50).unwrap();
    assert!(!events.is_empty());
}

#[test]
fn update_window_info() {
    let mut fm = FocusManager::new();
    let w1 = WindowId(1);
    fm.register_window(w1, win_info(100, 1));

    assert!(fm.window_info(w1).unwrap().enabled);

    fm.update_window(w1, disabled_info(100));
    assert!(!fm.window_info(w1).unwrap().enabled);
}

#[test]
fn window_count() {
    let mut fm = FocusManager::new();
    assert_eq!(fm.window_count(), 0);
    fm.register_window(WindowId(1), win_info(100, 1));
    fm.register_window(WindowId(2), win_info(200, 2));
    assert_eq!(fm.window_count(), 2);
    fm.unregister_window(WindowId(1));
    assert_eq!(fm.window_count(), 1);
}

#[test]
fn activate_reason_serialization() {
    let reason = ActivateReason::MinRestore;
    let json = serde_json::to_string(&reason).unwrap();
    let back: ActivateReason = serde_json::from_str(&json).unwrap();
    assert_eq!(back, reason);
}

#[test]
fn activation_event_serialization() {
    let event = ActivationEvent::Activate {
        window: WindowId(42),
        reason: ActivateReason::Click,
    };
    let json = serde_json::to_string(&event).unwrap();
    let back: ActivationEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(back, event);
}

#[test]
fn clear_input_awakened() {
    let mut fm = FocusManager::new();
    let w1 = WindowId(1);
    let w2 = WindowId(2);
    fm.register_window(w1, win_info(100, 1));
    fm.register_window(w2, win_info(200, 2));

    fm.activate_window(w1, ActivateReason::Click);
    fm.lock_foreground(100, 0);

    fm.mark_input_awakened(200);
    fm.clear_input_awakened(200);

    let err = fm.set_foreground(w2, 200, 50).unwrap_err();
    assert!(matches!(err, FocusError::ForegroundLocked { .. }));
}

#[test]
fn default_impls() {
    let _fm = FocusManager::default();
    let _state = ActivationState::default();
    let _chain = FocusChain::default();
    let _history = ActivationHistory::default();
    let _modal = ModalState::default();
}

#[test]
fn history_recent_clamped() {
    let mut h = ActivationHistory::new(4);
    h.push(ActivationRecord {
        window_id: WindowId(1),
        timestamp_ms: 100,
        reason: ActivateReason::Click,
    });
    // Ask for more than available.
    let recent = h.recent(100);
    assert_eq!(recent.len(), 1);
}

#[test]
fn chain_min_depth() {
    // Depth of 0 should be clamped to 1.
    let mut chain = FocusChain::new(0);
    chain.push_focus(WindowId(1));
    assert_eq!(chain.len(), 1);
    chain.push_focus(WindowId(2));
    // Oldest evicted, only WindowId(2) remains.
    assert_eq!(chain.len(), 1);
    assert_eq!(chain.pop_focus(), Some(WindowId(2)));
}

#[test]
fn set_foreground_records_in_history() {
    let mut fm = FocusManager::new();
    let w1 = WindowId(1);
    fm.register_window(w1, win_info(100, 1));

    fm.set_foreground(w1, 100, 5000).unwrap();

    let recent = fm.history.recent(1);
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].window_id, w1);
    assert_eq!(recent[0].timestamp_ms, 5000);
    assert_eq!(recent[0].reason, ActivateReason::Api);
}

// ===============================================================
// WindowMessage
// ===============================================================

#[test]
fn message_is_mouse() {
    assert!(WindowMessage::MouseMove { x: 0.0, y: 0.0 }.is_mouse());
    assert!(WindowMessage::MouseDown {
        button: MouseButton::Left,
        x: 0.0,
        y: 0.0,
    }
    .is_mouse());
    assert!(WindowMessage::MouseUp {
        button: MouseButton::Right,
        x: 0.0,
        y: 0.0,
    }
    .is_mouse());
    assert!(WindowMessage::MouseWheel { delta: 1.0 }.is_mouse());
    assert!(WindowMessage::MouseEnter.is_mouse());
    assert!(WindowMessage::MouseLeave.is_mouse());
    assert!(!WindowMessage::Paint.is_mouse());
    assert!(!WindowMessage::KeyDown {
        keycode: 65,
        modifiers: Modifiers::NONE,
    }
    .is_mouse());
}

#[test]
fn message_is_keyboard() {
    assert!(WindowMessage::KeyDown {
        keycode: 65,
        modifiers: Modifiers::NONE,
    }
    .is_keyboard());
    assert!(WindowMessage::KeyUp {
        keycode: 65,
        modifiers: Modifiers::NONE,
    }
    .is_keyboard());
    assert!(WindowMessage::CharInput('a').is_keyboard());
    assert!(!WindowMessage::MouseMove { x: 0.0, y: 0.0 }.is_keyboard());
}

#[test]
fn message_is_lifecycle() {
    assert!(WindowMessage::Created.is_lifecycle());
    assert!(WindowMessage::Close.is_lifecycle());
    assert!(WindowMessage::Destroy.is_lifecycle());
    assert!(WindowMessage::Show.is_lifecycle());
    assert!(WindowMessage::Hide.is_lifecycle());
    assert!(!WindowMessage::Paint.is_lifecycle());
    assert!(!WindowMessage::Activate.is_lifecycle());
}

#[test]
fn message_is_drag_drop() {
    assert!(WindowMessage::DragEnter.is_drag_drop());
    assert!(WindowMessage::DragOver.is_drag_drop());
    assert!(WindowMessage::DragLeave.is_drag_drop());
    assert!(WindowMessage::Drop.is_drag_drop());
    assert!(!WindowMessage::Paint.is_drag_drop());
}

#[test]
fn modifiers_none_is_empty() {
    assert!(Modifiers::NONE.is_empty());
    let m = Modifiers {
        shift: true,
        ..Modifiers::NONE
    };
    assert!(!m.is_empty());
}

#[test]
fn modifiers_default_is_none() {
    let m = Modifiers::default();
    assert!(m.is_empty());
    assert_eq!(m, Modifiers::NONE);
}

#[test]
fn min_max_info_defaults() {
    let info = MinMaxInfo::default();
    assert_eq!(info.min_width, 0);
    assert_eq!(info.min_height, 0);
    assert_eq!(info.max_width, u32::MAX);
    assert_eq!(info.max_height, u32::MAX);
}

#[test]
fn message_target_new() {
    let t = MessageTarget::new(WindowId(5), WindowMessage::Paint);
    assert_eq!(t.window_id, WindowId(5));
    assert_eq!(t.message, WindowMessage::Paint);
    assert_eq!(t.priority, MessagePriority::Normal);
}

#[test]
fn message_target_with_priority() {
    let t = MessageTarget::with_priority(
        WindowId(7),
        WindowMessage::Close,
        MessagePriority::High,
    );
    assert_eq!(t.priority, MessagePriority::High);
}

#[test]
fn message_priority_ordering() {
    // High < Normal < Low (High is most urgent).
    assert!(MessagePriority::High < MessagePriority::Normal);
    assert!(MessagePriority::Normal < MessagePriority::Low);
}

#[test]
fn message_priority_default_is_normal() {
    assert_eq!(MessagePriority::default(), MessagePriority::Normal);
}

// ===============================================================
// MessageQueue
// ===============================================================

#[test]
fn queue_empty_by_default() {
    let q = MessageQueue::new();
    assert!(q.is_empty());
    assert_eq!(q.len(), 0);
    assert!(!q.has_messages());
    assert!(q.peek().is_none());
}

#[test]
fn queue_post_and_get() {
    let mut q = MessageQueue::new();
    q.post_normal(WindowMessage::Paint);
    q.post_normal(WindowMessage::Close);

    assert_eq!(q.len(), 2);
    assert!(q.has_messages());
    assert_eq!(q.get(), Some(WindowMessage::Paint));
    assert_eq!(q.get(), Some(WindowMessage::Close));
    assert!(q.is_empty());
}

#[test]
fn queue_priority_ordering() {
    let mut q = MessageQueue::new();
    q.post(WindowMessage::Paint, MessagePriority::Low);
    q.post(WindowMessage::Close, MessagePriority::Normal);
    q.post(WindowMessage::Activate, MessagePriority::High);

    assert_eq!(q.get(), Some(WindowMessage::Activate)); // High first
    assert_eq!(q.get(), Some(WindowMessage::Close)); // Normal second
    assert_eq!(q.get(), Some(WindowMessage::Paint)); // Low last
}

#[test]
fn queue_fifo_within_priority() {
    let mut q = MessageQueue::new();
    q.post(WindowMessage::Created, MessagePriority::Normal);
    q.post(WindowMessage::Show, MessagePriority::Normal);
    q.post(WindowMessage::Paint, MessagePriority::Normal);

    assert_eq!(q.get(), Some(WindowMessage::Created));
    assert_eq!(q.get(), Some(WindowMessage::Show));
    assert_eq!(q.get(), Some(WindowMessage::Paint));
}

#[test]
fn queue_peek_returns_highest() {
    let mut q = MessageQueue::new();
    q.post(WindowMessage::Paint, MessagePriority::Low);
    assert_eq!(q.peek(), Some(&WindowMessage::Paint));

    q.post(WindowMessage::Close, MessagePriority::High);
    assert_eq!(q.peek(), Some(&WindowMessage::Close));
}

#[test]
fn queue_drain_paint_coalesces() {
    let mut q = MessageQueue::new();
    q.post(WindowMessage::Paint, MessagePriority::Normal);
    q.post(WindowMessage::Paint, MessagePriority::Normal);
    q.post(WindowMessage::Paint, MessagePriority::High);
    q.post(WindowMessage::Close, MessagePriority::Normal);

    let had_paint = q.drain_paint();
    assert!(had_paint);

    // Should have 1 Paint (Low) + 1 Close (Normal) = 2 messages.
    assert_eq!(q.len(), 2);

    // Close (Normal) drains before Paint (Low).
    assert_eq!(q.get(), Some(WindowMessage::Close));
    assert_eq!(q.get(), Some(WindowMessage::Paint));
}

#[test]
fn queue_drain_paint_no_paint() {
    let mut q = MessageQueue::new();
    q.post_normal(WindowMessage::Close);

    let had_paint = q.drain_paint();
    assert!(!had_paint);
    assert_eq!(q.len(), 1);
}

#[test]
fn queue_coalesce_mouse_move() {
    let mut q = MessageQueue::new();
    q.post_normal(WindowMessage::MouseMove { x: 1.0, y: 1.0 });
    q.post_normal(WindowMessage::MouseMove { x: 2.0, y: 2.0 });
    q.post_normal(WindowMessage::MouseMove { x: 3.0, y: 3.0 });
    q.post_normal(WindowMessage::Close);

    q.coalesce_mouse_move();

    // Should keep only the last MouseMove + the Close.
    assert_eq!(q.len(), 2);

    // Both are Normal; Close was already there, then MouseMove was re-inserted at end.
    let first = q.get().unwrap();
    let second = q.get().unwrap();
    // One should be Close, one should be MouseMove(3,3).
    let has_close = first == WindowMessage::Close || second == WindowMessage::Close;
    let has_move = matches!(
        (&first, &second),
        (WindowMessage::MouseMove { x, y }, _) | (_, WindowMessage::MouseMove { x, y })
        if (*x - 3.0).abs() < f64::EPSILON && (*y - 3.0).abs() < f64::EPSILON
    );
    assert!(has_close);
    assert!(has_move);
}

#[test]
fn queue_coalesce_mouse_move_empty() {
    let mut q = MessageQueue::new();
    q.coalesce_mouse_move(); // should not panic
    assert!(q.is_empty());
}

#[test]
fn queue_clear() {
    let mut q = MessageQueue::new();
    q.post_normal(WindowMessage::Paint);
    q.post(WindowMessage::Close, MessagePriority::High);
    q.post(WindowMessage::Destroy, MessagePriority::Low);

    q.clear();
    assert!(q.is_empty());
    assert_eq!(q.len(), 0);
}

#[test]
fn queue_len_at() {
    let mut q = MessageQueue::new();
    q.post(WindowMessage::Paint, MessagePriority::High);
    q.post(WindowMessage::Close, MessagePriority::Normal);
    q.post(WindowMessage::Destroy, MessagePriority::Low);
    q.post(WindowMessage::Show, MessagePriority::Low);

    assert_eq!(q.len_at(MessagePriority::High), 1);
    assert_eq!(q.len_at(MessagePriority::Normal), 1);
    assert_eq!(q.len_at(MessagePriority::Low), 2);
}

#[test]
fn queue_drain_all() {
    let mut q = MessageQueue::new();
    q.post(WindowMessage::Destroy, MessagePriority::Low);
    q.post(WindowMessage::Close, MessagePriority::Normal);
    q.post(WindowMessage::Activate, MessagePriority::High);

    let all = q.drain_all();
    assert_eq!(all.len(), 3);
    assert_eq!(all[0], WindowMessage::Activate); // High
    assert_eq!(all[1], WindowMessage::Close); // Normal
    assert_eq!(all[2], WindowMessage::Destroy); // Low
    assert!(q.is_empty());
}

#[test]
fn queue_default() {
    let q = MessageQueue::default();
    assert!(q.is_empty());
}

// ===============================================================
// Dispatcher
// ===============================================================

#[test]
fn dispatcher_empty() {
    let d = Dispatcher::new();
    assert_eq!(d.handler_count(), 0);
    assert!(!d.is_quit_requested());
}

#[test]
fn dispatcher_register_and_dispatch() {
    let mut d = Dispatcher::new();
    let w1 = WindowId(1);

    let handler = FnHandler(|_wid: WindowId, msg: &WindowMessage| {
        if matches!(msg, WindowMessage::Paint) {
            MessageResult::Handled
        } else {
            MessageResult::NotHandled
        }
    });
    d.register_handler(w1, Box::new(handler));

    assert!(d.has_handler(w1));
    assert_eq!(d.handler_count(), 1);

    let target = MessageTarget::new(w1, WindowMessage::Paint);
    assert_eq!(d.dispatch(&target), MessageResult::Handled);

    let target2 = MessageTarget::new(w1, WindowMessage::Close);
    assert_eq!(d.dispatch(&target2), MessageResult::NotHandled);
}

#[test]
fn dispatcher_unregister() {
    let mut d = Dispatcher::new();
    let w1 = WindowId(1);
    let handler = FnHandler(|_, _| MessageResult::Handled);
    d.register_handler(w1, Box::new(handler));

    assert!(d.unregister(w1));
    assert!(!d.has_handler(w1));
    assert!(!d.unregister(w1)); // second unregister returns false
}

#[test]
fn dispatcher_default_handler() {
    let mut d = Dispatcher::new();
    let w1 = WindowId(1);

    // No per-window handler, but default handler is set.
    let default = FnHandler(|_, _| MessageResult::Handled);
    d.set_default_handler(Box::new(default));

    let target = MessageTarget::new(w1, WindowMessage::Paint);
    assert_eq!(d.dispatch(&target), MessageResult::Handled);
}

#[test]
fn dispatcher_not_handled_falls_through_to_default() {
    let mut d = Dispatcher::new();
    let w1 = WindowId(1);

    let handler = FnHandler(|_, _| MessageResult::NotHandled);
    d.register_handler(w1, Box::new(handler));

    let default = FnHandler(|_, _| MessageResult::Handled);
    d.set_default_handler(Box::new(default));

    let target = MessageTarget::new(w1, WindowMessage::Paint);
    assert_eq!(d.dispatch(&target), MessageResult::Handled);
}

#[test]
fn dispatcher_forward_to_default() {
    let mut d = Dispatcher::new();
    let w1 = WindowId(1);

    let handler = FnHandler(|_, _| MessageResult::Forward(WindowMessage::Close));
    d.register_handler(w1, Box::new(handler));

    let default = FnHandler(|_, msg: &WindowMessage| {
        if matches!(msg, WindowMessage::Close) {
            MessageResult::Handled
        } else {
            MessageResult::NotHandled
        }
    });
    d.set_default_handler(Box::new(default));

    let target = MessageTarget::new(w1, WindowMessage::Paint);
    assert_eq!(d.dispatch(&target), MessageResult::Handled);
}

#[test]
fn dispatcher_broadcast() {
    let mut d = Dispatcher::new();
    let w1 = WindowId(1);
    let w2 = WindowId(2);

    let h1 = FnHandler(|_, _| MessageResult::Handled);
    let h2 = FnHandler(|_, _| MessageResult::NotHandled);
    d.register_handler(w1, Box::new(h1));
    d.register_handler(w2, Box::new(h2));

    let results = d.broadcast(&WindowMessage::ThemeChanged);
    assert_eq!(results.len(), 2);

    // Both windows should appear in results (order may vary).
    let ids: Vec<WindowId> = results.iter().map(|(id, _)| *id).collect();
    assert!(ids.contains(&w1));
    assert!(ids.contains(&w2));
}

#[test]
fn dispatcher_quit() {
    let mut d = Dispatcher::new();
    assert!(!d.is_quit_requested());
    d.post_quit();
    assert!(d.is_quit_requested());
    d.cancel_quit();
    assert!(!d.is_quit_requested());
}

#[test]
fn dispatcher_log() {
    let mut d = Dispatcher::new();
    let w1 = WindowId(1);
    let handler = FnHandler(|_, _| MessageResult::Handled);
    d.register_handler(w1, Box::new(handler));

    // Log disabled by default.
    assert!(d.log().is_none());

    d.enable_log();
    let target = MessageTarget::new(w1, WindowMessage::Paint);
    d.dispatch(&target);

    let log = d.log().unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].0, w1);
    assert_eq!(log[0].1, WindowMessage::Paint);

    d.clear_log();
    assert_eq!(d.log().unwrap().len(), 0);
}

#[test]
fn dispatcher_no_handler_no_default() {
    let mut d = Dispatcher::new();
    let target = MessageTarget::new(WindowId(99), WindowMessage::Paint);
    assert_eq!(d.dispatch(&target), MessageResult::NotHandled);
}

#[test]
fn dispatcher_default_impl() {
    let d = Dispatcher::default();
    assert_eq!(d.handler_count(), 0);
}

// ===============================================================
// TimerManager
// ===============================================================

#[test]
fn timer_manager_empty() {
    let tm = TimerManager::new();
    assert!(tm.is_empty());
    assert_eq!(tm.len(), 0);
}

#[test]
fn timer_one_shot() {
    let mut tm = TimerManager::new();
    let w1 = WindowId(1);
    let tid = tm.set_timer(w1, 100, false);
    assert_eq!(tm.len(), 1);

    // Not yet expired.
    let fired = tm.tick(50);
    assert!(fired.is_empty());
    assert_eq!(tm.len(), 1);

    // Expired after 100ms total (50 + 50).
    let fired = tm.tick(50);
    assert_eq!(fired.len(), 1);
    assert_eq!(fired[0].window_id, w1);
    assert_eq!(fired[0].message, WindowMessage::Timer(tid));
    assert_eq!(fired[0].priority, MessagePriority::High);

    // One-shot timer removed after firing.
    assert!(tm.is_empty());
}

#[test]
fn timer_repeating() {
    let mut tm = TimerManager::new();
    let w1 = WindowId(1);
    let _tid = tm.set_timer(w1, 100, true);

    // First fire.
    let fired = tm.tick(100);
    assert_eq!(fired.len(), 1);
    assert_eq!(tm.len(), 1); // Still alive (repeating).

    // Second fire.
    let fired = tm.tick(100);
    assert_eq!(fired.len(), 1);
    assert_eq!(tm.len(), 1);
}

#[test]
fn timer_repeating_overshoot() {
    let mut tm = TimerManager::new();
    let w1 = WindowId(1);
    let _tid = tm.set_timer(w1, 100, true);

    // Tick 150ms — fires once, remaining should be adjusted for 50ms overshoot.
    let fired = tm.tick(150);
    assert_eq!(fired.len(), 1);

    // Next fire should happen in ~50ms (100 - 50 overshoot).
    let fired = tm.tick(40);
    assert!(fired.is_empty());
    let fired = tm.tick(10);
    assert_eq!(fired.len(), 1);
}

#[test]
fn timer_kill() {
    let mut tm = TimerManager::new();
    let w1 = WindowId(1);
    let tid = tm.set_timer(w1, 100, true);

    assert!(tm.kill_timer(tid));
    assert!(tm.is_empty());
    assert!(!tm.kill_timer(tid)); // Already removed.
}

#[test]
fn timer_kill_all_for_window() {
    let mut tm = TimerManager::new();
    let w1 = WindowId(1);
    let w2 = WindowId(2);
    tm.set_timer(w1, 100, true);
    tm.set_timer(w1, 200, false);
    tm.set_timer(w2, 300, true);

    tm.kill_all_for_window(w1);
    assert_eq!(tm.len(), 1);

    // The remaining timer belongs to w2.
    let fired = tm.tick(300);
    assert_eq!(fired.len(), 1);
    assert_eq!(fired[0].window_id, w2);
}

#[test]
fn timer_get() {
    let mut tm = TimerManager::new();
    let w1 = WindowId(1);
    let tid = tm.set_timer(w1, 250, true);

    let timer = tm.get(tid).unwrap();
    assert_eq!(timer.id, tid);
    assert_eq!(timer.window_id, w1);
    assert_eq!(timer.interval_ms, 250);
    assert!(timer.repeat);

    assert!(tm.get(9999).is_none());
}

#[test]
fn timer_clear() {
    let mut tm = TimerManager::new();
    tm.set_timer(WindowId(1), 100, true);
    tm.set_timer(WindowId(2), 200, true);
    tm.clear();
    assert!(tm.is_empty());
}

#[test]
fn timer_default() {
    let tm = TimerManager::default();
    assert!(tm.is_empty());
}

#[test]
fn timer_multiple_fire_same_tick() {
    let mut tm = TimerManager::new();
    let w1 = WindowId(1);
    let w2 = WindowId(2);
    let t1 = tm.set_timer(w1, 50, false);
    let t2 = tm.set_timer(w2, 80, false);

    // Tick 100ms — both should fire.
    let fired = tm.tick(100);
    assert_eq!(fired.len(), 2);

    let ids: Vec<u64> = fired.iter().filter_map(|mt| {
        if let WindowMessage::Timer(id) = mt.message { Some(id) } else { None }
    }).collect();
    assert!(ids.contains(&t1));
    assert!(ids.contains(&t2));
    assert!(tm.is_empty());
}

#[test]
fn timer_unique_ids() {
    let mut tm = TimerManager::new();
    let w1 = WindowId(1);
    let t1 = tm.set_timer(w1, 100, false);
    let t2 = tm.set_timer(w1, 200, false);
    let t3 = tm.set_timer(w1, 300, false);
    assert_ne!(t1, t2);
    assert_ne!(t2, t3);
    assert_ne!(t1, t3);
}

// ===============================================================
// HookChain
// ===============================================================

#[test]
fn hook_chain_empty() {
    let chain = HookChain::new();
    assert!(chain.is_empty());
    assert_eq!(chain.len(), 0);
}

#[test]
fn hook_chain_pass() {
    let mut chain = HookChain::new();
    let h = FnHook(|_, _| HookResult::Pass);
    chain.install_hook(Box::new(h));

    let result = chain.run(WindowId(1), WindowMessage::Paint);
    assert_eq!(result, Some(WindowMessage::Paint));
}

#[test]
fn hook_chain_block() {
    let mut chain = HookChain::new();
    let h = FnHook(|_, _| HookResult::Block);
    chain.install_hook(Box::new(h));

    let result = chain.run(WindowId(1), WindowMessage::Paint);
    assert!(result.is_none());
}

#[test]
fn hook_chain_transform() {
    let mut chain = HookChain::new();
    let h = FnHook(|_, _| HookResult::Transform(WindowMessage::Close));
    chain.install_hook(Box::new(h));

    let result = chain.run(WindowId(1), WindowMessage::Paint);
    assert_eq!(result, Some(WindowMessage::Close));
}

#[test]
fn hook_chain_transform_propagates() {
    let mut chain = HookChain::new();

    // First hook transforms Paint → Close.
    let h1 = FnHook(|_, msg: &WindowMessage| {
        if matches!(msg, WindowMessage::Paint) {
            HookResult::Transform(WindowMessage::Close)
        } else {
            HookResult::Pass
        }
    });
    chain.install_hook(Box::new(h1));

    // Second hook sees Close (the transformed message) and passes it.
    let h2 = FnHook(|_, msg: &WindowMessage| {
        assert!(matches!(msg, WindowMessage::Close));
        HookResult::Pass
    });
    chain.install_hook(Box::new(h2));

    let result = chain.run(WindowId(1), WindowMessage::Paint);
    assert_eq!(result, Some(WindowMessage::Close));
}

#[test]
fn hook_chain_block_short_circuits() {
    let mut chain = HookChain::new();

    // First hook blocks.
    let h1 = FnHook(|_, _| HookResult::Block);
    chain.install_hook(Box::new(h1));

    // Second hook would panic if called — but it shouldn't be.
    let h2 = FnHook(|_, _| panic!("Should not be called"));
    chain.install_hook(Box::new(h2));

    let result = chain.run(WindowId(1), WindowMessage::Paint);
    assert!(result.is_none());
}

#[test]
fn hook_chain_remove() {
    let mut chain = HookChain::new();
    let id1 = chain.install_hook(Box::new(FnHook(|_, _| HookResult::Block)));
    let _id2 = chain.install_hook(Box::new(FnHook(|_, _| HookResult::Pass)));

    assert_eq!(chain.len(), 2);
    assert!(chain.remove_hook(id1));
    assert_eq!(chain.len(), 1);
    assert!(!chain.remove_hook(id1)); // Already gone.

    // Only the Pass hook remains — message should pass through.
    let result = chain.run(WindowId(1), WindowMessage::Paint);
    assert_eq!(result, Some(WindowMessage::Paint));
}

#[test]
fn hook_chain_clear() {
    let mut chain = HookChain::new();
    chain.install_hook(Box::new(FnHook(|_, _| HookResult::Pass)));
    chain.install_hook(Box::new(FnHook(|_, _| HookResult::Pass)));
    chain.clear();
    assert!(chain.is_empty());
}

#[test]
fn hook_chain_default() {
    let chain = HookChain::default();
    assert!(chain.is_empty());
}

#[test]
fn hook_chain_debug() {
    let chain = HookChain::new();
    let dbg = format!("{:?}", chain);
    assert!(dbg.contains("HookChain"));
}

#[test]
fn hook_window_specific_filter() {
    let mut chain = HookChain::new();

    // Block messages to window 42, pass everything else.
    let h = FnHook(|wid: WindowId, _: &WindowMessage| {
        if wid == WindowId(42) {
            HookResult::Block
        } else {
            HookResult::Pass
        }
    });
    chain.install_hook(Box::new(h));

    assert!(chain.run(WindowId(42), WindowMessage::Paint).is_none());
    assert!(chain.run(WindowId(1), WindowMessage::Paint).is_some());
}

#[test]
fn hook_transform_key_to_char() {
    let mut chain = HookChain::new();

    // Transform KeyDown(65) → CharInput('A').
    let h = FnHook(|_, msg: &WindowMessage| {
        if let WindowMessage::KeyDown { keycode: 65, .. } = msg {
            HookResult::Transform(WindowMessage::CharInput('A'))
        } else {
            HookResult::Pass
        }
    });
    chain.install_hook(Box::new(h));

    let result = chain.run(
        WindowId(1),
        WindowMessage::KeyDown {
            keycode: 65,
            modifiers: Modifiers::NONE,
        },
    );
    assert_eq!(result, Some(WindowMessage::CharInput('A')));

    // Other keys pass through.
    let result = chain.run(
        WindowId(1),
        WindowMessage::KeyDown {
            keycode: 66,
            modifiers: Modifiers::NONE,
        },
    );
    assert!(matches!(result, Some(WindowMessage::KeyDown { keycode: 66, .. })));
}

// ===============================================================
// Integration: queue + dispatch + hooks + timers
// ===============================================================

#[test]
fn integration_queue_to_dispatch() {
    let mut q = MessageQueue::new();
    let mut d = Dispatcher::new();
    let w1 = WindowId(1);

    let handler = FnHandler(|_, msg: &WindowMessage| {
        if matches!(msg, WindowMessage::Paint) {
            MessageResult::Handled
        } else {
            MessageResult::NotHandled
        }
    });
    d.register_handler(w1, Box::new(handler));

    q.post_normal(WindowMessage::Paint);
    q.post_normal(WindowMessage::Close);

    let mut results = Vec::new();
    while let Some(msg) = q.get() {
        let target = MessageTarget::new(w1, msg);
        results.push(d.dispatch(&target));
    }

    assert_eq!(results, vec![MessageResult::Handled, MessageResult::NotHandled]);
}

#[test]
fn integration_timer_to_queue_to_dispatch() {
    let mut tm = TimerManager::new();
    let mut q = MessageQueue::new();
    let mut d = Dispatcher::new();
    let w1 = WindowId(1);

    let tid = tm.set_timer(w1, 50, false);

    let handler = FnHandler(move |_, msg: &WindowMessage| {
        if matches!(msg, WindowMessage::Timer(id) if *id == tid) {
            MessageResult::Handled
        } else {
            MessageResult::NotHandled
        }
    });
    d.register_handler(w1, Box::new(handler));

    // Tick 60ms — timer fires.
    let targets = tm.tick(60);
    for t in &targets {
        q.post(t.message.clone(), t.priority);
    }

    assert_eq!(q.len(), 1);

    while let Some(msg) = q.get() {
        let target = MessageTarget::new(w1, msg);
        assert_eq!(d.dispatch(&target), MessageResult::Handled);
    }
}

#[test]
fn integration_hooks_before_dispatch() {
    let mut chain = HookChain::new();
    let mut d = Dispatcher::new();
    let w1 = WindowId(1);

    // Hook transforms Paint → Close.
    let h = FnHook(|_, msg: &WindowMessage| {
        if matches!(msg, WindowMessage::Paint) {
            HookResult::Transform(WindowMessage::Close)
        } else {
            HookResult::Pass
        }
    });
    chain.install_hook(Box::new(h));

    // Handler only handles Close.
    let handler = FnHandler(|_, msg: &WindowMessage| {
        if matches!(msg, WindowMessage::Close) {
            MessageResult::Handled
        } else {
            MessageResult::NotHandled
        }
    });
    d.register_handler(w1, Box::new(handler));

    // Run Paint through hook chain → becomes Close → dispatch → Handled.
    if let Some(msg) = chain.run(w1, WindowMessage::Paint) {
        let target = MessageTarget::new(w1, msg);
        assert_eq!(d.dispatch(&target), MessageResult::Handled);
    } else {
        panic!("Hook should not block Paint");
    }
}

#[test]
fn integration_hooks_block_prevents_dispatch() {
    let mut chain = HookChain::new();
    let w1 = WindowId(1);

    let h = FnHook(|_, _| HookResult::Block);
    chain.install_hook(Box::new(h));

    let result = chain.run(w1, WindowMessage::Paint);
    assert!(result.is_none());
    // Nothing to dispatch.
}

#[test]
fn integration_broadcast_theme_changed() {
    let mut d = Dispatcher::new();
    let w1 = WindowId(1);
    let w2 = WindowId(2);
    let w3 = WindowId(3);

    let h = FnHandler(|_, msg: &WindowMessage| {
        if matches!(msg, WindowMessage::ThemeChanged) {
            MessageResult::Handled
        } else {
            MessageResult::NotHandled
        }
    });
    d.register_handler(w1, Box::new(h));
    let h = FnHandler(|_, _| MessageResult::Handled);
    d.register_handler(w2, Box::new(h));
    let h = FnHandler(|_, _| MessageResult::NotHandled);
    d.register_handler(w3, Box::new(h));

    let results = d.broadcast(&WindowMessage::ThemeChanged);
    assert_eq!(results.len(), 3);
}

#[test]
fn integration_full_pipeline() {
    // Simulates: timer fires → message queued → hook chain → dispatch.
    let mut tm = TimerManager::new();
    let mut q = MessageQueue::new();
    let mut chain = HookChain::new();
    let mut d = Dispatcher::new();

    let w1 = WindowId(1);
    let tid = tm.set_timer(w1, 100, false);

    // Hook: pass Timer messages through.
    chain.install_hook(Box::new(FnHook(|_, _| HookResult::Pass)));

    // Handler: accept Timer messages.
    let handler = FnHandler(move |_, msg: &WindowMessage| {
        if let WindowMessage::Timer(id) = msg {
            if *id == tid {
                return MessageResult::Handled;
            }
        }
        MessageResult::NotHandled
    });
    d.register_handler(w1, Box::new(handler));

    // Tick.
    let targets = tm.tick(100);
    for t in targets {
        q.post(t.message, t.priority);
    }

    // Drain queue through hooks then dispatcher.
    while let Some(msg) = q.get() {
        if let Some(filtered) = chain.run(w1, msg) {
            let target = MessageTarget::new(w1, filtered);
            let result = d.dispatch(&target);
            assert_eq!(result, MessageResult::Handled);
        }
    }
}

#[test]
fn queue_multiple_coalesce_mouse_move_different_priorities() {
    let mut q = MessageQueue::new();
    q.post(
        WindowMessage::MouseMove { x: 1.0, y: 1.0 },
        MessagePriority::Normal,
    );
    q.post(
        WindowMessage::MouseMove { x: 2.0, y: 2.0 },
        MessagePriority::High,
    );

    q.coalesce_mouse_move();
    // Only one MouseMove should remain.
    let all = q.drain_all();
    let moves: Vec<_> = all
        .iter()
        .filter(|m| matches!(m, WindowMessage::MouseMove { .. }))
        .collect();
    assert_eq!(moves.len(), 1);
}

#[test]
fn message_clone_eq() {
    let m1 = WindowMessage::Resize {
        width: 800,
        height: 600,
    };
    let m2 = m1.clone();
    assert_eq!(m1, m2);
}

#[test]
fn message_target_clone_eq() {
    let t1 = MessageTarget::new(WindowId(1), WindowMessage::Paint);
    let t2 = t1.clone();
    assert_eq!(t1, t2);
}

#[test]
fn mouse_button_variants() {
    let buttons = [
        MouseButton::Left,
        MouseButton::Right,
        MouseButton::Middle,
        MouseButton::X1,
        MouseButton::X2,
    ];
    for (i, b) in buttons.iter().enumerate() {
        for (j, c) in buttons.iter().enumerate() {
            if i == j {
                assert_eq!(b, c);
            } else {
                assert_ne!(b, c);
            }
        }
    }
}

#[test]
fn timer_remaining_decrements() {
    let mut tm = TimerManager::new();
    let tid = tm.set_timer(WindowId(1), 200, false);

    tm.tick(50);
    let timer = tm.get(tid).unwrap();
    assert_eq!(timer.remaining_ms, 150);

    tm.tick(50);
    let timer = tm.get(tid).unwrap();
    assert_eq!(timer.remaining_ms, 100);
}

#[test]
fn dispatcher_replace_handler() {
    let mut d = Dispatcher::new();
    let w1 = WindowId(1);

    let h1 = FnHandler(|_, _| MessageResult::NotHandled);
    d.register_handler(w1, Box::new(h1));

    let target = MessageTarget::new(w1, WindowMessage::Paint);
    assert_eq!(d.dispatch(&target), MessageResult::NotHandled);

    // Replace with a handler that returns Handled.
    let h2 = FnHandler(|_, _| MessageResult::Handled);
    d.register_handler(w1, Box::new(h2));

    assert_eq!(d.dispatch(&target), MessageResult::Handled);
    assert_eq!(d.handler_count(), 1); // Still just one handler for w1.
}
