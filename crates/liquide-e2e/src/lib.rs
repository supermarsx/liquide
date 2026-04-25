//! Workspace-level end-to-end scenarios for built-in LiquiDE apps.
//!
//! The new app-harness report API gives this crate a deterministic way to
//! boot built-in apps through their production `AppBootstrap` path while
//! driving them with scripted standalone-platform events.

pub mod assertions;
pub mod scenario;

pub use assertions::{assert_basic_launch_report, assert_capture_size};
pub use scenario::{
    ScenarioOutcome, ScriptedInput, ScriptedScenario, ScriptedScenarioSurface,
    run_files_default, run_settings_default, run_software_center_default,
    run_task_manager_widget, run_task_manager_window, run_terminal_stub,
    run_text_editor_default,
};