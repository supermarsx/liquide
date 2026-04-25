//! Desktop portal interfaces (abstract).
//!
//! Defines trait-based abstractions for desktop portals following the
//! freedesktop.org Desktop Portal specification concepts. Portals provide
//! a sandboxed application a way to request privileged operations through
//! a well-defined request/response protocol.

use std::collections::HashMap;
use std::path::PathBuf;

/// Unique handle for a portal request.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RequestHandle(pub String);

/// Response status returned by portal operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResponseStatus {
    /// The user accepted the dialog / the operation succeeded.
    Success,
    /// The user cancelled the dialog.
    Cancelled,
    /// Something else went wrong.
    Other(u32),
}

/// Options for a file chooser portal request.
#[derive(Clone, Debug, Default)]
pub struct FileChooserOptions {
    /// Dialog title.
    pub title: Option<String>,
    /// Whether the dialog allows selecting multiple files.
    pub multiple: bool,
    /// Whether the dialog should allow selecting directories.
    pub directory: bool,
    /// Allowed MIME type filters (e.g. `["image/png", "image/jpeg"]`).
    pub accept_mime_types: Vec<String>,
    /// Suggested filename for save dialogs.
    pub current_name: Option<String>,
    /// Current directory to open the dialog in.
    pub current_folder: Option<PathBuf>,
}

/// Result of a file chooser dialog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileChooserResult {
    /// The selected file URIs.
    pub uris: Vec<String>,
    /// Whether the user accepted or cancelled.
    pub status: ResponseStatus,
}

/// Options for a screenshot portal request.
#[derive(Clone, Debug, Default)]
pub struct ScreenshotOptions {
    /// Whether to include the mouse cursor.
    pub include_cursor: bool,
    /// Whether to show an interactive region selector.
    pub interactive: bool,
}

/// Result of a screenshot capture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScreenshotResult {
    /// URI to the captured screenshot image.
    pub uri: Option<String>,
    /// Response status.
    pub status: ResponseStatus,
}

/// Priority level for portal notifications.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum NotificationPriority {
    Low,
    #[default]
    Normal,
    High,
    Urgent,
}

/// A notification to be displayed through the notification portal.
#[derive(Clone, Debug, Default)]
pub struct NotificationRequest {
    /// Unique notification ID (for replacement / withdrawal).
    pub id: String,
    /// Notification title.
    pub title: String,
    /// Notification body text.
    pub body: Option<String>,
    /// Icon name or path.
    pub icon: Option<String>,
    /// Priority level.
    pub priority: NotificationPriority,
    /// Action identifiers the user can click.
    pub actions: Vec<NotificationAction>,
}

/// An action button on a notification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotificationAction {
    /// Unique identifier for this action.
    pub id: String,
    /// Label displayed on the button.
    pub label: String,
}

/// Response to a notification interaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotificationResponse {
    /// The ID of the action the user clicked, or `None` for body click.
    pub action_id: Option<String>,
}

/// User account information returned by the account portal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountInfo {
    /// User display name.
    pub name: String,
    /// User avatar image URI.
    pub image: Option<String>,
    /// A unique user ID string.
    pub id: Option<String>,
}

/// A portal request that can be sent by sandboxed applications.
#[derive(Clone, Debug)]
pub enum PortalRequest {
    /// Open file dialog.
    OpenFile(FileChooserOptions),
    /// Save file dialog.
    SaveFile(FileChooserOptions),
    /// Capture a screenshot.
    Screenshot(ScreenshotOptions),
    /// Display a notification.
    Notification(NotificationRequest),
    /// Withdraw (close) a notification.
    WithdrawNotification(String),
    /// Query user account info.
    AccountInfo,
}

/// A portal response corresponding to a request.
#[derive(Clone, Debug)]
pub enum PortalResponse {
    /// Result of a file chooser dialog.
    FileChooser(FileChooserResult),
    /// Result of a screenshot operation.
    Screenshot(ScreenshotResult),
    /// Notification was displayed (or failed).
    NotificationSent(ResponseStatus),
    /// Notification was withdrawn.
    NotificationWithdrawn,
    /// User account info.
    Account(AccountInfo),
    /// The portal is not available or the request was rejected.
    Unsupported,
}

/// Trait for desktop portal implementations.
///
/// Concrete implementations may use D-Bus, direct syscalls, or mock
/// backends depending on the platform and sandboxing context.
pub trait Portal {
    /// Submit a portal request and return a handle for tracking.
    fn request(&mut self, req: PortalRequest) -> RequestHandle;

    /// Poll for a response to a previously submitted request.
    ///
    /// Returns `None` if the response is not yet available.
    fn poll_response(&mut self, handle: &RequestHandle) -> Option<PortalResponse>;

    /// Check whether a specific portal interface is available.
    fn is_available(&self, portal_name: &str) -> bool;
}

/// File chooser portal convenience trait.
pub trait FileChooserPortal {
    /// Open a file chooser dialog for opening files.
    fn open_file(&mut self, options: FileChooserOptions) -> RequestHandle;
    /// Open a file chooser dialog for saving a file.
    fn save_file(&mut self, options: FileChooserOptions) -> RequestHandle;
}

/// Screenshot portal convenience trait.
pub trait ScreenshotPortal {
    /// Capture a screenshot.
    fn capture(&mut self, options: ScreenshotOptions) -> RequestHandle;
}

/// Notification portal convenience trait.
pub trait NotificationPortal {
    /// Send a notification.
    fn notify(&mut self, request: NotificationRequest) -> RequestHandle;
    /// Withdraw a notification by ID.
    fn withdraw(&mut self, notification_id: &str) -> RequestHandle;
}

/// Account portal convenience trait.
pub trait AccountPortal {
    /// Query user account information.
    fn get_user_info(&mut self) -> RequestHandle;
}

/// A mock portal implementation for testing.
#[derive(Debug, Default)]
pub struct MockPortal {
    next_id: u64,
    responses: HashMap<String, PortalResponse>,
    available: Vec<String>,
}

impl MockPortal {
    /// Create a new mock portal with no pre-configured responses.
    pub fn new() -> Self {
        Self::default()
    }

    /// Pre-configure a response that will be returned for the next request.
    pub fn enqueue_response(&mut self, handle: &RequestHandle, response: PortalResponse) {
        self.responses.insert(handle.0.clone(), response);
    }

    /// Mark a portal interface as available.
    pub fn set_available(&mut self, name: &str) {
        self.available.push(name.to_string());
    }

    fn next_handle(&mut self) -> RequestHandle {
        self.next_id += 1;
        RequestHandle(format!("/mock/request/{}", self.next_id))
    }
}

impl Portal for MockPortal {
    fn request(&mut self, _req: PortalRequest) -> RequestHandle {
        self.next_handle()
    }

    fn poll_response(&mut self, handle: &RequestHandle) -> Option<PortalResponse> {
        self.responses.remove(&handle.0)
    }

    fn is_available(&self, portal_name: &str) -> bool {
        self.available.iter().any(|n| n == portal_name)
    }
}

impl FileChooserPortal for MockPortal {
    fn open_file(&mut self, options: FileChooserOptions) -> RequestHandle {
        self.request(PortalRequest::OpenFile(options))
    }

    fn save_file(&mut self, options: FileChooserOptions) -> RequestHandle {
        self.request(PortalRequest::SaveFile(options))
    }
}

impl ScreenshotPortal for MockPortal {
    fn capture(&mut self, options: ScreenshotOptions) -> RequestHandle {
        self.request(PortalRequest::Screenshot(options))
    }
}

impl NotificationPortal for MockPortal {
    fn notify(&mut self, request: NotificationRequest) -> RequestHandle {
        self.request(PortalRequest::Notification(request))
    }

    fn withdraw(&mut self, notification_id: &str) -> RequestHandle {
        self.request(PortalRequest::WithdrawNotification(
            notification_id.to_string(),
        ))
    }
}

impl AccountPortal for MockPortal {
    fn get_user_info(&mut self) -> RequestHandle {
        self.request(PortalRequest::AccountInfo)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_handle_equality() {
        let a = RequestHandle("/a/1".into());
        let b = RequestHandle("/a/1".into());
        assert_eq!(a, b);
    }

    #[test]
    fn response_status_variants() {
        assert_eq!(ResponseStatus::Success, ResponseStatus::Success);
        assert_eq!(ResponseStatus::Cancelled, ResponseStatus::Cancelled);
        assert_eq!(ResponseStatus::Other(42), ResponseStatus::Other(42));
        assert_ne!(ResponseStatus::Success, ResponseStatus::Cancelled);
    }

    #[test]
    fn file_chooser_options_defaults() {
        let opts = FileChooserOptions::default();
        assert!(!opts.multiple);
        assert!(!opts.directory);
        assert!(opts.accept_mime_types.is_empty());
        assert!(opts.title.is_none());
        assert!(opts.current_name.is_none());
    }

    #[test]
    fn notification_priority_default() {
        assert_eq!(
            NotificationPriority::default(),
            NotificationPriority::Normal
        );
    }

    #[test]
    fn mock_portal_generates_unique_handles() {
        let mut portal = MockPortal::new();
        let h1 = portal.request(PortalRequest::AccountInfo);
        let h2 = portal.request(PortalRequest::AccountInfo);
        assert_ne!(h1, h2);
    }

    #[test]
    fn mock_portal_enqueue_and_poll() {
        let mut portal = MockPortal::new();
        let handle = RequestHandle("/mock/request/1".into());
        portal.enqueue_response(
            &handle,
            PortalResponse::Account(AccountInfo {
                name: "Test User".into(),
                image: None,
                id: Some("1000".into()),
            }),
        );
        let resp = portal.poll_response(&handle).unwrap();
        match resp {
            PortalResponse::Account(info) => {
                assert_eq!(info.name, "Test User");
                assert_eq!(info.id.as_deref(), Some("1000"));
            }
            _ => panic!("expected Account response"),
        }
    }

    #[test]
    fn mock_portal_poll_returns_none_when_empty() {
        let mut portal = MockPortal::new();
        let handle = RequestHandle("nonexistent".into());
        assert!(portal.poll_response(&handle).is_none());
    }

    #[test]
    fn mock_portal_is_available() {
        let mut portal = MockPortal::new();
        assert!(!portal.is_available("filechooser"));
        portal.set_available("filechooser");
        assert!(portal.is_available("filechooser"));
    }

    #[test]
    fn file_chooser_portal_trait() {
        let mut portal = MockPortal::new();
        let handle = FileChooserPortal::open_file(
            &mut portal,
            FileChooserOptions {
                title: Some("Open".into()),
                multiple: true,
                ..Default::default()
            },
        );
        // Handle was generated.
        assert!(!handle.0.is_empty());
    }

    #[test]
    fn screenshot_portal_trait() {
        let mut portal = MockPortal::new();
        let handle = ScreenshotPortal::capture(
            &mut portal,
            ScreenshotOptions {
                include_cursor: true,
                interactive: false,
            },
        );
        assert!(!handle.0.is_empty());
    }

    #[test]
    fn notification_portal_trait() {
        let mut portal = MockPortal::new();
        let handle = NotificationPortal::notify(
            &mut portal,
            NotificationRequest {
                id: "n1".into(),
                title: "Hello".into(),
                body: Some("World".into()),
                priority: NotificationPriority::High,
                ..Default::default()
            },
        );
        assert!(!handle.0.is_empty());
    }

    #[test]
    fn notification_withdraw() {
        let mut portal = MockPortal::new();
        let handle = NotificationPortal::withdraw(&mut portal, "n1");
        assert!(!handle.0.is_empty());
    }

    #[test]
    fn account_portal_trait() {
        let mut portal = MockPortal::new();
        let handle = AccountPortal::get_user_info(&mut portal);
        assert!(!handle.0.is_empty());
    }

    #[test]
    fn file_chooser_result_equality() {
        let a = FileChooserResult {
            uris: vec!["file:///a.txt".into()],
            status: ResponseStatus::Success,
        };
        let b = FileChooserResult {
            uris: vec!["file:///a.txt".into()],
            status: ResponseStatus::Success,
        };
        assert_eq!(a, b);
    }

    #[test]
    fn screenshot_result_cancelled() {
        let r = ScreenshotResult {
            uri: None,
            status: ResponseStatus::Cancelled,
        };
        assert_eq!(r.status, ResponseStatus::Cancelled);
        assert!(r.uri.is_none());
    }

    #[test]
    fn notification_action_equality() {
        let a = NotificationAction {
            id: "reply".into(),
            label: "Reply".into(),
        };
        let b = NotificationAction {
            id: "reply".into(),
            label: "Reply".into(),
        };
        assert_eq!(a, b);
    }

    #[test]
    fn notification_response_with_action() {
        let r = NotificationResponse {
            action_id: Some("reply".into()),
        };
        assert_eq!(r.action_id.as_deref(), Some("reply"));
    }

    #[test]
    fn notification_response_body_click() {
        let r = NotificationResponse { action_id: None };
        assert!(r.action_id.is_none());
    }

    #[test]
    fn account_info_fields() {
        let info = AccountInfo {
            name: "Alice".into(),
            image: Some("file:///avatar.png".into()),
            id: Some("alice".into()),
        };
        assert_eq!(info.name, "Alice");
        assert_eq!(info.image.as_deref(), Some("file:///avatar.png"));
    }

    #[test]
    fn save_file_portal_trait() {
        let mut portal = MockPortal::new();
        let handle = FileChooserPortal::save_file(
            &mut portal,
            FileChooserOptions {
                current_name: Some("document.pdf".into()),
                ..Default::default()
            },
        );
        assert!(!handle.0.is_empty());
    }
}
