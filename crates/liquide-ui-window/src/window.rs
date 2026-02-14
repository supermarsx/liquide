//! Window type — the top-level container widget.

use liquide_ui_core::{
    Constraints, Event, EventResponse, LayoutResult, Painter, UiColor, UiTheme, WidgetId,
    widget::{Widget, WidgetState},
};
use serde::{Deserialize, Serialize};

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
    fn default() -> Self { Self::Normal }
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
    fn default() -> Self { Self::Normal }
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
        self.window_state = if fs { WindowState::Fullscreen } else { WindowState::Normal };
        if let Some(cb) = &mut self.on_state_change {
            cb(self.window_state);
        }
    }

    /// The client area rect (inside the title bar and frame).
    pub fn client_rect(&self) -> (f32, f32, f32, f32) {
        let tb_h = if self.flags.contains(WindowFlags::FRAMELESS) { 0.0 } else { self.title_bar.height };
        let bw = self.frame.style.border_width;
        (
            self.x + bw,
            self.y + tb_h,
            self.width - bw * 2.0,
            self.height - tb_h - bw,
        )
    }
}

impl Widget for Window {
    fn id(&self) -> WidgetId { self.state.id }
    fn visible(&self) -> bool { self.state.visible && self.window_state != WindowState::Hidden }
    fn set_visible(&mut self, v: bool) { self.state.visible = v; }
    fn enabled(&self) -> bool { self.state.enabled }
    fn set_enabled(&mut self, e: bool) { self.state.enabled = e; }
    fn focusable(&self) -> bool { true }
    fn tooltip(&self) -> Option<&str> { None }

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

        // Window shadow (elevated)
        if !frameless {
            let shadow_color = UiColor::new(0, 0, 0, 60);
            painter.fill_rounded_rect(
                self.x + 2.0, self.y + 4.0, self.width, self.height,
                self.frame.style.corner_radius, shadow_color,
            );
        }

        // Window background
        let radius = if frameless { 0.0 } else { self.frame.style.corner_radius };
        painter.fill_rounded_rect(self.x, self.y, self.width, self.height, radius, colors.background);

        // Border
        if !frameless {
            painter.stroke_rounded_rect(
                self.x, self.y, self.width, self.height,
                radius, colors.border, self.frame.style.border_width,
            );
        }

        // Title bar
        if !frameless {
            self.title_bar.paint(painter, theme, self.x, self.y, self.width, &self.title, self.flags);
        }
    }

    fn handle_event(&mut self, event: &Event) -> EventResponse {
        match event {
            Event::MouseEnter => { self.state.hovered = true; EventResponse::Consumed }
            Event::MouseLeave => { self.state.hovered = false; EventResponse::Consumed }
            Event::FocusIn => { self.state.focused = true; EventResponse::Consumed }
            Event::FocusOut => { self.state.focused = false; EventResponse::Consumed }
            _ => EventResponse::Ignored,
        }
    }

    fn children(&self) -> &[WidgetId] {
        &self.children
    }
}
