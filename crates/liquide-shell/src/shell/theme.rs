//! Theme loading and CSS style resolver management.

use std::sync::Arc;

use liquide_renderer_css::StyleResolver;

use crate::theme::ShellTheme;
use crate::theme_loader;

use super::Shell;

impl Shell {
    /// Set the shell theme (also clears the style resolver since the
    /// theme is now disconnected from CSS).
    pub fn set_theme(&mut self, theme: ShellTheme) {
        self.theme = theme;
        self.style_resolver = None;
        self.sync_pipeline_color_scheme();
    }

    /// Build the default Night CSS theme and its style resolver.
    pub(crate) fn build_default_theme() -> (ShellTheme, StyleResolver) {
        use liquide_theme_css::ThemeParser;
        let parser = ThemeParser::new();
        match parser.parse_str(theme_loader::default_theme_css()) {
            Ok(stylesheet) => {
                let engine = Arc::new(liquide_theme_css::ThemeEngine::new(stylesheet));
                let theme = theme_loader::css_to_shell_theme(&engine);
                let resolver = StyleResolver::from_arc(Arc::clone(&engine));
                (theme, resolver)
            }
            Err(_) => {
                // Fallback: hardcoded dark theme with a dummy resolver
                let theme = ShellTheme::default_dark();
                let empty_engine = Arc::new(liquide_theme_css::ThemeEngine::new(
                    liquide_theme_css::StyleSheet::new(),
                ));
                let resolver = StyleResolver::from_arc(empty_engine);
                (theme, resolver)
            }
        }
    }

    /// Load a CSS theme from a file, keeping the engine alive for CSS queries.
    ///
    /// # Example
    /// ```rust,ignore
    /// shell.load_css_theme("themes/nord.css")?;
    /// ```
    pub fn load_css_theme<P: AsRef<std::path::Path>>(&mut self, path: P) {
        match theme_loader::load_css_theme_with_engine(path) {
            Ok((theme, engine)) => {
                self.theme = theme;
                self.style_resolver = Some(StyleResolver::from_arc(engine));
                self.sync_pipeline_color_scheme();
            }
            Err(e) => tracing::warn!("Failed to load CSS theme: {}", e),
        }
    }

    /// Load the default Nord CSS theme
    pub fn load_default_css_theme(&mut self) {
        let (theme, resolver) = Self::build_default_theme();
        self.theme = theme;
        self.style_resolver = Some(resolver);
        self.sync_pipeline_color_scheme();
    }

    pub(crate) fn preferred_color_scheme_for_theme(theme: &ShellTheme) -> &'static str {
        let bg = theme.desktop_background;
        let luminance = 0.2126 * bg.r as f32 + 0.7152 * bg.g as f32 + 0.0722 * bg.b as f32;
        if luminance < 128.0 { "dark" } else { "light" }
    }

    pub(crate) fn sync_pipeline_color_scheme(&mut self) {
        self.css_pipeline
            .set_preferred_color_scheme(Self::preferred_color_scheme_for_theme(&self.theme));
    }
}
