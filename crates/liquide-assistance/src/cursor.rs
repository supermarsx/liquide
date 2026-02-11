//! Ghost cursor rendering for remote observers.

use serde::{Deserialize, Serialize};

use crate::mode::AssistanceMode;

/// Visual appearance of an observer's cursor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorAppearance {
    /// Opacity from 0.0 (invisible) to 1.0 (fully visible).
    pub opacity: f32,
    /// Optional RGBA ring color.
    pub ring_color: Option<[u8; 4]>,
    /// Optional label shown near the cursor.
    pub label: Option<String>,
    /// Whether the cursor is visible at all.
    pub visible: bool,
}

/// A ghost cursor representing an observer's pointer position.
#[derive(Debug, Clone)]
pub struct GhostCursor {
    /// The observer this cursor belongs to.
    pub observer_id: String,
    /// Horizontal position.
    pub x: f64,
    /// Vertical position.
    pub y: f64,
    /// Visual appearance.
    pub appearance: CursorAppearance,
    /// Last time the position was updated (unix ms).
    pub last_update: u64,
}

/// Return the cursor appearance for a given mode and observer name.
#[must_use]
pub fn cursor_appearance_for_mode(mode: AssistanceMode, observer_name: &str) -> CursorAppearance {
    match mode {
        AssistanceMode::ViewOnly => CursorAppearance {
            opacity: 0.5,
            ring_color: Some([100, 149, 237, 255]),
            label: Some(observer_name.to_string()),
            visible: true,
        },
        AssistanceMode::Interactive => CursorAppearance {
            opacity: 1.0,
            ring_color: Some([50, 205, 50, 255]),
            label: Some(observer_name.to_string()),
            visible: true,
        },
        AssistanceMode::Exclusive => CursorAppearance {
            opacity: 1.0,
            ring_color: None,
            label: Some("Remote Control".to_string()),
            visible: true,
        },
        AssistanceMode::Stealth => CursorAppearance {
            opacity: 0.0,
            ring_color: None,
            label: None,
            visible: false,
        },
    }
}
