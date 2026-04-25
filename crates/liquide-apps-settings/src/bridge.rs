//! Bridge that maps [`SettingNotification`]s onto [`DesktopCommand`]s
//! published on the shared [`DesktopCommandBus`].
//!
//! The settings runtime stores raw key/value mutations. Downstream
//! subsystems (theme-engine, compositor, audio, network) care about
//! semantic commands. This module translates the former into the latter
//! without either side depending on the other.

use std::sync::Arc;

use liquide_gateway::{DesktopCommand, DesktopCommandBus};

use crate::entry::SettingValue;
use crate::notify::SettingNotification;

/// Emits [`DesktopCommand`]s to subsystems whenever a setting changes.
#[derive(Clone, Default)]
pub struct SettingsBridge {
    bus: DesktopCommandBus,
}

impl SettingsBridge {
    /// Create a bridge with a fresh empty bus.
    #[must_use]
    pub fn new() -> Self {
        Self {
            bus: DesktopCommandBus::new(),
        }
    }

    /// Create a bridge that publishes to an existing bus.
    #[must_use]
    pub fn with_bus(bus: DesktopCommandBus) -> Self {
        Self { bus }
    }

    /// Shared access to the underlying command bus so subsystems can
    /// register their handlers.
    #[must_use]
    pub fn bus(&self) -> &DesktopCommandBus {
        &self.bus
    }

    /// Translate a single notification into zero or more
    /// [`DesktopCommand`]s and dispatch each on the bus. Returns the
    /// total number of handler invocations performed.
    pub fn emit(&self, notification: &SettingNotification) -> usize {
        let Some(cmd) = translate(&notification.key, &notification.value) else {
            return 0;
        };
        self.bus.dispatch(&cmd).len()
    }

    /// Bulk variant — translates and dispatches every notification.
    pub fn emit_all(&self, notifications: &[SettingNotification]) -> usize {
        notifications.iter().map(|n| self.emit(n)).sum()
    }
}

/// Pure translation from a `(key, value)` pair to a [`DesktopCommand`].
/// Exposed so the mapping is unit-testable without spinning up a bus.
#[must_use]
pub fn translate(key: &str, value: &SettingValue) -> Option<DesktopCommand> {
    match (key, value) {
        ("appearance.theme", SettingValue::Text(choice)) => {
            let lower = choice.to_ascii_lowercase();
            match lower.as_str() {
                "dark" => Some(DesktopCommand::SetDarkMode(true)),
                "light" => Some(DesktopCommand::SetDarkMode(false)),
                // "Auto" / other: fire a theme-name command so the
                // theme-engine can resolve its own policy.
                other => Some(DesktopCommand::SetActiveTheme(other.to_string())),
            }
        }
        ("appearance.accent_color", SettingValue::Text(name)) => Some(DesktopCommand::Custom {
            key: "accent_color".into(),
            value: name.clone(),
        }),
        ("network.proxy_enabled", SettingValue::Bool(b)) => {
            Some(DesktopCommand::SetNetworkEnabled(*b))
        }
        ("audio.master_volume", SettingValue::Number(v)) => Some(DesktopCommand::SetAudioVolume {
            device_id: "default".into(),
            volume: (*v as f32).clamp(0.0, 1.0),
        }),
        ("audio.mute", SettingValue::Bool(m)) => Some(DesktopCommand::SetAudioMute {
            device_id: "default".into(),
            muted: *m,
        }),
        ("display.scale", SettingValue::Number(v)) => Some(DesktopCommand::SetDisplayScale {
            display_id: 0,
            scale: *v as f32,
        }),
        _ => None,
    }
}

/// Convenience wrapper — register a stateless handler on the bus given a
/// closure. Useful for wiring ad-hoc subsystem adapters from application
/// code without implementing the trait manually.
pub fn register_fn<F>(bus: &DesktopCommandBus, f: F)
where
    F: Fn(&DesktopCommand) -> Result<(), String> + Send + Sync + 'static,
{
    struct Closure<F>(F);
    impl<F> liquide_gateway::DesktopCommandHandler for Closure<F>
    where
        F: Fn(&DesktopCommand) -> Result<(), String> + Send + Sync + 'static,
    {
        fn handle(&self, cmd: &DesktopCommand) -> Result<(), String> {
            (self.0)(cmd)
        }
    }
    bus.register(Arc::new(Closure(f)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_dark_translates_to_set_dark_mode() {
        let cmd = translate("appearance.theme", &SettingValue::Text("Dark".into())).unwrap();
        assert_eq!(cmd, DesktopCommand::SetDarkMode(true));
    }

    #[test]
    fn theme_light_translates_to_set_dark_mode_false() {
        let cmd = translate("appearance.theme", &SettingValue::Text("Light".into())).unwrap();
        assert_eq!(cmd, DesktopCommand::SetDarkMode(false));
    }

    #[test]
    fn theme_auto_routes_to_active_theme() {
        let cmd = translate("appearance.theme", &SettingValue::Text("Auto".into())).unwrap();
        assert!(matches!(cmd, DesktopCommand::SetActiveTheme(ref s) if s == "auto"));
    }

    #[test]
    fn unknown_key_returns_none() {
        assert!(translate("does.not.exist", &SettingValue::Bool(true)).is_none());
    }

    #[test]
    fn bridge_dispatches_via_bus() {
        let bridge = SettingsBridge::new();
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let c2 = counter.clone();
        register_fn(bridge.bus(), move |_| {
            c2.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        });
        let n = SettingNotification {
            key: "appearance.theme".into(),
            value: SettingValue::Text("Dark".into()),
            timestamp: 0,
        };
        assert_eq!(bridge.emit(&n), 1);
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}
