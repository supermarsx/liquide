/// Lock screen configuration
#[derive(Debug, Clone)]
pub struct LockScreenConfig {
    /// Auto-lock after idle (seconds). None = never auto-lock.
    pub auto_lock_timeout_secs: Option<u64>,
    /// Lock on suspend/sleep
    pub lock_on_suspend: bool,
    /// Lock on lid close (laptops)
    pub lock_on_lid_close: bool,
    /// Show notifications on lock screen
    pub show_notifications: bool,
    /// Show clock on lock screen
    pub show_clock: bool,
    /// Show user avatar
    pub show_avatar: bool,
    /// Custom background image path (None = blur desktop)
    pub background_image: Option<String>,
    /// Background blur radius (if no custom image)
    pub blur_radius: f32,
    /// Background dim opacity (0.0 - 1.0)
    pub dim_opacity: f32,
    /// Allow switching users from lock screen
    pub allow_user_switch: bool,
    /// Show power options (shutdown/restart) on lock screen
    pub show_power_options: bool,
    /// Max failed attempts before temporary lockout
    pub max_failed_attempts: u32,
    /// Lockout duration in seconds after max failed attempts
    pub lockout_duration_secs: u64,
    /// Grace period in seconds. This ONLY affects how soon the password prompt
    /// is revealed after locking (a renderer hint); it NEVER unlocks the screen
    /// without a successful authentication. Default is 0 (no grace window).
    /// See t49-e8-F5: a non-zero grace window must never bypass auth.
    pub grace_period_secs: u64,
}

impl Default for LockScreenConfig {
    fn default() -> Self {
        Self {
            auto_lock_timeout_secs: Some(300), // 5 minutes
            lock_on_suspend: true,
            lock_on_lid_close: true,
            show_notifications: true,
            show_clock: true,
            show_avatar: true,
            background_image: None,
            blur_radius: 20.0,
            dim_opacity: 0.4,
            allow_user_switch: false,
            show_power_options: true,
            max_failed_attempts: 5,
            lockout_duration_secs: 30,
            // Secure default: no grace window. Even when non-zero, the grace
            // period never bypasses authentication (see t49-e8-F5).
            grace_period_secs: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sensible() {
        let cfg = LockScreenConfig::default();
        assert_eq!(cfg.auto_lock_timeout_secs, Some(300));
        assert!(cfg.lock_on_suspend);
        assert!(cfg.lock_on_lid_close);
        assert!(cfg.show_clock);
        assert!(cfg.show_power_options);
        assert_eq!(cfg.max_failed_attempts, 5);
        assert!(cfg.lockout_duration_secs > 0);
        assert!(cfg.grace_period_secs < cfg.lockout_duration_secs);
        assert!(cfg.blur_radius > 0.0);
        assert!(cfg.dim_opacity > 0.0 && cfg.dim_opacity <= 1.0);
        assert!(cfg.background_image.is_none());
    }
}
