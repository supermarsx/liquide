//! Scene graph building for the shell's status bar.
//!
//! Converts the runtime `ShellStatusBar` state into a `SceneNode` tree
//! for the compositor, using theme colors and CSS-resolved layout params.

use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::Color;
use liquide_compositor::scene::{GlassParams, NodeProperties, SceneNode, SceneNodeKind};

use crate::items::StatusBarItemKind;
use crate::shell_bar::ShellStatusBar;
use crate::slot::StatusBarSlot;

// ---------------------------------------------------------------------------
// Node ID constants
// ---------------------------------------------------------------------------

/// Base node ID for the status bar root.
pub const NODE_STATUS_BAR: u64 = 1_000;
/// Base node ID for individual status bar items.
pub const NODE_STATUS_BAR_ITEM_BASE: u64 = 1_100;

// ---------------------------------------------------------------------------
// Theme colors
// ---------------------------------------------------------------------------

/// Colors needed to render the status bar scene graph.
///
/// The shell populates this from its `ShellTheme`.
pub struct StatusBarColors {
    /// Glass tint color for the status bar background.
    pub glass_tint: Color,
    /// Accent border at the bottom edge.
    pub border: Color,
    /// Text color for clock and custom items.
    pub text: Color,
    /// Color for a good connection quality indicator.
    pub connected: Color,
    /// Color for a degraded connection quality indicator.
    pub degraded: Color,
    /// Color for the notification indicator when there are unread notifications.
    pub notification_active: Color,
    /// Color for the notification indicator when there are no unread notifications.
    pub notification_inactive: Color,
    /// Color for the system tray area.
    pub tray: Color,
}

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------

/// Layout dimensions for the status bar, typically resolved from CSS.
#[derive(Debug, Clone)]
pub struct StatusBarLayout {
    pub height: f32,
    pub padding: f32,
    pub border_height: f32,
    pub blur_radius: u32,
}

impl Default for StatusBarLayout {
    fn default() -> Self {
        Self {
            height: 34.0,
            padding: 8.0,
            border_height: 2.0,
            blur_radius: 15,
        }
    }
}

// ---------------------------------------------------------------------------
// Scene helpers
// ---------------------------------------------------------------------------

fn solid_rect(id: u64, color: Color, bounds: Rect, z: u32) -> SceneNode {
    SceneNode::new(
        id,
        SceneNodeKind::Background { color },
        NodeProperties::new(bounds).with_z_order(z),
    )
}

fn text_node(
    id: u64,
    text: String,
    color: Color,
    bounds: Rect,
    z: u32,
    scale: u32,
) -> SceneNode {
    SceneNode::new(
        id,
        SceneNodeKind::Text {
            text,
            color,
            scale,
            font_family: "Inter".to_string(),
            font_size: 0.0,
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
        NodeProperties::new(bounds).with_z_order(z),
    )
}

fn icon_node(id: u64, icon_id: u32, color: Color, bounds: Rect, z: u32) -> SceneNode {
    SceneNode::new(
        id,
        SceneNodeKind::Icon { icon_id, color },
        NodeProperties::new(bounds).with_z_order(z),
    )
}

// ---------------------------------------------------------------------------
// build_scene
// ---------------------------------------------------------------------------

impl ShellStatusBar {
    /// Build the scene graph for the status bar.
    pub fn build_scene(
        &self,
        screen: Rect,
        colors: &StatusBarColors,
        layout: Option<&StatusBarLayout>,
    ) -> SceneNode {
        let defaults = StatusBarLayout::default();
        let layout = layout.unwrap_or(&defaults);

        if !self.is_enabled() {
            return SceneNode::new(
                NODE_STATUS_BAR,
                SceneNodeKind::Overlay,
                NodeProperties::new(Rect::ZERO).with_visible(false),
            );
        }

        let bar_bounds = self.compute_bounds(screen);
        let mut bar_node = SceneNode::new(
            NODE_STATUS_BAR,
            SceneNodeKind::Glass(GlassParams {
                blur_radius: layout.blur_radius,
                tint_color: colors.glass_tint,
                inner_glow: false,
                parallax: false,
            }),
            NodeProperties::new(bar_bounds).with_z_order(950),
        );

        // Use parent-relative coordinates for child items so that
        // walk_inner's translation from bar_bounds doesn't double-offset them.
        let padding = layout.padding;
        let item_height = bar_bounds.height - 4.0;
        let item_y = 2.0; // relative to bar top
        let mut left_x = padding;
        let center_x = bar_bounds.width / 2.0;
        let mut right_x = bar_bounds.width - padding;

        // Accent border at the bottom edge of the status bar.
        let border_rect =
            Rect::new(0.0, bar_bounds.height - layout.border_height, bar_bounds.width, layout.border_height);
        bar_node.add_child(solid_rect(
            NODE_STATUS_BAR + 1,
            colors.border,
            border_rect,
            952,
        ));

        for (i, item) in self.items().iter().enumerate() {
            if !item.visible {
                continue;
            }
            let item_id = NODE_STATUS_BAR_ITEM_BASE + i as u64;
            let item_width = match &item.kind {
                StatusBarItemKind::Clock { .. } => 120.0_f32,
                StatusBarItemKind::NotificationIndicator { .. } => 30.0,
                StatusBarItemKind::ConnectionQuality { .. } => 40.0,
                StatusBarItemKind::TrayArea => 80.0,
                StatusBarItemKind::Custom { .. } => 60.0,
                StatusBarItemKind::SessionButton => 28.0,
            };

            let ix = match item.slot {
                StatusBarSlot::Left => {
                    let x = left_x;
                    left_x += item_width + padding;
                    x
                }
                StatusBarSlot::Center => center_x - item_width / 2.0,
                StatusBarSlot::Right => {
                    right_x -= item_width;
                    let x = right_x;
                    right_x -= padding;
                    x
                }
            };

            let item_bounds = Rect::new(ix, item_y, item_width, item_height);
            let color = match &item.kind {
                StatusBarItemKind::Clock { .. } => colors.text,
                StatusBarItemKind::NotificationIndicator { unread_count, .. } => {
                    if *unread_count > 0 {
                        colors.notification_active
                    } else {
                        colors.notification_inactive
                    }
                }
                StatusBarItemKind::ConnectionQuality { quality_percent, .. } => {
                    if *quality_percent > 70 {
                        colors.connected
                    } else {
                        colors.degraded
                    }
                }
                StatusBarItemKind::TrayArea => colors.tray,
                StatusBarItemKind::Custom { .. } => colors.notification_inactive,
                StatusBarItemKind::SessionButton => colors.text,
            };

            match &item.kind {
                StatusBarItemKind::Clock { .. } => {
                    // Format time from the stored UNIX-epoch timestamp.
                    let total_secs = item.last_update_us / 1_000_000;
                    let hours = (total_secs / 3600) % 24;
                    let minutes = (total_secs / 60) % 60;
                    let time_str = format!("{hours:02}:{minutes:02}");
                    bar_node.add_child(text_node(
                        item_id, time_str, color, item_bounds, 951, 1,
                    ));
                }
                StatusBarItemKind::NotificationIndicator { unread_count, .. } => {
                    // Bell icon (icon_id 14 = Notification).
                    bar_node.add_child(icon_node(item_id, 14, color, item_bounds, 951));
                    // Badge count overlay if unread > 0.
                    if *unread_count > 0 {
                        let badge_text = format!("{unread_count}");
                        let badge_w = badge_text.len() as f32 * 8.0 + 4.0;
                        let badge_rect = Rect::new(
                            ix + item_width - badge_w,
                            item_y,
                            badge_w,
                            12.0,
                        );
                        bar_node.add_child(text_node(
                            item_id + 1,
                            badge_text,
                            colors.notification_active,
                            badge_rect,
                            952,
                            1,
                        ));
                    }
                }
                StatusBarItemKind::ConnectionQuality { .. } => {
                    // WiFi icon (icon_id 12 = Wifi).
                    bar_node.add_child(icon_node(item_id, 12, color, item_bounds, 951));
                }
                StatusBarItemKind::TrayArea => {
                    // Battery icon (icon_id 13 = Battery).
                    bar_node.add_child(icon_node(item_id, 13, color, item_bounds, 951));
                }
                StatusBarItemKind::Custom { content, .. } => {
                    bar_node.add_child(text_node(
                        item_id,
                        content.clone(),
                        color,
                        item_bounds,
                        951,
                        1,
                    ));
                }
                StatusBarItemKind::SessionButton => {
                    // Power icon (icon_id 16 = power/shutdown).
                    bar_node.add_child(icon_node(item_id, 16, color, item_bounds, 951));
                }
            }
        }

        bar_node
    }
}
