use crate::{Dialog, DialogId, DialogResult};

/// RGBA color (0-255 per channel)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// HSV color (h: 0-360, s: 0.0-1.0, v: 0.0-1.0)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hsv {
    pub h: f32,
    pub s: f32,
    pub v: f32,
}

/// Color picker state machine
#[derive(Debug)]
pub struct ColorPickerState {
    pub id: DialogId,
    pub title: String,
    pub hsv: Hsv,
    pub opacity: f32,
    pub hex_input: String,
    pub palette: Vec<Rgba>,
    pub recent_colors: Vec<Rgba>,
    pub saved_colors: Vec<Rgba>,
    pub eyedropper_active: bool,
    pub original_color: Option<Rgba>,
}

impl ColorPickerState {
    pub fn new(id: DialogId, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            hsv: Hsv {
                h: 0.0,
                s: 1.0,
                v: 1.0,
            },
            opacity: 1.0,
            hex_input: String::from("#FF0000"),
            palette: default_palette(),
            recent_colors: Vec::new(),
            saved_colors: Vec::new(),
            eyedropper_active: false,
            original_color: None,
        }
    }

    /// Create a picker starting from a given color
    pub fn with_initial_color(mut self, color: Rgba) -> Self {
        self.original_color = Some(color);
        self.opacity = color.a as f32 / 255.0;
        self.hsv = rgb_to_hsv(color.r, color.g, color.b);
        self.hex_input = rgb_to_hex(color.r, color.g, color.b);
        self
    }

    /// Set color via HSV
    pub fn set_hsv(&mut self, h: f32, s: f32, v: f32) {
        self.hsv = Hsv {
            h: h.clamp(0.0, 360.0),
            s: s.clamp(0.0, 1.0),
            v: v.clamp(0.0, 1.0),
        };
        let (r, g, b) = hsv_to_rgb(self.hsv.h, self.hsv.s, self.hsv.v);
        self.hex_input = rgb_to_hex(r, g, b);
    }

    /// Set color via RGB
    pub fn set_rgb(&mut self, r: u8, g: u8, b: u8) {
        self.hsv = rgb_to_hsv(r, g, b);
        self.hex_input = rgb_to_hex(r, g, b);
    }

    /// Set color via hex string (e.g. "#FF0000" or "FF0000")
    pub fn set_hex(&mut self, hex: &str) -> bool {
        if let Some((r, g, b)) = hex_to_rgb(hex) {
            self.hsv = rgb_to_hsv(r, g, b);
            self.hex_input = rgb_to_hex(r, g, b);
            true
        } else {
            false
        }
    }

    /// Set opacity (0.0 - 1.0)
    pub fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity.clamp(0.0, 1.0);
    }

    /// Get the current color as RGBA
    pub fn current_color(&self) -> Rgba {
        let (r, g, b) = hsv_to_rgb(self.hsv.h, self.hsv.s, self.hsv.v);
        Rgba {
            r,
            g,
            b,
            a: (self.opacity * 255.0).round() as u8,
        }
    }

    /// Toggle eyedropper mode
    pub fn toggle_eyedropper(&mut self) {
        self.eyedropper_active = !self.eyedropper_active;
    }

    /// Pick color from eyedropper
    pub fn eyedropper_pick(&mut self, color: Rgba) {
        self.eyedropper_active = false;
        self.set_rgb(color.r, color.g, color.b);
        self.opacity = color.a as f32 / 255.0;
    }

    /// Save current color to saved list
    pub fn save_current(&mut self) {
        let color = self.current_color();
        if !self.saved_colors.contains(&color) {
            self.saved_colors.push(color);
        }
    }

    /// Add current color to recent list
    pub fn push_recent(&mut self) {
        let color = self.current_color();
        self.recent_colors.retain(|c| c != &color);
        self.recent_colors.insert(0, color);
        if self.recent_colors.len() > 8 {
            self.recent_colors.truncate(8);
        }
    }

    /// Confirm selection
    pub fn confirm(&mut self) -> DialogResult<Rgba> {
        self.push_recent();
        DialogResult::Ok(self.current_color())
    }
}

/// Convert HSV to RGB
/// h: 0-360, s: 0-1, v: 0-1
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    if s <= 0.0 {
        let val = (v * 255.0).round() as u8;
        return (val, val, val);
    }

    let h = if h >= 360.0 { 0.0 } else { h / 60.0 };
    let i = h.floor() as i32;
    let f = h - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));

    let (r, g, b) = match i {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };

    (
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
    )
}

/// Convert RGB to HSV
pub fn rgb_to_hsv(r: u8, g: u8, b: u8) -> Hsv {
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let h = if delta < f32::EPSILON {
        0.0
    } else if (max - r).abs() < f32::EPSILON {
        60.0 * (((g - b) / delta) % 6.0)
    } else if (max - g).abs() < f32::EPSILON {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };

    let h = if h < 0.0 { h + 360.0 } else { h };

    let s = if max < f32::EPSILON { 0.0 } else { delta / max };

    Hsv { h, s, v: max }
}

/// Convert RGB to hex string
pub fn rgb_to_hex(r: u8, g: u8, b: u8) -> String {
    format!("#{:02X}{:02X}{:02X}", r, g, b)
}

/// Parse hex string to RGB
pub fn hex_to_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let hex = hex.trim_start_matches('#');
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some((r, g, b))
    } else if hex.len() == 3 {
        let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
        let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
        let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
        Some((r, g, b))
    } else {
        None
    }
}

fn default_palette() -> Vec<Rgba> {
    [
        (0, 0, 0),
        (128, 128, 128),
        (128, 0, 0),
        (128, 128, 0),
        (0, 128, 0),
        (0, 128, 128),
        (0, 0, 128),
        (128, 0, 128),
        (255, 255, 255),
        (192, 192, 192),
        (255, 0, 0),
        (255, 255, 0),
        (0, 255, 0),
        (0, 255, 255),
        (0, 0, 255),
        (255, 0, 255),
    ]
    .iter()
    .map(|&(r, g, b)| Rgba { r, g, b, a: 255 })
    .collect()
}

impl Dialog for ColorPickerState {
    type Output = Rgba;
    fn id(&self) -> DialogId {
        self.id
    }
    fn title(&self) -> &str {
        &self.title
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hsv_to_rgb_red() {
        assert_eq!(hsv_to_rgb(0.0, 1.0, 1.0), (255, 0, 0));
    }

    #[test]
    fn test_hsv_to_rgb_green() {
        assert_eq!(hsv_to_rgb(120.0, 1.0, 1.0), (0, 255, 0));
    }

    #[test]
    fn test_hsv_to_rgb_blue() {
        assert_eq!(hsv_to_rgb(240.0, 1.0, 1.0), (0, 0, 255));
    }

    #[test]
    fn test_hsv_to_rgb_white() {
        assert_eq!(hsv_to_rgb(0.0, 0.0, 1.0), (255, 255, 255));
    }

    #[test]
    fn test_hsv_to_rgb_black() {
        assert_eq!(hsv_to_rgb(0.0, 0.0, 0.0), (0, 0, 0));
    }

    #[test]
    fn test_rgb_to_hsv_round_trip() {
        for &(r, g, b) in &[
            (255u8, 0u8, 0u8),
            (0, 255, 0),
            (0, 0, 255),
            (255, 255, 0),
            (0, 255, 255),
            (255, 0, 255),
            (128, 64, 32),
        ] {
            let hsv = rgb_to_hsv(r, g, b);
            let (r2, g2, b2) = hsv_to_rgb(hsv.h, hsv.s, hsv.v);
            assert!(
                (r as i16 - r2 as i16).abs() <= 1
                    && (g as i16 - g2 as i16).abs() <= 1
                    && (b as i16 - b2 as i16).abs() <= 1,
                "round trip failed for ({r}, {g}, {b}) -> ({r2}, {g2}, {b2})"
            );
        }
    }

    #[test]
    fn test_rgb_to_hex() {
        assert_eq!(rgb_to_hex(255, 0, 0), "#FF0000");
        assert_eq!(rgb_to_hex(0, 128, 255), "#0080FF");
        assert_eq!(rgb_to_hex(0, 0, 0), "#000000");
    }

    #[test]
    fn test_hex_to_rgb() {
        assert_eq!(hex_to_rgb("#FF0000"), Some((255, 0, 0)));
        assert_eq!(hex_to_rgb("00FF00"), Some((0, 255, 0)));
        assert_eq!(hex_to_rgb("#F00"), Some((255, 0, 0)));
        assert_eq!(hex_to_rgb("invalid"), None);
    }

    #[test]
    fn test_hex_round_trip() {
        let hex = "#3A7BC8";
        let (r, g, b) = hex_to_rgb(hex).unwrap();
        let hex2 = rgb_to_hex(r, g, b);
        assert_eq!(hex, hex2);
    }

    #[test]
    fn test_set_hsv() {
        let mut picker = ColorPickerState::new(DialogId(1), "Pick Color");
        picker.set_hsv(120.0, 1.0, 1.0);
        let color = picker.current_color();
        assert_eq!(color.r, 0);
        assert_eq!(color.g, 255);
        assert_eq!(color.b, 0);
    }

    #[test]
    fn test_set_rgb() {
        let mut picker = ColorPickerState::new(DialogId(1), "Pick Color");
        picker.set_rgb(0, 0, 255);
        let color = picker.current_color();
        assert_eq!(color.b, 255);
        assert!(color.r <= 1);
        assert!(color.g <= 1);
    }

    #[test]
    fn test_set_hex() {
        let mut picker = ColorPickerState::new(DialogId(1), "Pick Color");
        assert!(picker.set_hex("#00FF00"));
        let color = picker.current_color();
        assert_eq!(color.g, 255);
        assert!(!picker.set_hex("nope"));
    }

    #[test]
    fn test_opacity() {
        let mut picker = ColorPickerState::new(DialogId(1), "Pick Color");
        picker.set_opacity(0.5);
        let color = picker.current_color();
        assert!((color.a as i16 - 128).abs() <= 1);
    }

    #[test]
    fn test_eyedropper() {
        let mut picker = ColorPickerState::new(DialogId(1), "Pick Color");
        picker.toggle_eyedropper();
        assert!(picker.eyedropper_active);
        picker.eyedropper_pick(Rgba {
            r: 100,
            g: 200,
            b: 50,
            a: 255,
        });
        assert!(!picker.eyedropper_active);
        let color = picker.current_color();
        assert_eq!(color.r, 100);
        assert_eq!(color.g, 200);
        assert_eq!(color.b, 50);
    }

    #[test]
    fn test_recent_colors() {
        let mut picker = ColorPickerState::new(DialogId(1), "Pick Color");
        picker.set_rgb(255, 0, 0);
        picker.push_recent();
        picker.set_rgb(0, 255, 0);
        picker.push_recent();

        assert_eq!(picker.recent_colors.len(), 2);
        // Most recent first
        assert_eq!(picker.recent_colors[0].g, 255);
    }

    #[test]
    fn test_recent_colors_dedup() {
        let mut picker = ColorPickerState::new(DialogId(1), "Pick Color");
        picker.set_rgb(255, 0, 0);
        picker.push_recent();
        picker.push_recent();
        assert_eq!(picker.recent_colors.len(), 1);
    }

    #[test]
    fn test_recent_colors_max_8() {
        let mut picker = ColorPickerState::new(DialogId(1), "Pick Color");
        for i in 0..12 {
            picker.set_rgb(i * 20, 0, 0);
            picker.push_recent();
        }
        assert_eq!(picker.recent_colors.len(), 8);
    }

    #[test]
    fn test_save_current() {
        let mut picker = ColorPickerState::new(DialogId(1), "Pick Color");
        picker.set_rgb(255, 0, 0);
        picker.save_current();
        picker.save_current(); // duplicate
        assert_eq!(picker.saved_colors.len(), 1);
    }

    #[test]
    fn test_default_palette_count() {
        let palette = default_palette();
        assert_eq!(palette.len(), 16);
    }

    #[test]
    fn test_with_initial_color() {
        let picker = ColorPickerState::new(DialogId(1), "Pick Color")
            .with_initial_color(Rgba {
                r: 0,
                g: 128,
                b: 255,
                a: 200,
            });
        assert!(picker.original_color.is_some());
        let color = picker.current_color();
        assert!((color.r as i16).abs() <= 1);
        assert!((color.g as i16 - 128).abs() <= 1);
        assert_eq!(color.b, 255);
        assert_eq!(color.a, 200);
    }
}
