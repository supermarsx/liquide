//! Cursor shape drawing for the software renderer.

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::{BlendMode, Color};
use liquide_compositor::scene::{CursorShape, FlatNode, ResizeDirection};

use crate::rasterizer;

use super::SoftwareRenderer;

impl SoftwareRenderer {
    /// Render a cursor node.
    pub(crate) fn render_cursor_node(&mut self, node: &FlatNode, fb: &mut FrameBuffer) {
        let bounds = node.absolute_bounds;
        if let liquide_compositor::scene::SceneNodeKind::Cursor { shape } = node.kind_ref() {
            let cx = bounds.x;
            let cy = bounds.y;
            let s = (bounds.width / 16.0).max(1.0);

            let outline = Color::new(0, 0, 0, 255);
            let fill = Color::WHITE;

            match shape {
                CursorShape::Arrow => {
                    Self::draw_cursor_arrow(fb, cx, cy, s, outline, fill);
                }
                CursorShape::Move => {
                    Self::draw_cursor_move(fb, cx, cy, s, outline, fill);
                }
                CursorShape::Resize(dir) => {
                    use ResizeDirection::*;
                    match dir {
                        North | South => {
                            Self::draw_cursor_resize_ns(fb, cx, cy, s, outline, fill);
                        }
                        East | West => {
                            Self::draw_cursor_resize_ew(fb, cx, cy, s, outline, fill);
                        }
                        NorthWest | SouthEast => {
                            Self::draw_cursor_resize_nwse(fb, cx, cy, s, outline, fill);
                        }
                        NorthEast | SouthWest => {
                            Self::draw_cursor_resize_nesw(fb, cx, cy, s, outline, fill);
                        }
                    }
                }
                CursorShape::Pointer => {
                    Self::draw_cursor_pointer(fb, cx, cy, s, outline, fill);
                }
                CursorShape::Text => {
                    Self::draw_cursor_text(fb, cx, cy, s, outline, fill);
                }
                CursorShape::NotAllowed => {
                    Self::draw_cursor_not_allowed(fb, cx, cy, s, outline, fill);
                }
                CursorShape::Wait => {
                    Self::draw_cursor_wait(fb, cx, cy, s, outline, fill);
                }
                CursorShape::Progress => {
                    // Arrow + small hourglass
                    Self::draw_cursor_arrow(fb, cx, cy, s, outline, fill);
                    Self::draw_cursor_wait(fb, cx + 8.0 * s, cy + 8.0 * s, s * 0.6, outline, fill);
                }
                CursorShape::Help => {
                    // Arrow + question mark
                    Self::draw_cursor_arrow(fb, cx, cy, s, outline, fill);
                    Self::draw_question_mark(fb, cx + 10.0 * s, cy + 10.0 * s, s * 0.7, outline);
                }
                CursorShape::Crosshair => {
                    Self::draw_cursor_crosshair(fb, cx, cy, s, outline);
                }
                CursorShape::Grab => {
                    Self::draw_cursor_hand(fb, cx, cy, s, outline, fill, false);
                }
                CursorShape::Grabbing => {
                    Self::draw_cursor_hand(fb, cx, cy, s, outline, fill, true);
                }
                CursorShape::ZoomIn => {
                    Self::draw_cursor_magnifier(fb, cx, cy, s, outline, fill, true);
                }
                CursorShape::ZoomOut => {
                    Self::draw_cursor_magnifier(fb, cx, cy, s, outline, fill, false);
                }
                CursorShape::ContextMenu => {
                    Self::draw_cursor_pointer(fb, cx, cy, s, outline, fill);
                }
                CursorShape::Alias => {
                    Self::draw_cursor_arrow(fb, cx, cy, s, outline, fill);
                }
                CursorShape::Copy => {
                    Self::draw_cursor_arrow(fb, cx, cy, s, outline, fill);
                }
                CursorShape::NoDrop => {
                    Self::draw_cursor_not_allowed(fb, cx, cy, s, outline, fill);
                }
                CursorShape::Cell => {
                    Self::draw_cursor_crosshair(fb, cx, cy, s, outline);
                }
                CursorShape::VerticalText => {
                    Self::draw_cursor_text_vertical(fb, cx, cy, s, outline, fill);
                }
                CursorShape::AllScroll => {
                    Self::draw_cursor_all_scroll(fb, cx, cy, s, outline, fill);
                }
                CursorShape::ColResize => {
                    Self::draw_cursor_resize_ew(fb, cx, cy, s, outline, fill);
                }
                CursorShape::RowResize => {
                    Self::draw_cursor_resize_ns(fb, cx, cy, s, outline, fill);
                }
                CursorShape::Custom { .. } | CursorShape::Hidden => {
                    // Custom cursors handled elsewhere, Hidden means don't draw
                }
            }
        }
    }

    // =======================================================================
    // Cursor shape drawing helpers
    // =======================================================================

    /// Arrow cursor: classic top-left pointer.
    fn draw_cursor_arrow(
        fb: &mut FrameBuffer,
        cx: f32,
        cy: f32,
        s: f32,
        outline: Color,
        fill: Color,
    ) {
        let arrow_rows: &[(f32, f32)] = &[
            (0.0, 1.0),
            (1.0, 2.0),
            (2.0, 3.0),
            (3.0, 4.0),
            (4.0, 5.0),
            (5.0, 6.0),
            (6.0, 7.0),
            (7.0, 8.0),
            (8.0, 9.0),
            (9.0, 10.0),
            (10.0, 11.0),
            (11.0, 12.0),
            (12.0, 7.0),
            (13.0, 5.0),
        ];
        for &(row_y, row_w) in arrow_rows {
            rasterizer::fill_rect(
                fb,
                Rect::new(
                    cx - s,
                    cy + row_y * s - 0.5 * s,
                    row_w * s + 2.0 * s,
                    2.0 * s,
                ),
                outline,
                BlendMode::SrcOver,
            );
        }
        for &(row_y, row_w) in arrow_rows {
            rasterizer::fill_rect(
                fb,
                Rect::new(cx, cy + row_y * s, row_w * s, s),
                fill,
                BlendMode::SrcOver,
            );
        }
    }

    /// Move cursor: four-way cross arrow (for window dragging).
    fn draw_cursor_move(
        fb: &mut FrameBuffer,
        cx: f32,
        cy: f32,
        s: f32,
        outline: Color,
        fill: Color,
    ) {
        let center_x = cx + 7.0 * s;
        let center_y = cy + 7.0 * s;
        let arm = 5.0 * s;
        let thickness = 2.0 * s;
        let half_t = thickness * 0.5;
        let arrow_w = 4.0 * s;
        let _arrow_h = 3.0 * s;

        // Outline (1px bigger each side)
        let o = s;
        // Vertical arm (outline)
        rasterizer::fill_rect(
            fb,
            Rect::new(
                center_x - half_t - o,
                center_y - arm - _arrow_h - o,
                thickness + 2.0 * o,
                arm * 2.0 + thickness + 2.0 * _arrow_h + 2.0 * o,
            ),
            outline,
            BlendMode::SrcOver,
        );
        // Horizontal arm (outline)
        rasterizer::fill_rect(
            fb,
            Rect::new(
                center_x - arm - _arrow_h - o,
                center_y - half_t - o,
                arm * 2.0 + thickness + 2.0 * _arrow_h + 2.0 * o,
                thickness + 2.0 * o,
            ),
            outline,
            BlendMode::SrcOver,
        );

        // Fill: vertical arm
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - half_t, center_y - arm, thickness, arm * 2.0),
            fill,
            BlendMode::SrcOver,
        );
        // Fill: horizontal arm
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - arm, center_y - half_t, arm * 2.0, thickness),
            fill,
            BlendMode::SrcOver,
        );

        // Arrowheads (triangles made of rects)
        // Up arrow
        for i in 0..3 {
            let fi = i as f32;
            let w = (arrow_w - fi * 2.0 * s).max(s);
            rasterizer::fill_rect(
                fb,
                Rect::new(center_x - w * 0.5, center_y - arm - fi * s, w, s),
                fill,
                BlendMode::SrcOver,
            );
        }
        // Down arrow
        for i in 0..3 {
            let fi = i as f32;
            let w = (arrow_w - fi * 2.0 * s).max(s);
            rasterizer::fill_rect(
                fb,
                Rect::new(center_x - w * 0.5, center_y + arm + fi * s, w, s),
                fill,
                BlendMode::SrcOver,
            );
        }
        // Left arrow
        for i in 0..3 {
            let fi = i as f32;
            let h = (arrow_w - fi * 2.0 * s).max(s);
            rasterizer::fill_rect(
                fb,
                Rect::new(center_x - arm - fi * s, center_y - h * 0.5, s, h),
                fill,
                BlendMode::SrcOver,
            );
        }
        // Right arrow
        for i in 0..3 {
            let fi = i as f32;
            let h = (arrow_w - fi * 2.0 * s).max(s);
            rasterizer::fill_rect(
                fb,
                Rect::new(center_x + arm + fi * s, center_y - h * 0.5, s, h),
                fill,
                BlendMode::SrcOver,
            );
        }
    }

    /// Vertical resize cursor: double-headed vertical arrow.
    fn draw_cursor_resize_ns(
        fb: &mut FrameBuffer,
        cx: f32,
        cy: f32,
        s: f32,
        outline: Color,
        fill: Color,
    ) {
        let center_x = cx + 6.0 * s;
        let center_y = cy + 7.0 * s;
        let arm = 5.0 * s;
        let thickness = 2.0 * s;
        let half_t = thickness * 0.5;
        let o = s;

        // Outline
        rasterizer::fill_rect(
            fb,
            Rect::new(
                center_x - half_t - o,
                center_y - arm - 3.0 * s - o,
                thickness + 2.0 * o,
                arm * 2.0 + 6.0 * s + 2.0 * o,
            ),
            outline,
            BlendMode::SrcOver,
        );
        // Fill: vertical bar
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - half_t, center_y - arm, thickness, arm * 2.0),
            fill,
            BlendMode::SrcOver,
        );
        // Up arrowhead
        for i in 0..3 {
            let fi = i as f32;
            let w = (6.0 * s - fi * 2.0 * s).max(s);
            rasterizer::fill_rect(
                fb,
                Rect::new(center_x - w * 0.5, center_y - arm - fi * s, w, s),
                fill,
                BlendMode::SrcOver,
            );
        }
        // Down arrowhead
        for i in 0..3 {
            let fi = i as f32;
            let w = (6.0 * s - fi * 2.0 * s).max(s);
            rasterizer::fill_rect(
                fb,
                Rect::new(center_x - w * 0.5, center_y + arm + fi * s, w, s),
                fill,
                BlendMode::SrcOver,
            );
        }
    }

    /// Horizontal resize cursor: double-headed horizontal arrow.
    fn draw_cursor_resize_ew(
        fb: &mut FrameBuffer,
        cx: f32,
        cy: f32,
        s: f32,
        outline: Color,
        fill: Color,
    ) {
        let center_x = cx + 7.0 * s;
        let center_y = cy + 6.0 * s;
        let arm = 5.0 * s;
        let thickness = 2.0 * s;
        let half_t = thickness * 0.5;
        let o = s;

        // Outline
        rasterizer::fill_rect(
            fb,
            Rect::new(
                center_x - arm - 3.0 * s - o,
                center_y - half_t - o,
                arm * 2.0 + 6.0 * s + 2.0 * o,
                thickness + 2.0 * o,
            ),
            outline,
            BlendMode::SrcOver,
        );
        // Fill: horizontal bar
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - arm, center_y - half_t, arm * 2.0, thickness),
            fill,
            BlendMode::SrcOver,
        );
        // Left arrowhead
        for i in 0..3 {
            let fi = i as f32;
            let h = (6.0 * s - fi * 2.0 * s).max(s);
            rasterizer::fill_rect(
                fb,
                Rect::new(center_x - arm - fi * s, center_y - h * 0.5, s, h),
                fill,
                BlendMode::SrcOver,
            );
        }
        // Right arrowhead
        for i in 0..3 {
            let fi = i as f32;
            let h = (6.0 * s - fi * 2.0 * s).max(s);
            rasterizer::fill_rect(
                fb,
                Rect::new(center_x + arm + fi * s, center_y - h * 0.5, s, h),
                fill,
                BlendMode::SrcOver,
            );
        }
    }

    /// Diagonal resize cursor (NW-SE).
    fn draw_cursor_resize_nwse(
        fb: &mut FrameBuffer,
        cx: f32,
        cy: f32,
        s: f32,
        outline: Color,
        fill: Color,
    ) {
        let o = s;
        // Diagonal line from top-left to bottom-right
        let len = 12;
        // Outline
        for i in 0..len {
            let fi = i as f32;
            rasterizer::fill_rect(
                fb,
                Rect::new(
                    cx + fi * s - o,
                    cy + fi * s - o,
                    2.0 * s + 2.0 * o,
                    2.0 * s + 2.0 * o,
                ),
                outline,
                BlendMode::SrcOver,
            );
        }
        // Fill
        for i in 0..len {
            let fi = i as f32;
            rasterizer::fill_rect(
                fb,
                Rect::new(cx + fi * s, cy + fi * s, 2.0 * s, 2.0 * s),
                fill,
                BlendMode::SrcOver,
            );
        }
        // NW arrowhead
        for i in 0..4 {
            let fi = i as f32;
            rasterizer::fill_rect(
                fb,
                Rect::new(cx, cy + fi * s, (4.0 - fi) * s, s),
                fill,
                BlendMode::SrcOver,
            );
        }
        // SE arrowhead
        let end = (len - 1) as f32;
        for i in 0..4 {
            let fi = i as f32;
            rasterizer::fill_rect(
                fb,
                Rect::new(
                    cx + (end - 3.0 + fi) * s + 2.0 * s,
                    cy + (end - fi) * s,
                    (4.0 - fi) * s,
                    s,
                ),
                fill,
                BlendMode::SrcOver,
            );
        }
    }

    /// Diagonal resize cursor (NE-SW).
    fn draw_cursor_resize_nesw(
        fb: &mut FrameBuffer,
        cx: f32,
        cy: f32,
        s: f32,
        outline: Color,
        fill: Color,
    ) {
        let o = s;
        let len = 12;
        let max_i = (len - 1) as f32;
        // Outline
        for i in 0..len {
            let fi = i as f32;
            rasterizer::fill_rect(
                fb,
                Rect::new(
                    cx + (max_i - fi) * s - o,
                    cy + fi * s - o,
                    2.0 * s + 2.0 * o,
                    2.0 * s + 2.0 * o,
                ),
                outline,
                BlendMode::SrcOver,
            );
        }
        // Fill
        for i in 0..len {
            let fi = i as f32;
            rasterizer::fill_rect(
                fb,
                Rect::new(cx + (max_i - fi) * s, cy + fi * s, 2.0 * s, 2.0 * s),
                fill,
                BlendMode::SrcOver,
            );
        }
        // NE arrowhead
        for i in 0..4 {
            let fi = i as f32;
            rasterizer::fill_rect(
                fb,
                Rect::new(cx + (max_i - 3.0 + fi) * s, cy + fi * s, (4.0 - fi) * s, s),
                fill,
                BlendMode::SrcOver,
            );
        }
        // SW arrowhead
        for i in 0..4 {
            let fi = i as f32;
            rasterizer::fill_rect(
                fb,
                Rect::new(cx, cy + (max_i - fi) * s, (4.0 - fi) * s, s),
                fill,
                BlendMode::SrcOver,
            );
        }
    }

    /// Pointer / hand cursor: pointing hand for clickable items.
    fn draw_cursor_pointer(
        fb: &mut FrameBuffer,
        cx: f32,
        cy: f32,
        s: f32,
        outline: Color,
        fill: Color,
    ) {
        // Simplified pointing hand: index finger + palm
        let finger_rows: &[(f32, f32, f32)] = &[
            // (y_offset, x_offset, width)
            (0.0, 4.0, 2.0), // fingertip
            (1.0, 4.0, 2.0),
            (2.0, 4.0, 2.0),
            (3.0, 4.0, 2.0),
            (4.0, 4.0, 2.0),
            (5.0, 4.0, 2.0),
            (6.0, 1.0, 9.0), // palm starts
            (7.0, 0.0, 10.0),
            (8.0, 0.0, 10.0),
            (9.0, 0.0, 10.0),
            (10.0, 0.0, 10.0),
            (11.0, 1.0, 9.0),
            (12.0, 1.0, 8.0),
            (13.0, 2.0, 6.0),
        ];
        // Outline
        for &(row_y, row_x, row_w) in finger_rows {
            rasterizer::fill_rect(
                fb,
                Rect::new(
                    cx + row_x * s - s,
                    cy + row_y * s - 0.5 * s,
                    row_w * s + 2.0 * s,
                    2.0 * s,
                ),
                outline,
                BlendMode::SrcOver,
            );
        }
        // Fill
        for &(row_y, row_x, row_w) in finger_rows {
            rasterizer::fill_rect(
                fb,
                Rect::new(cx + row_x * s, cy + row_y * s, row_w * s, s),
                fill,
                BlendMode::SrcOver,
            );
        }
    }

    /// Text / I-beam cursor for text selection.
    fn draw_cursor_text(
        fb: &mut FrameBuffer,
        cx: f32,
        cy: f32,
        s: f32,
        outline: Color,
        fill: Color,
    ) {
        let center_x = cx + 6.0 * s;
        let top = cy + 1.0 * s;
        let bottom = cy + 13.0 * s;
        let bar_h = bottom - top;
        let serif_w = 4.0 * s;
        let o = s;

        // Outline
        rasterizer::fill_rect(
            fb,
            Rect::new(
                center_x - s - o,
                top - o,
                2.0 * s + 2.0 * o,
                bar_h + 2.0 * o,
            ),
            outline,
            BlendMode::SrcOver,
        );
        rasterizer::fill_rect(
            fb,
            Rect::new(
                center_x - serif_w * 0.5 - o,
                top - o,
                serif_w + 2.0 * o,
                2.0 * s + 2.0 * o,
            ),
            outline,
            BlendMode::SrcOver,
        );
        rasterizer::fill_rect(
            fb,
            Rect::new(
                center_x - serif_w * 0.5 - o,
                bottom - s - o,
                serif_w + 2.0 * o,
                2.0 * s + 2.0 * o,
            ),
            outline,
            BlendMode::SrcOver,
        );

        // Fill: vertical bar
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - s, top, 2.0 * s, bar_h),
            fill,
            BlendMode::SrcOver,
        );
        // Top serif
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - serif_w * 0.5, top, serif_w, s),
            fill,
            BlendMode::SrcOver,
        );
        // Bottom serif
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - serif_w * 0.5, bottom - s, serif_w, s),
            fill,
            BlendMode::SrcOver,
        );
    }

    /// Not-allowed / forbidden cursor: circle with diagonal line.
    fn draw_cursor_not_allowed(
        fb: &mut FrameBuffer,
        cx: f32,
        cy: f32,
        s: f32,
        outline: Color,
        fill: Color,
    ) {
        let center_x = cx + 7.0 * s;
        let center_y = cy + 7.0 * s;
        let _radius = 6.0 * s;
        let _thickness = 2.0 * s;

        // Approximate circle outline with rect segments
        let segments: &[(f32, f32, f32, f32)] = &[
            // (x_off, y_off, w, h) relative to center
            (-2.0, -6.0, 4.0, 1.0), // top
            (-4.0, -5.0, 8.0, 1.0),
            (-5.0, -4.0, 2.0, 1.0),
            (3.0, -4.0, 2.0, 1.0),
            (-6.0, -2.0, 1.0, 4.0), // left
            (5.0, -2.0, 1.0, 4.0),  // right
            (-5.0, 3.0, 2.0, 1.0),
            (3.0, 3.0, 2.0, 1.0),
            (-4.0, 4.0, 8.0, 1.0),
            (-2.0, 5.0, 4.0, 1.0), // bottom
        ];

        // Outline
        for &(xo, yo, w, h) in segments {
            rasterizer::fill_rect(
                fb,
                Rect::new(
                    center_x + xo * s - s,
                    center_y + yo * s - s,
                    w * s + 2.0 * s,
                    h * s + 2.0 * s,
                ),
                outline,
                BlendMode::SrcOver,
            );
        }
        // Fill ring
        for &(xo, yo, w, h) in segments {
            rasterizer::fill_rect(
                fb,
                Rect::new(center_x + xo * s, center_y + yo * s, w * s, h * s),
                fill,
                BlendMode::SrcOver,
            );
        }
        // Diagonal line through the circle (outline + fill)
        for i in 0..10 {
            let fi = i as f32;
            rasterizer::fill_rect(
                fb,
                Rect::new(
                    center_x + (-4.0 + fi) * s - s,
                    center_y + (-4.0 + fi) * s - s,
                    2.0 * s + 2.0 * s,
                    2.0 * s + 2.0 * s,
                ),
                outline,
                BlendMode::SrcOver,
            );
        }
        for i in 0..10 {
            let fi = i as f32;
            rasterizer::fill_rect(
                fb,
                Rect::new(
                    center_x + (-4.0 + fi) * s,
                    center_y + (-4.0 + fi) * s,
                    2.0 * s,
                    2.0 * s,
                ),
                fill,
                BlendMode::SrcOver,
            );
        }
    }

    fn draw_cursor_wait(
        fb: &mut FrameBuffer,
        cx: f32,
        cy: f32,
        s: f32,
        outline: Color,
        fill: Color,
    ) {
        // Hourglass shape
        let center_x = cx + 7.0 * s;
        let center_y = cy + 7.0 * s;

        // Top half
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - 4.0 * s, center_y - 6.0 * s, 8.0 * s, 2.0 * s),
            outline,
            BlendMode::SrcOver,
        );
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - 3.0 * s, center_y - 5.0 * s, 6.0 * s, 1.5 * s),
            fill,
            BlendMode::SrcOver,
        );

        // Neck
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - 1.0 * s, center_y - 1.0 * s, 2.0 * s, 2.0 * s),
            outline,
            BlendMode::SrcOver,
        );

        // Bottom half
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - 4.0 * s, center_y + 4.0 * s, 8.0 * s, 2.0 * s),
            outline,
            BlendMode::SrcOver,
        );
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - 3.0 * s, center_y + 3.5 * s, 6.0 * s, 1.5 * s),
            fill,
            BlendMode::SrcOver,
        );
    }

    fn draw_question_mark(fb: &mut FrameBuffer, cx: f32, cy: f32, s: f32, color: Color) {
        // Simple question mark shape
        rasterizer::fill_rect(
            fb,
            Rect::new(cx, cy, 3.0 * s, 1.0 * s),
            color,
            BlendMode::SrcOver,
        );
        rasterizer::fill_rect(
            fb,
            Rect::new(cx + 2.0 * s, cy + 1.0 * s, 1.0 * s, 2.0 * s),
            color,
            BlendMode::SrcOver,
        );
        rasterizer::fill_rect(
            fb,
            Rect::new(cx + 1.0 * s, cy + 3.0 * s, 1.0 * s, 1.0 * s),
            color,
            BlendMode::SrcOver,
        );
        rasterizer::fill_rect(
            fb,
            Rect::new(cx + 1.0 * s, cy + 5.0 * s, 1.0 * s, 1.0 * s),
            color,
            BlendMode::SrcOver,
        );
    }

    fn draw_cursor_crosshair(fb: &mut FrameBuffer, cx: f32, cy: f32, s: f32, color: Color) {
        let center_x = cx + 8.0 * s;
        let center_y = cy + 8.0 * s;

        // Vertical line
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - 0.5 * s, center_y - 6.0 * s, 1.0 * s, 12.0 * s),
            color,
            BlendMode::SrcOver,
        );
        // Horizontal line
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - 6.0 * s, center_y - 0.5 * s, 12.0 * s, 1.0 * s),
            color,
            BlendMode::SrcOver,
        );
    }

    fn draw_cursor_hand(
        fb: &mut FrameBuffer,
        cx: f32,
        cy: f32,
        s: f32,
        outline: Color,
        fill: Color,
        closed: bool,
    ) {
        let offset_x = if closed { 2.0 * s } else { 0.0 };

        // Palm
        rasterizer::fill_rect(
            fb,
            Rect::new(cx + 4.0 * s + offset_x, cy + 8.0 * s, 5.0 * s, 6.0 * s),
            outline,
            BlendMode::SrcOver,
        );
        rasterizer::fill_rect(
            fb,
            Rect::new(cx + 4.5 * s + offset_x, cy + 8.5 * s, 4.0 * s, 5.0 * s),
            fill,
            BlendMode::SrcOver,
        );

        // Fingers (simplified)
        for i in 0..4 {
            let fi = i as f32;
            rasterizer::fill_rect(
                fb,
                Rect::new(
                    cx + (5.0 + fi * 1.2) * s + offset_x,
                    cy + 4.0 * s,
                    1.0 * s,
                    5.0 * s,
                ),
                outline,
                BlendMode::SrcOver,
            );
        }
    }

    fn draw_cursor_magnifier(
        fb: &mut FrameBuffer,
        cx: f32,
        cy: f32,
        s: f32,
        outline: Color,
        fill: Color,
        plus: bool,
    ) {
        let center_x = cx + 6.0 * s;
        let center_y = cy + 6.0 * s;

        // Circle (lens)
        let segments: &[(f32, f32, f32, f32)] = &[
            (-2.0, -4.0, 4.0, 1.0),
            (-3.0, -3.0, 6.0, 1.0),
            (-4.0, -2.0, 8.0, 4.0),
            (-3.0, 2.0, 6.0, 1.0),
            (-2.0, 3.0, 4.0, 1.0),
        ];

        for &(xo, yo, w, h) in segments {
            rasterizer::fill_rect(
                fb,
                Rect::new(center_x + xo * s, center_y + yo * s, w * s, h * s),
                outline,
                BlendMode::SrcOver,
            );
        }

        // Handle
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x + 3.0 * s, center_y + 3.0 * s, 4.0 * s, 1.0 * s),
            outline,
            BlendMode::SrcOver,
        );
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x + 4.0 * s, center_y + 4.0 * s, 3.0 * s, 1.0 * s),
            outline,
            BlendMode::SrcOver,
        );

        // Plus or minus symbol
        if plus {
            rasterizer::fill_rect(
                fb,
                Rect::new(center_x - 1.0 * s, center_y - 0.5 * s, 2.0 * s, 1.0 * s),
                fill,
                BlendMode::SrcOver,
            );
            rasterizer::fill_rect(
                fb,
                Rect::new(center_x - 0.5 * s, center_y - 1.0 * s, 1.0 * s, 2.0 * s),
                fill,
                BlendMode::SrcOver,
            );
        } else {
            rasterizer::fill_rect(
                fb,
                Rect::new(center_x - 1.0 * s, center_y - 0.5 * s, 2.0 * s, 1.0 * s),
                fill,
                BlendMode::SrcOver,
            );
        }
    }

    fn draw_cursor_text_vertical(
        fb: &mut FrameBuffer,
        cx: f32,
        cy: f32,
        s: f32,
        outline: Color,
        fill: Color,
    ) {
        let center_x = cx + 8.0 * s;
        let center_y = cy + 8.0 * s;

        // Horizontal I-beam
        rasterizer::fill_rect(
            fb,
            Rect::new(
                center_x - 0.5 * s - s,
                center_y - 6.0 * s - s,
                1.0 * s + 2.0 * s,
                12.0 * s + 2.0 * s,
            ),
            outline,
            BlendMode::SrcOver,
        );
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - 0.5 * s, center_y - 6.0 * s, 1.0 * s, 12.0 * s),
            fill,
            BlendMode::SrcOver,
        );

        // Top and bottom bars (horizontal)
        rasterizer::fill_rect(
            fb,
            Rect::new(
                center_x - 3.0 * s - s,
                center_y - 6.0 * s - s,
                6.0 * s + 2.0 * s,
                1.0 * s + 2.0 * s,
            ),
            outline,
            BlendMode::SrcOver,
        );
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - 3.0 * s, center_y - 6.0 * s, 6.0 * s, 1.0 * s),
            fill,
            BlendMode::SrcOver,
        );

        rasterizer::fill_rect(
            fb,
            Rect::new(
                center_x - 3.0 * s - s,
                center_y + 5.0 * s - s,
                6.0 * s + 2.0 * s,
                1.0 * s + 2.0 * s,
            ),
            outline,
            BlendMode::SrcOver,
        );
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - 3.0 * s, center_y + 5.0 * s, 6.0 * s, 1.0 * s),
            fill,
            BlendMode::SrcOver,
        );
    }

    fn draw_cursor_all_scroll(
        fb: &mut FrameBuffer,
        cx: f32,
        cy: f32,
        s: f32,
        outline: Color,
        fill: Color,
    ) {
        let center_x = cx + 8.0 * s;
        let center_y = cy + 8.0 * s;

        // Four arrows pointing outward
        // Up arrow
        Self::draw_small_arrow(fb, center_x, center_y - 4.0 * s, s, 0.0, outline, fill);
        // Down arrow
        Self::draw_small_arrow(fb, center_x, center_y + 4.0 * s, s, 180.0, outline, fill);
        // Left arrow
        Self::draw_small_arrow(fb, center_x - 4.0 * s, center_y, s, 270.0, outline, fill);
        // Right arrow
        Self::draw_small_arrow(fb, center_x + 4.0 * s, center_y, s, 90.0, outline, fill);

        // Center dot
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - 1.0 * s, center_y - 1.0 * s, 2.0 * s, 2.0 * s),
            outline,
            BlendMode::SrcOver,
        );
    }

    fn draw_small_arrow(
        fb: &mut FrameBuffer,
        cx: f32,
        cy: f32,
        s: f32,
        _rotation: f32,
        outline: Color,
        _fill: Color,
    ) {
        // Simplified arrow (pointing up by default)
        rasterizer::fill_rect(
            fb,
            Rect::new(cx - 2.0 * s, cy, 4.0 * s, 1.0 * s),
            outline,
            BlendMode::SrcOver,
        );
        rasterizer::fill_rect(
            fb,
            Rect::new(cx - 1.0 * s, cy - 1.0 * s, 2.0 * s, 1.0 * s),
            outline,
            BlendMode::SrcOver,
        );
    }
}
