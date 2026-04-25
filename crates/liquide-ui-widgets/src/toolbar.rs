//! Toolbar widget: horizontal strip of tool buttons, separators, and widgets.

use liquide_ui_core::WidgetId;
use serde::{Deserialize, Serialize};

/// Toolbar item identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolItemId(pub u64);

/// A toolbar item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolItem {
    /// A push button.
    Button {
        id: ToolItemId,
        icon: String,
        tooltip: String,
        enabled: bool,
    },
    /// A toggle button (stays pressed).
    Toggle {
        id: ToolItemId,
        icon: String,
        tooltip: String,
        pressed: bool,
        enabled: bool,
    },
    /// A vertical separator.
    Separator,
    /// A spacer that fills remaining space.
    Spacer,
    /// A dropdown button.
    DropdownButton {
        id: ToolItemId,
        icon: String,
        tooltip: String,
        enabled: bool,
    },
}

impl ToolItem {
    #[must_use]
    pub fn button(id: ToolItemId, icon: impl Into<String>, tooltip: impl Into<String>) -> Self {
        Self::Button {
            id,
            icon: icon.into(),
            tooltip: tooltip.into(),
            enabled: true,
        }
    }

    #[must_use]
    pub fn toggle(id: ToolItemId, icon: impl Into<String>, tooltip: impl Into<String>) -> Self {
        Self::Toggle {
            id,
            icon: icon.into(),
            tooltip: tooltip.into(),
            pressed: false,
            enabled: true,
        }
    }

    /// Get the item's ID (separators/spacers return None).
    #[must_use]
    pub fn id(&self) -> Option<ToolItemId> {
        match self {
            Self::Button { id, .. } | Self::Toggle { id, .. } | Self::DropdownButton { id, .. } => {
                Some(*id)
            }
            Self::Separator | Self::Spacer => None,
        }
    }

    /// Is this item enabled?
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        match self {
            Self::Button { enabled, .. }
            | Self::Toggle { enabled, .. }
            | Self::DropdownButton { enabled, .. } => *enabled,
            Self::Separator | Self::Spacer => false,
        }
    }
}

/// Toolbar orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolbarOrientation {
    Horizontal,
    Vertical,
}

/// The toolbar widget.
#[derive(Debug)]
pub struct Toolbar {
    pub id: WidgetId,
    items: Vec<ToolItem>,
    pub orientation: ToolbarOrientation,
    /// Button size in pixels.
    pub button_size: f32,
    /// Spacing between items.
    pub spacing: f32,
    /// Whether to show text labels below icons.
    pub show_labels: bool,
}

impl Toolbar {
    #[must_use]
    pub fn new(id: WidgetId) -> Self {
        Self {
            id,
            items: Vec::new(),
            orientation: ToolbarOrientation::Horizontal,
            button_size: 28.0,
            spacing: 2.0,
            show_labels: false,
        }
    }

    /// Add an item.
    pub fn add_item(&mut self, item: ToolItem) {
        self.items.push(item);
    }

    /// Get all items.
    #[must_use]
    pub fn items(&self) -> &[ToolItem] {
        &self.items
    }

    /// Find an item by ID.
    pub fn find_item_mut(&mut self, id: ToolItemId) -> Option<&mut ToolItem> {
        self.items.iter_mut().find(|item| item.id() == Some(id))
    }

    /// Enable or disable an item.
    pub fn set_enabled(&mut self, id: ToolItemId, enabled: bool) {
        if let Some(item) = self.find_item_mut(id) {
            match item {
                ToolItem::Button { enabled: e, .. }
                | ToolItem::Toggle { enabled: e, .. }
                | ToolItem::DropdownButton { enabled: e, .. } => *e = enabled,
                _ => {}
            }
        }
    }

    /// Toggle a toggle-button.
    pub fn toggle(&mut self, id: ToolItemId) {
        if let Some(ToolItem::Toggle { pressed, .. }) = self.find_item_mut(id) {
            *pressed = !*pressed;
        }
    }

    /// Check if a toggle-button is pressed.
    #[must_use]
    pub fn is_pressed(&self, id: ToolItemId) -> bool {
        self.items.iter().any(
            |item| matches!(item, ToolItem::Toggle { id: tid, pressed: true, .. } if *tid == id),
        )
    }

    /// Total toolbar size along the main axis.
    #[must_use]
    pub fn total_size(&self) -> f32 {
        let mut size = 0.0_f32;
        for item in &self.items {
            match item {
                ToolItem::Separator => size += self.spacing * 2.0 + 1.0,
                ToolItem::Spacer => {} // takes remaining space, handled during layout
                _ => size += self.button_size + self.spacing,
            }
        }
        size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toolbar_items() {
        let mut tb = Toolbar::new(WidgetId::from_raw(1));
        tb.add_item(ToolItem::button(ToolItemId(1), "new", "New File"));
        tb.add_item(ToolItem::Separator);
        tb.add_item(ToolItem::toggle(ToolItemId(2), "bold", "Bold"));
        assert_eq!(tb.items().len(), 3);
    }

    #[test]
    fn test_toggle() {
        let mut tb = Toolbar::new(WidgetId::from_raw(1));
        tb.add_item(ToolItem::toggle(ToolItemId(1), "bold", "Bold"));
        assert!(!tb.is_pressed(ToolItemId(1)));
        tb.toggle(ToolItemId(1));
        assert!(tb.is_pressed(ToolItemId(1)));
    }

    #[test]
    fn test_enable_disable() {
        let mut tb = Toolbar::new(WidgetId::from_raw(1));
        tb.add_item(ToolItem::button(ToolItemId(1), "save", "Save"));
        tb.set_enabled(ToolItemId(1), false);
        assert!(!tb.items()[0].is_enabled());
    }

    #[test]
    fn test_total_size() {
        let mut tb = Toolbar::new(WidgetId::from_raw(1));
        tb.button_size = 28.0;
        tb.spacing = 2.0;
        tb.add_item(ToolItem::button(ToolItemId(1), "a", "A"));
        tb.add_item(ToolItem::button(ToolItemId(2), "b", "B"));
        assert!(tb.total_size() > 0.0);
    }
}
