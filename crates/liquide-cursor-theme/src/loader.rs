//! XDG cursor theme loader with `Inherits=` chain traversal.
//!
//! Resolving a shape to an image in the XDG cursor spec works like this:
//!
//! 1. Look in the active theme's `cursors/` subdirectory.
//! 2. If not found, follow the theme's `Inherits=` list in order.
//! 3. Fall back to the `hicolor` (spec) / `default` pseudo-themes.
//!
//! A naïve loader that ignores `Inherits=` will silently fail for the
//! overwhelming majority of themes because themes ship only a handful of
//! shape overrides and inherit the rest (typically from `Adwaita`, `DMZ`,
//! or `hicolor`).
//!
//! This module implements the chain walk without pulling in any extra
//! XDG-specific deps — it uses existing workspace primitives.

use crate::cursor::CursorShape;
use crate::theme::{CursorTheme, CursorThemeManager};
use std::collections::HashSet;

/// Resolve a cursor shape to a theme name by walking the `Inherits=` chain
/// starting at `start_theme`. Returns the name of the first theme in the
/// chain that actually has an entry for `shape`, or `None` if the chain
/// terminates without a hit.
///
/// This function does not consult the filesystem — it only walks the
/// themes already registered in `mgr` (typically populated via
/// `mgr.discover_themes()`).
pub fn resolve_through_inherits(
    mgr: &CursorThemeManager,
    start_theme: &str,
    shape: CursorShape,
) -> Option<String> {
    walk_chain(mgr, start_theme, |t| t.has_cursor(shape))
}

/// Walk the inheritance chain rooted at `start_theme`, returning the name
/// of the first theme matching `predicate`. Loops are broken by tracking
/// visited theme names.
pub fn walk_chain<F>(
    mgr: &CursorThemeManager,
    start_theme: &str,
    mut predicate: F,
) -> Option<String>
where
    F: FnMut(&CursorTheme) -> bool,
{
    let mut stack: Vec<String> = vec![start_theme.to_string()];
    let mut visited: HashSet<String> = HashSet::new();

    while let Some(name) = stack.pop() {
        if !visited.insert(name.clone()) {
            continue;
        }
        let theme = match mgr.themes.get(&name) {
            Some(t) => t,
            None => continue,
        };
        if predicate(theme) {
            return Some(name);
        }
        // Parse comma-separated list; queue in reverse so first parent is
        // visited first (stack is LIFO).
        if let Some(inh) = theme.inherits.as_deref() {
            let mut parents: Vec<&str> = inh
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
            parents.reverse();
            for p in parents {
                if !visited.contains(p) {
                    stack.push(p.to_string());
                }
            }
        }
    }

    // Spec-mandated final fallback: hicolor.
    if !visited.contains("hicolor")
        && mgr
            .themes
            .get("hicolor")
            .map(|t| predicate(t))
            .unwrap_or(false)
    {
        return Some("hicolor".to_string());
    }
    None
}

/// Parse a raw `index.theme` / `cursor.theme` content and return the
/// inheritance chain (trimmed, non-empty names, in file order).
///
/// Useful for callers that want to know the chain without having the
/// `CursorTheme` populated.
pub fn parse_inherits(content: &str) -> Vec<String> {
    for line in content.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Inherits") {
            let rest = rest.trim_start();
            if let Some(eq) = rest.strip_prefix('=') {
                return eq
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect();
            }
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cursor::CursorImage;

    fn mk_theme(name: &str, inherits: Option<&str>, shapes: &[CursorShape]) -> CursorTheme {
        let mut t = CursorTheme::new(name);
        t.inherits = inherits.map(str::to_string);
        for &s in shapes {
            t.add_cursor(s, CursorImage::solid_square(24, 255, 255, 255));
        }
        t
    }

    #[test]
    fn parse_inherits_single() {
        let v = parse_inherits("[Icon Theme]\nName=Foo\nInherits=Adwaita\n");
        assert_eq!(v, vec!["Adwaita".to_string()]);
    }

    #[test]
    fn parse_inherits_comma_list() {
        let v = parse_inherits("Inherits=core, DMZ-White, hicolor");
        assert_eq!(v, vec!["core", "DMZ-White", "hicolor"]);
    }

    #[test]
    fn parse_inherits_absent() {
        assert!(parse_inherits("Name=Foo\n").is_empty());
    }

    #[test]
    fn walk_stops_at_first_hit() {
        let mut mgr = CursorThemeManager::new();
        mgr.themes
            .insert("child".into(), mk_theme("child", Some("parent"), &[]));
        mgr.themes.insert(
            "parent".into(),
            mk_theme("parent", None, &[CursorShape::Pointer]),
        );
        let hit = resolve_through_inherits(&mgr, "child", CursorShape::Pointer);
        assert_eq!(hit.as_deref(), Some("parent"));
    }

    #[test]
    fn walk_handles_loop() {
        let mut mgr = CursorThemeManager::new();
        mgr.themes.insert("a".into(), mk_theme("a", Some("b"), &[]));
        mgr.themes.insert("b".into(), mk_theme("b", Some("a"), &[]));
        // No shape → None, but importantly does not hang.
        assert!(resolve_through_inherits(&mgr, "a", CursorShape::Pointer).is_none());
    }

    #[test]
    fn walk_no_chain_direct_hit() {
        let mut mgr = CursorThemeManager::new();
        mgr.themes
            .insert("solo".into(), mk_theme("solo", None, &[CursorShape::Text]));
        assert_eq!(
            resolve_through_inherits(&mgr, "solo", CursorShape::Text).as_deref(),
            Some("solo"),
        );
    }
}
