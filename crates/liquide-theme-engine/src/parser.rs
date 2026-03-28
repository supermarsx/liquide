//! Simple TOML-like theme file parser.
//!
//! Supports `[section]` headers and `key = value` pairs. Values may be:
//! - Quoted strings: `name = "Night"`
//! - Numbers: `height = 36`
//! - Booleans: `supports_glass = true`
//! - Hex colors: `primary = #0a84ff`
//! - RGBA colors: `primary = rgba(10, 132, 255, 1.0)`
//!
//! Sections: `[metadata]`, `[palette]`, `[window]`, `[statusbar]`, `[dock]`,
//! `[menu]`, `[tooltip]`, `[notification]`, `[glass]`.

use crate::color::Color;
use crate::definition::*;
use crate::palette::ColorPalette;

/// Errors from parsing a theme file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Malformed section header.
    BadSection(usize, String),
    /// A key = value line could not be parsed.
    BadKeyValue(usize, String),
    /// A color value could not be parsed.
    BadColor(usize, String),
    /// A numeric value could not be parsed.
    BadNumber(usize, String),
    /// Unknown section name.
    UnknownSection(usize, String),
    /// Missing required metadata field.
    MissingField(String),
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BadSection(ln, s) => write!(f, "line {ln}: bad section header: {s}"),
            Self::BadKeyValue(ln, s) => write!(f, "line {ln}: bad key=value: {s}"),
            Self::BadColor(ln, s) => write!(f, "line {ln}: bad color value: {s}"),
            Self::BadNumber(ln, s) => write!(f, "line {ln}: bad number: {s}"),
            Self::UnknownSection(ln, s) => write!(f, "line {ln}: unknown section: {s}"),
            Self::MissingField(s) => write!(f, "missing required field: {s}"),
        }
    }
}

/// Parse a theme definition from a TOML-like string.
pub fn parse_theme(input: &str) -> Result<ThemeDefinition, ParseError> {
    let mut def = ThemeDefinition::default();
    let mut section = String::new();

    for (idx, raw_line) in input.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw_line.trim();

        // Skip blank lines and comments.
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }

        // Section header.
        if line.starts_with('[') {
            let end = line
                .find(']')
                .ok_or_else(|| ParseError::BadSection(line_no, line.to_string()))?;
            section = line[1..end].trim().to_ascii_lowercase();
            let valid = [
                "metadata",
                "palette",
                "window",
                "statusbar",
                "dock",
                "menu",
                "tooltip",
                "notification",
                "glass",
            ];
            if !valid.contains(&section.as_str()) {
                return Err(ParseError::UnknownSection(line_no, section));
            }
            continue;
        }

        // key = value
        let eq_pos = line
            .find('=')
            .ok_or_else(|| ParseError::BadKeyValue(line_no, line.to_string()))?;
        let key = line[..eq_pos].trim();
        let val = line[eq_pos + 1..].trim();

        match section.as_str() {
            "metadata" => apply_metadata(&mut def.metadata, key, val, line_no)?,
            "palette" => apply_palette(&mut def.palette, key, val, line_no)?,
            "window" => apply_window(&mut def.window, key, val, line_no)?,
            "statusbar" => apply_statusbar(&mut def.statusbar, key, val, line_no)?,
            "dock" => apply_dock(&mut def.dock, key, val, line_no)?,
            "menu" => apply_menu(&mut def.menu, key, val, line_no)?,
            "tooltip" => apply_tooltip(&mut def.tooltip, key, val, line_no)?,
            "notification" => apply_notification(&mut def.notification, key, val, line_no)?,
            "glass" => apply_glass(&mut def.glass, key, val, line_no)?,
            _ => {} // before any section — ignore
        }
    }

    if def.metadata.id.is_empty() {
        return Err(ParseError::MissingField("metadata.id".into()));
    }
    if def.metadata.name.is_empty() {
        def.metadata.name = def.metadata.id.clone();
    }

    Ok(def)
}

// ── Helpers ───────────────────────────────────────────────────────────

fn unquote(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

fn parse_color(val: &str, line_no: usize) -> Result<Color, ParseError> {
    let val = val.trim();
    // Try hex first.
    if val.starts_with('#') || val.chars().all(|c| c.is_ascii_hexdigit()) && val.len() >= 3 {
        if let Some(c) = Color::from_hex(val) {
            return Ok(c);
        }
    }
    // Try CSS rgba/rgb.
    if let Some(c) = Color::from_css_rgba(val) {
        return Ok(c);
    }
    // Try quoted hex.
    let uq = unquote(val);
    if let Some(c) = Color::from_hex(&uq) {
        return Ok(c);
    }
    Err(ParseError::BadColor(line_no, val.to_string()))
}

fn parse_f32(val: &str, line_no: usize) -> Result<f32, ParseError> {
    val.trim()
        .parse::<f32>()
        .map_err(|_| ParseError::BadNumber(line_no, val.to_string()))
}

fn parse_u32(val: &str, line_no: usize) -> Result<u32, ParseError> {
    val.trim()
        .parse::<u32>()
        .map_err(|_| ParseError::BadNumber(line_no, val.to_string()))
}

fn parse_bool(val: &str) -> bool {
    matches!(val.trim().to_ascii_lowercase().as_str(), "true" | "yes" | "1")
}

// ── Section appliers ──────────────────────────────────────────────────

fn apply_metadata(m: &mut ThemeMetadata, key: &str, val: &str, _ln: usize) -> Result<(), ParseError> {
    match key {
        "id" => m.id = unquote(val),
        "name" => m.name = unquote(val),
        "author" => m.author = unquote(val),
        "version" => m.version = unquote(val),
        "description" => m.description = unquote(val),
        "variant" => {
            m.variant = ThemeVariant::from_str_loose(&unquote(val)).unwrap_or(ThemeVariant::Dark);
        }
        "parent" => {
            let p = unquote(val);
            m.parent = if p.is_empty() || p == "none" { None } else { Some(p) };
        }
        "supports_glass" => m.supports_glass = parse_bool(val),
        _ => {} // unknown keys are silently ignored
    }
    Ok(())
}

fn apply_palette(p: &mut ColorPalette, key: &str, val: &str, ln: usize) -> Result<(), ParseError> {
    let c = parse_color(val, ln)?;
    match key {
        "primary" => p.primary = c,
        "secondary" => p.secondary = c,
        "accent" => p.accent = c,
        "background" => p.background = c,
        "surface" => p.surface = c,
        "error" => p.error = c,
        "warning" => p.warning = c,
        "success" => p.success = c,
        "info" => p.info = c,
        "text_primary" => p.text_primary = c,
        "text_secondary" => p.text_secondary = c,
        "text_disabled" => p.text_disabled = c,
        "border" => p.border = c,
        "divider" => p.divider = c,
        "shadow" => p.shadow = c,
        "selection_bg" => p.selection_bg = c,
        "selection_fg" => p.selection_fg = c,
        "link" => p.link = c,
        "link_visited" => p.link_visited = c,
        _ => {}
    }
    Ok(())
}

fn apply_window(w: &mut WindowTheme, key: &str, val: &str, ln: usize) -> Result<(), ParseError> {
    match key {
        "titlebar_height" => w.titlebar_height = parse_f32(val, ln)?,
        "titlebar_bg" => w.titlebar_bg = parse_color(val, ln)?,
        "titlebar_bg_focused" => w.titlebar_bg_focused = parse_color(val, ln)?,
        "titlebar_text" => w.titlebar_text = parse_color(val, ln)?,
        "border_color" => w.border_color = parse_color(val, ln)?,
        "border_color_focused" => w.border_color_focused = parse_color(val, ln)?,
        "border_radius" => w.border_radius = parse_f32(val, ln)?,
        "border_width" => w.border_width = parse_f32(val, ln)?,
        "shadow_color" => w.shadow_color = parse_color(val, ln)?,
        "content_bg" => w.content_bg = parse_color(val, ln)?,
        "close_button_bg" => w.close_button_bg = parse_color(val, ln)?,
        "control_button_bg" => w.control_button_bg = parse_color(val, ln)?,
        _ => {}
    }
    Ok(())
}

fn apply_statusbar(s: &mut StatusBarTheme, key: &str, val: &str, ln: usize) -> Result<(), ParseError> {
    match key {
        "height" => s.height = parse_f32(val, ln)?,
        "background" => s.background = parse_color(val, ln)?,
        "text_color" => s.text_color = parse_color(val, ln)?,
        "border_color" => s.border_color = parse_color(val, ln)?,
        "padding_horizontal" => s.padding_horizontal = parse_f32(val, ln)?,
        "font_size" => s.font_size = parse_f32(val, ln)?,
        _ => {}
    }
    Ok(())
}

fn apply_dock(d: &mut DockTheme, key: &str, val: &str, ln: usize) -> Result<(), ParseError> {
    match key {
        "height" => d.height = parse_f32(val, ln)?,
        "item_size" => d.item_size = parse_f32(val, ln)?,
        "spacing" => d.spacing = parse_f32(val, ln)?,
        "background" => d.background = parse_color(val, ln)?,
        "item_color" => d.item_color = parse_color(val, ln)?,
        "item_active_color" => d.item_active_color = parse_color(val, ln)?,
        "item_hover_bg" => d.item_hover_bg = parse_color(val, ln)?,
        "item_border_radius" => d.item_border_radius = parse_f32(val, ln)?,
        "indicator_color" => d.indicator_color = parse_color(val, ln)?,
        "border_color" => d.border_color = parse_color(val, ln)?,
        _ => {}
    }
    Ok(())
}

fn apply_menu(m: &mut MenuTheme, key: &str, val: &str, ln: usize) -> Result<(), ParseError> {
    match key {
        "item_height" => m.item_height = parse_f32(val, ln)?,
        "padding" => m.padding = parse_f32(val, ln)?,
        "background" => m.background = parse_color(val, ln)?,
        "text_color" => m.text_color = parse_color(val, ln)?,
        "hover_bg" => m.hover_bg = parse_color(val, ln)?,
        "disabled_color" => m.disabled_color = parse_color(val, ln)?,
        "border_color" => m.border_color = parse_color(val, ln)?,
        "border_radius" => m.border_radius = parse_f32(val, ln)?,
        "separator_color" => m.separator_color = parse_color(val, ln)?,
        "shortcut_color" => m.shortcut_color = parse_color(val, ln)?,
        "font_size" => m.font_size = parse_f32(val, ln)?,
        _ => {}
    }
    Ok(())
}

fn apply_tooltip(t: &mut TooltipTheme, key: &str, val: &str, ln: usize) -> Result<(), ParseError> {
    match key {
        "delay_ms" => t.delay_ms = parse_u32(val, ln)?,
        "background" => t.background = parse_color(val, ln)?,
        "text_color" => t.text_color = parse_color(val, ln)?,
        "border_radius" => t.border_radius = parse_f32(val, ln)?,
        "max_width" => t.max_width = parse_f32(val, ln)?,
        "font_size" => t.font_size = parse_f32(val, ln)?,
        "padding_horizontal" => t.padding_horizontal = parse_f32(val, ln)?,
        "padding_vertical" => t.padding_vertical = parse_f32(val, ln)?,
        _ => {}
    }
    Ok(())
}

fn apply_notification(n: &mut NotificationTheme, key: &str, val: &str, ln: usize) -> Result<(), ParseError> {
    match key {
        "width" => n.width = parse_f32(val, ln)?,
        "background" => n.background = parse_color(val, ln)?,
        "title_color" => n.title_color = parse_color(val, ln)?,
        "body_color" => n.body_color = parse_color(val, ln)?,
        "border_radius" => n.border_radius = parse_f32(val, ln)?,
        "spacing" => n.spacing = parse_f32(val, ln)?,
        "padding" => n.padding = parse_f32(val, ln)?,
        "action_bg" => n.action_bg = parse_color(val, ln)?,
        "action_color" => n.action_color = parse_color(val, ln)?,
        _ => {}
    }
    Ok(())
}

fn apply_glass(g: &mut GlassParams, key: &str, val: &str, ln: usize) -> Result<(), ParseError> {
    match key {
        "tint_color" => g.tint_color = parse_color(val, ln)?,
        "blur_radius" => g.blur_radius = parse_f32(val, ln)?,
        "saturation" => g.saturation = parse_f32(val, ln)?,
        "opacity" => g.opacity = parse_f32(val, ln)?,
        _ => {}
    }
    Ok(())
}
