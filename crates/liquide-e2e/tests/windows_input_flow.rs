#[cfg(not(target_os = "windows"))]
#[test]
fn windows_only_placeholder() {}

#[cfg(target_os = "windows")]
mod windows {
    use liquide_apps_task_manager::ui::TabId;
    use liquide_e2e::{
        ScriptedInput, ScriptedScenario, assert_capture_size, run_settings_default,
        run_task_manager_window, run_terminal_stub,
    };
    use liquide_input::keyboard::{KeyCode, Modifiers};
    use liquide_input::mouse::MouseButton;

    fn input_scenario() -> ScriptedScenario {
        ScriptedScenario::new(2).with_inputs([
            ScriptedInput::Resize {
                width: 1280,
                height: 720,
            },
            ScriptedInput::FocusGained,
            ScriptedInput::KeyDown {
                key: KeyCode::Enter,
                modifiers: Modifiers::from_bits(Modifiers::CTRL),
                scancode: 13,
                timestamp_us: 1,
            },
            ScriptedInput::KeyUp {
                key: KeyCode::Enter,
                modifiers: Modifiers::from_bits(Modifiers::CTRL),
                scancode: 13,
                timestamp_us: 2,
            },
            ScriptedInput::MouseMove { x: 48.0, y: 72.0 },
            ScriptedInput::MouseDown {
                button: MouseButton::Left,
                x: 48.0,
                y: 72.0,
            },
            ScriptedInput::MouseUp {
                button: MouseButton::Left,
                x: 48.0,
                y: 72.0,
            },
            ScriptedInput::FocusLost,
        ])
    }

    #[test]
    fn settings_survives_scripted_resize_focus_key_and_mouse_flow() {
        let outcome = run_settings_default(&input_scenario()).unwrap();

        assert_capture_size(&outcome.report, 1280, 720);
        assert!(outcome.state.category_count > 0);
        assert!(outcome.state.entry_count > 0);
    }

    #[test]
    fn terminal_survives_scripted_resize_focus_key_and_mouse_flow() {
        let outcome = run_terminal_stub(&input_scenario()).unwrap();

        assert_capture_size(&outcome.report, 1280, 720);
        assert_eq!(outcome.state.tab_count, 1);
        assert_eq!(outcome.state.shell_label, "stub");
    }

    #[test]
    fn task_manager_survives_scripted_resize_focus_key_and_mouse_flow() {
        let outcome = run_task_manager_window(&input_scenario(), Some(TabId::Performance)).unwrap();

        assert_capture_size(&outcome.report, 1280, 720);
        assert_eq!(outcome.state.active_tab, TabId::Performance);
    }
}
