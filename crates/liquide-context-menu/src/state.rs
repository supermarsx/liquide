//! Interactive state tracking for context menus.
//!
//! [`MenuState`] manages hover highlight, open submenus, submenu hover delay,
//! keyboard navigation, and type-ahead search. It operates purely on indices
//! and item metadata — no rendering or DOM coupling.

use crate::layout::MenuGeometry;
use crate::{MenuItem, MenuItemKind};

// ---------------------------------------------------------------------------
// Key and response enums
// ---------------------------------------------------------------------------

/// Keyboard input events that the menu system handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKey {
    /// Move highlight up.
    Up,
    /// Move highlight down.
    Down,
    /// Close submenu / go to parent.
    Left,
    /// Open submenu if hovered item has one.
    Right,
    /// Activate the highlighted item.
    Enter,
    /// Close menu (or submenu).
    Escape,
    /// Type-ahead: jump to first item starting with this character.
    Char(char),
}

/// Result of a user interaction with the menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuResponse {
    /// No action — the event was consumed but nothing changed.
    None,
    /// An item was hovered (the `u32` is the item's `id`).
    Hover(u32),
    /// An item was activated — the shell should dispatch this action ID.
    Activate(u32),
    /// A submenu should be opened for the given item ID.
    OpenSubmenu(u32),
    /// The currently open submenu should be closed.
    CloseSubmenu,
    /// The entire menu tree should be closed.
    Close,
}

// ---------------------------------------------------------------------------
// MenuState
// ---------------------------------------------------------------------------

/// Tracks the interactive state of one level of a context menu.
///
/// For nested menus, each submenu level has its own `MenuState` instance
/// reachable through `open_submenu`.
pub struct MenuState {
    /// Currently highlighted item index (in the items array), or `None`.
    hovered_index: Option<usize>,
    /// Currently highlighted item ID, or `None`.
    hovered_id: Option<u32>,
    /// Which submenu is open: `(parent_item_id, child state)`.
    open_submenu: Option<(u32, Box<MenuState>)>,
    /// Delay in milliseconds before a submenu opens on hover.
    pub submenu_delay_ms: u32,
    /// Timestamp (in microseconds) when the hover entered a submenu-parent item.
    submenu_hover_start_us: Option<u64>,
    /// Item ID that is pending submenu open (waiting for delay).
    submenu_pending_id: Option<u32>,
}

impl MenuState {
    /// Create a new state with no selection and default submenu delay.
    #[must_use]
    pub fn new() -> Self {
        Self {
            hovered_index: None,
            hovered_id: None,
            open_submenu: None,
            submenu_delay_ms: 200,
            submenu_hover_start_us: None,
            submenu_pending_id: None,
        }
    }

    /// The currently hovered item index.
    #[must_use]
    pub fn hovered_index(&self) -> Option<usize> {
        self.hovered_index
    }

    /// The currently hovered item ID.
    #[must_use]
    pub fn hovered_id(&self) -> Option<u32> {
        self.hovered_id
    }

    /// Reference to the open submenu state, if any.
    #[must_use]
    pub fn open_submenu(&self) -> Option<(u32, &MenuState)> {
        self.open_submenu
            .as_ref()
            .map(|(id, s)| (*id, s.as_ref()))
    }

    /// Mutable reference to the open submenu state, if any.
    pub fn open_submenu_mut(&mut self) -> Option<(u32, &mut MenuState)> {
        self.open_submenu
            .as_mut()
            .map(|(id, s)| (*id, s.as_mut()))
    }

    /// Close any open submenu.
    pub fn close_submenu(&mut self) {
        self.open_submenu = None;
        self.submenu_pending_id = None;
        self.submenu_hover_start_us = None;
    }

    /// Reset all state (hover, submenu, pending).
    pub fn reset(&mut self) {
        self.hovered_index = None;
        self.hovered_id = None;
        self.open_submenu = None;
        self.submenu_hover_start_us = None;
        self.submenu_pending_id = None;
    }

    // -----------------------------------------------------------------
    // Mouse interaction
    // -----------------------------------------------------------------

    /// Handle a mouse move at screen coordinates `(x, y)`.
    ///
    /// Uses the computed `MenuGeometry` to determine which item is under
    /// the cursor and manages submenu open/close timing.
    ///
    /// `now_us` is the current timestamp in microseconds (for submenu delay).
    pub fn on_mouse_move(
        &mut self,
        x: f32,
        y: f32,
        items: &[MenuItem],
        geo: &MenuGeometry,
        now_us: u64,
    ) -> MenuResponse {
        let hit = geo.hit_test(x, y);

        // Delegate to open submenu first (if mouse is inside it).
        // The caller should check submenu geometry separately and call
        // the child state's on_mouse_move. Here we just handle the
        // parent-level hover.

        let prev_id = self.hovered_id;
        match hit {
            Some(idx) if idx < items.len() => {
                let item = &items[idx];
                self.hovered_index = Some(idx);
                self.hovered_id = Some(item.id);

                // Submenu delay logic.
                if item.has_submenu() {
                    if self.submenu_pending_id != Some(item.id) {
                        // Start timing for a new submenu parent.
                        self.submenu_pending_id = Some(item.id);
                        self.submenu_hover_start_us = Some(now_us);
                    } else if let Some(start) = self.submenu_hover_start_us {
                        let elapsed_ms = (now_us.saturating_sub(start)) / 1000;
                        if elapsed_ms >= self.submenu_delay_ms as u64 {
                            // Open the submenu.
                            self.open_submenu =
                                Some((item.id, Box::new(MenuState::new())));
                            self.submenu_pending_id = None;
                            self.submenu_hover_start_us = None;
                            return MenuResponse::OpenSubmenu(item.id);
                        }
                    }
                } else {
                    // Moved to a non-submenu item — close any open submenu.
                    if self.open_submenu.is_some() {
                        self.close_submenu();
                    }
                    self.submenu_pending_id = None;
                    self.submenu_hover_start_us = None;
                }

                if Some(item.id) != prev_id {
                    MenuResponse::Hover(item.id)
                } else {
                    MenuResponse::None
                }
            }
            _ => {
                // Outside all items.
                if self.hovered_id.is_some() {
                    self.hovered_index = None;
                    self.hovered_id = None;
                    self.submenu_pending_id = None;
                    self.submenu_hover_start_us = None;
                    // Don't close submenu just because we left the parent area —
                    // the mouse might be moving towards the submenu.
                }
                MenuResponse::None
            }
        }
    }

    /// Handle a mouse click at screen coordinates `(x, y)`.
    pub fn on_click(
        &mut self,
        x: f32,
        y: f32,
        items: &[MenuItem],
        geo: &MenuGeometry,
    ) -> MenuResponse {
        let hit = geo.hit_test(x, y);
        match hit {
            Some(idx) if idx < items.len() => {
                let item = &items[idx];
                if item.separator || item.disabled {
                    return MenuResponse::None;
                }
                if item.has_submenu() {
                    // Clicking a submenu parent opens it immediately.
                    self.open_submenu = Some((item.id, Box::new(MenuState::new())));
                    return MenuResponse::OpenSubmenu(item.id);
                }
                MenuResponse::Activate(item.id)
            }
            _ => {
                // Click outside the menu — close.
                MenuResponse::Close
            }
        }
    }

    // -----------------------------------------------------------------
    // Keyboard navigation
    // -----------------------------------------------------------------

    /// Handle a keyboard input.
    pub fn on_key(&mut self, key: MenuKey, items: &[MenuItem]) -> MenuResponse {
        // If a submenu is open and the key should be forwarded, do so.
        if let Some((parent_id, ref mut sub_state)) = self.open_submenu {
            match key {
                MenuKey::Left => {
                    // Close submenu, return to parent.
                    self.open_submenu = None;
                    return MenuResponse::CloseSubmenu;
                }
                MenuKey::Escape => {
                    self.open_submenu = None;
                    return MenuResponse::CloseSubmenu;
                }
                _ => {
                    // Forward to submenu if it has items.
                    let sub_items = Self::find_submenu_items(items, parent_id);
                    if !sub_items.is_empty() {
                        return sub_state.on_key(key, sub_items);
                    }
                }
            }
        }

        match key {
            MenuKey::Up => self.move_highlight(-1, items),
            MenuKey::Down => self.move_highlight(1, items),
            MenuKey::Right => {
                // Open submenu if current item has one.
                if let Some(idx) = self.hovered_index {
                    if let Some(item) = items.get(idx) {
                        if item.has_submenu() {
                            self.open_submenu =
                                Some((item.id, Box::new(MenuState::new())));
                            return MenuResponse::OpenSubmenu(item.id);
                        }
                    }
                }
                MenuResponse::None
            }
            MenuKey::Enter => {
                if let Some(idx) = self.hovered_index {
                    if let Some(item) = items.get(idx) {
                        if item.separator || item.disabled {
                            return MenuResponse::None;
                        }
                        if item.has_submenu() {
                            self.open_submenu =
                                Some((item.id, Box::new(MenuState::new())));
                            return MenuResponse::OpenSubmenu(item.id);
                        }
                        return MenuResponse::Activate(item.id);
                    }
                }
                MenuResponse::None
            }
            MenuKey::Escape => MenuResponse::Close,
            MenuKey::Char(ch) => self.type_ahead(ch, items),
            _ => MenuResponse::None,
        }
    }

    /// Move the highlight by `delta` positions (+1 = down, -1 = up),
    /// skipping separators and disabled items.
    fn move_highlight(&mut self, delta: i32, items: &[MenuItem]) -> MenuResponse {
        if items.is_empty() {
            return MenuResponse::None;
        }

        let activatable_indices: Vec<usize> = items
            .iter()
            .enumerate()
            .filter(|(_, it)| it.is_activatable())
            .map(|(i, _)| i)
            .collect();

        if activatable_indices.is_empty() {
            return MenuResponse::None;
        }

        let current_pos = self
            .hovered_index
            .and_then(|idx| activatable_indices.iter().position(|&ai| ai == idx));

        let new_pos = match current_pos {
            Some(pos) => {
                let len = activatable_indices.len() as i32;
                ((pos as i32 + delta).rem_euclid(len)) as usize
            }
            None => {
                if delta > 0 {
                    0
                } else {
                    activatable_indices.len() - 1
                }
            }
        };

        let new_idx = activatable_indices[new_pos];
        self.hovered_index = Some(new_idx);
        self.hovered_id = Some(items[new_idx].id);
        MenuResponse::Hover(items[new_idx].id)
    }

    /// Type-ahead: jump to the first item whose label starts with `ch`.
    fn type_ahead(&mut self, ch: char, items: &[MenuItem]) -> MenuResponse {
        let ch_lower = ch.to_lowercase().next().unwrap_or(ch);
        let start = self.hovered_index.map_or(0, |i| i + 1);

        // Search from current position to end, then wrap around.
        let indices = (start..items.len()).chain(0..start);

        for idx in indices {
            let item = &items[idx];
            if item.separator || item.disabled {
                continue;
            }
            if let Some(first_char) = item.label.chars().next() {
                if first_char.to_lowercase().next() == Some(ch_lower) {
                    self.hovered_index = Some(idx);
                    self.hovered_id = Some(item.id);
                    return MenuResponse::Hover(item.id);
                }
            }
        }
        MenuResponse::None
    }

    /// Find the submenu children items for a given parent item ID.
    fn find_submenu_items<'a>(items: &'a [MenuItem], parent_id: u32) -> &'a [MenuItem] {
        for item in items {
            if item.id == parent_id {
                if let MenuItemKind::Submenu(ref children) = item.kind {
                    return children;
                }
            }
        }
        &[]
    }
}

impl Default for MenuState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{MenuLayout, MenuGeometry};
    use crate::theme::MenuTheme;
    use crate::{MenuAction, MenuItem};
    use liquide_compositor::geometry::Rect;

    fn screen() -> Rect {
        Rect::new(0.0, 0.0, 1920.0, 1080.0)
    }

    fn theme() -> MenuTheme {
        MenuTheme::default_theme()
    }

    fn basic_items() -> Vec<MenuItem> {
        vec![
            MenuItem::action("Cut", MenuAction(1)).with_shortcut("Ctrl+X"),
            MenuItem::action("Copy", MenuAction(2)).with_shortcut("Ctrl+C"),
            MenuItem::separator(),
            MenuItem::action("Paste", MenuAction(3)).with_shortcut("Ctrl+V"),
            MenuItem::action("Delete", MenuAction(4))
                .with_danger(true)
                .with_shortcut("Del"),
        ]
    }

    fn basic_geo(items: &[MenuItem]) -> MenuGeometry {
        MenuLayout::compute(items, (200.0, 200.0), screen(), &theme(), 1.0)
    }

    #[test]
    fn key_down_selects_first() {
        let items = basic_items();
        let mut state = MenuState::new();
        let resp = state.on_key(MenuKey::Down, &items);
        assert_eq!(state.hovered_index(), Some(0));
        assert!(matches!(resp, MenuResponse::Hover(_)));
    }

    #[test]
    fn key_up_selects_last() {
        let items = basic_items();
        let mut state = MenuState::new();
        let resp = state.on_key(MenuKey::Up, &items);
        // Should select last activatable item (index 4).
        assert_eq!(state.hovered_index(), Some(4));
        assert!(matches!(resp, MenuResponse::Hover(_)));
    }

    #[test]
    fn key_down_skips_separator() {
        let items = basic_items();
        let mut state = MenuState::new();
        // Down twice: 0 -> 1
        state.on_key(MenuKey::Down, &items);
        state.on_key(MenuKey::Down, &items);
        assert_eq!(state.hovered_index(), Some(1));
        // Down again: should skip separator (index 2) and go to 3.
        state.on_key(MenuKey::Down, &items);
        assert_eq!(state.hovered_index(), Some(3));
    }

    #[test]
    fn key_down_wraps_around() {
        let items = basic_items();
        let mut state = MenuState::new();
        // Navigate to last item then wrap.
        for _ in 0..4 {
            state.on_key(MenuKey::Down, &items);
        }
        assert_eq!(state.hovered_index(), Some(4));
        let resp = state.on_key(MenuKey::Down, &items);
        assert_eq!(state.hovered_index(), Some(0));
        assert!(matches!(resp, MenuResponse::Hover(_)));
    }

    #[test]
    fn key_enter_activates() {
        let items = basic_items();
        let mut state = MenuState::new();
        state.on_key(MenuKey::Down, &items); // select first
        let resp = state.on_key(MenuKey::Enter, &items);
        assert!(matches!(resp, MenuResponse::Activate(_)));
    }

    #[test]
    fn key_enter_on_separator_is_noop() {
        let items = vec![
            MenuItem::separator(),
            MenuItem::action("A", MenuAction(1)),
        ];
        let mut state = MenuState::new();
        // Force hover on separator.
        state.hovered_index = Some(0);
        state.hovered_id = Some(items[0].id);
        let resp = state.on_key(MenuKey::Enter, &items);
        assert_eq!(resp, MenuResponse::None);
    }

    #[test]
    fn key_escape_closes() {
        let items = basic_items();
        let mut state = MenuState::new();
        let resp = state.on_key(MenuKey::Escape, &items);
        assert_eq!(resp, MenuResponse::Close);
    }

    #[test]
    fn key_right_opens_submenu() {
        let items = vec![
            MenuItem::action("A", MenuAction(1)),
            MenuItem::submenu(
                "More",
                vec![MenuItem::action("Sub", MenuAction(10))],
            ),
        ];
        let mut state = MenuState::new();
        state.on_key(MenuKey::Down, &items);
        state.on_key(MenuKey::Down, &items);
        assert_eq!(state.hovered_index(), Some(1));
        let resp = state.on_key(MenuKey::Right, &items);
        assert!(matches!(resp, MenuResponse::OpenSubmenu(_)));
        assert!(state.open_submenu().is_some());
    }

    #[test]
    fn key_left_closes_submenu() {
        let items = vec![
            MenuItem::submenu(
                "Sub",
                vec![MenuItem::action("Child", MenuAction(10))],
            ),
        ];
        let mut state = MenuState::new();
        state.on_key(MenuKey::Down, &items);
        state.on_key(MenuKey::Right, &items); // open submenu
        assert!(state.open_submenu().is_some());
        let resp = state.on_key(MenuKey::Left, &items);
        assert_eq!(resp, MenuResponse::CloseSubmenu);
        assert!(state.open_submenu().is_none());
    }

    #[test]
    fn type_ahead_jumps_to_matching_item() {
        let items = vec![
            MenuItem::action("Apple", MenuAction(1)),
            MenuItem::action("Banana", MenuAction(2)),
            MenuItem::action("Cherry", MenuAction(3)),
            MenuItem::action("Blueberry", MenuAction(4)),
        ];
        let mut state = MenuState::new();
        let resp = state.on_key(MenuKey::Char('b'), &items);
        assert_eq!(state.hovered_index(), Some(1)); // Banana
        assert!(matches!(resp, MenuResponse::Hover(_)));
    }

    #[test]
    fn type_ahead_wraps_around() {
        let items = vec![
            MenuItem::action("Apple", MenuAction(1)),
            MenuItem::action("Avocado", MenuAction(2)),
            MenuItem::action("Apricot", MenuAction(3)),
        ];
        let mut state = MenuState::new();
        state.on_key(MenuKey::Char('a'), &items); // Apple (0)
        state.on_key(MenuKey::Char('a'), &items); // Avocado (1)
        state.on_key(MenuKey::Char('a'), &items); // Apricot (2)
        assert_eq!(state.hovered_index(), Some(2));
        state.on_key(MenuKey::Char('a'), &items); // wrap to Apple (0)
        assert_eq!(state.hovered_index(), Some(0));
    }

    #[test]
    fn type_ahead_case_insensitive() {
        let items = vec![
            MenuItem::action("delete", MenuAction(1)),
            MenuItem::action("Edit", MenuAction(2)),
        ];
        let mut state = MenuState::new();
        state.on_key(MenuKey::Char('E'), &items);
        assert_eq!(state.hovered_index(), Some(1));
    }

    #[test]
    fn click_activates_item() {
        let items = basic_items();
        let geo = basic_geo(&items);
        let mut state = MenuState::new();
        // Click on first item.
        let first = &geo.items[0];
        let resp = state.on_click(
            geo.x + first.rect.x + 5.0,
            geo.y + first.rect.y + 5.0,
            &items,
            &geo,
        );
        assert!(matches!(resp, MenuResponse::Activate(_)));
    }

    #[test]
    fn click_outside_closes() {
        let items = basic_items();
        let geo = basic_geo(&items);
        let mut state = MenuState::new();
        let resp = state.on_click(0.0, 0.0, &items, &geo);
        assert_eq!(resp, MenuResponse::Close);
    }

    #[test]
    fn click_disabled_is_noop() {
        let items = vec![
            MenuItem::action("Disabled", MenuAction(1)).with_disabled(true),
        ];
        let geo = basic_geo(&items);
        let mut state = MenuState::new();
        let first = &geo.items[0];
        let resp = state.on_click(
            geo.x + first.rect.x + 5.0,
            geo.y + first.rect.y + 5.0,
            &items,
            &geo,
        );
        assert_eq!(resp, MenuResponse::None);
    }

    #[test]
    fn click_separator_is_noop() {
        let items = vec![
            MenuItem::separator(),
            MenuItem::action("A", MenuAction(1)),
        ];
        let geo = basic_geo(&items);
        let mut state = MenuState::new();
        let sep = &geo.items[0];
        let resp = state.on_click(
            geo.x + sep.rect.x + 5.0,
            geo.y + sep.rect.y + 2.0,
            &items,
            &geo,
        );
        assert_eq!(resp, MenuResponse::None);
    }

    #[test]
    fn mouse_move_updates_hover() {
        let items = basic_items();
        let geo = basic_geo(&items);
        let mut state = MenuState::new();
        let first = &geo.items[0];
        let resp = state.on_mouse_move(
            geo.x + first.rect.x + 5.0,
            geo.y + first.rect.y + 5.0,
            &items,
            &geo,
            0,
        );
        assert!(matches!(resp, MenuResponse::Hover(_)));
        assert_eq!(state.hovered_index(), Some(0));
    }

    #[test]
    fn submenu_delay_opens_after_threshold() {
        let items = vec![
            MenuItem::submenu(
                "Sub",
                vec![MenuItem::action("Child", MenuAction(10))],
            ),
        ];
        let geo = basic_geo(&items);
        let mut state = MenuState::new();
        state.submenu_delay_ms = 100;

        let row = &geo.items[0];
        let mx = geo.x + row.rect.x + 5.0;
        let my = geo.y + row.rect.y + 5.0;

        // First move: starts timer.
        let r1 = state.on_mouse_move(mx, my, &items, &geo, 0);
        assert!(matches!(r1, MenuResponse::Hover(_)));
        assert!(state.open_submenu().is_none());

        // Not enough time.
        let r2 = state.on_mouse_move(mx, my, &items, &geo, 50_000);
        assert_eq!(r2, MenuResponse::None);
        assert!(state.open_submenu().is_none());

        // Enough time.
        let r3 = state.on_mouse_move(mx, my, &items, &geo, 200_000);
        assert!(matches!(r3, MenuResponse::OpenSubmenu(_)));
        assert!(state.open_submenu().is_some());
    }

    #[test]
    fn reset_clears_everything() {
        let items = basic_items();
        let mut state = MenuState::new();
        state.on_key(MenuKey::Down, &items);
        assert!(state.hovered_index().is_some());
        state.reset();
        assert!(state.hovered_index().is_none());
        assert!(state.hovered_id().is_none());
        assert!(state.open_submenu().is_none());
    }

    #[test]
    fn move_highlight_with_all_disabled() {
        let items = vec![
            MenuItem::separator(),
            MenuItem::action("A", MenuAction(1)).with_disabled(true),
        ];
        let mut state = MenuState::new();
        // No activatable items — should be no-op.
        let resp = state.on_key(MenuKey::Down, &items);
        assert_eq!(resp, MenuResponse::None);
        assert_eq!(state.hovered_index(), None);
    }

    #[test]
    fn click_submenu_opens_immediately() {
        let items = vec![
            MenuItem::submenu(
                "More",
                vec![MenuItem::action("Child", MenuAction(10))],
            ),
        ];
        let geo = basic_geo(&items);
        let mut state = MenuState::new();
        let row = &geo.items[0];
        let resp = state.on_click(
            geo.x + row.rect.x + 5.0,
            geo.y + row.rect.y + 5.0,
            &items,
            &geo,
        );
        assert!(matches!(resp, MenuResponse::OpenSubmenu(_)));
        assert!(state.open_submenu().is_some());
    }
}
