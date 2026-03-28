use std::collections::HashMap;

use crate::{HotkeyAction, HotkeyBackend, HotkeyError, HotkeyId, KeyBinding};

/// Stub hotkey manager — stores bindings but never fires.
/// Used for testing and unsupported platforms.
pub struct GlobalHotkeyManager {
    bindings: HashMap<HotkeyId, (KeyBinding, HotkeyAction)>,
    binding_keys: HashMap<KeyBinding, HotkeyId>,
}

impl GlobalHotkeyManager {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            binding_keys: HashMap::new(),
        }
    }
}

impl HotkeyBackend for GlobalHotkeyManager {
    fn register(
        &mut self,
        binding: KeyBinding,
        action: HotkeyAction,
    ) -> Result<HotkeyId, HotkeyError> {
        if self.binding_keys.contains_key(&binding) {
            return Err(HotkeyError::AlreadyRegistered(binding));
        }
        let id = HotkeyId::next();
        self.bindings.insert(id, (binding, action));
        self.binding_keys.insert(binding, id);
        Ok(id)
    }

    fn unregister(&mut self, id: HotkeyId) -> Result<(), HotkeyError> {
        if let Some((binding, _)) = self.bindings.remove(&id) {
            self.binding_keys.remove(&binding);
            Ok(())
        } else {
            Err(HotkeyError::NotFound(id))
        }
    }

    fn unregister_all(&mut self) {
        self.bindings.clear();
        self.binding_keys.clear();
    }

    fn poll(&mut self) -> Vec<(HotkeyId, HotkeyAction)> {
        Vec::new()
    }

    fn list_bindings(&self) -> Vec<(HotkeyId, KeyBinding, HotkeyAction)> {
        self.bindings
            .iter()
            .map(|(&id, (kb, action))| (id, *kb, action.clone()))
            .collect()
    }
}
