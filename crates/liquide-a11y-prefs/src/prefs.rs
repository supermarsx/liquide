//! Accessibility preferences and cursor size types.

/// System cursor size preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CursorSize {
    /// Smaller than default cursor.
    Small,
    /// Default system cursor size (typically 32px).
    #[default]
    Normal,
    /// Enlarged cursor (typically 48px).
    Large,
    /// Maximum enlargement (typically 64px+).
    ExtraLarge,
}

impl CursorSize {
    /// Convert a pixel size to a `CursorSize` bucket.
    #[must_use]
    pub fn from_pixels(px: u32) -> Self {
        match px {
            0..=23 => CursorSize::Small,
            24..=39 => CursorSize::Normal,
            40..=55 => CursorSize::Large,
            _ => CursorSize::ExtraLarge,
        }
    }

    /// Representative pixel size for this bucket.
    #[must_use]
    pub fn to_pixels(self) -> u32 {
        match self {
            CursorSize::Small => 16,
            CursorSize::Normal => 32,
            CursorSize::Large => 48,
            CursorSize::ExtraLarge => 64,
        }
    }
}

impl Default for CursorSize {
    fn default() -> Self {
        CursorSize::Normal
    }
}

/// Collected accessibility preferences from the operating system.
///
/// Use [`crate::platform::detect()`] to read the current values, or
/// [`AccessibilityPreferences::default()`] for sane defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct AccessibilityPreferences {
    /// System high-contrast mode is enabled (Windows High Contrast,
    /// GTK high-contrast theme, macOS Increase Contrast).
    pub high_contrast: bool,

    /// User prefers reduced motion (CSS `prefers-reduced-motion: reduce`).
    pub reduced_motion: bool,

    /// User prefers reduced transparency (no glass / blur effects).
    pub reduced_transparency: bool,

    /// User prefers increased contrast (stronger borders, less subtle
    /// colors) but not full high-contrast mode.
    pub increase_contrast: bool,

    /// System-wide color inversion is enabled.
    pub inverted_colors: bool,

    /// System requests larger text sizes.
    pub large_text: bool,

    /// System text scaling factor. 1.0 = 100%, 1.5 = 150%, etc.
    pub text_scale_factor: f32,

    /// User's preferred cursor size.
    pub cursor_size: CursorSize,

    /// A screen reader is currently running.
    pub screen_reader_active: bool,

    /// Sticky keys are enabled (modifier keys stay active after release).
    pub sticky_keys: bool,

    /// Slow keys are enabled (keys must be held briefly to register).
    pub slow_keys: bool,

    /// Bounce keys (key debounce) are enabled.
    pub bounce_keys: bool,
}

impl Default for AccessibilityPreferences {
    fn default() -> Self {
        Self {
            high_contrast: false,
            reduced_motion: false,
            reduced_transparency: false,
            increase_contrast: false,
            inverted_colors: false,
            large_text: false,
            text_scale_factor: 1.0,
            cursor_size: CursorSize::Normal,
            screen_reader_active: false,
            sticky_keys: false,
            slow_keys: false,
            bounce_keys: false,
        }
    }
}

impl AccessibilityPreferences {
    /// Returns `true` if any visual accessibility preference is active.
    #[must_use]
    pub fn has_visual_overrides(&self) -> bool {
        self.high_contrast
            || self.reduced_transparency
            || self.increase_contrast
            || self.inverted_colors
            || self.large_text
            || (self.text_scale_factor - 1.0).abs() > 0.01
    }

    /// Returns `true` if any motion-related preference is active.
    #[must_use]
    pub fn has_motion_overrides(&self) -> bool {
        self.reduced_motion
    }

    /// Returns `true` if any keyboard accessibility preference is active.
    #[must_use]
    pub fn has_keyboard_overrides(&self) -> bool {
        self.sticky_keys || self.slow_keys || self.bounce_keys
    }
}
