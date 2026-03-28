//! Status notifier item — the fundamental unit of the system tray.
//!
//! Models the freedesktop.org StatusNotifierItem specification: each item
//! has an identity, category, status, icon set (primary/overlay/attention),
//! tooltip, and an optional menu.

use serde::{Deserialize, Serialize};

/// Unique identifier for a status notifier item, typically a bus name or
/// reverse-DNS string such as `"org.kde.StatusNotifierItem-12345-1"`.
pub type ItemId = String;

/// Semantic category for a status notifier item (SNI spec `Category`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemCategory {
    /// Indicator of application state (e.g. media player, download manager).
    ApplicationStatus,
    /// Communications-related (mail client, chat, IRC).
    Communications,
    /// System service (printer daemon, file indexer, background task).
    SystemServices,
    /// Hardware-related (battery, bluetooth adapter, network interface).
    Hardware,
}

impl ItemCategory {
    /// Returns the sort key used to group items in the tray (lower = leftmost).
    pub fn sort_key(self) -> u8 {
        match self {
            Self::Hardware => 0,
            Self::SystemServices => 1,
            Self::Communications => 2,
            Self::ApplicationStatus => 3,
        }
    }

    /// Display name for the category.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::ApplicationStatus => "Application Status",
            Self::Communications => "Communications",
            Self::SystemServices => "System Services",
            Self::Hardware => "Hardware",
        }
    }
}

/// Status of a notifier item (SNI spec `Status`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemStatus {
    /// The item does not convey important information and can be hidden.
    Passive,
    /// The item is active and should be shown in its normal state.
    Active,
    /// The item needs user attention (e.g. new mail, low battery).
    NeedsAttention,
}

impl ItemStatus {
    /// Returns `true` if the item requests visibility.
    pub fn is_visible(self) -> bool {
        self != Self::Passive
    }
}

/// Rich tooltip data for a status notifier item (SNI spec `ToolTip`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolTip {
    /// Icon name from the icon theme (may be empty).
    pub icon_name: String,
    /// Short summary title.
    pub title: String,
    /// Longer descriptive text (may contain basic markup).
    pub description: String,
}

impl ToolTip {
    /// Create a tooltip with just a title.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            icon_name: String::new(),
            title: title.into(),
            description: String::new(),
        }
    }

    /// Create a tooltip with title and description.
    pub fn with_description(title: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            icon_name: String::new(),
            title: title.into(),
            description: description.into(),
        }
    }

    /// Returns `true` if the tooltip has no useful content.
    pub fn is_empty(&self) -> bool {
        self.title.is_empty() && self.description.is_empty()
    }
}

/// ARGB32 pixel map for an icon. Each pixel is 4 bytes: alpha, red, green, blue
/// in network (big-endian) byte order, matching the SNI `IconPixmap` type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pixmap {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Raw ARGB32 pixel data, length must be `width * height * 4`.
    pub data: Vec<u8>,
}

impl Pixmap {
    /// Create a new pixmap. Returns `None` if the data length does not match
    /// `width * height * 4`.
    pub fn new(width: u32, height: u32, data: Vec<u8>) -> Option<Self> {
        let expected = (width as usize) * (height as usize) * 4;
        if data.len() == expected {
            Some(Self { width, height, data })
        } else {
            None
        }
    }

    /// Total number of pixels.
    pub fn pixel_count(&self) -> usize {
        (self.width as usize) * (self.height as usize)
    }

    /// Returns `true` if the pixmap has zero area.
    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }
}

/// A status notifier item — the core data structure representing a system tray
/// entry. Follows the StatusNotifierItem D-Bus specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusNotifierItem {
    /// Unique identifier (bus name or reverse-DNS ID).
    pub id: ItemId,
    /// Human-readable title.
    pub title: String,
    /// Semantic category.
    pub category: ItemCategory,
    /// Current status.
    pub status: ItemStatus,

    // ── Icon set ────────────────────────────────────────────────────────
    /// Primary icon name from the icon theme.
    pub icon_name: String,
    /// Primary icon pixmap (fallback when icon_name is not found in theme).
    pub icon_pixmap: Vec<Pixmap>,
    /// Overlay icon name (small badge over the primary icon).
    pub overlay_icon_name: String,
    /// Overlay icon pixmap.
    pub overlay_icon_pixmap: Vec<Pixmap>,
    /// Attention icon name (shown when status is `NeedsAttention`).
    pub attention_icon_name: String,
    /// Attention icon pixmap.
    pub attention_icon_pixmap: Vec<Pixmap>,

    // ── Tooltip & menu ─────────────────────────────────────────────────
    /// Rich tooltip data.
    pub tooltip: Option<ToolTip>,
    /// Associated menu (see the `menu` module for tree structure).
    pub menu: Option<crate::menu::TrayMenu>,

    /// Monotonic timestamp (microseconds) of registration.
    pub registered_at_us: u64,
}

impl StatusNotifierItem {
    /// Start building a new status notifier item.
    pub fn builder(id: impl Into<String>) -> StatusNotifierItemBuilder {
        StatusNotifierItemBuilder::new(id)
    }

    /// Returns the best icon name for the current status.
    pub fn effective_icon_name(&self) -> &str {
        if self.status == ItemStatus::NeedsAttention && !self.attention_icon_name.is_empty() {
            &self.attention_icon_name
        } else {
            &self.icon_name
        }
    }

    /// Returns the best icon pixmap for the current status.
    pub fn effective_icon_pixmap(&self) -> &[Pixmap] {
        if self.status == ItemStatus::NeedsAttention && !self.attention_icon_pixmap.is_empty() {
            &self.attention_icon_pixmap
        } else {
            &self.icon_pixmap
        }
    }

    /// Returns `true` if the item currently requests user attention.
    pub fn needs_attention(&self) -> bool {
        self.status == ItemStatus::NeedsAttention
    }

    /// Returns `true` if the item has an overlay icon.
    pub fn has_overlay(&self) -> bool {
        !self.overlay_icon_name.is_empty() || !self.overlay_icon_pixmap.is_empty()
    }

    /// Returns `true` if the item has a tooltip.
    pub fn has_tooltip(&self) -> bool {
        self.tooltip.as_ref().is_some_and(|t| !t.is_empty())
    }
}

impl std::fmt::Display for StatusNotifierItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SNI({}, title={:?}, category={:?}, status={:?})",
            self.id, self.title, self.category, self.status
        )
    }
}

/// Builder for constructing a [`StatusNotifierItem`] incrementally.
pub struct StatusNotifierItemBuilder {
    id: String,
    title: String,
    category: ItemCategory,
    status: ItemStatus,
    icon_name: String,
    icon_pixmap: Vec<Pixmap>,
    overlay_icon_name: String,
    overlay_icon_pixmap: Vec<Pixmap>,
    attention_icon_name: String,
    attention_icon_pixmap: Vec<Pixmap>,
    tooltip: Option<ToolTip>,
    menu: Option<crate::menu::TrayMenu>,
    registered_at_us: u64,
}

impl StatusNotifierItemBuilder {
    /// Create a builder with the given item ID.
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            title: id.clone(),
            id,
            category: ItemCategory::ApplicationStatus,
            status: ItemStatus::Active,
            icon_name: String::new(),
            icon_pixmap: Vec::new(),
            overlay_icon_name: String::new(),
            overlay_icon_pixmap: Vec::new(),
            attention_icon_name: String::new(),
            attention_icon_pixmap: Vec::new(),
            tooltip: None,
            menu: None,
            registered_at_us: 0,
        }
    }

    /// Set the human-readable title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Set the category.
    pub fn category(mut self, category: ItemCategory) -> Self {
        self.category = category;
        self
    }

    /// Set the status.
    pub fn status(mut self, status: ItemStatus) -> Self {
        self.status = status;
        self
    }

    /// Set the primary icon name.
    pub fn icon_name(mut self, name: impl Into<String>) -> Self {
        self.icon_name = name.into();
        self
    }

    /// Set the primary icon pixmap(s).
    pub fn icon_pixmap(mut self, pixmaps: Vec<Pixmap>) -> Self {
        self.icon_pixmap = pixmaps;
        self
    }

    /// Set the overlay icon name.
    pub fn overlay_icon_name(mut self, name: impl Into<String>) -> Self {
        self.overlay_icon_name = name.into();
        self
    }

    /// Set the overlay icon pixmap(s).
    pub fn overlay_icon_pixmap(mut self, pixmaps: Vec<Pixmap>) -> Self {
        self.overlay_icon_pixmap = pixmaps;
        self
    }

    /// Set the attention icon name.
    pub fn attention_icon_name(mut self, name: impl Into<String>) -> Self {
        self.attention_icon_name = name.into();
        self
    }

    /// Set the attention icon pixmap(s).
    pub fn attention_icon_pixmap(mut self, pixmaps: Vec<Pixmap>) -> Self {
        self.attention_icon_pixmap = pixmaps;
        self
    }

    /// Set the tooltip.
    pub fn tooltip(mut self, tooltip: ToolTip) -> Self {
        self.tooltip = Some(tooltip);
        self
    }

    /// Set the menu.
    pub fn menu(mut self, menu: crate::menu::TrayMenu) -> Self {
        self.menu = Some(menu);
        self
    }

    /// Set the registration timestamp (microseconds).
    pub fn registered_at_us(mut self, us: u64) -> Self {
        self.registered_at_us = us;
        self
    }

    /// Build the finished [`StatusNotifierItem`].
    pub fn build(self) -> StatusNotifierItem {
        StatusNotifierItem {
            id: self.id,
            title: self.title,
            category: self.category,
            status: self.status,
            icon_name: self.icon_name,
            icon_pixmap: self.icon_pixmap,
            overlay_icon_name: self.overlay_icon_name,
            overlay_icon_pixmap: self.overlay_icon_pixmap,
            attention_icon_name: self.attention_icon_name,
            attention_icon_pixmap: self.attention_icon_pixmap,
            tooltip: self.tooltip,
            menu: self.menu,
            registered_at_us: self.registered_at_us,
        }
    }
}
