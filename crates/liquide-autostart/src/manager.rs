use crate::entry::{EntrySource, StartupEntry};
use crate::error::AutostartError;

/// Manages the set of autostart entries for the current session.
///
/// Entries can come from system directories, user configuration, or be
/// added dynamically for the current session only.
pub struct AutostartManager {
    entries: Vec<StartupEntry>,
}

impl AutostartManager {
    /// Create a new empty manager.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Create a manager pre-populated with the given entries.
    pub fn with_entries(entries: Vec<StartupEntry>) -> Self {
        Self { entries }
    }

    /// Return all startup entries (enabled and disabled).
    pub fn list(&self) -> Vec<StartupEntry> {
        self.entries.clone()
    }

    /// Return references to only the enabled entries.
    pub fn enabled_entries(&self) -> Vec<&StartupEntry> {
        self.entries.iter().filter(|e| e.enabled).collect()
    }

    /// Add a new startup entry.
    ///
    /// Returns an error if the id is empty, the command is empty,
    /// or an entry with the same id already exists.
    pub fn add(&mut self, entry: StartupEntry) -> Result<(), AutostartError> {
        if entry.id.is_empty() {
            return Err(AutostartError::InvalidId);
        }
        if entry.command.trim().is_empty() {
            return Err(AutostartError::InvalidCommand(
                "command must not be empty".into(),
            ));
        }
        if self.entries.iter().any(|e| e.id == entry.id) {
            return Err(AutostartError::DuplicateEntry(entry.id.clone()));
        }
        self.entries.push(entry);
        Ok(())
    }

    /// Remove an entry by id.
    ///
    /// System entries cannot be removed (only disabled).
    /// Returns an error if the entry is not found or is a system entry.
    pub fn remove(&mut self, id: &str) -> Result<(), AutostartError> {
        let pos = self
            .entries
            .iter()
            .position(|e| e.id == id)
            .ok_or_else(|| AutostartError::NotFound(id.into()))?;

        if self.entries[pos].source == EntrySource::System {
            return Err(AutostartError::SystemEntryCannotBeRemoved(id.into()));
        }

        self.entries.remove(pos);
        Ok(())
    }

    /// Enable an entry by id. No-op if already enabled.
    /// Returns an error if the entry is not found.
    pub fn enable(&mut self, id: &str) -> Result<(), AutostartError> {
        let entry = self
            .entries
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or_else(|| AutostartError::NotFound(id.into()))?;
        entry.enabled = true;
        Ok(())
    }

    /// Disable an entry by id. No-op if already disabled.
    /// Returns an error if the entry is not found.
    pub fn disable(&mut self, id: &str) -> Result<(), AutostartError> {
        let entry = self
            .entries
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or_else(|| AutostartError::NotFound(id.into()))?;
        entry.enabled = false;
        Ok(())
    }

    /// Set the startup delay for an entry.
    /// Returns an error if the entry is not found.
    pub fn set_delay(&mut self, id: &str, seconds: u32) -> Result<(), AutostartError> {
        let entry = self
            .entries
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or_else(|| AutostartError::NotFound(id.into()))?;
        entry.delay_seconds = seconds;
        Ok(())
    }

    /// Return enabled entries sorted by launch order:
    /// first by delay (ascending), then by name (alphabetical).
    pub fn launch_order(&self) -> Vec<&StartupEntry> {
        let mut enabled: Vec<&StartupEntry> = self.enabled_entries();
        enabled.sort_by(|a, b| {
            a.delay_seconds
                .cmp(&b.delay_seconds)
                .then_with(|| a.name.cmp(&b.name))
        });
        enabled
    }

    /// Estimate the total boot time in milliseconds for a set of entries.
    ///
    /// This sums each entry's estimated startup time (delay + 500ms spawn overhead).
    /// Entries with the same delay are assumed to run in parallel, so only the
    /// longest spawn overhead within each delay group counts.
    pub fn estimate_boot_time(entries: &[StartupEntry]) -> u32 {
        if entries.is_empty() {
            return 0;
        }

        // Group by delay_seconds, find the max delay, then add per-group overhead.
        let mut max_delay: u32 = 0;
        let mut delay_groups: std::collections::HashMap<u32, u32> =
            std::collections::HashMap::new();

        for entry in entries {
            if !entry.enabled {
                continue;
            }
            if entry.delay_seconds > max_delay {
                max_delay = entry.delay_seconds;
            }
            let count = delay_groups.entry(entry.delay_seconds).or_insert(0);
            *count += 1;
        }

        if delay_groups.is_empty() {
            return 0;
        }

        // Total time = max delay * 1000 + 500ms (process spawn overhead for the last group).
        // Each group launches in parallel internally, so we don't multiply by count.
        max_delay * 1000 + 500
    }

    /// Return a reference to the entry with the given id, if it exists.
    pub fn get(&self, id: &str) -> Option<&StartupEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Return a mutable reference to the entry with the given id, if it exists.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut StartupEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    /// Return the total number of entries (enabled + disabled).
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// Return the number of enabled entries.
    pub fn enabled_count(&self) -> usize {
        self.entries.iter().filter(|e| e.enabled).count()
    }

    /// Filter entries that should be shown in a given desktop environment.
    pub fn entries_for_desktop(&self, desktop: &str) -> Vec<&StartupEntry> {
        self.entries
            .iter()
            .filter(|e| e.should_show_in(desktop))
            .collect()
    }
}

impl Default for AutostartManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(id: &str, name: &str, cmd: &str) -> StartupEntry {
        StartupEntry::new(id, name, cmd)
    }

    #[test]
    fn new_manager_is_empty() {
        let mgr = AutostartManager::new();
        assert_eq!(mgr.count(), 0);
        assert!(mgr.list().is_empty());
    }

    #[test]
    fn add_and_list() {
        let mut mgr = AutostartManager::new();
        mgr.add(make_entry("firefox", "Firefox", "/usr/bin/firefox"))
            .unwrap();
        assert_eq!(mgr.count(), 1);
        assert_eq!(mgr.list()[0].name, "Firefox");
    }

    #[test]
    fn add_duplicate_fails() {
        let mut mgr = AutostartManager::new();
        mgr.add(make_entry("app", "App", "/bin/app")).unwrap();
        let err = mgr.add(make_entry("app", "App 2", "/bin/app2")).unwrap_err();
        assert_eq!(err, AutostartError::DuplicateEntry("app".into()));
    }

    #[test]
    fn add_empty_id_fails() {
        let mut mgr = AutostartManager::new();
        let err = mgr
            .add(make_entry("", "No ID", "/bin/test"))
            .unwrap_err();
        assert_eq!(err, AutostartError::InvalidId);
    }

    #[test]
    fn add_empty_command_fails() {
        let mut mgr = AutostartManager::new();
        let err = mgr.add(make_entry("app", "App", "  ")).unwrap_err();
        match err {
            AutostartError::InvalidCommand(_) => {}
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn remove_user_entry() {
        let mut mgr = AutostartManager::new();
        mgr.add(make_entry("app", "App", "/bin/app")).unwrap();
        mgr.remove("app").unwrap();
        assert_eq!(mgr.count(), 0);
    }

    #[test]
    fn remove_system_entry_fails() {
        let mut mgr = AutostartManager::new();
        mgr.add(
            make_entry("sys", "System App", "/bin/sys").with_source(EntrySource::System),
        )
        .unwrap();
        let err = mgr.remove("sys").unwrap_err();
        assert_eq!(
            err,
            AutostartError::SystemEntryCannotBeRemoved("sys".into())
        );
    }

    #[test]
    fn remove_not_found() {
        let mut mgr = AutostartManager::new();
        let err = mgr.remove("nonexistent").unwrap_err();
        assert_eq!(err, AutostartError::NotFound("nonexistent".into()));
    }

    #[test]
    fn enable_disable() {
        let mut mgr = AutostartManager::new();
        mgr.add(make_entry("app", "App", "/bin/app")).unwrap();
        assert!(mgr.get("app").unwrap().enabled);

        mgr.disable("app").unwrap();
        assert!(!mgr.get("app").unwrap().enabled);
        assert_eq!(mgr.enabled_count(), 0);

        mgr.enable("app").unwrap();
        assert!(mgr.get("app").unwrap().enabled);
        assert_eq!(mgr.enabled_count(), 1);
    }

    #[test]
    fn enable_not_found() {
        let mut mgr = AutostartManager::new();
        assert_eq!(
            mgr.enable("nope").unwrap_err(),
            AutostartError::NotFound("nope".into())
        );
    }

    #[test]
    fn disable_not_found() {
        let mut mgr = AutostartManager::new();
        assert_eq!(
            mgr.disable("nope").unwrap_err(),
            AutostartError::NotFound("nope".into())
        );
    }

    #[test]
    fn set_delay() {
        let mut mgr = AutostartManager::new();
        mgr.add(make_entry("app", "App", "/bin/app")).unwrap();
        mgr.set_delay("app", 5).unwrap();
        assert_eq!(mgr.get("app").unwrap().delay_seconds, 5);
    }

    #[test]
    fn set_delay_not_found() {
        let mut mgr = AutostartManager::new();
        assert_eq!(
            mgr.set_delay("nope", 5).unwrap_err(),
            AutostartError::NotFound("nope".into())
        );
    }

    #[test]
    fn launch_order_sorts_by_delay_then_name() {
        let mut mgr = AutostartManager::new();
        mgr.add(make_entry("c", "Charlie", "/bin/c").with_delay(2))
            .unwrap();
        mgr.add(make_entry("a", "Alice", "/bin/a").with_delay(0))
            .unwrap();
        mgr.add(make_entry("b", "Bob", "/bin/b").with_delay(0))
            .unwrap();
        mgr.add(make_entry("d", "Delta", "/bin/d").with_delay(1))
            .unwrap();

        let order = mgr.launch_order();
        assert_eq!(order[0].name, "Alice");
        assert_eq!(order[1].name, "Bob");
        assert_eq!(order[2].name, "Delta");
        assert_eq!(order[3].name, "Charlie");
    }

    #[test]
    fn launch_order_excludes_disabled() {
        let mut mgr = AutostartManager::new();
        mgr.add(make_entry("a", "A", "/bin/a")).unwrap();
        mgr.add(make_entry("b", "B", "/bin/b").with_enabled(false))
            .unwrap();
        let order = mgr.launch_order();
        assert_eq!(order.len(), 1);
        assert_eq!(order[0].name, "A");
    }

    #[test]
    fn estimate_boot_time_empty() {
        assert_eq!(AutostartManager::estimate_boot_time(&[]), 0);
    }

    #[test]
    fn estimate_boot_time_single() {
        let entries = vec![make_entry("a", "A", "/bin/a").with_delay(3)];
        // 3 * 1000 + 500 = 3500
        assert_eq!(AutostartManager::estimate_boot_time(&entries), 3500);
    }

    #[test]
    fn estimate_boot_time_parallel_group() {
        let entries = vec![
            make_entry("a", "A", "/bin/a").with_delay(0),
            make_entry("b", "B", "/bin/b").with_delay(0),
            make_entry("c", "C", "/bin/c").with_delay(0),
        ];
        // All at delay 0, parallel: 0 * 1000 + 500 = 500
        assert_eq!(AutostartManager::estimate_boot_time(&entries), 500);
    }

    #[test]
    fn estimate_boot_time_mixed_delays() {
        let entries = vec![
            make_entry("a", "A", "/bin/a").with_delay(0),
            make_entry("b", "B", "/bin/b").with_delay(5),
            make_entry("c", "C", "/bin/c").with_delay(2),
        ];
        // Max delay is 5: 5 * 1000 + 500 = 5500
        assert_eq!(AutostartManager::estimate_boot_time(&entries), 5500);
    }

    #[test]
    fn estimate_boot_time_ignores_disabled() {
        let entries = vec![
            make_entry("a", "A", "/bin/a")
                .with_delay(10)
                .with_enabled(false),
            make_entry("b", "B", "/bin/b").with_delay(1),
        ];
        // Only "b" counts: 1 * 1000 + 500 = 1500
        assert_eq!(AutostartManager::estimate_boot_time(&entries), 1500);
    }

    #[test]
    fn with_entries_constructor() {
        let entries = vec![
            make_entry("a", "A", "/bin/a"),
            make_entry("b", "B", "/bin/b"),
        ];
        let mgr = AutostartManager::with_entries(entries);
        assert_eq!(mgr.count(), 2);
    }

    #[test]
    fn entries_for_desktop_filtering() {
        let mut mgr = AutostartManager::new();
        mgr.add(
            make_entry("gnome-only", "GNOME App", "/bin/g")
                .with_only_show_in(vec!["GNOME".into()]),
        )
        .unwrap();
        mgr.add(
            make_entry("no-kde", "No KDE", "/bin/nk").with_not_show_in(vec!["KDE".into()]),
        )
        .unwrap();
        mgr.add(make_entry("universal", "Universal", "/bin/u"))
            .unwrap();

        let gnome = mgr.entries_for_desktop("GNOME");
        assert_eq!(gnome.len(), 3); // gnome-only + no-kde + universal

        let kde = mgr.entries_for_desktop("KDE");
        assert_eq!(kde.len(), 1); // universal only (gnome-only excluded, no-kde excluded)
        assert_eq!(kde[0].id, "universal");
    }

    #[test]
    fn get_mut_modifies_entry() {
        let mut mgr = AutostartManager::new();
        mgr.add(make_entry("app", "App", "/bin/app")).unwrap();
        mgr.get_mut("app").unwrap().name = "Updated App".into();
        assert_eq!(mgr.get("app").unwrap().name, "Updated App");
    }

    #[test]
    fn remove_session_entry() {
        let mut mgr = AutostartManager::new();
        mgr.add(
            make_entry("sess", "Session App", "/bin/sess").with_source(EntrySource::Session),
        )
        .unwrap();
        mgr.remove("sess").unwrap();
        assert_eq!(mgr.count(), 0);
    }
}
