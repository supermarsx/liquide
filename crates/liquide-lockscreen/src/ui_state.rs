/// Lock screen UI state machine.
///
/// Models the user-facing lock screen states, keyboard/media input,
/// and transitions between clock display, password entry, and authentication.

use crate::auth::AuthResult;

/// Visual state of the lock screen.
#[derive(Debug, Clone, PartialEq)]
pub enum LockScreenState {
    /// Clock/wallpaper display (no prompt yet).
    Clock,
    /// Password entry field is active.
    PasswordEntry,
    /// Authentication is in progress (spinner).
    Authenticating,
    /// Authentication failed (shows error message).
    AuthFailed(String),
    /// User switching panel is open.
    Switching,
    /// Media controls overlay.
    MediaControls,
}

/// Key events the lock screen handles.
#[derive(Debug, Clone, PartialEq)]
pub enum LockKey {
    /// A printable character.
    Char(char),
    /// Backspace (delete last character).
    Backspace,
    /// Enter/Return (submit password).
    Enter,
    /// Escape (cancel / return to clock).
    Escape,
    /// Tab (cycle focus, or switch to user list).
    Tab,
}

/// Media key events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKey {
    PlayPause,
    Next,
    Previous,
    VolumeUp,
    VolumeDown,
}

/// Actions emitted by the lock screen model to the shell.
#[derive(Debug, Clone, PartialEq)]
pub enum LockAction {
    /// No action needed.
    None,
    /// Authenticate with the given password.
    Authenticate(String),
    /// Unlock the session (auth succeeded).
    Unlock,
    /// Switch to another user session.
    SwitchUser,
    /// Toggle media play/pause.
    PlayPause,
    /// Skip to next media track.
    MediaNext,
    /// Skip to previous media track.
    MediaPrev,
}

/// Lock screen UI model.
///
/// Manages the full state machine: clock display, password entry,
/// authentication flow, media controls, and user switching.
pub struct LockScreenModel {
    pub state: LockScreenState,
    pub username: String,
    password_buffer: String,
    pub clock_format_24h: bool,
    pub show_notifications: bool,
    pub wallpaper_path: Option<String>,
    pub media_playing: bool,
    pub media_title: Option<String>,
    pub message: Option<String>,
    /// Timer for auto-dismissing error messages (milliseconds remaining).
    error_dismiss_timer_ms: f32,
    /// Duration before auto-dismissing AuthFailed state (ms).
    error_dismiss_duration_ms: f32,
}

impl LockScreenModel {
    /// Create a new lock screen model for the given user.
    pub fn new(username: String) -> Self {
        Self {
            state: LockScreenState::Clock,
            username,
            password_buffer: String::new(),
            clock_format_24h: true,
            show_notifications: true,
            wallpaper_path: None,
            media_playing: false,
            media_title: None,
            message: None,
            error_dismiss_timer_ms: 0.0,
            error_dismiss_duration_ms: 2000.0,
        }
    }

    /// Handle a key event. Returns the action the shell should take.
    pub fn on_key(&mut self, key: LockKey) -> LockAction {
        match &self.state {
            LockScreenState::Clock => {
                // Any key wakes to password entry
                match key {
                    LockKey::Char(c) => {
                        self.state = LockScreenState::PasswordEntry;
                        self.password_buffer.clear();
                        self.password_buffer.push(c);
                    }
                    _ => {
                        self.state = LockScreenState::PasswordEntry;
                        self.password_buffer.clear();
                    }
                }
                LockAction::None
            }
            LockScreenState::PasswordEntry => match key {
                LockKey::Char(c) => {
                    self.password_buffer.push(c);
                    LockAction::None
                }
                LockKey::Backspace => {
                    self.password_buffer.pop();
                    LockAction::None
                }
                LockKey::Enter => {
                    if self.password_buffer.is_empty() {
                        return LockAction::None;
                    }
                    let password = self.password_buffer.clone();
                    self.state = LockScreenState::Authenticating;
                    LockAction::Authenticate(password)
                }
                LockKey::Escape => {
                    self.password_buffer.clear();
                    self.state = LockScreenState::Clock;
                    LockAction::None
                }
                LockKey::Tab => {
                    self.state = LockScreenState::Switching;
                    LockAction::None
                }
            },
            LockScreenState::Authenticating => {
                // Ignore keys while authenticating
                LockAction::None
            }
            LockScreenState::AuthFailed(_) => {
                // Any key transitions back to password entry
                match key {
                    LockKey::Char(c) => {
                        self.state = LockScreenState::PasswordEntry;
                        self.password_buffer.clear();
                        self.password_buffer.push(c);
                        self.error_dismiss_timer_ms = 0.0;
                    }
                    LockKey::Escape => {
                        self.state = LockScreenState::Clock;
                        self.password_buffer.clear();
                        self.error_dismiss_timer_ms = 0.0;
                    }
                    _ => {
                        self.state = LockScreenState::PasswordEntry;
                        self.password_buffer.clear();
                        self.error_dismiss_timer_ms = 0.0;
                    }
                }
                LockAction::None
            }
            LockScreenState::Switching => {
                match key {
                    LockKey::Escape => {
                        self.state = LockScreenState::PasswordEntry;
                    }
                    LockKey::Enter => {
                        return LockAction::SwitchUser;
                    }
                    _ => {}
                }
                LockAction::None
            }
            LockScreenState::MediaControls => {
                match key {
                    LockKey::Escape => {
                        self.state = LockScreenState::Clock;
                    }
                    _ => {}
                }
                LockAction::None
            }
        }
    }

    /// Handle an authentication result.
    pub fn on_auth_result(&mut self, result: AuthResult) -> LockAction {
        self.password_buffer.clear();
        match result {
            AuthResult::Success => {
                self.state = LockScreenState::PasswordEntry; // briefly, then unlock
                LockAction::Unlock
            }
            AuthResult::Failed(msg) => {
                self.state = LockScreenState::AuthFailed(msg);
                self.error_dismiss_timer_ms = self.error_dismiss_duration_ms;
                LockAction::None
            }
            AuthResult::Locked(retry_after_ms) => {
                let msg = format!("Account locked. Retry in {} seconds.", retry_after_ms / 1000);
                self.state = LockScreenState::AuthFailed(msg);
                self.error_dismiss_timer_ms = retry_after_ms as f32;
                LockAction::None
            }
            AuthResult::RequiresMfa => {
                self.state =
                    LockScreenState::AuthFailed("Multi-factor authentication required.".into());
                self.error_dismiss_timer_ms = self.error_dismiss_duration_ms;
                LockAction::None
            }
        }
    }

    /// Handle a media key event.
    pub fn on_media_key(&mut self, key: MediaKey) -> LockAction {
        match key {
            MediaKey::PlayPause => {
                self.media_playing = !self.media_playing;
                LockAction::PlayPause
            }
            MediaKey::Next => LockAction::MediaNext,
            MediaKey::Previous => LockAction::MediaPrev,
            MediaKey::VolumeUp | MediaKey::VolumeDown => {
                // Volume handled at system level, no action needed
                LockAction::None
            }
        }
    }

    /// Tick animation timers and auto-dismiss messages.
    pub fn tick(&mut self, dt_ms: f32) {
        if self.error_dismiss_timer_ms > 0.0 {
            self.error_dismiss_timer_ms -= dt_ms;
            if self.error_dismiss_timer_ms <= 0.0 {
                self.error_dismiss_timer_ms = 0.0;
                if matches!(self.state, LockScreenState::AuthFailed(_)) {
                    self.state = LockScreenState::PasswordEntry;
                    self.password_buffer.clear();
                }
            }
        }
    }

    /// Number of characters in the password buffer (for showing dots).
    pub fn password_length(&self) -> usize {
        self.password_buffer.len()
    }

    /// Get the current state.
    pub fn current_state(&self) -> &LockScreenState {
        &self.state
    }

    /// Set a custom lock message (shown on the lock screen).
    pub fn set_message(&mut self, msg: Option<String>) {
        self.message = msg;
    }

    /// Set media metadata.
    pub fn set_media_info(&mut self, playing: bool, title: Option<String>) {
        self.media_playing = playing;
        self.media_title = title;
    }

    /// Open the media controls overlay.
    pub fn show_media_controls(&mut self) {
        self.state = LockScreenState::MediaControls;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_is_clock() {
        let model = LockScreenModel::new("alice".into());
        assert_eq!(*model.current_state(), LockScreenState::Clock);
        assert_eq!(model.password_length(), 0);
    }

    #[test]
    fn any_key_wakes_from_clock() {
        let mut model = LockScreenModel::new("alice".into());
        let action = model.on_key(LockKey::Char('x'));
        assert_eq!(*model.current_state(), LockScreenState::PasswordEntry);
        assert_eq!(action, LockAction::None);
        assert_eq!(model.password_length(), 1);
    }

    #[test]
    fn enter_from_clock_goes_to_password_entry() {
        let mut model = LockScreenModel::new("alice".into());
        model.on_key(LockKey::Enter);
        assert_eq!(*model.current_state(), LockScreenState::PasswordEntry);
        assert_eq!(model.password_length(), 0);
    }

    #[test]
    fn typing_accumulates_chars() {
        let mut model = LockScreenModel::new("alice".into());
        model.on_key(LockKey::Char('p'));
        model.on_key(LockKey::Char('a'));
        model.on_key(LockKey::Char('s'));
        model.on_key(LockKey::Char('s'));
        assert_eq!(model.password_length(), 4);
    }

    #[test]
    fn backspace_removes_char() {
        let mut model = LockScreenModel::new("alice".into());
        model.on_key(LockKey::Char('a'));
        model.on_key(LockKey::Char('b'));
        model.on_key(LockKey::Char('c'));
        model.on_key(LockKey::Backspace);
        assert_eq!(model.password_length(), 2);
    }

    #[test]
    fn backspace_on_empty_does_nothing() {
        let mut model = LockScreenModel::new("alice".into());
        model.on_key(LockKey::Char('a')); // go to PasswordEntry
        model.on_key(LockKey::Backspace); // remove 'a'
        model.on_key(LockKey::Backspace); // empty, no crash
        assert_eq!(model.password_length(), 0);
    }

    #[test]
    fn enter_submits_password() {
        let mut model = LockScreenModel::new("alice".into());
        model.on_key(LockKey::Char('s'));
        model.on_key(LockKey::Char('e'));
        model.on_key(LockKey::Char('c'));
        let action = model.on_key(LockKey::Enter);
        assert_eq!(action, LockAction::Authenticate("sec".into()));
        assert_eq!(*model.current_state(), LockScreenState::Authenticating);
    }

    #[test]
    fn enter_on_empty_password_does_nothing() {
        let mut model = LockScreenModel::new("alice".into());
        model.on_key(LockKey::Enter); // wake to PasswordEntry
        let action = model.on_key(LockKey::Enter); // empty password
        assert_eq!(action, LockAction::None);
        assert_eq!(*model.current_state(), LockScreenState::PasswordEntry);
    }

    #[test]
    fn escape_returns_to_clock() {
        let mut model = LockScreenModel::new("alice".into());
        model.on_key(LockKey::Char('x'));
        model.on_key(LockKey::Escape);
        assert_eq!(*model.current_state(), LockScreenState::Clock);
        assert_eq!(model.password_length(), 0);
    }

    #[test]
    fn auth_success_unlocks() {
        let mut model = LockScreenModel::new("alice".into());
        model.on_key(LockKey::Char('p'));
        model.on_key(LockKey::Enter);
        let action = model.on_auth_result(AuthResult::Success);
        assert_eq!(action, LockAction::Unlock);
    }

    #[test]
    fn auth_failed_shows_error() {
        let mut model = LockScreenModel::new("alice".into());
        model.on_key(LockKey::Char('p'));
        model.on_key(LockKey::Enter);
        let action = model.on_auth_result(AuthResult::Failed("wrong password".into()));
        assert_eq!(action, LockAction::None);
        assert!(matches!(
            model.current_state(),
            LockScreenState::AuthFailed(msg) if msg == "wrong password"
        ));
    }

    #[test]
    fn auth_failed_clears_password() {
        let mut model = LockScreenModel::new("alice".into());
        model.on_key(LockKey::Char('p'));
        model.on_key(LockKey::Enter);
        model.on_auth_result(AuthResult::Failed("bad".into()));
        assert_eq!(model.password_length(), 0);
    }

    #[test]
    fn auth_failed_auto_dismiss_after_timeout() {
        let mut model = LockScreenModel::new("alice".into());
        model.on_key(LockKey::Char('p'));
        model.on_key(LockKey::Enter);
        model.on_auth_result(AuthResult::Failed("bad".into()));

        // Tick less than dismiss duration — stays in AuthFailed
        model.tick(1000.0);
        assert!(matches!(
            model.current_state(),
            LockScreenState::AuthFailed(_)
        ));

        // Tick past dismiss duration — returns to PasswordEntry
        model.tick(1500.0);
        assert_eq!(*model.current_state(), LockScreenState::PasswordEntry);
    }

    #[test]
    fn auth_locked_shows_retry_timer() {
        let mut model = LockScreenModel::new("alice".into());
        model.on_key(LockKey::Char('p'));
        model.on_key(LockKey::Enter);
        let action = model.on_auth_result(AuthResult::Locked(30_000));
        assert_eq!(action, LockAction::None);
        assert!(matches!(
            model.current_state(),
            LockScreenState::AuthFailed(msg) if msg.contains("30 seconds")
        ));
    }

    #[test]
    fn auth_requires_mfa() {
        let mut model = LockScreenModel::new("alice".into());
        model.on_key(LockKey::Char('p'));
        model.on_key(LockKey::Enter);
        let action = model.on_auth_result(AuthResult::RequiresMfa);
        assert_eq!(action, LockAction::None);
        assert!(matches!(
            model.current_state(),
            LockScreenState::AuthFailed(msg) if msg.contains("Multi-factor")
        ));
    }

    #[test]
    fn keypress_from_auth_failed_goes_to_password_entry() {
        let mut model = LockScreenModel::new("alice".into());
        model.on_key(LockKey::Char('p'));
        model.on_key(LockKey::Enter);
        model.on_auth_result(AuthResult::Failed("bad".into()));

        model.on_key(LockKey::Char('n'));
        assert_eq!(*model.current_state(), LockScreenState::PasswordEntry);
        assert_eq!(model.password_length(), 1); // the 'n'
    }

    #[test]
    fn escape_from_auth_failed_goes_to_clock() {
        let mut model = LockScreenModel::new("alice".into());
        model.on_key(LockKey::Char('p'));
        model.on_key(LockKey::Enter);
        model.on_auth_result(AuthResult::Failed("bad".into()));

        model.on_key(LockKey::Escape);
        assert_eq!(*model.current_state(), LockScreenState::Clock);
    }

    #[test]
    fn tab_opens_user_switching() {
        let mut model = LockScreenModel::new("alice".into());
        model.on_key(LockKey::Char('x')); // go to PasswordEntry
        model.on_key(LockKey::Tab);
        assert_eq!(*model.current_state(), LockScreenState::Switching);
    }

    #[test]
    fn escape_from_switching_returns_to_password() {
        let mut model = LockScreenModel::new("alice".into());
        model.on_key(LockKey::Char('x'));
        model.on_key(LockKey::Tab);
        model.on_key(LockKey::Escape);
        assert_eq!(*model.current_state(), LockScreenState::PasswordEntry);
    }

    #[test]
    fn enter_in_switching_emits_switch_user() {
        let mut model = LockScreenModel::new("alice".into());
        model.on_key(LockKey::Char('x'));
        model.on_key(LockKey::Tab);
        let action = model.on_key(LockKey::Enter);
        assert_eq!(action, LockAction::SwitchUser);
    }

    #[test]
    fn keys_ignored_during_authenticating() {
        let mut model = LockScreenModel::new("alice".into());
        model.on_key(LockKey::Char('p'));
        model.on_key(LockKey::Enter);
        assert_eq!(*model.current_state(), LockScreenState::Authenticating);

        let action = model.on_key(LockKey::Char('x'));
        assert_eq!(action, LockAction::None);
        assert_eq!(*model.current_state(), LockScreenState::Authenticating);
    }

    #[test]
    fn media_play_pause() {
        let mut model = LockScreenModel::new("alice".into());
        assert!(!model.media_playing);
        let action = model.on_media_key(MediaKey::PlayPause);
        assert_eq!(action, LockAction::PlayPause);
        assert!(model.media_playing);

        let action = model.on_media_key(MediaKey::PlayPause);
        assert_eq!(action, LockAction::PlayPause);
        assert!(!model.media_playing);
    }

    #[test]
    fn media_next_prev() {
        let mut model = LockScreenModel::new("alice".into());
        assert_eq!(model.on_media_key(MediaKey::Next), LockAction::MediaNext);
        assert_eq!(
            model.on_media_key(MediaKey::Previous),
            LockAction::MediaPrev
        );
    }

    #[test]
    fn media_volume_no_action() {
        let mut model = LockScreenModel::new("alice".into());
        assert_eq!(model.on_media_key(MediaKey::VolumeUp), LockAction::None);
        assert_eq!(model.on_media_key(MediaKey::VolumeDown), LockAction::None);
    }

    #[test]
    fn media_controls_state() {
        let mut model = LockScreenModel::new("alice".into());
        model.show_media_controls();
        assert_eq!(*model.current_state(), LockScreenState::MediaControls);
        model.on_key(LockKey::Escape);
        assert_eq!(*model.current_state(), LockScreenState::Clock);
    }

    #[test]
    fn set_message() {
        let mut model = LockScreenModel::new("alice".into());
        assert!(model.message.is_none());
        model.set_message(Some("Do not disturb".into()));
        assert_eq!(model.message.as_deref(), Some("Do not disturb"));
        model.set_message(None);
        assert!(model.message.is_none());
    }

    #[test]
    fn set_media_info() {
        let mut model = LockScreenModel::new("alice".into());
        model.set_media_info(true, Some("Song Title".into()));
        assert!(model.media_playing);
        assert_eq!(model.media_title.as_deref(), Some("Song Title"));
    }

    #[test]
    fn full_unlock_flow() {
        let mut model = LockScreenModel::new("alice".into());
        // Start at clock
        assert_eq!(*model.current_state(), LockScreenState::Clock);

        // Press a key to wake
        model.on_key(LockKey::Char('m'));
        assert_eq!(*model.current_state(), LockScreenState::PasswordEntry);
        assert_eq!(model.password_length(), 1);

        // Type more password
        model.on_key(LockKey::Char('y'));
        model.on_key(LockKey::Char('p'));
        model.on_key(LockKey::Char('w'));
        assert_eq!(model.password_length(), 4);

        // Submit
        let action = model.on_key(LockKey::Enter);
        assert_eq!(action, LockAction::Authenticate("mypw".into()));
        assert_eq!(*model.current_state(), LockScreenState::Authenticating);

        // Auth success
        let action = model.on_auth_result(AuthResult::Success);
        assert_eq!(action, LockAction::Unlock);
    }

    #[test]
    fn failed_auth_recovery_flow() {
        let mut model = LockScreenModel::new("alice".into());
        model.on_key(LockKey::Char('b'));
        model.on_key(LockKey::Char('a'));
        model.on_key(LockKey::Char('d'));
        model.on_key(LockKey::Enter);

        // Auth fails
        model.on_auth_result(AuthResult::Failed("incorrect".into()));
        assert!(matches!(
            model.current_state(),
            LockScreenState::AuthFailed(_)
        ));

        // Type again — should go to PasswordEntry with new buffer
        model.on_key(LockKey::Char('g'));
        assert_eq!(*model.current_state(), LockScreenState::PasswordEntry);
        assert_eq!(model.password_length(), 1);

        // Now enter correct password
        model.on_key(LockKey::Char('o'));
        model.on_key(LockKey::Char('o'));
        model.on_key(LockKey::Char('d'));
        let action = model.on_key(LockKey::Enter);
        assert_eq!(action, LockAction::Authenticate("good".into()));
    }

    #[test]
    fn password_not_exposed() {
        let mut model = LockScreenModel::new("alice".into());
        model.on_key(LockKey::Char('s'));
        model.on_key(LockKey::Char('e'));
        model.on_key(LockKey::Char('c'));
        // We can see the length but not the actual password
        assert_eq!(model.password_length(), 3);
    }

    #[test]
    fn username_preserved() {
        let model = LockScreenModel::new("bob".into());
        assert_eq!(model.username, "bob");
    }

    #[test]
    fn clock_format_default() {
        let model = LockScreenModel::new("alice".into());
        assert!(model.clock_format_24h);
        assert!(model.show_notifications);
    }

    #[test]
    fn tick_with_no_timer_is_noop() {
        let mut model = LockScreenModel::new("alice".into());
        model.tick(1000.0); // should not crash or change state
        assert_eq!(*model.current_state(), LockScreenState::Clock);
    }
}
