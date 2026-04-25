//! Window type — the top-level container widget.

use liquide_ui_core::{
    Constraints, Event, EventResponse, LayoutResult, MouseButton, Painter, UiColor, UiTheme,
    WidgetId,
    widget::{Widget, WidgetState},
};
use serde::{Deserialize, Serialize};

use crate::frame::ResizeEdge;

/// Window kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WindowKind {
    /// Standard top-level window (QMainWindow).
    Normal,
    /// Dialog window — typically modal, no taskbar entry, smaller.
    Dialog,
    /// Popup window — no decorations, stays above parent.
    Popup,
    /// Splash screen — no decorations, centered.
    Splash,
    /// Tooltip window — tiny, no decorations, auto-dismiss.
    Tooltip,
}

impl Default for WindowKind {
    fn default() -> Self {
        Self::Normal
    }
}

/// Window state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WindowState {
    Normal,
    Minimized,
    Maximized,
    Fullscreen,
    Hidden,
}

impl Default for WindowState {
    fn default() -> Self {
        Self::Normal
    }
}

bitflags::bitflags! {
    /// Window capability flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct WindowFlags: u32 {
        const CLOSABLE    = 0b0000_0001;
        const MINIMIZABLE = 0b0000_0010;
        const MAXIMIZABLE = 0b0000_0100;
        const RESIZABLE   = 0b0000_1000;
        const MOVABLE     = 0b0001_0000;
        const ALWAYS_ON_TOP = 0b0010_0000;
        const FRAMELESS   = 0b0100_0000;
        const TRANSPARENT = 0b1000_0000;
    }
}

impl Default for WindowFlags {
    fn default() -> Self {
        Self::CLOSABLE | Self::MINIMIZABLE | Self::MAXIMIZABLE | Self::RESIZABLE | Self::MOVABLE
    }
}

/// A top-level window container.
pub struct Window {
    state: WidgetState,
    pub kind: WindowKind,
    pub window_state: WindowState,
    pub flags: WindowFlags,
    pub title: String,
    pub icon: Option<u32>,
    pub min_width: f32,
    pub min_height: f32,
    pub max_width: f32,
    pub max_height: f32,
    pub opacity: f32,
    /// Client area child widget IDs.
    children: Vec<WidgetId>,
    // Position / size
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    // Title bar
    pub title_bar: super::title_bar::TitleBar,
    // Frame
    pub frame: super::frame::WindowFrame,
    /// Device-pixel-ratio scale used for chrome sizing (resize tolerance,
    /// shadow blur, traffic-light buttons). 1.0 on standard displays,
    /// 2.0 on retina, etc.
    pub dpi_scale: f32,
    /// Whether the window should render its chrome in right-to-left layout
    /// (mirrors traffic-light buttons to the trailing edge).
    pub rtl: bool,
    active_title_button: Option<usize>,
    active_resize_edge: ResizeEdge,
    resize_start_cursor_x: f32,
    resize_start_cursor_y: f32,
    resize_start_x: f32,
    resize_start_y: f32,
    resize_start_width: f32,
    resize_start_height: f32,
    // Callbacks
    on_close: Option<Box<dyn FnMut() + Send>>,
    on_resize: Option<Box<dyn FnMut(f32, f32) + Send>>,
    on_move: Option<Box<dyn FnMut(f32, f32) + Send>>,
    on_state_change: Option<Box<dyn FnMut(WindowState) + Send>>,
}

impl Window {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            state: WidgetState::new(WidgetId::new()),
            kind: WindowKind::Normal,
            window_state: WindowState::Normal,
            flags: WindowFlags::default(),
            title: title.into(),
            icon: None,
            min_width: 200.0,
            min_height: 150.0,
            max_width: f32::MAX,
            max_height: f32::MAX,
            opacity: 1.0,
            children: Vec::new(),
            x: 100.0,
            y: 100.0,
            width: 640.0,
            height: 480.0,
            title_bar: super::title_bar::TitleBar::new(),
            frame: super::frame::WindowFrame::new(),
            dpi_scale: 1.0,
            rtl: false,
            active_title_button: None,
            active_resize_edge: ResizeEdge::None,
            resize_start_cursor_x: 0.0,
            resize_start_cursor_y: 0.0,
            resize_start_x: 0.0,
            resize_start_y: 0.0,
            resize_start_width: 0.0,
            resize_start_height: 0.0,
            on_close: None,
            on_resize: None,
            on_move: None,
            on_state_change: None,
        }
    }

    pub fn on_close(mut self, f: impl FnMut() + Send + 'static) -> Self {
        self.on_close = Some(Box::new(f));
        self
    }

    pub fn on_resize(mut self, f: impl FnMut(f32, f32) + Send + 'static) -> Self {
        self.on_resize = Some(Box::new(f));
        self
    }

    pub fn on_move(mut self, f: impl FnMut(f32, f32) + Send + 'static) -> Self {
        self.on_move = Some(Box::new(f));
        self
    }

    pub fn on_state_change(mut self, f: impl FnMut(WindowState) + Send + 'static) -> Self {
        self.on_state_change = Some(Box::new(f));
        self
    }

    pub fn add_child(&mut self, id: WidgetId) {
        self.children.push(id);
    }

    pub fn close(&mut self) {
        if self.flags.contains(WindowFlags::CLOSABLE) {
            self.window_state = WindowState::Hidden;
            if let Some(cb) = &mut self.on_close {
                cb();
            }
        }
    }

    pub fn minimize(&mut self) {
        if self.flags.contains(WindowFlags::MINIMIZABLE) {
            self.window_state = WindowState::Minimized;
            if let Some(cb) = &mut self.on_state_change {
                cb(WindowState::Minimized);
            }
        }
    }

    pub fn maximize(&mut self) {
        if self.flags.contains(WindowFlags::MAXIMIZABLE) {
            self.window_state = match self.window_state {
                WindowState::Maximized => WindowState::Normal,
                _ => WindowState::Maximized,
            };
            if let Some(cb) = &mut self.on_state_change {
                cb(self.window_state);
            }
        }
    }

    pub fn set_fullscreen(&mut self, fs: bool) {
        self.window_state = if fs {
            WindowState::Fullscreen
        } else {
            WindowState::Normal
        };
        if let Some(cb) = &mut self.on_state_change {
            cb(self.window_state);
        }
    }

    /// The client area rect (inside the title bar and frame).
    pub fn client_rect(&self) -> (f32, f32, f32, f32) {
        let tb_h = if self.flags.contains(WindowFlags::FRAMELESS) {
            0.0
        } else {
            self.title_bar.height
        };
        let bw = self.frame.style.border_width;
        (
            self.x + bw,
            self.y + tb_h,
            self.width - bw * 2.0,
            self.height - tb_h - bw,
        )
    }

    fn update_title_bar_hover(&mut self, x: f32, y: f32) {
        if self.flags.contains(WindowFlags::FRAMELESS) {
            self.title_bar.set_hovered_button(None);
            return;
        }

        let hovered = self.title_bar.hit_test_button(
            x,
            y,
            self.x,
            self.y,
            self.width,
            self.flags,
            self.rtl,
            self.dpi_scale,
        );
        self.title_bar.set_hovered_button(hovered);
    }

    fn begin_resize(&mut self, edge: ResizeEdge, cursor_x: f32, cursor_y: f32) {
        self.active_resize_edge = edge;
        self.resize_start_cursor_x = cursor_x;
        self.resize_start_cursor_y = cursor_y;
        self.resize_start_x = self.x;
        self.resize_start_y = self.y;
        self.resize_start_width = self.width;
        self.resize_start_height = self.height;
    }

    fn resize_from_cursor(&mut self, cursor_x: f32, cursor_y: f32) {
        let edge = self.active_resize_edge;
        if edge == ResizeEdge::None {
            return;
        }

        let right = self.resize_start_x + self.resize_start_width;
        let bottom = self.resize_start_y + self.resize_start_height;
        let dx = cursor_x - self.resize_start_cursor_x;
        let dy = cursor_y - self.resize_start_cursor_y;

        let mut new_x = self.resize_start_x;
        let mut new_y = self.resize_start_y;
        let mut new_width = self.resize_start_width;
        let mut new_height = self.resize_start_height;

        if matches!(edge, ResizeEdge::Left | ResizeEdge::TopLeft | ResizeEdge::BottomLeft) {
            let candidate_width = right - (self.resize_start_x + dx);
            new_width = candidate_width.clamp(self.min_width, self.max_width);
            new_x = right - new_width;
        }
        if matches!(edge, ResizeEdge::Right | ResizeEdge::TopRight | ResizeEdge::BottomRight) {
            new_width = (self.resize_start_width + dx).clamp(self.min_width, self.max_width);
        }
        if matches!(edge, ResizeEdge::Top | ResizeEdge::TopLeft | ResizeEdge::TopRight) {
            let candidate_height = bottom - (self.resize_start_y + dy);
            new_height = candidate_height.clamp(self.min_height, self.max_height);
            new_y = bottom - new_height;
        }
        if matches!(edge, ResizeEdge::Bottom | ResizeEdge::BottomLeft | ResizeEdge::BottomRight) {
            new_height = (self.resize_start_height + dy).clamp(self.min_height, self.max_height);
        }

        let moved = (new_x - self.x).abs() > f32::EPSILON || (new_y - self.y).abs() > f32::EPSILON;
        let resized = (new_width - self.width).abs() > f32::EPSILON
            || (new_height - self.height).abs() > f32::EPSILON;

        self.x = new_x;
        self.y = new_y;
        self.width = new_width;
        self.height = new_height;

        if moved {
            if let Some(cb) = &mut self.on_move {
                cb(self.x, self.y);
            }
        }
        if resized {
            if let Some(cb) = &mut self.on_resize {
                cb(self.width, self.height);
            }
        }
    }

    fn handle_title_button_action(&mut self, index: usize) {
        match self.title_bar.buttons[index].kind {
            super::title_bar::TitleBarButtonKind::Close => self.close(),
            super::title_bar::TitleBarButtonKind::Minimize => self.minimize(),
            super::title_bar::TitleBarButtonKind::Maximize => self.maximize(),
            super::title_bar::TitleBarButtonKind::AlwaysOnTop => {
                if self.flags.contains(WindowFlags::ALWAYS_ON_TOP) {
                    self.flags.remove(WindowFlags::ALWAYS_ON_TOP);
                } else {
                    self.flags.insert(WindowFlags::ALWAYS_ON_TOP);
                }
            }
        }
    }

    fn reset_chrome_interaction(&mut self) {
        self.active_title_button = None;
        self.active_resize_edge = ResizeEdge::None;
        self.title_bar.clear_interaction_state();
    }
}

impl Widget for Window {
    fn id(&self) -> WidgetId {
        self.state.id
    }
    fn visible(&self) -> bool {
        self.state.visible && self.window_state != WindowState::Hidden
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
        None
    }

    fn measure(&self, constraints: &Constraints, _theme: &UiTheme) -> LayoutResult {
        let (w, h) = constraints.clamp(self.width, self.height);
        LayoutResult::new(w, h)
    }

    fn layout(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.x = x;
        self.y = y;
        self.width = w.clamp(self.min_width, self.max_width);
        self.height = h.clamp(self.min_height, self.max_height);
    }

    fn paint(&self, painter: &mut Painter, theme: &UiTheme) {
        let colors = &theme.colors;
        let frameless = self.flags.contains(WindowFlags::FRAMELESS);
        let s = self.dpi_scale.max(0.25);

        // Window shadow — approximate a Gaussian drop shadow by layering
        // several offset rounded rects with decreasing alpha and slightly
        // growing radii. We can't emit a real blur primitive from this
        // crate (the painter has no BoxShadow op yet), so this widens the
        // footprint proportionally to `FrameStyle::shadow_blur` and
        // `dpi_scale` — visually closer to a blurred shadow than the old
        // single-offset rect.
        if !frameless {
            let style = self.frame.style;
            let base_radius = self.frame.style.corner_radius;
            let blur = style.shadow_blur * s;
            let offset_y = style.shadow_offset_y * s;
            let max_alpha = (255.0 * style.shadow_opacity).clamp(0.0, 255.0) as u32;
            // Five shells widening out from the window rect.
            for k in (1..=5).rev() {
                let t = k as f32 / 5.0;
                let grow = blur * t;
                let a = ((max_alpha as f32) * (1.0 - t) * 0.9 + 10.0).min(255.0) as u8;
                let color = UiColor::new(0, 0, 0, a);
                painter.fill_rounded_rect(
                    self.x - grow,
                    self.y + offset_y - grow,
                    self.width + grow * 2.0,
                    self.height + grow * 2.0,
                    base_radius + grow,
                    color,
                );
            }
        }

        // Window background
        let radius = if frameless {
            0.0
        } else {
            self.frame.style.corner_radius
        };
        painter.fill_rounded_rect(
            self.x,
            self.y,
            self.width,
            self.height,
            radius,
            colors.background,
        );

        // Border — DPI-scaled width.
        if !frameless {
            painter.stroke_rounded_rect(
                self.x,
                self.y,
                self.width,
                self.height,
                radius,
                colors.border,
                self.frame.style.border_width * s,
            );
        }

        // Title bar
        if !frameless {
            self.title_bar.paint(
                painter,
                theme,
                self.x,
                self.y,
                self.width,
                &self.title,
                self.flags,
                self.rtl,
                self.dpi_scale,
            );
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
                if !self.title_bar.dragging && self.active_resize_edge == ResizeEdge::None {
                    self.title_bar.set_hovered_button(None);
                }
                EventResponse::Consumed
            }
            Event::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } => {
                self.state.pressed = true;

                if self.flags.contains(WindowFlags::RESIZABLE)
                    && self.window_state == WindowState::Normal
                {
                    let edge = self.frame.hit_test_resize_scaled(
                        *x,
                        *y,
                        self.x,
                        self.y,
                        self.width,
                        self.height,
                        self.dpi_scale,
                    );
                    if edge.is_some() {
                        self.begin_resize(edge, *x, *y);
                        return EventResponse::RequestFocus;
                    }
                }

                if !self.flags.contains(WindowFlags::FRAMELESS) {
                    if let Some(index) = self.title_bar.hit_test_button(
                        *x,
                        *y,
                        self.x,
                        self.y,
                        self.width,
                        self.flags,
                        self.rtl,
                        self.dpi_scale,
                    ) {
                        self.active_title_button = Some(index);
                        self.title_bar.set_pressed_button(Some(index));
                        return EventResponse::RequestFocus;
                    }

                    if self.flags.contains(WindowFlags::MOVABLE)
                        && self.title_bar.hit_test_drag_region(
                            *x,
                            *y,
                            self.x,
                            self.y,
                            self.width,
                            self.flags,
                            self.rtl,
                            self.dpi_scale,
                        )
                    {
                        self.title_bar.begin_drag(*x, *y, self.x, self.y);
                        return EventResponse::RequestFocus;
                    }
                }

                EventResponse::RequestFocus
            }
            Event::MouseMove { x, y } => {
                if self.active_resize_edge.is_some() {
                    self.resize_from_cursor(*x, *y);
                    return EventResponse::Consumed;
                }

                if self.title_bar.dragging {
                    let (drag_start_x, drag_start_y) = self.title_bar.drag_offset();
                    let new_x = *x - drag_start_x;
                    let new_y = *y - drag_start_y;
                    if (new_x - self.x).abs() > f32::EPSILON || (new_y - self.y).abs() > f32::EPSILON {
                        self.x = new_x;
                        self.y = new_y;
                        if let Some(cb) = &mut self.on_move {
                            cb(self.x, self.y);
                        }
                    }
                    return EventResponse::Consumed;
                }

                self.update_title_bar_hover(*x, *y);

                let in_title_bar = !self.flags.contains(WindowFlags::FRAMELESS)
                    && *x >= self.x
                    && *x < self.x + self.width
                    && *y >= self.y
                    && *y < self.y + self.title_bar.height;
                let on_resize_edge = self.flags.contains(WindowFlags::RESIZABLE)
                    && self
                        .frame
                        .hit_test_resize_scaled(
                            *x,
                            *y,
                            self.x,
                            self.y,
                            self.width,
                            self.height,
                            self.dpi_scale,
                        )
                        .is_some();

                if in_title_bar || on_resize_edge {
                    EventResponse::Consumed
                } else {
                    EventResponse::Ignored
                }
            }
            Event::MouseUp {
                x,
                y,
                button: MouseButton::Left,
            } => {
                self.state.pressed = false;

                if self.active_resize_edge.is_some() {
                    self.active_resize_edge = ResizeEdge::None;
                    return EventResponse::Consumed;
                }

                if self.title_bar.dragging {
                    self.title_bar.dragging = false;
                    return EventResponse::Consumed;
                }

                if let Some(index) = self.active_title_button.take() {
                    let hovered = self.title_bar.hit_test_button(
                        *x,
                        *y,
                        self.x,
                        self.y,
                        self.width,
                        self.flags,
                        self.rtl,
                        self.dpi_scale,
                    );
                    self.title_bar.set_pressed_button(None);
                    if hovered == Some(index) {
                        self.handle_title_button_action(index);
                    }
                    return EventResponse::Consumed;
                }

                EventResponse::Ignored
            }
            Event::FocusIn => {
                self.state.focused = true;
                EventResponse::Consumed
            }
            Event::FocusOut => {
                self.state.focused = false;
                self.reset_chrome_interaction();
                EventResponse::Consumed
            }
            _ => EventResponse::Ignored,
        }
    }

    fn children(&self) -> &[WidgetId] {
        &self.children
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_window_new_defaults() {
        let w = Window::new("Test");
        assert_eq!(w.title, "Test");
        assert_eq!(w.kind, WindowKind::Normal);
        assert_eq!(w.window_state, WindowState::Normal);
        assert_eq!(w.width, 640.0);
        assert_eq!(w.height, 480.0);
        assert_eq!(w.opacity, 1.0);
        assert!(w.flags.contains(WindowFlags::CLOSABLE));
        assert!(w.flags.contains(WindowFlags::RESIZABLE));
    }

    #[test]
    fn test_window_close_sets_hidden() {
        let mut w = Window::new("Test");
        w.close();
        assert_eq!(w.window_state, WindowState::Hidden);
    }

    #[test]
    fn test_window_close_not_closable_noop() {
        let mut w = Window::new("Test");
        w.flags.remove(WindowFlags::CLOSABLE);
        w.close();
        assert_eq!(w.window_state, WindowState::Normal);
    }

    #[test]
    fn test_window_minimize() {
        let mut w = Window::new("Test");
        w.minimize();
        assert_eq!(w.window_state, WindowState::Minimized);
    }

    #[test]
    fn test_window_minimize_not_minimizable_noop() {
        let mut w = Window::new("Test");
        w.flags.remove(WindowFlags::MINIMIZABLE);
        w.minimize();
        assert_eq!(w.window_state, WindowState::Normal);
    }

    #[test]
    fn test_window_maximize_toggle() {
        let mut w = Window::new("Test");
        w.maximize();
        assert_eq!(w.window_state, WindowState::Maximized);
        w.maximize();
        assert_eq!(w.window_state, WindowState::Normal);
    }

    #[test]
    fn test_window_maximize_not_maximizable_noop() {
        let mut w = Window::new("Test");
        w.flags.remove(WindowFlags::MAXIMIZABLE);
        w.maximize();
        assert_eq!(w.window_state, WindowState::Normal);
    }

    #[test]
    fn test_window_fullscreen() {
        let mut w = Window::new("Test");
        w.set_fullscreen(true);
        assert_eq!(w.window_state, WindowState::Fullscreen);
        w.set_fullscreen(false);
        assert_eq!(w.window_state, WindowState::Normal);
    }

    #[test]
    fn test_window_client_rect_with_frame() {
        let w = Window::new("Test");
        let (cx, cy, cw, ch) = w.client_rect();
        // cx = x + border_width, cy = y + title_bar_height
        assert!(cx > w.x);
        assert!(cy > w.y);
        assert!(cw < w.width);
        assert!(ch < w.height);
    }

    #[test]
    fn test_window_client_rect_frameless() {
        let mut w = Window::new("Test");
        w.flags.insert(WindowFlags::FRAMELESS);
        let (_cx, cy, _cw, _ch) = w.client_rect();
        // Frameless: no title bar height offset
        assert_eq!(cy, w.y); // no title_bar offset since FRAMELESS → tb_h = 0
    }

    #[test]
    fn test_window_add_child() {
        let mut w = Window::new("Test");
        let child = WidgetId::new();
        w.add_child(child);
        assert_eq!(w.children.len(), 1);
        assert_eq!(w.children[0], child);
    }

    #[test]
    fn test_window_visible_when_hidden() {
        let mut w = Window::new("Test");
        w.close();
        assert!(!w.visible()); // Widget::visible() is false when Hidden
    }

    #[test]
    fn test_window_flags_default() {
        let flags = WindowFlags::default();
        assert!(flags.contains(WindowFlags::CLOSABLE));
        assert!(flags.contains(WindowFlags::MINIMIZABLE));
        assert!(flags.contains(WindowFlags::MAXIMIZABLE));
        assert!(flags.contains(WindowFlags::RESIZABLE));
        assert!(flags.contains(WindowFlags::MOVABLE));
        assert!(!flags.contains(WindowFlags::ALWAYS_ON_TOP));
        assert!(!flags.contains(WindowFlags::FRAMELESS));
        assert!(!flags.contains(WindowFlags::TRANSPARENT));
    }

    #[test]
    fn test_window_kind_default() {
        assert_eq!(WindowKind::default(), WindowKind::Normal);
    }

    #[test]
    fn test_window_state_default() {
        assert_eq!(WindowState::default(), WindowState::Normal);
    }

    #[test]
    fn test_window_close_button_dispatches() {
        let mut w = Window::new("Test");
        let button_x = w.x + 20.0;
        let button_y = w.y + w.title_bar.height / 2.0;

        let down = w.handle_event(&Event::MouseDown {
            x: button_x,
            y: button_y,
            button: MouseButton::Left,
        });
        assert_eq!(down, EventResponse::RequestFocus);
        let up = w.handle_event(&Event::MouseUp {
            x: button_x,
            y: button_y,
            button: MouseButton::Left,
        });
        assert_eq!(up, EventResponse::Consumed);
        assert_eq!(w.window_state, WindowState::Hidden);
    }

    #[test]
    fn test_window_drag_region_moves_window_and_invokes_callback() {
        let moved = Arc::new(Mutex::new(None));
        let moved_clone = moved.clone();
        let mut w = Window::new("Test").on_move(move |x, y| {
            *moved_clone.lock().unwrap() = Some((x, y));
        });

        let down_x = w.x + w.width / 2.0;
        let down_y = w.y + 10.0;
        let _ = w.handle_event(&Event::MouseDown {
            x: down_x,
            y: down_y,
            button: MouseButton::Left,
        });
        let _ = w.handle_event(&Event::MouseMove {
            x: down_x + 50.0,
            y: down_y + 40.0,
        });
        let _ = w.handle_event(&Event::MouseUp {
            x: down_x + 50.0,
            y: down_y + 40.0,
            button: MouseButton::Left,
        });

        assert_eq!(w.x, 150.0);
        assert_eq!(w.y, 140.0);
        assert_eq!(*moved.lock().unwrap(), Some((150.0, 140.0)));
    }

    #[test]
    fn test_window_resize_edge_updates_geometry_and_callback() {
        let resized = Arc::new(Mutex::new(None));
        let resized_clone = resized.clone();
        let mut w = Window::new("Test").on_resize(move |width, height| {
            *resized_clone.lock().unwrap() = Some((width, height));
        });

        let down_x = w.x + w.width;
        let down_y = w.y + w.height / 2.0;
        let _ = w.handle_event(&Event::MouseDown {
            x: down_x,
            y: down_y,
            button: MouseButton::Left,
        });
        let _ = w.handle_event(&Event::MouseMove {
            x: down_x + 40.0,
            y: down_y,
        });
        let _ = w.handle_event(&Event::MouseUp {
            x: down_x + 40.0,
            y: down_y,
            button: MouseButton::Left,
        });

        assert_eq!(w.width, 680.0);
        assert_eq!(w.height, 480.0);
        assert_eq!(*resized.lock().unwrap(), Some((680.0, 480.0)));
    }
}
