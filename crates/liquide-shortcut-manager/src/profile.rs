use crate::action::*;
use crate::binding::*;
use crate::registry::*;

/// A named shortcut profile — a set of action-to-binding overrides that can
/// be applied on top of the defaults.
#[derive(Debug, Clone)]
pub struct ShortcutProfile {
    pub name: String,
    pub description: String,
    pub overrides: Vec<(ShortcutAction, KeyBinding)>,
}

/// Built-in profile: standard desktop environment shortcuts (no overrides needed,
/// just the defaults).
pub fn profile_default() -> ShortcutProfile {
    ShortcutProfile {
        name: "Default".into(),
        description: "Standard desktop environment shortcuts".into(),
        overrides: Vec::new(),
    }
}

/// Built-in profile: one-hand shortcuts optimized for compact keyboards.
/// Replaces multi-modifier combos with simpler ones where possible.
pub fn profile_compact() -> ShortcutProfile {
    ShortcutProfile {
        name: "Compact".into(),
        description: "One-hand shortcuts optimized for small keyboards".into(),
        overrides: vec![
            // Close window: Alt+F4 → Alt+W
            (
                ShortcutAction::Window(WindowAction::Close),
                KeyBinding::new(MOD_ALT, KeyCode::W),
            ),
            // Screenshot region: Ctrl+Shift+PrintScreen → Alt+S
            (
                ShortcutAction::Desktop(DesktopAction::ScreenshotRegion),
                KeyBinding::new(MOD_ALT, KeyCode::S),
            ),
            // Toggle fullscreen: Super+F11 → Super+F
            (
                ShortcutAction::Window(WindowAction::ToggleFullscreen),
                KeyBinding::new(MOD_SUPER, KeyCode::F),
            ),
            // Terminal: Ctrl+Alt+T → Super+T
            (
                ShortcutAction::App(AppAction::Launch("terminal".into())),
                KeyBinding::new(MOD_SUPER, KeyCode::T),
            ),
        ],
    }
}

/// Built-in profile: simpler shortcuts with no multi-modifier combos, designed
/// for accessibility.
pub fn profile_accessibility() -> ShortcutProfile {
    ShortcutProfile {
        name: "Accessibility".into(),
        description: "Simple shortcuts with no multi-modifier combinations".into(),
        overrides: vec![
            // Close window: Alt+F4 → Super+W
            (
                ShortcutAction::Window(WindowAction::Close),
                KeyBinding::new(MOD_SUPER, KeyCode::W),
            ),
            // Screenshot region: Ctrl+Shift+PrintScreen → Super+S
            (
                ShortcutAction::Desktop(DesktopAction::ScreenshotRegion),
                KeyBinding::new(MOD_SUPER, KeyCode::S),
            ),
            // Task manager: Ctrl+Alt+Delete → Super+M
            (
                ShortcutAction::App(AppAction::Launch("task-manager".into())),
                KeyBinding::new(MOD_SUPER, KeyCode::M),
            ),
            // Log out: Ctrl+Alt+Escape → Super+Q
            (
                ShortcutAction::Desktop(DesktopAction::LogOut),
                KeyBinding::new(MOD_SUPER, KeyCode::Q),
            ),
            // Terminal: Ctrl+Alt+T → Super+T
            (
                ShortcutAction::App(AppAction::Launch("terminal".into())),
                KeyBinding::new(MOD_SUPER, KeyCode::T),
            ),
        ],
    }
}

/// Apply a profile to a registry: for each override, rebind the action to
/// the profile's binding. Non-overridden bindings are left unchanged.
/// Conflicts during rebinding are silently skipped (the override is not applied).
pub fn apply_profile(registry: &mut ShortcutRegistry, profile: &ShortcutProfile) {
    for (action, new_binding) in &profile.overrides {
        // Attempt rebind; if it conflicts, skip this override
        let _ = registry.rebind(action, *new_binding);
    }
}

/// Export the current state of a registry as a profile. The resulting profile
/// contains one override entry per registered shortcut.
pub fn export_profile(registry: &ShortcutRegistry) -> ShortcutProfile {
    let overrides = registry
        .all_entries()
        .iter()
        .map(|e| (e.action.clone(), e.binding))
        .collect();

    ShortcutProfile {
        name: "Exported".into(),
        description: "Exported from current configuration".into(),
        overrides,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defaults::register_defaults;

    #[test]
    fn default_profile_has_no_overrides() {
        let profile = profile_default();
        assert_eq!(profile.name, "Default");
        assert!(profile.overrides.is_empty());
    }

    #[test]
    fn compact_profile_has_overrides() {
        let profile = profile_compact();
        assert_eq!(profile.name, "Compact");
        assert!(!profile.overrides.is_empty());
    }

    #[test]
    fn accessibility_profile_has_overrides() {
        let profile = profile_accessibility();
        assert_eq!(profile.name, "Accessibility");
        assert!(!profile.overrides.is_empty());
    }

    #[test]
    fn apply_compact_profile() {
        let mut reg = ShortcutRegistry::new();
        register_defaults(&mut reg);

        let profile = profile_compact();
        apply_profile(&mut reg, &profile);

        // Alt+W should now close window (was Alt+F4)
        let result = reg.lookup(MOD_ALT, &KeyCode::W, &[ShortcutContext::Global]);
        assert_eq!(result, Some(&ShortcutAction::Window(WindowAction::Close)));

        // Alt+F4 should no longer close window
        let result = reg.lookup(MOD_ALT, &KeyCode::F4, &[ShortcutContext::Global]);
        assert!(
            result.is_none() || result != Some(&ShortcutAction::Window(WindowAction::Close)),
            "Alt+F4 should no longer be Close after compact profile"
        );
    }

    #[test]
    fn apply_accessibility_profile() {
        let mut reg = ShortcutRegistry::new();
        register_defaults(&mut reg);

        let profile = profile_accessibility();
        apply_profile(&mut reg, &profile);

        // Super+W should close window
        let result = reg.lookup(MOD_SUPER, &KeyCode::W, &[ShortcutContext::Global]);
        assert_eq!(result, Some(&ShortcutAction::Window(WindowAction::Close)));
    }

    #[test]
    fn apply_default_profile_is_noop() {
        let mut reg = ShortcutRegistry::new();
        register_defaults(&mut reg);
        let before = reg.len();

        let profile = profile_default();
        apply_profile(&mut reg, &profile);

        assert_eq!(reg.len(), before);
    }

    #[test]
    fn export_profile_roundtrip() {
        let mut reg = ShortcutRegistry::new();
        register_defaults(&mut reg);

        let exported = export_profile(&reg);
        assert_eq!(exported.name, "Exported");
        assert_eq!(exported.overrides.len(), reg.len());

        // Every action+binding pair in the export matches the registry
        for (action, binding) in &exported.overrides {
            let found = reg
                .all_entries()
                .iter()
                .any(|e| e.action == *action && e.binding == *binding);
            assert!(
                found,
                "exported entry {:?} / {} not found in registry",
                action,
                binding.to_string()
            );
        }
    }

    #[test]
    fn export_empty_registry() {
        let reg = ShortcutRegistry::new();
        let exported = export_profile(&reg);
        assert!(exported.overrides.is_empty());
    }

    #[test]
    fn apply_profile_with_conflict_skips_gracefully() {
        let mut reg = ShortcutRegistry::new();
        register_defaults(&mut reg);

        // Create a profile that tries to rebind Close to Super+L (which is LockScreen)
        let conflicting_profile = ShortcutProfile {
            name: "Conflicting".into(),
            description: "Has a conflict".into(),
            overrides: vec![(
                ShortcutAction::Window(WindowAction::Close),
                KeyBinding::new(MOD_SUPER, KeyCode::L),
            )],
        };

        // Should not panic, conflict is silently skipped
        apply_profile(&mut reg, &conflicting_profile);

        // LockScreen should still be on Super+L
        let result = reg.lookup(MOD_SUPER, &KeyCode::L, &[ShortcutContext::Global]);
        assert_eq!(
            result,
            Some(&ShortcutAction::Desktop(DesktopAction::LockScreen))
        );
    }

    #[test]
    fn profile_descriptions() {
        assert!(!profile_default().description.is_empty());
        assert!(!profile_compact().description.is_empty());
        assert!(!profile_accessibility().description.is_empty());
    }
}
