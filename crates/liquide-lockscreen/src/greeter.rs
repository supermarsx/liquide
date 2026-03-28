/// Login greeter UI model.
///
/// Models the visual greeter shown at login / session switch:
/// user list, provider selection buttons, credential input fields,
/// clock display, and power actions.  Inspired by GDM/SDDM greeter
/// patterns.

use crate::provider::ProviderRegistry;

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// A user entry shown in the greeter's user list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserEntry {
    /// Login username.
    pub username: String,
    /// Human-readable display name.
    pub display_name: String,
    /// Path to the user's avatar image, if any.
    pub avatar_path: Option<String>,
    /// Whether the user has logged in before (affects sort order).
    pub has_logged_in_before: bool,
    /// Session type the user last used (e.g. `"wayland"`, `"x11"`).
    pub session_type: String,
}

impl UserEntry {
    pub fn new(username: &str, display_name: &str) -> Self {
        Self {
            username: username.to_string(),
            display_name: display_name.to_string(),
            avatar_path: None,
            has_logged_in_before: false,
            session_type: "wayland".to_string(),
        }
    }

    /// Builder: set avatar path.
    pub fn with_avatar(mut self, path: &str) -> Self {
        self.avatar_path = Some(path.to_string());
        self
    }

    /// Builder: mark as having logged in before.
    pub fn with_previous_login(mut self) -> Self {
        self.has_logged_in_before = true;
        self
    }

    /// Builder: set session type.
    pub fn with_session_type(mut self, session_type: &str) -> Self {
        self.session_type = session_type.to_string();
        self
    }
}

/// Power actions available from the greeter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerAction {
    Shutdown,
    Reboot,
    Suspend,
    Hibernate,
}

/// Events emitted by the greeter when the user interacts with it.
#[derive(Debug, Clone, PartialEq)]
pub enum GreeterEvent {
    /// A user was selected from the list.
    UserSelected(String),
    /// A credential provider was selected.
    ProviderSelected(String),
    /// A credential field value changed.
    FieldChanged { field_id: String, value: String },
    /// The submit / login button was clicked.
    SubmitClicked,
    /// The "switch user" / back button was clicked.
    SwitchUser,
    /// A power action was requested.
    PowerAction(PowerAction),
}

// ---------------------------------------------------------------------------
// GreeterLayout
// ---------------------------------------------------------------------------

/// Computed layout positions for greeter elements.
///
/// All coordinates are in logical pixels relative to the greeter surface.
#[derive(Debug, Clone)]
pub struct GreeterLayout {
    pub screen_width: f32,
    pub screen_height: f32,
    /// Bounding box for the user list area.
    pub user_list_x: f32,
    pub user_list_y: f32,
    pub user_list_width: f32,
    pub user_list_height: f32,
    /// Bounding box for the provider buttons.
    pub provider_area_x: f32,
    pub provider_area_y: f32,
    pub provider_area_width: f32,
    pub provider_area_height: f32,
    /// Bounding box for the input fields area.
    pub input_area_x: f32,
    pub input_area_y: f32,
    pub input_area_width: f32,
    pub input_area_height: f32,
    /// Clock position.
    pub clock_x: f32,
    pub clock_y: f32,
    /// Power buttons position.
    pub power_area_x: f32,
    pub power_area_y: f32,
    pub power_area_width: f32,
    pub power_area_height: f32,
}

impl GreeterLayout {
    /// Compute layout for the given screen dimensions, user count, and
    /// provider count.
    pub fn compute(
        screen_width: f32,
        screen_height: f32,
        user_count: usize,
        provider_count: usize,
        field_count: usize,
    ) -> Self {
        let center_x = screen_width / 2.0;
        let center_y = screen_height / 2.0;

        // User list: centered, 320px wide, 64px per user (max 5 visible)
        let user_item_height = 64.0;
        let user_list_width = 320.0;
        let visible_users = user_count.min(5) as f32;
        let user_list_height = visible_users * user_item_height;
        let user_list_x = center_x - user_list_width / 2.0;
        let user_list_y = center_y - user_list_height / 2.0 - 80.0;

        // Provider buttons: below user list, 280px wide, 48px per button
        let provider_btn_height = 48.0;
        let provider_area_width = 280.0;
        let provider_area_height = provider_count.max(1) as f32 * provider_btn_height;
        let provider_area_x = center_x - provider_area_width / 2.0;
        let provider_area_y = user_list_y + user_list_height + 24.0;

        // Input fields: below providers, 280px wide, 48px per field + 48 for button
        let field_height = 48.0;
        let input_area_width = 280.0;
        let input_area_height = (field_count.max(1) as f32 + 1.0) * field_height;
        let input_area_x = center_x - input_area_width / 2.0;
        let input_area_y = provider_area_y + provider_area_height + 16.0;

        // Clock: top center
        let clock_x = center_x;
        let clock_y = 60.0;

        // Power buttons: bottom right, 4 buttons x 48px
        let power_area_width = 4.0 * 48.0;
        let power_area_height = 48.0;
        let power_area_x = screen_width - power_area_width - 24.0;
        let power_area_y = screen_height - power_area_height - 24.0;

        Self {
            screen_width,
            screen_height,
            user_list_x,
            user_list_y,
            user_list_width,
            user_list_height,
            provider_area_x,
            provider_area_y,
            provider_area_width,
            provider_area_height,
            input_area_x,
            input_area_y,
            input_area_width,
            input_area_height,
            clock_x,
            clock_y,
            power_area_x,
            power_area_y,
            power_area_width,
            power_area_height,
        }
    }

    /// Test whether a point is inside the user list area.
    pub fn hit_user_list(&self, x: f32, y: f32) -> bool {
        x >= self.user_list_x
            && x <= self.user_list_x + self.user_list_width
            && y >= self.user_list_y
            && y <= self.user_list_y + self.user_list_height
    }

    /// Test whether a point is inside the power button area.
    pub fn hit_power_area(&self, x: f32, y: f32) -> bool {
        x >= self.power_area_x
            && x <= self.power_area_x + self.power_area_width
            && y >= self.power_area_y
            && y <= self.power_area_y + self.power_area_height
    }
}

// ---------------------------------------------------------------------------
// GreeterModel
// ---------------------------------------------------------------------------

/// The greeter's full UI model.
pub struct GreeterModel {
    /// List of users available for login.
    pub users: Vec<UserEntry>,
    /// Index of the currently selected user, if any.
    pub selected_user_index: Option<usize>,
    /// Clock text (e.g. `"14:35"`).
    pub clock_text: String,
    /// Date text (e.g. `"Monday, March 8"`).
    pub date_text: String,
    /// Background image path, if any.
    pub background_path: Option<String>,
    /// Message displayed on the greeter (e.g. hostname, MOTD).
    pub message: Option<String>,
    /// Whether power actions are shown.
    pub show_power_actions: bool,
    /// Pending events to deliver to the session controller.
    pending_events: Vec<GreeterEvent>,
}

impl GreeterModel {
    /// Create a new greeter with the given user list.
    pub fn new(users: Vec<UserEntry>) -> Self {
        Self {
            users,
            selected_user_index: None,
            clock_text: String::new(),
            date_text: String::new(),
            background_path: None,
            message: None,
            show_power_actions: true,
            pending_events: Vec::new(),
        }
    }

    /// Set the clock/date text (called by the shell on each tick).
    pub fn update_clock(&mut self, clock: &str, date: &str) {
        self.clock_text = clock.to_string();
        self.date_text = date.to_string();
    }

    /// Select a user by index.  Emits `GreeterEvent::UserSelected`.
    pub fn select_user(&mut self, index: usize) -> bool {
        if index >= self.users.len() {
            return false;
        }
        self.selected_user_index = Some(index);
        let username = self.users[index].username.clone();
        self.pending_events
            .push(GreeterEvent::UserSelected(username));
        true
    }

    /// Select a user by username.
    pub fn select_user_by_name(&mut self, username: &str) -> bool {
        if let Some(idx) = self.users.iter().position(|u| u.username == username) {
            self.select_user(idx)
        } else {
            false
        }
    }

    /// Get the currently selected user entry.
    pub fn selected_user(&self) -> Option<&UserEntry> {
        self.selected_user_index
            .and_then(|i| self.users.get(i))
    }

    /// Handle provider selection.  Emits `GreeterEvent::ProviderSelected`.
    pub fn select_provider(&mut self, provider_id: &str) {
        self.pending_events
            .push(GreeterEvent::ProviderSelected(provider_id.to_string()));
    }

    /// Handle a credential field change.
    pub fn field_changed(&mut self, field_id: &str, value: &str) {
        self.pending_events.push(GreeterEvent::FieldChanged {
            field_id: field_id.to_string(),
            value: value.to_string(),
        });
    }

    /// Handle submit button click.
    pub fn submit(&mut self) {
        self.pending_events.push(GreeterEvent::SubmitClicked);
    }

    /// Handle switch-user / back button.
    pub fn switch_user(&mut self) {
        self.selected_user_index = None;
        self.pending_events.push(GreeterEvent::SwitchUser);
    }

    /// Handle a power action request.
    pub fn power_action(&mut self, action: PowerAction) {
        self.pending_events
            .push(GreeterEvent::PowerAction(action));
    }

    /// Drain pending events.
    pub fn take_events(&mut self) -> Vec<GreeterEvent> {
        std::mem::take(&mut self.pending_events)
    }

    /// Compute the layout for the current state.
    pub fn compute_layout(
        &self,
        screen_width: f32,
        screen_height: f32,
        registry: &ProviderRegistry,
    ) -> GreeterLayout {
        let field_count = if let Some(provider_id) = self
            .selected_user_index
            .and_then(|_| Some("password")) // default provider for layout estimation
        {
            registry
                .get(provider_id)
                .map(|p| p.field_descriptors().len())
                .unwrap_or(2)
        } else {
            0
        };

        GreeterLayout::compute(
            screen_width,
            screen_height,
            self.users.len(),
            registry.len(),
            field_count,
        )
    }

    /// Users sorted for display: previously-logged-in users first.
    pub fn sorted_users(&self) -> Vec<&UserEntry> {
        let mut sorted: Vec<&UserEntry> = self.users.iter().collect();
        sorted.sort_by(|a, b| {
            b.has_logged_in_before
                .cmp(&a.has_logged_in_before)
                .then_with(|| a.username.cmp(&b.username))
        });
        sorted
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_users() -> Vec<UserEntry> {
        vec![
            UserEntry::new("alice", "Alice Smith")
                .with_avatar("/var/lib/AccountsService/icons/alice")
                .with_previous_login(),
            UserEntry::new("bob", "Bob Jones"),
            UserEntry::new("charlie", "Charlie Brown")
                .with_session_type("x11")
                .with_previous_login(),
        ]
    }

    #[test]
    fn user_entry_new() {
        let u = UserEntry::new("dave", "Dave");
        assert_eq!(u.username, "dave");
        assert_eq!(u.display_name, "Dave");
        assert!(u.avatar_path.is_none());
        assert!(!u.has_logged_in_before);
        assert_eq!(u.session_type, "wayland");
    }

    #[test]
    fn user_entry_builders() {
        let u = UserEntry::new("eve", "Eve")
            .with_avatar("/tmp/eve.png")
            .with_previous_login()
            .with_session_type("x11");
        assert_eq!(u.avatar_path.as_deref(), Some("/tmp/eve.png"));
        assert!(u.has_logged_in_before);
        assert_eq!(u.session_type, "x11");
    }

    #[test]
    fn greeter_model_initial_state() {
        let model = GreeterModel::new(sample_users());
        assert_eq!(model.users.len(), 3);
        assert!(model.selected_user_index.is_none());
        assert!(model.clock_text.is_empty());
        assert!(model.show_power_actions);
    }

    #[test]
    fn greeter_select_user() {
        let mut model = GreeterModel::new(sample_users());
        assert!(model.select_user(1));
        assert_eq!(model.selected_user_index, Some(1));
        let events = model.take_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], GreeterEvent::UserSelected("bob".into()));
    }

    #[test]
    fn greeter_select_user_invalid_index() {
        let mut model = GreeterModel::new(sample_users());
        assert!(!model.select_user(10));
        assert!(model.selected_user_index.is_none());
        assert!(model.take_events().is_empty());
    }

    #[test]
    fn greeter_select_user_by_name() {
        let mut model = GreeterModel::new(sample_users());
        assert!(model.select_user_by_name("charlie"));
        assert_eq!(model.selected_user_index, Some(2));
        assert!(!model.select_user_by_name("nobody"));
    }

    #[test]
    fn greeter_selected_user_accessor() {
        let mut model = GreeterModel::new(sample_users());
        assert!(model.selected_user().is_none());
        model.select_user(0);
        let user = model.selected_user().unwrap();
        assert_eq!(user.username, "alice");
    }

    #[test]
    fn greeter_update_clock() {
        let mut model = GreeterModel::new(vec![]);
        model.update_clock("14:35", "Monday, March 8");
        assert_eq!(model.clock_text, "14:35");
        assert_eq!(model.date_text, "Monday, March 8");
    }

    #[test]
    fn greeter_provider_selected_event() {
        let mut model = GreeterModel::new(vec![]);
        model.select_provider("pin");
        let events = model.take_events();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            GreeterEvent::ProviderSelected("pin".into())
        );
    }

    #[test]
    fn greeter_field_changed_event() {
        let mut model = GreeterModel::new(vec![]);
        model.field_changed("password", "sec");
        let events = model.take_events();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            GreeterEvent::FieldChanged {
                field_id: "password".into(),
                value: "sec".into()
            }
        );
    }

    #[test]
    fn greeter_submit_event() {
        let mut model = GreeterModel::new(vec![]);
        model.submit();
        let events = model.take_events();
        assert_eq!(events[0], GreeterEvent::SubmitClicked);
    }

    #[test]
    fn greeter_switch_user_clears_selection() {
        let mut model = GreeterModel::new(sample_users());
        model.select_user(1);
        model.take_events(); // drain
        model.switch_user();
        assert!(model.selected_user_index.is_none());
        let events = model.take_events();
        assert_eq!(events[0], GreeterEvent::SwitchUser);
    }

    #[test]
    fn greeter_power_action() {
        let mut model = GreeterModel::new(vec![]);
        model.power_action(PowerAction::Shutdown);
        model.power_action(PowerAction::Reboot);
        let events = model.take_events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], GreeterEvent::PowerAction(PowerAction::Shutdown));
        assert_eq!(events[1], GreeterEvent::PowerAction(PowerAction::Reboot));
    }

    #[test]
    fn greeter_take_events_drains() {
        let mut model = GreeterModel::new(vec![]);
        model.submit();
        assert_eq!(model.take_events().len(), 1);
        assert!(model.take_events().is_empty());
    }

    #[test]
    fn greeter_sorted_users_previous_first() {
        let model = GreeterModel::new(sample_users());
        let sorted = model.sorted_users();
        // alice and charlie have logged in before
        assert!(sorted[0].has_logged_in_before);
        assert!(sorted[1].has_logged_in_before);
        assert!(!sorted[2].has_logged_in_before);
        assert_eq!(sorted[2].username, "bob");
    }

    #[test]
    fn greeter_sorted_users_alpha_within_group() {
        let model = GreeterModel::new(sample_users());
        let sorted = model.sorted_users();
        // Within logged-in group: alice < charlie
        assert_eq!(sorted[0].username, "alice");
        assert_eq!(sorted[1].username, "charlie");
    }

    // -- GreeterLayout tests --

    #[test]
    fn layout_compute_basic() {
        let layout = GreeterLayout::compute(1920.0, 1080.0, 3, 2, 2);
        assert_eq!(layout.screen_width, 1920.0);
        assert_eq!(layout.screen_height, 1080.0);
        assert!(layout.user_list_width > 0.0);
        assert!(layout.provider_area_height > 0.0);
        assert!(layout.input_area_height > 0.0);
    }

    #[test]
    fn layout_user_list_centered() {
        let layout = GreeterLayout::compute(1920.0, 1080.0, 2, 1, 1);
        let center_x = 1920.0 / 2.0;
        let list_center = layout.user_list_x + layout.user_list_width / 2.0;
        assert!((list_center - center_x).abs() < 1.0);
    }

    #[test]
    fn layout_user_list_max_five() {
        let layout = GreeterLayout::compute(1920.0, 1080.0, 10, 1, 1);
        // 5 * 64 = 320 max
        assert!((layout.user_list_height - 320.0).abs() < 1.0);
    }

    #[test]
    fn layout_clock_at_top() {
        let layout = GreeterLayout::compute(1920.0, 1080.0, 1, 1, 1);
        assert_eq!(layout.clock_y, 60.0);
        assert!((layout.clock_x - 960.0).abs() < 1.0);
    }

    #[test]
    fn layout_power_at_bottom_right() {
        let layout = GreeterLayout::compute(1920.0, 1080.0, 1, 1, 1);
        assert!(layout.power_area_x > 1920.0 / 2.0);
        assert!(layout.power_area_y > 1080.0 / 2.0);
    }

    #[test]
    fn layout_hit_user_list() {
        let layout = GreeterLayout::compute(1920.0, 1080.0, 3, 1, 1);
        let mid_x = layout.user_list_x + layout.user_list_width / 2.0;
        let mid_y = layout.user_list_y + layout.user_list_height / 2.0;
        assert!(layout.hit_user_list(mid_x, mid_y));
        assert!(!layout.hit_user_list(0.0, 0.0));
    }

    #[test]
    fn layout_hit_power_area() {
        let layout = GreeterLayout::compute(1920.0, 1080.0, 1, 1, 1);
        let mid_x = layout.power_area_x + layout.power_area_width / 2.0;
        let mid_y = layout.power_area_y + layout.power_area_height / 2.0;
        assert!(layout.hit_power_area(mid_x, mid_y));
        assert!(!layout.hit_power_area(0.0, 0.0));
    }

    #[test]
    fn power_action_variants() {
        assert_ne!(PowerAction::Shutdown, PowerAction::Reboot);
        assert_ne!(PowerAction::Suspend, PowerAction::Hibernate);
        assert_eq!(PowerAction::Shutdown, PowerAction::Shutdown);
    }

    #[test]
    fn greeter_message() {
        let mut model = GreeterModel::new(vec![]);
        assert!(model.message.is_none());
        model.message = Some("Welcome to LiquiDE".into());
        assert_eq!(model.message.as_deref(), Some("Welcome to LiquiDE"));
    }

    #[test]
    fn greeter_background() {
        let mut model = GreeterModel::new(vec![]);
        model.background_path = Some("/usr/share/backgrounds/default.jpg".into());
        assert!(model.background_path.is_some());
    }
}
