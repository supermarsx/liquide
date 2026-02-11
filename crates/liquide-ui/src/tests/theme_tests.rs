//! Tests for theme types.

use crate::paint::Color;
use crate::theme::{Theme, ThemeBorderRadius, ThemeColors, ThemeSpacing};

// ---------------------------------------------------------------------------
// Default light theme
// ---------------------------------------------------------------------------

#[test]
fn test_default_light_theme() {
    let theme = Theme::default_light();
    assert_eq!(theme.name, "Light");
    assert_eq!(theme.font_family, "Inter");
    assert_eq!(theme.font_size_base, 14.0);
}

#[test]
fn test_default_light_colors() {
    let theme = Theme::default_light();
    assert_eq!(theme.colors.background, Color::from_rgb(255, 255, 255));
    assert_eq!(theme.colors.foreground, Color::from_rgb(33, 33, 33));
}

#[test]
fn test_default_is_light() {
    let theme = Theme::default();
    assert_eq!(theme.name, "Light");
}

// ---------------------------------------------------------------------------
// Default dark theme
// ---------------------------------------------------------------------------

#[test]
fn test_default_dark_theme() {
    let theme = Theme::default_dark();
    assert_eq!(theme.name, "Dark");
    assert_eq!(theme.font_family, "Inter");
}

#[test]
fn test_default_dark_colors() {
    let theme = Theme::default_dark();
    assert_eq!(theme.colors.background, Color::from_rgb(18, 18, 18));
    assert_eq!(theme.colors.foreground, Color::from_rgb(238, 238, 238));
}

#[test]
fn test_dark_surface_is_dark() {
    let theme = Theme::default_dark();
    assert_eq!(theme.colors.surface, Color::from_rgb(33, 33, 33));
}

// ---------------------------------------------------------------------------
// Theme::new
// ---------------------------------------------------------------------------

#[test]
fn test_theme_new_custom_name() {
    let theme = Theme::new("Custom");
    assert_eq!(theme.name, "Custom");
    // Should use light defaults for everything else.
    assert_eq!(theme.font_family, "Inter");
}

// ---------------------------------------------------------------------------
// ThemeColors
// ---------------------------------------------------------------------------

#[test]
fn test_theme_colors_default() {
    let colors = ThemeColors::default();
    assert_eq!(colors.error, Color::from_rgb(244, 67, 54));
    assert_eq!(colors.warning, Color::from_rgb(255, 152, 0));
    assert_eq!(colors.success, Color::from_rgb(76, 175, 80));
}

#[test]
fn test_theme_colors_accent() {
    let colors = ThemeColors::default();
    assert_eq!(colors.accent, Color::from_rgb(0, 188, 212));
}

// ---------------------------------------------------------------------------
// ThemeSpacing
// ---------------------------------------------------------------------------

#[test]
fn test_theme_spacing_default() {
    let spacing = ThemeSpacing::default();
    assert_eq!(spacing.xs, 4.0);
    assert_eq!(spacing.sm, 8.0);
    assert_eq!(spacing.md, 16.0);
    assert_eq!(spacing.lg, 24.0);
    assert_eq!(spacing.xl, 32.0);
}

// ---------------------------------------------------------------------------
// ThemeBorderRadius
// ---------------------------------------------------------------------------

#[test]
fn test_theme_border_radius_default() {
    let br = ThemeBorderRadius::default();
    assert_eq!(br.sm, 4.0);
    assert_eq!(br.md, 8.0);
    assert_eq!(br.lg, 16.0);
    assert_eq!(br.full, 9999.0);
}

// ---------------------------------------------------------------------------
// Theme equality
// ---------------------------------------------------------------------------

#[test]
fn test_theme_light_and_dark_differ() {
    let light = Theme::default_light();
    let dark = Theme::default_dark();
    assert_ne!(light, dark);
}

#[test]
fn test_theme_clone() {
    let theme = Theme::default_dark();
    let cloned = theme.clone();
    assert_eq!(theme, cloned);
}
