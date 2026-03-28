use std::collections::HashMap;

use crate::atom::ClassAtom;
use crate::class::{ClassInfo, WindowClass};
use crate::error::ClassError;
use crate::extra_data::ExtraData;
use crate::style::ClassStyle;

/// Well-known field indices for [`ClassRegistry::set_class_long`].
pub mod field {
    /// Replace the handler_id.
    pub const HANDLER_ID: i32 = -24;
    /// Replace the class style bits.
    pub const STYLE: i32 = -26;
    /// Replace the extra window bytes count (only before any windows created).
    pub const EXTRA_WINDOW_BYTES: i32 = -18;
    /// Replace the extra class bytes count.
    pub const EXTRA_CLASS_BYTES: i32 = -20;
    /// Replace the module_id.
    pub const MODULE_ID: i32 = -16;
    /// Replace the icon name.
    pub const ICON: i32 = -14;
    /// Replace the cursor name.
    pub const CURSOR: i32 = -12;
    /// Replace the background color.
    pub const BACKGROUND: i32 = -10;
    /// Replace the menu name.
    pub const MENU_NAME: i32 = -8;
}

/// Central window class registry.
///
/// Mirrors the NT model: classes are scoped to their registering module unless
/// `GLOBALCLASS` is set.  System classes are always visible and cannot be
/// unregistered.
///
/// **Lookup order**: private (matching module_id) -> global -> system.
pub struct ClassRegistry {
    /// All registered classes keyed by atom.
    classes: HashMap<ClassAtom, WindowClass>,
    /// Per-class instance reference count (live windows using this class).
    instance_counts: HashMap<ClassAtom, usize>,
}

impl ClassRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            classes: HashMap::new(),
            instance_counts: HashMap::new(),
        }
    }

    /// Register a new window class. Returns the assigned [`ClassAtom`].
    ///
    /// Fails if `class.name` is empty or if a class with the same name is
    /// already visible in the same scope.
    pub fn register_class(&mut self, mut class: WindowClass) -> Result<ClassAtom, ClassError> {
        if class.name.is_empty() {
            return Err(ClassError::EmptyName);
        }

        // Duplicate detection — same name in same scope.
        if self.name_conflicts(&class.name, class.module_id, class.is_global() || class.is_system)
        {
            return Err(ClassError::AlreadyRegistered {
                name: class.name.clone(),
            });
        }

        // Allocate class extra data buffer now.
        if class.extra_class_bytes > 0 && class.class_extra.capacity() != class.extra_class_bytes {
            class.class_extra = ExtraData::new(class.extra_class_bytes);
        }

        let atom = ClassAtom::next();
        class.atom = atom;
        self.classes.insert(atom, class);
        self.instance_counts.insert(atom, 0);
        Ok(atom)
    }

    /// Unregister a class. Fails if:
    /// - The atom is unknown.
    /// - The class is a system class.
    /// - There are still windows using this class.
    pub fn unregister_class(&mut self, atom: ClassAtom) -> Result<(), ClassError> {
        let class = self
            .classes
            .get(&atom)
            .ok_or(ClassError::NotFound { atom })?;

        if class.is_system {
            return Err(ClassError::SystemClass { atom });
        }

        let count = self.instance_counts.get(&atom).copied().unwrap_or(0);
        if count > 0 {
            return Err(ClassError::WindowsExist { atom, count });
        }

        self.classes.remove(&atom);
        self.instance_counts.remove(&atom);
        Ok(())
    }

    /// Find a class by name with scoping rules:
    /// 1. Private class owned by `module_id`
    /// 2. Global class (GLOBALCLASS set, any module)
    /// 3. System class
    pub fn find_by_name(&self, name: &str, module_id: u64) -> Option<&WindowClass> {
        // 1. Private (same module, not global, not system)
        let private = self.classes.values().find(|c| {
            c.name == name && c.module_id == module_id && !c.is_global() && !c.is_system
        });
        if private.is_some() {
            return private;
        }

        // 2. Global (GLOBALCLASS, not system)
        let global = self
            .classes
            .values()
            .find(|c| c.name == name && c.is_global() && !c.is_system);
        if global.is_some() {
            return global;
        }

        // 3. System
        self.classes
            .values()
            .find(|c| c.name == name && c.is_system)
    }

    /// Find a class by its atom.
    pub fn find_by_atom(&self, atom: ClassAtom) -> Option<&WindowClass> {
        self.classes.get(&atom)
    }

    /// Get a read-only info snapshot of a class.
    pub fn get_class_info(&self, atom: ClassAtom) -> Option<ClassInfo> {
        self.classes.get(&atom).map(ClassInfo::from)
    }

    /// Modify a class property at runtime.
    ///
    /// `value` is interpreted differently depending on the field index.
    /// Returns the previous value as a `u64`.
    pub fn set_class_long(
        &mut self,
        atom: ClassAtom,
        field_index: i32,
        value: u64,
    ) -> Result<u64, ClassError> {
        let class = self
            .classes
            .get_mut(&atom)
            .ok_or(ClassError::NotFound { atom })?;

        match field_index {
            field::HANDLER_ID => {
                let old = class.handler_id;
                class.handler_id = value;
                Ok(old)
            }
            field::STYLE => {
                let old = class.style.bits() as u64;
                class.style = ClassStyle::from_bits_unchecked(value as u32);
                Ok(old)
            }
            field::MODULE_ID => {
                let old = class.module_id;
                class.module_id = value;
                Ok(old)
            }
            field::BACKGROUND => {
                let old = class.background.unwrap_or(0) as u64;
                class.background = Some(value as u32);
                Ok(old)
            }
            field::EXTRA_WINDOW_BYTES => {
                let old = class.extra_window_bytes as u64;
                class.extra_window_bytes = value as usize;
                Ok(old)
            }
            field::EXTRA_CLASS_BYTES => {
                let old = class.extra_class_bytes as u64;
                class.extra_class_bytes = value as usize;
                Ok(old)
            }
            _ => Err(ClassError::InvalidField {
                field: field_index,
            }),
        }
    }

    /// Returns all registered class atoms.
    pub fn registered_classes(&self) -> Vec<ClassAtom> {
        self.classes.keys().copied().collect()
    }

    /// Returns atoms for classes registered by a given module.
    pub fn classes_for_module(&self, module_id: u64) -> Vec<ClassAtom> {
        self.classes
            .values()
            .filter(|c| c.module_id == module_id)
            .map(|c| c.atom)
            .collect()
    }

    /// Increment the live-window count for a class. Called when a window is
    /// created with this class.
    pub fn add_instance(&mut self, atom: ClassAtom) {
        *self.instance_counts.entry(atom).or_insert(0) += 1;
    }

    /// Decrement the live-window count. Called when a window is destroyed.
    pub fn remove_instance(&mut self, atom: ClassAtom) {
        if let Some(count) = self.instance_counts.get_mut(&atom) {
            *count = count.saturating_sub(1);
        }
    }

    /// Query the current window instance count for a class.
    pub fn instance_count(&self, atom: ClassAtom) -> usize {
        self.instance_counts.get(&atom).copied().unwrap_or(0)
    }

    /// Read per-class extra data.
    pub fn get_class_extra(&self, atom: ClassAtom) -> Option<&ExtraData> {
        self.classes.get(&atom).map(|c| &c.class_extra)
    }

    /// Mutable access to per-class extra data.
    pub fn get_class_extra_mut(&mut self, atom: ClassAtom) -> Option<&mut ExtraData> {
        self.classes.get_mut(&atom).map(|c| &mut c.class_extra)
    }

    /// Superclass: create a new class based on an existing one with a new name
    /// and handler. The original handler_id is preserved as `base_handler_id`.
    pub fn superclass(
        &mut self,
        base_atom: ClassAtom,
        new_name: &str,
        new_handler_id: u64,
        new_module_id: u64,
    ) -> Result<ClassAtom, ClassError> {
        let base = self
            .classes
            .get(&base_atom)
            .ok_or(ClassError::NotFound { atom: base_atom })?
            .clone();

        if new_name.is_empty() {
            return Err(ClassError::EmptyName);
        }

        let is_global_or_system = base.is_global();
        if self.name_conflicts(new_name, new_module_id, is_global_or_system) {
            return Err(ClassError::AlreadyRegistered {
                name: new_name.to_string(),
            });
        }

        let atom = ClassAtom::next();
        let new_class = WindowClass {
            atom,
            name: new_name.to_string(),
            style: base.style & !ClassStyle::GLOBALCLASS, // private by default
            handler_id: new_handler_id,
            icon: base.icon,
            cursor: base.cursor,
            background: base.background,
            menu_name: base.menu_name,
            extra_window_bytes: base.extra_window_bytes,
            extra_class_bytes: base.extra_class_bytes,
            module_id: new_module_id,
            is_system: false,
            base_handler_id: Some(base.handler_id),
            class_extra: ExtraData::new(base.extra_class_bytes),
        };

        self.classes.insert(atom, new_class);
        self.instance_counts.insert(atom, 0);
        Ok(atom)
    }

    // ---- internal helpers ----

    /// Returns `true` if registering a class with this name would conflict.
    fn name_conflicts(&self, name: &str, module_id: u64, is_global: bool) -> bool {
        for c in self.classes.values() {
            if c.name != name {
                continue;
            }
            // Same module private class
            if c.module_id == module_id && !c.is_global() && !c.is_system {
                return true;
            }
            // Global or system class with same name
            if (c.is_global() || c.is_system) && is_global {
                return true;
            }
            // Existing global/system conflicts with any new class of same name
            if c.is_system {
                return true;
            }
        }
        false
    }
}

impl Default for ClassRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::class::WindowClass;
    use crate::style::ClassStyle;

    fn make_class(name: &str, module_id: u64) -> WindowClass {
        WindowClass::new(name, 1, module_id)
    }

    #[test]
    fn register_and_find_by_atom() {
        let mut reg = ClassRegistry::new();
        let atom = reg.register_class(make_class("Foo", 1)).unwrap();
        assert!(reg.find_by_atom(atom).is_some());
        assert_eq!(reg.find_by_atom(atom).unwrap().name, "Foo");
    }

    #[test]
    fn register_and_find_by_name() {
        let mut reg = ClassRegistry::new();
        reg.register_class(make_class("Bar", 1)).unwrap();
        assert!(reg.find_by_name("Bar", 1).is_some());
    }

    #[test]
    fn duplicate_private_class_fails() {
        let mut reg = ClassRegistry::new();
        reg.register_class(make_class("Dup", 1)).unwrap();
        assert!(reg.register_class(make_class("Dup", 1)).is_err());
    }

    #[test]
    fn same_name_different_module_ok() {
        let mut reg = ClassRegistry::new();
        reg.register_class(make_class("Priv", 1)).unwrap();
        // Different module can register same private name
        assert!(reg.register_class(make_class("Priv", 2)).is_ok());
    }

    #[test]
    fn unregister_basic() {
        let mut reg = ClassRegistry::new();
        let atom = reg.register_class(make_class("Gone", 1)).unwrap();
        reg.unregister_class(atom).unwrap();
        assert!(reg.find_by_atom(atom).is_none());
    }

    #[test]
    fn unregister_system_fails() {
        let mut reg = ClassRegistry::new();
        let atom = reg
            .register_class(make_class("Sys", 0).as_system())
            .unwrap();
        assert_eq!(
            reg.unregister_class(atom),
            Err(ClassError::SystemClass { atom })
        );
    }

    #[test]
    fn unregister_with_windows_fails() {
        let mut reg = ClassRegistry::new();
        let atom = reg.register_class(make_class("Busy", 1)).unwrap();
        reg.add_instance(atom);
        assert!(matches!(
            reg.unregister_class(atom),
            Err(ClassError::WindowsExist { .. })
        ));
        reg.remove_instance(atom);
        assert!(reg.unregister_class(atom).is_ok());
    }

    #[test]
    fn scoping_private_before_global() {
        let mut reg = ClassRegistry::new();
        let _global = reg
            .register_class(
                WindowClass::new("Btn", 10, 99).with_style(ClassStyle::GLOBALCLASS),
            )
            .unwrap();
        let private = reg.register_class(make_class("Btn", 1)).unwrap();

        // Module 1 sees its private class
        assert_eq!(reg.find_by_name("Btn", 1).unwrap().atom, private);
        // Module 2 (no private) sees global
        assert_eq!(reg.find_by_name("Btn", 2).unwrap().handler_id, 10);
    }

    #[test]
    fn scoping_global_before_system() {
        let mut reg = ClassRegistry::new();
        let sys = reg
            .register_class(WindowClass::new("Ctrl", 1, 0).as_system())
            .unwrap();
        // global cannot conflict with system name
        let global_result = reg.register_class(
            WindowClass::new("Ctrl", 2, 50).with_style(ClassStyle::GLOBALCLASS),
        );
        // System class already exists with that name, so global registration
        // should fail because system class name conflicts.
        assert!(global_result.is_err());

        // System is still visible
        assert_eq!(reg.find_by_name("Ctrl", 50).unwrap().atom, sys);
    }

    #[test]
    fn set_class_long_handler() {
        let mut reg = ClassRegistry::new();
        let atom = reg.register_class(make_class("Mod", 1)).unwrap();
        let old = reg
            .set_class_long(atom, field::HANDLER_ID, 42)
            .unwrap();
        assert_eq!(old, 1); // original handler_id
        assert_eq!(reg.find_by_atom(atom).unwrap().handler_id, 42);
    }

    #[test]
    fn set_class_long_invalid_field() {
        let mut reg = ClassRegistry::new();
        let atom = reg.register_class(make_class("X", 1)).unwrap();
        assert!(reg.set_class_long(atom, 9999, 0).is_err());
    }

    #[test]
    fn registered_classes_list() {
        let mut reg = ClassRegistry::new();
        let a1 = reg.register_class(make_class("A", 1)).unwrap();
        let a2 = reg.register_class(make_class("B", 1)).unwrap();
        let list = reg.registered_classes();
        assert!(list.contains(&a1));
        assert!(list.contains(&a2));
    }

    #[test]
    fn classes_for_module() {
        let mut reg = ClassRegistry::new();
        reg.register_class(make_class("M1", 1)).unwrap();
        reg.register_class(make_class("M2", 2)).unwrap();
        reg.register_class(make_class("M3", 1)).unwrap();
        assert_eq!(reg.classes_for_module(1).len(), 2);
        assert_eq!(reg.classes_for_module(2).len(), 1);
        assert_eq!(reg.classes_for_module(999).len(), 0);
    }

    #[test]
    fn empty_name_rejected() {
        let mut reg = ClassRegistry::new();
        assert_eq!(
            reg.register_class(make_class("", 1)),
            Err(ClassError::EmptyName)
        );
    }

    #[test]
    fn get_class_info() {
        let mut reg = ClassRegistry::new();
        let atom = reg
            .register_class(make_class("Info", 1).with_icon("ic"))
            .unwrap();
        let info = reg.get_class_info(atom).unwrap();
        assert_eq!(info.name, "Info");
        assert_eq!(info.icon.as_deref(), Some("ic"));
    }

    #[test]
    fn class_extra_data() {
        let mut reg = ClassRegistry::new();
        let atom = reg
            .register_class(make_class("Extra", 1).with_extra_class_bytes(16))
            .unwrap();
        reg.get_class_extra_mut(atom)
            .unwrap()
            .set_long(0, 0xCAFE)
            .unwrap();
        assert_eq!(reg.get_class_extra(atom).unwrap().get_long(0), Some(0xCAFE));
    }

    #[test]
    fn instance_count_tracking() {
        let mut reg = ClassRegistry::new();
        let atom = reg.register_class(make_class("IC", 1)).unwrap();
        assert_eq!(reg.instance_count(atom), 0);
        reg.add_instance(atom);
        reg.add_instance(atom);
        assert_eq!(reg.instance_count(atom), 2);
        reg.remove_instance(atom);
        assert_eq!(reg.instance_count(atom), 1);
    }

    #[test]
    fn superclass_basic() {
        let mut reg = ClassRegistry::new();
        let base = reg
            .register_class(
                WindowClass::new("Base", 100, 1)
                    .with_cursor("arrow")
                    .with_extra_window_bytes(8),
            )
            .unwrap();
        let derived = reg.superclass(base, "Derived", 200, 1).unwrap();
        let dc = reg.find_by_atom(derived).unwrap();
        assert_eq!(dc.name, "Derived");
        assert_eq!(dc.handler_id, 200);
        assert_eq!(dc.base_handler_id, Some(100));
        assert_eq!(dc.cursor.as_deref(), Some("arrow"));
        assert_eq!(dc.extra_window_bytes, 8);
    }

    #[test]
    fn superclass_unknown_base() {
        let mut reg = ClassRegistry::new();
        assert!(reg
            .superclass(ClassAtom::from_raw(9999), "X", 1, 1)
            .is_err());
    }
}
