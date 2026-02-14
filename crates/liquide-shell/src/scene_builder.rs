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
pub const NODE_SESSION_MENU: u64 = 300_000;
pub const NODE_APP_MENU: u64 = 350_000;
pub const NODE_CONTEXT_MENU: u64 = 400_000;
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

/// Helper to create a text label node (bitmap fallback).
pub fn text_node(
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
            font_family: String::new(),
            font_size: 0.0,
            font_weight: 400,
            letter_spacing: 0.0,
            line_height: 1.4,
        },
        NodeProperties::new(bounds).with_z_order(z),
    )
}

/// Helper to create a text label node with real font info.
pub fn rich_text_node(
    id: u64,
    text: String,
    color: Color,
    bounds: Rect,
    z: u32,
    font_family: &str,
    font_size: f32,
    font_weight: u16,
    letter_spacing: f32,
    line_height: f32,
) -> SceneNode {
    SceneNode::new(
        id,
        SceneNodeKind::Text {
            text,
            color,
            scale: 1,
            font_family: font_family.to_string(),
            font_size,
            font_weight,
            letter_spacing,
            line_height,
        },
        NodeProperties::new(bounds).with_z_order(z),
    )
}

/// Helper to create a built-in icon node.
pub fn icon_node(id: u64, icon_id: u32, color: Color, bounds: Rect, z: u32) -> SceneNode {
    SceneNode::new(
        id,
        SceneNodeKind::Icon { icon_id, color },
        NodeProperties::new(bounds).with_z_order(z),
    )
}

/// Map a named icon string to the numeric icon ID used by the renderer.
///
/// Returns 0 for unrecognised names (the renderer draws a filled rect
/// as a fallback for unknown IDs).
pub fn icon_id_for_name(name: &str) -> u32 {
    match name {
        "folder" | "file-manager" => 1,
        "terminal" | "console" | "utilities-terminal" => 2,
        "web-browser" | "browser" | "internet-web-browser" => 3,
        "preferences-system" | "settings" | "system-preferences" => 4,
        "calculator" | "accessories-calculator" => 5,
        "text-editor" | "accessories-text-editor" => 6,
        "audio-x-generic" | "music" | "multimedia-audio-player" => 7,
        "camera" | "camera-photo" => 8,
        "mail" | "internet-mail" => 9,
        "calendar" | "office-calendar" => 10,
        "clock" | "preferences-clock" => 11,
        "network-wireless" | "wifi" => 12,
        "battery" | "battery-full" => 13,
        "notification" | "preferences-desktop-notification" => 14,
        "search" | "system-search" | "edit-find" => 15,
        "power" | "system-shutdown" => 16,
        "audio-volume-high" | "volume" => 17,
        "user-trash" | "trash" => 18,
        _ => 0,
    }
}
