/// Desktop events that can trigger sound effects.
///
/// These map roughly to the freedesktop.org sound naming specification
/// categories: login/logout, notifications, window management, device,
/// battery, and dialog sounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundEvent {
    // Session
    Login,
    Logout,
    Lock,
    Unlock,

    // Notifications
    NotificationDefault,
    NotificationUrgent,
    NotificationChat,

    // Window management
    WindowOpen,
    WindowClose,
    WindowMinimize,
    WindowMaximize,

    // System actions
    VolumeChange,
    ScreenCapture,

    // Dialogs
    Error,
    Warning,
    Question,
    Information,

    // Desktop actions
    TrashEmpty,
    DeviceConnect,
    DeviceDisconnect,

    // Battery
    BatteryLow,
    BatteryFull,

    // Extended session
    DesktopLogin,
    SessionStart,
}

impl SoundEvent {
    /// Returns a stable string identifier for this event, suitable for
    /// use as a key in config files or freedesktop sound theme index files.
    pub fn as_str(&self) -> &'static str {
        match self {
            SoundEvent::Login => "login",
            SoundEvent::Logout => "logout",
            SoundEvent::Lock => "lock",
            SoundEvent::Unlock => "unlock",
            SoundEvent::NotificationDefault => "notification-default",
            SoundEvent::NotificationUrgent => "notification-urgent",
            SoundEvent::NotificationChat => "notification-chat",
            SoundEvent::WindowOpen => "window-open",
            SoundEvent::WindowClose => "window-close",
            SoundEvent::WindowMinimize => "window-minimize",
            SoundEvent::WindowMaximize => "window-maximize",
            SoundEvent::VolumeChange => "volume-change",
            SoundEvent::ScreenCapture => "screen-capture",
            SoundEvent::Error => "error",
            SoundEvent::Warning => "warning",
            SoundEvent::Question => "question",
            SoundEvent::Information => "information",
            SoundEvent::TrashEmpty => "trash-empty",
            SoundEvent::DeviceConnect => "device-connect",
            SoundEvent::DeviceDisconnect => "device-disconnect",
            SoundEvent::BatteryLow => "battery-low",
            SoundEvent::BatteryFull => "battery-full",
            SoundEvent::DesktopLogin => "desktop-login",
            SoundEvent::SessionStart => "session-start",
        }
    }

    /// Parse a string identifier back into a SoundEvent.
    pub fn from_str(s: &str) -> Option<SoundEvent> {
        match s {
            "login" => Some(SoundEvent::Login),
            "logout" => Some(SoundEvent::Logout),
            "lock" => Some(SoundEvent::Lock),
            "unlock" => Some(SoundEvent::Unlock),
            "notification-default" => Some(SoundEvent::NotificationDefault),
            "notification-urgent" => Some(SoundEvent::NotificationUrgent),
            "notification-chat" => Some(SoundEvent::NotificationChat),
            "window-open" => Some(SoundEvent::WindowOpen),
            "window-close" => Some(SoundEvent::WindowClose),
            "window-minimize" => Some(SoundEvent::WindowMinimize),
            "window-maximize" => Some(SoundEvent::WindowMaximize),
            "volume-change" => Some(SoundEvent::VolumeChange),
            "screen-capture" => Some(SoundEvent::ScreenCapture),
            "error" => Some(SoundEvent::Error),
            "warning" => Some(SoundEvent::Warning),
            "question" => Some(SoundEvent::Question),
            "information" => Some(SoundEvent::Information),
            "trash-empty" => Some(SoundEvent::TrashEmpty),
            "device-connect" => Some(SoundEvent::DeviceConnect),
            "device-disconnect" => Some(SoundEvent::DeviceDisconnect),
            "battery-low" => Some(SoundEvent::BatteryLow),
            "battery-full" => Some(SoundEvent::BatteryFull),
            "desktop-login" => Some(SoundEvent::DesktopLogin),
            "session-start" => Some(SoundEvent::SessionStart),
            _ => None,
        }
    }

    /// Returns all sound event variants.
    pub fn all() -> &'static [SoundEvent] {
        &[
            SoundEvent::Login,
            SoundEvent::Logout,
            SoundEvent::Lock,
            SoundEvent::Unlock,
            SoundEvent::NotificationDefault,
            SoundEvent::NotificationUrgent,
            SoundEvent::NotificationChat,
            SoundEvent::WindowOpen,
            SoundEvent::WindowClose,
            SoundEvent::WindowMinimize,
            SoundEvent::WindowMaximize,
            SoundEvent::VolumeChange,
            SoundEvent::ScreenCapture,
            SoundEvent::Error,
            SoundEvent::Warning,
            SoundEvent::Question,
            SoundEvent::Information,
            SoundEvent::TrashEmpty,
            SoundEvent::DeviceConnect,
            SoundEvent::DeviceDisconnect,
            SoundEvent::BatteryLow,
            SoundEvent::BatteryFull,
            SoundEvent::DesktopLogin,
            SoundEvent::SessionStart,
        ]
    }
}

impl std::fmt::Display for SoundEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
