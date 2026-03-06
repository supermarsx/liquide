//! Loading overlay scene — shown during first-frame startup.

use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::Color;
use liquide_compositor::scene::{NodeProperties, SceneNode, SceneNodeKind};

use super::DesktopCompositor;

impl DesktopCompositor {
    /// Build a loading overlay scene — shown during first-frame startup.
    ///
    /// Renders a polished startup screen with a dark background, centered
    /// glass-style panel with branding elements and a progress bar.
    pub(super) fn build_loading_scene(&self) -> SceneNode {
        let w = self.width as f32;
        let h = self.height as f32;
        let screen = Rect::new(0.0, 0.0, w, h);

        let mut root = SceneNode::new(0, SceneNodeKind::Root, NodeProperties::new(screen));

        // Full-screen dark background with a subtle blue tint.
        root.add_child(SceneNode::new(
            1,
            SceneNodeKind::Background {
                color: Color::new(12, 16, 24, 255),
            },
            NodeProperties::new(screen).with_z_order(0),
        ));

        // Subtle radial-ish gradient: lighter center area behind the panel.
        let glow_size = 600.0_f32.min(w * 0.6);
        let glow = Rect::new(
            (w - glow_size) / 2.0,
            (h - glow_size * 0.6) / 2.0,
            glow_size,
            glow_size * 0.6,
        );
        root.add_child(SceneNode::new(
            2,
            SceneNodeKind::Background {
                color: Color::new(20, 30, 50, 120),
            },
            NodeProperties::new(glow).with_z_order(1),
        ));

        // Main panel — glass-style with a dark semi-transparent fill.
        let panel_w = 480.0_f32.min(w - 80.0);
        let panel_h = 200.0_f32.min(h - 80.0);
        let px = (w - panel_w) / 2.0;
        let py = (h - panel_h) / 2.0;
        let panel = Rect::new(px, py, panel_w, panel_h);

        root.add_child(SceneNode::new(
            10,
            SceneNodeKind::Background {
                color: Color::new(24, 28, 40, 230),
            },
            NodeProperties::new(panel).with_z_order(10),
        ));

        // Top accent bar — vibrant blue gradient strip.
        let accent = Rect::new(px, py, panel_w, 3.0);
        root.add_child(SceneNode::new(
            11,
            SceneNodeKind::Background {
                color: Color::new(60, 140, 240, 255),
            },
            NodeProperties::new(accent).with_z_order(11),
        ));

        // Side accent glow — thin vertical blue lines on panel edges.
        let left_accent = Rect::new(px, py + 3.0, 1.0, panel_h - 3.0);
        root.add_child(SceneNode::new(
            12,
            SceneNodeKind::Background {
                color: Color::new(60, 140, 240, 40),
            },
            NodeProperties::new(left_accent).with_z_order(12),
        ));
        let right_accent = Rect::new(px + panel_w - 1.0, py + 3.0, 1.0, panel_h - 3.0);
        root.add_child(SceneNode::new(
            13,
            SceneNodeKind::Background {
                color: Color::new(60, 140, 240, 40),
            },
            NodeProperties::new(right_accent).with_z_order(12),
        ));

        // Bottom border.
        let bottom_border = Rect::new(px, py + panel_h - 1.0, panel_w, 1.0);
        root.add_child(SceneNode::new(
            14,
            SceneNodeKind::Background {
                color: Color::new(60, 140, 240, 30),
            },
            NodeProperties::new(bottom_border).with_z_order(12),
        ));

        // "LIQUIDE" branding — rendered as 7 block letters since we
        // don't have text rendering yet.  Each letter is a small
        // colored rectangle arranged horizontally.
        let letter_w = 18.0_f32;
        let letter_h = 28.0_f32;
        let letter_gap = 8.0_f32;
        let brand_count = 7.0_f32; // L I Q U I D E
        let brand_total_w = brand_count * letter_w + (brand_count - 1.0) * letter_gap;
        let brand_x = px + (panel_w - brand_total_w) / 2.0;
        let brand_y = py + 35.0;

        for i in 0..7 {
            let lx = brand_x + i as f32 * (letter_w + letter_gap);
            // Alternate slightly different blues for visual interest.
            let blue = if i % 2 == 0 { 240 } else { 200 };
            let alpha = if i % 2 == 0 { 255 } else { 220 };
            root.add_child(SceneNode::new(
                20 + i as u64,
                SceneNodeKind::Background {
                    color: Color::new(60, 140, blue, alpha),
                },
                NodeProperties::new(Rect::new(lx, brand_y, letter_w, letter_h)).with_z_order(13),
            ));
        }

        // Subtitle line — thin white bar below the branding.
        let sub_w = brand_total_w * 0.6;
        let sub_rect = Rect::new(
            px + (panel_w - sub_w) / 2.0,
            brand_y + letter_h + 16.0,
            sub_w,
            2.0,
        );
        root.add_child(SceneNode::new(
            30,
            SceneNodeKind::Background {
                color: Color::new(180, 190, 210, 100),
            },
            NodeProperties::new(sub_rect).with_z_order(13),
        ));

        // Progress bar track — dark inset.
        let bar_w = panel_w - 80.0;
        let bar_h = 6.0_f32;
        let bar_x = px + 40.0;
        let bar_y = py + panel_h - 45.0;
        let bar_track = Rect::new(bar_x, bar_y, bar_w, bar_h);
        root.add_child(SceneNode::new(
            40,
            SceneNodeKind::Background {
                color: Color::new(10, 14, 22, 200),
            },
            NodeProperties::new(bar_track).with_z_order(13),
        ));

        // Progress bar fill — animated blue glow.
        // Use frame_count to create a simple shimmer effect.
        let progress = 0.35_f32; // fixed 35% for static loading screen
        let fill_w = bar_w * progress;
        let bar_fill = Rect::new(bar_x, bar_y, fill_w, bar_h);
        root.add_child(SceneNode::new(
            41,
            SceneNodeKind::Background {
                color: Color::new(60, 150, 255, 255),
            },
            NodeProperties::new(bar_fill).with_z_order(14),
        ));

        // Progress bar leading edge glow.
        let edge_w = 20.0_f32.min(fill_w);
        let edge_rect = Rect::new(bar_x + fill_w - edge_w, bar_y - 1.0, edge_w, bar_h + 2.0);
        root.add_child(SceneNode::new(
            42,
            SceneNodeKind::Background {
                color: Color::new(120, 200, 255, 180),
            },
            NodeProperties::new(edge_rect).with_z_order(15),
        ));

        // Status text placeholder — thin gray bar below progress.
        let status_w = 120.0_f32;
        let status_rect = Rect::new(
            px + (panel_w - status_w) / 2.0,
            bar_y + bar_h + 12.0,
            status_w,
            3.0,
        );
        root.add_child(SceneNode::new(
            50,
            SceneNodeKind::Background {
                color: Color::new(120, 130, 150, 80),
            },
            NodeProperties::new(status_rect).with_z_order(13),
        ));

        root
    }
}
