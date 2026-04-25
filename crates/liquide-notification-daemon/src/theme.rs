//! Notification theming — background/accent colours per urgency level.
//!
//! Used by the shell's notification renderer to pick a default background
//! and accent colour for each [`crate::spec::Urgency`] variant when the
//! application doesn't supply custom styling. Themes are intentionally
//! expressed as sRGB `[R, G, B, A]` bytes so this crate stays free of any
//! colour-math dependency.

use crate::spec::Urgency;
use serde::{Deserialize, Serialize};

/// 8-bit sRGB colour with alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl NotificationColor {
    /// Create an opaque colour.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Create a colour with custom alpha.
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

/// Per-urgency colour pair: background tint + accent (typically used for
/// the left-edge indicator strip and the notification title).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UrgencyColors {
    pub background: NotificationColor,
    pub accent: NotificationColor,
    pub text: NotificationColor,
}

/// A theme describing how notifications should look at each urgency level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationTheme {
    pub low: UrgencyColors,
    pub normal: UrgencyColors,
    pub critical: UrgencyColors,
}

impl NotificationTheme {
    /// Lookup the colour set for a given urgency.
    pub fn colors_for(&self, urgency: Urgency) -> &UrgencyColors {
        match urgency {
            Urgency::Low => &self.low,
            Urgency::Normal => &self.normal,
            Urgency::Critical => &self.critical,
        }
    }
}

impl Default for NotificationTheme {
    fn default() -> Self {
        // Dark-ish panel tints; accents follow a muted → vivid progression
        // from Low to Critical so urgency is visually unambiguous.
        Self {
            low: UrgencyColors {
                background: NotificationColor::rgba(32, 32, 38, 230),
                accent: NotificationColor::rgb(110, 120, 140),
                text: NotificationColor::rgb(220, 220, 225),
            },
            normal: UrgencyColors {
                background: NotificationColor::rgba(32, 32, 38, 240),
                accent: NotificationColor::rgb(70, 130, 220),
                text: NotificationColor::rgb(235, 235, 240),
            },
            critical: UrgencyColors {
                background: NotificationColor::rgba(48, 18, 18, 245),
                accent: NotificationColor::rgb(220, 70, 70),
                text: NotificationColor::rgb(255, 240, 240),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_distinguishes_urgencies() {
        let theme = NotificationTheme::default();
        assert_ne!(
            theme.colors_for(Urgency::Low).accent,
            theme.colors_for(Urgency::Critical).accent
        );
        assert_ne!(
            theme.colors_for(Urgency::Normal).background,
            theme.colors_for(Urgency::Critical).background
        );
    }

    #[test]
    fn colors_for_critical_is_reddish() {
        let theme = NotificationTheme::default();
        let c = theme.colors_for(Urgency::Critical);
        assert!(c.accent.r > c.accent.g);
        assert!(c.accent.r > c.accent.b);
    }
}
