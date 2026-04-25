//! Title bar — macOS / Qt-style window title bar with traffic-light buttons.

use super::window::WindowFlags;
use liquide_ui_core::{Painter, UiColor, UiTheme};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TitleBarButtonKind {
    Close,
    Minimize,
    Maximize,
    AlwaysOnTop,
}

#[derive(Debug, Clone)]
pub struct TitleBarButton {
    pub kind: TitleBarButtonKind,
    pub hovered: bool,
    pub pressed: bool,
}

impl TitleBarButton {
    pub fn new(kind: TitleBarButtonKind) -> Self {
        Self {
            kind,
            hovered: false,
            pressed: false,
        }
    }
}

pub struct TitleBar {
    pub height: f32,
    pub buttons: Vec<TitleBarButton>,
    pub dragging: bool,
    #[allow(dead_code)]
    drag_start_x: f32,
    #[allow(dead_code)]
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

    fn button_should_show(kind: TitleBarButtonKind, flags: WindowFlags) -> bool {
        match kind {
            TitleBarButtonKind::Close => flags.contains(WindowFlags::CLOSABLE),
            TitleBarButtonKind::Minimize => flags.contains(WindowFlags::MINIMIZABLE),
            TitleBarButtonKind::Maximize => flags.contains(WindowFlags::MAXIMIZABLE),
            TitleBarButtonKind::AlwaysOnTop => true,
        }
    }

    pub(crate) fn hit_test_button(
        &self,
        x: f32,
        y: f32,
        win_x: f32,
        win_y: f32,
        win_width: f32,
        flags: WindowFlags,
        rtl: bool,
        dpi_scale: f32,
    ) -> Option<usize> {
        let btn_radius = 6.0 * dpi_scale.max(0.25);
        let btn_spacing = 8.0 * dpi_scale.max(0.25);
        let btn_edge_inset = 14.0 * dpi_scale.max(0.25);
        let btn_y = win_y + self.height / 2.0;

        let ordered: Vec<(usize, &TitleBarButton)> = if rtl {
            self.buttons.iter().enumerate().rev().collect()
        } else {
            self.buttons.iter().enumerate().collect()
        };

        for (slot, (index, button)) in ordered.iter().enumerate() {
            if !Self::button_should_show(button.kind, flags) {
                continue;
            }
            let step = slot as f32 * (btn_radius * 2.0 + btn_spacing);
            let btn_x = if rtl {
                win_x + win_width - btn_edge_inset - btn_radius - step
            } else {
                win_x + btn_edge_inset + btn_radius + step
            };
            let dx = x - btn_x;
            let dy = y - btn_y;
            if dx * dx + dy * dy <= btn_radius * btn_radius {
                return Some(*index);
            }
        }

        None
    }

    pub(crate) fn hit_test_drag_region(
        &self,
        x: f32,
        y: f32,
        win_x: f32,
        win_y: f32,
        win_width: f32,
        flags: WindowFlags,
        rtl: bool,
        dpi_scale: f32,
    ) -> bool {
        x >= win_x
            && x < win_x + win_width
            && y >= win_y
            && y < win_y + self.height
            && self
                .hit_test_button(x, y, win_x, win_y, win_width, flags, rtl, dpi_scale)
                .is_none()
    }

    pub(crate) fn set_hovered_button(&mut self, hovered: Option<usize>) {
        for (index, button) in self.buttons.iter_mut().enumerate() {
            button.hovered = hovered == Some(index);
        }
    }

    pub(crate) fn set_pressed_button(&mut self, pressed: Option<usize>) {
        for (index, button) in self.buttons.iter_mut().enumerate() {
            button.pressed = pressed == Some(index);
        }
    }

    pub(crate) fn clear_interaction_state(&mut self) {
        self.dragging = false;
        self.set_hovered_button(None);
        self.set_pressed_button(None);
    }

    pub(crate) fn begin_drag(&mut self, pointer_x: f32, pointer_y: f32, win_x: f32, win_y: f32) {
        self.dragging = true;
        self.drag_start_x = pointer_x - win_x;
        self.drag_start_y = pointer_y - win_y;
    }

    pub(crate) fn drag_offset(&self) -> (f32, f32) {
        (self.drag_start_x, self.drag_start_y)
    }

    pub fn paint(
        &self,
        painter: &mut Painter,
        theme: &UiTheme,
        win_x: f32,
        win_y: f32,
        win_width: f32,
        title: &str,
        flags: WindowFlags,
        rtl: bool,
        dpi_scale: f32,
    ) {
        let colors = &theme.colors;
        let tb_h = self.height;
        let radius = theme.radius_lg;
        let s = dpi_scale.max(0.25);

        painter.fill_rounded_rect(win_x, win_y, win_width, tb_h, radius, colors.surface);
        painter.draw_line(
            win_x,
            win_y + tb_h,
            win_x + win_width,
            win_y + tb_h,
            colors.border_subtle,
            1.0,
        );

        let btn_radius = 6.0 * s;
        let btn_spacing = 8.0 * s;
        let btn_edge_inset = 14.0 * s;
        let btn_y = win_y + tb_h / 2.0;

        let ordered: Vec<(usize, &TitleBarButton)> = if rtl {
            self.buttons.iter().enumerate().rev().collect()
        } else {
            self.buttons.iter().enumerate().collect()
        };

        for (i, (_, button)) in ordered.iter().enumerate() {
            let (bg, should_show) = match button.kind {
                TitleBarButtonKind::Close => {
                    if flags.contains(WindowFlags::CLOSABLE) {
                        let c = if button.hovered {
                            UiColor::new(255, 95, 87, 255)
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
                            UiColor::new(255, 189, 46, 255)
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
                            UiColor::new(39, 201, 63, 255)
                        } else {
                            UiColor::new(39, 201, 63, 200)
                        };
                        (c, true)
                    } else {
                        (colors.surface_hover, false)
                    }
                }
                TitleBarButtonKind::AlwaysOnTop => (colors.surface_hover, true),
            };

            let step = i as f32 * (btn_radius * 2.0 + btn_spacing);
            let btn_x = if rtl {
                win_x + win_width - btn_edge_inset - btn_radius - step
            } else {
                win_x + btn_edge_inset + btn_radius + step
            };

            if should_show {
                painter.fill_circle(btn_x, btn_y, btn_radius, bg);
                if !button.hovered {
                    let border = bg.with_alpha(100);
                    painter.stroke_rounded_rect(
                        btn_x - btn_radius,
                        btn_y - btn_radius,
                        btn_radius * 2.0,
                        btn_radius * 2.0,
                        btn_radius,
                        border,
                        0.5,
                    );
                }
            }
        }

        let font = &theme.fonts.window_title;
        let title_fs = font.size;
        let title_w = measure_title_width(title, title_fs);
        let title_x = win_x + (win_width - title_w) / 2.0;
        let title_y = win_y + (tb_h - title_fs) / 2.0;
        painter.draw_text(
            title,
            title_x,
            title_y,
            title_fs,
            colors.text_primary,
            &font.family,
            font.weight >= 600,
        );
    }
}

impl Default for TitleBar {
    fn default() -> Self {
        Self::new()
    }
}

pub fn measure_title_width(title: &str, size: f32) -> f32 {
    let mut total = 0.0_f32;
    for g in UnicodeSegmentation::graphemes(title, true) {
        let first = g.chars().next();
        let advance = match first {
            Some(c) if is_wide(c) => size,
            Some(c) if is_emoji_like(c) => size * 1.1,
            _ => size * 0.55,
        };
        total += advance;
    }
    total
}

fn is_wide(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x115F | 0x2E80..=0x303E | 0x3041..=0x33FF | 0x3400..=0x4DBF |
        0x4E00..=0x9FFF | 0xA000..=0xA4CF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF |
        0xFE30..=0xFE4F | 0xFF00..=0xFF60 | 0xFFE0..=0xFFE6
    )
}

fn is_emoji_like(c: char) -> bool {
    matches!(c as u32, 0x1F300..=0x1FAFF | 0x2600..=0x27BF | 0x1F000..=0x1F2FF)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measure_counts_graphemes_not_bytes() {
        let size = 14.0;
        let ascii = measure_title_width("aaa", size);
        let cjk = measure_title_width("中", size);
        assert!((ascii - size * 0.55 * 3.0).abs() < 0.001);
        assert!((cjk - size).abs() < 0.001);
    }

    #[test]
    fn measure_combining_mark_is_one_cluster() {
        let s = "e\u{0301}";
        let w = measure_title_width(s, 14.0);
        assert!((w - 14.0 * 0.55).abs() < 0.001);
    }

    #[test]
    fn measure_empty_is_zero() {
        assert_eq!(measure_title_width("", 14.0), 0.0);
    }
}
