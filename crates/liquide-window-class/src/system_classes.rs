use crate::class::WindowClass;
use crate::registry::ClassRegistry;
use crate::style::ClassStyle;

/// Module ID used for all system classes.
pub const SYSTEM_MODULE_ID: u64 = 0;

/// Handler IDs for built-in system classes. These are well-known constants that
/// the shell / compositor can match against to invoke the correct default
/// window procedure.
pub mod handler {
    pub const DESKTOP_WINDOW: u64 = 0x0001;
    pub const FRAME_WINDOW: u64 = 0x0002;
    pub const POPUP_WINDOW: u64 = 0x0003;
    pub const CHILD_WINDOW: u64 = 0x0004;
    pub const DIALOG_WINDOW: u64 = 0x0005;
    pub const BUTTON: u64 = 0x0010;
    pub const LABEL: u64 = 0x0011;
    pub const TEXT_INPUT: u64 = 0x0012;
    pub const TEXT_AREA: u64 = 0x0013;
    pub const SCROLL_BAR: u64 = 0x0020;
    pub const LIST_BOX: u64 = 0x0021;
    pub const COMBO_BOX: u64 = 0x0022;
    pub const PROGRESS_BAR: u64 = 0x0030;
    pub const SLIDER: u64 = 0x0031;
    pub const TAB_CONTROL: u64 = 0x0032;
    pub const MENU_WINDOW: u64 = 0x0040;
    pub const TOOLTIP_WINDOW: u64 = 0x0041;
}

/// Register all built-in system window classes.
///
/// These classes are always visible to every module and cannot be unregistered.
/// Call this once during desktop initialization.
pub fn register_system_classes(registry: &mut ClassRegistry) {
    let classes = [
        // ── Desktop & top-level containers ──
        system_class("DesktopWindow", handler::DESKTOP_WINDOW)
            .with_style(ClassStyle::HREDRAW | ClassStyle::VREDRAW)
            .with_cursor("arrow")
            .with_background(0xFF_1A_1A_2E), // dark desktop
        system_class("FrameWindow", handler::FRAME_WINDOW)
            .with_style(ClassStyle::HREDRAW | ClassStyle::VREDRAW | ClassStyle::DBLCLKS)
            .with_cursor("arrow")
            .with_icon("window")
            .with_background(0xFF_2D_2D_44)
            .with_extra_window_bytes(32), // room for user data pointers
        system_class("PopupWindow", handler::POPUP_WINDOW)
            .with_style(ClassStyle::SAVEBITS | ClassStyle::DROPSHADOW)
            .with_cursor("arrow"),
        system_class("ChildWindow", handler::CHILD_WINDOW)
            .with_style(ClassStyle::PARENTDC | ClassStyle::DBLCLKS)
            .with_cursor("arrow"),
        system_class("DialogWindow", handler::DIALOG_WINDOW)
            .with_style(
                ClassStyle::SAVEBITS | ClassStyle::DBLCLKS | ClassStyle::DROPSHADOW,
            )
            .with_cursor("arrow")
            .with_background(0xFF_2D_2D_44)
            .with_extra_window_bytes(16), // DLGWINDOWEXTRA equivalent
        // ── Controls ──
        system_class("Button", handler::BUTTON)
            .with_style(ClassStyle::HREDRAW | ClassStyle::VREDRAW | ClassStyle::DBLCLKS | ClassStyle::PARENTDC)
            .with_cursor("arrow")
            .with_extra_window_bytes(8),
        system_class("Label", handler::LABEL)
            .with_style(ClassStyle::PARENTDC | ClassStyle::HREDRAW | ClassStyle::VREDRAW)
            .with_cursor("arrow"),
        system_class("TextInput", handler::TEXT_INPUT)
            .with_style(ClassStyle::DBLCLKS | ClassStyle::PARENTDC)
            .with_cursor("text")
            .with_extra_window_bytes(8),
        system_class("TextArea", handler::TEXT_AREA)
            .with_style(ClassStyle::DBLCLKS | ClassStyle::PARENTDC | ClassStyle::HREDRAW | ClassStyle::VREDRAW)
            .with_cursor("text")
            .with_extra_window_bytes(8),
        system_class("ScrollBar", handler::SCROLL_BAR)
            .with_style(ClassStyle::HREDRAW | ClassStyle::VREDRAW | ClassStyle::PARENTDC)
            .with_cursor("arrow")
            .with_extra_window_bytes(16),
        system_class("ListBox", handler::LIST_BOX)
            .with_style(ClassStyle::DBLCLKS | ClassStyle::PARENTDC)
            .with_cursor("arrow")
            .with_extra_window_bytes(8),
        system_class("ComboBox", handler::COMBO_BOX)
            .with_style(ClassStyle::DBLCLKS | ClassStyle::PARENTDC)
            .with_cursor("arrow")
            .with_extra_window_bytes(16),
        system_class("ProgressBar", handler::PROGRESS_BAR)
            .with_style(ClassStyle::HREDRAW | ClassStyle::VREDRAW)
            .with_cursor("arrow")
            .with_extra_window_bytes(8),
        system_class("Slider", handler::SLIDER)
            .with_style(ClassStyle::HREDRAW | ClassStyle::VREDRAW)
            .with_cursor("arrow")
            .with_extra_window_bytes(8),
        system_class("TabControl", handler::TAB_CONTROL)
            .with_style(ClassStyle::DBLCLKS)
            .with_cursor("arrow")
            .with_extra_window_bytes(16),
        // ── Shell chrome ──
        system_class("MenuWindow", handler::MENU_WINDOW)
            .with_style(ClassStyle::SAVEBITS | ClassStyle::DROPSHADOW)
            .with_cursor("arrow"),
        system_class("TooltipWindow", handler::TOOLTIP_WINDOW)
            .with_style(ClassStyle::SAVEBITS | ClassStyle::DROPSHADOW)
            .with_cursor("arrow"),
    ];

    for class in classes {
        registry
            .register_class(class)
            .expect("system class registration must not fail");
    }
}

/// Helper to construct a system class with GLOBALCLASS flag and `is_system = true`.
fn system_class(name: &str, handler_id: u64) -> WindowClass {
    WindowClass::new(name, handler_id, SYSTEM_MODULE_ID).as_system()
}

/// The number of built-in system classes.
pub const SYSTEM_CLASS_COUNT: usize = 17;

/// Well-known system class names.
pub mod names {
    pub const DESKTOP_WINDOW: &str = "DesktopWindow";
    pub const FRAME_WINDOW: &str = "FrameWindow";
    pub const POPUP_WINDOW: &str = "PopupWindow";
    pub const CHILD_WINDOW: &str = "ChildWindow";
    pub const DIALOG_WINDOW: &str = "DialogWindow";
    pub const BUTTON: &str = "Button";
    pub const LABEL: &str = "Label";
    pub const TEXT_INPUT: &str = "TextInput";
    pub const TEXT_AREA: &str = "TextArea";
    pub const SCROLL_BAR: &str = "ScrollBar";
    pub const LIST_BOX: &str = "ListBox";
    pub const COMBO_BOX: &str = "ComboBox";
    pub const PROGRESS_BAR: &str = "ProgressBar";
    pub const SLIDER: &str = "Slider";
    pub const TAB_CONTROL: &str = "TabControl";
    pub const MENU_WINDOW: &str = "MenuWindow";
    pub const TOOLTIP_WINDOW: &str = "TooltipWindow";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_system_classes_registered() {
        let mut reg = ClassRegistry::new();
        register_system_classes(&mut reg);
        assert_eq!(reg.registered_classes().len(), SYSTEM_CLASS_COUNT);
    }

    #[test]
    fn system_classes_are_system() {
        let mut reg = ClassRegistry::new();
        register_system_classes(&mut reg);
        for atom in reg.registered_classes() {
            let c = reg.find_by_atom(atom).unwrap();
            assert!(c.is_system, "class '{}' should be system", c.name);
        }
    }

    #[test]
    fn system_classes_are_global() {
        let mut reg = ClassRegistry::new();
        register_system_classes(&mut reg);
        for atom in reg.registered_classes() {
            let c = reg.find_by_atom(atom).unwrap();
            assert!(c.is_global(), "class '{}' should be global", c.name);
        }
    }

    #[test]
    fn system_classes_visible_to_any_module() {
        let mut reg = ClassRegistry::new();
        register_system_classes(&mut reg);
        // Any module_id should find FrameWindow
        assert!(reg.find_by_name("FrameWindow", 0).is_some());
        assert!(reg.find_by_name("FrameWindow", 12345).is_some());
        assert!(reg.find_by_name("FrameWindow", u64::MAX).is_some());
    }

    #[test]
    fn system_classes_cannot_unregister() {
        let mut reg = ClassRegistry::new();
        register_system_classes(&mut reg);
        let atom = reg.find_by_name("Button", 0).unwrap().atom;
        assert!(reg.unregister_class(atom).is_err());
    }

    #[test]
    fn frame_window_has_extra_bytes() {
        let mut reg = ClassRegistry::new();
        register_system_classes(&mut reg);
        let frame = reg.find_by_name("FrameWindow", 0).unwrap();
        assert_eq!(frame.extra_window_bytes, 32);
    }

    #[test]
    fn desktop_window_has_background() {
        let mut reg = ClassRegistry::new();
        register_system_classes(&mut reg);
        let desktop = reg.find_by_name("DesktopWindow", 0).unwrap();
        assert!(desktop.background.is_some());
    }

    #[test]
    fn text_input_has_text_cursor() {
        let mut reg = ClassRegistry::new();
        register_system_classes(&mut reg);
        let ti = reg.find_by_name("TextInput", 0).unwrap();
        assert_eq!(ti.cursor.as_deref(), Some("text"));
    }

    #[test]
    fn menu_window_has_dropshadow() {
        let mut reg = ClassRegistry::new();
        register_system_classes(&mut reg);
        let menu = reg.find_by_name("MenuWindow", 0).unwrap();
        assert!(menu.style.contains(ClassStyle::DROPSHADOW));
    }

    #[test]
    fn tooltip_window_has_savebits() {
        let mut reg = ClassRegistry::new();
        register_system_classes(&mut reg);
        let tip = reg.find_by_name("TooltipWindow", 0).unwrap();
        assert!(tip.style.contains(ClassStyle::SAVEBITS));
    }

    #[test]
    fn all_names_findable() {
        let mut reg = ClassRegistry::new();
        register_system_classes(&mut reg);
        let all_names = [
            names::DESKTOP_WINDOW,
            names::FRAME_WINDOW,
            names::POPUP_WINDOW,
            names::CHILD_WINDOW,
            names::DIALOG_WINDOW,
            names::BUTTON,
            names::LABEL,
            names::TEXT_INPUT,
            names::TEXT_AREA,
            names::SCROLL_BAR,
            names::LIST_BOX,
            names::COMBO_BOX,
            names::PROGRESS_BAR,
            names::SLIDER,
            names::TAB_CONTROL,
            names::MENU_WINDOW,
            names::TOOLTIP_WINDOW,
        ];
        for name in all_names {
            assert!(
                reg.find_by_name(name, 0).is_some(),
                "system class '{name}' not found"
            );
        }
    }
}
