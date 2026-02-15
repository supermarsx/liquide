//! Painter — walks the layout tree and generates a display list.

use liquide_compositor::pixel::BlendMode;
use liquide_dom::{Document, NodeData};
use liquide_layout::tree::{LayoutBoxId, LayoutTree};
use liquide_style_engine::computed::*;
use liquide_style_engine::StyleMap;

use crate::display_list::{BorderEdge, DisplayItem, DisplayList};

/// The painter walks the layout tree and emits paint commands.
pub struct Painter;

impl Painter {
    pub fn new() -> Self {
        Self
    }

    /// Paint the entire layout tree into a display list.
    pub fn paint(
        &self,
        doc: &Document,
        layout: &LayoutTree,
        styles: &StyleMap,
    ) -> DisplayList {
        let mut list = DisplayList::new();
        self.paint_box(doc, layout, styles, layout.root, &mut list);
        list
    }

    fn paint_box(
        &self,
        doc: &Document,
        layout: &LayoutTree,
        styles: &StyleMap,
        box_id: LayoutBoxId,
        list: &mut DisplayList,
    ) {
        let layout_box = match layout.get(box_id) {
            Some(b) => b,
            None => return,
        };

        let style = styles.get(layout_box.node).cloned().unwrap_or_default();

        // Skip invisible elements
        if !style.is_visible() {
            return;
        }

        // Push stacking context if needed
        let needs_sc = style.creates_stacking_context();
        if needs_sc {
            list.push(DisplayItem::PushStackingContext {
                z_index: style.z_index.unwrap_or(0),
                isolation: style.isolation,
            });
        }

        // Push opacity
        if style.opacity < 1.0 {
            list.push(DisplayItem::PushOpacity {
                opacity: style.opacity,
            });
        }

        // Push transform
        if !style.transform.is_empty() {
            let (tx, ty, sx, sy, r) = flatten_transforms(&style.transform);
            list.push(DisplayItem::PushTransform {
                translate_x: tx,
                translate_y: ty,
                scale_x: sx,
                scale_y: sy,
                rotate: r,
            });
        }

        // Push blend mode
        if style.mix_blend_mode != BlendMode::SrcOver {
            list.push(DisplayItem::PushBlendMode {
                mode: style.mix_blend_mode,
            });
        }

        // Push clipping for overflow
        let needs_clip = matches!(
            style.overflow_x,
            liquide_compositor::scene::Overflow::Hidden | liquide_compositor::scene::Overflow::Scroll
        ) || matches!(
            style.overflow_y,
            liquide_compositor::scene::Overflow::Hidden | liquide_compositor::scene::Overflow::Scroll
        );

        if needs_clip {
            list.push(DisplayItem::PushClip {
                rect: layout_box.padding_rect,
                radius: style.border_radius.clone(),
            });
        }

        // Paint box shadows (outer, before background)
        for shadow in &style.box_shadow {
            list.push(DisplayItem::BoxShadow {
                rect: layout_box.border_rect,
                offset_x: shadow.offset_x,
                offset_y: shadow.offset_y,
                blur_radius: shadow.blur_radius,
                spread_radius: shadow.spread_radius,
                color: shadow.color,
                inset: shadow.inset,
                radius: style.border_radius.clone(),
            });
        }

        // Paint background
        let bg = style.background_color;
        if bg.a > 0 {
            list.push(DisplayItem::SolidColor {
                rect: layout_box.padding_rect,
                color: bg,
                radius: style.border_radius.clone(),
            });
        }

        // Paint border
        let has_border = style.border_width.top > 0.0
            || style.border_width.right > 0.0
            || style.border_width.bottom > 0.0
            || style.border_width.left > 0.0;

        if has_border {
            list.push(DisplayItem::Border {
                rect: layout_box.border_rect,
                top: BorderEdge {
                    width: style.border_width.top,
                    style: style.border_style.top,
                    color: style.border_color.top,
                },
                right: BorderEdge {
                    width: style.border_width.right,
                    style: style.border_style.right,
                    color: style.border_color.right,
                },
                bottom: BorderEdge {
                    width: style.border_width.bottom,
                    style: style.border_style.bottom,
                    color: style.border_color.bottom,
                },
                left: BorderEdge {
                    width: style.border_width.left,
                    style: style.border_style.left,
                    color: style.border_color.left,
                },
                radius: style.border_radius.clone(),
            });
        }

        // Paint text content
        if let Some(node) = doc.get(layout_box.node) {
            match &node.data {
                NodeData::Text(text) => {
                    list.push(DisplayItem::Text {
                        rect: layout_box.content_rect,
                        text: text.clone(),
                        color: style.color,
                        font_size: style.font_size,
                        font_family: style.font_family.clone(),
                        font_weight: style.font_weight,
                        font_style: style.font_style.clone(),
                        letter_spacing: style.letter_spacing,
                        word_spacing: style.word_spacing,
                        line_height: style.line_height.clone(),
                        text_align: style.text_align,
                        text_transform: style.text_transform,
                        text_overflow: style.text_overflow,
                        white_space: style.white_space,
                        word_break: style.word_break,
                        text_indent: style.text_indent,
                        text_decoration: style.text_decoration.clone(),
                        text_shadows: style.text_shadow.clone(),
                    });
                }
                NodeData::Image { src, .. } => {
                    list.push(DisplayItem::Image {
                        rect: layout_box.content_rect,
                        src: src.clone(),
                        radius: style.border_radius.clone(),
                    });
                }
                NodeData::Surface { surface_id } => {
                    list.push(DisplayItem::Surface {
                        rect: layout_box.content_rect,
                        surface_id: *surface_id,
                    });
                }
                _ => {}
            }
        }

        // Paint outline (after content, outside border)
        if let Some(ref outline) = style.outline {
            // Paint outline as a border with offset
            list.push(DisplayItem::Border {
                rect: liquide_layout::Rect::new(
                    layout_box.border_rect.x - outline.width,
                    layout_box.border_rect.y - outline.width,
                    layout_box.border_rect.width + outline.width * 2.0,
                    layout_box.border_rect.height + outline.width * 2.0,
                ),
                top: BorderEdge {
                    width: outline.width,
                    style: BorderLineStyle::Solid,
                    color: outline.color,
                },
                right: BorderEdge {
                    width: outline.width,
                    style: BorderLineStyle::Solid,
                    color: outline.color,
                },
                bottom: BorderEdge {
                    width: outline.width,
                    style: BorderLineStyle::Solid,
                    color: outline.color,
                },
                left: BorderEdge {
                    width: outline.width,
                    style: BorderLineStyle::Solid,
                    color: outline.color,
                },
                radius: style.border_radius.clone(),
            });
        }

        // Paint children
        // Collect and sort by z-index for proper stacking order
        let children = layout_box.children.clone();
        let mut sorted_children: Vec<(LayoutBoxId, i32)> = children
            .iter()
            .map(|&child_id| {
                let z = layout
                    .get(child_id)
                    .and_then(|cb| styles.get(cb.node))
                    .and_then(|s| s.z_index)
                    .unwrap_or(0);
                (child_id, z)
            })
            .collect();
        sorted_children.sort_by_key(|&(_, z)| z);

        for (child_id, _) in sorted_children {
            self.paint_box(doc, layout, styles, child_id, list);
        }

        // Pop state in reverse order
        if needs_clip {
            list.push(DisplayItem::PopClip);
        }
        if style.mix_blend_mode != BlendMode::SrcOver {
            list.push(DisplayItem::PopBlendMode);
        }
        if !style.transform.is_empty() {
            list.push(DisplayItem::PopTransform);
        }
        if style.opacity < 1.0 {
            list.push(DisplayItem::PopOpacity);
        }
        if needs_sc {
            list.push(DisplayItem::PopStackingContext);
        }
    }
}

impl Default for Painter {
    fn default() -> Self {
        Self::new()
    }
}

/// Flatten a list of transforms into (translate_x, translate_y, scale_x, scale_y, rotate).
fn flatten_transforms(transforms: &[Transform]) -> (f32, f32, f32, f32, f32) {
    let mut tx = 0.0f32;
    let mut ty = 0.0f32;
    let mut sx = 1.0f32;
    let mut sy = 1.0f32;
    let mut r = 0.0f32;

    for t in transforms {
        match t {
            Transform::Translate(x, y) => {
                tx += x;
                ty += y;
            }
            Transform::Scale(x, y) => {
                sx *= x;
                sy *= y;
            }
            Transform::Rotate(deg) => {
                r += deg;
            }
            Transform::Skew(_, _) => {
                // Simplified: skip skew
            }
            Transform::Matrix(_a, _b, _c, _d, e, f) => {
                // Simplified: extract translate from affine matrix
                tx += e;
                ty += f;
            }
        }
    }

    (tx, ty, sx, sy, r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_dom::Document;
    use liquide_layout::{DefaultTextMeasurer, DefaultImageMeasurer, LayoutEngine, Size};
    use liquide_style_engine::engine::{StyleEngine, ViewportSize};

    #[test]
    fn basic_paint() {
        let mut doc = Document::new();
        let root = doc.root();
        let div = doc.create_element("div");
        doc.append_child(root, div);

        let mut se = StyleEngine::default();
        se.add_stylesheet("div { background-color: red; width: 100px; height: 50px; }");

        let style_map = se.restyle_all(&doc);
        let mut le = LayoutEngine::new(Size::new(1920.0, 1080.0), 16.0);
        let layout_tree = le.layout(&doc, &style_map, &DefaultTextMeasurer, &DefaultImageMeasurer);

        let painter = Painter::new();
        let display_list = painter.paint(&doc, &layout_tree, &style_map);

        assert!(!display_list.is_empty(), "Display list should have paint commands");
    }
}
