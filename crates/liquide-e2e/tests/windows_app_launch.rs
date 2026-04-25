#[cfg(not(target_os = "windows"))]
#[test]
fn windows_only_placeholder() {}

#[cfg(target_os = "windows")]
mod windows {
    use liquide_apps_task_manager::ui::TabId;
    use liquide_e2e::{
        ScriptedScenario, assert_capture_size, run_files_default, run_settings_default,
        run_software_center_default, run_task_manager_window, run_terminal_stub,
        run_text_editor_default,
    };

    #[test]
    fn files_launch_smoke() {
        let outcome = run_files_default(&ScriptedScenario::single_frame()).unwrap();

        assert_capture_size(
            &outcome.report,
            liquide_apps_files::FILES_INITIAL_SIZE.width,
            liquide_apps_files::FILES_INITIAL_SIZE.height,
        );
        assert!(!outcome.state.listing_path.is_empty());
    }

    #[test]
    fn settings_launch_smoke() {
        let outcome = run_settings_default(&ScriptedScenario::single_frame()).unwrap();

        assert_capture_size(
            &outcome.report,
            liquide_apps_settings::SETTINGS_INITIAL_SIZE.width,
            liquide_apps_settings::SETTINGS_INITIAL_SIZE.height,
        );
        assert!(outcome.state.category_count > 0);
        assert!(outcome.state.entry_count > 0);
    }

    #[test]
    fn terminal_launch_smoke_uses_stub_pty() {
        let outcome = run_terminal_stub(&ScriptedScenario::single_frame()).unwrap();

        assert_capture_size(
            &outcome.report,
            liquide_apps_terminal::TERMINAL_INITIAL_SIZE.width,
            liquide_apps_terminal::TERMINAL_INITIAL_SIZE.height,
        );
        assert_eq!(
            outcome.state.mode,
            liquide_apps_terminal::TerminalLaunchMode::StubPty
        );
        assert_eq!(outcome.state.tab_count, 1);
        assert_eq!(outcome.state.shell_label, "stub");
    }

    #[test]
    fn text_editor_launch_smoke() {
        let outcome = run_text_editor_default(&ScriptedScenario::single_frame()).unwrap();

        assert_capture_size(
            &outcome.report,
            liquide_apps_text_editor::DEFAULT_WINDOW_SIZE.width,
            liquide_apps_text_editor::DEFAULT_WINDOW_SIZE.height,
        );
        assert_eq!(outcome.state.document_count, 1);
    }

    #[test]
    fn software_center_launch_smoke() {
        let outcome = run_software_center_default(&ScriptedScenario::single_frame()).unwrap();

        assert_capture_size(
            &outcome.report,
            liquide_apps_software_center::DEFAULT_WINDOW_SIZE.width,
            liquide_apps_software_center::DEFAULT_WINDOW_SIZE.height,
        );
        assert_eq!(outcome.state.repository_count, 3);
    }

    #[test]
    fn task_manager_launch_smoke() {
        let outcome = run_task_manager_window(&ScriptedScenario::single_frame(), None).unwrap();

        assert_capture_size(
            &outcome.report,
            liquide_apps_task_manager::DEFAULT_WINDOW_SIZE.width,
            liquide_apps_task_manager::DEFAULT_WINDOW_SIZE.height,
        );
        assert_eq!(outcome.state.active_tab, TabId::Processes);
        assert_eq!(
            outcome.state.mode,
            liquide_apps_task_manager::TaskManagerLaunchMode::Window
        );
    }
}