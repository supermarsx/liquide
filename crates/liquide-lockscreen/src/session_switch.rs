/// User switching from the lock screen.
///
/// Provides a list of available user sessions for switching
/// without fully unlocking the current session.

/// Information about a user account available for switching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserInfo {
    /// Login username.
    pub username: String,
    /// Human-readable display name.
    pub display_name: String,
    /// Whether this user has an avatar image.
    pub has_avatar: bool,
    /// Whether this user has an active (logged-in) session.
    pub session_active: bool,
}

impl UserInfo {
    /// Create a new UserInfo.
    pub fn new(username: String, display_name: String) -> Self {
        Self {
            username,
            display_name,
            has_avatar: false,
            session_active: false,
        }
    }
}

/// Manages the list of users available for session switching.
pub struct SessionSwitcher {
    available_users: Vec<UserInfo>,
    selected_index: Option<usize>,
}

impl SessionSwitcher {
    /// Create a new empty session switcher.
    pub fn new() -> Self {
        Self {
            available_users: Vec::new(),
            selected_index: None,
        }
    }

    /// Set the list of available users.
    pub fn set_users(&mut self, users: Vec<UserInfo>) {
        self.available_users = users;
        self.selected_index = None;
    }

    /// Select a user by index. Returns the user info if valid.
    pub fn select_user(&mut self, idx: usize) -> Option<&UserInfo> {
        if idx < self.available_users.len() {
            self.selected_index = Some(idx);
            Some(&self.available_users[idx])
        } else {
            None
        }
    }

    /// Get the list of available users.
    pub fn users(&self) -> &[UserInfo] {
        &self.available_users
    }

    /// Get the currently selected user index.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Get the currently selected user.
    pub fn selected_user(&self) -> Option<&UserInfo> {
        self.selected_index
            .and_then(|idx| self.available_users.get(idx))
    }

    /// Number of available users.
    pub fn user_count(&self) -> usize {
        self.available_users.len()
    }

    /// Whether there are any users to switch to.
    pub fn has_users(&self) -> bool {
        !self.available_users.is_empty()
    }

    /// Find a user by username.
    pub fn find_user(&self, username: &str) -> Option<usize> {
        self.available_users
            .iter()
            .position(|u| u.username == username)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_users() -> Vec<UserInfo> {
        vec![
            UserInfo {
                username: "alice".into(),
                display_name: "Alice Smith".into(),
                has_avatar: true,
                session_active: true,
            },
            UserInfo {
                username: "bob".into(),
                display_name: "Bob Jones".into(),
                has_avatar: false,
                session_active: false,
            },
            UserInfo {
                username: "charlie".into(),
                display_name: "Charlie Brown".into(),
                has_avatar: true,
                session_active: true,
            },
        ]
    }

    #[test]
    fn new_is_empty() {
        let sw = SessionSwitcher::new();
        assert!(sw.users().is_empty());
        assert!(!sw.has_users());
        assert_eq!(sw.user_count(), 0);
        assert_eq!(sw.selected_index(), None);
    }

    #[test]
    fn set_users_populates_list() {
        let mut sw = SessionSwitcher::new();
        sw.set_users(sample_users());
        assert_eq!(sw.user_count(), 3);
        assert!(sw.has_users());
    }

    #[test]
    fn select_valid_user() {
        let mut sw = SessionSwitcher::new();
        sw.set_users(sample_users());
        let user = sw.select_user(1);
        assert!(user.is_some());
        assert_eq!(user.unwrap().username, "bob");
        assert_eq!(sw.selected_index(), Some(1));
    }

    #[test]
    fn select_invalid_index() {
        let mut sw = SessionSwitcher::new();
        sw.set_users(sample_users());
        let user = sw.select_user(10);
        assert!(user.is_none());
    }

    #[test]
    fn select_from_empty_list() {
        let mut sw = SessionSwitcher::new();
        assert!(sw.select_user(0).is_none());
    }

    #[test]
    fn selected_user_accessor() {
        let mut sw = SessionSwitcher::new();
        sw.set_users(sample_users());
        assert!(sw.selected_user().is_none());
        sw.select_user(2);
        let user = sw.selected_user().unwrap();
        assert_eq!(user.username, "charlie");
    }

    #[test]
    fn set_users_clears_selection() {
        let mut sw = SessionSwitcher::new();
        sw.set_users(sample_users());
        sw.select_user(0);
        assert_eq!(sw.selected_index(), Some(0));
        sw.set_users(vec![]);
        assert_eq!(sw.selected_index(), None);
    }

    #[test]
    fn find_user_by_username() {
        let mut sw = SessionSwitcher::new();
        sw.set_users(sample_users());
        assert_eq!(sw.find_user("bob"), Some(1));
        assert_eq!(sw.find_user("alice"), Some(0));
        assert_eq!(sw.find_user("charlie"), Some(2));
        assert_eq!(sw.find_user("nobody"), None);
    }

    #[test]
    fn user_info_fields() {
        let users = sample_users();
        assert!(users[0].has_avatar);
        assert!(users[0].session_active);
        assert!(!users[1].has_avatar);
        assert!(!users[1].session_active);
    }

    #[test]
    fn user_info_new() {
        let u = UserInfo::new("dave".into(), "Dave".into());
        assert_eq!(u.username, "dave");
        assert_eq!(u.display_name, "Dave");
        assert!(!u.has_avatar);
        assert!(!u.session_active);
    }

    #[test]
    fn user_info_clone_eq() {
        let u = UserInfo::new("eve".into(), "Eve".into());
        let u2 = u.clone();
        assert_eq!(u, u2);
    }

    #[test]
    fn users_slice_access() {
        let mut sw = SessionSwitcher::new();
        sw.set_users(sample_users());
        let users = sw.users();
        assert_eq!(users[0].display_name, "Alice Smith");
        assert_eq!(users[2].display_name, "Charlie Brown");
    }
}
