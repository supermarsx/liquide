use crate::atom::ClassAtom;
use crate::extra_data::ExtraData;
use crate::style::ClassStyle;

/// A registered window class definition.
///
/// This mirrors the concept of `WNDCLASSEX` in NT — it bundles together all the
/// properties that every window of a given "kind" shares.
#[derive(Debug, Clone)]
pub struct WindowClass {
    /// Unique atom assigned at registration time.
    pub atom: ClassAtom,
    /// Human-readable class name (e.g. "Button", "FrameWindow").
    pub name: String,
    /// Class style flags.
    pub style: ClassStyle,
    /// Identifies the window procedure / handler function.
    pub handler_id: u64,
    /// Default icon name (if any).
    pub icon: Option<String>,
    /// Default cursor name (if any).
    pub cursor: Option<String>,
    /// Background brush/color in ARGB (if any).
    pub background: Option<u32>,
    /// Default menu name (if any).
    pub menu_name: Option<String>,
    /// Extra bytes allocated per window instance (`cbWndExtra`).
    pub extra_window_bytes: usize,
    /// Extra bytes allocated per class (`cbClsExtra`).
    pub extra_class_bytes: usize,
    /// Module/process that registered this class.
    pub module_id: u64,
    /// System classes cannot be unregistered.
    pub is_system: bool,
    /// For superclassed windows: the handler_id of the base class so the
    /// new handler can call through to the original procedure.
    pub base_handler_id: Option<u64>,
    /// Per-class extra data (shared by all instances).
    pub(crate) class_extra: ExtraData,
}

impl WindowClass {
    /// Create a new `WindowClass` builder with required fields.
    pub fn new(name: impl Into<String>, handler_id: u64, module_id: u64) -> Self {
        Self {
            atom: ClassAtom::NULL,
            name: name.into(),
            style: ClassStyle::NONE,
            handler_id,
            icon: None,
            cursor: None,
            background: None,
            menu_name: None,
            extra_window_bytes: 0,
            extra_class_bytes: 0,
            module_id,
            is_system: false,
            base_handler_id: None,
            class_extra: ExtraData::new(0),
        }
    }

    /// Add class style flags (OR'd with any existing flags).
    pub fn with_style(mut self, style: ClassStyle) -> Self {
        self.style = self.style | style;
        self
    }

    /// Set the default icon name.
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Set the default cursor name.
    pub fn with_cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// Set the background brush/color (ARGB).
    pub fn with_background(mut self, bg: u32) -> Self {
        self.background = Some(bg);
        self
    }

    /// Set the default menu name.
    pub fn with_menu(mut self, menu: impl Into<String>) -> Self {
        self.menu_name = Some(menu.into());
        self
    }

    /// Set extra bytes per window instance.
    pub fn with_extra_window_bytes(mut self, n: usize) -> Self {
        self.extra_window_bytes = n;
        self
    }

    /// Set extra bytes per class.
    pub fn with_extra_class_bytes(mut self, n: usize) -> Self {
        self.extra_class_bytes = n;
        self
    }

    /// Mark as a system class.
    pub fn as_system(mut self) -> Self {
        self.is_system = true;
        self.style |= ClassStyle::GLOBALCLASS;
        self
    }

    /// Returns `true` if this class has the `GLOBALCLASS` style.
    pub fn is_global(&self) -> bool {
        self.style.contains(ClassStyle::GLOBALCLASS)
    }
}

/// Public read-only snapshot of class properties returned by
/// [`ClassRegistry::get_class_info`](crate::registry::ClassRegistry::get_class_info).
#[derive(Debug, Clone)]
pub struct ClassInfo {
    pub atom: ClassAtom,
    pub name: String,
    pub style: ClassStyle,
    pub handler_id: u64,
    pub icon: Option<String>,
    pub cursor: Option<String>,
    pub background: Option<u32>,
    pub menu_name: Option<String>,
    pub extra_window_bytes: usize,
    pub extra_class_bytes: usize,
    pub module_id: u64,
    pub is_system: bool,
    pub base_handler_id: Option<u64>,
}

impl From<&WindowClass> for ClassInfo {
    fn from(c: &WindowClass) -> Self {
        Self {
            atom: c.atom,
            name: c.name.clone(),
            style: c.style,
            handler_id: c.handler_id,
            icon: c.icon.clone(),
            cursor: c.cursor.clone(),
            background: c.background,
            menu_name: c.menu_name.clone(),
            extra_window_bytes: c.extra_window_bytes,
            extra_class_bytes: c.extra_class_bytes,
            module_id: c.module_id,
            is_system: c.is_system,
            base_handler_id: c.base_handler_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_defaults() {
        let wc = WindowClass::new("Test", 1, 100);
        assert_eq!(wc.name, "Test");
        assert_eq!(wc.handler_id, 1);
        assert_eq!(wc.module_id, 100);
        assert!(wc.atom.is_null());
        assert!(wc.style.is_empty());
        assert!(wc.icon.is_none());
        assert_eq!(wc.extra_window_bytes, 0);
        assert!(!wc.is_system);
        assert!(!wc.is_global());
    }

    #[test]
    fn builder_chain() {
        let wc = WindowClass::new("Frame", 2, 1)
            .with_style(ClassStyle::HREDRAW | ClassStyle::VREDRAW)
            .with_icon("app-icon")
            .with_cursor("arrow")
            .with_background(0xFF_00_00_00)
            .with_menu("main-menu")
            .with_extra_window_bytes(16)
            .with_extra_class_bytes(8)
            .as_system();

        assert!(wc.is_system);
        assert!(wc.is_global());
        assert_eq!(wc.icon.as_deref(), Some("app-icon"));
        assert_eq!(wc.cursor.as_deref(), Some("arrow"));
        assert_eq!(wc.background, Some(0xFF_00_00_00));
        assert_eq!(wc.menu_name.as_deref(), Some("main-menu"));
        assert_eq!(wc.extra_window_bytes, 16);
        assert_eq!(wc.extra_class_bytes, 8);
        assert!(wc.style.contains(ClassStyle::HREDRAW));
    }

    #[test]
    fn class_info_from_class() {
        let wc = WindowClass::new("Info", 10, 5).with_icon("ic");
        let info = ClassInfo::from(&wc);
        assert_eq!(info.name, "Info");
        assert_eq!(info.handler_id, 10);
        assert_eq!(info.icon.as_deref(), Some("ic"));
    }
}
