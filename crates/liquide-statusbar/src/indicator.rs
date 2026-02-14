//! System indicators — clock, battery, WiFi, notifications.

use liquide_ui_core::{Painter, UiColor, UiTheme};
use serde::{Deserialize, Serialize};

/// Indicator type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IndicatorKind {
    Clock {
        format: String,
        timestamp_us: u64,
    },
    Battery {
        percent: u8,
        charging: bool,
    },
    Wifi {
        quality_percent: u8,
        ssid: Option<String>,
    },
    Notification {
        unread_count: u32,
        dnd: bool,
    },
    Volume {
        level: u8,
        muted: bool,
    },
}

/// A system indicator displayed on the right side of the status bar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemIndicator {
    pub id: String,
    pub kind: IndicatorKind,
    pub visible: bool,
}

impl SystemIndicator {
    pub fn clock() -> Self {
        Self {
            id: "clock".into(),
            kind: IndicatorKind::Clock { format: "%H:%M".into(), timestamp_us: 0 },
            visible: true,
        }
    }

    pub fn battery(percent: u8) -> Self {
        Self {
            id: "battery".into(),
            kind: IndicatorKind::Battery { percent, charging: false },
            visible: true,
        }
    }

    pub fn wifi(quality: u8) -> Self {
        Self {
            id: "wifi".into(),
            kind: IndicatorKind::Wifi { quality_percent: quality, ssid: None },
            visible: true,
        }
    }

    pub fn notification() -> Self {
        Self {
            id: "notifications".into(),
            kind: IndicatorKind::Notification { unread_count: 0, dnd: false },
            visible: true,
        }
    }

    pub fn volume(level: u8) -> Self {
        Self {
            id: "volume".into(),
            kind: IndicatorKind::Volume { level, muted: false },
            visible: true,
        }
    }

    /// Paint this indicator. Returns the width consumed.
    pub fn paint(
        &self,
        painter: &mut Painter,
        theme: &UiTheme,
        x: f32,
        bar_y: f32,
        bar_h: f32,
    ) -> f32 {
        if !self.visible { return 0.0; }

        let colors = &theme.colors;
        let font_size = theme.font_size * 0.9;
        let text_y = bar_y + (bar_h - font_size) / 2.0;

        match &self.kind {
            IndicatorKind::Clock { timestamp_us, .. } => {
                let total_secs = timestamp_us / 1_000_000;
                let hours = (total_secs / 3600) % 24;
                let minutes = (total_secs / 60) % 60;
                let time_str = format!("{hours:02}:{minutes:02}");
                let w = time_str.len() as f32 * font_size * 0.55;
                painter.draw_text(
                    &time_str, x, text_y, font_size,
                    colors.text_primary, &theme.font_family, false,
                );
                w + 8.0
            }
            IndicatorKind::Battery { percent, charging } => {
                let icon = if *charging { "⚡" } else if *percent > 20 { "🔋" } else { "🪫" };
                let label = format!("{icon} {percent}%");
                let w = label.len() as f32 * font_size * 0.5;
                painter.draw_text(
                    &label, x, text_y, font_size,
                    colors.text_secondary, &theme.font_family, false,
                );
                w + 8.0
            }
            IndicatorKind::Wifi { quality_percent, .. } => {
                // Simple WiFi strength text indicator
                let bars = if *quality_percent > 75 { "▂▄▆█" } else if *quality_percent > 50 { "▂▄▆" } else if *quality_percent > 25 { "▂▄" } else { "▂" };
                let w = 28.0;
                painter.draw_text(
                    bars, x, text_y, font_size * 0.8,
                    colors.text_secondary, &theme.font_family, false,
                );
                w
            }
            IndicatorKind::Notification { unread_count, dnd } => {
                let icon = if *dnd { "🔕" } else { "🔔" };
                let w = 20.0;
                painter.draw_text(
                    icon, x, text_y, font_size,
                    if *unread_count > 0 { colors.accent } else { colors.text_secondary },
                    &theme.font_family, false,
                );
                if *unread_count > 0 {
                    let badge = format!("{unread_count}");
                    let badge_fs = font_size * 0.7;
                    painter.draw_text(
                        &badge, x + 12.0, bar_y + 2.0, badge_fs,
                        colors.error, &theme.font_family, true,
                    );
                }
                w + 8.0
            }
            IndicatorKind::Volume { level, muted } => {
                let icon = if *muted { "🔇" } else if *level > 66 { "🔊" } else if *level > 33 { "🔉" } else { "🔈" };
                let w = 20.0;
                painter.draw_text(
                    icon, x, text_y, font_size,
                    colors.text_secondary, &theme.font_family, false,
                );
                w
            }
        }
    }
}
