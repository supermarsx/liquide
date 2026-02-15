//! Scroll view container widget.
//!
//! A container that clips its child content and provides scrollbars when
//! content exceeds available space. Inspired by Qt's QScrollArea and
//! GTK's GtkScrolledWindow.

use liquide_ui_core::{
    Constraints, Event, EventResponse, LayoutResult, Painter, UiTheme, WidgetId,
    widget::{Widget, WidgetState},
};

/// Scroll bar visibility policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollBarPolicy {
    /// Show scrollbar when content exceeds viewport.
    Auto,
    /// Always show scrollbar.
    AlwaysOn,
    /// Never show scrollbar.
    AlwaysOff,
}

/// Scroll view container.
pub struct ScrollView {
    state: WidgetState,
    /// Scroll offset (pixels from top-left).
    scroll_x: f32,
    scroll_y: f32,
    /// Total content size (set by child).
    content_width: f32,
    content_height: f32,
    h_policy: ScrollBarPolicy,
    v_policy: ScrollBarPolicy,
    scroll_speed: f32,
    /// Whether user is dragging the scrollbar thumb.
    dragging: bool,
    #[allow(dead_code)]
    drag_start_y: f32,
    drag_scroll_start: f32,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

const SCROLLBAR_WIDTH: f32 = 8.0;
const SCROLLBAR_MIN_THUMB: f32 = 20.0;

impl ScrollView {
    pub fn new() -> Self {
        Self {
            state: WidgetState::new(WidgetId::new()),
            scroll_x: 0.0,
            scroll_y: 0.0,
            content_width: 0.0,
            content_height: 0.0,
            h_policy: ScrollBarPolicy::Auto,
            v_policy: ScrollBarPolicy::Auto,
            scroll_speed: 40.0,
            dragging: false,
            drag_start_y: 0.0,
            drag_scroll_start: 0.0,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        }
    }

    pub fn with_content_size(mut self, w: f32, h: f32) -> Self {
        self.content_width = w;
        self.content_height = h;
        self
    }

    pub fn with_v_policy(mut self, policy: ScrollBarPolicy) -> Self {
        self.v_policy = policy;
        self
    }

    pub fn with_h_policy(mut self, policy: ScrollBarPolicy) -> Self {
        self.h_policy = policy;
        self
    }

    pub fn with_scroll_speed(mut self, speed: f32) -> Self {
        self.scroll_speed = speed;
        self
    }

    pub fn set_content_size(&mut self, w: f32, h: f32) {
        self.content_width = w;
        self.content_height = h;
        self.clamp_scroll();
    }

    pub fn scroll_offset(&self) -> (f32, f32) {
        (self.scroll_x, self.scroll_y)
    }

    pub fn scroll_to(&mut self, x: f32, y: f32) {
        self.scroll_x = x;
        self.scroll_y = y;
        self.clamp_scroll();
    }

    fn clamp_scroll(&mut self) {
        let max_x = (self.content_width - self.width).max(0.0);
        let max_y = (self.content_height - self.height).max(0.0);
        self.scroll_x = self.scroll_x.clamp(0.0, max_x);
        self.scroll_y = self.scroll_y.clamp(0.0, max_y);
    }

    fn needs_v_scrollbar(&self) -> bool {
        match self.v_policy {
            ScrollBarPolicy::AlwaysOn => true,
            ScrollBarPolicy::AlwaysOff => false,
            ScrollBarPolicy::Auto => self.content_height > self.height,
        }
    }

    fn needs_h_scrollbar(&self) -> bool {
        match self.h_policy {
            ScrollBarPolicy::AlwaysOn => true,
            ScrollBarPolicy::AlwaysOff => false,
            ScrollBarPolicy::Auto => self.content_width > self.width,
        }
    }

    fn v_thumb_rect(&self) -> (f32, f32, f32, f32) {
        if self.content_height <= self.height {
            return (0.0, 0.0, 0.0, 0.0);
        }
        let track_h = self.height;
        let ratio = self.height / self.content_height;
        let thumb_h = (track_h * ratio).max(SCROLLBAR_MIN_THUMB);
        let max_scroll = self.content_height - self.height;
        let t = if max_scroll > 0.0 { self.scroll_y / max_scroll } else { 0.0 };
        let thumb_y = self.y + t * (track_h - thumb_h);
        let thumb_x = self.x + self.width - SCROLLBAR_WIDTH;
        (thumb_x, thumb_y, SCROLLBAR_WIDTH, thumb_h)
    }
}

impl Default for ScrollView {
    fn default() -> Self { Self::new() }
}

impl Widget for ScrollView {
    fn id(&self) -> WidgetId { self.state.id }
    fn visible(&self) -> bool { self.state.visible }
    fn set_visible(&mut self, v: bool) { self.state.visible = v; }
    fn enabled(&self) -> bool { self.state.enabled }
    fn set_enabled(&mut self, e: bool) { self.state.enabled = e; }
    fn focusable(&self) -> bool { true }
    fn tooltip(&self) -> Option<&str> { self.state.tooltip.as_deref() }

    fn measure(&self, constraints: &Constraints, _theme: &UiTheme) -> LayoutResult {
        let (w, h) = constraints.clamp(300.0, 200.0);
        LayoutResult::new(w, h)
    }

    fn layout(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.x = x; self.y = y; self.width = w; self.height = h;
        self.clamp_scroll();
    }

    fn paint(&self, painter: &mut Painter, theme: &UiTheme) {
        let colors = &theme.colors;

        // Clip region
        painter.push_clip(self.x, self.y, self.width, self.height);

        // Content area (children should be painted by the parent compositor
        // with translate(-scroll_x, -scroll_y), but we paint the background here)
        painter.fill_rect(self.x, self.y, self.width, self.height, colors.background);

        painter.pop_clip();

        // Vertical scrollbar
        if self.needs_v_scrollbar() {
            let track_x = self.x + self.width - SCROLLBAR_WIDTH;
            // Track
            painter.fill_rounded_rect(
                track_x, self.y, SCROLLBAR_WIDTH, self.height,
                SCROLLBAR_WIDTH / 2.0, colors.surface,
            );
            // Thumb
            let (tx, ty, tw, th) = self.v_thumb_rect();
            let thumb_color = if self.dragging { colors.accent } else { colors.text_secondary };
            painter.fill_rounded_rect(tx, ty, tw, th, tw / 2.0, thumb_color);
        }

        // Horizontal scrollbar
        if self.needs_h_scrollbar() {
            let track_y = self.y + self.height - SCROLLBAR_WIDTH;
            let track_w = if self.needs_v_scrollbar() { self.width - SCROLLBAR_WIDTH } else { self.width };
            painter.fill_rounded_rect(
                self.x, track_y, track_w, SCROLLBAR_WIDTH,
                SCROLLBAR_WIDTH / 2.0, colors.surface,
            );
            // Thumb
            if self.content_width > self.width {
                let ratio = self.width / self.content_width;
                let thumb_w = (track_w * ratio).max(SCROLLBAR_MIN_THUMB);
                let max_scroll = self.content_width - self.width;
                let t = if max_scroll > 0.0 { self.scroll_x / max_scroll } else { 0.0 };
                let thumb_x = self.x + t * (track_w - thumb_w);
                let thumb_color = if self.dragging { colors.accent } else { colors.text_secondary };
                painter.fill_rounded_rect(thumb_x, track_y, thumb_w, SCROLLBAR_WIDTH, SCROLLBAR_WIDTH / 2.0, thumb_color);
            }
        }
    }

    fn handle_event(&mut self, event: &Event) -> EventResponse {
        match event {
            Event::MouseEnter => { self.state.hovered = true; EventResponse::Consumed }
            Event::MouseLeave => { self.state.hovered = false; EventResponse::Consumed }
            Event::Scroll { dx, dy, .. } => {
                self.scroll_x += dx * self.scroll_speed;
                self.scroll_y += dy * self.scroll_speed;
                self.clamp_scroll();
                EventResponse::Consumed
            }
            Event::MouseDown { x, y, .. } => {
                // Check if clicking on vertical scrollbar thumb
                if self.needs_v_scrollbar() {
                    let (tx, ty, tw, th) = self.v_thumb_rect();
                    if *x >= tx && *x <= tx + tw && *y >= ty && *y <= ty + th {
                        self.dragging = true;
                        self.drag_start_y = *y;
                        self.drag_scroll_start = self.scroll_y;
                        return EventResponse::Consumed;
                    }
                }
                EventResponse::Ignored
            }
            Event::MouseUp { .. } => {
                if self.dragging {
                    self.dragging = false;
                    return EventResponse::Consumed;
                }
                EventResponse::Ignored
            }
            Event::MouseMove { y, .. } if self.dragging => {
                let delta = *y - self.drag_start_y;
                let track_h = self.height;
                let ratio = self.content_height / track_h;
                self.scroll_y = (self.drag_scroll_start + delta * ratio).clamp(0.0, (self.content_height - self.height).max(0.0));
                EventResponse::Consumed
            }
            _ => EventResponse::Ignored,
        }
    }
}
