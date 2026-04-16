//! Font role assignments and font stacks.
//!
//! Each UI context (primary UI, terminal, titles, data-dense areas,
//! accessibility, emoji) gets its own font stack with ordered fallbacks.

use serde::{Deserialize, Serialize};

/// A role that determines which font stack to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FontRole {
    /// Primary sans-serif for general UI (buttons, labels, menus).
    PrimaryUi,
    /// Display / branding font for window titles, headings.
    Display,
    /// Monospace font for terminals, code editors, logs.
    Terminal,
    /// Dense-data font for tables, metrics, small controls.
    DataDense,
    /// Accessibility-focused font with wide Unicode coverage.
    Accessibility,
    /// Color emoji font.
    Emoji,
    /// Status bar text.
    StatusBar,
    /// Dock labels.
    Dock,
    /// Window title bars.
    WindowTitle,
    /// Notification body text.
    Notification,
    /// Launcher search / results.
    Launcher,
}

impl std::fmt::Display for FontRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PrimaryUi => write!(f, "Primary UI"),
            Self::Display => write!(f, "Display/Brand"),
            Self::Terminal => write!(f, "Terminal/Code"),
            Self::DataDense => write!(f, "Data/Dense UI"),
            Self::Accessibility => write!(f, "Accessibility"),
            Self::Emoji => write!(f, "Emoji"),
            Self::StatusBar => write!(f, "Status Bar"),
            Self::Dock => write!(f, "Dock"),
            Self::WindowTitle => write!(f, "Window Title"),
            Self::Notification => write!(f, "Notification"),
            Self::Launcher => write!(f, "Launcher"),
        }
    }
}

/// An ordered list of font families to try for a given role.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FontStack {
    /// The role this stack applies to.
    pub role: FontRole,
    /// Ordered list of font family names (first match wins).
    pub families: Vec<String>,
    /// Base size in logical pixels.
    pub size: f32,
    /// Weight (100–900, 400 = Regular, 700 = Bold).
    pub weight: u16,
    /// Letter-spacing adjustment in pixels (positive = looser, negative = tighter).
    pub letter_spacing: f32,
    /// Line-height multiplier (1.0 = tight, 1.5 = normal).
    pub line_height: f32,
    /// Whether to enable subpixel antialiasing.
    pub subpixel_aa: bool,
    /// Whether to enable font hinting.
    pub hinting: bool,
}

impl FontStack {
    /// Create a new font stack for a role with a single family.
    #[must_use]
    pub fn new(role: FontRole, families: Vec<String>, size: f32) -> Self {
        Self {
            role,
            families,
            size,
            weight: 400,
            letter_spacing: 0.0,
            line_height: 1.4,
            subpixel_aa: true,
            hinting: true,
        }
    }

    /// Builder: set weight.
    #[must_use]
    pub fn with_weight(mut self, weight: u16) -> Self {
        self.weight = weight;
        self
    }

    /// Builder: set letter spacing.
    #[must_use]
    pub fn with_letter_spacing(mut self, spacing: f32) -> Self {
        self.letter_spacing = spacing;
        self
    }

    /// Builder: set line height.
    #[must_use]
    pub fn with_line_height(mut self, line_height: f32) -> Self {
        self.line_height = line_height;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_stack_defaults() {
        let stack = FontStack::new(
            FontRole::PrimaryUi,
            vec!["Manrope".into()],
            14.0,
        );
        assert_eq!(stack.weight, 400);
        assert!((stack.letter_spacing - 0.0).abs() < f32::EPSILON);
        assert!((stack.line_height - 1.4).abs() < f32::EPSILON);
        assert!(stack.subpixel_aa);
        assert!(stack.hinting);
    }

    #[test]
    fn font_stack_builder_chain() {
        let stack = FontStack::new(FontRole::Terminal, vec!["JetBrains Mono".into()], 13.0)
            .with_weight(700)
            .with_letter_spacing(-0.5)
            .with_line_height(1.2);

        assert_eq!(stack.weight, 700);
        assert!((stack.letter_spacing - (-0.5)).abs() < f32::EPSILON);
        assert!((stack.line_height - 1.2).abs() < f32::EPSILON);
    }

    #[test]
    fn font_role_display() {
        assert_eq!(FontRole::PrimaryUi.to_string(), "Primary UI");
        assert_eq!(FontRole::Terminal.to_string(), "Terminal/Code");
        assert_eq!(FontRole::Emoji.to_string(), "Emoji");
    }

    #[test]
    fn font_stack_multiple_families_fallback() {
        let stack = FontStack::new(
            FontRole::PrimaryUi,
            vec!["Manrope".into(), "Inter".into(), "sans-serif".into()],
            14.0,
        );
        assert_eq!(stack.families.len(), 3);
        assert_eq!(stack.families[0], "Manrope");
        assert_eq!(stack.families[2], "sans-serif");
    }
}
