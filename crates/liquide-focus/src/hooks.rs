//! Message hooks — inspect, transform, or block messages before dispatch.
//!
//! A [`HookChain`] is an ordered pipeline of [`MessageHook`] implementations.
//! Each hook sees the message and can:
//!
//! * **Pass** it unchanged to the next hook.
//! * **Block** it (the message is silently discarded).
//! * **Transform** it into a different [`WindowMessage`].
//!
//! Use cases include global hotkey interception, accessibility event
//! injection, and message-level debugging / logging.

use crate::message::WindowMessage;
use crate::types::WindowId;

/// Opaque hook identifier returned by [`HookChain::install_hook`].
pub type HookId = u64;

/// The result of a hook inspecting a message.
#[derive(Debug, Clone, PartialEq)]
pub enum HookResult {
    /// Let the message continue to the next hook (or to the handler).
    Pass,
    /// Discard the message entirely.
    Block,
    /// Replace the message with a transformed version.
    Transform(WindowMessage),
}

/// A hook that can inspect and optionally modify or block a message.
pub trait MessageHook {
    /// Inspect a message destined for `window_id`.
    ///
    /// Called before the message reaches the window's handler.
    fn filter(&mut self, window_id: WindowId, message: &WindowMessage) -> HookResult;
}

/// A hook backed by a closure.
pub struct FnHook<F>(pub F)
where
    F: FnMut(WindowId, &WindowMessage) -> HookResult;

impl<F> MessageHook for FnHook<F>
where
    F: FnMut(WindowId, &WindowMessage) -> HookResult,
{
    fn filter(&mut self, window_id: WindowId, message: &WindowMessage) -> HookResult {
        (self.0)(window_id, message)
    }
}

/// An entry in the hook chain.
struct HookEntry {
    id: HookId,
    hook: Box<dyn MessageHook>,
}

/// An ordered chain of message hooks.
///
/// Hooks are evaluated in installation order (first installed = first to see
/// the message).  If any hook returns [`HookResult::Block`], the message is
/// dropped immediately.  If a hook returns [`HookResult::Transform`], the
/// replacement message is fed to subsequent hooks.
pub struct HookChain {
    hooks: Vec<HookEntry>,
    next_id: HookId,
}

impl HookChain {
    /// Create an empty hook chain.
    #[must_use]
    pub fn new() -> Self {
        Self {
            hooks: Vec::new(),
            next_id: 1,
        }
    }

    /// Install a hook at the end of the chain.
    ///
    /// Returns a [`HookId`] that can be used with [`remove_hook`].
    pub fn install_hook(&mut self, hook: Box<dyn MessageHook>) -> HookId {
        let id = self.next_id;
        self.next_id += 1;
        self.hooks.push(HookEntry { id, hook });
        id
    }

    /// Remove a previously installed hook.
    ///
    /// Returns `true` if the hook was found and removed.
    pub fn remove_hook(&mut self, id: HookId) -> bool {
        let before = self.hooks.len();
        self.hooks.retain(|e| e.id != id);
        self.hooks.len() < before
    }

    /// Run a message through the hook chain.
    ///
    /// Returns `Some(message)` if the message survived all hooks (possibly
    /// transformed), or `None` if any hook blocked it.
    pub fn run(&mut self, window_id: WindowId, message: WindowMessage) -> Option<WindowMessage> {
        let mut current = message;

        for entry in &mut self.hooks {
            match entry.hook.filter(window_id, &current) {
                HookResult::Pass => { /* continue with `current` unchanged */ }
                HookResult::Block => return None,
                HookResult::Transform(replacement) => {
                    current = replacement;
                }
            }
        }

        Some(current)
    }

    /// Number of installed hooks.
    #[must_use]
    pub fn len(&self) -> usize {
        self.hooks.len()
    }

    /// Whether the chain has no hooks.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.hooks.is_empty()
    }

    /// Remove all hooks.
    pub fn clear(&mut self) {
        self.hooks.clear();
    }
}

impl Default for HookChain {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for HookChain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HookChain")
            .field("count", &self.hooks.len())
            .field("next_id", &self.next_id)
            .finish()
    }
}
