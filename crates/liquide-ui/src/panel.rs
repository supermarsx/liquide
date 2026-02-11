//! Panel and container widget types for the desktop shell.

use std::collections::HashMap;

use crate::widget::WidgetId;

/// Position of a panel on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelPosition {
    /// Panel at the top of the screen.
    Top,
    /// Panel at the bottom of the screen.
    Bottom,
    /// Panel on the left edge.
    Left,
    /// Panel on the right edge.
    Right,
}

impl std::fmt::Display for PanelPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Top => write!(f, "Top"),
            Self::Bottom => write!(f, "Bottom"),
            Self::Left => write!(f, "Left"),
            Self::Right => write!(f, "Right"),
        }
    }
}

/// Configuration for a panel.
#[derive(Debug, Clone)]
pub struct PanelConfig {
    /// Where the panel is positioned.
    pub position: PanelPosition,
    /// Thickness of the panel in pixels.
    pub thickness: u32,
    /// Whether the panel hides automatically.
    pub auto_hide: bool,
    /// Stacking z-index.
    pub z_index: i32,
}

impl Default for PanelConfig {
    fn default() -> Self {
        Self {
            position: PanelPosition::Bottom,
            thickness: 48,
            auto_hide: false,
            z_index: 100,
        }
    }
}

/// A panel container that holds widgets along a screen edge.
#[derive(Debug, Clone)]
pub struct Panel {
    /// Panel configuration.
    config: PanelConfig,
    /// Widgets contained in this panel.
    widget_ids: Vec<WidgetId>,
    /// Whether the panel is currently visible.
    visible: bool,
}

impl Panel {
    /// Create a new panel with the given configuration.
    #[must_use]
    pub fn new(config: PanelConfig) -> Self {
        Self {
            config,
            widget_ids: Vec::new(),
            visible: true,
        }
    }

    /// Add a widget to this panel.
    pub fn add_widget(&mut self, id: WidgetId) {
        if !self.widget_ids.contains(&id) {
            self.widget_ids.push(id);
        }
    }

    /// Remove a widget from this panel.
    pub fn remove_widget(&mut self, id: &WidgetId) {
        self.widget_ids.retain(|w| w != id);
    }

    /// The panel's position.
    #[must_use]
    pub fn position(&self) -> PanelPosition {
        self.config.position
    }

    /// The panel's thickness in pixels.
    #[must_use]
    pub fn thickness(&self) -> u32 {
        self.config.thickness
    }

    /// Set whether the panel automatically hides.
    pub fn set_auto_hide(&mut self, auto_hide: bool) {
        self.config.auto_hide = auto_hide;
    }

    /// Whether auto-hide is enabled.
    #[must_use]
    pub fn auto_hide(&self) -> bool {
        self.config.auto_hide
    }

    /// Whether the panel is currently visible.
    #[must_use]
    pub fn visible(&self) -> bool {
        self.visible
    }

    /// Set visibility.
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// The widgets in this panel.
    #[must_use]
    pub fn widget_ids(&self) -> &[WidgetId] {
        &self.widget_ids
    }

    /// The panel configuration.
    #[must_use]
    pub fn config(&self) -> &PanelConfig {
        &self.config
    }
}

/// Slot position in a status bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatusBarSlot {
    /// Left-aligned slot.
    Left,
    /// Center slot.
    Center,
    /// Right-aligned slot.
    Right,
}

/// A status bar with left, center, and right slots.
#[derive(Debug, Clone, Default)]
pub struct StatusBar {
    /// Widgets in each slot.
    slots: HashMap<StatusBarSlot, Vec<WidgetId>>,
}

impl StatusBar {
    /// Create a new empty status bar.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a widget to a slot.
    pub fn add_to_slot(&mut self, slot: StatusBarSlot, widget_id: WidgetId) {
        self.slots.entry(slot).or_default().push(widget_id);
    }

    /// Remove a widget from a slot.
    pub fn remove_from_slot(&mut self, slot: StatusBarSlot, id: &WidgetId) {
        if let Some(widgets) = self.slots.get_mut(&slot) {
            widgets.retain(|w| w != id);
        }
    }

    /// The widgets in a given slot.
    #[must_use]
    pub fn widgets_in_slot(&self, slot: StatusBarSlot) -> &[WidgetId] {
        self.slots.get(&slot).map_or(&[], |v| v.as_slice())
    }
}
