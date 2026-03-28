use crate::auth::{AuthBackend, AuthResult, Credentials};
use crate::config::LockScreenConfig;
use std::time::{Duration, Instant};

/// Lock screen visual state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenPhase {
    /// Clock/notification display (no auth prompt yet)
    Clock,
    /// Password entry (auth prompt visible)
    PasswordEntry,
    /// Authenticating (spinner/progress)
    Authenticating,
    /// Auth failed (shake animation, error message)
    AuthFailed,
    /// Temporarily locked out after too many failures
    LockedOut,
    /// Unlocking animation
    Unlocking,
}

/// Events emitted by the lock screen to the compositor
#[derive(Debug, Clone)]
pub enum LockScreenEvent {
    /// Lock screen wants to unlock (auth succeeded)
    Unlock,
    /// Lock screen wants to switch user
    SwitchUser,
    /// Power action requested
    Shutdown,
    Restart,
    Suspend,
    /// Auth error to show in notification
    AuthError(String),
    /// Request screen grab for blur background
    RequestBackgroundCapture,
}

/// Actions the shell can send to the lock screen
#[derive(Debug, Clone)]
pub enum LockScreenAction {
    /// User pressed a key (wake from clock phase)
    KeyPress(char),
    /// User pressed enter (submit password)
    Submit,
    /// User pressed escape (back to clock)
    Cancel,
    /// User pressed backspace
    Backspace,
    /// Mouse/touch click at (x, y) in lock screen coords
    Click(f32, f32),
    /// External lock request (from hotkey, lid close, etc.)
    Lock,
    /// Timer tick (for clock update, lockout countdown)
    Tick,
}

/// Lock screen state
pub struct LockScreenState {
    pub config: LockScreenConfig,
    pub phase: ScreenPhase,
    pub password_input: String,
    pub error_message: Option<String>,
    pub failed_attempts: u32,
    pub lockout_until: Option<Instant>,
    pub locked_at: Instant,
    pub last_activity: Instant,
    pub clock_text: String,
    pub date_text: String,
    pub username: String,
    pub display_name: String,
    pub avatar_path: Option<String>,
    pending_events: Vec<LockScreenEvent>,
}

impl LockScreenState {
    pub fn new(
        config: LockScreenConfig,
        username: String,
        display_name: String,
        avatar_path: Option<String>,
    ) -> Self {
        let now = Instant::now();
        Self {
            config,
            phase: ScreenPhase::Clock,
            password_input: String::new(),
            error_message: None,
            failed_attempts: 0,
            lockout_until: None,
            locked_at: now,
            last_activity: now,
            clock_text: String::new(),
            date_text: String::new(),
            username,
            display_name,
            avatar_path,
            pending_events: Vec::new(),
        }
    }

    /// Process an action and return any events
    pub fn handle_action(
        &mut self,
        action: LockScreenAction,
        auth: &dyn AuthBackend,
    ) -> Vec<LockScreenEvent> {
        self.pending_events.clear();
        self.last_activity = Instant::now();

        match action {
            LockScreenAction::Lock => {
                self.phase = ScreenPhase::Clock;
                self.password_input.clear();
                self.error_message = None;
                self.locked_at = Instant::now();
                self.pending_events.push(LockScreenEvent::RequestBackgroundCapture);
            }

            LockScreenAction::KeyPress(c) => {
                if self.is_locked_out() {
                    return self.pending_events.clone();
                }

                // Check grace period
                if self.locked_at.elapsed() < Duration::from_secs(self.config.grace_period_secs) {
                    self.pending_events.push(LockScreenEvent::Unlock);
                    return self.pending_events.clone();
                }

                match self.phase {
                    ScreenPhase::Clock => {
                        self.phase = ScreenPhase::PasswordEntry;
                        self.password_input.clear();
                        self.error_message = None;
                        if !c.is_control() {
                            self.password_input.push(c);
                        }
                    }
                    ScreenPhase::PasswordEntry => {
                        if !c.is_control() {
                            self.password_input.push(c);
                        }
                    }
                    ScreenPhase::AuthFailed => {
                        self.phase = ScreenPhase::PasswordEntry;
                        self.password_input.clear();
                        self.error_message = None;
                        if !c.is_control() {
                            self.password_input.push(c);
                        }
                    }
                    _ => {}
                }
            }

            LockScreenAction::Backspace => {
                if self.phase == ScreenPhase::PasswordEntry {
                    self.password_input.pop();
                }
            }

            LockScreenAction::Submit => {
                if self.phase == ScreenPhase::PasswordEntry && !self.password_input.is_empty() {
                    self.phase = ScreenPhase::Authenticating;

                    let creds = Credentials {
                        username: self.username.clone(),
                        password: self.password_input.clone(),
                    };

                    let result = auth.authenticate(&creds.username, &creds.password);
                    self.password_input.clear();

                    match result {
                        AuthResult::Success => {
                            self.phase = ScreenPhase::Unlocking;
                            self.failed_attempts = 0;
                            self.pending_events.push(LockScreenEvent::Unlock);
                        }
                        AuthResult::Failed(msg) => {
                            self.failed_attempts += 1;
                            if self.failed_attempts >= self.config.max_failed_attempts {
                                self.phase = ScreenPhase::LockedOut;
                                self.lockout_until = Some(
                                    Instant::now()
                                        + Duration::from_secs(self.config.lockout_duration_secs),
                                );
                                self.error_message = Some(format!(
                                    "Too many attempts. Try again in {} seconds.",
                                    self.config.lockout_duration_secs
                                ));
                            } else {
                                self.phase = ScreenPhase::AuthFailed;
                                let remaining =
                                    self.config.max_failed_attempts - self.failed_attempts;
                                self.error_message = Some(format!(
                                    "{}. {} attempt{} remaining.",
                                    msg,
                                    remaining,
                                    if remaining == 1 { "" } else { "s" }
                                ));
                            }
                        }
                        AuthResult::Locked(_ms) => {
                            self.phase = ScreenPhase::AuthFailed;
                            self.error_message = Some("Account is locked.".into());
                        }
                        AuthResult::RequiresMfa => {
                            self.phase = ScreenPhase::AuthFailed;
                            self.error_message = Some("Multi-factor authentication required.".into());
                        }
                    }
                }
            }

            LockScreenAction::Cancel => match self.phase {
                ScreenPhase::PasswordEntry | ScreenPhase::AuthFailed => {
                    self.phase = ScreenPhase::Clock;
                    self.password_input.clear();
                    self.error_message = None;
                }
                _ => {}
            },

            LockScreenAction::Click(x, y) => {
                self.handle_click(x, y);
            }

            LockScreenAction::Tick => {
                self.update_clock();

                // Check lockout expiry
                if let Some(until) = self.lockout_until {
                    if Instant::now() >= until {
                        self.lockout_until = None;
                        self.failed_attempts = 0;
                        self.phase = ScreenPhase::PasswordEntry;
                        self.error_message = None;
                    } else {
                        let remaining = (until - Instant::now()).as_secs();
                        self.error_message = Some(format!(
                            "Too many attempts. Try again in {} seconds.",
                            remaining
                        ));
                    }
                }
            }
        }

        self.pending_events.clone()
    }

    fn is_locked_out(&self) -> bool {
        self.lockout_until
            .map(|until| Instant::now() < until)
            .unwrap_or(false)
    }

    fn handle_click(&mut self, _x: f32, _y: f32) {
        // Click regions would be defined by the renderer
        // For now, any click on clock phase transitions to password entry
        if self.phase == ScreenPhase::Clock {
            self.phase = ScreenPhase::PasswordEntry;
            self.password_input.clear();
            self.error_message = None;
        }
    }

    fn update_clock(&mut self) {
        // Format current time -- simple approach without chrono
        // The shell will provide formatted time if needed
        // For now, store epoch-based placeholder
        use std::time::SystemTime;
        let secs = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let hours = ((secs % 86400) / 3600) as u32;
        let minutes = ((secs % 3600) / 60) as u32;
        self.clock_text = format!("{:02}:{:02}", hours, minutes);

        // Day of week + date (simplified)
        let days = (secs / 86400) as u32;
        let weekday = match (days + 4) % 7 {
            // Jan 1 1970 was Thursday (4)
            0 => "Sunday",
            1 => "Monday",
            2 => "Tuesday",
            3 => "Wednesday",
            4 => "Thursday",
            5 => "Friday",
            6 => "Saturday",
            _ => "",
        };
        self.date_text = format!("{}", weekday);
    }

    /// Get password display (dots for each character)
    pub fn password_display(&self) -> String {
        "\u{2022}".repeat(self.password_input.len()) // bullet character
    }

    /// Number of password characters entered
    pub fn password_length(&self) -> usize {
        self.password_input.len()
    }

    /// Is the lock screen currently active/visible?
    pub fn is_locked(&self) -> bool {
        self.phase != ScreenPhase::Unlocking
    }

    /// Layout info for rendering (the shell can use these to build DOM)
    pub fn layout_info(&self) -> LockScreenLayout {
        LockScreenLayout {
            phase: self.phase,
            clock_text: self.clock_text.clone(),
            date_text: self.date_text.clone(),
            display_name: self.display_name.clone(),
            avatar_path: self.avatar_path.clone(),
            password_dots: self.password_display(),
            error_message: self.error_message.clone(),
            show_clock: self.config.show_clock,
            show_avatar: self.config.show_avatar,
            show_power_options: self.config.show_power_options,
            allow_user_switch: self.config.allow_user_switch,
            blur_radius: self.config.blur_radius,
            dim_opacity: self.config.dim_opacity,
        }
    }
}

/// Data needed to render the lock screen
#[derive(Debug, Clone)]
pub struct LockScreenLayout {
    pub phase: ScreenPhase,
    pub clock_text: String,
    pub date_text: String,
    pub display_name: String,
    pub avatar_path: Option<String>,
    pub password_dots: String,
    pub error_message: Option<String>,
    pub show_clock: bool,
    pub show_avatar: bool,
    pub show_power_options: bool,
    pub allow_user_switch: bool,
    pub blur_radius: f32,
    pub dim_opacity: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{AuthResult, NullAuth};
    use crate::config::LockScreenConfig;

    /// Auth backend that always rejects
    struct RejectAuth;
    impl AuthBackend for RejectAuth {
        fn authenticate(&self, _username: &str, _credential: &str) -> AuthResult {
            AuthResult::Failed("Incorrect password.".into())
        }
    }

    fn make_state() -> LockScreenState {
        // Use a long grace period of 0 so tests don't auto-unlock
        let mut cfg = LockScreenConfig::default();
        cfg.grace_period_secs = 0;
        LockScreenState::new(cfg, "testuser".into(), "Test User".into(), None)
    }

    #[test]
    fn initial_phase_is_clock() {
        let state = make_state();
        assert_eq!(state.phase, ScreenPhase::Clock);
        assert!(state.is_locked());
    }

    #[test]
    fn keypress_transitions_clock_to_password_entry() {
        let mut state = make_state();
        let auth = NullAuth::new();
        state.handle_action(LockScreenAction::KeyPress('a'), &auth);
        assert_eq!(state.phase, ScreenPhase::PasswordEntry);
        assert_eq!(state.password_input, "a");
    }

    #[test]
    fn typing_accumulates_password() {
        let mut state = make_state();
        let auth = NullAuth::new();
        state.handle_action(LockScreenAction::KeyPress('p'), &auth);
        state.handle_action(LockScreenAction::KeyPress('a'), &auth);
        state.handle_action(LockScreenAction::KeyPress('s'), &auth);
        assert_eq!(state.password_input, "pas");
        assert_eq!(state.password_length(), 3);
        assert_eq!(state.password_display(), "\u{2022}\u{2022}\u{2022}");
    }

    #[test]
    fn backspace_removes_last_char() {
        let mut state = make_state();
        let auth = NullAuth::new();
        state.handle_action(LockScreenAction::KeyPress('a'), &auth);
        state.handle_action(LockScreenAction::KeyPress('b'), &auth);
        state.handle_action(LockScreenAction::Backspace, &auth);
        assert_eq!(state.password_input, "a");
    }

    #[test]
    fn submit_with_null_auth_unlocks() {
        let mut state = make_state();
        let auth = NullAuth::new();
        state.handle_action(LockScreenAction::KeyPress('x'), &auth);
        let events = state.handle_action(LockScreenAction::Submit, &auth);
        assert_eq!(state.phase, ScreenPhase::Unlocking);
        assert!(!state.is_locked());
        assert!(events.iter().any(|e| matches!(e, LockScreenEvent::Unlock)));
    }

    #[test]
    fn submit_empty_password_does_nothing() {
        let mut state = make_state();
        let auth = NullAuth::new();
        // Move to password entry but don't type anything
        state.handle_action(LockScreenAction::Click(0.0, 0.0), &auth);
        assert_eq!(state.phase, ScreenPhase::PasswordEntry);
        state.handle_action(LockScreenAction::Submit, &auth);
        // Should still be in PasswordEntry, not Authenticating
        assert_eq!(state.phase, ScreenPhase::PasswordEntry);
    }

    #[test]
    fn failed_auth_shows_error() {
        let mut state = make_state();
        let auth = RejectAuth;
        state.handle_action(LockScreenAction::KeyPress('x'), &auth);
        state.handle_action(LockScreenAction::Submit, &auth);
        assert_eq!(state.phase, ScreenPhase::AuthFailed);
        assert!(state.error_message.is_some());
        assert_eq!(state.failed_attempts, 1);
    }

    #[test]
    fn lockout_after_max_attempts() {
        let mut state = make_state();
        state.config.max_failed_attempts = 2;
        let auth = RejectAuth;

        // First failure
        state.handle_action(LockScreenAction::KeyPress('x'), &auth);
        state.handle_action(LockScreenAction::Submit, &auth);
        assert_eq!(state.phase, ScreenPhase::AuthFailed);

        // Second failure -> lockout
        state.handle_action(LockScreenAction::KeyPress('y'), &auth);
        state.handle_action(LockScreenAction::Submit, &auth);
        assert_eq!(state.phase, ScreenPhase::LockedOut);
        assert!(state.lockout_until.is_some());
    }

    #[test]
    fn cancel_returns_to_clock() {
        let mut state = make_state();
        let auth = NullAuth::new();
        state.handle_action(LockScreenAction::KeyPress('a'), &auth);
        assert_eq!(state.phase, ScreenPhase::PasswordEntry);
        state.handle_action(LockScreenAction::Cancel, &auth);
        assert_eq!(state.phase, ScreenPhase::Clock);
        assert!(state.password_input.is_empty());
    }

    #[test]
    fn lock_action_resets_state() {
        let mut state = make_state();
        let auth = NullAuth::new();
        state.handle_action(LockScreenAction::KeyPress('a'), &auth);
        let events = state.handle_action(LockScreenAction::Lock, &auth);
        assert_eq!(state.phase, ScreenPhase::Clock);
        assert!(state.password_input.is_empty());
        assert!(events
            .iter()
            .any(|e| matches!(e, LockScreenEvent::RequestBackgroundCapture)));
    }

    #[test]
    fn click_on_clock_transitions_to_password_entry() {
        let mut state = make_state();
        let auth = NullAuth::new();
        state.handle_action(LockScreenAction::Click(100.0, 200.0), &auth);
        assert_eq!(state.phase, ScreenPhase::PasswordEntry);
    }

    #[test]
    fn tick_updates_clock_text() {
        let mut state = make_state();
        let auth = NullAuth::new();
        state.handle_action(LockScreenAction::Tick, &auth);
        assert!(!state.clock_text.is_empty());
        assert!(!state.date_text.is_empty());
    }

    #[test]
    fn layout_info_reflects_state() {
        let mut state = make_state();
        let auth = NullAuth::new();
        state.handle_action(LockScreenAction::Tick, &auth);
        let layout = state.layout_info();
        assert_eq!(layout.phase, ScreenPhase::Clock);
        assert_eq!(layout.display_name, "Test User");
        assert!(layout.show_clock);
    }

    #[test]
    fn auth_failed_then_keypress_clears_error() {
        let mut state = make_state();
        let auth = RejectAuth;
        state.handle_action(LockScreenAction::KeyPress('x'), &auth);
        state.handle_action(LockScreenAction::Submit, &auth);
        assert_eq!(state.phase, ScreenPhase::AuthFailed);
        assert!(state.error_message.is_some());

        // Typing again should clear the error and move to PasswordEntry
        state.handle_action(LockScreenAction::KeyPress('y'), &auth);
        assert_eq!(state.phase, ScreenPhase::PasswordEntry);
        assert!(state.error_message.is_none());
        assert_eq!(state.password_input, "y");
    }
}
