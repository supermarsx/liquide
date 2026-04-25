//! Autostart management for the LiquiDE desktop environment.
//!
//! This crate provides:
//! - [`StartupEntry`] — an autostart application descriptor
//! - [`AutostartManager`] — add, remove, enable/disable, and query startup entries
//! - [`desktop_file`] — parse and write freedesktop `.desktop` files
//! - [`platform`] — discover autostart entries from OS-specific locations
//! - [`StartupTimer`] — track per-app startup timing for the session

pub mod desktop_file;
pub mod entry;
pub mod error;
pub mod manager;
pub mod platform;
pub mod timer;

pub use entry::{EntrySource, StartupEntry};
pub use error::{AutostartError, ParseError};
pub use manager::AutostartManager;
pub use timer::{AppReportEntry, AppTiming, StartupReport, StartupTimer};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_new_defaults() {
        let e = StartupEntry::new("test-app", "Test App", "/usr/bin/test-app");
        assert_eq!(e.id, "test-app");
        assert_eq!(e.name, "Test App");
        assert_eq!(e.command, "/usr/bin/test-app");
        assert!(e.enabled);
        assert_eq!(e.delay_seconds, 0);
        assert_eq!(e.source, EntrySource::User);
        assert!(e.comment.is_none());
        assert!(e.icon.is_none());
        assert!(e.only_show_in.is_empty());
        assert!(e.not_show_in.is_empty());
    }

    #[test]
    fn entry_builder_chain() {
        let e = StartupEntry::new("slack", "Slack", "/usr/bin/slack")
            .with_comment("Messaging app")
            .with_icon("slack")
            .with_delay(5)
            .with_source(EntrySource::System)
            .with_enabled(false)
            .with_only_show_in(vec!["GNOME".into()])
            .with_not_show_in(vec!["KDE".into()]);

        assert_eq!(e.comment.as_deref(), Some("Messaging app"));
        assert_eq!(e.icon.as_deref(), Some("slack"));
        assert_eq!(e.delay_seconds, 5);
        assert_eq!(e.source, EntrySource::System);
        assert!(!e.enabled);
        assert_eq!(e.only_show_in, vec!["GNOME"]);
        assert_eq!(e.not_show_in, vec!["KDE"]);
    }

    #[test]
    fn entry_should_show_in_only_show() {
        let e = StartupEntry::new("a", "A", "/bin/a")
            .with_only_show_in(vec!["GNOME".into(), "KDE".into()]);
        assert!(e.should_show_in("GNOME"));
        assert!(e.should_show_in("KDE"));
        assert!(!e.should_show_in("XFCE"));
    }

    #[test]
    fn entry_should_show_in_not_show() {
        let e = StartupEntry::new("a", "A", "/bin/a").with_not_show_in(vec!["XFCE".into()]);
        assert!(e.should_show_in("GNOME"));
        assert!(!e.should_show_in("XFCE"));
    }

    #[test]
    fn entry_should_show_in_universal() {
        let e = StartupEntry::new("a", "A", "/bin/a");
        assert!(e.should_show_in("anything"));
    }

    #[test]
    fn entry_estimated_startup_ms() {
        let e = StartupEntry::new("a", "A", "/bin/a").with_delay(3);
        assert_eq!(e.estimated_startup_ms(), 3500);

        let e2 = StartupEntry::new("b", "B", "/bin/b");
        assert_eq!(e2.estimated_startup_ms(), 500);
    }

    #[test]
    fn entry_source_display() {
        assert_eq!(format!("{}", EntrySource::System), "system");
        assert_eq!(format!("{}", EntrySource::User), "user");
        assert_eq!(format!("{}", EntrySource::Session), "session");
    }

    #[test]
    fn error_display() {
        let e = AutostartError::NotFound("foo".into());
        assert!(format!("{e}").contains("foo"));

        let e2 = AutostartError::SystemEntryCannotBeRemoved("bar".into());
        assert!(format!("{e2}").contains("bar"));
        assert!(format!("{e2}").contains("disable"));

        let e3 = AutostartError::DuplicateEntry("baz".into());
        assert!(format!("{e3}").contains("baz"));

        let e4 = AutostartError::InvalidCommand("empty".into());
        assert!(format!("{e4}").contains("empty"));

        let e5 = AutostartError::InvalidId;
        assert!(format!("{e5}").contains("empty"));
    }

    #[test]
    fn parse_error_display() {
        let e = ParseError::MissingDesktopEntrySection;
        assert!(format!("{e}").contains("[Desktop Entry]"));

        let e2 = ParseError::MissingKey("Name".into());
        assert!(format!("{e2}").contains("Name"));

        let e3 = ParseError::InvalidValue {
            key: "Exec".into(),
            reason: "empty".into(),
        };
        assert!(format!("{e3}").contains("Exec"));
    }

    #[test]
    fn manager_full_workflow() {
        let mut mgr = AutostartManager::new();

        // Add entries.
        mgr.add(StartupEntry::new("firefox", "Firefox", "/usr/bin/firefox").with_delay(0))
            .unwrap();
        mgr.add(StartupEntry::new("slack", "Slack", "/usr/bin/slack").with_delay(3))
            .unwrap();
        mgr.add(
            StartupEntry::new("nm-applet", "Network Manager", "/usr/bin/nm-applet")
                .with_source(EntrySource::System),
        )
        .unwrap();

        assert_eq!(mgr.count(), 3);
        assert_eq!(mgr.enabled_count(), 3);

        // Disable one.
        mgr.disable("slack").unwrap();
        assert_eq!(mgr.enabled_count(), 2);

        // Launch order: only enabled, sorted by delay then name.
        let order = mgr.launch_order();
        assert_eq!(order.len(), 2);
        assert_eq!(order[0].name, "Firefox"); // delay 0
        assert_eq!(order[1].name, "Network Manager"); // delay 0, "N" > "F"

        // Can't remove system entry.
        assert!(mgr.remove("nm-applet").is_err());

        // Can remove user entry.
        mgr.remove("firefox").unwrap();
        assert_eq!(mgr.count(), 2);

        // Re-enable.
        mgr.enable("slack").unwrap();
        assert_eq!(mgr.enabled_count(), 2);
    }

    #[test]
    fn desktop_file_roundtrip_through_manager() {
        let content = "\
[Desktop Entry]
Type=Application
Name=Test App
Exec=/usr/bin/test-app --start
Comment=A test application
Icon=test-icon
X-GNOME-Autostart-Delay=2
";
        let entry = desktop_file::parse_desktop_file(content).unwrap();
        let mut mgr = AutostartManager::new();
        mgr.add(entry).unwrap();

        let got = mgr.get("test-app").unwrap();
        assert_eq!(got.name, "Test App");
        assert_eq!(got.command, "/usr/bin/test-app --start");
        assert_eq!(got.delay_seconds, 2);
        assert_eq!(got.icon.as_deref(), Some("test-icon"));
    }

    #[test]
    fn timer_full_workflow() {
        use std::time::{Duration, Instant};

        let mut timer = StartupTimer::new();
        let t0 = Instant::now();
        timer.begin_at(t0);

        timer.app_started_at("firefox", t0 + Duration::from_millis(10));
        timer.app_ready_at("firefox", t0 + Duration::from_millis(800));

        timer.app_started_at("slack", t0 + Duration::from_millis(3000));
        timer.app_ready_at("slack", t0 + Duration::from_millis(5000));

        assert_eq!(timer.total_time_ms(), 5000);

        let report = timer.report();
        assert_eq!(report.started_count, 2);
        assert_eq!(report.ready_count, 2);
        assert_eq!(report.total_time_ms, 5000);

        // firefox is first (total 800ms), slack second (total 5000ms).
        assert_eq!(report.apps[0].id, "firefox");
        assert_eq!(report.apps[0].total_ms, Some(800));
        assert_eq!(report.apps[0].startup_duration_ms, Some(790));

        assert_eq!(report.apps[1].id, "slack");
        assert_eq!(report.apps[1].total_ms, Some(5000));
    }
}
