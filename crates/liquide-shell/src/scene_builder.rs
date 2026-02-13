//! Scene node ID allocation and shell scene building helpers.

use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::Color;
use liquide_compositor::scene::{GlassParams, NodeProperties, SceneNode, SceneNodeKind};

// Node ID ranges — each subsystem gets a reserved range.
pub const NODE_ROOT: u64 = 0;
pub const NODE_BACKGROUND: u64 = 1;
pub const NODE_WORKSPACE_BASE: u64 = 100;
pub const NODE_STATUS_BAR: u64 = 1_000;
pub const NODE_STATUS_BAR_ITEM_BASE: u64 = 1_100;
pub const NODE_DOCK: u64 = 2_000;
pub const NODE_DOCK_ITEM_BASE: u64 = 2_100;
pub const NODE_WINDOW_BASE: u64 = 10_000;
pub const NODE_WINDOW_STRIDE: u64 = 10;
pub const NODE_NOTIFICATION_BASE: u64 = 100_000;
pub const NODE_LAUNCHER: u64 = 200_000;
pub const NODE_CURSOR: u64 = 999_999;

/// Helper to create a simple solid-color rectangle node.
pub fn solid_rect(id: u64, color: Color, bounds: Rect, z: u32) -> SceneNode {
    SceneNode::new(
        id,
        SceneNodeKind::Background { color },
        NodeProperties::new(bounds).with_z_order(z),
    )
}

/// Helper to create a Glass panel node.
pub fn glass_panel(id: u64, tint: Color, bounds: Rect, z: u32) -> SceneNode {
    SceneNode::new(
        id,
        SceneNodeKind::Glass(GlassParams {
            blur_radius: 20,
            tint_color: tint,
            inner_glow: true,
            parallax: false,
        }),
        NodeProperties::new(bounds).with_z_order(z),
    )
}

/// Helper to create a tint overlay node.
pub fn tint_overlay(id: u64, color: Color, bounds: Rect, z: u32) -> SceneNode {
    SceneNode::new(
        id,
        SceneNodeKind::Tint { color },
        NodeProperties::new(bounds).with_z_order(z),
    )
}
