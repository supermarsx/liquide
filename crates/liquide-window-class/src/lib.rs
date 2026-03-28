//! # liquide-window-class
//!
//! Window class registration system for the LiquiDE desktop environment.
//!
//! Window classes define the shared behaviour and appearance of windows:
//! every window is an instance of exactly one class, and the class determines
//! the default handler, cursor, icon, background, and per-instance extra data
//! size.
//!
//! ## Key types
//!
//! - [`WindowClass`] — the class definition (analogous to `WNDCLASSEX`)
//! - [`ClassAtom`] — unique opaque identifier returned on registration
//! - [`ClassStyle`] — bitflag style constants (`CS_HREDRAW`, etc.)
//! - [`ClassRegistry`] — central registry with hierarchical scoping rules
//! - [`SubclassManager`] — per-instance handler chain management
//! - [`ExtraData`] — raw byte storage for per-window / per-class extra data
//!
//! ## Scoping rules
//!
//! Name lookup follows the NT model:
//! 1. **Private** class (same module_id, not global)
//! 2. **Global** class (`GLOBALCLASS` flag set)
//! 3. **System** class (always visible, cannot be unregistered)

mod atom;
mod class;
mod error;
mod extra_data;
mod registry;
mod style;
mod subclass;
mod system_classes;

pub use atom::ClassAtom;
pub use class::{ClassInfo, WindowClass};
pub use error::ClassError;
pub use style::ClassStyle;
pub use extra_data::{ClassExtraData, ExtraData, ExtraDataError, WindowExtraData};
pub use registry::{field, ClassRegistry};
pub use subclass::{SubclassEntry, SubclassManager};
pub use system_classes::{
    handler, names, register_system_classes, SYSTEM_CLASS_COUNT, SYSTEM_MODULE_ID,
};

#[cfg(test)]
mod tests {
    //! Integration tests that exercise multiple modules together.
    use super::*;

    #[test]
    fn full_lifecycle() {
        let mut reg = ClassRegistry::new();
        register_system_classes(&mut reg);

        // Register an application class
        let app_class = WindowClass::new("MyApp", 1000, 42)
            .with_style(ClassStyle::HREDRAW | ClassStyle::VREDRAW)
            .with_cursor("arrow")
            .with_extra_window_bytes(16);
        let atom = reg.register_class(app_class).unwrap();

        // Find it
        let found = reg.find_by_name("MyApp", 42).unwrap();
        assert_eq!(found.atom, atom);

        // Create some windows
        reg.add_instance(atom);
        reg.add_instance(atom);
        assert_eq!(reg.instance_count(atom), 2);

        // Cannot unregister while windows exist
        assert!(reg.unregister_class(atom).is_err());

        // Destroy windows
        reg.remove_instance(atom);
        reg.remove_instance(atom);

        // Now unregister
        reg.unregister_class(atom).unwrap();
        assert!(reg.find_by_atom(atom).is_none());
    }

    #[test]
    fn superclass_and_subclass_workflow() {
        let mut reg = ClassRegistry::new();
        register_system_classes(&mut reg);

        // Superclass Button
        let btn_atom = reg.find_by_name("Button", 0).unwrap().atom;
        let custom_btn = reg.superclass(btn_atom, "FancyButton", 5000, 1).unwrap();
        let cb = reg.find_by_atom(custom_btn).unwrap();
        assert_eq!(cb.base_handler_id, Some(handler::BUTTON));

        // Subclass a specific window
        let mut sub_mgr = SubclassManager::new();
        let window_id = 100;
        sub_mgr.install(window_id, 6000, 0);
        sub_mgr.install(window_id, 7000, 0);

        let chain = sub_mgr.get_chain(window_id);
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[0].handler_id, 7000); // last installed, first called
        assert_eq!(chain[1].handler_id, 6000);
    }

    #[test]
    fn window_extra_data_lifecycle() {
        let mut reg = ClassRegistry::new();
        let atom = reg
            .register_class(WindowClass::new("Ed", 1, 1).with_extra_window_bytes(24))
            .unwrap();
        let class = reg.find_by_atom(atom).unwrap();

        // Simulate per-window extra data
        let mut wnd_extra = WindowExtraData::new(class.extra_window_bytes);
        wnd_extra.set_ptr(0, 0xDEAD).unwrap();
        wnd_extra.set_long(8, -42).unwrap();
        wnd_extra.set_ptr(16, 0xBEEF).unwrap();

        assert_eq!(wnd_extra.get_ptr(0), Some(0xDEAD));
        assert_eq!(wnd_extra.get_long(8), Some(-42));
        assert_eq!(wnd_extra.get_ptr(16), Some(0xBEEF));
    }

    #[test]
    fn private_class_isolation() {
        let mut reg = ClassRegistry::new();

        // Module 1 registers private "Panel"
        let a1 = reg
            .register_class(WindowClass::new("Panel", 100, 1))
            .unwrap();
        // Module 2 registers its own private "Panel"
        let a2 = reg
            .register_class(WindowClass::new("Panel", 200, 2))
            .unwrap();

        // Each module sees its own
        assert_eq!(reg.find_by_name("Panel", 1).unwrap().atom, a1);
        assert_eq!(reg.find_by_name("Panel", 2).unwrap().atom, a2);
        // Module 3 sees neither (no global/system "Panel")
        assert!(reg.find_by_name("Panel", 3).is_none());
    }

    #[test]
    fn set_class_long_style_and_background() {
        let mut reg = ClassRegistry::new();
        let atom = reg
            .register_class(WindowClass::new("SC", 1, 1))
            .unwrap();

        // Change style
        reg.set_class_long(atom, field::STYLE, ClassStyle::OWNDC.bits() as u64)
            .unwrap();
        assert!(reg.find_by_atom(atom).unwrap().style.contains(ClassStyle::OWNDC));

        // Change background
        reg.set_class_long(atom, field::BACKGROUND, 0xFF_FF_00_00)
            .unwrap();
        assert_eq!(
            reg.find_by_atom(atom).unwrap().background,
            Some(0xFF_FF_00_00)
        );
    }

    #[test]
    fn class_extra_data_shared() {
        let mut reg = ClassRegistry::new();
        let atom = reg
            .register_class(WindowClass::new("Shared", 1, 1).with_extra_class_bytes(8))
            .unwrap();
        reg.get_class_extra_mut(atom)
            .unwrap()
            .set_long(0, 12345)
            .unwrap();
        // All windows of this class would see the same value
        assert_eq!(reg.get_class_extra(atom).unwrap().get_long(0), Some(12345));
    }
}
