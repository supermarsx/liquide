//! Font preview — generate preview renderings with sample text.

use serde::{Deserialize, Serialize};

/// Supported sample text kinds for font preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreviewSample {
    /// Classic Lorem Ipsum paragraph.
    LoremIpsum,
    /// Quick brown fox pangram.
    Pangram,
    /// Full alphabet (upper + lower + digits + symbols).
    Alphabet,
    /// Custom user-provided text.
    Custom(String),
    /// Font name displayed in itself (meta-preview).
    FontName,
    /// Paragraph simulating UI text.
    UiSample,
    /// Code sample (for monospace fonts).
    CodeSample,
}

impl PreviewSample {
    /// Get the preview text content.
    #[must_use]
    pub fn text(&self, font_name: &str) -> String {
        match self {
            Self::LoremIpsum => LOREM_IPSUM.to_string(),
            Self::Pangram => "The quick brown fox jumps over the lazy dog.\n\
                              Pack my box with five dozen liquor jugs.\n\
                              How vexingly quick daft zebras jump!"
                .to_string(),
            Self::Alphabet => "ABCDEFGHIJKLMNOPQRSTUVWXYZ\n\
                               abcdefghijklmnopqrstuvwxyz\n\
                               0123456789 !@#$%^&*()_+-=[]{}|;':\",./<>?"
                .to_string(),
            Self::Custom(text) => text.clone(),
            Self::FontName => font_name.to_string(),
            Self::UiSample => format!(
                "Settings — Display\n\
                 Brightness: 75%\n\
                 Resolution: 2560 × 1440\n\
                 Scale: 100%\n\
                 \n\
                 {font_name}"
            ),
            Self::CodeSample => "fn main() {\n\
                                 \tlet greeting = \"Hello, LiquiDE!\";\n\
                                 \tprintln!(\"{}\", greeting);\n\
                                 \tfor i in 0..10 {\n\
                                 \t\tdbg!(i * i);\n\
                                 \t}\n\
                                 \t// 0O Il1| {[()]}\n\
                                 }"
            .to_string(),
        }
    }
}

/// Preview configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewConfig {
    /// Sample text to render.
    pub sample: PreviewSample,
    /// Font sizes to show (multiple for comparison).
    pub sizes: Vec<f32>,
    /// Whether to show all available weights.
    pub show_weights: bool,
    /// Whether to show italic variant.
    pub show_italic: bool,
    /// Background color for preview (hex string like "#1e1e2e").
    pub background: String,
    /// Text color for preview (hex string like "#cdd6f4").
    pub foreground: String,
    /// Line height multiplier.
    pub line_height: f32,
    /// Letter spacing in pixels.
    pub letter_spacing: f32,
}

impl Default for PreviewConfig {
    fn default() -> Self {
        Self {
            sample: PreviewSample::Pangram,
            sizes: vec![12.0, 14.0, 16.0, 20.0, 28.0, 36.0],
            show_weights: true,
            show_italic: false,
            background: "#1e1e2e".into(),
            foreground: "#cdd6f4".into(),
            line_height: 1.4,
            letter_spacing: 0.0,
        }
    }
}

/// Result of a font preview rendering.
#[derive(Debug, Clone)]
pub struct PreviewResult {
    /// Font family name.
    pub family: String,
    /// Size that was previewed.
    pub size: f32,
    /// Weight that was previewed.
    pub weight: u16,
    /// The sample text that was rendered.
    pub sample_text: String,
    /// Width of the rendered preview in pixels.
    pub width: u32,
    /// Height of the rendered preview in pixels.
    pub height: u32,
    /// RGBA pixel data for the preview.
    pub pixels: Vec<u8>,
}

const LOREM_IPSUM: &str = "\
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor \
incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis \
nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. \
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore \
eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt \
in culpa qui officia deserunt mollit anim id est laborum.";
