//! Widget → Scene graph bridge.
//!
//! Converts `PaintCommand`s produced by the widget tree into `SceneNode`s
//! that the compositor can render. This bridges the gap between the
//! retained-mode UI toolkit and the compositor's scene graph.
//!
//! # Pipeline
//!
//! ```text
//! Widget::paint() → Painter → Vec<PaintCommand>
//!                                │
//!                      SceneBridge::convert()
//!                                │
//!                                ▼
//!                     Vec<SceneNode> → Compositor
//! ```

use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::Color;
use liquide_compositor::scene::{
    GlassParams, NodeProperties, SceneNode, SceneNodeKind,
};

use crate::color::UiColor;
use crate::painter::PaintCommand;

/// Converts widget paint commands to compositor scene nodes.
pub struct SceneBridge {
    /// Base node ID for generating unique IDs.
    next_id: u64,
    /// Base Z-order offset.
    z_base: u32,
}

impl SceneBridge {
    /// Create a new bridge starting at the given base ID.
    #[must_use]
    pub fn new(base_id: u64, z_base: u32) -> Self {
        Self {
            next_id: base_id,
            z_base,
        }
    }

    fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Convert a list of paint commands into scene nodes.
    pub fn convert(&mut self, commands: &[PaintCommand], parent_x: f32, parent_y: f32) -> Vec<SceneNode> {
        let mut nodes = Vec::with_capacity(commands.len());
        let mut z = self.z_base;

        for cmd in commands {
            if let Some(node) = self.convert_command(cmd, parent_x, parent_y, z) {
                nodes.push(node);
                z += 1;
            }
        }

        nodes
    }

    /// Convert a single paint command to a scene node.
    fn convert_command(
        &mut self,
        cmd: &PaintCommand,
        offset_x: f32,
        offset_y: f32,
        z: u32,
    ) -> Option<SceneNode> {
        match cmd {
            PaintCommand::FillRect {
                x,
                y,
                w,
                h,
                color,
            } => {
                let bounds = Rect::new(x + offset_x, y + offset_y, *w, *h);
                Some(SceneNode::new(
                    self.alloc_id(),
                    SceneNodeKind::Background {
                        color: ui_to_scene_color(color),
                    },
                    NodeProperties::new(bounds).with_z_order(z),
                ))
            }

            PaintCommand::FillRoundedRect {
                x,
                y,
                w,
                h,
                radius: _radius,
                color,
            } => {
                let bounds = Rect::new(x + offset_x, y + offset_y, *w, *h);
                // Use Glass node for rounded rects.
                Some(SceneNode::new(
                    self.alloc_id(),
                    SceneNodeKind::Glass(GlassParams {
                        blur_radius: 0,
                        tint_color: ui_to_scene_color(color),
                        inner_glow: false,
                        parallax: false,
                    }),
                    NodeProperties::new(bounds).with_z_order(z),
                ))
            }

            PaintCommand::DrawText {
                text,
                x,
                y,
                size,
                color,
                font_family,
                bold,
            } => {
                // Map font size to scale factor for bitmap fallback (base = 16px).
                let scale = (*size / 16.0).max(1.0).round() as u32;
                let weight = if *bold { 700_u16 } else { 400 };
                let bounds = Rect::new(
                    x + offset_x,
                    y + offset_y,
                    text.len() as f32 * size * 0.55, // Approximate width.
                    *size * 1.2, // Approximate height.
                );
                Some(SceneNode::new(
                    self.alloc_id(),
                    SceneNodeKind::Text {
                        text: text.clone(),
                        color: ui_to_scene_color(color),
                        scale,
                        font_family: font_family.clone(),
                        font_size: *size,
                        font_weight: weight,
                        letter_spacing: 0.0,
                        line_height: 1.4,
                        text_decoration: None,
                        text_shadows: vec![],
                    },
                    NodeProperties::new(bounds).with_z_order(z),
                ))
            }

            PaintCommand::DrawIcon {
                icon_id,
                x,
                y,
                size,
                color,
            } => {
                let bounds = Rect::new(x + offset_x, y + offset_y, *size, *size);
                Some(SceneNode::new(
                    self.alloc_id(),
                    SceneNodeKind::Icon {
                        icon_id: *icon_id,
                        color: ui_to_scene_color(color),
                    },
                    NodeProperties::new(bounds).with_z_order(z),
                ))
            }

            PaintCommand::DrawLine {
                x1,
                y1,
                x2,
                y2,
                color,
                width: line_width,
            } => {
                // Represent a line as a thin filled rect.
                let min_x = x1.min(*x2);
                let min_y = y1.min(*y2);
                let w = (x2 - x1).abs().max(*line_width);
                let h = (y2 - y1).abs().max(*line_width);
                let bounds = Rect::new(min_x + offset_x, min_y + offset_y, w, h);
                Some(SceneNode::new(
                    self.alloc_id(),
                    SceneNodeKind::Background {
                        color: ui_to_scene_color(color),
                    },
                    NodeProperties::new(bounds).with_z_order(z),
                ))
            }

            PaintCommand::FillCircle {
                cx,
                cy,
                r,
                color,
            } => {
                let bounds = Rect::new(
                    cx - r + offset_x,
                    cy - r + offset_y,
                    r * 2.0,
                    r * 2.0,
                );
                // Approximate circle as a Glass node with full corner radius.
                Some(SceneNode::new(
                    self.alloc_id(),
                    SceneNodeKind::Glass(GlassParams {
                        blur_radius: 0,
                        tint_color: ui_to_scene_color(color),
                        inner_glow: false,
                        parallax: false,
                    }),
                    NodeProperties::new(bounds).with_z_order(z),
                ))
            }

            PaintCommand::StrokeRect {
                x,
                y,
                w,
                h,
                color,
                width: line_width,
            } => {
                // Stroke as a decoration with border only.
                let bounds = Rect::new(x + offset_x, y + offset_y, *w, *h);
                let c = ui_to_scene_color(color);
                Some(SceneNode::new(
                    self.alloc_id(),
                    SceneNodeKind::Decoration {
                        title: None,
                        title_color: Color::new(0, 0, 0, 0),
                        background: Color::new(0, 0, 0, 0),
                        border_color: c,
                        border_width: *line_width,
                        corner_radius: 0.0,
                        button_state: Default::default(),
                        button_colors: Default::default(),
                        button_layout: Default::default(),
                    },
                    NodeProperties::new(bounds).with_z_order(z),
                ))
            }

            PaintCommand::StrokeRoundedRect {
                x,
                y,
                w,
                h,
                radius,
                color,
                width: line_width,
            } => {
                let bounds = Rect::new(x + offset_x, y + offset_y, *w, *h);
                let c = ui_to_scene_color(color);
                Some(SceneNode::new(
                    self.alloc_id(),
                    SceneNodeKind::Decoration {
                        title: None,
                        title_color: Color::new(0, 0, 0, 0),
                        background: Color::new(0, 0, 0, 0),
                        border_color: c,
                        border_width: *line_width,
                        corner_radius: *radius,
                        button_state: Default::default(),
                        button_colors: Default::default(),
                        button_layout: Default::default(),
                    },
                    NodeProperties::new(bounds).with_z_order(z),
                ))
            }

            PaintCommand::PushClip { .. } | PaintCommand::PopClip => {
                // Clips are handled at a higher level via SceneNode clip properties.
                None
            }
        }
    }
}

/// Convert a UI color to a compositor Color.
fn ui_to_scene_color(c: &UiColor) -> Color {
    Color::new(c.r, c.g, c.b, c.a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::UiColor;
    use crate::painter::Painter;

    #[test]
    fn test_convert_fill_rect() {
        let mut bridge = SceneBridge::new(1000, 0);
        let cmds = vec![PaintCommand::FillRect {
            x: 10.0,
            y: 20.0,
            w: 100.0,
            h: 50.0,
            color: UiColor::new(255, 0, 0, 255),
        }];
        let nodes = bridge.convert(&cmds, 0.0, 0.0);
        assert_eq!(nodes.len(), 1);
        assert!(matches!(nodes[0].kind, SceneNodeKind::Background { .. }));
    }

    #[test]
    fn test_convert_text() {
        let mut bridge = SceneBridge::new(2000, 10);
        let cmds = vec![PaintCommand::DrawText {
            text: "Hello".into(),
            x: 5.0,
            y: 10.0,
            size: 14.0,
            color: UiColor::new(255, 255, 255, 255),
            font_family: "Manrope".into(),
            bold: false,
        }];
        let nodes = bridge.convert(&cmds, 0.0, 0.0);
        assert_eq!(nodes.len(), 1);
        assert!(matches!(nodes[0].kind, SceneNodeKind::Text { .. }));
    }

    #[test]
    fn test_painter_to_scene() {
        let mut painter = Painter::new();
        painter.fill_rect(0.0, 0.0, 200.0, 100.0, UiColor::new(30, 30, 40, 255));
        painter.draw_text("Test", 10.0, 10.0, 14.0, UiColor::white(), "Inter", false);

        let mut bridge = SceneBridge::new(5000, 0);
        let nodes = bridge.convert(painter.commands(), 0.0, 0.0);
        assert_eq!(nodes.len(), 2);
    }
}
