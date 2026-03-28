use crate::action::*;
use crate::binding::*;
use crate::registry::*;

/// Register the full set of built-in default shortcuts for the desktop environment.
pub fn register_defaults(registry: &mut ShortcutRegistry) {
    let defaults: Vec<(u8, KeyCode, ShortcutAction, ShortcutContext)> = vec![
        // Desktop actions
        (MOD_SUPER, KeyCode::Space, ShortcutAction::Desktop(DesktopAction::ShowOverview), ShortcutContext::Global),
        (MOD_SUPER, KeyCode::L, ShortcutAction::Desktop(DesktopAction::LockScreen), ShortcutContext::Global),
        (MOD_NONE, KeyCode::PrintScreen, ShortcutAction::Desktop(DesktopAction::Screenshot), ShortcutContext::Global),
        (MOD_CTRL | MOD_SHIFT, KeyCode::PrintScreen, ShortcutAction::Desktop(DesktopAction::ScreenshotRegion), ShortcutContext::Global),
        (MOD_SUPER, KeyCode::N, ShortcutAction::Desktop(DesktopAction::ShowNotifications), ShortcutContext::Global),

        // Window actions
        (MOD_ALT, KeyCode::F4, ShortcutAction::Window(WindowAction::Close), ShortcutContext::Global),
        (MOD_SUPER, KeyCode::Left, ShortcutAction::Window(WindowAction::TileLeft), ShortcutContext::Global),
        (MOD_SUPER, KeyCode::Right, ShortcutAction::Window(WindowAction::TileRight), ShortcutContext::Global),
        (MOD_SUPER, KeyCode::Up, ShortcutAction::Window(WindowAction::Maximize), ShortcutContext::Global),
        (MOD_SUPER, KeyCode::Down, ShortcutAction::Window(WindowAction::Minimize), ShortcutContext::Global),
        (MOD_SUPER, KeyCode::F11, ShortcutAction::Window(WindowAction::ToggleFullscreen), ShortcutContext::Global),

        // App actions
        (MOD_SUPER, KeyCode::D, ShortcutAction::Desktop(DesktopAction::ShowOverview), ShortcutContext::Window),
        (MOD_SUPER, KeyCode::E, ShortcutAction::App(AppAction::Launch("file-manager".into())), ShortcutContext::Global),
        (MOD_SUPER, KeyCode::Tab, ShortcutAction::App(AppAction::CycleWindows), ShortcutContext::Global),
        (MOD_ALT, KeyCode::Tab, ShortcutAction::App(AppAction::CycleWindows), ShortcutContext::Window),
        (MOD_CTRL | MOD_ALT, KeyCode::Delete, ShortcutAction::App(AppAction::Launch("task-manager".into())), ShortcutContext::Global),
        (MOD_CTRL | MOD_ALT, KeyCode::T, ShortcutAction::App(AppAction::Launch("terminal".into())), ShortcutContext::Global),

        // System / media keys
        (MOD_NONE, KeyCode::VolumeUp, ShortcutAction::System(SystemAction::VolumeUp), ShortcutContext::Global),
        (MOD_NONE, KeyCode::VolumeDown, ShortcutAction::System(SystemAction::VolumeDown), ShortcutContext::Global),
        (MOD_NONE, KeyCode::VolumeMute, ShortcutAction::System(SystemAction::VolumeMute), ShortcutContext::Global),
        (MOD_NONE, KeyCode::BrightnessUp, ShortcutAction::System(SystemAction::BrightnessUp), ShortcutContext::Global),
        (MOD_NONE, KeyCode::BrightnessDown, ShortcutAction::System(SystemAction::BrightnessDown), ShortcutContext::Global),
        (MOD_NONE, KeyCode::MediaPlay, ShortcutAction::System(SystemAction::MediaPlay), ShortcutContext::Global),
        (MOD_NONE, KeyCode::MediaNext, ShortcutAction::System(SystemAction::MediaNext), ShortcutContext::Global),
        (MOD_NONE, KeyCode::MediaPrev, ShortcutAction::System(SystemAction::MediaPrev), ShortcutContext::Global),

        // Workspace switching: Super+1 through Super+9
        (MOD_SUPER, KeyCode::Digit1, ShortcutAction::Desktop(DesktopAction::SwitchWorkspace(1)), ShortcutContext::Global),
        (MOD_SUPER, KeyCode::Digit2, ShortcutAction::Desktop(DesktopAction::SwitchWorkspace(2)), ShortcutContext::Global),
        (MOD_SUPER, KeyCode::Digit3, ShortcutAction::Desktop(DesktopAction::SwitchWorkspace(3)), ShortcutContext::Global),
        (MOD_SUPER, KeyCode::Digit4, ShortcutAction::Desktop(DesktopAction::SwitchWorkspace(4)), ShortcutContext::Global),
        (MOD_SUPER, KeyCode::Digit5, ShortcutAction::Desktop(DesktopAction::SwitchWorkspace(5)), ShortcutContext::Global),
        (MOD_SUPER, KeyCode::Digit6, ShortcutAction::Desktop(DesktopAction::SwitchWorkspace(6)), ShortcutContext::Global),
        (MOD_SUPER, KeyCode::Digit7, ShortcutAction::Desktop(DesktopAction::SwitchWorkspace(7)), ShortcutContext::Global),
        (MOD_SUPER, KeyCode::Digit8, ShortcutAction::Desktop(DesktopAction::SwitchWorkspace(8)), ShortcutContext::Global),
        (MOD_SUPER, KeyCode::Digit9, ShortcutAction::Desktop(DesktopAction::SwitchWorkspace(9)), ShortcutContext::Global),

        // Move window to workspace: Super+Shift+1 through Super+Shift+9
        (MOD_SUPER | MOD_SHIFT, KeyCode::Digit1, ShortcutAction::Window(WindowAction::MoveToWorkspace(1)), ShortcutContext::Global),
        (MOD_SUPER | MOD_SHIFT, KeyCode::Digit2, ShortcutAction::Window(WindowAction::MoveToWorkspace(2)), ShortcutContext::Global),
        (MOD_SUPER | MOD_SHIFT, KeyCode::Digit3, ShortcutAction::Window(WindowAction::MoveToWorkspace(3)), ShortcutContext::Global),
        (MOD_SUPER | MOD_SHIFT, KeyCode::Digit4, ShortcutAction::Window(WindowAction::MoveToWorkspace(4)), ShortcutContext::Global),
        (MOD_SUPER | MOD_SHIFT, KeyCode::Digit5, ShortcutAction::Window(WindowAction::MoveToWorkspace(5)), ShortcutContext::Global),
        (MOD_SUPER | MOD_SHIFT, KeyCode::Digit6, ShortcutAction::Window(WindowAction::MoveToWorkspace(6)), ShortcutContext::Global),
        (MOD_SUPER | MOD_SHIFT, KeyCode::Digit7, ShortcutAction::Window(WindowAction::MoveToWorkspace(7)), ShortcutContext::Global),
        (MOD_SUPER | MOD_SHIFT, KeyCode::Digit8, ShortcutAction::Window(WindowAction::MoveToWorkspace(8)), ShortcutContext::Global),
        (MOD_SUPER | MOD_SHIFT, KeyCode::Digit9, ShortcutAction::Window(WindowAction::MoveToWorkspace(9)), ShortcutContext::Global),

        // Window tiling (additional)
        (MOD_SUPER | MOD_CTRL, KeyCode::Up, ShortcutAction::Window(WindowAction::TileUp), ShortcutContext::Global),
        (MOD_SUPER | MOD_CTRL, KeyCode::Down, ShortcutAction::Window(WindowAction::TileDown), ShortcutContext::Global),

        // Log out
        (MOD_CTRL | MOD_ALT, KeyCode::Escape, ShortcutAction::Desktop(DesktopAction::LogOut), ShortcutContext::Global),
    ];

    for (mods, key, action, context) in defaults {
        let entry = ShortcutEntry {
            binding: KeyBinding::new(mods, key),
            action,
            context,
            source: ShortcutSource::BuiltIn,
            enabled: true,
        };
        // Defaults should never conflict with each other; panic in tests if they do
        registry
            .register(entry)
            .expect("default shortcut conflict — this is a bug");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_register_without_conflict() {
        let mut reg = ShortcutRegistry::new();
        register_defaults(&mut reg);
        assert!(reg.len() > 30, "expected 30+ defaults, got {}", reg.len());
    }

    #[test]
    fn defaults_no_duplicates() {
        let mut reg = ShortcutRegistry::new();
        register_defaults(&mut reg);

        // Check that all (binding, context) pairs are unique
        let entries = reg.all_entries();
        for i in 0..entries.len() {
            for j in (i + 1)..entries.len() {
                if entries[i].binding == entries[j].binding
                    && entries[i].context == entries[j].context
                {
                    panic!(
                        "duplicate default: {} in {:?} — actions {:?} and {:?}",
                        entries[i].binding.to_string(),
                        entries[i].context,
                        entries[i].action,
                        entries[j].action,
                    );
                }
            }
        }
    }

    #[test]
    fn lookup_alt_f4_close() {
        let mut reg = ShortcutRegistry::new();
        register_defaults(&mut reg);

        let result = reg.lookup(MOD_ALT, &KeyCode::F4, &[ShortcutContext::Global]);
        assert_eq!(
            result,
            Some(&ShortcutAction::Window(WindowAction::Close))
        );
    }

    #[test]
    fn lookup_super_l_lock() {
        let mut reg = ShortcutRegistry::new();
        register_defaults(&mut reg);

        let result = reg.lookup(MOD_SUPER, &KeyCode::L, &[ShortcutContext::Global]);
        assert_eq!(
            result,
            Some(&ShortcutAction::Desktop(DesktopAction::LockScreen))
        );
    }

    #[test]
    fn lookup_screenshot() {
        let mut reg = ShortcutRegistry::new();
        register_defaults(&mut reg);

        let result = reg.lookup(MOD_NONE, &KeyCode::PrintScreen, &[ShortcutContext::Global]);
        assert_eq!(
            result,
            Some(&ShortcutAction::Desktop(DesktopAction::Screenshot))
        );
    }

    #[test]
    fn lookup_screenshot_region() {
        let mut reg = ShortcutRegistry::new();
        register_defaults(&mut reg);

        let result = reg.lookup(
            MOD_CTRL | MOD_SHIFT,
            &KeyCode::PrintScreen,
            &[ShortcutContext::Global],
        );
        assert_eq!(
            result,
            Some(&ShortcutAction::Desktop(DesktopAction::ScreenshotRegion))
        );
    }

    #[test]
    fn lookup_workspace_switch() {
        let mut reg = ShortcutRegistry::new();
        register_defaults(&mut reg);

        for i in 1..=9 {
            let key = match i {
                1 => KeyCode::Digit1, 2 => KeyCode::Digit2, 3 => KeyCode::Digit3,
                4 => KeyCode::Digit4, 5 => KeyCode::Digit5, 6 => KeyCode::Digit6,
                7 => KeyCode::Digit7, 8 => KeyCode::Digit8, 9 => KeyCode::Digit9,
                _ => unreachable!(),
            };
            let result = reg.lookup(MOD_SUPER, &key, &[ShortcutContext::Global]);
            assert_eq!(
                result,
                Some(&ShortcutAction::Desktop(DesktopAction::SwitchWorkspace(i))),
                "workspace {} lookup failed",
                i
            );
        }
    }

    #[test]
    fn lookup_move_to_workspace() {
        let mut reg = ShortcutRegistry::new();
        register_defaults(&mut reg);

        let result = reg.lookup(
            MOD_SUPER | MOD_SHIFT,
            &KeyCode::Digit3,
            &[ShortcutContext::Global],
        );
        assert_eq!(
            result,
            Some(&ShortcutAction::Window(WindowAction::MoveToWorkspace(3)))
        );
    }

    #[test]
    fn lookup_volume_keys() {
        let mut reg = ShortcutRegistry::new();
        register_defaults(&mut reg);

        assert_eq!(
            reg.lookup(MOD_NONE, &KeyCode::VolumeUp, &[ShortcutContext::Global]),
            Some(&ShortcutAction::System(SystemAction::VolumeUp))
        );
        assert_eq!(
            reg.lookup(MOD_NONE, &KeyCode::VolumeDown, &[ShortcutContext::Global]),
            Some(&ShortcutAction::System(SystemAction::VolumeDown))
        );
        assert_eq!(
            reg.lookup(MOD_NONE, &KeyCode::VolumeMute, &[ShortcutContext::Global]),
            Some(&ShortcutAction::System(SystemAction::VolumeMute))
        );
    }

    #[test]
    fn lookup_tiling() {
        let mut reg = ShortcutRegistry::new();
        register_defaults(&mut reg);

        assert_eq!(
            reg.lookup(MOD_SUPER, &KeyCode::Left, &[ShortcutContext::Global]),
            Some(&ShortcutAction::Window(WindowAction::TileLeft))
        );
        assert_eq!(
            reg.lookup(MOD_SUPER, &KeyCode::Right, &[ShortcutContext::Global]),
            Some(&ShortcutAction::Window(WindowAction::TileRight))
        );
    }

    #[test]
    fn lookup_maximize_minimize() {
        let mut reg = ShortcutRegistry::new();
        register_defaults(&mut reg);

        assert_eq!(
            reg.lookup(MOD_SUPER, &KeyCode::Up, &[ShortcutContext::Global]),
            Some(&ShortcutAction::Window(WindowAction::Maximize))
        );
        assert_eq!(
            reg.lookup(MOD_SUPER, &KeyCode::Down, &[ShortcutContext::Global]),
            Some(&ShortcutAction::Window(WindowAction::Minimize))
        );
    }

    #[test]
    fn lookup_media_keys() {
        let mut reg = ShortcutRegistry::new();
        register_defaults(&mut reg);

        assert_eq!(
            reg.lookup(MOD_NONE, &KeyCode::MediaPlay, &[ShortcutContext::Global]),
            Some(&ShortcutAction::System(SystemAction::MediaPlay))
        );
        assert_eq!(
            reg.lookup(MOD_NONE, &KeyCode::MediaNext, &[ShortcutContext::Global]),
            Some(&ShortcutAction::System(SystemAction::MediaNext))
        );
        assert_eq!(
            reg.lookup(MOD_NONE, &KeyCode::MediaPrev, &[ShortcutContext::Global]),
            Some(&ShortcutAction::System(SystemAction::MediaPrev))
        );
    }

    #[test]
    fn defaults_all_enabled() {
        let mut reg = ShortcutRegistry::new();
        register_defaults(&mut reg);
        for entry in reg.all_entries() {
            assert!(entry.enabled, "default {:?} is disabled", entry.action);
        }
    }

    #[test]
    fn defaults_all_builtin_source() {
        let mut reg = ShortcutRegistry::new();
        register_defaults(&mut reg);
        for entry in reg.all_entries() {
            assert_eq!(entry.source, ShortcutSource::BuiltIn);
        }
    }

    #[test]
    fn search_finds_defaults() {
        let mut reg = ShortcutRegistry::new();
        register_defaults(&mut reg);

        let results = reg.search("lock");
        assert!(
            results.iter().any(|e| e.action == ShortcutAction::Desktop(DesktopAction::LockScreen)),
            "search for 'lock' should find LockScreen"
        );
    }
}
