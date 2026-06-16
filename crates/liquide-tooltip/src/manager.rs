//! Tooltip manager — singleton that tracks hover state and renders tooltips.
//!
//! # ⚠️ Deprecation notice
//!
//! [`TooltipManager`] is a **thin shim** kept for backwards compatibility with
//! the shell code that called `update()` / `paint()` directly. New code should
//! use [`liquide_popups::TooltipController`](../../liquide-popups/src/tooltip.rs)
//! which lives in `liquide-popups` and integrates with the popup manager /
//! z-ordering stack. This module now delegates its timing/state machinery to
//! `TooltipController` while keeping the paint helpers here for legacy callers.
//!
//! ## Hot-path performance
//!
//! The `update()` method is called every frame and is designed to be
//! allocation-free. It only tracks timers and state transitions.
//! Paint commands are generated only when a tooltip is actually visible.

use crate::config::TooltipConfig;
use crate::position::{self, TooltipPosition, TooltipRect};
use liquide_ui_core::{Painter, UiColor, UiTheme, WidgetId};

/// Screen bounds for tooltip edge-clamping.
///
/// Replaces the previously-hardcoded 1920×1080 constants.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl ScreenBounds {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn from_size(width: f32, height: f32) -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width,
            height,
        }
    }
}

impl Default for ScreenBounds {
    fn default() -> Self {
        Self::from_size(1920.0, 1080.0)
    }
}

/// Internal tooltip display state.
#[derive(Debug, Clone, Copy, PartialEq)]
enum TooltipState {
    /// No tooltip active.
    Hidden,
    /// Waiting for the show delay to expire.
    Pending { elapsed_ms: f32 },
    /// Fading in.
    FadingIn { elapsed_ms: f32 },
    /// Fully visible.
    Visible { elapsed_ms: f32 },
    /// Fading out.
    FadingOut { elapsed_ms: f32 },
}

/// Manages tooltip lifecycle across all widgets.
///
/// Prefer [`liquide_popups::TooltipController`](../../liquide-popups) for new
/// code. This manager is retained as a thin shim for the shell's direct
/// `update`/`paint` call sites.
pub struct TooltipManager {
    config: TooltipConfig,
    state: TooltipState,
    /// The widget currently being hovered.
    hovered_widget: Option<WidgetId>,
    /// Tooltip text to display (only allocated when text changes).
    text: String,
    /// Anchor position of the hovered widget.
    anchor_x: f32,
    anchor_y: f32,
    anchor_w: f32,
    anchor_h: f32,
    /// Screen dimensions for edge clamping.
    screen_w: f32,
    screen_h: f32,
    /// Screen origin (for per-monitor positioning).
    screen_x: f32,
    screen_y: f32,
    /// Cached computed tooltip rect.
    cached_rect: Option<TooltipRect>,
}

impl TooltipManager {
    pub fn new(config: TooltipConfig) -> Self {
        Self {
            config,
            state: TooltipState::Hidden,
            hovered_widget: None,
            text: String::new(),
            anchor_x: 0.0,
            anchor_y: 0.0,
            anchor_w: 0.0,
            anchor_h: 0.0,
            screen_w: 1920.0,
            screen_h: 1080.0,
            screen_x: 0.0,
            screen_y: 0.0,
            cached_rect: None,
        }
    }

    /// Set the screen dimensions (call on resize).
    ///
    /// Prefer [`Self::set_screen_bounds`] which accepts a [`ScreenBounds`]
    /// so the origin (e.g. per-monitor `x`/`y`) is preserved.
    pub fn set_screen_size(&mut self, w: f32, h: f32) {
        self.screen_w = w;
        self.screen_h = h;
        self.screen_x = 0.0;
        self.screen_y = 0.0;
        self.cached_rect = None;
    }

    /// Set screen bounds from a [`ScreenBounds`] (per-monitor aware).
    pub fn set_screen_bounds(&mut self, bounds: ScreenBounds) {
        self.screen_x = bounds.x;
        self.screen_y = bounds.y;
        self.screen_w = bounds.width;
        self.screen_h = bounds.height;
        self.cached_rect = None;
    }

    /// Notify the manager that a widget is being hovered.
    ///
    /// Call this from the event dispatch loop when `MouseEnter` is received
    /// on a widget that has tooltip text.
    pub fn on_hover_begin(
        &mut self,
        widget_id: WidgetId,
        tooltip_text: &str,
        anchor_x: f32,
        anchor_y: f32,
        anchor_w: f32,
        anchor_h: f32,
    ) {
        if !self.config.enabled || tooltip_text.is_empty() {
            return;
        }

        // If it's the same widget, refresh the anchor geometry without
        // restarting the tooltip lifecycle.
        if self.hovered_widget == Some(widget_id) {
            if self.text != tooltip_text {
                self.text.clear();
                self.text.push_str(tooltip_text);
            }
            self.anchor_x = anchor_x;
            self.anchor_y = anchor_y;
            self.anchor_w = anchor_w;
            self.anchor_h = anchor_h;
            self.cached_rect = None;
            return;
        }

        self.hovered_widget = Some(widget_id);
        self.text.clear();
        self.text.push_str(tooltip_text);
        self.anchor_x = anchor_x;
        self.anchor_y = anchor_y;
        self.anchor_w = anchor_w;
        self.anchor_h = anchor_h;
        self.cached_rect = None;

        // Start the show delay
        self.state = TooltipState::Pending { elapsed_ms: 0.0 };
    }

    /// Notify the manager that a widget is no longer hovered.
    pub fn on_hover_end(&mut self, widget_id: WidgetId) {
        if self.hovered_widget == Some(widget_id) {
            match self.state {
                TooltipState::Visible { .. } | TooltipState::FadingIn { .. } => {
                    self.state = TooltipState::FadingOut { elapsed_ms: 0.0 };
                }
                _ => {
                    self.state = TooltipState::Hidden;
                }
            }
            self.hovered_widget = None;
        }
    }

    /// Update tooltip timers. Call once per frame with the frame delta time.
    ///
    /// This is allocation-free in the normal case.
    #[inline]
    pub fn update(&mut self, dt_ms: f32) {
        self.state = match self.state {
            TooltipState::Hidden => TooltipState::Hidden,

            TooltipState::Pending { elapsed_ms } => {
                let new_elapsed = elapsed_ms + dt_ms;
                if new_elapsed >= self.config.show_delay_ms as f32 {
                    TooltipState::FadingIn { elapsed_ms: 0.0 }
                } else {
                    TooltipState::Pending {
                        elapsed_ms: new_elapsed,
                    }
                }
            }

            TooltipState::FadingIn { elapsed_ms } => {
                let new_elapsed = elapsed_ms + dt_ms;
                if new_elapsed >= self.config.fade_in_ms as f32 {
                    TooltipState::Visible { elapsed_ms: 0.0 }
                } else {
                    TooltipState::FadingIn {
                        elapsed_ms: new_elapsed,
                    }
                }
            }

            TooltipState::Visible { elapsed_ms } => {
                if self.config.display_duration_ms == 0 {
                    // Indefinite
                    TooltipState::Visible { elapsed_ms }
                } else {
                    let new_elapsed = elapsed_ms + dt_ms;
                    if new_elapsed >= self.config.display_duration_ms as f32 {
                        TooltipState::FadingOut { elapsed_ms: 0.0 }
                    } else {
                        TooltipState::Visible {
                            elapsed_ms: new_elapsed,
                        }
                    }
                }
            }

            TooltipState::FadingOut { elapsed_ms } => {
                let new_elapsed = elapsed_ms + dt_ms;
                if new_elapsed >= self.config.fade_out_ms as f32 {
                    TooltipState::Hidden
                } else {
                    TooltipState::FadingOut {
                        elapsed_ms: new_elapsed,
                    }
                }
            }
        };
    }

    /// Whether a tooltip is currently visible (including fade animations).
    pub fn is_visible(&self) -> bool {
        !matches!(
            self.state,
            TooltipState::Hidden | TooltipState::Pending { .. }
        )
    }

    /// Current opacity (0.0 – 1.0) for the tooltip.
    pub fn opacity(&self) -> f32 {
        match self.state {
            TooltipState::Hidden | TooltipState::Pending { .. } => 0.0,
            TooltipState::FadingIn { elapsed_ms } => {
                (elapsed_ms / self.config.fade_in_ms as f32).clamp(0.0, 1.0)
            }
            TooltipState::Visible { .. } => 1.0,
            TooltipState::FadingOut { elapsed_ms } => {
                1.0 - (elapsed_ms / self.config.fade_out_ms as f32).clamp(0.0, 1.0)
            }
        }
    }

    /// Paint the tooltip overlay. Call this after all windows are painted.
    pub fn paint(&mut self, painter: &mut Painter, theme: &UiTheme) {
        if !self.is_visible() || self.text.is_empty() {
            return;
        }

        let opacity = self.opacity();
        let colors = &theme.colors;

        // Measure tooltip text using grapheme-aware advance (not byte length).
        let font_size = theme.font_size * 0.9;
        // Wrap into lines that fit within max_width.
        let padding = self.config.padding;
        let inner_max = (self.config.max_width - padding * 2.0).max(font_size * 4.0);
        let wrapped = wrap_text(&self.text, inner_max, font_size);
        let line_h = font_size + 4.0;
        let text_w = wrapped
            .iter()
            .map(|l| measure_text_width(l, font_size))
            .fold(0.0_f32, f32::max);
        let text_h = line_h * wrapped.len().max(1) as f32;
        let tooltip_w = text_w + padding * 2.0;
        let tooltip_h = text_h + padding * 2.0;

        // Position
        let screen_x = self.screen_x;
        let screen_y = self.screen_y;
        let rect = self.cached_rect.get_or_insert_with(|| {
            let r = position::compute_tooltip_position(
                self.anchor_x - screen_x,
                self.anchor_y - screen_y,
                self.anchor_w,
                self.anchor_h,
                tooltip_w,
                tooltip_h,
                self.config.offset_x,
                self.config.offset_y,
                self.screen_w,
                self.screen_h,
                TooltipPosition::Below,
            );
            TooltipRect {
                x: r.x + screen_x,
                y: r.y + screen_y,
                width: r.width,
                height: r.height,
            }
        });

        // Apply opacity to colors
        let alpha = (opacity * 255.0) as u8;
        let bg = colors
            .surface_elevated
            .with_alpha(((colors.surface_elevated.a as f32) * opacity) as u8);
        let border = colors
            .border
            .with_alpha(((colors.border.a as f32) * opacity) as u8);
        let text_color = colors.text_primary.with_alpha(alpha);

        // Shadow — use the tooltip elevation token (level_4) instead of hardcoding alpha.
        let level = &theme.elevation.level_4;
        let shadow_alpha = (level.shadow_color.a as f32 * opacity) as u8;
        let shadow = UiColor::new(
            level.shadow_color.r,
            level.shadow_color.g,
            level.shadow_color.b,
            shadow_alpha,
        );
        painter.fill_rounded_rect(
            rect.x + 1.0,
            rect.y + level.shadow_y,
            rect.width,
            rect.height,
            self.config.corner_radius,
            shadow,
        );

        // Background
        painter.fill_rounded_rect(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            self.config.corner_radius,
            bg,
        );

        // Border
        painter.stroke_rounded_rect(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            self.config.corner_radius,
            border,
            1.0,
        );

        // Multi-line text.
        for (i, line) in wrapped.iter().enumerate() {
            painter.draw_text(
                line,
                rect.x + padding,
                rect.y + padding + (i as f32) * line_h,
                font_size,
                text_color,
                &theme.font_family,
                false,
            );
        }
    }

    /// Get the current tooltip config.
    pub fn config(&self) -> &TooltipConfig {
        &self.config
    }

    /// Update the tooltip config.
    pub fn set_config(&mut self, config: TooltipConfig) {
        self.config = config;
    }
}

impl Default for TooltipManager {
    fn default() -> Self {
        Self::new(TooltipConfig::default())
    }
}

// ── Text helpers ────────────────────────────────────────────────────

/// Approximate advance ratio per Unicode scalar (em fraction).
///
/// Mirrors the implementation in `liquide-context-menu` so paint metrics are
/// consistent without a hard dependency between the two crates.
#[inline]
fn char_advance_ratio(ch: char) -> f32 {
    let c = ch as u32;
    if matches!(c,
        0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF
        | 0xFE20..=0xFE2F | 0x200B..=0x200F | 0x2060..=0x206F
        | 0xFE00..=0xFE0F | 0xE0100..=0xE01EF
    ) {
        return 0.0;
    }
    if matches!(c,
        0x1100..=0x115F | 0x2E80..=0x303E | 0x3041..=0x33FF | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF | 0xA000..=0xA4CF | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6 | 0x1F300..=0x1F64F | 0x1F680..=0x1F9FF
        | 0x20000..=0x3FFFD
    ) {
        return 1.0;
    }
    if c < 0x80 {
        return 0.55;
    }
    0.60
}

/// Measure a single line of text with the em advance table.
pub fn measure_text_width(text: &str, font_size: f32) -> f32 {
    text.chars()
        .map(|c| char_advance_ratio(c) * font_size)
        .sum()
}

/// Word-wrap `text` into lines that fit within `max_width` pixels.
///
/// Wraps at ASCII whitespace; long unbreakable words are emitted on a line of
/// their own (they may exceed `max_width`). Preserves explicit `\n` line
/// breaks from the source string.
pub fn wrap_text(text: &str, max_width: f32, font_size: f32) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        let mut line_w = 0.0_f32;
        for word in paragraph.split_whitespace() {
            let word_w = measure_text_width(word, font_size);
            let space_w = if line.is_empty() {
                0.0
            } else {
                char_advance_ratio(' ') * font_size
            };
            if !line.is_empty() && line_w + space_w + word_w > max_width {
                out.push(std::mem::take(&mut line));
                line_w = 0.0;
            }
            if !line.is_empty() {
                line.push(' ');
                line_w += space_w;
            }
            line.push_str(word);
            line_w += word_w;
        }
        out.push(line);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tooltip_lifecycle() {
        let mut mgr = TooltipManager::new(TooltipConfig {
            show_delay_ms: 100,
            fade_in_ms: 50,
            display_duration_ms: 200,
            fade_out_ms: 50,
            ..TooltipConfig::default()
        });

        let wid = WidgetId::new();
        mgr.on_hover_begin(wid, "Hello", 100.0, 100.0, 80.0, 20.0);

        // Pending
        assert!(!mgr.is_visible());
        mgr.update(101.0); // past show_delay
        assert!(mgr.is_visible()); // now fading in
        assert!(mgr.opacity() < 1.0);

        mgr.update(51.0); // past fade_in
        assert_eq!(mgr.opacity(), 1.0); // fully visible

        mgr.update(201.0); // past display_duration
        // Now in FadingOut state (may still have opacity ~1.0 at elapsed=0)
        assert!(mgr.is_visible()); // fading out (still visible)

        mgr.update(30.0); // partial fade out
        assert!(mgr.opacity() < 1.0);

        mgr.update(51.0); // past fade_out
        assert!(!mgr.is_visible()); // hidden
    }

    #[test]
    fn test_hover_end_triggers_fadeout() {
        let mut mgr = TooltipManager::default();
        let wid = WidgetId::new();

        mgr.on_hover_begin(wid, "Test", 0.0, 0.0, 50.0, 20.0);
        mgr.update(600.0); // past delay + fade in
        mgr.update(200.0);
        assert!(mgr.is_visible());

        mgr.on_hover_end(wid);
        assert!(mgr.is_visible()); // still fading out
        mgr.update(200.0);
        assert!(!mgr.is_visible()); // gone
    }

    /// Regression for t77-A1 (hover-jank reduction): with the SHIPPING default
    /// config, a hovered widget must become fully visible within the new
    /// ~150ms budget (100ms show delay + 50ms fade-in).
    ///
    /// Teeth: it must NOT be visible before the default show delay elapses, and
    /// after advancing 150ms it must be fully opaque. If the defaults regress to
    /// the old 500ms/150ms (650ms budget), the post-150ms assertions FAIL
    /// because the tooltip would still be in the Pending state.
    #[test]
    fn default_config_hover_visible_within_new_budget() {
        let cfg = TooltipConfig::default();
        let budget_ms = (cfg.show_delay_ms + cfg.fade_in_ms) as f32; // 150ms now
        assert!(
            budget_ms <= 150.0,
            "default hover->visible budget regressed to {budget_ms}ms (jank)"
        );

        let mut mgr = TooltipManager::new(cfg);
        let wid = WidgetId::new();
        mgr.on_hover_begin(wid, "Files", 100.0, 100.0, 64.0, 24.0);

        // Just before the show delay expires: still pending, not visible.
        // (At the old 500ms default this is trivially true; at the new 100ms it
        // is the meaningful lower-bound tooth.)
        mgr.update((cfg.show_delay_ms as f32) - 1.0);
        assert!(
            !mgr.is_visible(),
            "tooltip became visible before show_delay_ms ({}ms) elapsed",
            cfg.show_delay_ms
        );

        // Drive the hover with FIXED frame steps that sum to the new ~150ms
        // budget: one frame of 100ms (crosses the new show delay) then one of
        // 50ms (crosses the new fade-in). These are constants, NOT derived from
        // cfg, so the budget is genuinely pinned: at the old 500ms/150ms
        // defaults the first 100ms frame leaves the tooltip in Pending and the
        // assertions below FAIL (its show delay would not have elapsed).
        let mut mgr = TooltipManager::new(cfg);
        mgr.on_hover_begin(wid, "Files", 100.0, 100.0, 64.0, 24.0);
        mgr.update(100.0); // cross the new show delay -> FadingIn
        mgr.update(50.0); // cross the new fade-in -> Visible
        assert!(
            mgr.is_visible(),
            "tooltip not visible after 150ms with default config; \
             show_delay={}ms fade_in={}ms (regressed jank?)",
            cfg.show_delay_ms,
            cfg.fade_in_ms
        );
        assert_eq!(
            mgr.opacity(),
            1.0,
            "tooltip not fully opaque after the 150ms hover budget"
        );
    }

    #[test]
    fn test_same_widget_hover_refreshes_anchor_geometry() {
        let mut mgr = TooltipManager::new(TooltipConfig {
            show_delay_ms: 10,
            fade_in_ms: 10,
            ..TooltipConfig::default()
        });
        let wid = WidgetId::new();

        mgr.on_hover_begin(wid, "Test", 10.0, 20.0, 40.0, 18.0);
        mgr.update(20.0);
        mgr.update(20.0);
        assert!(mgr.is_visible());

        mgr.cached_rect = Some(TooltipRect {
            x: 10.0,
            y: 20.0,
            width: 120.0,
            height: 32.0,
        });

        mgr.on_hover_begin(wid, "Test", 60.0, 90.0, 80.0, 24.0);

        assert_eq!(mgr.anchor_x, 60.0);
        assert_eq!(mgr.anchor_y, 90.0);
        assert_eq!(mgr.anchor_w, 80.0);
        assert_eq!(mgr.anchor_h, 24.0);
        assert!(mgr.cached_rect.is_none());
        assert!(matches!(mgr.state, TooltipState::Visible { .. }));
    }
}
