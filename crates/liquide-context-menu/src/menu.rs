//! Core context menu types and logic.

use liquide_compositor::geometry::{Point, Rect};
use liquide_compositor::pixel::Color;
use liquide_compositor::scene::{GlassParams, NodeProperties, SceneNode, SceneNodeKind};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Visual configuration for context menus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextMenuConfig {
    /// Width of the menu panel in logical pixels.
    pub width: f32,
    /// Height of each menu item row.
    pub item_height: f32,
    /// Vertical padding at the top and bottom of the menu.
    pub padding: f32,
    /// Horizontal padding inside each item.
    pub item_padding: f32,
    /// Corner radius for the glass panel.
    pub corner_radius: f32,
    /// Blur radius for the glass backdrop.
    pub blur_radius: u32,
}

impl Default for ContextMenuConfig {
    fn default() -> Self {
        Self {
            width: 260.0,
            item_height: 36.0,
            padding: 8.0,
            item_padding: 12.0,
            corner_radius: 8.0,
            blur_radius: 20,
        }
    }
}

// ---------------------------------------------------------------------------
// Menu item types
// ---------------------------------------------------------------------------

/// What an action payload looks like — just a u32 tag that the shell maps
/// to its own `ShellAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MenuAction(pub u32);

/// The kind of content in a menu entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MenuItemKind {
    /// A normal clickable item.
    Action(MenuAction),
    /// A submenu that opens when hovered.
    Submenu(Vec<MenuItem>),
    /// A toggle / checkbox item.
    Toggle {
        action: MenuAction,
        checked: bool,
    },
}

/// A horizontal separator between menu item groups.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MenuSeparator;

/// A single entry in a context menu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MenuItem {
    /// Display label.
    pub label: String,
    /// Optional icon name (resolved elsewhere).
    pub icon: Option<String>,
    /// What happens when the item is activated.
    pub kind: MenuItemKind,
    /// Whether the item is greyed out.
    pub disabled: bool,
    /// Optional keyboard shortcut hint displayed on the right.
    pub shortcut_hint: Option<String>,
}

impl MenuItem {
    /// Create a simple action item.
    pub fn action(label: impl Into<String>, action: MenuAction) -> Self {
        Self {
            label: label.into(),
            icon: None,
            kind: MenuItemKind::Action(action),
            disabled: false,
            shortcut_hint: None,
        }
    }

    /// Create an action item with an icon.
    pub fn action_with_icon(
        label: impl Into<String>,
        icon: impl Into<String>,
        action: MenuAction,
    ) -> Self {
        Self {
            label: label.into(),
            icon: Some(icon.into()),
            kind: MenuItemKind::Action(action),
            disabled: false,
            shortcut_hint: None,
        }
    }

    /// Create a submenu item.
    pub fn submenu(label: impl Into<String>, children: Vec<MenuItem>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            kind: MenuItemKind::Submenu(children),
            disabled: false,
            shortcut_hint: None,
        }
    }

    /// Builder: set the shortcut hint.
    #[must_use]
    pub fn with_shortcut(mut self, hint: impl Into<String>) -> Self {
        self.shortcut_hint = Some(hint.into());
        self
    }

    /// Builder: set disabled state.
    #[must_use]
    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

// ---------------------------------------------------------------------------
// ContextMenu runtime state
// ---------------------------------------------------------------------------

/// A generic context menu that can be opened at any screen position.
///
/// `ContextMenu` manages visibility, position, items, and hover state.
/// The shell is responsible for translating `MenuAction` into its own
/// action type.
pub struct ContextMenu {
    config: ContextMenuConfig,
    items: Vec<MenuItem>,
    visible: bool,
    position: Point,
    hover_index: Option<usize>,
}

impl ContextMenu {
    /// Create a new context menu with the given items and default config.
    #[must_use]
    pub fn new(items: Vec<MenuItem>) -> Self {
        Self {
            config: ContextMenuConfig::default(),
            items,
            visible: false,
            position: Point::new(0.0, 0.0),
            hover_index: None,
        }
    }

    /// Create a new context menu with custom config.
    #[must_use]
    pub fn with_config(items: Vec<MenuItem>, config: ContextMenuConfig) -> Self {
        Self {
            config,
            items,
            visible: false,
            position: Point::new(0.0, 0.0),
            hover_index: None,
        }
    }

    /// Replace the items in this menu.
    pub fn set_items(&mut self, items: Vec<MenuItem>) {
        self.items = items;
        self.hover_index = None;
    }

    /// Open the menu at the given screen position.
    pub fn open(&mut self, position: Point) {
        self.visible = true;
        self.position = position;
        self.hover_index = None;
    }

    /// Close the menu.
    pub fn close(&mut self) {
        self.visible = false;
        self.hover_index = None;
    }

    /// Toggle visibility at the given position.
    pub fn toggle(&mut self, position: Point) {
        if self.visible {
            self.close();
        } else {
            self.open(position);
        }
    }

    /// Whether the menu is currently visible.
    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// The menu items.
    #[must_use]
    pub fn items(&self) -> &[MenuItem] {
        &self.items
    }

    /// Current hover index.
    #[must_use]
    pub fn hover_index(&self) -> Option<usize> {
        self.hover_index
    }

    /// Compute the bounding rectangle of this menu on screen,
    /// clamped so it stays within `screen`.
    #[must_use]
    pub fn compute_bounds(&self, screen: Rect) -> Rect {
        let w = self.config.width;
        let raw_h = self.config.padding * 2.0 + self.items.len() as f32 * self.config.item_height;
        // Cap height to 80% of screen height so long menus don't overflow.
        let max_h = (screen.height * 0.8).max(100.0);
        let h = raw_h.min(max_h);
        let x = self.position.x.min(screen.width - w - 4.0).max(0.0);
        let y = self.position.y.min(screen.height - h - 4.0).max(0.0);
        Rect::new(x, y, w, h)
    }

    /// Hit-test a mouse position. Returns `Some(index)` if the point
    /// is over a menu item, or `None` if outside or over padding.
    #[must_use]
    pub fn hit_test(&self, screen: Rect, point: Point) -> Option<usize> {
        if !self.visible {
            return None;
        }
        let bounds = self.compute_bounds(screen);
        if !bounds.contains(point) {
            return None;
        }
        let rel_y = point.y - bounds.y - self.config.padding;
        if rel_y < 0.0 {
            return None;
        }
        let idx = (rel_y / self.config.item_height) as usize;
        // Only allow hitting items that are within the visible (clamped) bounds.
        let max_visible = ((bounds.height - self.config.padding * 2.0) / self.config.item_height)
            .floor()
            .max(0.0) as usize;
        let visible_count = self.items.len().min(max_visible);
        if idx < visible_count {
            Some(idx)
        } else {
            None
        }
    }

    /// Update hover state based on mouse position. Returns `true` if
    /// the hover index changed (i.e. a redraw is needed).
    pub fn update_hover(&mut self, screen: Rect, point: Point) -> bool {
        let prev = self.hover_index;
        self.hover_index = self.hit_test(screen, point);
        self.hover_index != prev
    }

    /// Activate the currently hovered item. Returns the `MenuAction` if
    /// the item is not disabled and is an action type.
    #[must_use]
    pub fn activate_hovered(&self) -> Option<MenuAction> {
        let idx = self.hover_index?;
        let item = self.items.get(idx)?;
        if item.disabled {
            return None;
        }
        match &item.kind {
            MenuItemKind::Action(a) => Some(*a),
            MenuItemKind::Toggle { action, .. } => Some(*action),
            MenuItemKind::Submenu(_) => None,
        }
    }

    /// Activate an item at a specific index. Returns the `MenuAction`.
    #[must_use]
    pub fn activate_at(&self, index: usize) -> Option<MenuAction> {
        let item = self.items.get(index)?;
        if item.disabled {
            return None;
        }
        match &item.kind {
            MenuItemKind::Action(a) => Some(*a),
            MenuItemKind::Toggle { action, .. } => Some(*action),
            MenuItemKind::Submenu(_) => None,
        }
    }

    /// Build a scene node tree for this context menu.
    ///
    /// # Parameters
    /// - `screen`: the full screen rect for clamping
    /// - `base_id`: the base scene node ID for this menu
    /// - `z_base`: the base z-order for layering
    /// - `glass_tint`: glass panel tint color
    /// - `text_color`: color for item labels
    /// - `hover_color`: highlight color for hovered items
    /// - `icon_resolver`: function to map icon name → numeric icon ID
    #[must_use]
    pub fn build_scene(
        &self,
        screen: Rect,
        base_id: u64,
        z_base: u32,
        glass_tint: Color,
        text_color: Color,
        hover_color: Color,
        icon_resolver: &dyn Fn(&str) -> u32,
    ) -> Option<SceneNode> {
        if !self.visible {
            return None;
        }

        let bounds = self.compute_bounds(screen);
        let mut panel = SceneNode::new(
            base_id,
            SceneNodeKind::Glass(GlassParams {
                blur_radius: self.config.blur_radius,
                tint_color: glass_tint,
                inner_glow: true,
                parallax: false,
            }),
            NodeProperties::new(bounds).with_z_order(z_base),
        );

        let pad = self.config.padding;
        let item_h = self.config.item_height;
        let item_pad = self.config.item_padding;

        // Only render items that fit within the clamped bounds.
        let max_visible = ((bounds.height - pad * 2.0) / item_h).floor().max(0.0) as usize;
        let visible_items = self.items.len().min(max_visible);

        for (i, item) in self.items.iter().enumerate().take(visible_items) {
            let iy = pad + i as f32 * item_h;

            // Hover highlight (relative to panel)
            if self.hover_index == Some(i) && !item.disabled {
                panel.add_child(SceneNode::new(
                    base_id + 5 + i as u64,
                    SceneNodeKind::Tint {
                        color: hover_color,
                    },
                    NodeProperties::new(Rect::new(4.0, iy, bounds.width - 8.0, item_h))
                        .with_z_order(z_base + 1),
                ));
            }

            // Icon (if present)
            let text_x = if let Some(ref icon_name) = item.icon {
                let icon_id = icon_resolver(icon_name);
                panel.add_child(SceneNode::new(
                    base_id + 10 + i as u64 * 3,
                    SceneNodeKind::Icon {
                        icon_id,
                        color: if item.disabled {
                            Color::new(text_color.r, text_color.g, text_color.b, 100)
                        } else {
                            text_color
                        },
                    },
                    NodeProperties::new(Rect::new(item_pad, iy + 4.0, 24.0, 24.0))
                        .with_z_order(z_base + 2),
                ));
                item_pad + 32.0
            } else {
                item_pad
            };

            // Label
            let label_color = if item.disabled {
                Color::new(text_color.r, text_color.g, text_color.b, 100)
            } else {
                text_color
            };
            panel.add_child(SceneNode::new(
                base_id + 11 + i as u64 * 3,
                SceneNodeKind::Text {
                    text: item.label.clone(),
                    color: label_color,
                    scale: 1,
                    font_family: "Manrope".into(),
                    font_size: 13.0,
                    font_weight: 400,
                    font_style_italic: false,
                    letter_spacing: 0.0,
                    word_spacing: 0.0,
                    line_height: 1.4,
                    text_align: 0,
                    text_transform: 0,
                    text_overflow: 0,
                    white_space: 0,
                    text_indent: 0.0,
                    text_decoration: None,
                    text_shadows: vec![],
                },
                NodeProperties::new(Rect::new(
                    text_x,
                    iy + 6.0,
                    bounds.width - text_x - item_pad,
                    20.0,
                ))
                .with_z_order(z_base + 2),
            ));

            // Shortcut hint (right-aligned)
            if let Some(ref hint) = item.shortcut_hint {
                let hint_color = Color::new(text_color.r, text_color.g, text_color.b, 140);
                panel.add_child(SceneNode::new(
                    base_id + 12 + i as u64 * 3,
                    SceneNodeKind::Text {
                        text: hint.clone(),
                        color: hint_color,
                        scale: 1,
                        font_family: "Manrope".into(),
                        font_size: 12.0,
                        font_weight: 400,
                        font_style_italic: false,
                        letter_spacing: 0.0,
                        word_spacing: 0.0,
                        line_height: 1.4,
                        text_align: 0,
                        text_transform: 0,
                        text_overflow: 0,
                        white_space: 0,
                        text_indent: 0.0,
                        text_decoration: None,
                        text_shadows: vec![],
                    },
                    NodeProperties::new(Rect::new(
                        bounds.width - 80.0,
                        iy + 6.0,
                        72.0,
                        20.0,
                    ))
                    .with_z_order(z_base + 2),
                ));
            }
        }

        Some(panel)
    }
}

impl std::fmt::Display for ContextMenu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ContextMenu({} items, visible={})",
            self.items.len(),
            self.visible,
        )
    }
}
