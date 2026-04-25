//! Window event hook system — enables accessibility tools, automation,
//! screen recording, and keyboard remapping without modifying the shell core.
//!
//! Hooks are registered with a priority, dispatched in priority order,
//! and can suppress or modify events.

/// Hook priority — lower number = called first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct HookPriority(pub i32);

impl HookPriority {
    pub const ACCESSIBILITY: Self = Self(-100);
    pub const SYSTEM: Self = Self(0);
    pub const NORMAL: Self = Self(100);
    pub const LOW: Self = Self(200);
}

/// Result from a hook callback — determines if the event continues propagating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookResult {
    /// Continue propagation to next hook and default handler.
    Continue,
    /// Event was handled — stop propagation.
    Handled,
    /// Transform the event (hook modifies it, propagation continues).
    Modified,
}

/// Hook ID for registration/unregistration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HookId(pub u64);

/// Events that hooks can intercept.
#[derive(Debug, Clone)]
pub enum ShellHookEvent {
    // ── Window lifecycle ────────────────────────────────────────────
    /// A new window was created.
    WindowCreated { window_id: u64 },
    /// A window was closed.
    WindowClosed { window_id: u64 },
    /// A window received focus.
    WindowActivated { window_id: u64 },
    /// A window lost focus.
    WindowDeactivated { window_id: u64 },
    /// A window was moved.
    WindowMoved { window_id: u64, x: i32, y: i32 },
    /// A window was resized.
    WindowResized {
        window_id: u64,
        width: u32,
        height: u32,
    },
    /// A window was minimized.
    WindowMinimized { window_id: u64 },
    /// A window was maximized.
    WindowMaximized { window_id: u64 },
    /// A window was restored from minimized/maximized.
    WindowRestored { window_id: u64 },
    /// A window's title changed.
    WindowTitleChanged { window_id: u64, title: String },

    // ── Input events ────────────────────────────────────────────────
    /// A key was pressed.
    KeyDown { key_code: u32, modifiers: u32 },
    /// A key was released.
    KeyUp { key_code: u32, modifiers: u32 },
    /// The mouse moved.
    MouseMove { x: f32, y: f32 },
    /// A mouse button was pressed or released.
    MouseButton {
        button: u8,
        pressed: bool,
        x: f32,
        y: f32,
    },

    // ── Workspace events ────────────────────────────────────────────
    /// The active workspace changed.
    WorkspaceChanged { from: u32, to: u32 },
    /// A window was moved to a different workspace.
    WindowMovedToWorkspace { window_id: u64, workspace: u32 },

    // ── Shell state ─────────────────────────────────────────────────
    /// The application launcher was opened.
    LauncherOpened,
    /// The application launcher was closed.
    LauncherClosed,
    /// The lock screen was activated.
    LockScreenActivated,
    /// The lock screen was deactivated.
    LockScreenDeactivated,
    /// The shell theme changed.
    ThemeChanged { theme_name: String },

    // ── Custom events (for plugins) ─────────────────────────────────
    /// A custom event with an arbitrary name and data payload.
    Custom { name: String, data: String },
}

/// A registered hook entry.
struct HookEntry {
    id: HookId,
    priority: HookPriority,
    callback: Box<dyn Fn(&ShellHookEvent) -> HookResult + Send + Sync>,
    active: bool,
}

/// The hook manager — holds all registered hooks and dispatches events.
pub struct HookManager {
    hooks: Vec<HookEntry>,
    next_id: u64,
}

impl HookManager {
    /// Create a new, empty hook manager.
    pub fn new() -> Self {
        Self {
            hooks: Vec::new(),
            next_id: 1,
        }
    }

    /// Register a hook callback with the given priority.
    ///
    /// Returns a [`HookId`] that can be used to unregister or disable the hook.
    pub fn register(
        &mut self,
        priority: HookPriority,
        callback: Box<dyn Fn(&ShellHookEvent) -> HookResult + Send + Sync>,
    ) -> HookId {
        let id = HookId(self.next_id);
        self.next_id += 1;

        self.hooks.push(HookEntry {
            id,
            priority,
            callback,
            active: true,
        });

        // Keep sorted by priority (lower = first).
        self.hooks.sort_by_key(|h| h.priority);

        id
    }

    /// Unregister a hook by its ID. Returns `true` if the hook was found and removed.
    pub fn unregister(&mut self, id: HookId) -> bool {
        if let Some(pos) = self.hooks.iter().position(|h| h.id == id) {
            self.hooks.remove(pos);
            true
        } else {
            false
        }
    }

    /// Temporarily enable or disable a hook without removing it.
    pub fn set_active(&mut self, id: HookId, active: bool) {
        if let Some(hook) = self.hooks.iter_mut().find(|h| h.id == id) {
            hook.active = active;
        }
    }

    /// Dispatch an event through the hook chain.
    ///
    /// Hooks are called in priority order.  If any hook returns
    /// [`HookResult::Handled`], propagation stops immediately.
    /// [`HookResult::Modified`] is sticky — if any hook returns it the
    /// overall result is `Modified` (unless a later hook returns `Handled`).
    pub fn dispatch(&self, event: &ShellHookEvent) -> HookResult {
        let mut result = HookResult::Continue;

        for hook in &self.hooks {
            if !hook.active {
                continue;
            }

            match (hook.callback)(event) {
                HookResult::Handled => return HookResult::Handled,
                HookResult::Modified => result = HookResult::Modified,
                HookResult::Continue => {}
            }
        }

        result
    }

    /// Number of registered hooks (active + inactive).
    pub fn hook_count(&self) -> usize {
        self.hooks.len()
    }

    /// Number of currently active hooks.
    pub fn active_count(&self) -> usize {
        self.hooks.iter().filter(|h| h.active).count()
    }

    /// Remove all hooks.
    pub fn clear(&mut self) {
        self.hooks.clear();
    }
}

impl Default for HookManager {
    fn default() -> Self {
        Self::new()
    }
}
