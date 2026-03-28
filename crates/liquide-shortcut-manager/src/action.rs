/// Window management actions.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WindowAction {
    Close,
    Minimize,
    Maximize,
    ToggleFullscreen,
    MoveToWorkspace(u32),
    TileLeft,
    TileRight,
    TileUp,
    TileDown,
}

/// Desktop / session actions.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DesktopAction {
    ShowOverview,
    LockScreen,
    LogOut,
    SwitchWorkspace(u32),
    ShowNotifications,
    Screenshot,
    ScreenshotRegion,
}

/// Application actions.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AppAction {
    Launch(String),
    SwitchTo(String),
    CycleWindows,
}

/// System / hardware key actions.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SystemAction {
    VolumeUp,
    VolumeDown,
    VolumeMute,
    BrightnessUp,
    BrightnessDown,
    MediaPlay,
    MediaNext,
    MediaPrev,
}

/// Top-level shortcut action enum.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ShortcutAction {
    Window(WindowAction),
    Desktop(DesktopAction),
    App(AppAction),
    System(SystemAction),
    Custom(String),
}

/// Return a human-readable display name for the given action.
pub fn action_display_name(action: &ShortcutAction) -> &str {
    match action {
        ShortcutAction::Window(w) => match w {
            WindowAction::Close => "Close Window",
            WindowAction::Minimize => "Minimize Window",
            WindowAction::Maximize => "Maximize Window",
            WindowAction::ToggleFullscreen => "Toggle Fullscreen",
            WindowAction::MoveToWorkspace(_) => "Move to Workspace",
            WindowAction::TileLeft => "Tile Left",
            WindowAction::TileRight => "Tile Right",
            WindowAction::TileUp => "Tile Up",
            WindowAction::TileDown => "Tile Down",
        },
        ShortcutAction::Desktop(d) => match d {
            DesktopAction::ShowOverview => "Show Overview",
            DesktopAction::LockScreen => "Lock Screen",
            DesktopAction::LogOut => "Log Out",
            DesktopAction::SwitchWorkspace(_) => "Switch Workspace",
            DesktopAction::ShowNotifications => "Show Notifications",
            DesktopAction::Screenshot => "Screenshot",
            DesktopAction::ScreenshotRegion => "Screenshot Region",
        },
        ShortcutAction::App(a) => match a {
            AppAction::Launch(_) => "Launch Application",
            AppAction::SwitchTo(_) => "Switch to Application",
            AppAction::CycleWindows => "Cycle Windows",
        },
        ShortcutAction::System(s) => match s {
            SystemAction::VolumeUp => "Volume Up",
            SystemAction::VolumeDown => "Volume Down",
            SystemAction::VolumeMute => "Volume Mute",
            SystemAction::BrightnessUp => "Brightness Up",
            SystemAction::BrightnessDown => "Brightness Down",
            SystemAction::MediaPlay => "Media Play/Pause",
            SystemAction::MediaNext => "Media Next",
            SystemAction::MediaPrev => "Media Previous",
        },
        ShortcutAction::Custom(name) => name.as_str(),
    }
}

/// Return the category name for the given action.
pub fn action_category(action: &ShortcutAction) -> &str {
    match action {
        ShortcutAction::Window(_) => "Window",
        ShortcutAction::Desktop(_) => "Desktop",
        ShortcutAction::App(_) => "Application",
        ShortcutAction::System(_) => "System",
        ShortcutAction::Custom(_) => "Custom",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_name_window_actions() {
        assert_eq!(
            action_display_name(&ShortcutAction::Window(WindowAction::Close)),
            "Close Window"
        );
        assert_eq!(
            action_display_name(&ShortcutAction::Window(WindowAction::TileLeft)),
            "Tile Left"
        );
        assert_eq!(
            action_display_name(&ShortcutAction::Window(WindowAction::ToggleFullscreen)),
            "Toggle Fullscreen"
        );
    }

    #[test]
    fn display_name_desktop_actions() {
        assert_eq!(
            action_display_name(&ShortcutAction::Desktop(DesktopAction::ShowOverview)),
            "Show Overview"
        );
        assert_eq!(
            action_display_name(&ShortcutAction::Desktop(DesktopAction::LockScreen)),
            "Lock Screen"
        );
        assert_eq!(
            action_display_name(&ShortcutAction::Desktop(DesktopAction::Screenshot)),
            "Screenshot"
        );
    }

    #[test]
    fn display_name_system_actions() {
        assert_eq!(
            action_display_name(&ShortcutAction::System(SystemAction::VolumeUp)),
            "Volume Up"
        );
        assert_eq!(
            action_display_name(&ShortcutAction::System(SystemAction::MediaPlay)),
            "Media Play/Pause"
        );
    }

    #[test]
    fn display_name_app_actions() {
        assert_eq!(
            action_display_name(&ShortcutAction::App(AppAction::CycleWindows)),
            "Cycle Windows"
        );
        assert_eq!(
            action_display_name(&ShortcutAction::App(AppAction::Launch("term".into()))),
            "Launch Application"
        );
    }

    #[test]
    fn display_name_custom() {
        assert_eq!(
            action_display_name(&ShortcutAction::Custom("my-plugin-action".into())),
            "my-plugin-action"
        );
    }

    #[test]
    fn category_window() {
        assert_eq!(
            action_category(&ShortcutAction::Window(WindowAction::Close)),
            "Window"
        );
    }

    #[test]
    fn category_desktop() {
        assert_eq!(
            action_category(&ShortcutAction::Desktop(DesktopAction::LockScreen)),
            "Desktop"
        );
    }

    #[test]
    fn category_app() {
        assert_eq!(
            action_category(&ShortcutAction::App(AppAction::CycleWindows)),
            "Application"
        );
    }

    #[test]
    fn category_system() {
        assert_eq!(
            action_category(&ShortcutAction::System(SystemAction::VolumeMute)),
            "System"
        );
    }

    #[test]
    fn category_custom() {
        assert_eq!(
            action_category(&ShortcutAction::Custom("x".into())),
            "Custom"
        );
    }
}
