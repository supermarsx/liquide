//! CSS Counter Registry — implements `counter-reset`, `counter-increment`,
//! `counter-set`, `counter()` and `counters()` per CSS Lists Level 3.
//!
//! Each counter name maps to a stack of values.  `counter-reset` pushes a new
//! scope; the scope is popped when the element's subtree layout completes.
//! `counter-increment` and `counter-set` modify the top-of-stack value.

use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    /// Thread-local counter registry used during layout.
    /// Reset at the start of each full layout pass via `LayoutEngine::layout()`.
    pub static COUNTER_REGISTRY: RefCell<CounterRegistry> = RefCell::new(CounterRegistry::new());
}

/// CSS counter registry holding stacked counter values.
#[derive(Debug, Clone, Default)]
pub struct CounterRegistry {
    /// Counter name → stack of (value, depth).  The depth tracks nesting so we
    /// know which scopes to pop when leaving an element.
    counters: HashMap<String, Vec<i32>>,
    /// Stack of counter names that were reset at each depth level.
    /// Used to pop scopes when exiting an element.
    scope_stack: Vec<Vec<String>>,
}

impl CounterRegistry {
    /// Create a new empty counter registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Process a `counter-reset` declaration string.
    /// Format: `"name1 value1 name2 value2 ..."` — value defaults to 0.
    pub fn apply_reset(&mut self, decl: &str) {
        let mut names_pushed = Vec::new();
        let tokens: Vec<&str> = decl.split_whitespace().collect();
        let mut i = 0;
        while i < tokens.len() {
            let name = tokens[i];
            if name == "none" {
                i += 1;
                continue;
            }
            let value = if i + 1 < tokens.len() {
                if let Ok(v) = tokens[i + 1].parse::<i32>() {
                    i += 2;
                    v
                } else {
                    i += 1;
                    0
                }
            } else {
                i += 1;
                0
            };
            self.counters
                .entry(name.to_string())
                .or_default()
                .push(value);
            names_pushed.push(name.to_string());
        }
        self.scope_stack.push(names_pushed);
    }

    /// Process a `counter-increment` declaration string.
    /// Format: `"name1 value1 name2 value2 ..."` — value defaults to 1.
    pub fn apply_increment(&mut self, decl: &str) {
        let tokens: Vec<&str> = decl.split_whitespace().collect();
        let mut i = 0;
        while i < tokens.len() {
            let name = tokens[i];
            if name == "none" {
                i += 1;
                continue;
            }
            let value = if i + 1 < tokens.len() {
                if let Ok(v) = tokens[i + 1].parse::<i32>() {
                    i += 2;
                    v
                } else {
                    i += 1;
                    1
                }
            } else {
                i += 1;
                1
            };
            // If the counter doesn't exist yet, implicitly create it at 0
            let stack = self.counters.entry(name.to_string()).or_default();
            if stack.is_empty() {
                stack.push(0);
            }
            if let Some(top) = stack.last_mut() {
                *top += value;
            }
        }
    }

    /// Process a `counter-set` declaration string.
    /// Format: `"name1 value1 name2 value2 ..."` — value defaults to 0.
    pub fn apply_set(&mut self, decl: &str) {
        let tokens: Vec<&str> = decl.split_whitespace().collect();
        let mut i = 0;
        while i < tokens.len() {
            let name = tokens[i];
            if name == "none" {
                i += 1;
                continue;
            }
            let value = if i + 1 < tokens.len() {
                if let Ok(v) = tokens[i + 1].parse::<i32>() {
                    i += 2;
                    v
                } else {
                    i += 1;
                    0
                }
            } else {
                i += 1;
                0
            };
            let stack = self.counters.entry(name.to_string()).or_default();
            if stack.is_empty() {
                stack.push(value);
            } else if let Some(top) = stack.last_mut() {
                *top = value;
            }
        }
    }

    /// Pop the most recent scope pushed by `apply_reset`.
    pub fn pop_scope(&mut self) {
        if let Some(names) = self.scope_stack.pop() {
            for name in &names {
                if let Some(stack) = self.counters.get_mut(name) {
                    stack.pop();
                    if stack.is_empty() {
                        self.counters.remove(name);
                    }
                }
            }
        }
    }

    /// Push an empty scope (for elements that don't reset any counters but
    /// need a matching `pop_scope` call).
    pub fn push_empty_scope(&mut self) {
        self.scope_stack.push(Vec::new());
    }

    /// Get the current value of a counter (top of its stack), or 0 if undefined.
    pub fn counter_value(&self, name: &str) -> i32 {
        self.counters
            .get(name)
            .and_then(|stack| stack.last().copied())
            .unwrap_or(0)
    }

    /// Get a string joining all nested counter values with a separator.
    /// E.g., for `counters(section, ".")` with stack [1, 3, 2] → "1.3.2".
    pub fn counters_value(&self, name: &str, separator: &str) -> String {
        match self.counters.get(name) {
            Some(stack) if !stack.is_empty() => stack
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(separator),
            _ => "0".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_and_increment() {
        let mut reg = CounterRegistry::new();
        reg.apply_reset("section 0");
        assert_eq!(reg.counter_value("section"), 0);
        reg.apply_increment("section");
        assert_eq!(reg.counter_value("section"), 1);
        reg.apply_increment("section 5");
        assert_eq!(reg.counter_value("section"), 6);
    }

    #[test]
    fn nested_scopes() {
        let mut reg = CounterRegistry::new();
        reg.apply_reset("item 0");
        reg.apply_increment("item");
        assert_eq!(reg.counter_value("item"), 1);

        // Nested reset
        reg.apply_reset("item 0");
        reg.apply_increment("item");
        assert_eq!(reg.counter_value("item"), 1);
        assert_eq!(reg.counters_value("item", "."), "1.1");

        reg.pop_scope();
        assert_eq!(reg.counter_value("item"), 1);
        assert_eq!(reg.counters_value("item", "."), "1");
    }

    #[test]
    fn counter_set() {
        let mut reg = CounterRegistry::new();
        reg.apply_reset("x 0");
        reg.apply_set("x 10");
        assert_eq!(reg.counter_value("x"), 10);
    }

    #[test]
    fn implicit_counter_on_increment() {
        let mut reg = CounterRegistry::new();
        reg.apply_increment("foo");
        assert_eq!(reg.counter_value("foo"), 1);
    }

    #[test]
    fn counters_value_format() {
        let mut reg = CounterRegistry::new();
        reg.apply_reset("c 1");
        reg.apply_reset("c 2");
        reg.apply_reset("c 3");
        assert_eq!(reg.counters_value("c", "."), "1.2.3");
        assert_eq!(reg.counters_value("c", " > "), "1 > 2 > 3");
    }

    #[test]
    fn undefined_counter() {
        let reg = CounterRegistry::new();
        assert_eq!(reg.counter_value("nope"), 0);
        assert_eq!(reg.counters_value("nope", "."), "0");
    }

    #[test]
    fn multiple_counters_in_one_decl() {
        let mut reg = CounterRegistry::new();
        reg.apply_reset("a 1 b 2");
        assert_eq!(reg.counter_value("a"), 1);
        assert_eq!(reg.counter_value("b"), 2);
        reg.apply_increment("a 10 b 20");
        assert_eq!(reg.counter_value("a"), 11);
        assert_eq!(reg.counter_value("b"), 22);
    }

    #[test]
    fn pop_scope_empty() {
        let mut reg = CounterRegistry::new();
        reg.push_empty_scope();
        reg.pop_scope(); // should not panic
    }
}
