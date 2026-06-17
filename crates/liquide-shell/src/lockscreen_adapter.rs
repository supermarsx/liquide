//! Lock-screen integration adapter for the shell.
//!
//! Bridges the shell's session-menu **Lock** path onto the canonical
//! [`liquide_lockscreen`] state machine. Before this adapter, the shell's
//! `ShellAction::LockSession` was a visual-feedback no-op (see t49-e5-F02);
//! this wires it to the real [`LockScreenState`] / [`AuthBackend`] so locking
//! drives the canonical, security-fixed lock-screen logic (t50-e23).
//!
//! The lock-screen crate is consumed **read-only**: this module never edits
//! lockscreen internals. The shell holds the canonical [`LockScreenState`] in
//! its dormant `chrome_lockscreen` field (added by t51-e7) and a default
//! [`AuthBackend`] supplied here. The `Lock` action itself never invokes the
//! backend — it resets the screen to its fresh-lock (clock) phase — but the
//! backend is held so that subsequent password submission flows through the
//! canonical authentication path.

use liquide_lockscreen::{
    AuthBackend, AuthResult, LockScreenAction, LockScreenConfig, LockScreenEvent, LockScreenState,
};

use crate::shell::Shell;

/// Default shell authentication backend.
///
/// The simulated shell does not link a real PAM/credential provider, so the
/// canonical default is **fail-closed**: it rejects every credential. Real
/// deployments replace this with a platform backend (PAM, Windows Hello,
/// etc.). Locking does not depend on this backend — only password submission
/// does — but holding a concrete fail-closed backend keeps the auth path
/// honest rather than silently unlocking.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ShellLockAuth;

impl AuthBackend for ShellLockAuth {
    fn authenticate(&self, _username: &str, _credential: &str) -> AuthResult {
        AuthResult::Failed("Authentication backend not configured.".into())
    }
}

impl Shell {
    /// Ensure the canonical lock-screen state exists, constructing it lazily
    /// on first use, and return a mutable reference to it.
    ///
    /// Constructed from the shell's lock-screen config defaults bound to the
    /// shell's session identity. Kept lazy so the dormant `chrome_lockscreen`
    /// field (t51-e7) stays `None` until the Lock path is actually exercised,
    /// avoiding any behavior change for shells that never lock.
    fn ensure_lockscreen(&mut self) -> &mut LockScreenState {
        if self.chrome_lockscreen.is_none() {
            let config = LockScreenConfig::default();
            // The simulated shell has a single session identity; a real shell
            // would thread through the logged-in user / display name / avatar.
            let state = LockScreenState::new(config, "user".to_string(), "User".to_string(), None);
            self.chrome_lockscreen = Some(state);
        }
        self.chrome_lockscreen
            .as_mut()
            .expect("lockscreen just constructed")
    }

    /// Drive the canonical lock-screen for the session-menu **Lock** action.
    ///
    /// Sends [`LockScreenAction::Lock`] through the canonical
    /// [`LockScreenState`] with the shell's default [`AuthBackend`], so the
    /// screen transitions into its fresh-lock (clock) phase via the real
    /// security-fixed logic. Returns the events the lock screen emitted (e.g.
    /// [`LockScreenEvent::RequestBackgroundCapture`]) so the caller can drive
    /// background capture / overview clearing.
    ///
    /// This is the canonical replacement for the old no-op `LockSession`
    /// handler (t49-e5-F02).
    pub(crate) fn lock_session(&mut self) -> Vec<LockScreenEvent> {
        self.mark_wired(crate::shell::WiringBit::LockScreen);
        let auth = ShellLockAuth;
        let state = self.ensure_lockscreen();
        state.handle_action(LockScreenAction::Lock, &auth)
    }

    /// Whether the canonical lock screen is currently engaged (locked).
    ///
    /// Returns `false` when the lock screen has never been constructed or has
    /// transitioned out of the locked state.
    #[must_use]
    pub(crate) fn is_session_locked(&self) -> bool {
        self.chrome_lockscreen
            .as_ref()
            .is_some_and(LockScreenState::is_locked)
    }

    /// Absolute bounds of the lock-screen password field, read from the CSS
    /// **layout** (t95-p4 / t86 hit-test-from-CSS-geometry contract).
    ///
    /// The field is the laid-out `#lockscreen-password` (`lockscreen-prompt`)
    /// DOM box; its rect is resolved from the live hit-test engine's layout
    /// tree, NOT from a hardcoded constant. A theme change that moves/resizes
    /// the box (via the `lockscreen-prompt` rule) therefore moves this rect —
    /// the click-zone tracks the painted field. Returns `None` when the screen
    /// is not locked, the overlay has not been laid out yet, or the field has
    /// no layout box (e.g. `display: none`).
    #[must_use]
    pub(crate) fn lockscreen_password_field_bounds(
        &self,
    ) -> Option<liquide_layout::geometry::Rect> {
        if !self.is_session_locked() {
            return None;
        }
        let node = self
            .desktop_dom
            .doc
            .get_element_by_id("lockscreen-password")?;
        self.hit_test_engine
            .as_ref()?
            .bounds_for_node(node)
    }

    /// Handle a primary press on the locked screen at `(x, y)`.
    ///
    /// The lock surface is modal/topmost, so while locked EVERY press is
    /// consumed here (it must not leak to windows/chrome behind the scrim).
    /// When the press lands inside the CSS-laid-out password field box
    /// ([`Self::lockscreen_password_field_bounds`]), the canonical
    /// [`LockScreenAction::Click`] is driven, focusing the field
    /// (Clock → PasswordEntry). The focus thus follows the CSS box: a theme
    /// change that moves the field moves the click-zone with it.
    ///
    /// Returns `true` when the field was focused this press (so the caller can
    /// invalidate the scene), `false` when the press was merely swallowed.
    pub(crate) fn lockscreen_press(&mut self, x: f32, y: f32) -> bool {
        let pt = liquide_layout::geometry::Point::new(x, y);
        let in_field = self
            .lockscreen_password_field_bounds()
            .is_some_and(|r| r.contains(pt));
        if !in_field {
            return false;
        }
        // Drive the canonical click handler so the focus transition flows
        // through the real lock-screen logic (Clock → PasswordEntry).
        let auth = ShellLockAuth;
        if let Some(state) = self.chrome_lockscreen.as_mut() {
            state.handle_action(LockScreenAction::Click(x, y), &auth);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_lockscreen::screen::ScreenPhase;

    #[test]
    fn shell_lock_auth_is_fail_closed() {
        let auth = ShellLockAuth;
        assert_eq!(
            auth.authenticate("user", "whatever"),
            AuthResult::Failed("Authentication backend not configured.".into()),
        );
    }

    /// Driving the session-menu Lock action must transition the canonical
    /// `LockScreenState` (in `chrome_lockscreen`) into the locked clock phase
    /// via the real lock-screen logic — not the old no-op path.
    #[test]
    fn lock_session_drives_canonical_lockscreen() {
        let mut shell = Shell::new(1920.0, 1080.0);

        // Dormant before the Lock path is exercised.
        assert!(shell.chrome_lockscreen.is_none());
        assert!(!shell.is_session_locked());

        let events = shell.lock_session();

        // The canonical state now exists and reports locked.
        assert!(shell.chrome_lockscreen.is_some());
        assert!(shell.is_session_locked());

        let state = shell.chrome_lockscreen.as_ref().unwrap();
        assert_eq!(state.phase, ScreenPhase::Clock);
        // Fresh-lock transition: no stale password / failed attempts.
        assert_eq!(state.password_length(), 0);
        assert_eq!(state.failed_attempts, 0);

        // The Lock action drives the canonical events (capture + overview
        // clearing), proving the real lock-screen logic ran.
        assert!(
            events
                .iter()
                .any(|e| matches!(e, LockScreenEvent::RequestBackgroundCapture)),
            "Lock must request a background capture via the canonical state"
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, LockScreenEvent::ClearOverview)),
            "Lock must clear the overview via the canonical state"
        );
    }

    /// A second Lock re-enters the fresh-lock transition through the canonical
    /// state, clearing any stale lockout/failed-attempt state — exercising the
    /// t50-e23 security-fixed `enter_fresh_lock_transition` path.
    #[test]
    fn relock_clears_stale_state_via_canonical_logic() {
        let mut shell = Shell::new(1920.0, 1080.0);
        shell.lock_session();

        // Simulate stale failure state on the canonical screen.
        {
            let state = shell.chrome_lockscreen.as_mut().unwrap();
            state.failed_attempts = 3;
        }

        shell.lock_session();

        let state = shell.chrome_lockscreen.as_ref().unwrap();
        assert_eq!(
            state.failed_attempts, 0,
            "re-lock must clear stale failed attempts via canonical logic"
        );
        assert!(shell.is_session_locked());
    }
}
