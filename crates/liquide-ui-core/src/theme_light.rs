//! Light widget theme — mirror of `WidgetThemes::liquid_glass_dark` with
//! inverted luminance for use on light (Midday) backgrounds.
//!
//! The dark variants use translucent *white* overlays on dark surfaces;
//! the light variants use translucent *black* overlays on light surfaces
//! to preserve AA contrast (WCAG 2.2 AA for text against background).
//!
//! Provides:
//! - [`WidgetThemes::liquid_glass_light`]
//! - [`UiTheme::light`] — a complete light theme swapping in the Midday
//!   colour palette + light widget styles.

use crate::color::UiColor;
use crate::theme::{
    ElementStateStyle, ElementTheme, ThemeColors, ThemeElevation, ThemeFonts, ThemeMode,
    ThemeMotion, UiTheme, WidgetThemes,
};

impl WidgetThemes {
    /// Light-mode widget themes matching the Midday palette.
    pub fn liquid_glass_light() -> Self {
        // Accent (slightly darker than dark-mode's 0/122/255 for contrast).
        let accent = UiColor::new(0, 113, 179, 255);
        let accent_hover = UiColor::new(0, 133, 209, 255);
        let accent_active = UiColor::new(0, 93, 149, 255);
        // Light surfaces use near-black with alpha for hover/active.
        let surface = UiColor::new(28, 27, 24, 10);
        let surface_hover = UiColor::new(28, 27, 24, 18);
        let surface_active = UiColor::new(28, 27, 24, 28);
        let text = UiColor::new(28, 27, 24, 255);
        let text_disabled = UiColor::new(28, 27, 24, 77);
        let border = UiColor::new(28, 27, 24, 26);
        let border_focus = UiColor::new(0, 113, 179, 128);
        let on_accent = UiColor::white();

        let element = |bg: UiColor, fg: UiColor, bw: f32, bc: UiColor, rr: f32| ElementStateStyle {
            background: bg,
            foreground: fg,
            border_color: bc,
            border_width: bw,
            border_radius: rr,
            shadow_blur: 0.0,
            shadow_color: UiColor::transparent(),
            shadow_offset_y: 0.0,
            opacity: 1.0,
        };

        // Interactive "solid" accent button — mirrors the dark preset's shape,
        // but foreground = on_accent (white) over a mid-saturation blue.
        let button = ElementTheme {
            default: ElementStateStyle {
                background: accent,
                foreground: on_accent,
                border_color: UiColor::transparent(),
                border_width: 0.0,
                border_radius: 10.0,
                shadow_blur: 4.0,
                shadow_color: UiColor::new(0, 0, 0, 40),
                shadow_offset_y: 2.0,
                opacity: 1.0,
            },
            hovered: ElementStateStyle {
                background: accent_hover,
                foreground: on_accent,
                border_color: UiColor::transparent(),
                border_width: 0.0,
                border_radius: 10.0,
                shadow_blur: 6.0,
                shadow_color: UiColor::new(0, 0, 0, 60),
                shadow_offset_y: 3.0,
                opacity: 1.0,
            },
            active: ElementStateStyle {
                background: accent_active,
                foreground: on_accent,
                border_color: UiColor::transparent(),
                border_width: 0.0,
                border_radius: 10.0,
                shadow_blur: 2.0,
                shadow_color: UiColor::new(0, 0, 0, 30),
                shadow_offset_y: 1.0,
                opacity: 1.0,
            },
            focused: ElementStateStyle {
                background: accent,
                foreground: on_accent,
                border_color: border_focus,
                border_width: 2.0,
                border_radius: 10.0,
                shadow_blur: 4.0,
                shadow_color: UiColor::new(0, 113, 179, 80),
                shadow_offset_y: 0.0,
                opacity: 1.0,
            },
            disabled: ElementStateStyle {
                background: surface,
                foreground: text_disabled,
                border_color: border,
                border_width: 1.0,
                border_radius: 10.0,
                shadow_blur: 0.0,
                shadow_color: UiColor::transparent(),
                shadow_offset_y: 0.0,
                opacity: 0.6,
            },
        };

        // Text-bearing surfaces (text_input, dropdown, list_item, menu_item,
        // toolbar_button, badge, tooltip, progress_bar fill).
        let mut input_default = element(surface, text, 1.0, border, 8.0);
        input_default.shadow_blur = 0.0;
        let input_hover = element(surface_hover, text, 1.0, border, 8.0);
        let input_active = element(surface_active, text, 1.0, border_focus, 8.0);
        let input_focused = element(surface_hover, text, 2.0, border_focus, 8.0);
        let input_disabled = ElementStateStyle {
            background: surface,
            foreground: text_disabled,
            border_color: border,
            border_width: 1.0,
            border_radius: 8.0,
            shadow_blur: 0.0,
            shadow_color: UiColor::transparent(),
            shadow_offset_y: 0.0,
            opacity: 0.7,
        };

        let text_input = ElementTheme {
            default: input_default.clone(),
            hovered: input_hover.clone(),
            active: input_active.clone(),
            focused: input_focused.clone(),
            disabled: input_disabled.clone(),
        };
        let dropdown = text_input.clone();
        let slider = text_input.clone();

        let checkbox = ElementTheme {
            default: element(surface, accent, 1.5, border, 4.0),
            hovered: element(surface_hover, accent_hover, 1.5, border, 4.0),
            active: element(accent, on_accent, 1.5, accent, 4.0),
            focused: element(surface, accent, 2.0, border_focus, 4.0),
            disabled: input_disabled.clone(),
        };
        let radio = checkbox.clone();
        let toggle = checkbox.clone();

        let tab = ElementTheme {
            default: element(
                UiColor::transparent(),
                text,
                0.0,
                UiColor::transparent(),
                0.0,
            ),
            hovered: element(surface_hover, text, 0.0, UiColor::transparent(), 0.0),
            active: element(surface_active, accent, 0.0, UiColor::transparent(), 0.0),
            focused: element(surface_hover, accent, 2.0, border_focus, 0.0),
            disabled: element(
                UiColor::transparent(),
                text_disabled,
                0.0,
                UiColor::transparent(),
                0.0,
            ),
        };

        let list_item = ElementTheme {
            default: element(
                UiColor::transparent(),
                text,
                0.0,
                UiColor::transparent(),
                6.0,
            ),
            hovered: element(surface_hover, text, 0.0, UiColor::transparent(), 6.0),
            active: element(accent, on_accent, 0.0, UiColor::transparent(), 6.0),
            focused: element(surface_hover, text, 2.0, border_focus, 6.0),
            disabled: element(
                UiColor::transparent(),
                text_disabled,
                0.0,
                UiColor::transparent(),
                6.0,
            ),
        };
        let menu_item = list_item.clone();
        let toolbar_button = ElementTheme {
            default: element(
                UiColor::transparent(),
                text,
                0.0,
                UiColor::transparent(),
                6.0,
            ),
            hovered: element(surface_hover, text, 0.0, UiColor::transparent(), 6.0),
            active: element(surface_active, text, 0.0, UiColor::transparent(), 6.0),
            focused: element(surface_hover, text, 2.0, border_focus, 6.0),
            disabled: element(
                UiColor::transparent(),
                text_disabled,
                0.0,
                UiColor::transparent(),
                6.0,
            ),
        };

        let scrollbar = ElementTheme {
            default: element(
                UiColor::new(28, 27, 24, 51),
                UiColor::transparent(),
                0.0,
                UiColor::transparent(),
                9999.0,
            ),
            hovered: element(
                UiColor::new(28, 27, 24, 90),
                UiColor::transparent(),
                0.0,
                UiColor::transparent(),
                9999.0,
            ),
            active: element(
                UiColor::new(28, 27, 24, 140),
                UiColor::transparent(),
                0.0,
                UiColor::transparent(),
                9999.0,
            ),
            focused: element(
                UiColor::new(28, 27, 24, 90),
                UiColor::transparent(),
                0.0,
                UiColor::transparent(),
                9999.0,
            ),
            disabled: element(
                UiColor::new(28, 27, 24, 26),
                UiColor::transparent(),
                0.0,
                UiColor::transparent(),
                9999.0,
            ),
        };

        let tooltip = ElementTheme {
            default: ElementStateStyle {
                background: UiColor::new(28, 27, 24, 240),
                foreground: UiColor::white(),
                border_color: UiColor::new(0, 0, 0, 30),
                border_width: 1.0,
                border_radius: 6.0,
                shadow_blur: 8.0,
                shadow_color: UiColor::new(0, 0, 0, 60),
                shadow_offset_y: 3.0,
                opacity: 1.0,
            },
            hovered: element(
                UiColor::new(28, 27, 24, 240),
                UiColor::white(),
                1.0,
                UiColor::new(0, 0, 0, 30),
                6.0,
            ),
            active: element(
                UiColor::new(28, 27, 24, 240),
                UiColor::white(),
                1.0,
                UiColor::new(0, 0, 0, 30),
                6.0,
            ),
            focused: element(
                UiColor::new(28, 27, 24, 240),
                UiColor::white(),
                1.0,
                UiColor::new(0, 0, 0, 30),
                6.0,
            ),
            disabled: element(
                UiColor::new(28, 27, 24, 240),
                UiColor::white(),
                1.0,
                UiColor::new(0, 0, 0, 30),
                6.0,
            ),
        };

        let progress_bar = ElementTheme {
            default: element(accent, on_accent, 0.0, UiColor::transparent(), 9999.0),
            hovered: element(accent_hover, on_accent, 0.0, UiColor::transparent(), 9999.0),
            active: element(
                accent_active,
                on_accent,
                0.0,
                UiColor::transparent(),
                9999.0,
            ),
            focused: element(accent, on_accent, 0.0, UiColor::transparent(), 9999.0),
            disabled: element(surface, text_disabled, 0.0, UiColor::transparent(), 9999.0),
        };

        let badge = ElementTheme {
            default: element(surface_hover, text, 0.0, UiColor::transparent(), 9999.0),
            hovered: element(surface_active, text, 0.0, UiColor::transparent(), 9999.0),
            active: element(surface_active, text, 0.0, UiColor::transparent(), 9999.0),
            focused: element(surface_active, text, 2.0, border_focus, 9999.0),
            disabled: element(surface, text_disabled, 0.0, UiColor::transparent(), 9999.0),
        };

        Self {
            button,
            text_input,
            checkbox,
            radio,
            toggle,
            slider,
            dropdown,
            tab,
            list_item,
            menu_item,
            toolbar_button,
            scrollbar,
            tooltip,
            progress_bar,
            badge,
        }
    }
}

impl UiTheme {
    /// Light theme — Midday palette with the new light widget variants.
    pub fn light() -> Self {
        Self {
            name: "Midday Light".into(),
            mode: ThemeMode::Light,
            colors: ThemeColors::midday(),
            fonts: ThemeFonts::default(),
            widgets: WidgetThemes::liquid_glass_light(),
            elevation: ThemeElevation::default(),
            motion: ThemeMotion::default(),
            font_size: 14.0,
            font_family: "Manrope".into(),
            radius_sm: 6.0,
            radius_md: 10.0,
            radius_lg: 12.0,
            radius_xl: 16.0,
            radius_full: 9999.0,
            spacing_xs: 4.0,
            spacing_sm: 8.0,
            spacing_md: 12.0,
            spacing_lg: 16.0,
            spacing_xl: 24.0,
        }
    }

    /// Dark theme — alias of the Liquid Glass Standard preset, provided so
    /// downstream code can pair `UiTheme::dark()` and `UiTheme::light()`
    /// symmetrically.
    pub fn dark() -> Self {
        Self::liquid_glass()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn relative_luminance(c: UiColor) -> f32 {
        fn ch(v: u8) -> f32 {
            let s = v as f32 / 255.0;
            if s <= 0.03928 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * ch(c.r) + 0.7152 * ch(c.g) + 0.0722 * ch(c.b)
    }

    fn contrast_ratio(a: UiColor, b: UiColor) -> f32 {
        let la = relative_luminance(a);
        let lb = relative_luminance(b);
        let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
        (hi + 0.05) / (lo + 0.05)
    }

    #[test]
    fn light_theme_body_text_meets_wcag_aa() {
        let t = UiTheme::light();
        // Body text on background must have ≥ 4.5 contrast for AA normal text.
        let ratio = contrast_ratio(t.colors.text_primary, t.colors.background);
        assert!(
            ratio >= 4.5,
            "light theme text/bg contrast {:.2} < 4.5 (AA)",
            ratio
        );
    }

    #[test]
    fn light_theme_is_light_mode() {
        let t = UiTheme::light();
        assert_eq!(t.mode, ThemeMode::Light);
    }

    #[test]
    fn dark_alias_is_liquid_glass() {
        let d = UiTheme::dark();
        assert_eq!(d.mode, ThemeMode::Dark);
    }
}
