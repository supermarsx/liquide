use crate::action::ShortcutAction;
use crate::binding::{KeyBinding, KeyCode};

/// Determines in which input context a shortcut is active.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ShortcutContext {
    Global,
    Window,
    TextInput,
    Menu,
    Overview,
    Custom(String),
}

/// Where the shortcut registration originated.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ShortcutSource {
    BuiltIn,
    User,
    Extension(String),
}

/// A single registered shortcut entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutEntry {
    pub binding: KeyBinding,
    pub action: ShortcutAction,
    pub context: ShortcutContext,
    pub source: ShortcutSource,
    pub enabled: bool,
}

/// Error returned when a shortcut conflicts with an existing registration.
#[derive(Debug, Clone)]
pub struct ConflictError {
    pub existing: ShortcutEntry,
}

impl std::fmt::Display for ConflictError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "shortcut conflict: {} is already bound to {:?} in context {:?}",
            self.existing.binding.to_string(),
            self.existing.action,
            self.existing.context,
        )
    }
}

impl std::error::Error for ConflictError {}

/// Context priority order for lookup. Lower index = higher priority.
/// More specific contexts take precedence over Global.
fn context_priority(ctx: &ShortcutContext) -> u8 {
    match ctx {
        ShortcutContext::TextInput => 0,
        ShortcutContext::Menu => 1,
        ShortcutContext::Overview => 2,
        ShortcutContext::Window => 3,
        ShortcutContext::Custom(_) => 4,
        ShortcutContext::Global => 5,
    }
}

/// Central shortcut registry with conflict detection and context-aware lookup.
pub struct ShortcutRegistry {
    entries: Vec<ShortcutEntry>,
}

impl ShortcutRegistry {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Register a new shortcut entry. Returns `ConflictError` if the same binding
    /// is already registered in the same context.
    pub fn register(&mut self, entry: ShortcutEntry) -> Result<(), ConflictError> {
        // Check for conflicts in the same context
        for existing in &self.entries {
            if existing.binding == entry.binding
                && existing.context == entry.context
                && existing.enabled
            {
                return Err(ConflictError {
                    existing: existing.clone(),
                });
            }
        }
        self.entries.push(entry);
        Ok(())
    }

    /// Remove the shortcut matching the given binding and context.
    /// Returns `true` if an entry was removed.
    pub fn unregister(&mut self, binding: &KeyBinding, context: &ShortcutContext) -> bool {
        let before = self.entries.len();
        self.entries
            .retain(|e| !(e.binding == *binding && e.context == *context));
        self.entries.len() < before
    }

    /// Look up the action for a key event given the currently active contexts.
    /// Contexts are checked in priority order (most specific first); Global is always
    /// checked last as a fallback.
    pub fn lookup(
        &self,
        modifiers: u8,
        key: &KeyCode,
        contexts: &[ShortcutContext],
    ) -> Option<&ShortcutAction> {
        // Build a list of contexts sorted by priority
        let mut sorted_contexts: Vec<&ShortcutContext> = contexts.iter().collect();
        sorted_contexts.sort_by_key(|c| context_priority(c));

        for ctx in &sorted_contexts {
            for entry in &self.entries {
                if entry.enabled && entry.binding.matches(modifiers, key) && entry.context == **ctx
                {
                    return Some(&entry.action);
                }
            }
        }
        None
    }

    /// Find all entries that conflict with the given binding in the given context.
    pub fn conflicts(
        &self,
        binding: &KeyBinding,
        context: &ShortcutContext,
    ) -> Vec<&ShortcutEntry> {
        self.entries
            .iter()
            .filter(|e| e.binding == *binding && e.context == *context && e.enabled)
            .collect()
    }

    /// Rebind an existing action to a new key binding. Returns the old binding if
    /// one was replaced, or `ConflictError` if the new binding conflicts with a
    /// different action in the same context.
    pub fn rebind(
        &mut self,
        action: &ShortcutAction,
        new_binding: KeyBinding,
    ) -> Result<Option<KeyBinding>, ConflictError> {
        // Find the entry with this action
        let idx = self.entries.iter().position(|e| e.action == *action);
        let idx = match idx {
            Some(i) => i,
            None => return Ok(None),
        };

        let context = self.entries[idx].context.clone();

        // Check for conflicts with the new binding (excluding the entry being rebound)
        for (i, existing) in self.entries.iter().enumerate() {
            if i != idx
                && existing.binding == new_binding
                && existing.context == context
                && existing.enabled
            {
                return Err(ConflictError {
                    existing: existing.clone(),
                });
            }
        }

        let old_binding = self.entries[idx].binding;
        self.entries[idx].binding = new_binding;
        Ok(Some(old_binding))
    }

    /// Return all entries registered for a given context.
    pub fn entries_for_context(&self, context: &ShortcutContext) -> Vec<&ShortcutEntry> {
        self.entries
            .iter()
            .filter(|e| e.context == *context)
            .collect()
    }

    /// Search entries by action display name or binding string (case-insensitive substring match).
    pub fn search(&self, query: &str) -> Vec<&ShortcutEntry> {
        let query_lower = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                let action_name = crate::action::action_display_name(&e.action).to_lowercase();
                let binding_str = e.binding.to_string().to_lowercase();
                action_name.contains(&query_lower) || binding_str.contains(&query_lower)
            })
            .collect()
    }

    /// Return all registered entries.
    pub fn all_entries(&self) -> &[ShortcutEntry] {
        &self.entries
    }

    /// Total number of registered entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Remove all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Default for ShortcutRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::*;
    use crate::binding::*;

    fn entry(
        mods: u8,
        key: KeyCode,
        action: ShortcutAction,
        ctx: ShortcutContext,
    ) -> ShortcutEntry {
        ShortcutEntry {
            binding: KeyBinding::new(mods, key),
            action,
            context: ctx,
            source: ShortcutSource::BuiltIn,
            enabled: true,
        }
    }

    #[test]
    fn register_and_lookup() {
        let mut reg = ShortcutRegistry::new();
        reg.register(entry(
            MOD_ALT,
            KeyCode::F4,
            ShortcutAction::Window(WindowAction::Close),
            ShortcutContext::Global,
        ))
        .unwrap();

        let result = reg.lookup(MOD_ALT, &KeyCode::F4, &[ShortcutContext::Global]);
        assert_eq!(result, Some(&ShortcutAction::Window(WindowAction::Close)));
    }

    #[test]
    fn lookup_no_match() {
        let reg = ShortcutRegistry::new();
        assert!(
            reg.lookup(MOD_CTRL, &KeyCode::A, &[ShortcutContext::Global])
                .is_none()
        );
    }

    #[test]
    fn conflict_detection_same_context() {
        let mut reg = ShortcutRegistry::new();
        reg.register(entry(
            MOD_CTRL,
            KeyCode::S,
            ShortcutAction::Custom("save".into()),
            ShortcutContext::Window,
        ))
        .unwrap();

        let result = reg.register(entry(
            MOD_CTRL,
            KeyCode::S,
            ShortcutAction::Custom("other".into()),
            ShortcutContext::Window,
        ));
        assert!(result.is_err());
    }

    #[test]
    fn no_conflict_different_context() {
        let mut reg = ShortcutRegistry::new();
        reg.register(entry(
            MOD_CTRL,
            KeyCode::S,
            ShortcutAction::Custom("save".into()),
            ShortcutContext::Window,
        ))
        .unwrap();

        // Same binding, different context — should succeed
        reg.register(entry(
            MOD_CTRL,
            KeyCode::S,
            ShortcutAction::Custom("search".into()),
            ShortcutContext::TextInput,
        ))
        .unwrap();

        assert_eq!(reg.len(), 2);
    }

    #[test]
    fn context_priority_lookup() {
        let mut reg = ShortcutRegistry::new();

        // Global binding
        reg.register(entry(
            MOD_CTRL,
            KeyCode::C,
            ShortcutAction::Custom("global-copy".into()),
            ShortcutContext::Global,
        ))
        .unwrap();

        // TextInput binding (higher priority)
        reg.register(entry(
            MOD_CTRL,
            KeyCode::C,
            ShortcutAction::Custom("text-copy".into()),
            ShortcutContext::TextInput,
        ))
        .unwrap();

        // When both contexts are active, TextInput wins
        let result = reg.lookup(
            MOD_CTRL,
            &KeyCode::C,
            &[ShortcutContext::Global, ShortcutContext::TextInput],
        );
        assert_eq!(result, Some(&ShortcutAction::Custom("text-copy".into())));
    }

    #[test]
    fn context_priority_global_fallback() {
        let mut reg = ShortcutRegistry::new();

        reg.register(entry(
            MOD_SUPER,
            KeyCode::L,
            ShortcutAction::Desktop(DesktopAction::LockScreen),
            ShortcutContext::Global,
        ))
        .unwrap();

        // Lookup with Window context — no Window binding, falls back to Global
        let result = reg.lookup(
            MOD_SUPER,
            &KeyCode::L,
            &[ShortcutContext::Window, ShortcutContext::Global],
        );
        assert_eq!(
            result,
            Some(&ShortcutAction::Desktop(DesktopAction::LockScreen))
        );
    }

    #[test]
    fn unregister() {
        let mut reg = ShortcutRegistry::new();
        let binding = KeyBinding::new(MOD_CTRL, KeyCode::W);
        reg.register(ShortcutEntry {
            binding,
            action: ShortcutAction::Window(WindowAction::Close),
            context: ShortcutContext::Window,
            source: ShortcutSource::BuiltIn,
            enabled: true,
        })
        .unwrap();

        assert!(reg.unregister(&binding, &ShortcutContext::Window));
        assert!(reg.is_empty());
    }

    #[test]
    fn unregister_not_found() {
        let mut reg = ShortcutRegistry::new();
        let binding = KeyBinding::new(MOD_CTRL, KeyCode::Q);
        assert!(!reg.unregister(&binding, &ShortcutContext::Global));
    }

    #[test]
    fn rebind_success() {
        let mut reg = ShortcutRegistry::new();
        let action = ShortcutAction::Window(WindowAction::Close);
        reg.register(entry(
            MOD_ALT,
            KeyCode::F4,
            action.clone(),
            ShortcutContext::Global,
        ))
        .unwrap();

        let old = reg
            .rebind(&action, KeyBinding::new(MOD_CTRL, KeyCode::W))
            .unwrap();
        assert_eq!(old, Some(KeyBinding::new(MOD_ALT, KeyCode::F4)));

        // New binding should work
        let result = reg.lookup(MOD_CTRL, &KeyCode::W, &[ShortcutContext::Global]);
        assert_eq!(result, Some(&action));

        // Old binding should not
        let result = reg.lookup(MOD_ALT, &KeyCode::F4, &[ShortcutContext::Global]);
        assert!(result.is_none());
    }

    #[test]
    fn rebind_conflict() {
        let mut reg = ShortcutRegistry::new();
        reg.register(entry(
            MOD_ALT,
            KeyCode::F4,
            ShortcutAction::Window(WindowAction::Close),
            ShortcutContext::Global,
        ))
        .unwrap();
        reg.register(entry(
            MOD_CTRL,
            KeyCode::W,
            ShortcutAction::Window(WindowAction::Minimize),
            ShortcutContext::Global,
        ))
        .unwrap();

        let result = reg.rebind(
            &ShortcutAction::Window(WindowAction::Close),
            KeyBinding::new(MOD_CTRL, KeyCode::W),
        );
        assert!(result.is_err());
    }

    #[test]
    fn rebind_action_not_found() {
        let mut reg = ShortcutRegistry::new();
        let result = reg.rebind(
            &ShortcutAction::Custom("nonexistent".into()),
            KeyBinding::new(MOD_CTRL, KeyCode::A),
        );
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn entries_for_context() {
        let mut reg = ShortcutRegistry::new();
        reg.register(entry(
            MOD_CTRL,
            KeyCode::S,
            ShortcutAction::Custom("save".into()),
            ShortcutContext::Window,
        ))
        .unwrap();
        reg.register(entry(
            MOD_CTRL,
            KeyCode::C,
            ShortcutAction::Custom("copy".into()),
            ShortcutContext::TextInput,
        ))
        .unwrap();
        reg.register(entry(
            MOD_CTRL,
            KeyCode::V,
            ShortcutAction::Custom("paste".into()),
            ShortcutContext::Window,
        ))
        .unwrap();

        let window_entries = reg.entries_for_context(&ShortcutContext::Window);
        assert_eq!(window_entries.len(), 2);
        let text_entries = reg.entries_for_context(&ShortcutContext::TextInput);
        assert_eq!(text_entries.len(), 1);
    }

    #[test]
    fn search_by_action_name() {
        let mut reg = ShortcutRegistry::new();
        reg.register(entry(
            MOD_ALT,
            KeyCode::F4,
            ShortcutAction::Window(WindowAction::Close),
            ShortcutContext::Global,
        ))
        .unwrap();
        reg.register(entry(
            MOD_SUPER,
            KeyCode::L,
            ShortcutAction::Desktop(DesktopAction::LockScreen),
            ShortcutContext::Global,
        ))
        .unwrap();

        let results = reg.search("close");
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].action,
            ShortcutAction::Window(WindowAction::Close)
        );
    }

    #[test]
    fn search_by_binding_string() {
        let mut reg = ShortcutRegistry::new();
        reg.register(entry(
            MOD_ALT,
            KeyCode::F4,
            ShortcutAction::Window(WindowAction::Close),
            ShortcutContext::Global,
        ))
        .unwrap();
        reg.register(entry(
            MOD_SUPER,
            KeyCode::L,
            ShortcutAction::Desktop(DesktopAction::LockScreen),
            ShortcutContext::Global,
        ))
        .unwrap();

        let results = reg.search("Alt+F4");
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].action,
            ShortcutAction::Window(WindowAction::Close)
        );
    }

    #[test]
    fn search_case_insensitive() {
        let mut reg = ShortcutRegistry::new();
        reg.register(entry(
            MOD_NONE,
            KeyCode::PrintScreen,
            ShortcutAction::Desktop(DesktopAction::Screenshot),
            ShortcutContext::Global,
        ))
        .unwrap();

        let results = reg.search("screenshot");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn search_no_results() {
        let reg = ShortcutRegistry::new();
        let results = reg.search("nonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn conflicts_method() {
        let mut reg = ShortcutRegistry::new();
        reg.register(entry(
            MOD_CTRL,
            KeyCode::S,
            ShortcutAction::Custom("save".into()),
            ShortcutContext::Window,
        ))
        .unwrap();

        let binding = KeyBinding::new(MOD_CTRL, KeyCode::S);
        let conflicts = reg.conflicts(&binding, &ShortcutContext::Window);
        assert_eq!(conflicts.len(), 1);

        let conflicts = reg.conflicts(&binding, &ShortcutContext::Global);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn disabled_entry_not_looked_up() {
        let mut reg = ShortcutRegistry::new();
        reg.register(ShortcutEntry {
            binding: KeyBinding::new(MOD_CTRL, KeyCode::D),
            action: ShortcutAction::Custom("disabled".into()),
            context: ShortcutContext::Global,
            source: ShortcutSource::BuiltIn,
            enabled: false,
        })
        .unwrap();

        assert!(
            reg.lookup(MOD_CTRL, &KeyCode::D, &[ShortcutContext::Global])
                .is_none()
        );
    }

    #[test]
    fn disabled_entry_no_conflict() {
        let mut reg = ShortcutRegistry::new();
        reg.register(ShortcutEntry {
            binding: KeyBinding::new(MOD_CTRL, KeyCode::D),
            action: ShortcutAction::Custom("disabled".into()),
            context: ShortcutContext::Global,
            source: ShortcutSource::BuiltIn,
            enabled: false,
        })
        .unwrap();

        // Same binding, same context — should succeed because the first is disabled
        reg.register(entry(
            MOD_CTRL,
            KeyCode::D,
            ShortcutAction::Custom("active".into()),
            ShortcutContext::Global,
        ))
        .unwrap();

        assert_eq!(reg.len(), 2);
    }

    #[test]
    fn conflict_error_display() {
        let err = ConflictError {
            existing: ShortcutEntry {
                binding: KeyBinding::new(MOD_CTRL, KeyCode::S),
                action: ShortcutAction::Custom("save".into()),
                context: ShortcutContext::Window,
                source: ShortcutSource::BuiltIn,
                enabled: true,
            },
        };
        let msg = format!("{}", err);
        assert!(msg.contains("conflict"));
        assert!(msg.contains("Ctrl+S"));
    }

    #[test]
    fn clear_registry() {
        let mut reg = ShortcutRegistry::new();
        reg.register(entry(
            MOD_CTRL,
            KeyCode::A,
            ShortcutAction::Custom("a".into()),
            ShortcutContext::Global,
        ))
        .unwrap();
        assert!(!reg.is_empty());
        reg.clear();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
    }

    #[test]
    fn all_entries_returns_slice() {
        let mut reg = ShortcutRegistry::new();
        reg.register(entry(
            MOD_CTRL,
            KeyCode::A,
            ShortcutAction::Custom("a".into()),
            ShortcutContext::Global,
        ))
        .unwrap();
        reg.register(entry(
            MOD_CTRL,
            KeyCode::B,
            ShortcutAction::Custom("b".into()),
            ShortcutContext::Global,
        ))
        .unwrap();
        assert_eq!(reg.all_entries().len(), 2);
    }

    #[test]
    fn source_variants() {
        let mut reg = ShortcutRegistry::new();
        reg.register(ShortcutEntry {
            binding: KeyBinding::new(MOD_CTRL, KeyCode::A),
            action: ShortcutAction::Custom("a".into()),
            context: ShortcutContext::Global,
            source: ShortcutSource::User,
            enabled: true,
        })
        .unwrap();
        reg.register(ShortcutEntry {
            binding: KeyBinding::new(MOD_CTRL, KeyCode::B),
            action: ShortcutAction::Custom("b".into()),
            context: ShortcutContext::Global,
            source: ShortcutSource::Extension("my-ext".into()),
            enabled: true,
        })
        .unwrap();

        assert_eq!(reg.all_entries()[0].source, ShortcutSource::User);
        assert_eq!(
            reg.all_entries()[1].source,
            ShortcutSource::Extension("my-ext".into())
        );
    }

    #[test]
    fn custom_context() {
        let mut reg = ShortcutRegistry::new();
        reg.register(entry(
            MOD_CTRL,
            KeyCode::P,
            ShortcutAction::Custom("palette".into()),
            ShortcutContext::Custom("editor".into()),
        ))
        .unwrap();

        let result = reg.lookup(
            MOD_CTRL,
            &KeyCode::P,
            &[ShortcutContext::Custom("editor".into())],
        );
        assert_eq!(result, Some(&ShortcutAction::Custom("palette".into())));

        // Different custom context should not match
        let result = reg.lookup(
            MOD_CTRL,
            &KeyCode::P,
            &[ShortcutContext::Custom("browser".into())],
        );
        assert!(result.is_none());
    }
}
