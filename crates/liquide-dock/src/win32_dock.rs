//! Win32 dock integration — enumerates running Windows apps and syncs them to the dock.
//!
//! Uses [`liquide_platform_win32::Win32WindowEnumerator`] to discover visible
//! top-level windows, then calls `Dock::add_running` / `Dock::remove_running`
//! to keep the dock item list in sync with what is actually running on the
//! Windows desktop.
//!
//! # Architecture
//!
//! ```text
//! Win32DockIntegration
//!     ├── Win32WindowEnumerator::discover()  → known HWND list
//!     ├── diff against previous snapshot      → opened / closed events
//!     └── Dock::add_running / remove_running → dock updated
//! ```

use std::collections::HashMap;

use liquide_platform_win32::enumerator::Win32WindowEnumerator;
use liquide_platform_win32::types::{Win32AppEvent, Win32AppInfo};

use crate::Dock;

/// Syncs Win32 windows to the shell dock.
pub struct Win32DockIntegration {
    enumerator: Win32WindowEnumerator,
    /// Maps app_id (exe name) → set of HWNDs.
    app_windows: HashMap<String, Vec<u64>>,
}

impl Win32DockIntegration {
    /// Create a new Win32 dock integration.
    pub fn new() -> Self {
        Self {
            enumerator: Win32WindowEnumerator::new(),
            app_windows: HashMap::new(),
        }
    }

    /// Poll for Win32 window changes and update the dock.
    ///
    /// Call this periodically (e.g. every 500ms or on a timer) to keep
    /// the dock in sync with running Windows applications.
    pub fn poll(&mut self, dock: &mut Dock) {
        // Discover current windows.
        let windows = self.enumerator.discover();

        // Drain events from the enumerator.
        let events = self.enumerator.poll_events();

        for event in &events {
            match event {
                Win32AppEvent::WindowOpened(info) => {
                    self.handle_window_opened(dock, info);
                }
                Win32AppEvent::WindowClosed { hwnd, .. } => {
                    self.handle_window_closed(dock, *hwnd as u64);
                }
                Win32AppEvent::WindowChanged(info) => {
                    // Title or state changed — ensure the app is tracked and
                    // re-emit to the dock so attention indicators can clear.
                    let app_id = Self::app_id_for(info);
                    if !self.app_windows.contains_key(&app_id) {
                        self.handle_window_opened(dock, info);
                    }
                    dock.on_window_changed(&app_id);
                }
                Win32AppEvent::WindowFocused { hwnd } => {
                    // Highlight the dock item whose app owns this HWND.
                    let app_id = self.app_id_for_hwnd(*hwnd as u64);
                    if let Some(ref id) = app_id {
                        // Focusing an app implicitly clears its attention flag.
                        dock.set_needs_attention(id, false);
                    }
                    dock.set_focused_app(app_id.as_deref());
                }
                Win32AppEvent::WindowUnfocused { hwnd } => {
                    // Only clear focus if the unfocused HWND matched the
                    // currently focused app (the shell may send unfocus for a
                    // window whose app already lost focus).
                    if let Some(app_id) = self.app_id_for_hwnd(*hwnd as u64) {
                        if dock.focused_app() == Some(app_id.as_str()) {
                            dock.set_focused_app(None);
                        }
                    }
                }
            }
        }

        // If no events were generated, do a full reconciliation against the
        // current window list to catch anything missed.
        if events.is_empty() {
            if let Ok(ref windows) = windows {
                self.reconcile(dock, windows);
            }
        }
    }

    /// Handle a newly opened window.
    fn handle_window_opened(&mut self, dock: &mut Dock, info: &Win32AppInfo) {
        let app_id = Self::app_id_for(info);
        let hwnd = info.hwnd as u64;

        let entry = self.app_windows.entry(app_id.clone()).or_default();
        if !entry.contains(&hwnd) {
            entry.push(hwnd);
        }

        // Tell the dock about this running app.
        dock.add_running(&app_id);

        tracing::debug!(
            app_id = %app_id,
            hwnd = %hwnd,
            title = %info.title,
            "Win32 window opened → dock updated"
        );
    }

    /// Handle a closed window.
    fn handle_window_closed(&mut self, dock: &mut Dock, hwnd: u64) {
        // Find which app this HWND belongs to.
        let mut removed_app = None;
        for (app_id, hwnds) in &mut self.app_windows {
            if let Some(pos) = hwnds.iter().position(|&h| h == hwnd) {
                hwnds.remove(pos);
                dock.remove_running(app_id);
                if hwnds.is_empty() {
                    removed_app = Some(app_id.clone());
                }
                break;
            }
        }

        if let Some(app_id) = removed_app {
            self.app_windows.remove(&app_id);
            tracing::debug!(
                app_id = %app_id,
                hwnd = %hwnd,
                "Win32 app fully closed → removed from dock"
            );
        }
    }

    /// Full reconciliation: compare known windows vs actual windows.
    fn reconcile(&mut self, dock: &mut Dock, current_windows: &[Win32AppInfo]) {
        // Build set of current HWNDs.
        let mut current_hwnds: HashMap<String, Vec<u64>> = HashMap::new();
        for info in current_windows {
            let app_id = Self::app_id_for(info);
            current_hwnds
                .entry(app_id)
                .or_default()
                .push(info.hwnd as u64);
        }

        // Find newly appeared apps.
        for (app_id, hwnds) in &current_hwnds {
            if !self.app_windows.contains_key(app_id) {
                for _hwnd in hwnds {
                    dock.add_running(app_id);
                }
            }
        }

        // Find disappeared apps.
        let old_apps: Vec<String> = self.app_windows.keys().cloned().collect();
        for app_id in old_apps {
            if !current_hwnds.contains_key(&app_id) {
                // All windows for this app are gone.
                let count = self.app_windows.get(&app_id).map_or(0, |v| v.len());
                for _ in 0..count {
                    dock.remove_running(&app_id);
                }
            }
        }

        // Update our snapshot.
        self.app_windows = current_hwnds;
    }

    /// Derive a dock-friendly app_id from Win32 window info.
    ///
    /// Uses the exe name (without extension) as the app identifier,
    /// falling back to the window class name.
    fn app_id_for(info: &Win32AppInfo) -> String {
        if !info.app_name.is_empty() {
            // Clean exe name: "chrome.exe" → "chrome"
            info.app_name
                .trim_end_matches(".exe")
                .trim_end_matches(".EXE")
                .to_lowercase()
        } else if !info.exe_path.is_empty() {
            // Extract filename from path.
            std::path::Path::new(&info.exe_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_lowercase()
        } else if !info.class_name.is_empty() {
            info.class_name.to_lowercase()
        } else {
            format!("win32-{}", info.hwnd)
        }
    }

    /// Get the current list of tracked Win32 apps and their window counts.
    pub fn tracked_apps(&self) -> Vec<(String, usize)> {
        self.app_windows
            .iter()
            .map(|(app_id, hwnds)| (app_id.clone(), hwnds.len()))
            .collect()
    }

    /// Look up which tracked `app_id` owns a given HWND, if any.
    fn app_id_for_hwnd(&self, hwnd: u64) -> Option<String> {
        self.app_windows.iter().find_map(|(app_id, hwnds)| {
            if hwnds.contains(&hwnd) {
                Some(app_id.clone())
            } else {
                None
            }
        })
    }
}

impl Default for Win32DockIntegration {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DockConfig;

    #[test]
    fn test_app_id_for() {
        let info = Win32AppInfo {
            hwnd: 12345,
            title: "Google Chrome".to_string(),
            class_name: "Chrome_WidgetWin_1".to_string(),
            pid: 1234,
            exe_path: r"C:\Program Files\Google\Chrome\chrome.exe".to_string(),
            app_name: "chrome.exe".to_string(),
            visible: true,
            minimized: false,
            focused: false,
            state: liquide_platform_win32::types::Win32WindowState::Normal,
            bounds: (100, 100, 800, 600),
        };

        let app_id = Win32DockIntegration::app_id_for(&info);
        assert_eq!(app_id, "chrome");
    }

    #[test]
    fn test_win32_dock_integration_new() {
        let integration = Win32DockIntegration::new();
        assert!(integration.tracked_apps().is_empty());
    }

    #[test]
    fn test_reconcile_adds_new_apps() {
        let mut integration = Win32DockIntegration::new();
        let mut dock = Dock::new(DockConfig::default());

        let windows = vec![Win32AppInfo {
            hwnd: 100,
            title: "Notepad".to_string(),
            class_name: "Notepad".to_string(),
            pid: 999,
            exe_path: r"C:\Windows\notepad.exe".to_string(),
            app_name: "notepad.exe".to_string(),
            visible: true,
            minimized: false,
            focused: true,
            state: liquide_platform_win32::types::Win32WindowState::Normal,
            bounds: (0, 0, 640, 480),
        }];

        integration.reconcile(&mut dock, &windows);

        assert_eq!(integration.tracked_apps().len(), 1);
        assert!(
            integration
                .tracked_apps()
                .iter()
                .any(|(id, _)| id == "notepad")
        );
    }
}
