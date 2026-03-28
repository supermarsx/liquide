//! Session restore planning — compares a saved [`SessionState`] against the
//! current environment and produces a [`RestorePlan`].

use crate::state::{DisplayState, SessionState, WindowVisualState};

/// A window that should be (re-)opened during restore.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowRestore {
    pub app_id: String,
    pub bounds: (f32, f32, f32, f32),
    pub workspace_id: u32,
    pub state: WindowVisualState,
}

/// Describes a change in display configuration between the saved session and
/// the current environment.
#[derive(Debug, Clone, PartialEq)]
pub enum DisplayChange {
    /// A new display that was not present in the saved session.
    Added(String),
    /// A display from the saved session that is no longer connected.
    Removed(String),
    /// A display that moved position.
    Moved {
        connector: String,
        from: (i32, i32),
        to: (i32, i32),
    },
}

/// The output of [`SessionRestorer::plan_restore`] — everything needed to
/// recreate the previous session.
#[derive(Debug, Clone, PartialEq)]
pub struct RestorePlan {
    pub windows_to_restore: Vec<WindowRestore>,
    /// Apps referenced in the saved session that are not currently available.
    pub missing_apps: Vec<String>,
    /// Display topology changes detected.
    pub display_changes: Vec<DisplayChange>,
}

/// Builds a [`RestorePlan`] from a saved session and the current environment.
pub struct SessionRestorer;

impl SessionRestorer {
    /// Compare the saved session against the list of currently available
    /// applications and produce a plan.
    pub fn plan_restore(saved: &SessionState, available_apps: &[String]) -> RestorePlan {
        let mut windows_to_restore = Vec::new();
        let mut missing_apps: Vec<String> = Vec::new();

        for w in &saved.windows {
            if available_apps.iter().any(|a| *a == w.app_id) {
                windows_to_restore.push(WindowRestore {
                    app_id: w.app_id.clone(),
                    bounds: w.bounds,
                    workspace_id: w.workspace_id,
                    state: w.state,
                });
            } else if !missing_apps.contains(&w.app_id) {
                missing_apps.push(w.app_id.clone());
            }
        }

        RestorePlan {
            windows_to_restore,
            missing_apps,
            display_changes: Vec::new(),
        }
    }

    /// Adjust window bounds in the plan so that windows land on actually
    /// connected monitors. Also populates display change information.
    ///
    /// Rules:
    /// - If a saved display is still present but moved, shift windows by the
    ///   delta.
    /// - If a saved display is removed, move its windows to the primary
    ///   display (or the first available).
    /// - Clamp all windows so they are fully on-screen.
    pub fn adjust_for_display_changes(
        plan: &mut RestorePlan,
        saved_displays: &[DisplayState],
        current_displays: &[DisplayState],
    ) {
        // Build display-change list.
        let changes = Self::compute_display_changes(saved_displays, current_displays);
        plan.display_changes = changes;

        // Determine the fallback display for orphaned windows.
        let fallback = current_displays
            .iter()
            .find(|d| d.primary)
            .or(current_displays.first());

        // Build a map: connector -> current display.
        let current_map: std::collections::HashMap<&str, &DisplayState> = current_displays
            .iter()
            .map(|d| (d.connector.as_str(), d))
            .collect();

        for win in &mut plan.windows_to_restore {
            // Figure out which saved display this window was on.
            let saved_disp = Self::find_display_for_window(saved_displays, win.bounds);

            match saved_disp {
                Some(sd) => {
                    if let Some(cd) = current_map.get(sd.connector.as_str()) {
                        // Display still exists — apply position delta.
                        let dx = cd.position.0 - sd.position.0;
                        let dy = cd.position.1 - sd.position.1;
                        win.bounds.0 += dx as f32;
                        win.bounds.1 += dy as f32;
                        Self::clamp_to_display(win, cd);
                    } else {
                        // Display removed — move to fallback.
                        if let Some(fb) = fallback {
                            Self::move_to_display(win, fb);
                        }
                    }
                }
                None => {
                    // Could not determine original display; clamp to fallback.
                    if let Some(fb) = fallback {
                        Self::clamp_to_display(win, fb);
                    }
                }
            }
        }
    }

    /// Compute the list of display changes between two configurations.
    fn compute_display_changes(
        saved: &[DisplayState],
        current: &[DisplayState],
    ) -> Vec<DisplayChange> {
        let mut changes = Vec::new();

        // Check for removed / moved displays.
        for sd in saved {
            match current.iter().find(|c| c.connector == sd.connector) {
                Some(cd) => {
                    if cd.position != sd.position {
                        changes.push(DisplayChange::Moved {
                            connector: sd.connector.clone(),
                            from: sd.position,
                            to: cd.position,
                        });
                    }
                }
                None => {
                    changes.push(DisplayChange::Removed(sd.connector.clone()));
                }
            }
        }

        // Check for added displays.
        for cd in current {
            if !saved.iter().any(|s| s.connector == cd.connector) {
                changes.push(DisplayChange::Added(cd.connector.clone()));
            }
        }

        changes
    }

    /// Find which display a window (by its top-left corner) belongs to.
    fn find_display_for_window<'a>(
        displays: &'a [DisplayState],
        bounds: (f32, f32, f32, f32),
    ) -> Option<&'a DisplayState> {
        let wx = bounds.0 as i32;
        let wy = bounds.1 as i32;

        // Find the display whose rect contains the window origin.
        for d in displays {
            let dx = d.position.0;
            let dy = d.position.1;
            let dw = d.resolution.0 as i32;
            let dh = d.resolution.1 as i32;
            if wx >= dx && wx < dx + dw && wy >= dy && wy < dy + dh {
                return Some(d);
            }
        }

        // Fallback: closest display by center distance.
        if displays.is_empty() {
            return None;
        }
        let mut best = &displays[0];
        let mut best_dist = i64::MAX;
        for d in displays {
            let cx = d.position.0 as i64 + d.resolution.0 as i64 / 2;
            let cy = d.position.1 as i64 + d.resolution.1 as i64 / 2;
            let dist = (wx as i64 - cx).abs() + (wy as i64 - cy).abs();
            if dist < best_dist {
                best_dist = dist;
                best = d;
            }
        }
        Some(best)
    }

    /// Move a window to the center of a display.
    fn move_to_display(win: &mut WindowRestore, display: &DisplayState) {
        let dw = display.resolution.0 as f32;
        let dh = display.resolution.1 as f32;
        let dx = display.position.0 as f32;
        let dy = display.position.1 as f32;

        // Clamp window size to display size.
        let ww = win.bounds.2.min(dw);
        let wh = win.bounds.3.min(dh);

        // Center on the display.
        win.bounds.0 = dx + (dw - ww) / 2.0;
        win.bounds.1 = dy + (dh - wh) / 2.0;
        win.bounds.2 = ww;
        win.bounds.3 = wh;
    }

    /// Clamp a window so it is fully within the given display.
    fn clamp_to_display(win: &mut WindowRestore, display: &DisplayState) {
        let dx = display.position.0 as f32;
        let dy = display.position.1 as f32;
        let dw = display.resolution.0 as f32;
        let dh = display.resolution.1 as f32;

        // Clamp size.
        if win.bounds.2 > dw {
            win.bounds.2 = dw;
        }
        if win.bounds.3 > dh {
            win.bounds.3 = dh;
        }

        // Clamp position.
        if win.bounds.0 < dx {
            win.bounds.0 = dx;
        }
        if win.bounds.1 < dy {
            win.bounds.1 = dy;
        }
        if win.bounds.0 + win.bounds.2 > dx + dw {
            win.bounds.0 = dx + dw - win.bounds.2;
        }
        if win.bounds.1 + win.bounds.3 > dy + dh {
            win.bounds.1 = dy + dh - win.bounds.3;
        }
    }
}
