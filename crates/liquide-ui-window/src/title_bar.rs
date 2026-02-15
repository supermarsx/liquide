//! Title bar — macOS / Qt-style window title bar with traffic-light buttons.

use liquide_ui_core::{Painter, UiColor, UiTheme};
use super::window::WindowFlags;

/// Button kind in the title bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TitleBarButtonKind {
    Close,
    Minimize,
    Maximize,
    AlwaysOnTop,
}

/// A single title bar button.
#[derive(Debug, Clone)]
pub struct TitleBarButton {
    pub kind: TitleBarButtonKind,
    pub hovered: bool,
    pub pressed: bool,
}

impl TitleBarButton {
    pub fn new(kind: TitleBarButtonKind) -> Self {
        Self { kind, hovered: false, pressed: false }
    }
}

/// macOS / Qt-style title bar.
pub struct TitleBar {
    pub height: f32,
    pub buttons: Vec<TitleBarButton>,
    pub dragging: bool,
    drag_start_x: f32,
    drag_start_y: f32,
}

impl TitleBar {
    pub fn new() -> Self {
        Self {
            height: 32.0,
            buttons: vec![
                TitleBarButton::new(TitleBarButtonKind::Close),
                TitleBarButton::new(TitleBarButtonKind::Minimize),
                TitleBarButton::new(TitleBarButtonKind::Maximize),
            ],
            dragging: false,
            drag_start_x: 0.0,
            drag_start_y: 0.0,
        }
    }

    /// Paint the title bar.
    pub fn paint(
        &self,
        painter: &mut Painter,
        theme: &UiTheme,
        win_x: f32,
        win_y: f32,
        win_width: f32,
        title: &str,
        flags: WindowFlags,
    ) {
        let colors = &theme.colors;
        let tb_h = self.height;
        let radius = theme.radius_lg;

        // Title bar background (slightly different from window body)
        painter.fill_rounded_rect(
            win_x, win_y, win_width, tb_h,
            radius, colors.surface,
        );

        // Bottom separator line
        painter.draw_line(
            win_x, win_y + tb_h,
            win_x + win_width, win_y + tb_h,
            colors.border_subtle, 1.0,
        );

        // macOS-style traffic light buttons (left side)
        let btn_radius = 6.0;
        let btn_spacing = 8.0;
        let btn_y = win_y + tb_h / 2.0;
        let mut btn_x = win_x + 14.0;

        for button in &self.buttons {
            let (bg, should_show) = match button.kind {
                TitleBarButtonKind::Close => {
                    if flags.contains(WindowFlags::CLOSABLE) {
                        let c = if button.hovered {
                            UiColor::new(255, 95, 87, 255)  // macOS red
                        } else {
                            UiColor::new(255, 95, 87, 200)
                        };
                        (c, true)
                    } else {
                        (colors.surface_hover, false)
                    }
                }
                TitleBarButtonKind::Minimize => {
                    if flags.contains(WindowFlags::MINIMIZABLE) {
                        let c = if button.hovered {
                            UiColor::new(255, 189, 46, 255)  // macOS yellow
                        } else {
                            UiColor::new(255, 189, 46, 200)
                        };
                        (c, true)
                    } else {
                        (colors.surface_hover, false)
                    }
                }
                TitleBarButtonKind::Maximize => {
                    if flags.contains(WindowFlags::MAXIMIZABLE) {
                        let c = if button.hovered {
                            UiColor::new(39, 201, 63, 255)  // macOS green
                        } else {
                            UiColor::new(39, 201, 63, 200)
                        };
                        (c, true)
                    } else {
                        (colors.surface_hover, false)
                    }
                }
                TitleBarButtonKind::AlwaysOnTop => {
                    (colors.surface_hover, true)
                }
            };

            if should_show {
                painter.fill_circle(btn_x, btn_y, btn_radius, bg);
                if !button.hovered {
                    // Subtle border when not hovered
                    let border = bg.with_alpha(100);
                    painter.stroke_rounded_rect(
                        btn_x - btn_radius, btn_y - btn_radius,
                        btn_radius * 2.0, btn_radius * 2.0,
                        btn_radius, border, 0.5,
                    );
                }
            }
            btn_x += btn_radius * 2.0 + btn_spacing;
        }

        // Title text (centered)
        let title_fs = theme.font_size;
        let title_w = title.len() as f32 * title_fs * 0.55;
        let title_x = win_x + (win_width - title_w) / 2.0;
        let title_y = win_y + (tb_h - title_fs) / 2.0;
        painter.draw_text(title, title_x, title_y, title_fs, colors.text_primary, &theme.font_family, true);
    }
}

impl Default for TitleBar {
    fn default() -> Self { Self::new() }
}
