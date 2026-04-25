//! Desktop command channel — narrow in-process IPC bus for routing
//! configuration commands from the Settings app (or any privileged
//! desktop-facing caller) to subsystem handlers (theme-engine, compositor,
//! audio, network, etc.).
//!
//! Added by t9-e15 as the "minimal transport" referenced in
//! `.orchestration/plans/t9.md` §Phase 3 when no existing command channel
//! was found on `liquide-gateway`. Scope is intentionally surgical: an
//! enum of commands, a handler trait, and a simple fan-out bus.
//!
//! This is **not** a generalised message queue — callers that need
//! queuing / persistence / cross-process delivery should layer a real
//! transport on top. For the Phase-3 dark-mode round-trip requirement
//! a synchronous `Arc`-share is sufficient.

use std::sync::{Arc, RwLock};

/// A configuration command emitted by a desktop-facing app and consumed
/// by a subsystem (theme-engine / compositor / display / audio / network).
#[derive(Debug, Clone, PartialEq)]
pub enum DesktopCommand {
    /// Switch the active theme between its light and dark variant.
    SetDarkMode(bool),
    /// Activate a specific named theme (by `theme_id`).
    SetActiveTheme(String),
    /// Change logical DPI / scale for a display.
    SetDisplayScale { display_id: u32, scale: f32 },
    /// Set the master volume for an audio output device (0.0..=1.0).
    SetAudioVolume { device_id: String, volume: f32 },
    /// Mute or unmute an audio device.
    SetAudioMute { device_id: String, muted: bool },
    /// Toggle networking on/off.
    SetNetworkEnabled(bool),
    /// Request the compositor reload its configuration.
    CompositorReload,
    /// Unstructured key/value for forward-compatible extensions.
    Custom { key: String, value: String },
}

impl DesktopCommand {
    /// Short machine-readable label for logging / routing.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::SetDarkMode(_) => "set_dark_mode",
            Self::SetActiveTheme(_) => "set_active_theme",
            Self::SetDisplayScale { .. } => "set_display_scale",
            Self::SetAudioVolume { .. } => "set_audio_volume",
            Self::SetAudioMute { .. } => "set_audio_mute",
            Self::SetNetworkEnabled(_) => "set_network_enabled",
            Self::CompositorReload => "compositor_reload",
            Self::Custom { .. } => "custom",
        }
    }
}

/// Result of handling a single command. A non-`Ok` outcome does not cancel
/// fan-out: the bus continues delivering to subsequent handlers and
/// aggregates failures.
pub type HandlerResult = Result<(), String>;

/// A subsystem-side consumer of [`DesktopCommand`]s. Implementations should
/// be cheap + non-blocking; long-running work belongs behind an internal
/// channel owned by the subsystem.
pub trait DesktopCommandHandler: Send + Sync {
    /// Inspect a command and, if relevant, apply it.
    fn handle(&self, cmd: &DesktopCommand) -> HandlerResult;
}

/// In-process fan-out bus. Clones share the underlying handler list.
#[derive(Clone, Default)]
pub struct DesktopCommandBus {
    handlers: Arc<RwLock<Vec<Arc<dyn DesktopCommandHandler>>>>,
}

impl DesktopCommandBus {
    #[must_use]
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Register a new handler. All subsequent `dispatch` calls include it.
    pub fn register(&self, handler: Arc<dyn DesktopCommandHandler>) {
        if let Ok(mut guard) = self.handlers.write() {
            guard.push(handler);
        }
    }

    /// Number of registered handlers.
    #[must_use]
    pub fn handler_count(&self) -> usize {
        self.handlers.read().map(|g| g.len()).unwrap_or(0)
    }

    /// Fan the command out to every registered handler. Returns the list
    /// of per-handler outcomes in registration order.
    #[must_use]
    pub fn dispatch(&self, cmd: &DesktopCommand) -> Vec<HandlerResult> {
        let guard = match self.handlers.read() {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };
        guard.iter().map(|h| h.handle(cmd)).collect()
    }
}

impl std::fmt::Debug for DesktopCommandBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DesktopCommandBus")
            .field("handler_count", &self.handler_count())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Counter {
        hits: AtomicUsize,
    }

    impl DesktopCommandHandler for Counter {
        fn handle(&self, _cmd: &DesktopCommand) -> HandlerResult {
            self.hits.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn bus_fans_out_to_all_handlers() {
        let bus = DesktopCommandBus::new();
        let a = Arc::new(Counter {
            hits: AtomicUsize::new(0),
        });
        let b = Arc::new(Counter {
            hits: AtomicUsize::new(0),
        });
        bus.register(a.clone());
        bus.register(b.clone());

        let results = bus.dispatch(&DesktopCommand::SetDarkMode(true));
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(Result::is_ok));
        assert_eq!(a.hits.load(Ordering::SeqCst), 1);
        assert_eq!(b.hits.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn bus_aggregates_errors_without_cancelling() {
        struct Fail;
        impl DesktopCommandHandler for Fail {
            fn handle(&self, _: &DesktopCommand) -> HandlerResult {
                Err("boom".into())
            }
        }
        let bus = DesktopCommandBus::new();
        bus.register(Arc::new(Fail));
        let ok = Arc::new(Counter {
            hits: AtomicUsize::new(0),
        });
        bus.register(ok.clone());

        let results = bus.dispatch(&DesktopCommand::CompositorReload);
        assert_eq!(results.len(), 2);
        assert!(results[0].is_err());
        assert!(results[1].is_ok());
        assert_eq!(ok.hits.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn command_kind_strings() {
        assert_eq!(DesktopCommand::SetDarkMode(false).kind(), "set_dark_mode");
        assert_eq!(
            DesktopCommand::SetActiveTheme("night".into()).kind(),
            "set_active_theme",
        );
        assert_eq!(DesktopCommand::CompositorReload.kind(), "compositor_reload");
    }
}
