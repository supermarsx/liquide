//! Host-side application-view factory (t70-s6).
//!
//! The desktop shell (`liquide-shell`) is deliberately app-agnostic: it knows
//! only the [`liquide_interop::AppView`] trait and stores a `Box<dyn AppView>`
//! per window. The *host* (this session crate) is what links the concrete
//! built-in app crates and constructs their runtimes. This module supplies the
//! factory closure the shell calls from `open_app_window`: given the window's
//! `app_id`, it builds the matching app's runtime (already wrapped as a
//! `dyn AppView` in each app crate) so the window runs the real app, renders its
//! real content view, and receives keyboard input.
//!
//! ## `app_id` reconcile
//!
//! The shell's `open_app_window` stores the *short* reverse-DNS ids
//! (`com.liquide.terminal`, …) on each `Window`, while the app crates expose the
//! canonical `com.liquide.apps.*` constants. The factory accepts BOTH spellings
//! so registration succeeds regardless of which id the launch path used.

use liquide_interop::AppView;

/// Build the per-window [`AppView`] for `app_id`, or `None` if this host does
/// not back the id with a real built-in app (e.g. browser/calculator have no
/// app crate yet → the shell keeps its placeholder painting for those).
pub(super) fn build_app_view(app_id: &str) -> Option<Box<dyn AppView>> {
    match app_id {
        // Terminal — start a real tab so the PTY/VT grid is live.
        "com.liquide.terminal" | liquide_apps_terminal::TERMINAL_APP_ID => {
            let mut rt = liquide_apps_terminal::TerminalRuntime::new(
                liquide_apps_terminal::TerminalConfig::default(),
            );
            // Start a real PTY-backed tab so the window has a live VT grid.
            let _ = rt.new_tab(None);
            Some(Box::new(rt))
        }
        // Text editor — open an empty document to type into.
        "com.liquide.text-editor" | "com.liquide.editor" | liquide_apps_text_editor::APP_ID => {
            let mut rt = liquide_apps_text_editor::EditorRuntime::new(
                liquide_apps_text_editor::EditorConfig::default(),
            );
            let _ = rt.new_document();
            Some(Box::new(rt))
        }
        "com.liquide.files" | liquide_apps_files::FILES_APP_ID => {
            let rt = liquide_apps_files::FilesRuntime::new(
                liquide_apps_files::FilesConfig::default(),
            );
            Some(Box::new(rt))
        }
        "com.liquide.settings" | liquide_apps_settings::SETTINGS_APP_ID => {
            let rt = liquide_apps_settings::SettingsRuntime::new(
                liquide_apps_settings::SettingsConfig::default(),
            );
            Some(Box::new(rt))
        }
        "com.liquide.taskmanager"
        | "com.liquide.task-manager"
        | liquide_apps_task_manager::APP_ID => {
            let rt = liquide_apps_task_manager::TaskManagerRuntime::new(
                liquide_apps_task_manager::TaskManagerConfig::default(),
            );
            Some(Box::new(rt))
        }
        "com.liquide.software-center"
        | "com.liquide.softwarecenter"
        | liquide_apps_software_center::APP_ID => {
            let rt = liquide_apps_software_center::SoftwareCenterRuntime::new(
                liquide_apps_software_center::SoftwareCenterConfig::default(),
            );
            Some(Box::new(rt))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factory_backs_all_six_apps_by_short_id() {
        for id in [
            "com.liquide.terminal",
            "com.liquide.text-editor",
            "com.liquide.files",
            "com.liquide.settings",
            "com.liquide.taskmanager",
            "com.liquide.software-center",
        ] {
            let view = build_app_view(id);
            assert!(view.is_some(), "factory should back {id}");
        }
    }

    #[test]
    fn factory_backs_all_six_apps_by_canonical_id() {
        for id in [
            liquide_apps_terminal::TERMINAL_APP_ID,
            liquide_apps_text_editor::APP_ID,
            liquide_apps_files::FILES_APP_ID,
            liquide_apps_settings::SETTINGS_APP_ID,
            liquide_apps_task_manager::APP_ID,
            liquide_apps_software_center::APP_ID,
        ] {
            assert!(build_app_view(id).is_some(), "factory should back {id}");
        }
    }

    #[test]
    fn factory_returns_none_for_unbacked_app() {
        assert!(build_app_view("com.liquide.browser").is_none());
        assert!(build_app_view("com.liquide.calculator").is_none());
    }

    #[test]
    fn terminal_view_renders_real_grid_and_takes_input() {
        let mut view = build_app_view("com.liquide.terminal").expect("terminal view");
        // A real terminal grid has many rows (placeholder was a single label).
        let model = view.content_view(80, 24);
        assert!(model.rows.len() >= 2, "expected grid, got {}", model.rows.len());
        // Typing routes into the model without error.
        let _ = view.handle_text("echo hi");
    }
}
