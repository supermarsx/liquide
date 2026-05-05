use anyhow::{Context, Result};
use liquide_app_harness::{AppBootstrap, AppRunReport};
use liquide_input::keyboard::{KeyCode, KeyEvent, KeyState, Modifiers};
use liquide_input::mouse::{ButtonState, MouseButton, MouseEvent};
use liquide_platform::event_loop::PlatformEvent;
use liquide_platform::standalone::{StandaloneConfig, StandalonePlatform};
use liquide_platform::window_host::NativeWindowHandle;
use liquide_ui_core::widget::Widget;

pub const SCRIPTED_WINDOW_HANDLE: NativeWindowHandle = NativeWindowHandle(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptedScenarioSurface {
    pub width: u32,
    pub height: u32,
}

impl Default for ScriptedScenarioSurface {
    fn default() -> Self {
        Self {
            width: 1600,
            height: 900,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScriptedInput {
    Resize {
        width: u32,
        height: u32,
    },
    FocusGained,
    FocusLost,
    KeyDown {
        key: KeyCode,
        modifiers: Modifiers,
        scancode: u32,
        timestamp_us: u64,
    },
    KeyUp {
        key: KeyCode,
        modifiers: Modifiers,
        scancode: u32,
        timestamp_us: u64,
    },
    MouseMove {
        x: f32,
        y: f32,
    },
    MouseDown {
        button: MouseButton,
        x: f32,
        y: f32,
    },
    MouseUp {
        button: MouseButton,
        x: f32,
        y: f32,
    },
    Quit,
}

impl ScriptedInput {
    fn into_platform_event(self) -> PlatformEvent {
        match self {
            Self::Resize { width, height } => PlatformEvent::WindowResized {
                handle: SCRIPTED_WINDOW_HANDLE,
                width,
                height,
            },
            Self::FocusGained => PlatformEvent::FocusGained {
                handle: SCRIPTED_WINDOW_HANDLE,
            },
            Self::FocusLost => PlatformEvent::FocusLost {
                handle: SCRIPTED_WINDOW_HANDLE,
            },
            Self::KeyDown {
                key,
                modifiers,
                scancode,
                timestamp_us,
            } => PlatformEvent::KeyInput {
                handle: SCRIPTED_WINDOW_HANDLE,
                event: KeyEvent::new(key, KeyState::Pressed, modifiers, scancode, timestamp_us),
            },
            Self::KeyUp {
                key,
                modifiers,
                scancode,
                timestamp_us,
            } => PlatformEvent::KeyInput {
                handle: SCRIPTED_WINDOW_HANDLE,
                event: KeyEvent::new(key, KeyState::Released, modifiers, scancode, timestamp_us),
            },
            Self::MouseMove { x, y } => PlatformEvent::MouseInput {
                handle: SCRIPTED_WINDOW_HANDLE,
                event: MouseEvent::Move { x, y },
            },
            Self::MouseDown { button, x, y } => PlatformEvent::MouseInput {
                handle: SCRIPTED_WINDOW_HANDLE,
                event: MouseEvent::Button {
                    button,
                    state: ButtonState::Pressed,
                    x,
                    y,
                },
            },
            Self::MouseUp { button, x, y } => PlatformEvent::MouseInput {
                handle: SCRIPTED_WINDOW_HANDLE,
                event: MouseEvent::Button {
                    button,
                    state: ButtonState::Released,
                    x,
                    y,
                },
            },
            Self::Quit => PlatformEvent::Quit,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScriptedScenario {
    pub frames: u32,
    pub surface: ScriptedScenarioSurface,
    pub inputs: Vec<ScriptedInput>,
}

impl ScriptedScenario {
    #[must_use]
    pub fn new(frames: u32) -> Self {
        Self {
            frames,
            surface: ScriptedScenarioSurface::default(),
            inputs: Vec::new(),
        }
    }

    #[must_use]
    pub fn single_frame() -> Self {
        Self::new(1)
    }

    #[must_use]
    pub fn with_surface(mut self, width: u32, height: u32) -> Self {
        self.surface = ScriptedScenarioSurface { width, height };
        self
    }

    #[must_use]
    pub fn with_inputs<I>(mut self, inputs: I) -> Self
    where
        I: IntoIterator<Item = ScriptedInput>,
    {
        self.inputs = inputs.into_iter().collect();
        self
    }
}

pub struct ScenarioOutcome<State> {
    pub state: State,
    pub report: AppRunReport,
}

pub fn run_files_default(
    scenario: &ScriptedScenario,
) -> Result<ScenarioOutcome<liquide_apps_files::FilesLaunchContract>> {
    let state = liquide_apps_files::prepare_launch(liquide_apps_files::FilesConfig::default());
    run_scripted_app(
        liquide_apps_files::app_bootstrap(),
        state,
        liquide_apps_files::build_root,
        scenario,
    )
}

pub fn run_settings_default(
    scenario: &ScriptedScenario,
) -> Result<ScenarioOutcome<liquide_apps_settings::SettingsLaunchContract>> {
    let state =
        liquide_apps_settings::prepare_launch(liquide_apps_settings::SettingsConfig::default());
    run_scripted_app(
        liquide_apps_settings::app_bootstrap(),
        state,
        liquide_apps_settings::build_root,
        scenario,
    )
}

pub fn run_terminal_stub(
    scenario: &ScriptedScenario,
) -> Result<ScenarioOutcome<liquide_apps_terminal::TerminalLaunchContract>> {
    let state = liquide_apps_terminal::prepare_launch(
        liquide_apps_terminal::TerminalConfig::default(),
        liquide_apps_terminal::TerminalLaunchMode::StubPty,
    )?;
    run_scripted_app(
        liquide_apps_terminal::app_bootstrap(),
        state,
        liquide_apps_terminal::build_root,
        scenario,
    )
}

pub fn run_text_editor_default(
    scenario: &ScriptedScenario,
) -> Result<ScenarioOutcome<liquide_apps_text_editor::EditorLaunchState>> {
    let state = liquide_apps_text_editor::default_launch_state(
        liquide_apps_text_editor::EditorConfig::default(),
    );
    run_scripted_app(
        liquide_apps_text_editor::default_bootstrap(),
        state,
        |state| Box::new(liquide_apps_text_editor::build_root_from_state(state)),
        scenario,
    )
}

pub fn run_software_center_default(
    scenario: &ScriptedScenario,
) -> Result<ScenarioOutcome<liquide_apps_software_center::SoftwareCenterLaunchState>> {
    let state = liquide_apps_software_center::default_launch_state(
        liquide_apps_software_center::SoftwareCenterConfig::default(),
    );
    run_scripted_app(
        liquide_apps_software_center::default_bootstrap(),
        state,
        |state| Box::new(liquide_apps_software_center::build_root_from_state(state)),
        scenario,
    )
}

pub fn run_task_manager_window(
    scenario: &ScriptedScenario,
    active_tab: Option<liquide_apps_task_manager::ui::TabId>,
) -> Result<ScenarioOutcome<liquide_apps_task_manager::TaskManagerLaunchState>> {
    run_task_manager_graphical(
        scenario,
        active_tab,
        liquide_apps_task_manager::TaskManagerLaunchMode::Window,
    )
}

pub fn run_task_manager_widget(
    scenario: &ScriptedScenario,
    active_tab: Option<liquide_apps_task_manager::ui::TabId>,
) -> Result<ScenarioOutcome<liquide_apps_task_manager::TaskManagerLaunchState>> {
    run_task_manager_graphical(
        scenario,
        active_tab,
        liquide_apps_task_manager::TaskManagerLaunchMode::Widget,
    )
}

fn run_task_manager_graphical(
    scenario: &ScriptedScenario,
    active_tab: Option<liquide_apps_task_manager::ui::TabId>,
    mode: liquide_apps_task_manager::TaskManagerLaunchMode,
) -> Result<ScenarioOutcome<liquide_apps_task_manager::TaskManagerLaunchState>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("build Tokio runtime for task manager launch state")?;
    let state = runtime.block_on(liquide_apps_task_manager::graphical_launch_state(
        liquide_apps_task_manager::TaskManagerConfig::default(),
        active_tab,
        mode,
    ));
    run_scripted_app(
        liquide_apps_task_manager::bootstrap_for_mode(mode),
        state,
        |state| Box::new(liquide_apps_task_manager::build_graphical_root(state)),
        scenario,
    )
}

fn run_scripted_app<State, Build>(
    bootstrap: AppBootstrap,
    state: State,
    build_root: Build,
    scenario: &ScriptedScenario,
) -> Result<ScenarioOutcome<State>>
where
    State: Clone,
    Build: Fn(&State) -> Box<dyn Widget>,
{
    let platform = StandalonePlatform::new(StandaloneConfig {
        width: scenario.surface.width,
        height: scenario.surface.height,
        hardware_cursor: false,
        ..StandaloneConfig::default()
    })
    .context("construct standalone scripted platform")?;
    let script = platform.script_handle();
    if !scenario.inputs.is_empty() {
        script.push_events(
            scenario
                .inputs
                .iter()
                .copied()
                .map(ScriptedInput::into_platform_event),
        );
    }

    let state_for_root = state.clone();
    let report = bootstrap
        .with_platform(Box::new(platform))
        .run_for_frames_with_report(scenario.frames, move |_cx| build_root(&state_for_root))
        .context("run scripted app scenario")?;

    Ok(ScenarioOutcome { state, report })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_input_translates_to_platform_resize() {
        let event = ScriptedInput::Resize {
            width: 1280,
            height: 720,
        }
        .into_platform_event();

        match event {
            PlatformEvent::WindowResized {
                handle,
                width,
                height,
            } => {
                assert_eq!(handle, SCRIPTED_WINDOW_HANDLE);
                assert_eq!(width, 1280);
                assert_eq!(height, 720);
            }
            other => panic!("unexpected translated event: {other:?}"),
        }
    }

    #[test]
    fn scenario_builder_replaces_inputs() {
        let scenario = ScriptedScenario::new(2)
            .with_surface(1920, 1080)
            .with_inputs([ScriptedInput::FocusGained, ScriptedInput::FocusLost]);

        assert_eq!(scenario.frames, 2);
        assert_eq!(scenario.surface.width, 1920);
        assert_eq!(scenario.surface.height, 1080);
        assert_eq!(scenario.inputs.len(), 2);
    }
}
