use crate::definition::{ThemeDefinition, ThemeMetadata, ThemeVariant};
use crate::parser::{ParsedTheme, ThemeOverrides};
use std::sync::Arc;

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

/// Callback invoked whenever [`ThemeManager`] regenerates CSS or switches the
/// active theme. Consumers (style engine, live-reload hooks) can subscribe to
/// re-inject the generated `:root` ruleset without polling. The callback sees
/// the resolved active theme rather than the raw registered child definition.
pub type ThemeChangeCallback = Arc<dyn Fn(&ThemeDefinition, &str) + Send + Sync>;

#[derive(Debug, Clone)]
struct RegisteredTheme {
    definition: ThemeDefinition,
    overrides: ThemeOverrides,
}

impl RegisteredTheme {
    fn explicit(definition: ThemeDefinition) -> Self {
        let overrides = ThemeOverrides::from_definition(&definition);
        Self {
            definition,
            overrides,
        }
    }

    fn parsed(parsed: ParsedTheme) -> Self {
        let (definition, overrides) = parsed.into_parts();
        Self {
            definition,
            overrides,
        }
    }
}

/// Runtime theme manager — register, switch, and resolve themes.
pub struct ThemeManager {
    themes: Vec<RegisteredTheme>,
    active_id: Option<String>,
    on_change: Vec<ThemeChangeCallback>,
    system_theme_variant: ThemeVariant,
}

impl ThemeManager {
    /// Create an empty manager with no themes registered.
    pub fn new() -> Self {
        Self {
            themes: Vec::new(),
            active_id: None,
            on_change: Vec::new(),
            system_theme_variant: ThemeVariant::Dark,
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

    /// Register a callback invoked whenever a theme regenerates CSS via
    /// [`Self::generate_active_css`] or the active theme changes via
    /// [`Self::set_active`]. The callback receives the active theme and the
    /// freshly generated `:root` CSS block.
    pub fn on_theme_change<F>(&mut self, callback: F)
    where
        F: Fn(&ThemeDefinition, &str) + Send + Sync + 'static,
    {
        self.on_change.push(Arc::new(callback));
    }

    fn fire_theme_change(&self, theme: &ThemeDefinition, css: &str) {
        for cb in &self.on_change {
            cb(theme, css);
        }
    }

    fn registered_theme(&self, id: &str) -> Option<&RegisteredTheme> {
        self.themes.iter().find(|theme| theme.definition.metadata.id == id)
    }

    fn refresh_active_theme(&self) {
        if let Ok(theme) = self.resolved_active_theme() {
            let css = Self::generate_css(&theme);
            self.fire_theme_change(&theme, &css);
        }
    }

    fn upsert_theme(&mut self, theme: RegisteredTheme) {
        let id = theme.definition.metadata.id.clone();
        let is_active = self.active_id.as_deref() == Some(id.as_str());
        if let Some(pos) = self
            .themes
            .iter()
            .position(|existing| existing.definition.metadata.id == id)
        {
            self.themes[pos] = theme;
        } else {
            self.themes.push(theme);
        }

        if is_active {
            self.refresh_active_theme();
        }
    }

    pub fn system_theme_variant(&self) -> ThemeVariant {
        self.system_theme_variant
    }

    /// Update the concrete system theme preference used to resolve
    /// [`ThemeVariant::Auto`] themes.
    pub fn set_system_theme_variant(&mut self, variant: ThemeVariant) {
        let resolved = variant.resolve_auto(ThemeVariant::Dark);
        if self.system_theme_variant != resolved {
            self.system_theme_variant = resolved;
            self.refresh_active_theme();
        }
    }

    /// Register (or re-register) a theme.  If a theme with the same `id`
    /// already exists it is replaced. The provided definition is treated as a
    /// fully materialized theme; every field is considered explicit when
    /// inheritance is resolved.
    pub fn register_theme(&mut self, definition: ThemeDefinition) {
        self.upsert_theme(RegisteredTheme::explicit(definition));
    }

    /// Register a parsed theme while preserving omitted-field information for
    /// inheritance-aware merges.
    pub fn register_parsed_theme(&mut self, parsed: ParsedTheme) {
        self.upsert_theme(RegisteredTheme::parsed(parsed));
    }

    /// Set the active theme by ID. Returns an error if the theme is not registered.
    pub fn set_active(&mut self, theme_id: &str) -> Result<(), ThemeError> {
        if self
            .themes
            .iter()
            .any(|theme| theme.definition.metadata.id == theme_id)
        {
            self.active_id = Some(theme_id.to_string());
            self.refresh_active_theme();
            Ok(())
        } else {
            Err(ThemeError::NotFound(theme_id.to_string()))
        }
    }

    /// Generate CSS for the currently active theme and notify any registered
    /// change callbacks. Useful as a hot-switch entry point after palette
    /// edits or runtime tweaks.
    pub fn generate_active_css(&self) -> Result<String, ThemeError> {
        let theme = self.resolved_active_theme()?;
        let css = Self::generate_css(&theme);
        self.fire_theme_change(&theme, &css);
        Ok(css)
    }

    /// Get the currently active registered theme definition without resolving
    /// inheritance.
    pub fn active_theme(&self) -> Result<&ThemeDefinition, ThemeError> {
        match &self.active_id {
            Some(id) => self
                .themes
                .iter()
                .find(|theme| theme.definition.metadata.id == *id)
                .map(|theme| &theme.definition)
                .ok_or_else(|| ThemeError::NotFound(id.clone())),
            None => Err(ThemeError::NoActiveTheme),
        }
    }

    /// Get the currently active theme after resolving inheritance and auto
    /// variant state.
    pub fn resolved_active_theme(&self) -> Result<ThemeDefinition, ThemeError> {
        let active = self.active_theme()?;
        self.resolve_inheritance(active)
    }

    /// Resolve a registered theme by ID.
    pub fn resolved_theme(&self, id: &str) -> Result<ThemeDefinition, ThemeError> {
        let theme = self
            .registered_theme(id)
            .map(|theme| &theme.definition)
            .ok_or_else(|| ThemeError::NotFound(id.to_string()))?;
        self.resolve_inheritance(theme)
    }

    /// List metadata for every registered theme.
    pub fn available_themes(&self) -> Vec<&ThemeMetadata> {
        self.themes
            .iter()
            .map(|theme| &theme.definition.metadata)
            .collect()
    }

    /// Look up a theme by id.
    pub fn get_theme(&self, id: &str) -> Option<&ThemeDefinition> {
        self.registered_theme(id).map(|theme| &theme.definition)
    }

    /// Resolve inheritance: if `theme` has a parent, merge the parent's values
    /// underneath the child's (child wins for every field that isn't the parent
    /// default).  Returns a fully resolved `ThemeDefinition` with `parent` set to `None`.
    ///
    /// This performs a shallow single-level merge — if the parent itself has a
    /// parent, that chain is resolved recursively (up to a depth limit of 16
    /// to guard against cycles).
    pub fn resolve_inheritance(
        &self,
        theme: &ThemeDefinition,
    ) -> Result<ThemeDefinition, ThemeError> {
        self.resolve_inner(theme, 0)
    }

    fn resolve_inner(
        &self,
        theme: &ThemeDefinition,
        depth: usize,
    ) -> Result<ThemeDefinition, ThemeError> {
        if depth > 16 {
            return Err(ThemeError::CircularInheritance(theme.metadata.id.clone()));
        }

        let parent_id = match &theme.metadata.parent {
            Some(id) => id.clone(),
            None => return Ok(self.normalize_theme(theme.clone())),
        };

        let parent = self
            .registered_theme(&parent_id)
            .map(|theme| &theme.definition)
            .ok_or_else(|| ThemeError::NotFound(parent_id.clone()))?;

        // Recursively resolve the parent first.
        let resolved_parent = self.resolve_inner(parent, depth + 1)?;

        let merged = if let Some(registered) = self.registered_theme(&theme.metadata.id) {
            merge_themes(&resolved_parent, theme, &registered.overrides)
        } else {
            let explicit = ThemeOverrides::from_definition(theme);
            merge_themes(&resolved_parent, theme, &explicit)
        };

        Ok(self.normalize_theme(merged))
    }

    fn normalize_theme(&self, mut theme: ThemeDefinition) -> ThemeDefinition {
        theme.metadata.variant = theme.metadata.variant.resolve_auto(self.system_theme_variant);
        theme
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

        let mut css = String::with_capacity(4096);
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
        write_var(
            &mut css,
            "color-text-primary",
            &p.text_primary.to_css_rgba(),
        );
        write_var(
            &mut css,
            "color-text-secondary",
            &p.text_secondary.to_css_rgba(),
        );
        write_var(
            &mut css,
            "color-text-disabled",
            &p.text_disabled.to_css_rgba(),
        );
        write_var(&mut css, "color-border", &p.border.to_css_rgba());
        write_var(&mut css, "color-divider", &p.divider.to_css_rgba());
        write_var(&mut css, "color-shadow", &p.shadow.to_css_rgba());
        write_var(
            &mut css,
            "color-selection-bg",
            &p.selection_bg.to_css_rgba(),
        );
        write_var(
            &mut css,
            "color-selection-fg",
            &p.selection_fg.to_css_rgba(),
        );
        write_var(&mut css, "color-link", &p.link.to_css_rgba());
        write_var(
            &mut css,
            "color-link-visited",
            &p.link_visited.to_css_rgba(),
        );

        // Window
        write_var(
            &mut css,
            "window-titlebar-height",
            &format!("{}px", w.titlebar_height),
        );
        write_var(&mut css, "window-titlebar-bg", &w.titlebar_bg.to_css_rgba());
        write_var(
            &mut css,
            "window-titlebar-bg-focused",
            &w.titlebar_bg_focused.to_css_rgba(),
        );
        write_var(
            &mut css,
            "window-titlebar-text",
            &w.titlebar_text.to_css_rgba(),
        );
        write_var(
            &mut css,
            "window-border-color",
            &w.border_color.to_css_rgba(),
        );
        write_var(
            &mut css,
            "window-border-color-focused",
            &w.border_color_focused.to_css_rgba(),
        );
        write_var(
            &mut css,
            "window-border-radius",
            &format!("{}px", w.border_radius),
        );
        write_var(
            &mut css,
            "window-border-width",
            &format!("{}px", w.border_width),
        );
        write_var(
            &mut css,
            "window-shadow-color",
            &w.shadow_color.to_css_rgba(),
        );
        write_var(&mut css, "window-content-bg", &w.content_bg.to_css_rgba());
        write_var(
            &mut css,
            "window-close-button-bg",
            &w.close_button_bg.to_css_rgba(),
        );
        write_var(
            &mut css,
            "window-control-button-bg",
            &w.control_button_bg.to_css_rgba(),
        );

        // Statusbar
        write_var(&mut css, "statusbar-height", &format!("{}px", s.height));
        write_var(&mut css, "statusbar-bg", &s.background.to_css_rgba());
        write_var(&mut css, "statusbar-text", &s.text_color.to_css_rgba());
        write_var(
            &mut css,
            "statusbar-border-color",
            &s.border_color.to_css_rgba(),
        );
        write_var(
            &mut css,
            "statusbar-padding-horizontal",
            &format!("{}px", s.padding_horizontal),
        );
        write_var(
            &mut css,
            "statusbar-font-size",
            &format!("{}px", s.font_size),
        );

        // Dock
        write_var(&mut css, "dock-height", &format!("{}px", d.height));
        write_var(&mut css, "dock-item-size", &format!("{}px", d.item_size));
        write_var(&mut css, "dock-spacing", &format!("{}px", d.spacing));
        write_var(&mut css, "dock-bg", &d.background.to_css_rgba());
        write_var(&mut css, "dock-item-color", &d.item_color.to_css_rgba());
        write_var(
            &mut css,
            "dock-item-active-color",
            &d.item_active_color.to_css_rgba(),
        );
        write_var(
            &mut css,
            "dock-item-hover-bg",
            &d.item_hover_bg.to_css_rgba(),
        );
        write_var(
            &mut css,
            "dock-item-border-radius",
            &format!("{}px", d.item_border_radius),
        );
        write_var(
            &mut css,
            "dock-indicator-color",
            &d.indicator_color.to_css_rgba(),
        );
        write_var(&mut css, "dock-border-color", &d.border_color.to_css_rgba());

        // Menu
        write_var(
            &mut css,
            "menu-item-height",
            &format!("{}px", m.item_height),
        );
        write_var(&mut css, "menu-padding", &format!("{}px", m.padding));
        write_var(&mut css, "menu-bg", &m.background.to_css_rgba());
        write_var(&mut css, "menu-text", &m.text_color.to_css_rgba());
        write_var(&mut css, "menu-hover-bg", &m.hover_bg.to_css_rgba());
        write_var(
            &mut css,
            "menu-disabled-color",
            &m.disabled_color.to_css_rgba(),
        );
        write_var(&mut css, "menu-border-color", &m.border_color.to_css_rgba());
        write_var(
            &mut css,
            "menu-border-radius",
            &format!("{}px", m.border_radius),
        );
        write_var(
            &mut css,
            "menu-separator-color",
            &m.separator_color.to_css_rgba(),
        );
        write_var(
            &mut css,
            "menu-shortcut-color",
            &m.shortcut_color.to_css_rgba(),
        );
        write_var(&mut css, "menu-font-size", &format!("{}px", m.font_size));

        // Tooltip
        write_var(&mut css, "tooltip-delay", &format!("{}ms", t.delay_ms));
        write_var(&mut css, "tooltip-bg", &t.background.to_css_rgba());
        write_var(&mut css, "tooltip-text", &t.text_color.to_css_rgba());
        write_var(
            &mut css,
            "tooltip-border-radius",
            &format!("{}px", t.border_radius),
        );
        write_var(&mut css, "tooltip-max-width", &format!("{}px", t.max_width));
        write_var(&mut css, "tooltip-font-size", &format!("{}px", t.font_size));
        write_var(
            &mut css,
            "tooltip-padding-horizontal",
            &format!("{}px", t.padding_horizontal),
        );
        write_var(
            &mut css,
            "tooltip-padding-vertical",
            &format!("{}px", t.padding_vertical),
        );

        // Notification
        write_var(&mut css, "notification-width", &format!("{}px", n.width));
        write_var(&mut css, "notification-bg", &n.background.to_css_rgba());
        write_var(
            &mut css,
            "notification-title-color",
            &n.title_color.to_css_rgba(),
        );
        write_var(
            &mut css,
            "notification-body-color",
            &n.body_color.to_css_rgba(),
        );
        write_var(
            &mut css,
            "notification-border-radius",
            &format!("{}px", n.border_radius),
        );
        write_var(&mut css, "notification-spacing", &format!("{}px", n.spacing));
        write_var(&mut css, "notification-padding", &format!("{}px", n.padding));
        write_var(
            &mut css,
            "notification-action-bg",
            &n.action_bg.to_css_rgba(),
        );
        write_var(
            &mut css,
            "notification-action-color",
            &n.action_color.to_css_rgba(),
        );

        // Glass
        write_var(&mut css, "glass-tint", &g.tint_color.to_css_rgba());
        write_var(
            &mut css,
            "glass-blur-radius",
            &format!("{}px", g.blur_radius),
        );
        write_var(
            &mut css,
            "glass-saturation",
            &format!("{:.2}", g.saturation),
        );
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
// Theme merging — parsed themes carry explicit-field tracking, while direct
// `ThemeDefinition` registration is treated as a fully explicit definition.
// ---------------------------------------------------------------------------
macro_rules! apply_override {
    ($target:expr, $overrides:expr, $field:ident) => {
        if let Some(value) = &$overrides.$field {
            $target.$field = value.clone();
        }
    };
}

fn merge_themes(
    parent: &ThemeDefinition,
    child: &ThemeDefinition,
    overrides: &ThemeOverrides,
) -> ThemeDefinition {
    let mut merged = parent.clone();
    merged.metadata = ThemeMetadata {
        id: child.metadata.id.clone(),
        name: child.metadata.name.clone(),
        author: parent.metadata.author.clone(),
        version: parent.metadata.version.clone(),
        description: parent.metadata.description.clone(),
        variant: parent.metadata.variant,
        parent: None,
        supports_glass: parent.metadata.supports_glass,
    };

    apply_override!(merged.metadata, overrides.metadata, author);
    apply_override!(merged.metadata, overrides.metadata, version);
    apply_override!(merged.metadata, overrides.metadata, description);
    apply_override!(merged.metadata, overrides.metadata, variant);
    apply_override!(merged.metadata, overrides.metadata, supports_glass);

    apply_override!(merged.palette, overrides.palette, primary);
    apply_override!(merged.palette, overrides.palette, secondary);
    apply_override!(merged.palette, overrides.palette, accent);
    apply_override!(merged.palette, overrides.palette, background);
    apply_override!(merged.palette, overrides.palette, surface);
    apply_override!(merged.palette, overrides.palette, error);
    apply_override!(merged.palette, overrides.palette, warning);
    apply_override!(merged.palette, overrides.palette, success);
    apply_override!(merged.palette, overrides.palette, info);
    apply_override!(merged.palette, overrides.palette, text_primary);
    apply_override!(merged.palette, overrides.palette, text_secondary);
    apply_override!(merged.palette, overrides.palette, text_disabled);
    apply_override!(merged.palette, overrides.palette, border);
    apply_override!(merged.palette, overrides.palette, divider);
    apply_override!(merged.palette, overrides.palette, shadow);
    apply_override!(merged.palette, overrides.palette, selection_bg);
    apply_override!(merged.palette, overrides.palette, selection_fg);
    apply_override!(merged.palette, overrides.palette, link);
    apply_override!(merged.palette, overrides.palette, link_visited);

    apply_override!(merged.window, overrides.window, titlebar_height);
    apply_override!(merged.window, overrides.window, titlebar_bg);
    apply_override!(merged.window, overrides.window, titlebar_bg_focused);
    apply_override!(merged.window, overrides.window, titlebar_text);
    apply_override!(merged.window, overrides.window, border_color);
    apply_override!(merged.window, overrides.window, border_color_focused);
    apply_override!(merged.window, overrides.window, border_radius);
    apply_override!(merged.window, overrides.window, border_width);
    apply_override!(merged.window, overrides.window, shadow_color);
    apply_override!(merged.window, overrides.window, content_bg);
    apply_override!(merged.window, overrides.window, close_button_bg);
    apply_override!(merged.window, overrides.window, control_button_bg);

    apply_override!(merged.statusbar, overrides.statusbar, height);
    apply_override!(merged.statusbar, overrides.statusbar, background);
    apply_override!(merged.statusbar, overrides.statusbar, text_color);
    apply_override!(merged.statusbar, overrides.statusbar, border_color);
    apply_override!(merged.statusbar, overrides.statusbar, padding_horizontal);
    apply_override!(merged.statusbar, overrides.statusbar, font_size);

    apply_override!(merged.dock, overrides.dock, height);
    apply_override!(merged.dock, overrides.dock, item_size);
    apply_override!(merged.dock, overrides.dock, spacing);
    apply_override!(merged.dock, overrides.dock, background);
    apply_override!(merged.dock, overrides.dock, item_color);
    apply_override!(merged.dock, overrides.dock, item_active_color);
    apply_override!(merged.dock, overrides.dock, item_hover_bg);
    apply_override!(merged.dock, overrides.dock, item_border_radius);
    apply_override!(merged.dock, overrides.dock, indicator_color);
    apply_override!(merged.dock, overrides.dock, border_color);

    apply_override!(merged.menu, overrides.menu, item_height);
    apply_override!(merged.menu, overrides.menu, padding);
    apply_override!(merged.menu, overrides.menu, background);
    apply_override!(merged.menu, overrides.menu, text_color);
    apply_override!(merged.menu, overrides.menu, hover_bg);
    apply_override!(merged.menu, overrides.menu, disabled_color);
    apply_override!(merged.menu, overrides.menu, border_color);
    apply_override!(merged.menu, overrides.menu, border_radius);
    apply_override!(merged.menu, overrides.menu, separator_color);
    apply_override!(merged.menu, overrides.menu, shortcut_color);
    apply_override!(merged.menu, overrides.menu, font_size);

    apply_override!(merged.tooltip, overrides.tooltip, delay_ms);
    apply_override!(merged.tooltip, overrides.tooltip, background);
    apply_override!(merged.tooltip, overrides.tooltip, text_color);
    apply_override!(merged.tooltip, overrides.tooltip, border_radius);
    apply_override!(merged.tooltip, overrides.tooltip, max_width);
    apply_override!(merged.tooltip, overrides.tooltip, font_size);
    apply_override!(merged.tooltip, overrides.tooltip, padding_horizontal);
    apply_override!(merged.tooltip, overrides.tooltip, padding_vertical);

    apply_override!(merged.notification, overrides.notification, width);
    apply_override!(merged.notification, overrides.notification, background);
    apply_override!(merged.notification, overrides.notification, title_color);
    apply_override!(merged.notification, overrides.notification, body_color);
    apply_override!(merged.notification, overrides.notification, border_radius);
    apply_override!(merged.notification, overrides.notification, spacing);
    apply_override!(merged.notification, overrides.notification, padding);
    apply_override!(merged.notification, overrides.notification, action_bg);
    apply_override!(merged.notification, overrides.notification, action_color);

    apply_override!(merged.glass, overrides.glass, tint_color);
    apply_override!(merged.glass, overrides.glass, blur_radius);
    apply_override!(merged.glass, overrides.glass, saturation);
    apply_override!(merged.glass, overrides.glass, opacity);

    merged
}
