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
use liquide_compositor::scene::{GlassParams, NodeProperties, SceneNode, SceneNodeKind};

use crate::color::UiColor;
use crate::painter::PaintCommand;
use crate::text::{SimpleTextMeasure, TextMeasure};

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
    pub fn convert(
        &mut self,
        commands: &[PaintCommand],
        parent_x: f32,
        parent_y: f32,
    ) -> Vec<SceneNode> {
        let mut nodes = Vec::with_capacity(commands.len());
        let mut z = self.z_base;
        let mut clip_stack: Vec<Rect> = Vec::new();

        for cmd in commands {
            match cmd {
                PaintCommand::PushClip { x, y, w, h } => {
                    let next = Rect::new(x + parent_x, y + parent_y, *w, *h);
                    let clip = clip_stack
                        .last()
                        .copied()
                        .map(|current| intersect_rect(current, next))
                        .unwrap_or(next);
                    clip_stack.push(clip);
                }
                PaintCommand::PopClip => {
                    clip_stack.pop();
                }
                _ => {
                    if let Some(node) =
                        self.convert_command(cmd, parent_x, parent_y, z, clip_stack.last().copied())
                    {
                        nodes.push(node);
                        z += 1;
                    }
                }
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
        active_clip: Option<Rect>,
    ) -> Option<SceneNode> {
        match cmd {
            PaintCommand::FillRect { x, y, w, h, color } => {
                let bounds = Rect::new(x + offset_x, y + offset_y, *w, *h);
                Some(SceneNode::new(
                    self.alloc_id(),
                    SceneNodeKind::Background {
                        color: ui_to_scene_color(color),
                    },
                    apply_clip(NodeProperties::new(bounds).with_z_order(z), active_clip),
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
                    apply_clip(NodeProperties::new(bounds).with_z_order(z), active_clip),
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
                let measurer = SimpleTextMeasure;
                let (width, height) = measurer.measure_text(text, *size, *bold);
                // Map font size to scale factor for bitmap fallback (base = 16px).
                let scale = (*size / 16.0).max(1.0).round() as u32;
                let weight = if *bold { 700_u16 } else { 400 };
                let bounds = Rect::new(x + offset_x, y + offset_y, width, height);
                Some(SceneNode::new(
                    self.alloc_id(),
                    SceneNodeKind::Text {
                        text: text.clone(),
                        color: ui_to_scene_color(color),
                        scale,
                        font_family: font_family.clone(),
                        font_size: *size,
                        font_weight: weight,
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
                    apply_clip(NodeProperties::new(bounds).with_z_order(z), active_clip),
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
                    apply_clip(NodeProperties::new(bounds).with_z_order(z), active_clip),
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
                    apply_clip(NodeProperties::new(bounds).with_z_order(z), active_clip),
                ))
            }

            PaintCommand::FillCircle { cx, cy, r, color } => {
                let bounds = Rect::new(cx - r + offset_x, cy - r + offset_y, r * 2.0, r * 2.0);
                // Approximate circle as a Glass node with full corner radius.
                Some(SceneNode::new(
                    self.alloc_id(),
                    SceneNodeKind::Glass(GlassParams {
                        blur_radius: 0,
                        tint_color: ui_to_scene_color(color),
                        inner_glow: false,
                        parallax: false,
                    }),
                    apply_clip(NodeProperties::new(bounds).with_z_order(z), active_clip),
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
                    apply_clip(NodeProperties::new(bounds).with_z_order(z), active_clip),
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
                    apply_clip(NodeProperties::new(bounds).with_z_order(z), active_clip),
                ))
            }

            PaintCommand::PushClip { .. } | PaintCommand::PopClip => None,
        }
    }
}

fn apply_clip(properties: NodeProperties, clip: Option<Rect>) -> NodeProperties {
    if let Some(clip_rect) = clip {
        properties.with_clip(clip_rect)
    } else {
        properties
    }
}

fn intersect_rect(a: Rect, b: Rect) -> Rect {
    let left = a.x.max(b.x);
    let top = a.y.max(b.y);
    let right = a.right().min(b.right());
    let bottom = a.bottom().min(b.bottom());
    Rect::new(left, top, (right - left).max(0.0), (bottom - top).max(0.0))
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

    #[test]
    fn test_nested_clips_are_propagated_to_scene_nodes() {
        let mut painter = Painter::new();
        painter.push_clip(0.0, 0.0, 50.0, 50.0);
        painter.push_clip(25.0, 25.0, 50.0, 50.0);
        painter.fill_rect(10.0, 10.0, 100.0, 100.0, UiColor::white());
        painter.pop_clip();
        painter.pop_clip();

        let mut bridge = SceneBridge::new(6000, 0);
        let nodes = bridge.convert(painter.commands(), 0.0, 0.0);
        assert_eq!(nodes.len(), 1);

        let clip = nodes[0].properties.clip.expect("clip should be preserved");
        assert_eq!(clip.x, 25.0);
        assert_eq!(clip.y, 25.0);
        assert_eq!(clip.width, 25.0);
        assert_eq!(clip.height, 25.0);
    }
}
