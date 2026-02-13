//! Taskbar / dock integration.
//!
//! Provides the [`TaskbarIntegration`] trait for progress indicators,
//! overlay icons, badge counts, and jump lists, and a [`NullTaskbar`]
//! that silently accepts all calls for testing.

use serde::{Deserialize, Serialize};

use crate::PlatformResult;

/// A single entry in a taskbar jump list (Windows) or quicklist (Linux).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JumpListItem {
    /// Display title of the item.
    pub title: String,
    /// Tooltip / description.
    pub description: String,
    /// Icon name or path.
    pub icon: String,
    /// Action identifier dispatched when the item is activated.
    pub action: String,
}

/// Backend for taskbar / dock integration features.
pub trait TaskbarIntegration: Send {
    /// Set the progress indicator on a taskbar button.
    ///
    /// `progress` should be in the range `0.0..=1.0`.
    fn set_progress(&mut self, handle: u64, progress: f64) -> PlatformResult<()>;

    /// Set an overlay icon on the taskbar button (e.g. a status badge).
    fn set_overlay_icon(&mut self, handle: u64, icon_data: &[u8]) -> PlatformResult<()>;

    /// Set the unread / notification badge count.
    fn set_badge_count(&mut self, count: u32) -> PlatformResult<()>;

    /// Add an item to the jump list / quicklist.
    fn add_jump_list_item(&mut self, item: JumpListItem) -> PlatformResult<()>;
}

/// A [`TaskbarIntegration`] that accepts all calls as no-ops.
#[derive(Debug, Default)]
pub struct NullTaskbar;

impl TaskbarIntegration for NullTaskbar {
    fn set_progress(&mut self, _handle: u64, _progress: f64) -> PlatformResult<()> {
        Ok(())
    }

    fn set_overlay_icon(&mut self, _handle: u64, _icon_data: &[u8]) -> PlatformResult<()> {
        Ok(())
    }

    fn set_badge_count(&mut self, _count: u32) -> PlatformResult<()> {
        Ok(())
    }

    fn add_jump_list_item(&mut self, _item: JumpListItem) -> PlatformResult<()> {
        Ok(())
    }
}
