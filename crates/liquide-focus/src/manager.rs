//! The activation protocol engine.
//!
//! Implements the activation sequence: ordered event dispatch,
//! foreground steal prevention, modal blocking, and focus chain management.

use std::collections::{HashMap, HashSet};

use crate::chain::FocusChain;
use crate::error::FocusError;
use crate::events::ActivationEvent;
use crate::history::{ActivationHistory, ActivationRecord};
use crate::modal::ModalState;
use crate::state::ActivationState;
use crate::types::{ActivateReason, WindowId};

/// Information about a window that the focus manager needs to know.
#[derive(Debug, Clone)]
pub struct WindowInfo {
    /// The process ID that owns this window.
    pub pid: u32,
    /// A thread ID (message-queue thread) associated with this window.
    pub thread_id: u32,
    /// Whether the window is currently enabled (can receive input).
    pub enabled: bool,
    /// Whether the window is minimised.
    pub minimized: bool,
    /// Whether the window is visible.
    pub visible: bool,
    /// The parent or owner window (for child→active-parent resolution).
    pub parent: Option<WindowId>,
}

/// The activation protocol engine.
///
/// Call [`register_window`] / [`unregister_window`] to keep the window
/// registry in sync, then use [`activate_window`], [`set_focus`],
/// [`set_foreground`] to drive the protocol.
pub struct FocusManager {
    /// Desktop-wide activation state.
    pub state: ActivationState,
    /// Window registry (must be kept in sync by the caller).
    windows: HashMap<WindowId, WindowInfo>,
    /// Focus chain (bounded stack of previous focus targets).
    pub chain: FocusChain,
    /// Activation history ring buffer.
    pub history: ActivationHistory,
    /// Modal window tracking.
    pub modal: ModalState,
    /// Set of PIDs that have been granted temporary foreground permission
    /// via `allow_set_foreground`.
    allowed_foreground_pids: HashSet<u32>,
    /// Set of PIDs that recently received user input (input-awakened) and
    /// are therefore allowed to set foreground regardless of the lock.
    input_awakened_pids: HashSet<u32>,
}

impl FocusManager {
    /// Create a new focus manager with no windows registered.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: ActivationState::new(),
            windows: HashMap::new(),
            chain: FocusChain::default(),
            history: ActivationHistory::default(),
            modal: ModalState::default(),
            allowed_foreground_pids: HashSet::new(),
            input_awakened_pids: HashSet::new(),
        }
    }

    // ---------------------------------------------------------------
    // Window registry
    // ---------------------------------------------------------------

    /// Register a window.  Must be called before the window can be
    /// activated or focused.
    pub fn register_window(&mut self, id: WindowId, info: WindowInfo) {
        self.windows.insert(id, info);
    }

    /// Unregister a window (e.g. on close).
    ///
    /// Returns activation events if the removed window was active/focused
    /// and focus needed to be transferred.
    pub fn unregister_window(&mut self, id: WindowId) -> Vec<ActivationEvent> {
        self.windows.remove(&id);
        self.chain.remove(id);
        self.modal.remove_window(id);

        let mut events = Vec::new();

        // If the closed window was the focus window, send FocusLost.
        if self.state.focus_window == Some(id) {
            events.push(ActivationEvent::FocusLost { window: id });
            self.state.focus_window = None;
        }

        // If the closed window was active, try to activate the next from
        // the focus chain.
        if self.state.active_window == Some(id) {
            events.push(ActivationEvent::Deactivate { window: id });
            events.push(ActivationEvent::NcActivate {
                window: id,
                active: false,
            });
            self.state.active_window = None;

            // Try focus chain first, then foreground, then nothing.
            if let Some(next) = self.chain.pop_focus() {
                if self.windows.contains_key(&next) {
                    let mut sub = self.activate_window(next, ActivateReason::Other);
                    events.append(&mut sub);
                }
            }
        }

        if self.state.foreground_window == Some(id) {
            self.state.foreground_window = None;
        }
        if self.state.last_active == Some(id) {
            self.state.last_active = None;
        }
        if self.state.capture_window == Some(id) {
            self.state.capture_window = None;
        }

        events
    }

    /// Get information about a registered window.
    #[must_use]
    pub fn window_info(&self, id: WindowId) -> Option<&WindowInfo> {
        self.windows.get(&id)
    }

    /// Update a window's info (e.g. after enable/disable or minimize).
    pub fn update_window(&mut self, id: WindowId, info: WindowInfo) {
        self.windows.insert(id, info);
    }

    /// Set the capture window (mouse capture for drag/resize/menu).
    pub fn set_capture(&mut self, window: Option<WindowId>) {
        self.state.capture_window = window;
    }

    // ---------------------------------------------------------------
    // Core activation protocol
    // ---------------------------------------------------------------

    /// Activate a window, following the standard activation sequence.
    ///
    /// Returns the ordered list of events that must be dispatched.
    ///
    /// # Sequence
    ///
    /// 1. If same as current active, return empty.
    /// 2. `CancelMode` to current capture window.
    /// 3. `NcActivate(false)` to old active.
    /// 4. `Deactivate` to old active.
    /// 5. Update `active_window`.
    /// 6. `NcActivate(true)` to new active.
    /// 7. `Activate` to new active.
    /// 8. If process changed, `ActivateApp(false)` to old, `ActivateApp(true)` to new.
    /// 9. `FocusLost` to old focus.
    /// 10. `FocusGained` to new focus (child of new active, or new active itself).
    pub fn activate_window(
        &mut self,
        window_id: WindowId,
        reason: ActivateReason,
    ) -> Vec<ActivationEvent> {
        // Step 1: If same as current active, no-op.
        if self.state.active_window == Some(window_id) {
            return Vec::new();
        }

        let mut events = Vec::new();
        let old_active = self.state.active_window;
        let old_focus = self.state.focus_window;

        // Step 2: CancelMode to capture window.
        if let Some(capture) = self.state.capture_window.take() {
            events.push(ActivationEvent::CancelMode { window: capture });
        }

        // Resolve PIDs for ActivateApp.
        let old_pid = old_active.and_then(|w| self.windows.get(&w).map(|i| i.pid));
        let new_pid = self.windows.get(&window_id).map(|i| i.pid);

        // Step 3: NcActivate(false) to old active.
        if let Some(old) = old_active {
            events.push(ActivationEvent::NcActivate {
                window: old,
                active: false,
            });
        }

        // Step 4: Deactivate old active.
        if let Some(old) = old_active {
            events.push(ActivationEvent::Deactivate { window: old });
        }

        // Step 5: Update active_window.
        self.state.last_active = old_active;
        self.state.active_window = Some(window_id);

        // Push old active onto focus chain.
        if let Some(old) = old_active {
            self.chain.push_focus(old);
        }

        // Step 6: NcActivate(true) to new active.
        events.push(ActivationEvent::NcActivate {
            window: window_id,
            active: true,
        });

        // Step 7: Activate new active.
        events.push(ActivationEvent::Activate {
            window: window_id,
            reason,
        });

        // Step 8: ActivateApp if process changed.
        if old_pid != new_pid {
            if let Some(old) = old_active {
                if let Some(info) = self.windows.get(&old) {
                    let tid = info.thread_id;
                    events.push(ActivationEvent::ActivateApp {
                        window: old,
                        activating: false,
                        thread_id: tid,
                    });
                }
            }
            if let Some(info) = self.windows.get(&window_id) {
                let tid = info.thread_id;
                events.push(ActivationEvent::ActivateApp {
                    window: window_id,
                    activating: true,
                    thread_id: tid,
                });
            }
        }

        // Step 9: FocusLost to old focus.
        if let Some(old_f) = old_focus {
            events.push(ActivationEvent::FocusLost { window: old_f });
        }

        // Step 10: FocusGained to new focus.
        // The focus target is the new active window itself (a more
        // sophisticated system would find the first focusable child,
        // but the caller can call set_focus() afterwards).
        self.state.focus_window = Some(window_id);
        events.push(ActivationEvent::FocusGained { window: window_id });

        // Update foreground window to match.
        self.state.foreground_window = Some(window_id);

        events
    }

    /// Change keyboard focus within the active window (or to a child).
    ///
    /// Returns `FocusLost` for the old focus and `FocusGained` for the new.
    /// If `window_id` is not registered, returns an error.
    pub fn set_focus(&mut self, window_id: WindowId) -> Result<Vec<ActivationEvent>, FocusError> {
        if !self.windows.contains_key(&window_id) {
            return Err(FocusError::WindowNotFound(window_id));
        }
        if let Some(info) = self.windows.get(&window_id) {
            if !info.enabled {
                return Err(FocusError::WindowDisabled(window_id));
            }
        }

        let mut events = Vec::new();
        let old_focus = self.state.focus_window;

        if old_focus == Some(window_id) {
            return Ok(events);
        }

        if let Some(old) = old_focus {
            events.push(ActivationEvent::FocusLost { window: old });
        }

        self.state.focus_window = Some(window_id);
        events.push(ActivationEvent::FocusGained { window: window_id });

        Ok(events)
    }

    /// Attempt to set a window as the foreground window.
    ///
    /// Subject to foreground-steal prevention: only the foreground process,
    /// processes granted via [`allow_set_foreground`], or input-awakened
    /// processes may succeed.  Others receive `FocusError::ForegroundLocked`.
    ///
    /// `caller_pid` is the PID of the process making the request.
    /// `now_ms` is the current monotonic time in milliseconds.
    pub fn set_foreground(
        &mut self,
        window_id: WindowId,
        caller_pid: u32,
        now_ms: u64,
    ) -> Result<Vec<ActivationEvent>, FocusError> {
        // Window must exist.
        let info = self
            .windows
            .get(&window_id)
            .ok_or(FocusError::WindowNotFound(window_id))?;

        if !info.enabled {
            return Err(FocusError::WindowDisabled(window_id));
        }
        if info.minimized {
            return Err(FocusError::WindowMinimized(window_id));
        }

        // Modal blocking check.
        if self.modal.should_block_activation(window_id) {
            if let Some(modal) = self.modal.topmost_modal() {
                return Err(FocusError::ModalBlocked {
                    modal_window: modal,
                });
            }
        }

        // Foreground-steal prevention.
        if !self.can_set_foreground(caller_pid, now_ms) {
            return Err(FocusError::ForegroundLocked {
                flash_window: window_id,
            });
        }

        // Permission granted — clear the one-shot allowance.
        self.allowed_foreground_pids.remove(&caller_pid);
        self.input_awakened_pids.remove(&caller_pid);

        let events = self.activate_window(window_id, ActivateReason::Api);

        // Record in history.
        self.history.push(ActivationRecord {
            window_id,
            timestamp_ms: now_ms,
            reason: ActivateReason::Api,
        });

        Ok(events)
    }

    // ---------------------------------------------------------------
    // Foreground lock
    // ---------------------------------------------------------------

    /// Lock foreground activation to the given process.
    ///
    /// While locked, only `pid` (and processes granted via
    /// [`allow_set_foreground`]) may call [`set_foreground`] successfully.
    ///
    /// `now_ms` is the current monotonic time in milliseconds.
    pub fn lock_foreground(&mut self, pid: u32, now_ms: u64) {
        self.state.foreground_lock_pid = Some(pid);
        self.state.foreground_lock_timestamp_us = now_ms;
    }

    /// Unlock foreground activation.
    ///
    /// Only the process that locked it (or `pid == 0` for unconditional
    /// unlock) may unlock.
    pub fn unlock_foreground(&mut self, pid: u32) {
        if pid == 0 || self.state.foreground_lock_pid == Some(pid) {
            self.state.foreground_lock_pid = None;
            self.state.foreground_lock_timestamp_us = 0;
        }
    }

    /// Grant a process temporary permission to set the foreground window.
    ///
    /// This is the equivalent of `AllowSetForegroundWindow(pid)`.
    pub fn allow_set_foreground(&mut self, pid: u32) {
        self.allowed_foreground_pids.insert(pid);
    }

    /// Mark a process as input-awakened (it just received user input).
    ///
    /// Input-awakened processes are allowed to set foreground regardless
    /// of the lock.
    pub fn mark_input_awakened(&mut self, pid: u32) {
        self.input_awakened_pids.insert(pid);
    }

    /// Clear the input-awakened flag for a process.
    pub fn clear_input_awakened(&mut self, pid: u32) {
        self.input_awakened_pids.remove(&pid);
    }

    // ---------------------------------------------------------------
    // Queries
    // ---------------------------------------------------------------

    /// Currently active window.
    #[must_use]
    pub fn active_window(&self) -> Option<WindowId> {
        self.state.active_window
    }

    /// Currently focused window.
    #[must_use]
    pub fn focus_window(&self) -> Option<WindowId> {
        self.state.focus_window
    }

    /// Current foreground window.
    #[must_use]
    pub fn foreground_window(&self) -> Option<WindowId> {
        self.state.foreground_window
    }

    /// Previously active window.
    #[must_use]
    pub fn last_active(&self) -> Option<WindowId> {
        self.state.last_active
    }

    /// Number of registered windows.
    #[must_use]
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    // ---------------------------------------------------------------
    // Convenience: activate with history tracking
    // ---------------------------------------------------------------

    /// Like [`activate_window`] but also records the activation in the
    /// history ring buffer.
    pub fn activate_and_record(
        &mut self,
        window_id: WindowId,
        reason: ActivateReason,
        now_ms: u64,
    ) -> Vec<ActivationEvent> {
        let events = self.activate_window(window_id, reason);
        if !events.is_empty() {
            self.history.push(ActivationRecord {
                window_id,
                timestamp_ms: now_ms,
                reason,
            });
        }
        events
    }

    // ---------------------------------------------------------------
    // Internal helpers
    // ---------------------------------------------------------------

    /// Check if `caller_pid` is allowed to set the foreground window.
    fn can_set_foreground(&self, caller_pid: u32, now_ms: u64) -> bool {
        // If no foreground lock is held, anyone can.
        if !self.state.is_foreground_locked(now_ms) {
            return true;
        }

        let lock_pid = match self.state.foreground_lock_pid {
            Some(p) => p,
            None => return true,
        };

        // The locking process itself can always set foreground.
        if caller_pid == lock_pid {
            return true;
        }

        // The current foreground process can always set foreground.
        if let Some(fg) = self.state.foreground_window {
            if let Some(info) = self.windows.get(&fg) {
                if info.pid == caller_pid {
                    return true;
                }
            }
        }

        // Explicitly allowed processes.
        if self.allowed_foreground_pids.contains(&caller_pid) {
            return true;
        }

        // Input-awakened processes.
        if self.input_awakened_pids.contains(&caller_pid) {
            return true;
        }

        false
    }
}

impl Default for FocusManager {
    fn default() -> Self {
        Self::new()
    }
}
