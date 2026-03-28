//! Polling-based preference change detection.
//!
//! Call [`check_for_changes`] with the old and new
//! [`AccessibilityPreferences`] to get a list of what changed.
//! Pair this with a periodic poll of [`crate::platform::detect()`]
//! to implement live preference tracking.

use crate::prefs::{AccessibilityPreferences, CursorSize};

/// A single preference that changed between two snapshots.
#[derive(Debug, Clone, PartialEq)]
pub enum PreferenceChange {
    /// High-contrast mode toggled.
    HighContrast(bool),
    /// Reduced-motion preference toggled.
    ReducedMotion(bool),
    /// Reduced-transparency preference toggled.
    ReducedTransparency(bool),
    /// Increase-contrast preference toggled.
    IncreaseContrast(bool),
    /// Color inversion toggled.
    InvertedColors(bool),
    /// Large-text preference toggled.
    LargeText(bool),
    /// Text scale factor changed. Contains the new value.
    TextScaleFactor(f32),
    /// Cursor size changed. Contains the new value.
    CursorSize(CursorSize),
    /// Screen reader active state toggled.
    ScreenReaderActive(bool),
    /// Sticky keys toggled.
    StickyKeys(bool),
    /// Slow keys toggled.
    SlowKeys(bool),
    /// Bounce keys toggled.
    BounceKeys(bool),
}

impl PreferenceChange {
    /// Human-readable label for the changed preference.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            PreferenceChange::HighContrast(_) => "high-contrast",
            PreferenceChange::ReducedMotion(_) => "reduced-motion",
            PreferenceChange::ReducedTransparency(_) => "reduced-transparency",
            PreferenceChange::IncreaseContrast(_) => "increase-contrast",
            PreferenceChange::InvertedColors(_) => "inverted-colors",
            PreferenceChange::LargeText(_) => "large-text",
            PreferenceChange::TextScaleFactor(_) => "text-scale-factor",
            PreferenceChange::CursorSize(_) => "cursor-size",
            PreferenceChange::ScreenReaderActive(_) => "screen-reader-active",
            PreferenceChange::StickyKeys(_) => "sticky-keys",
            PreferenceChange::SlowKeys(_) => "slow-keys",
            PreferenceChange::BounceKeys(_) => "bounce-keys",
        }
    }

    /// Returns `true` if this change affects visual rendering
    /// (themes, contrast, transparency, text size).
    #[must_use]
    pub fn is_visual(&self) -> bool {
        matches!(
            self,
            PreferenceChange::HighContrast(_)
                | PreferenceChange::ReducedTransparency(_)
                | PreferenceChange::IncreaseContrast(_)
                | PreferenceChange::InvertedColors(_)
                | PreferenceChange::LargeText(_)
                | PreferenceChange::TextScaleFactor(_)
                | PreferenceChange::CursorSize(_)
        )
    }

    /// Returns `true` if this change affects animation behaviour.
    #[must_use]
    pub fn is_motion(&self) -> bool {
        matches!(self, PreferenceChange::ReducedMotion(_))
    }

    /// Returns `true` if this change affects keyboard input handling.
    #[must_use]
    pub fn is_keyboard(&self) -> bool {
        matches!(
            self,
            PreferenceChange::StickyKeys(_)
                | PreferenceChange::SlowKeys(_)
                | PreferenceChange::BounceKeys(_)
        )
    }
}

/// Compare two snapshots and return a list of all preferences that differ.
///
/// Returns an empty `Vec` when nothing changed.
///
/// # Example
///
/// ```
/// use liquide_a11y_prefs::prefs::AccessibilityPreferences;
/// use liquide_a11y_prefs::watcher::check_for_changes;
///
/// let mut old = AccessibilityPreferences::default();
/// let mut new = old.clone();
/// new.high_contrast = true;
///
/// let changes = check_for_changes(&old, &new);
/// assert_eq!(changes.len(), 1);
/// ```
#[must_use]
pub fn check_for_changes(
    old: &AccessibilityPreferences,
    new: &AccessibilityPreferences,
) -> Vec<PreferenceChange> {
    let mut changes = Vec::new();

    if old.high_contrast != new.high_contrast {
        changes.push(PreferenceChange::HighContrast(new.high_contrast));
    }
    if old.reduced_motion != new.reduced_motion {
        changes.push(PreferenceChange::ReducedMotion(new.reduced_motion));
    }
    if old.reduced_transparency != new.reduced_transparency {
        changes.push(PreferenceChange::ReducedTransparency(
            new.reduced_transparency,
        ));
    }
    if old.increase_contrast != new.increase_contrast {
        changes.push(PreferenceChange::IncreaseContrast(new.increase_contrast));
    }
    if old.inverted_colors != new.inverted_colors {
        changes.push(PreferenceChange::InvertedColors(new.inverted_colors));
    }
    if old.large_text != new.large_text {
        changes.push(PreferenceChange::LargeText(new.large_text));
    }
    if (old.text_scale_factor - new.text_scale_factor).abs() > f32::EPSILON {
        changes.push(PreferenceChange::TextScaleFactor(new.text_scale_factor));
    }
    if old.cursor_size != new.cursor_size {
        changes.push(PreferenceChange::CursorSize(new.cursor_size));
    }
    if old.screen_reader_active != new.screen_reader_active {
        changes.push(PreferenceChange::ScreenReaderActive(
            new.screen_reader_active,
        ));
    }
    if old.sticky_keys != new.sticky_keys {
        changes.push(PreferenceChange::StickyKeys(new.sticky_keys));
    }
    if old.slow_keys != new.slow_keys {
        changes.push(PreferenceChange::SlowKeys(new.slow_keys));
    }
    if old.bounce_keys != new.bounce_keys {
        changes.push(PreferenceChange::BounceKeys(new.bounce_keys));
    }

    changes
}

/// Convenience: returns `true` if any preference changed.
#[must_use]
pub fn has_changes(
    old: &AccessibilityPreferences,
    new: &AccessibilityPreferences,
) -> bool {
    old != new
}

/// Convenience: returns `true` if any visual preference changed.
#[must_use]
pub fn has_visual_changes(
    old: &AccessibilityPreferences,
    new: &AccessibilityPreferences,
) -> bool {
    check_for_changes(old, new).iter().any(|c| c.is_visual())
}

/// Convenience: returns `true` if any motion preference changed.
#[must_use]
pub fn has_motion_changes(
    old: &AccessibilityPreferences,
    new: &AccessibilityPreferences,
) -> bool {
    check_for_changes(old, new).iter().any(|c| c.is_motion())
}
