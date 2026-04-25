//! Window builder — fluent API for constructing windows.

use super::frame::FrameStyle;
use super::window::{Window, WindowFlags, WindowKind};

/// Fluent builder for constructing `Window` instances.
pub struct WindowBuilder {
    title: String,
    kind: WindowKind,
    flags: WindowFlags,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    min_width: f32,
    min_height: f32,
    max_width: f32,
    max_height: f32,
    opacity: f32,
    icon: Option<u32>,
    frame_style: Option<FrameStyle>,
    title_bar_height: Option<f32>,
}

impl WindowBuilder {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            kind: WindowKind::Normal,
            flags: WindowFlags::default(),
            x: 100.0,
            y: 100.0,
            width: 640.0,
            height: 480.0,
            min_width: 200.0,
            min_height: 150.0,
            max_width: f32::MAX,
            max_height: f32::MAX,
            opacity: 1.0,
            icon: None,
            frame_style: None,
            title_bar_height: None,
        }
    }

    pub fn kind(mut self, kind: WindowKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn dialog(self) -> Self {
        self.kind(WindowKind::Dialog)
            .flags(WindowFlags::CLOSABLE | WindowFlags::MOVABLE)
    }

    pub fn popup(self) -> Self {
        self.kind(WindowKind::Popup).flags(WindowFlags::FRAMELESS)
    }

    pub fn splash(self) -> Self {
        self.kind(WindowKind::Splash).flags(WindowFlags::FRAMELESS)
    }

    pub fn flags(mut self, flags: WindowFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn closable(mut self, v: bool) -> Self {
        self.flags.set(WindowFlags::CLOSABLE, v);
        self
    }

    pub fn resizable(mut self, v: bool) -> Self {
        self.flags.set(WindowFlags::RESIZABLE, v);
        self
    }

    pub fn frameless(mut self, v: bool) -> Self {
        self.flags.set(WindowFlags::FRAMELESS, v);
        self
    }

    pub fn always_on_top(mut self, v: bool) -> Self {
        self.flags.set(WindowFlags::ALWAYS_ON_TOP, v);
        self
    }

    pub fn position(mut self, x: f32, y: f32) -> Self {
        self.x = x;
        self.y = y;
        self
    }

    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.width = w;
        self.height = h;
        self
    }

    pub fn min_size(mut self, w: f32, h: f32) -> Self {
        self.min_width = w;
        self.min_height = h;
        self
    }

    pub fn max_size(mut self, w: f32, h: f32) -> Self {
        self.max_width = w;
        self.max_height = h;
        self
    }

    pub fn opacity(mut self, o: f32) -> Self {
        self.opacity = o;
        self
    }

    pub fn icon(mut self, icon_id: u32) -> Self {
        self.icon = Some(icon_id);
        self
    }

    pub fn frame_style(mut self, style: FrameStyle) -> Self {
        self.frame_style = Some(style);
        self
    }

    pub fn title_bar_height(mut self, h: f32) -> Self {
        self.title_bar_height = Some(h);
        self
    }

    /// Build the window.
    pub fn build(self) -> Window {
        let mut window = Window::new(self.title);
        window.kind = self.kind;
        window.flags = self.flags;
        window.x = self.x;
        window.y = self.y;
        window.width = self.width;
        window.height = self.height;
        window.min_width = self.min_width;
        window.min_height = self.min_height;
        window.max_width = self.max_width;
        window.max_height = self.max_height;
        window.opacity = self.opacity;
        window.icon = self.icon;
        if let Some(style) = self.frame_style {
            window.frame = super::frame::WindowFrame::with_style(style);
        }
        if let Some(h) = self.title_bar_height {
            window.title_bar.height = h;
        }
        window
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_basic() {
        let w = WindowBuilder::new("Hello").build();
        assert_eq!(w.title, "Hello");
        assert_eq!(w.kind, WindowKind::Normal);
    }

    #[test]
    fn test_builder_dialog() {
        let w = WindowBuilder::new("Dlg").dialog().build();
        assert_eq!(w.kind, WindowKind::Dialog);
        assert!(w.flags.contains(WindowFlags::CLOSABLE));
        assert!(w.flags.contains(WindowFlags::MOVABLE));
        assert!(!w.flags.contains(WindowFlags::RESIZABLE));
    }

    #[test]
    fn test_builder_popup() {
        let w = WindowBuilder::new("Pop").popup().build();
        assert_eq!(w.kind, WindowKind::Popup);
        assert!(w.flags.contains(WindowFlags::FRAMELESS));
    }

    #[test]
    fn test_builder_splash() {
        let w = WindowBuilder::new("Splash").splash().build();
        assert_eq!(w.kind, WindowKind::Splash);
        assert!(w.flags.contains(WindowFlags::FRAMELESS));
    }

    #[test]
    fn test_builder_size_and_position() {
        let w = WindowBuilder::new("Test")
            .position(200.0, 300.0)
            .size(800.0, 600.0)
            .build();
        assert_eq!(w.x, 200.0);
        assert_eq!(w.y, 300.0);
        assert_eq!(w.width, 800.0);
        assert_eq!(w.height, 600.0);
    }

    #[test]
    fn test_builder_min_max_size() {
        let w = WindowBuilder::new("Test")
            .min_size(100.0, 80.0)
            .max_size(2000.0, 1500.0)
            .build();
        assert_eq!(w.min_width, 100.0);
        assert_eq!(w.min_height, 80.0);
        assert_eq!(w.max_width, 2000.0);
        assert_eq!(w.max_height, 1500.0);
    }

    #[test]
    fn test_builder_opacity() {
        let w = WindowBuilder::new("Test").opacity(0.8).build();
        assert_eq!(w.opacity, 0.8);
    }

    #[test]
    fn test_builder_flags() {
        let w = WindowBuilder::new("Test")
            .resizable(false)
            .closable(false)
            .always_on_top(true)
            .build();
        assert!(!w.flags.contains(WindowFlags::RESIZABLE));
        assert!(!w.flags.contains(WindowFlags::CLOSABLE));
        assert!(w.flags.contains(WindowFlags::ALWAYS_ON_TOP));
    }

    #[test]
    fn test_builder_frameless() {
        let w = WindowBuilder::new("Test").frameless(true).build();
        assert!(w.flags.contains(WindowFlags::FRAMELESS));
    }

    #[test]
    fn test_builder_title_bar_height() {
        let w = WindowBuilder::new("Test").title_bar_height(40.0).build();
        assert_eq!(w.title_bar.height, 40.0);
    }

    #[test]
    fn test_builder_icon() {
        let w = WindowBuilder::new("Test").icon(42).build();
        assert_eq!(w.icon, Some(42));
    }
}
