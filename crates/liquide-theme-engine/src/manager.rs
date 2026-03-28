use crate::color::Color;
use crate::definition::{ThemeDefinition, ThemeMetadata};
use crate::palette::ColorPalette;

/// Errors that can occur during theme management.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeError {
    /// The requested theme ID was not registered.
    NotFound(String),
    /// Circular inheritance detected among themes.
    CircularInheritance(String),
    /// No active theme is set.
    NoActiveTheme,
}

impl core::fmt::Display for ThemeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "theme not found: {id}"),
            Self::CircularInheritance(id) => write!(f, "circular inheritance from: {id}"),
            Self::NoActiveTheme => write!(f, "no active theme set"),
        }
    }
}

/// Runtime theme manager — register, switch, and resolve themes.
pub struct ThemeManager {
    themes: Vec<ThemeDefinition>,
    active_id: Option<String>,
}

impl ThemeManager {
    /// Create an empty manager with no themes registered.
    pub fn new() -> Self {
        Self {
            themes: Vec::new(),
            active_id: None,
        }
    }

    /// Create a manager pre-loaded with all four built-in themes.
    /// The first theme (`night`) is set as active.
    pub fn with_builtins() -> Self {
        let mut mgr = Self::new();
        mgr.register_theme(crate::builtin::builtin_night());
        mgr.register_theme(crate::builtin::builtin_midday());
        mgr.register_theme(crate::builtin::builtin_sunset());
        mgr.register_theme(crate::builtin::builtin_liquid_glass());
        mgr.active_id = Some("night".into());
        mgr
    }

    /// Register (or re-register) a theme.  If a theme with the same `id`
    /// already exists it is replaced.
    pub fn register_theme(&mut self, definition: ThemeDefinition) {
        let id = definition.metadata.id.clone();
        if let Some(pos) = self.themes.iter().position(|t| t.metadata.id == id) {
            self.themes[pos] = definition;
        } else {
            self.themes.push(definition);
        }
    }

    /// Set the active theme by ID. Returns an error if the theme is not registered.
    pub fn set_active(&mut self, theme_id: &str) -> Result<(), ThemeError> {
        if self.themes.iter().any(|t| t.metadata.id == theme_id) {
            self.active_id = Some(theme_id.to_string());
            Ok(())
        } else {
            Err(ThemeError::NotFound(theme_id.to_string()))
        }
    }

    /// Get the currently active theme.
    pub fn active_theme(&self) -> Result<&ThemeDefinition, ThemeError> {
        match &self.active_id {
            Some(id) => self
                .themes
                .iter()
                .find(|t| t.metadata.id == *id)
                .ok_or_else(|| ThemeError::NotFound(id.clone())),
            None => Err(ThemeError::NoActiveTheme),
        }
    }

    /// List metadata for every registered theme.
    pub fn available_themes(&self) -> Vec<&ThemeMetadata> {
        self.themes.iter().map(|t| &t.metadata).collect()
    }

    /// Look up a theme by id.
    pub fn get_theme(&self, id: &str) -> Option<&ThemeDefinition> {
        self.themes.iter().find(|t| t.metadata.id == id)
    }

    /// Resolve inheritance: if `theme` has a parent, merge the parent's values
    /// underneath the child's (child wins for every field that isn't the parent
    /// default).  Returns a fully resolved `ThemeDefinition` with `parent` set to `None`.
    ///
    /// This performs a shallow single-level merge — if the parent itself has a
    /// parent, that chain is resolved recursively (up to a depth limit of 16
    /// to guard against cycles).
    pub fn resolve_inheritance(&self, theme: &ThemeDefinition) -> Result<ThemeDefinition, ThemeError> {
        self.resolve_inner(theme, 0)
    }

    fn resolve_inner(&self, theme: &ThemeDefinition, depth: usize) -> Result<ThemeDefinition, ThemeError> {
        if depth > 16 {
            return Err(ThemeError::CircularInheritance(theme.metadata.id.clone()));
        }

        let parent_id = match &theme.metadata.parent {
            Some(id) => id.clone(),
            None => return Ok(theme.clone()),
        };

        let parent = self
            .themes
            .iter()
            .find(|t| t.metadata.id == parent_id)
            .ok_or_else(|| ThemeError::NotFound(parent_id.clone()))?;

        // Recursively resolve the parent first.
        let resolved_parent = self.resolve_inner(parent, depth + 1)?;

        Ok(merge_themes(&resolved_parent, theme))
    }

    /// Generate a CSS custom-property block (`--variable: value;`) from a
    /// theme definition.  The output is a single ruleset scoped to `:root`.
    pub fn generate_css(theme: &ThemeDefinition) -> String {
        let p = &theme.palette;
        let w = &theme.window;
        let s = &theme.statusbar;
        let d = &theme.dock;
        let m = &theme.menu;
        let t = &theme.tooltip;
        let n = &theme.notification;
        let g = &theme.glass;

        let mut css = String::with_capacity(2048);
        css.push_str(":root {\n");

        // Palette
        write_var(&mut css, "color-primary", &p.primary.to_css_rgba());
        write_var(&mut css, "color-secondary", &p.secondary.to_css_rgba());
        write_var(&mut css, "color-accent", &p.accent.to_css_rgba());
        write_var(&mut css, "color-background", &p.background.to_css_rgba());
        write_var(&mut css, "color-surface", &p.surface.to_css_rgba());
        write_var(&mut css, "color-error", &p.error.to_css_rgba());
        write_var(&mut css, "color-warning", &p.warning.to_css_rgba());
        write_var(&mut css, "color-success", &p.success.to_css_rgba());
        write_var(&mut css, "color-info", &p.info.to_css_rgba());
        write_var(&mut css, "color-text-primary", &p.text_primary.to_css_rgba());
        write_var(&mut css, "color-text-secondary", &p.text_secondary.to_css_rgba());
        write_var(&mut css, "color-text-disabled", &p.text_disabled.to_css_rgba());
        write_var(&mut css, "color-border", &p.border.to_css_rgba());
        write_var(&mut css, "color-divider", &p.divider.to_css_rgba());
        write_var(&mut css, "color-shadow", &p.shadow.to_css_rgba());
        write_var(&mut css, "color-selection-bg", &p.selection_bg.to_css_rgba());
        write_var(&mut css, "color-selection-fg", &p.selection_fg.to_css_rgba());
        write_var(&mut css, "color-link", &p.link.to_css_rgba());
        write_var(&mut css, "color-link-visited", &p.link_visited.to_css_rgba());

        // Window
        write_var(&mut css, "window-titlebar-height", &format!("{}px", w.titlebar_height));
        write_var(&mut css, "window-titlebar-bg", &w.titlebar_bg.to_css_rgba());
        write_var(&mut css, "window-titlebar-bg-focused", &w.titlebar_bg_focused.to_css_rgba());
        write_var(&mut css, "window-titlebar-text", &w.titlebar_text.to_css_rgba());
        write_var(&mut css, "window-border-color", &w.border_color.to_css_rgba());
        write_var(&mut css, "window-border-color-focused", &w.border_color_focused.to_css_rgba());
        write_var(&mut css, "window-border-radius", &format!("{}px", w.border_radius));
        write_var(&mut css, "window-shadow-color", &w.shadow_color.to_css_rgba());
        write_var(&mut css, "window-content-bg", &w.content_bg.to_css_rgba());

        // Statusbar
        write_var(&mut css, "statusbar-height", &format!("{}px", s.height));
        write_var(&mut css, "statusbar-bg", &s.background.to_css_rgba());
        write_var(&mut css, "statusbar-text", &s.text_color.to_css_rgba());

        // Dock
        write_var(&mut css, "dock-height", &format!("{}px", d.height));
        write_var(&mut css, "dock-item-size", &format!("{}px", d.item_size));
        write_var(&mut css, "dock-bg", &d.background.to_css_rgba());
        write_var(&mut css, "dock-item-color", &d.item_color.to_css_rgba());
        write_var(&mut css, "dock-indicator-color", &d.indicator_color.to_css_rgba());

        // Menu
        write_var(&mut css, "menu-item-height", &format!("{}px", m.item_height));
        write_var(&mut css, "menu-bg", &m.background.to_css_rgba());
        write_var(&mut css, "menu-text", &m.text_color.to_css_rgba());
        write_var(&mut css, "menu-hover-bg", &m.hover_bg.to_css_rgba());
        write_var(&mut css, "menu-border-radius", &format!("{}px", m.border_radius));

        // Tooltip
        write_var(&mut css, "tooltip-delay", &format!("{}ms", t.delay_ms));
        write_var(&mut css, "tooltip-bg", &t.background.to_css_rgba());
        write_var(&mut css, "tooltip-text", &t.text_color.to_css_rgba());
        write_var(&mut css, "tooltip-border-radius", &format!("{}px", t.border_radius));
        write_var(&mut css, "tooltip-max-width", &format!("{}px", t.max_width));

        // Notification
        write_var(&mut css, "notification-width", &format!("{}px", n.width));
        write_var(&mut css, "notification-bg", &n.background.to_css_rgba());
        write_var(&mut css, "notification-title-color", &n.title_color.to_css_rgba());
        write_var(&mut css, "notification-body-color", &n.body_color.to_css_rgba());
        write_var(&mut css, "notification-border-radius", &format!("{}px", n.border_radius));

        // Glass
        write_var(&mut css, "glass-tint", &g.tint_color.to_css_rgba());
        write_var(&mut css, "glass-blur-radius", &format!("{}px", g.blur_radius));
        write_var(&mut css, "glass-saturation", &format!("{:.2}", g.saturation));
        write_var(&mut css, "glass-opacity", &format!("{:.2}", g.opacity));

        css.push_str("}\n");
        css
    }
}

impl Default for ThemeManager {
    fn default() -> Self {
        Self::new()
    }
}

fn write_var(css: &mut String, name: &str, value: &str) {
    css.push_str("    --");
    css.push_str(name);
    css.push_str(": ");
    css.push_str(value);
    css.push_str(";\n");
}

// ---------------------------------------------------------------------------
// Theme merging — child values override parent for every field.
//
// Because we don't have a sentinel "unset" per-field, the convention is:
// child always wins. The parent provides the base that gets fully overridden.
// This is appropriate for the "override parent" model: the child theme is
// expected to supply any values it wants to differ; everything else comes from
// the parent.
// ---------------------------------------------------------------------------
fn merge_themes(parent: &ThemeDefinition, child: &ThemeDefinition) -> ThemeDefinition {
    ThemeDefinition {
        metadata: ThemeMetadata {
            id: child.metadata.id.clone(),
            name: child.metadata.name.clone(),
            author: if child.metadata.author.is_empty() {
                parent.metadata.author.clone()
            } else {
                child.metadata.author.clone()
            },
            version: child.metadata.version.clone(),
            description: if child.metadata.description.is_empty() {
                parent.metadata.description.clone()
            } else {
                child.metadata.description.clone()
            },
            variant: child.metadata.variant,
            parent: None, // resolved
            supports_glass: child.metadata.supports_glass,
        },
        palette: merge_palette(&parent.palette, &child.palette),
        window: child.window.clone(),
        statusbar: child.statusbar.clone(),
        dock: child.dock.clone(),
        menu: child.menu.clone(),
        tooltip: child.tooltip.clone(),
        notification: child.notification.clone(),
        glass: child.glass.clone(),
    }
}

/// Palette merge: child field wins when it differs from the default palette.
/// If the child field equals the default, we assume the child didn't override it
/// and keep the parent value.
fn merge_palette(parent: &ColorPalette, child: &ColorPalette) -> ColorPalette {
    let def = ColorPalette::default();
    let pick = |c: &Color, p: &Color, d: &Color| -> Color {
        if c != d { *c } else { *p }
    };
    ColorPalette {
        primary: pick(&child.primary, &parent.primary, &def.primary),
        secondary: pick(&child.secondary, &parent.secondary, &def.secondary),
        accent: pick(&child.accent, &parent.accent, &def.accent),
        background: pick(&child.background, &parent.background, &def.background),
        surface: pick(&child.surface, &parent.surface, &def.surface),
        error: pick(&child.error, &parent.error, &def.error),
        warning: pick(&child.warning, &parent.warning, &def.warning),
        success: pick(&child.success, &parent.success, &def.success),
        info: pick(&child.info, &parent.info, &def.info),
        text_primary: pick(&child.text_primary, &parent.text_primary, &def.text_primary),
        text_secondary: pick(&child.text_secondary, &parent.text_secondary, &def.text_secondary),
        text_disabled: pick(&child.text_disabled, &parent.text_disabled, &def.text_disabled),
        border: pick(&child.border, &parent.border, &def.border),
        divider: pick(&child.divider, &parent.divider, &def.divider),
        shadow: pick(&child.shadow, &parent.shadow, &def.shadow),
        selection_bg: pick(&child.selection_bg, &parent.selection_bg, &def.selection_bg),
        selection_fg: pick(&child.selection_fg, &parent.selection_fg, &def.selection_fg),
        link: pick(&child.link, &parent.link, &def.link),
        link_visited: pick(&child.link_visited, &parent.link_visited, &def.link_visited),
    }
}
