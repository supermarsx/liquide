//! Message dispatcher — routes [`WindowMessage`]s to registered handlers.
//!
//! The [`Dispatcher`] maintains a per-window handler registry and a
//! default-handler callback.  Messages are routed through the hook chain
//! (see [`crate::hooks`]) before reaching the target handler.

use std::collections::HashMap;

use crate::message::{MessageTarget, WindowMessage};
use crate::types::WindowId;

/// The outcome of handling a message.
#[derive(Debug, Clone, PartialEq)]
pub enum MessageResult {
    /// The handler fully processed the message.
    Handled,
    /// The handler did not process the message — the dispatcher should try the
    /// default handler.
    NotHandled,
    /// The handler wants to re-route the message (possibly transformed) to
    /// another target.
    Forward(WindowMessage),
}

/// A handler that can process messages for a window.
///
/// Implementations receive the window ID and the message, and return a
/// [`MessageResult`] describing what happened.
pub trait MessageHandler {
    /// Process a message for the given window.
    fn handle_message(&mut self, window_id: WindowId, message: &WindowMessage) -> MessageResult;
}

/// A simple handler backed by a closure.
///
/// Useful for tests and lightweight one-off handlers.
pub struct FnHandler<F>(pub F)
where
    F: FnMut(WindowId, &WindowMessage) -> MessageResult;

impl<F> MessageHandler for FnHandler<F>
where
    F: FnMut(WindowId, &WindowMessage) -> MessageResult,
{
    fn handle_message(&mut self, window_id: WindowId, message: &WindowMessage) -> MessageResult {
        (self.0)(window_id, message)
    }
}

/// Central message dispatcher.
///
/// The dispatcher keeps a per-window handler table plus an optional default
/// handler that processes messages not claimed by any window handler.
pub struct Dispatcher {
    /// Per-window handlers.
    handlers: HashMap<WindowId, Box<dyn MessageHandler>>,
    /// Optional default handler invoked when the per-window handler returns
    /// `NotHandled` (or when no handler is registered for a window).
    default_handler: Option<Box<dyn MessageHandler>>,
    /// When `true`, the dispatch loop should terminate.
    quit_requested: bool,
    /// Log of dispatched messages (window_id, message clone) — useful for
    /// debugging / testing.  Disabled by default; call `enable_log()`.
    log: Option<Vec<(WindowId, WindowMessage)>>,
}

impl Dispatcher {
    /// Create a new dispatcher with no handlers.
    #[must_use]
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            default_handler: None,
            quit_requested: false,
            log: None,
        }
    }

    // ---------------------------------------------------------------
    // Handler registration
    // ---------------------------------------------------------------

    /// Register a handler for a specific window.
    ///
    /// If a handler was already registered for this window, it is replaced.
    pub fn register_handler(&mut self, window_id: WindowId, handler: Box<dyn MessageHandler>) {
        self.handlers.insert(window_id, handler);
    }

    /// Remove the handler for a window.
    ///
    /// Returns `true` if a handler was present and removed.
    pub fn unregister(&mut self, window_id: WindowId) -> bool {
        self.handlers.remove(&window_id).is_some()
    }

    /// Set the fallback default handler.
    pub fn set_default_handler(&mut self, handler: Box<dyn MessageHandler>) {
        self.default_handler = Some(handler);
    }

    /// Returns `true` if a handler is registered for the given window.
    #[must_use]
    pub fn has_handler(&self, window_id: WindowId) -> bool {
        self.handlers.contains_key(&window_id)
    }

    /// Number of registered per-window handlers.
    #[must_use]
    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }

    // ---------------------------------------------------------------
    // Dispatch
    // ---------------------------------------------------------------

    /// Dispatch a single targeted message.
    ///
    /// 1. If a per-window handler exists, call it.
    /// 2. If the handler returns `NotHandled` (or no handler exists), call the
    ///    default handler.
    /// 3. If the handler returns `Forward(msg)`, dispatch the transformed
    ///    message to the *same* window's default handler.
    pub fn dispatch(&mut self, target: &MessageTarget) -> MessageResult {
        let wid = target.window_id;
        let msg = &target.message;

        // Record in log if enabled.
        if let Some(ref mut log) = self.log {
            log.push((wid, msg.clone()));
        }

        let result = if let Some(handler) = self.handlers.get_mut(&wid) {
            handler.handle_message(wid, msg)
        } else {
            MessageResult::NotHandled
        };

        match result {
            MessageResult::Handled => MessageResult::Handled,
            MessageResult::NotHandled => {
                // Try default handler.
                if let Some(ref mut dh) = self.default_handler {
                    dh.handle_message(wid, msg)
                } else {
                    MessageResult::NotHandled
                }
            }
            MessageResult::Forward(fwd_msg) => {
                // Forward to default handler.
                if let Some(ref mut dh) = self.default_handler {
                    dh.handle_message(wid, &fwd_msg)
                } else {
                    MessageResult::NotHandled
                }
            }
        }
    }

    /// Broadcast a message to *all* registered window handlers.
    ///
    /// Returns a `Vec` of `(WindowId, MessageResult)` pairs.
    pub fn broadcast(&mut self, message: &WindowMessage) -> Vec<(WindowId, MessageResult)> {
        // Collect window IDs first so we don't borrow `self` mutably twice.
        let ids: Vec<WindowId> = self.handlers.keys().copied().collect();
        let mut results = Vec::with_capacity(ids.len());

        for wid in ids {
            if let Some(ref mut log) = self.log {
                log.push((wid, message.clone()));
            }
            if let Some(handler) = self.handlers.get_mut(&wid) {
                let r = handler.handle_message(wid, message);
                results.push((wid, r));
            }
        }
        results
    }

    // ---------------------------------------------------------------
    // Quit
    // ---------------------------------------------------------------

    /// Signal that the application should exit.
    ///
    /// The caller's event loop should check [`is_quit_requested`] after each
    /// dispatch cycle.
    pub fn post_quit(&mut self) {
        self.quit_requested = true;
    }

    /// Whether [`post_quit`] has been called.
    #[must_use]
    pub fn is_quit_requested(&self) -> bool {
        self.quit_requested
    }

    /// Reset the quit flag (e.g. if the user cancelled shutdown).
    pub fn cancel_quit(&mut self) {
        self.quit_requested = false;
    }

    // ---------------------------------------------------------------
    // Logging
    // ---------------------------------------------------------------

    /// Enable the dispatch log.  Previously logged entries are preserved.
    pub fn enable_log(&mut self) {
        if self.log.is_none() {
            self.log = Some(Vec::new());
        }
    }

    /// Return a reference to the dispatch log, if enabled.
    #[must_use]
    pub fn log(&self) -> Option<&[(WindowId, WindowMessage)]> {
        self.log.as_deref()
    }

    /// Clear the dispatch log.
    pub fn clear_log(&mut self) {
        if let Some(ref mut log) = self.log {
            log.clear();
        }
    }
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self::new()
    }
}
