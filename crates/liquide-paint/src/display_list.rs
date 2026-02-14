//! Display list — a flat list of paint commands.

use liquide_compositor::pixel::{BlendMode, Color};
use liquide_layout::Rect;
use liquide_style_engine::computed::{BorderLineStyle, Isolation};
use liquide_style_engine::dimension::Corners;

use serde::{Deserialize, Serialize};

/// A single paint command.
#[derive(Debug, Clone)]
pub enum DisplayItem {
    // ── Backgrounds ──
    SolidColor {
        rect: Rect,
        color: Color,
        radius: Corners<f32>,
    },

    // ── Borders ──
    Border {
        rect: Rect,
        top: BorderEdge,
        right: BorderEdge,
        bottom: BorderEdge,
        left: BorderEdge,
        radius: Corners<f32>,
    },

    // ── Shadows ──
    BoxShadow {
        rect: Rect,
        offset_x: f32,
        offset_y: f32,
        blur_radius: f32,
        spread_radius: f32,
        color: Color,
        inset: bool,
        radius: Corners<f32>,
    },

    // ── Text ──
    Text {
        rect: Rect,
        text: String,
        color: Color,
        font_size: f32,
        font_family: Vec<String>,
        font_weight: u16,
        text_decoration: Option<liquide_compositor::scene::TextDecoration>,
        text_shadows: Vec<liquide_compositor::scene::TextShadow>,
    },

    // ── Images ──
    Image {
        rect: Rect,
        src: String,
        radius: Corners<f32>,
    },

    // ── Clip ──
    PushClip {
        rect: Rect,
        radius: Corners<f32>,
    },
    PopClip,

    // ── Opacity ──
    PushOpacity {
        opacity: f32,
    },
    PopOpacity,

    // ── Transform ──
    PushTransform {
        translate_x: f32,
        translate_y: f32,
        scale_x: f32,
        scale_y: f32,
        rotate: f32,
    },
    PopTransform,

    // ── Blend mode ──
    PushBlendMode {
        mode: BlendMode,
    },
    PopBlendMode,

    // ── Stacking context ──
    PushStackingContext {
        z_index: i32,
        isolation: Isolation,
    },
    PopStackingContext,

    // ── External surface (sandboxed app) ──
    Surface {
        rect: Rect,
        surface_id: u64,
    },
}

/// A border edge for painting.
#[derive(Debug, Clone)]
pub struct BorderEdge {
    pub width: f32,
    pub style: BorderLineStyle,
    pub color: Color,
}

impl Default for BorderEdge {
    fn default() -> Self {
        Self {
            width: 0.0,
            style: BorderLineStyle::None,
            color: Color { r: 0, g: 0, b: 0, a: 0 },
        }
    }
}

/// An ordered list of paint commands.
#[derive(Debug, Clone)]
pub struct DisplayList {
    pub items: Vec<DisplayItem>,
}

impl DisplayList {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn push(&mut self, item: DisplayItem) {
        self.items.push(item);
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }
}

impl Default for DisplayList {
    fn default() -> Self {
        Self::new()
    }
}
