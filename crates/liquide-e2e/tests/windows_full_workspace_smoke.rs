#[cfg(not(target_os = "windows"))]
#[test]
fn windows_only_placeholder() {}

#[cfg(target_os = "windows")]
mod windows {
    use liquide_apps_task_manager::ui::TabId;
    use liquide_e2e::{
        ScriptedScenario, assert_basic_launch_report, run_files_default, run_settings_default,
        run_software_center_default, run_task_manager_window, run_terminal_stub,
        run_text_editor_default,
    };

    #[test]
    fn full_workspace_smoke_boots_all_builtin_apps_sequentially() {
        let scenario = ScriptedScenario::single_frame();

        let files = run_files_default(&scenario).unwrap();
        let settings = run_settings_default(&scenario).unwrap();
        let terminal = run_terminal_stub(&scenario).unwrap();
        let editor = run_text_editor_default(&scenario).unwrap();
        let software_center = run_software_center_default(&scenario).unwrap();
        let task_manager = run_task_manager_window(&scenario, None).unwrap();

        for (name, report) in [
            ("Files", &files.report),
            ("Settings", &settings.report),
            ("Terminal", &terminal.report),
            ("Text Editor", &editor.report),
            ("Software Center", &software_center.report),
            ("Task Manager", &task_manager.report),
        ] {
            let capture = assert_basic_launch_report(report);
            assert!(
                !capture.pixels.is_empty(),
                "{name} should retain a presented frame buffer"
            );
        }

        assert!(!files.state.listing_path.is_empty());
        assert!(settings.state.category_count > 0);
        assert_eq!(
            terminal.state.mode,
            liquide_apps_terminal::TerminalLaunchMode::StubPty
        );
        assert_eq!(editor.state.document_count, 1);
        assert_eq!(software_center.state.repository_count, 3);
        assert_eq!(task_manager.state.active_tab, TabId::Processes);
    }
}