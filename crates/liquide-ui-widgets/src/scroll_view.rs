//! Scroll view container widget.
//!
//! A container that clips its child content and provides scrollbars when
//! content exceeds available space. Inspired by Qt's QScrollArea and
//! GTK's GtkScrolledWindow.

use liquide_ui_core::{
    Constraints, Event, EventResponse, Key, LayoutResult, Painter, UiTheme, WidgetId,
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

/// Axis currently being dragged via a scrollbar thumb.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DragAxis {
    Vertical,
    Horizontal,
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
    /// Which scrollbar thumb (if any) is currently being dragged.
    drag_axis: Option<DragAxis>,
    drag_start_x: f32,
    drag_start_y: f32,
    drag_scroll_start_x: f32,
    drag_scroll_start_y: f32,
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
            drag_axis: None,
            drag_start_x: 0.0,
            drag_start_y: 0.0,
            drag_scroll_start_x: 0.0,
            drag_scroll_start_y: 0.0,
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
        let t = if max_scroll > 0.0 {
            self.scroll_y / max_scroll
        } else {
            0.0
        };
        let thumb_y = self.y + t * (track_h - thumb_h);
        let thumb_x = self.x + self.width - SCROLLBAR_WIDTH;
        (thumb_x, thumb_y, SCROLLBAR_WIDTH, thumb_h)
    }

    fn h_thumb_rect(&self) -> (f32, f32, f32, f32) {
        if self.content_width <= self.width {
            return (0.0, 0.0, 0.0, 0.0);
        }
        // Leave room on the right edge for the vertical scrollbar when
        // both are visible so the two thumbs can't overlap.
        let track_w = if self.needs_v_scrollbar() {
            self.width - SCROLLBAR_WIDTH
        } else {
            self.width
        };
        let ratio = self.width / self.content_width;
        let thumb_w = (track_w * ratio).max(SCROLLBAR_MIN_THUMB);
        let max_scroll = self.content_width - self.width;
        let t = if max_scroll > 0.0 {
            self.scroll_x / max_scroll
        } else {
            0.0
        };
        let thumb_x = self.x + t * (track_w - thumb_w);
        let thumb_y = self.y + self.height - SCROLLBAR_WIDTH;
        (thumb_x, thumb_y, thumb_w, SCROLLBAR_WIDTH)
    }

    fn page_height(&self) -> f32 {
        (self.height - SCROLLBAR_WIDTH).max(SCROLLBAR_MIN_THUMB)
    }

    fn page_width(&self) -> f32 {
        (self.width - SCROLLBAR_WIDTH).max(SCROLLBAR_MIN_THUMB)
    }

    fn scroll_by_page_x(&mut self, forward: bool) {
        let step = self.page_width();
        self.scroll_x += if forward { step } else { -step };
        self.clamp_scroll();
    }
}

impl Default for ScrollView {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for ScrollView {
    fn id(&self) -> WidgetId {
        self.state.id
    }
    fn visible(&self) -> bool {
        self.state.visible
    }
    fn set_visible(&mut self, v: bool) {
        self.state.visible = v;
    }
    fn enabled(&self) -> bool {
        self.state.enabled
    }
    fn set_enabled(&mut self, e: bool) {
        self.state.enabled = e;
    }
    fn focusable(&self) -> bool {
        true
    }
    fn tooltip(&self) -> Option<&str> {
        self.state.tooltip.as_deref()
    }

    fn measure(&self, constraints: &Constraints, _theme: &UiTheme) -> LayoutResult {
        let (w, h) = constraints.clamp(300.0, 200.0);
        LayoutResult::new(w, h)
    }

    fn layout(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.x = x;
        self.y = y;
        self.width = w;
        self.height = h;
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
            painter.fill_rounded_rect(
                track_x,
                self.y,
                SCROLLBAR_WIDTH,
                self.height,
                SCROLLBAR_WIDTH / 2.0,
                colors.surface,
            );
            let (tx, ty, tw, th) = self.v_thumb_rect();
            let thumb_color = if self.drag_axis == Some(DragAxis::Vertical) {
                colors.accent
            } else {
                colors.text_secondary
            };
            painter.fill_rounded_rect(tx, ty, tw, th, tw / 2.0, thumb_color);
        }

        // Horizontal scrollbar
        if self.needs_h_scrollbar() {
            let track_y = self.y + self.height - SCROLLBAR_WIDTH;
            let track_w = if self.needs_v_scrollbar() {
                self.width - SCROLLBAR_WIDTH
            } else {
                self.width
            };
            painter.fill_rounded_rect(
                self.x,
                track_y,
                track_w,
                SCROLLBAR_WIDTH,
                SCROLLBAR_WIDTH / 2.0,
                colors.surface,
            );
            if self.content_width > self.width {
                let (tx, ty, tw, th) = self.h_thumb_rect();
                let thumb_color = if self.drag_axis == Some(DragAxis::Horizontal) {
                    colors.accent
                } else {
                    colors.text_secondary
                };
                painter.fill_rounded_rect(tx, ty, tw, th, th / 2.0, thumb_color);
            }
        }
    }

    fn handle_event(&mut self, event: &Event) -> EventResponse {
        match event {
            Event::MouseEnter => {
                self.state.hovered = true;
                EventResponse::Consumed
            }
            Event::MouseLeave => {
                self.state.hovered = false;
                EventResponse::Consumed
            }
            Event::FocusIn => {
                self.state.focused = true;
                EventResponse::Consumed
            }
            Event::FocusOut => {
                self.state.focused = false;
                EventResponse::Consumed
            }
            Event::Scroll { dx, dy, .. } => {
                self.scroll_x += dx * self.scroll_speed;
                self.scroll_y += dy * self.scroll_speed;
                self.clamp_scroll();
                EventResponse::Consumed
            }
            Event::MouseDown { x, y, .. } => {
                // Vertical thumb has priority when both overlap in the
                // bottom-right corner.
                if self.needs_v_scrollbar() {
                    let (tx, ty, tw, th) = self.v_thumb_rect();
                    if *x >= tx && *x <= tx + tw && *y >= ty && *y <= ty + th {
                        self.drag_axis = Some(DragAxis::Vertical);
                        self.drag_start_x = *x;
                        self.drag_start_y = *y;
                        self.drag_scroll_start_x = self.scroll_x;
                        self.drag_scroll_start_y = self.scroll_y;
                        return EventResponse::Consumed;
                    }
                }
                if self.needs_h_scrollbar() {
                    let (tx, ty, tw, th) = self.h_thumb_rect();
                    if *x >= tx && *x <= tx + tw && *y >= ty && *y <= ty + th {
                        self.drag_axis = Some(DragAxis::Horizontal);
                        self.drag_start_x = *x;
                        self.drag_start_y = *y;
                        self.drag_scroll_start_x = self.scroll_x;
                        self.drag_scroll_start_y = self.scroll_y;
                        return EventResponse::Consumed;
                    }
                }
                EventResponse::RequestFocus
            }
            Event::MouseUp { .. } => {
                if self.drag_axis.is_some() {
                    self.drag_axis = None;
                    return EventResponse::Consumed;
                }
                EventResponse::Ignored
            }
            Event::MouseMove { x, y } => match self.drag_axis {
                Some(DragAxis::Vertical) => {
                    let delta = *y - self.drag_start_y;
                    let track_h = self.height;
                    let ratio = if track_h > 0.0 {
                        self.content_height / track_h
                    } else {
                        1.0
                    };
                    let max_scroll = (self.content_height - self.height).max(0.0);
                    self.scroll_y =
                        (self.drag_scroll_start_y + delta * ratio).clamp(0.0, max_scroll);
                    EventResponse::Consumed
                }
                Some(DragAxis::Horizontal) => {
                    let delta = *x - self.drag_start_x;
                    let track_w = if self.needs_v_scrollbar() {
                        self.width - SCROLLBAR_WIDTH
                    } else {
                        self.width
                    };
                    let ratio = if track_w > 0.0 {
                        self.content_width / track_w
                    } else {
                        1.0
                    };
                    let max_scroll = (self.content_width - self.width).max(0.0);
                    self.scroll_x =
                        (self.drag_scroll_start_x + delta * ratio).clamp(0.0, max_scroll);
                    EventResponse::Consumed
                }
                None => EventResponse::Ignored,
            },
            Event::KeyDown { key, modifiers } if self.state.focused => {
                let step = self.scroll_speed;
                match key {
                    Key::ArrowUp => {
                        self.scroll_y -= step;
                        self.clamp_scroll();
                        EventResponse::Consumed
                    }
                    Key::ArrowDown => {
                        self.scroll_y += step;
                        self.clamp_scroll();
                        EventResponse::Consumed
                    }
                    Key::ArrowLeft => {
                        self.scroll_x -= step;
                        self.clamp_scroll();
                        EventResponse::Consumed
                    }
                    Key::ArrowRight => {
                        self.scroll_x += step;
                        self.clamp_scroll();
                        EventResponse::Consumed
                    }
                    Key::PageUp => {
                        if modifiers.shift {
                            self.scroll_by_page_x(false);
                        } else {
                            self.scroll_y -= self.page_height();
                            self.clamp_scroll();
                        }
                        EventResponse::Consumed
                    }
                    Key::PageDown => {
                        if modifiers.shift {
                            self.scroll_by_page_x(true);
                        } else {
                            self.scroll_y += self.page_height();
                            self.clamp_scroll();
                        }
                        EventResponse::Consumed
                    }
                    Key::Home => {
                        if modifiers.ctrl {
                            self.scroll_y = 0.0;
                        }
                        self.scroll_x = 0.0;
                        self.clamp_scroll();
                        EventResponse::Consumed
                    }
                    Key::End => {
                        if modifiers.ctrl {
                            self.scroll_y = (self.content_height - self.height).max(0.0);
                        }
                        self.scroll_x = (self.content_width - self.width).max(0.0);
                        self.clamp_scroll();
                        EventResponse::Consumed
                    }
                    Key::Space if modifiers.shift => {
                        self.scroll_y -= self.page_height();
                        self.clamp_scroll();
                        EventResponse::Consumed
                    }
                    Key::Space => {
                        self.scroll_y += self.page_height();
                        self.clamp_scroll();
                        EventResponse::Consumed
                    }
                    _ => EventResponse::Ignored,
                }
            }
            _ => EventResponse::Ignored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_ui_core::{Event, Key, Modifiers, MouseButton};

    fn make_view(w: f32, h: f32, cw: f32, ch: f32) -> ScrollView {
        let mut sv = ScrollView::new();
        sv.layout(0.0, 0.0, w, h);
        sv.set_content_size(cw, ch);
        sv.state.focused = true;
        sv
    }

    #[test]
    fn horizontal_thumb_drag_moves_scroll_x() {
        let mut sv = make_view(200.0, 100.0, 1000.0, 100.0);
        // Hit the horizontal thumb at its starting position.
        let (tx, ty, _, _) = sv.h_thumb_rect();
        let _ = sv.handle_event(&Event::MouseDown {
            x: tx + 1.0,
            y: ty + 1.0,
            button: MouseButton::Left,
        });
        assert_eq!(sv.drag_axis, Some(DragAxis::Horizontal));
        // Drag right by 50 px along the track.
        let _ = sv.handle_event(&Event::MouseMove {
            x: tx + 51.0,
            y: ty + 1.0,
        });
        // Content is 5× the viewport — horizontal thumb drag should yield
        // roughly content_width/track_w × delta pixels of scroll.
        assert!(
            sv.scroll_x > 0.0,
            "scroll_x did not advance after h-thumb drag"
        );
    }

    #[test]
    fn keyboard_pagedown_advances_near_viewport() {
        let mut sv = make_view(200.0, 100.0, 100.0, 1000.0);
        let start = sv.scroll_y;
        let _ = sv.handle_event(&Event::KeyDown {
            key: Key::PageDown,
            modifiers: Modifiers::NONE,
        });
        assert!(
            sv.scroll_y > start + 50.0,
            "PageDown did not scroll ~ one viewport"
        );
    }

    #[test]
    fn keyboard_ctrl_end_jumps_to_bottom() {
        let mut sv = make_view(200.0, 100.0, 100.0, 1000.0);
        let m = Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        };
        let _ = sv.handle_event(&Event::KeyDown {
            key: Key::End,
            modifiers: m,
        });
        assert!((sv.scroll_y - 900.0).abs() < 0.001);
    }
}
