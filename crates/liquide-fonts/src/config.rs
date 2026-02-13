//! Font configuration — central definition of all font settings.
//!
//! Includes per-role font stacks, global rendering options, directory
//! watch lists, and Google Fonts integration settings.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::google_fonts::GoogleFontsConfig;
use crate::roles::{FontRole, FontStack};

/// Top-level font configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontConfig {
    /// Per-role font stack assignments.
    pub roles: HashMap<FontRole, FontStack>,

    /// Global font rendering settings.
    pub rendering: FontRenderingConfig,

    /// Directories to watch for font changes (hot reload).
    pub watch_dirs: Vec<PathBuf>,

    /// Font installation directory.
    pub install_dir: PathBuf,

    /// Google Fonts integration settings.
    pub google_fonts: GoogleFontsConfig,

    /// Auto-activation: automatically activate fonts when an app requests them.
    pub auto_activate: bool,

    /// Whether to enable drag-and-drop font installation.
    pub drag_drop_install: bool,

    /// URL import safety: list of allowed domains for URL-based font import.
    pub allowed_import_domains: Vec<String>,
}

/// Global font rendering settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontRenderingConfig {
    /// Enable subpixel antialiasing globally.
    pub subpixel_aa: bool,
    /// Enable font hinting globally.
    pub hinting: bool,
    /// Hint style: "none", "slight", "medium", "full".
    pub hint_style: String,
    /// LCD filter for subpixel rendering: "none", "default", "light".
    pub lcd_filter: String,
    /// Global DPI scaling factor.
    pub dpi_scale: f32,
    /// Minimum font size in pixels (prevents unreadable small text).
    pub min_size: f32,
    /// Maximum font size in pixels (prevents absurdly large rendering).
    pub max_size: f32,
}

impl Default for FontRenderingConfig {
    fn default() -> Self {
        Self {
            subpixel_aa: true,
            hinting: true,
            hint_style: "slight".into(),
            lcd_filter: "default".into(),
            dpi_scale: 1.0,
            min_size: 6.0,
            max_size: 200.0,
        }
    }
}

impl FontConfig {
    /// Get the font stack for a given role.
    #[must_use]
    pub fn stack_for_role(&self, role: FontRole) -> &FontStack {
        self.roles.get(&role).unwrap_or_else(|| {
            // Fallback to PrimaryUi if the specific role isn't configured.
            self.roles
                .get(&FontRole::PrimaryUi)
                .expect("PrimaryUi font stack must always be configured")
        })
    }

    /// Set or update a font stack for a role.
    pub fn set_stack(&mut self, stack: FontStack) {
        self.roles.insert(stack.role, stack);
    }

    /// Get all configured roles.
    #[must_use]
    pub fn configured_roles(&self) -> Vec<FontRole> {
        self.roles.keys().copied().collect()
    }
}

impl Default for FontConfig {
    fn default() -> Self {
        let mut roles = HashMap::new();

        // Primary UI: Manrope
        roles.insert(
            FontRole::PrimaryUi,
            FontStack::new(
                FontRole::PrimaryUi,
                vec![
                    "Manrope".into(),
                    "Inter".into(),
                    "Noto Sans".into(),
                    "sans-serif".into(),
                ],
                14.0,
            )
            .with_weight(400)
            .with_letter_spacing(-0.2)
            .with_line_height(1.4),
        );

        // Display / Brand: Space Grotesk
        roles.insert(
            FontRole::Display,
            FontStack::new(
                FontRole::Display,
                vec![
                    "Space Grotesk".into(),
                    "Manrope".into(),
                    "Inter".into(),
                    "sans-serif".into(),
                ],
                20.0,
            )
            .with_weight(600)
            .with_letter_spacing(-0.5)
            .with_line_height(1.2),
        );

        // Terminal / Code: JetBrains Mono
        roles.insert(
            FontRole::Terminal,
            FontStack::new(
                FontRole::Terminal,
                vec![
                    "JetBrains Mono".into(),
                    "Fira Code".into(),
                    "Cascadia Code".into(),
                    "monospace".into(),
                ],
                13.0,
            )
            .with_weight(400)
            .with_letter_spacing(0.0)
            .with_line_height(1.5),
        );

        // Data / Dense UI: Inter
        roles.insert(
            FontRole::DataDense,
            FontStack::new(
                FontRole::DataDense,
                vec![
                    "Inter".into(),
                    "Manrope".into(),
                    "Noto Sans".into(),
                    "sans-serif".into(),
                ],
                12.0,
            )
            .with_weight(400)
            .with_letter_spacing(0.0)
            .with_line_height(1.3),
        );

        // Accessibility: Noto Sans (wide Unicode coverage)
        roles.insert(
            FontRole::Accessibility,
            FontStack::new(
                FontRole::Accessibility,
                vec![
                    "Noto Sans".into(),
                    "Inter".into(),
                    "sans-serif".into(),
                ],
                16.0,
            )
            .with_weight(400)
            .with_line_height(1.5),
        );

        // Emoji: Noto Color Emoji
        roles.insert(
            FontRole::Emoji,
            FontStack::new(
                FontRole::Emoji,
                vec![
                    "Noto Color Emoji".into(),
                    "Segoe UI Emoji".into(),
                    "Apple Color Emoji".into(),
                    "emoji".into(),
                ],
                14.0,
            ),
        );

        // Status bar inherits primary UI by default
        roles.insert(
            FontRole::StatusBar,
            FontStack::new(
                FontRole::StatusBar,
                vec![
                    "Manrope".into(),
                    "Inter".into(),
                    "sans-serif".into(),
                ],
                12.0,
            )
            .with_weight(500)
            .with_letter_spacing(-0.1),
        );

        // Dock labels
        roles.insert(
            FontRole::Dock,
            FontStack::new(
                FontRole::Dock,
                vec![
                    "Manrope".into(),
                    "Inter".into(),
                    "sans-serif".into(),
                ],
                11.0,
            )
            .with_weight(500),
        );

        // Window title bars: Space Grotesk (brand heading)
        roles.insert(
            FontRole::WindowTitle,
            FontStack::new(
                FontRole::WindowTitle,
                vec![
                    "Space Grotesk".into(),
                    "Manrope".into(),
                    "sans-serif".into(),
                ],
                13.0,
            )
            .with_weight(600)
            .with_letter_spacing(-0.3),
        );

        // Notifications
        roles.insert(
            FontRole::Notification,
            FontStack::new(
                FontRole::Notification,
                vec![
                    "Manrope".into(),
                    "Inter".into(),
                    "sans-serif".into(),
                ],
                13.0,
            )
            .with_weight(400),
        );

        // Launcher
        roles.insert(
            FontRole::Launcher,
            FontStack::new(
                FontRole::Launcher,
                vec![
                    "Manrope".into(),
                    "Inter".into(),
                    "sans-serif".into(),
                ],
                15.0,
            )
            .with_weight(400),
        );

        let install_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("liquide")
            .join("fonts");

        let watch_dirs = vec![
            install_dir.clone(),
            // System font directories
            #[cfg(target_os = "windows")]
            PathBuf::from(r"C:\Windows\Fonts"),
            #[cfg(target_os = "linux")]
            PathBuf::from("/usr/share/fonts"),
            #[cfg(target_os = "macos")]
            PathBuf::from("/Library/Fonts"),
        ];

        Self {
            roles,
            rendering: FontRenderingConfig::default(),
            watch_dirs,
            install_dir,
            google_fonts: GoogleFontsConfig::default(),
            auto_activate: true,
            drag_drop_install: true,
            allowed_import_domains: vec![
                "fonts.google.com".into(),
                "github.com".into(),
                "gitlab.com".into(),
                "fonts.bunny.net".into(),
            ],
        }
    }
}
