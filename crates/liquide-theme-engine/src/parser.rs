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

/// Parsed theme source that preserves which fields were explicitly provided.
///
/// Register this via [`crate::ThemeManager::register_parsed_theme`] when
/// inheritance needs to distinguish omitted fields from explicit resets.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedTheme {
    definition: ThemeDefinition,
    pub(crate) overrides: ThemeOverrides,
}

impl ParsedTheme {
    pub fn definition(&self) -> &ThemeDefinition {
        &self.definition
    }

    pub fn into_definition(self) -> ThemeDefinition {
        self.definition
    }

    pub(crate) fn into_parts(self) -> (ThemeDefinition, ThemeOverrides) {
        (self.definition, self.overrides)
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ThemeOverrides {
    pub metadata: ThemeMetadataOverrides,
    pub palette: ColorPaletteOverrides,
    pub window: WindowThemeOverrides,
    pub statusbar: StatusBarThemeOverrides,
    pub dock: DockThemeOverrides,
    pub menu: MenuThemeOverrides,
    pub tooltip: TooltipThemeOverrides,
    pub notification: NotificationThemeOverrides,
    pub glass: GlassParamsOverrides,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ThemeMetadataOverrides {
    pub author: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub variant: Option<ThemeVariant>,
    pub supports_glass: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ColorPaletteOverrides {
    pub primary: Option<Color>,
    pub secondary: Option<Color>,
    pub accent: Option<Color>,
    pub background: Option<Color>,
    pub surface: Option<Color>,
    pub error: Option<Color>,
    pub warning: Option<Color>,
    pub success: Option<Color>,
    pub info: Option<Color>,
    pub text_primary: Option<Color>,
    pub text_secondary: Option<Color>,
    pub text_disabled: Option<Color>,
    pub border: Option<Color>,
    pub divider: Option<Color>,
    pub shadow: Option<Color>,
    pub selection_bg: Option<Color>,
    pub selection_fg: Option<Color>,
    pub link: Option<Color>,
    pub link_visited: Option<Color>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct WindowThemeOverrides {
    pub titlebar_height: Option<f32>,
    pub titlebar_bg: Option<Color>,
    pub titlebar_bg_focused: Option<Color>,
    pub titlebar_text: Option<Color>,
    pub border_color: Option<Color>,
    pub border_color_focused: Option<Color>,
    pub border_radius: Option<f32>,
    pub border_width: Option<f32>,
    pub shadow_color: Option<Color>,
    pub content_bg: Option<Color>,
    pub close_button_bg: Option<Color>,
    pub control_button_bg: Option<Color>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct StatusBarThemeOverrides {
    pub height: Option<f32>,
    pub background: Option<Color>,
    pub text_color: Option<Color>,
    pub border_color: Option<Color>,
    pub padding_horizontal: Option<f32>,
    pub font_size: Option<f32>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct DockThemeOverrides {
    pub height: Option<f32>,
    pub item_size: Option<f32>,
    pub spacing: Option<f32>,
    pub background: Option<Color>,
    pub item_color: Option<Color>,
    pub item_active_color: Option<Color>,
    pub item_hover_bg: Option<Color>,
    pub item_border_radius: Option<f32>,
    pub indicator_color: Option<Color>,
    pub border_color: Option<Color>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct MenuThemeOverrides {
    pub item_height: Option<f32>,
    pub padding: Option<f32>,
    pub background: Option<Color>,
    pub text_color: Option<Color>,
    pub hover_bg: Option<Color>,
    pub disabled_color: Option<Color>,
    pub border_color: Option<Color>,
    pub border_radius: Option<f32>,
    pub separator_color: Option<Color>,
    pub shortcut_color: Option<Color>,
    pub font_size: Option<f32>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct TooltipThemeOverrides {
    pub delay_ms: Option<u32>,
    pub background: Option<Color>,
    pub text_color: Option<Color>,
    pub border_radius: Option<f32>,
    pub max_width: Option<f32>,
    pub font_size: Option<f32>,
    pub padding_horizontal: Option<f32>,
    pub padding_vertical: Option<f32>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct NotificationThemeOverrides {
    pub width: Option<f32>,
    pub background: Option<Color>,
    pub title_color: Option<Color>,
    pub body_color: Option<Color>,
    pub border_radius: Option<f32>,
    pub spacing: Option<f32>,
    pub padding: Option<f32>,
    pub action_bg: Option<Color>,
    pub action_color: Option<Color>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct GlassParamsOverrides {
    pub tint_color: Option<Color>,
    pub blur_radius: Option<f32>,
    pub saturation: Option<f32>,
    pub opacity: Option<f32>,
}

impl ThemeOverrides {
    pub(crate) fn from_definition(definition: &ThemeDefinition) -> Self {
        Self {
            metadata: ThemeMetadataOverrides {
                author: Some(definition.metadata.author.clone()),
                version: Some(definition.metadata.version.clone()),
                description: Some(definition.metadata.description.clone()),
                variant: Some(definition.metadata.variant),
                supports_glass: Some(definition.metadata.supports_glass),
            },
            palette: ColorPaletteOverrides {
                primary: Some(definition.palette.primary),
                secondary: Some(definition.palette.secondary),
                accent: Some(definition.palette.accent),
                background: Some(definition.palette.background),
                surface: Some(definition.palette.surface),
                error: Some(definition.palette.error),
                warning: Some(definition.palette.warning),
                success: Some(definition.palette.success),
                info: Some(definition.palette.info),
                text_primary: Some(definition.palette.text_primary),
                text_secondary: Some(definition.palette.text_secondary),
                text_disabled: Some(definition.palette.text_disabled),
                border: Some(definition.palette.border),
                divider: Some(definition.palette.divider),
                shadow: Some(definition.palette.shadow),
                selection_bg: Some(definition.palette.selection_bg),
                selection_fg: Some(definition.palette.selection_fg),
                link: Some(definition.palette.link),
                link_visited: Some(definition.palette.link_visited),
            },
            window: WindowThemeOverrides {
                titlebar_height: Some(definition.window.titlebar_height),
                titlebar_bg: Some(definition.window.titlebar_bg),
                titlebar_bg_focused: Some(definition.window.titlebar_bg_focused),
                titlebar_text: Some(definition.window.titlebar_text),
                border_color: Some(definition.window.border_color),
                border_color_focused: Some(definition.window.border_color_focused),
                border_radius: Some(definition.window.border_radius),
                border_width: Some(definition.window.border_width),
                shadow_color: Some(definition.window.shadow_color),
                content_bg: Some(definition.window.content_bg),
                close_button_bg: Some(definition.window.close_button_bg),
                control_button_bg: Some(definition.window.control_button_bg),
            },
            statusbar: StatusBarThemeOverrides {
                height: Some(definition.statusbar.height),
                background: Some(definition.statusbar.background),
                text_color: Some(definition.statusbar.text_color),
                border_color: Some(definition.statusbar.border_color),
                padding_horizontal: Some(definition.statusbar.padding_horizontal),
                font_size: Some(definition.statusbar.font_size),
            },
            dock: DockThemeOverrides {
                height: Some(definition.dock.height),
                item_size: Some(definition.dock.item_size),
                spacing: Some(definition.dock.spacing),
                background: Some(definition.dock.background),
                item_color: Some(definition.dock.item_color),
                item_active_color: Some(definition.dock.item_active_color),
                item_hover_bg: Some(definition.dock.item_hover_bg),
                item_border_radius: Some(definition.dock.item_border_radius),
                indicator_color: Some(definition.dock.indicator_color),
                border_color: Some(definition.dock.border_color),
            },
            menu: MenuThemeOverrides {
                item_height: Some(definition.menu.item_height),
                padding: Some(definition.menu.padding),
                background: Some(definition.menu.background),
                text_color: Some(definition.menu.text_color),
                hover_bg: Some(definition.menu.hover_bg),
                disabled_color: Some(definition.menu.disabled_color),
                border_color: Some(definition.menu.border_color),
                border_radius: Some(definition.menu.border_radius),
                separator_color: Some(definition.menu.separator_color),
                shortcut_color: Some(definition.menu.shortcut_color),
                font_size: Some(definition.menu.font_size),
            },
            tooltip: TooltipThemeOverrides {
                delay_ms: Some(definition.tooltip.delay_ms),
                background: Some(definition.tooltip.background),
                text_color: Some(definition.tooltip.text_color),
                border_radius: Some(definition.tooltip.border_radius),
                max_width: Some(definition.tooltip.max_width),
                font_size: Some(definition.tooltip.font_size),
                padding_horizontal: Some(definition.tooltip.padding_horizontal),
                padding_vertical: Some(definition.tooltip.padding_vertical),
            },
            notification: NotificationThemeOverrides {
                width: Some(definition.notification.width),
                background: Some(definition.notification.background),
                title_color: Some(definition.notification.title_color),
                body_color: Some(definition.notification.body_color),
                border_radius: Some(definition.notification.border_radius),
                spacing: Some(definition.notification.spacing),
                padding: Some(definition.notification.padding),
                action_bg: Some(definition.notification.action_bg),
                action_color: Some(definition.notification.action_color),
            },
            glass: GlassParamsOverrides {
                tint_color: Some(definition.glass.tint_color),
                blur_radius: Some(definition.glass.blur_radius),
                saturation: Some(definition.glass.saturation),
                opacity: Some(definition.glass.opacity),
            },
        }
    }
}

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
    Ok(parse_theme_source(input)?.into_definition())
}

/// Parse a theme source while preserving omitted-field information for
/// inheritance-aware registration.
pub fn parse_theme_source(input: &str) -> Result<ParsedTheme, ParseError> {
    let mut def = ThemeDefinition::default();
    let mut overrides = ThemeOverrides::default();
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
            "metadata" => apply_metadata(
                &mut def.metadata,
                &mut overrides.metadata,
                key,
                val,
                line_no,
            )?,
            "palette" => {
                apply_palette(&mut def.palette, &mut overrides.palette, key, val, line_no)?
            }
            "window" => apply_window(&mut def.window, &mut overrides.window, key, val, line_no)?,
            "statusbar" => apply_statusbar(
                &mut def.statusbar,
                &mut overrides.statusbar,
                key,
                val,
                line_no,
            )?,
            "dock" => apply_dock(&mut def.dock, &mut overrides.dock, key, val, line_no)?,
            "menu" => apply_menu(&mut def.menu, &mut overrides.menu, key, val, line_no)?,
            "tooltip" => {
                apply_tooltip(&mut def.tooltip, &mut overrides.tooltip, key, val, line_no)?
            }
            "notification" => apply_notification(
                &mut def.notification,
                &mut overrides.notification,
                key,
                val,
                line_no,
            )?,
            "glass" => apply_glass(&mut def.glass, &mut overrides.glass, key, val, line_no)?,
            _ => {} // before any section — ignore
        }
    }

    if def.metadata.id.is_empty() {
        return Err(ParseError::MissingField("metadata.id".into()));
    }
    if def.metadata.name.is_empty() {
        def.metadata.name = def.metadata.id.clone();
    }

    Ok(ParsedTheme {
        definition: def,
        overrides,
    })
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
    matches!(
        val.trim().to_ascii_lowercase().as_str(),
        "true" | "yes" | "1"
    )
}

// ── Section appliers ──────────────────────────────────────────────────

fn apply_metadata(
    m: &mut ThemeMetadata,
    overrides: &mut ThemeMetadataOverrides,
    key: &str,
    val: &str,
    _ln: usize,
) -> Result<(), ParseError> {
    match key {
        "id" => m.id = unquote(val),
        "name" => m.name = unquote(val),
        "author" => {
            m.author = unquote(val);
            overrides.author = Some(m.author.clone());
        }
        "version" => {
            m.version = unquote(val);
            overrides.version = Some(m.version.clone());
        }
        "description" => {
            m.description = unquote(val);
            overrides.description = Some(m.description.clone());
        }
        "variant" => {
            m.variant = ThemeVariant::from_str_loose(&unquote(val)).unwrap_or(ThemeVariant::Dark);
            overrides.variant = Some(m.variant);
        }
        "parent" => {
            let p = unquote(val);
            m.parent = if p.is_empty() || p == "none" {
                None
            } else {
                Some(p)
            };
        }
        "supports_glass" => {
            m.supports_glass = parse_bool(val);
            overrides.supports_glass = Some(m.supports_glass);
        }
        _ => {} // unknown keys are silently ignored
    }
    Ok(())
}

fn apply_palette(
    p: &mut ColorPalette,
    overrides: &mut ColorPaletteOverrides,
    key: &str,
    val: &str,
    ln: usize,
) -> Result<(), ParseError> {
    let c = parse_color(val, ln)?;
    match key {
        "primary" => {
            p.primary = c;
            overrides.primary = Some(c);
        }
        "secondary" => {
            p.secondary = c;
            overrides.secondary = Some(c);
        }
        "accent" => {
            p.accent = c;
            overrides.accent = Some(c);
        }
        "background" => {
            p.background = c;
            overrides.background = Some(c);
        }
        "surface" => {
            p.surface = c;
            overrides.surface = Some(c);
        }
        "error" => {
            p.error = c;
            overrides.error = Some(c);
        }
        "warning" => {
            p.warning = c;
            overrides.warning = Some(c);
        }
        "success" => {
            p.success = c;
            overrides.success = Some(c);
        }
        "info" => {
            p.info = c;
            overrides.info = Some(c);
        }
        "text_primary" => {
            p.text_primary = c;
            overrides.text_primary = Some(c);
        }
        "text_secondary" => {
            p.text_secondary = c;
            overrides.text_secondary = Some(c);
        }
        "text_disabled" => {
            p.text_disabled = c;
            overrides.text_disabled = Some(c);
        }
        "border" => {
            p.border = c;
            overrides.border = Some(c);
        }
        "divider" => {
            p.divider = c;
            overrides.divider = Some(c);
        }
        "shadow" => {
            p.shadow = c;
            overrides.shadow = Some(c);
        }
        "selection_bg" => {
            p.selection_bg = c;
            overrides.selection_bg = Some(c);
        }
        "selection_fg" => {
            p.selection_fg = c;
            overrides.selection_fg = Some(c);
        }
        "link" => {
            p.link = c;
            overrides.link = Some(c);
        }
        "link_visited" => {
            p.link_visited = c;
            overrides.link_visited = Some(c);
        }
        _ => {}
    }
    Ok(())
}

fn apply_window(
    w: &mut WindowTheme,
    overrides: &mut WindowThemeOverrides,
    key: &str,
    val: &str,
    ln: usize,
) -> Result<(), ParseError> {
    match key {
        "titlebar_height" => {
            w.titlebar_height = parse_f32(val, ln)?;
            overrides.titlebar_height = Some(w.titlebar_height);
        }
        "titlebar_bg" => {
            w.titlebar_bg = parse_color(val, ln)?;
            overrides.titlebar_bg = Some(w.titlebar_bg);
        }
        "titlebar_bg_focused" => {
            w.titlebar_bg_focused = parse_color(val, ln)?;
            overrides.titlebar_bg_focused = Some(w.titlebar_bg_focused);
        }
        "titlebar_text" => {
            w.titlebar_text = parse_color(val, ln)?;
            overrides.titlebar_text = Some(w.titlebar_text);
        }
        "border_color" => {
            w.border_color = parse_color(val, ln)?;
            overrides.border_color = Some(w.border_color);
        }
        "border_color_focused" => {
            w.border_color_focused = parse_color(val, ln)?;
            overrides.border_color_focused = Some(w.border_color_focused);
        }
        "border_radius" => {
            w.border_radius = parse_f32(val, ln)?;
            overrides.border_radius = Some(w.border_radius);
        }
        "border_width" => {
            w.border_width = parse_f32(val, ln)?;
            overrides.border_width = Some(w.border_width);
        }
        "shadow_color" => {
            w.shadow_color = parse_color(val, ln)?;
            overrides.shadow_color = Some(w.shadow_color);
        }
        "content_bg" => {
            w.content_bg = parse_color(val, ln)?;
            overrides.content_bg = Some(w.content_bg);
        }
        "close_button_bg" => {
            w.close_button_bg = parse_color(val, ln)?;
            overrides.close_button_bg = Some(w.close_button_bg);
        }
        "control_button_bg" => {
            w.control_button_bg = parse_color(val, ln)?;
            overrides.control_button_bg = Some(w.control_button_bg);
        }
        _ => {}
    }
    Ok(())
}

fn apply_statusbar(
    s: &mut StatusBarTheme,
    overrides: &mut StatusBarThemeOverrides,
    key: &str,
    val: &str,
    ln: usize,
) -> Result<(), ParseError> {
    match key {
        "height" => {
            s.height = parse_f32(val, ln)?;
            overrides.height = Some(s.height);
        }
        "background" => {
            s.background = parse_color(val, ln)?;
            overrides.background = Some(s.background);
        }
        "text_color" => {
            s.text_color = parse_color(val, ln)?;
            overrides.text_color = Some(s.text_color);
        }
        "border_color" => {
            s.border_color = parse_color(val, ln)?;
            overrides.border_color = Some(s.border_color);
        }
        "padding_horizontal" => {
            s.padding_horizontal = parse_f32(val, ln)?;
            overrides.padding_horizontal = Some(s.padding_horizontal);
        }
        "font_size" => {
            s.font_size = parse_f32(val, ln)?;
            overrides.font_size = Some(s.font_size);
        }
        _ => {}
    }
    Ok(())
}

fn apply_dock(
    d: &mut DockTheme,
    overrides: &mut DockThemeOverrides,
    key: &str,
    val: &str,
    ln: usize,
) -> Result<(), ParseError> {
    match key {
        "height" => {
            d.height = parse_f32(val, ln)?;
            overrides.height = Some(d.height);
        }
        "item_size" => {
            d.item_size = parse_f32(val, ln)?;
            overrides.item_size = Some(d.item_size);
        }
        "spacing" => {
            d.spacing = parse_f32(val, ln)?;
            overrides.spacing = Some(d.spacing);
        }
        "background" => {
            d.background = parse_color(val, ln)?;
            overrides.background = Some(d.background);
        }
        "item_color" => {
            d.item_color = parse_color(val, ln)?;
            overrides.item_color = Some(d.item_color);
        }
        "item_active_color" => {
            d.item_active_color = parse_color(val, ln)?;
            overrides.item_active_color = Some(d.item_active_color);
        }
        "item_hover_bg" => {
            d.item_hover_bg = parse_color(val, ln)?;
            overrides.item_hover_bg = Some(d.item_hover_bg);
        }
        "item_border_radius" => {
            d.item_border_radius = parse_f32(val, ln)?;
            overrides.item_border_radius = Some(d.item_border_radius);
        }
        "indicator_color" => {
            d.indicator_color = parse_color(val, ln)?;
            overrides.indicator_color = Some(d.indicator_color);
        }
        "border_color" => {
            d.border_color = parse_color(val, ln)?;
            overrides.border_color = Some(d.border_color);
        }
        _ => {}
    }
    Ok(())
}

fn apply_menu(
    m: &mut MenuTheme,
    overrides: &mut MenuThemeOverrides,
    key: &str,
    val: &str,
    ln: usize,
) -> Result<(), ParseError> {
    match key {
        "item_height" => {
            m.item_height = parse_f32(val, ln)?;
            overrides.item_height = Some(m.item_height);
        }
        "padding" => {
            m.padding = parse_f32(val, ln)?;
            overrides.padding = Some(m.padding);
        }
        "background" => {
            m.background = parse_color(val, ln)?;
            overrides.background = Some(m.background);
        }
        "text_color" => {
            m.text_color = parse_color(val, ln)?;
            overrides.text_color = Some(m.text_color);
        }
        "hover_bg" => {
            m.hover_bg = parse_color(val, ln)?;
            overrides.hover_bg = Some(m.hover_bg);
        }
        "disabled_color" => {
            m.disabled_color = parse_color(val, ln)?;
            overrides.disabled_color = Some(m.disabled_color);
        }
        "border_color" => {
            m.border_color = parse_color(val, ln)?;
            overrides.border_color = Some(m.border_color);
        }
        "border_radius" => {
            m.border_radius = parse_f32(val, ln)?;
            overrides.border_radius = Some(m.border_radius);
        }
        "separator_color" => {
            m.separator_color = parse_color(val, ln)?;
            overrides.separator_color = Some(m.separator_color);
        }
        "shortcut_color" => {
            m.shortcut_color = parse_color(val, ln)?;
            overrides.shortcut_color = Some(m.shortcut_color);
        }
        "font_size" => {
            m.font_size = parse_f32(val, ln)?;
            overrides.font_size = Some(m.font_size);
        }
        _ => {}
    }
    Ok(())
}

fn apply_tooltip(
    t: &mut TooltipTheme,
    overrides: &mut TooltipThemeOverrides,
    key: &str,
    val: &str,
    ln: usize,
) -> Result<(), ParseError> {
    match key {
        "delay_ms" => {
            t.delay_ms = parse_u32(val, ln)?;
            overrides.delay_ms = Some(t.delay_ms);
        }
        "background" => {
            t.background = parse_color(val, ln)?;
            overrides.background = Some(t.background);
        }
        "text_color" => {
            t.text_color = parse_color(val, ln)?;
            overrides.text_color = Some(t.text_color);
        }
        "border_radius" => {
            t.border_radius = parse_f32(val, ln)?;
            overrides.border_radius = Some(t.border_radius);
        }
        "max_width" => {
            t.max_width = parse_f32(val, ln)?;
            overrides.max_width = Some(t.max_width);
        }
        "font_size" => {
            t.font_size = parse_f32(val, ln)?;
            overrides.font_size = Some(t.font_size);
        }
        "padding_horizontal" => {
            t.padding_horizontal = parse_f32(val, ln)?;
            overrides.padding_horizontal = Some(t.padding_horizontal);
        }
        "padding_vertical" => {
            t.padding_vertical = parse_f32(val, ln)?;
            overrides.padding_vertical = Some(t.padding_vertical);
        }
        _ => {}
    }
    Ok(())
}

fn apply_notification(
    n: &mut NotificationTheme,
    overrides: &mut NotificationThemeOverrides,
    key: &str,
    val: &str,
    ln: usize,
) -> Result<(), ParseError> {
    match key {
        "width" => {
            n.width = parse_f32(val, ln)?;
            overrides.width = Some(n.width);
        }
        "background" => {
            n.background = parse_color(val, ln)?;
            overrides.background = Some(n.background);
        }
        "title_color" => {
            n.title_color = parse_color(val, ln)?;
            overrides.title_color = Some(n.title_color);
        }
        "body_color" => {
            n.body_color = parse_color(val, ln)?;
            overrides.body_color = Some(n.body_color);
        }
        "border_radius" => {
            n.border_radius = parse_f32(val, ln)?;
            overrides.border_radius = Some(n.border_radius);
        }
        "spacing" => {
            n.spacing = parse_f32(val, ln)?;
            overrides.spacing = Some(n.spacing);
        }
        "padding" => {
            n.padding = parse_f32(val, ln)?;
            overrides.padding = Some(n.padding);
        }
        "action_bg" => {
            n.action_bg = parse_color(val, ln)?;
            overrides.action_bg = Some(n.action_bg);
        }
        "action_color" => {
            n.action_color = parse_color(val, ln)?;
            overrides.action_color = Some(n.action_color);
        }
        _ => {}
    }
    Ok(())
}

fn apply_glass(
    g: &mut GlassParams,
    overrides: &mut GlassParamsOverrides,
    key: &str,
    val: &str,
    ln: usize,
) -> Result<(), ParseError> {
    match key {
        "tint_color" => {
            g.tint_color = parse_color(val, ln)?;
            overrides.tint_color = Some(g.tint_color);
        }
        "blur_radius" => {
            g.blur_radius = parse_f32(val, ln)?;
            overrides.blur_radius = Some(g.blur_radius);
        }
        "saturation" => {
            g.saturation = parse_f32(val, ln)?;
            overrides.saturation = Some(g.saturation);
        }
        "opacity" => {
            g.opacity = parse_f32(val, ln)?;
            overrides.opacity = Some(g.opacity);
        }
        _ => {}
    }
    Ok(())
}
