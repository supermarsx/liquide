//! Mouse and keyboard event handling, click dispatch, DOM event forwarding.

use liquide_compositor::geometry::{Point, Rect};
use liquide_compositor::scene::CursorShape;
use liquide_input::mouse::{ButtonState, MouseButton, MouseEvent};
use liquide_platform::PlatformEvent;

use crate::decoration::{HitZone, hit_test_decoration};
use crate::launcher::SearchResultKind;
use crate::shortcuts::ShellAction;
use crate::window::{WindowFlags, WindowId, WindowState};
use liquide_hit_test::EventDispatcher;
use liquide_hit_test::event::{DomEventKind, MouseButton as DomMouseButton};
use liquide_statusbar::{StatusBarItemKind, StatusBarSlot};

use super::hooks::ShellHookEvent;
use super::{ContextMenuItem, DragState, Shell};

const SHELL_BAR_MENU_WIDTH: f32 = 180.0;
const SHELL_BAR_PADDING_X: f32 = 8.0;
const SHELL_BAR_ITEM_SPACING: f32 = 8.0;
const SHELL_BAR_BRANDING_TEXT: &str = "LiquiDE";
const SHELL_TRAY_ITEM_SIZE: f32 = 18.0;
const SHELL_TRAY_ITEM_GAP: f32 = 4.0;

impl Shell {
    /// Forward a mouse event to the DOM EventDispatcher.
    pub(crate) fn dispatch_dom_mouse_event(&mut self, me: &MouseEvent) {
        use liquide_layout::geometry::Point as LayoutPoint;

        if let MouseEvent::Scroll { x, y, axis, delta } = me {
            let pos = LayoutPoint::new(*x, *y);
            let (dx, dy) = match axis {
                liquide_input::mouse::ScrollAxis::Horizontal => (*delta, 0.0),
                liquide_input::mouse::ScrollAxis::Vertical => (0.0, *delta),
            };
            let scroll_target = {
                let hit_test = match self.hit_test_engine.as_ref() {
                    Some(ht) => ht,
                    None => return,
                };
                self.event_dispatcher.dispatch_scroll(pos, dx, dy, hit_test);
                hit_test.hit_test(pos).and_then(|hit| {
                    let layout = hit_test.layout();
                    layout.find_box_id_by_node(hit.node).and_then(|box_id| {
                        if layout
                            .get(box_id)
                            .map_or(false, |b| b.scroll_size.is_some())
                        {
                            Some(box_id)
                        } else {
                            layout.find_scroll_container(box_id)
                        }
                    })
                })
            };
            if let Some(container_id) = scroll_target {
                if let Some(ht_mut) = self.hit_test_engine.as_mut() {
                    ht_mut.layout_mut().set_scroll_offset(container_id, dx, dy);
                }
            }
            return;
        }

        let hit_test = match self.hit_test_engine.as_ref() {
            Some(ht) => ht,
            None => return,
        };

        match me {
            MouseEvent::Move { x, y } => {
                let pos = liquide_layout::geometry::Point::new(*x, *y);
                self.event_dispatcher
                    .dispatch_mouse_move(pos, &mut self.desktop_dom.doc, hit_test);
            }
            MouseEvent::Button {
                x,
                y,
                button,
                state,
            } => {
                let pos = liquide_layout::geometry::Point::new(*x, *y);
                let dom_btn = match button {
                    MouseButton::Left => DomMouseButton::Left,
                    MouseButton::Right => DomMouseButton::Right,
                    MouseButton::Middle => DomMouseButton::Middle,
                    _ => DomMouseButton::Left,
                };
                match state {
                    ButtonState::Pressed => {
                        self.event_dispatcher.dispatch_mouse_down(
                            pos,
                            dom_btn,
                            &mut self.desktop_dom.doc,
                            hit_test,
                        );
                    }
                    ButtonState::Released => {
                        self.event_dispatcher.dispatch_mouse_up(
                            pos,
                            dom_btn,
                            &mut self.desktop_dom.doc,
                            hit_test,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    /// Register an event handler on a DOM node (bubble phase).
    pub fn add_event_handler(
        &mut self,
        node: liquide_dom::NodeId,
        kind_filter: Option<DomEventKind>,
        handler: liquide_hit_test::dispatch::EventHandler,
    ) {
        self.event_dispatcher
            .add_handler(node, kind_filter, handler);
    }

    /// Register an event listener on a DOM node with an explicit capture flag
    /// (W3C `addEventListener` semantics) — `capture = true` fires during the
    /// capture phase (root → target), so a modal/overlay root can swallow a
    /// descendant event via `stopPropagation` (t64-p1 capability, wired here in
    /// t65-s2). These listeners are driven by [`Self::dispatch_dom_event_path`]
    /// (the three-phase `dispatch_events` path), NOT the legacy hover-chain
    /// `dispatch_dom_mouse_event` path.
    pub fn add_capturing_event_handler(
        &mut self,
        node: liquide_dom::NodeId,
        kind_filter: Option<DomEventKind>,
        capture: bool,
        handler: liquide_hit_test::dispatch::EventHandler,
    ) {
        self.event_dispatcher
            .add_event_listener(node, kind_filter, capture, handler);
    }

    /// Register a listener that can call `preventDefault` to suppress the shell's
    /// default action (e.g. a shortcut) for an event (t65-s2).
    ///
    /// The wrapped predicate runs inside the listener; returning `true` means
    /// "preventDefault" — it flips the shared [`Shell::dom_default_prevented`]
    /// flag that [`Self::dispatch_dom_event_path`] reads after dispatch. The
    /// listener always returns [`Propagation::Continue`] so it does not by itself
    /// stop propagation (preventDefault is independent of propagation per W3C).
    pub fn add_preventable_event_handler<F>(
        &mut self,
        node: liquide_dom::NodeId,
        kind_filter: Option<DomEventKind>,
        capture: bool,
        predicate: F,
    ) where
        F: Fn(&liquide_hit_test::event::DomEvent) -> bool + Send + 'static,
    {
        use std::sync::atomic::Ordering;
        let flag = std::sync::Arc::clone(&self.dom_default_prevented);
        self.event_dispatcher.add_event_listener(
            node,
            kind_filter,
            capture,
            Box::new(move |event| {
                if predicate(event) {
                    flag.store(true, Ordering::SeqCst);
                }
                liquide_hit_test::event::Propagation::Continue
            }),
        );
    }

    /// Build the root-first `event_path` for `target` and dispatch the given DOM
    /// events through the W3C three-phase [`EventDispatcher::dispatch_events`]
    /// path (capture → target → bubble).
    ///
    /// Per t64-p1, the path is `doc.ancestors(target).rev()` (the DOM
    /// `ancestors` returns leaf→root; the dispatcher wants root→parent). The
    /// shared [`Shell::dom_default_prevented`] flag is reset before dispatch and
    /// its post-dispatch value is returned so callers can gate default actions
    /// (e.g. shortcuts) on `preventDefault` (t65-s2 item 3).
    ///
    /// Returns `true` if a listener called `preventDefault`.
    pub fn dispatch_dom_event_path(
        &mut self,
        target: liquide_dom::NodeId,
        mut events: Vec<liquide_hit_test::event::DomEvent>,
    ) -> bool {
        use std::sync::atomic::Ordering;
        let mut path = self.desktop_dom.doc.ancestors(target);
        path.reverse();
        for ev in &mut events {
            ev.event_path = path.clone();
        }
        self.dom_default_prevented.store(false, Ordering::SeqCst);
        self.event_dispatcher.dispatch_events(&events);
        self.dom_default_prevented.load(Ordering::SeqCst)
    }

    /// Set the DOM-focused node on the event dispatcher (disjoint-borrow helper
    /// so callers don't have to split `self.event_dispatcher` and
    /// `self.desktop_dom.doc`). The dispatcher's focus drives which node
    /// [`Self::dispatch_dom_keyboard_event`] targets.
    pub fn set_dom_focus(&mut self, node: Option<liquide_dom::NodeId>) {
        self.event_dispatcher
            .set_focus(node, &mut self.desktop_dom.doc);
    }

    /// Dispatch a keyboard event into the DOM event pipeline (t65-s2 item 2).
    ///
    /// Keyboard input previously reached `input_state` + the shortcut table but
    /// NEVER the DOM `event_dispatcher`, so focused DOM/app KeyDown/KeyUp
    /// listeners were ignored and text input to a focused DOM field was broken.
    /// This routes the key into the DOM-focused node (the dispatcher's own focus)
    /// using full three-phase propagation, and returns whether a listener called
    /// `preventDefault` so the caller can suppress the matching shortcut.
    pub(crate) fn dispatch_dom_keyboard_event(
        &mut self,
        ke: &liquide_input::keyboard::KeyEvent,
    ) -> bool {
        use liquide_hit_test::event::{DomEvent, DomEventKind};
        use liquide_input::keyboard::KeyState;

        let Some(focused) = self.event_dispatcher.focus() else {
            return false;
        };
        let key = ke.key as u32;
        let modifiers = ke.modifiers.bits() as u32;
        let kind = match ke.state {
            KeyState::Released => DomEventKind::KeyUp { key, modifiers },
            // Pressed and Repeat both deliver a KeyDown to the DOM.
            _ => DomEventKind::KeyDown { key, modifiers },
        };
        let event = DomEvent::new(focused, kind);
        self.dispatch_dom_event_path(focused, vec![event])
    }

    /// Get a reference to the DOM event dispatcher.
    pub fn event_dispatcher(&self) -> &EventDispatcher {
        &self.event_dispatcher
    }

    /// Get a mutable reference to the DOM event dispatcher.
    pub fn event_dispatcher_mut(&mut self) -> &mut EventDispatcher {
        &mut self.event_dispatcher
    }

    fn shell_bar_branding_bounds(&self) -> Option<Rect> {
        if !self.status_bar.config().show_app_menu {
            return None;
        }

        let bar_bounds = self.status_bar.compute_bounds(self.screen_rect);
        let width = SHELL_BAR_BRANDING_TEXT.len() as f32 * 8.0 + 20.0;
        Some(Rect::new(
            bar_bounds.x + SHELL_BAR_PADDING_X,
            bar_bounds.y,
            width,
            bar_bounds.height,
        ))
    }

    fn shell_bar_tray_count(&self) -> usize {
        self.notifications.visible_tray_icons().len() + self.seamless.tray_icon_count()
    }

    fn shell_bar_item_width(&self, item: &liquide_statusbar::StatusBarItem) -> f32 {
        match &item.kind {
            StatusBarItemKind::Clock { format } => {
                let text = self
                    .status_bar
                    .format_clock_timestamp(item.last_update_us, format);
                text.len() as f32 * 7.5 + 12.0
            }
            StatusBarItemKind::NotificationIndicator { unread_count, .. } => {
                let badge_width = if *unread_count > 0 {
                    unread_count.to_string().len() as f32 * 6.0
                } else {
                    0.0
                };
                20.0 + badge_width + 8.0
            }
            StatusBarItemKind::ConnectionQuality {
                quality_percent, ..
            } => format!("{quality_percent}%").len() as f32 * 7.0 + 10.0,
            StatusBarItemKind::TrayArea => {
                let count = self.shell_bar_tray_count();
                if count == 0 {
                    20.0
                } else {
                    count as f32 * SHELL_TRAY_ITEM_SIZE
                        + count.saturating_sub(1) as f32 * SHELL_TRAY_ITEM_GAP
                        + 8.0
                }
            }
            StatusBarItemKind::SessionButton => 40.0,
            StatusBarItemKind::Custom { content, .. } => content.len() as f32 * 7.0 + 12.0,
        }
    }

    pub(crate) fn status_bar_item_bounds(&self, item_id: &str) -> Option<Rect> {
        if item_id == "launcher" {
            return self.shell_bar_branding_bounds();
        }

        let bar_bounds = self.status_bar.compute_bounds(self.screen_rect);
        let mut right_x = bar_bounds.x + bar_bounds.width - SHELL_BAR_PADDING_X;

        for item in self.status_bar.items().iter().rev() {
            if !item.visible || item.slot != StatusBarSlot::Right {
                continue;
            }

            let width = self.shell_bar_item_width(item);
            right_x -= width;
            let bounds = Rect::new(right_x, bar_bounds.y, width, bar_bounds.height);
            if item.id == item_id {
                return Some(bounds);
            }
            right_x -= SHELL_BAR_ITEM_SPACING;
        }

        None
    }

    pub(crate) fn session_menu_bounds(&self) -> Rect {
        let menu_h =
            self.menu_padding() * 2.0 + self.session_menu_items.len() as f32 * self.menu_item_height();
        let anchor = self.status_bar_item_bounds("session").unwrap_or_else(|| {
            let bar_bounds = self.status_bar.compute_bounds(self.screen_rect);
            Rect::new(
                bar_bounds.x + bar_bounds.width - SHELL_BAR_MENU_WIDTH - SHELL_BAR_PADDING_X,
                bar_bounds.y,
                SHELL_BAR_MENU_WIDTH,
                bar_bounds.height,
            )
        });

        let screen_right = self.screen_rect.x + self.screen_rect.width;
        let screen_bottom = self.screen_rect.y + self.screen_rect.height;
        let menu_x = (anchor.x + anchor.width - SHELL_BAR_MENU_WIDTH)
            .min(screen_right - SHELL_BAR_MENU_WIDTH - 4.0)
            .max(self.screen_rect.x);
        let menu_y = (anchor.y + anchor.height + 4.0)
            .min(screen_bottom - menu_h - 4.0)
            .max(self.screen_rect.y);

        Rect::new(menu_x, menu_y, SHELL_BAR_MENU_WIDTH, menu_h)
    }

    pub(crate) fn app_menu_target_window_id(&self) -> Option<WindowId> {
        self.app_menu_open
            .as_deref()
            .and_then(|value| value.strip_prefix("window-"))
            .and_then(|value| value.parse::<u64>().ok())
            .map(WindowId)
    }

    pub(crate) fn app_menu_bounds(&self, item_count: usize) -> Option<Rect> {
        let target_window = self.app_menu_target_window_id()?;
        let window = self.windows.get(&target_window)?;
        let menu_h = self.menu_padding() * 2.0 + item_count as f32 * self.menu_item_height();
        let screen_right = self.screen_rect.x + self.screen_rect.width;
        let screen_bottom = self.screen_rect.y + self.screen_rect.height;
        let menu_x = (window.bounds.x + 8.0)
            .min(screen_right - SHELL_BAR_MENU_WIDTH - 4.0)
            .max(self.screen_rect.x);
        let menu_y = (window.bounds.y + self.decoration_style.title_bar_height)
            .min(screen_bottom - menu_h - 4.0)
            .max(self.screen_rect.y);

        Some(Rect::new(menu_x, menu_y, SHELL_BAR_MENU_WIDTH, menu_h))
    }

    fn cycle_menu_index(current: Option<usize>, len: usize, delta: isize) -> Option<usize> {
        if len == 0 {
            return None;
        }

        let next = match current {
            Some(index) => (index as isize + delta).rem_euclid(len as isize) as usize,
            None if delta >= 0 => 0,
            None => len - 1,
        };
        Some(next)
    }

    fn activate_app_menu_index(&mut self, index: usize) -> Option<ShellAction> {
        let target_window = self.app_menu_target_window_id();
        self.app_menu_open = None;
        self.app_menu_hover_index = None;

        let action = match index {
            0 => ShellAction::MinimizeWindow,
            1 => ShellAction::MaximizeWindow,
            2 => ShellAction::CloseWindow,
            3 => ShellAction::OpenSettings,
            4 => ShellAction::Redraw,
            _ => return Some(ShellAction::Redraw),
        };

        if matches!(
            action,
            ShellAction::MinimizeWindow | ShellAction::MaximizeWindow | ShellAction::CloseWindow
        ) {
            if let Some(window_id) = target_window {
                let _ = self.set_focus(window_id);
                let _ = self.raise_window(window_id);
            }
        }

        Some(action)
    }

    /// Handle a platform event and return any resulting shell action.
    pub fn handle_platform_event(&mut self, event: &PlatformEvent) -> Option<ShellAction> {
        use liquide_input::keyboard::{KeyCode, KeyState};

        match event {
            PlatformEvent::KeyInput { event: ke, .. } => {
                // Route the key into the DOM event dispatcher FIRST (t65-s2
                // item 2) so a focused DOM/app KeyDown/KeyUp listener sees it.
                // A listener may call `preventDefault`, which gates the shell
                // shortcut below (t65-s2 item 3). KeyUp/Repeat are forwarded to
                // the DOM too, then non-Pressed states return (the shell shortcut
                // table is press-only).
                let default_prevented = self.dispatch_dom_keyboard_event(ke);
                if ke.state != KeyState::Pressed {
                    return None;
                }
                if self.launcher.is_visible() {
                    match ke.key {
                        KeyCode::Escape => {
                            self.launcher.close();
                            self.hook_manager.dispatch(&ShellHookEvent::LauncherClosed);
                            return Some(ShellAction::Redraw);
                        }
                        KeyCode::ArrowUp => {
                            self.launcher.select_prev();
                            return Some(ShellAction::Redraw);
                        }
                        KeyCode::ArrowDown => {
                            self.launcher.select_next();
                            return Some(ShellAction::Redraw);
                        }
                        KeyCode::Enter => {
                            if let Some(kind) = self.launcher.activate_selected().cloned() {
                                self.launcher.close();
                                self.hook_manager.dispatch(&ShellHookEvent::LauncherClosed);
                                if let SearchResultKind::Application { ref app_id } = kind {
                                    self.open_app_window(app_id);
                                }
                            } else {
                                self.launcher.close();
                                self.hook_manager.dispatch(&ShellHookEvent::LauncherClosed);
                            }
                            return Some(ShellAction::Redraw);
                        }
                        KeyCode::Backspace => {
                            let q = self.launcher.query().to_string();
                            if !q.is_empty() {
                                let new_q = if let Some((idx, _)) = q.char_indices().last() {
                                    &q[..idx]
                                } else {
                                    ""
                                };
                                self.launcher.set_query(new_q);
                            }
                            return Some(ShellAction::Redraw);
                        }
                        other => {
                            if let Some(ch) = Self::keycode_to_char(other) {
                                let mut q = self.launcher.query().to_string();
                                q.push(ch);
                                self.launcher.set_query(&q);
                                return Some(ShellAction::Redraw);
                            }
                            return None;
                        }
                    }
                }
                if self.context_menu_visible {
                    // Keyboard nav for the desktop right-click context menu,
                    // mirroring the session-menu arms below: ArrowDown/ArrowUp
                    // advance/wrap the highlight, Enter activates the highlighted
                    // item, Escape closes. The highlight renders via the
                    // `menu-item:hover` pseudo-state set in `sync_context_menu_template`.
                    let ctx_len = ContextMenuItem::defaults().len();
                    match ke.key {
                        KeyCode::Escape => {
                            self.context_menu_visible = false;
                            self.context_menu_hover_index = None;
                            return Some(ShellAction::Redraw);
                        }
                        KeyCode::ArrowDown => {
                            self.context_menu_hover_index =
                                Self::cycle_menu_index(self.context_menu_hover_index, ctx_len, 1);
                            return Some(ShellAction::Redraw);
                        }
                        KeyCode::ArrowUp => {
                            self.context_menu_hover_index =
                                Self::cycle_menu_index(self.context_menu_hover_index, ctx_len, -1);
                            return Some(ShellAction::Redraw);
                        }
                        KeyCode::Enter => {
                            let idx = self.context_menu_hover_index.unwrap_or(0);
                            self.context_menu_visible = false;
                            self.context_menu_hover_index = None;
                            let ctx_items = ContextMenuItem::defaults();
                            if idx < ctx_items.len() {
                                return Some(ctx_items[idx].action.clone());
                            }
                            return Some(ShellAction::Redraw);
                        }
                        _ => {}
                    }
                }
                if self.session_menu_visible {
                    match ke.key {
                        KeyCode::Escape => {
                            self.session_menu_visible = false;
                            self.session_menu_hover_index = None;
                            return Some(ShellAction::Redraw);
                        }
                        KeyCode::ArrowDown => {
                            self.session_menu_hover_index = Self::cycle_menu_index(
                                self.session_menu_hover_index,
                                self.session_menu_items.len(),
                                1,
                            );
                            return Some(ShellAction::Redraw);
                        }
                        KeyCode::ArrowUp => {
                            self.session_menu_hover_index = Self::cycle_menu_index(
                                self.session_menu_hover_index,
                                self.session_menu_items.len(),
                                -1,
                            );
                            return Some(ShellAction::Redraw);
                        }
                        KeyCode::Enter => {
                            let idx = self.session_menu_hover_index.unwrap_or(0);
                            self.session_menu_visible = false;
                            self.session_menu_hover_index = None;
                            if idx < self.session_menu_items.len() {
                                return Some(self.session_menu_items[idx].action.clone());
                            }
                            return Some(ShellAction::Redraw);
                        }
                        _ => {}
                    }
                }
                if self.app_menu_open.is_some() {
                    match ke.key {
                        KeyCode::Escape => {
                            self.app_menu_open = None;
                            self.app_menu_hover_index = None;
                            return Some(ShellAction::Redraw);
                        }
                        KeyCode::ArrowDown => {
                            self.app_menu_hover_index =
                                Self::cycle_menu_index(self.app_menu_hover_index, 5, 1);
                            return Some(ShellAction::Redraw);
                        }
                        KeyCode::ArrowUp => {
                            self.app_menu_hover_index =
                                Self::cycle_menu_index(self.app_menu_hover_index, 5, -1);
                            return Some(ShellAction::Redraw);
                        }
                        KeyCode::Enter => {
                            let idx = self.app_menu_hover_index.unwrap_or(0);
                            return self.activate_app_menu_index(idx);
                        }
                        _ => {}
                    }
                }
                // preventDefault gate (t65-s2 item 3): if a focused DOM/app
                // listener consumed this key via `preventDefault`, the key has
                // already been handled by the DOM — do NOT also run the shell's
                // default action (text-input fall-through or a global shortcut).
                if default_prevented {
                    return Some(ShellAction::Redraw);
                }

                // Text-input seam (t57-fG feature 2): when no shell overlay is
                // capturing the key (handled above) and a printable character is
                // typed with no command modifier (ctrl/alt/super), route it into
                // the FOCUSED window's text buffer so keyboard text reaches the
                // focused app/window. Command-modified keys fall through to the
                // shortcut table below so hotkeys are unaffected.
                if !ke.modifiers.ctrl() && !ke.modifiers.alt() && !ke.modifiers.super_key() {
                    if let Some(ch) = Self::keycode_to_char(ke.key) {
                        if let Some(wid) = self.focus.focused() {
                            if self.windows.contains_key(&wid) {
                                self.route_char_to_focused_app(wid, ch);
                                return Some(ShellAction::Redraw);
                            }
                        }
                    }
                    // Non-printable navigation/edit keys (Enter/Backspace/arrows/…)
                    // reach the focused window's live app view (t70-s6) so the
                    // app's model — terminal CR, editor newline, list navigation —
                    // sees them. Only forwarded when an app view is registered;
                    // otherwise the key falls through to the shortcut table below.
                    if let Some(app_key) = Self::keycode_to_app_key(ke.key) {
                        if let Some(wid) = self.focus.focused() {
                            if self.app_views.contains_key(&wid)
                                && self.route_key_to_focused_app(wid, &app_key)
                            {
                                return Some(ShellAction::Redraw);
                            }
                        }
                    }
                }
                self.shortcuts.handle_key_event(ke).cloned()
            }
            PlatformEvent::MouseInput { event: me, .. } => {
                self.dispatch_dom_mouse_event(me);
                match me {
                    MouseEvent::Move { x, y } => self.handle_mouse_move(*x, *y),
                    MouseEvent::Button {
                        button,
                        state,
                        x,
                        y,
                    } => self.handle_mouse_button(*button, *state, *x, *y),
                    _ => None,
                }
            }
            PlatformEvent::WindowResized { width, height, .. } => {
                self.resize_screen(*width as f32, *height as f32);
                None
            }
            _ => None,
        }
    }

    fn handle_mouse_move(&mut self, x: f32, y: f32) -> Option<ShellAction> {
        let pt = Point::new(x, y);
        let mut need_redraw = false;

        // Track cursor Y for status-bar auto-reveal on top-edge hover.
        self.last_cursor_y = y;

        // Active drag handling
        if let Some(drag) = self.drag_state {
            match drag {
                DragState::Moving {
                    window_id,
                    offset_x,
                    offset_y,
                } => {
                    self.cursor_shape = CursorShape::Move;
                    let mut window_scene_changed = false;
                    if let Some(window) = self.windows.get_mut(&window_id) {
                        window.bounds.x = x - offset_x;
                        window.bounds.y = y - offset_y;
                        if window.state == WindowState::Maximized {
                            window.state = WindowState::Normal;
                        }
                        // A free move un-tiles the window; re-snap is decided
                        // below from the live cursor position.
                        window.tiled = false;
                        window.tile_zone = None;
                        window_scene_changed = true;
                    }
                    // Consult the canonical tiling snap zones for the live cursor
                    // position so the active snap target is previewed during the
                    // drag (applied on release by `apply_snap_on_release`).
                    self.update_snap_preview_for_drag(x, y);
                    if window_scene_changed {
                        self.mark_window_scene_dirty();
                    }
                    return Some(ShellAction::Redraw);
                }
                DragState::Resizing {
                    window_id,
                    edge,
                    start_bounds,
                    start_x,
                    start_y,
                } => {
                    self.cursor_shape = Self::cursor_for_hit_zone(edge);
                    let dx = x - start_x;
                    let dy = y - start_y;
                    let min_w = self
                        .windows
                        .get(&window_id)
                        .and_then(|w| w.min_size)
                        .map(|(mw, _)| mw)
                        .unwrap_or(120.0);
                    let min_h = self
                        .windows
                        .get(&window_id)
                        .and_then(|w| w.min_size)
                        .map(|(_, mh)| mh)
                        .unwrap_or(80.0);
                    let mut window_scene_changed = false;
                    if let Some(window) = self.windows.get_mut(&window_id) {
                        match edge {
                            HitZone::ResizeRight => {
                                window.bounds.width = (start_bounds.width + dx).max(min_w);
                            }
                            HitZone::ResizeBottom => {
                                window.bounds.height = (start_bounds.height + dy).max(min_h);
                            }
                            HitZone::ResizeLeft => {
                                let new_w = (start_bounds.width - dx).max(min_w);
                                window.bounds.x = start_bounds.x + start_bounds.width - new_w;
                                window.bounds.width = new_w;
                            }
                            HitZone::ResizeTop => {
                                let new_h = (start_bounds.height - dy).max(min_h);
                                window.bounds.y = start_bounds.y + start_bounds.height - new_h;
                                window.bounds.height = new_h;
                            }
                            HitZone::ResizeTopLeft => {
                                let new_w = (start_bounds.width - dx).max(min_w);
                                let new_h = (start_bounds.height - dy).max(min_h);
                                window.bounds.x = start_bounds.x + start_bounds.width - new_w;
                                window.bounds.y = start_bounds.y + start_bounds.height - new_h;
                                window.bounds.width = new_w;
                                window.bounds.height = new_h;
                            }
                            HitZone::ResizeTopRight => {
                                let new_h = (start_bounds.height - dy).max(min_h);
                                window.bounds.y = start_bounds.y + start_bounds.height - new_h;
                                window.bounds.width = (start_bounds.width + dx).max(min_w);
                                window.bounds.height = new_h;
                            }
                            HitZone::ResizeBottomLeft => {
                                let new_w = (start_bounds.width - dx).max(min_w);
                                window.bounds.x = start_bounds.x + start_bounds.width - new_w;
                                window.bounds.width = new_w;
                                window.bounds.height = (start_bounds.height + dy).max(min_h);
                            }
                            HitZone::ResizeBottomRight => {
                                window.bounds.width = (start_bounds.width + dx).max(min_w);
                                window.bounds.height = (start_bounds.height + dy).max(min_h);
                            }
                            _ => {}
                        }
                        window_scene_changed = true;
                    }
                    if window_scene_changed {
                        self.mark_window_scene_dirty();
                    }
                    return Some(ShellAction::Redraw);
                }
            }
        }

        // Decoration button hover detection
        let prev_hover = self.hovered_button;
        self.hovered_button = None;
        let tbh = self.decoration_style.title_bar_height;
        for window in self.visible_windows().into_iter().rev() {
            if !window.flags.contains(WindowFlags::DECORATED) {
                continue;
            }
            if y >= window.bounds.y
                && y < window.bounds.y + tbh
                && x >= window.bounds.x
                && x < window.bounds.x + window.bounds.width
            {
                let client = Rect::new(
                    window.bounds.x,
                    window.bounds.y + tbh,
                    window.bounds.width,
                    (window.bounds.height - tbh).max(0.0),
                );
                let zone = hit_test_decoration(client, &self.decoration_style, x, y);
                match zone {
                    HitZone::CloseButton
                    | HitZone::MaximizeButton
                    | HitZone::MinimizeButton
                    | HitZone::AlwaysOnTopButton => {
                        self.hovered_button = Some((window.id, zone));
                    }
                    _ => {}
                }
                break;
            }
        }
        if self.hovered_button != prev_hover {
            need_redraw = true;
            self.mark_window_scene_dirty();
        }

        // Dock hover
        let dock_bounds = self.dock.compute_bounds(self.screen_rect);
        if dock_bounds.contains(pt) {
            let item_rects = self.dock.compute_item_rects(self.screen_rect);
            let mut found = None;
            for (i, (_, rect)) in item_rects.iter().enumerate() {
                if rect.contains(pt) {
                    found = Some(i);
                    break;
                }
            }
            let prev = self.dock.hover_index();
            if let Some(idx) = found {
                self.dock.on_hover(idx);
                // Set tooltip to the dock item's label, positioned at center-top.
                let items = self.dock.items();
                if idx < items.len() {
                    let label = items[idx].label.clone();
                    let (_, item_rect) = &item_rects[idx];
                    // Approximate tooltip width to clamp position to screen
                    let tip_w = (label.len() as f32 * 7.0 + 16.0).max(40.0).min(300.0);
                    let tip_x = (x - tip_w / 2.0)
                        .max(4.0)
                        .min(self.screen_rect.width - tip_w - 4.0);
                    let tip_y = (item_rect.y - 32.0).max(4.0); // above the dock item
                    // The show-delay dwell is owned by the canonical
                    // TooltipManager (t51-e9/e15), driven from this hover state
                    // each frame; the retired `tooltip_timer_us` timer reset is
                    // no longer needed here. (The manager keys hover on a single
                    // shell slot, so moving between dock items keeps the tooltip
                    // up while only the rendered `tooltip_text` changes.)
                    self.tooltip_text = Some(label);
                    self.tooltip_pos = Point::new(tip_x, tip_y);
                }
            } else {
                self.dock.on_hover_leave();
                self.tooltip_text = None;
            }
            if self.dock.hover_index() != prev {
                need_redraw = true;
            }
        } else {
            if self.dock.hover_index().is_some() {
                need_redraw = true;
            }
            self.dock.on_hover_leave();
            self.tooltip_text = None;
        }

        // Auto-hide reveal: feed the cursor sample to the dock's reveal state
        // machine. When the cursor reaches the dock's edge hot-zone (or stays
        // over a revealed dock) the dock shows; leaving hides it again (subject
        // to mode). A visibility flip needs a redraw + scene reflow so the
        // `data-hidden` attr / placement updates (t72-dock follow-up §4). No-op
        // when auto-hide is Off.
        let dock_was_visible = self.dock.is_visible();
        self.dock.on_cursor_moved(self.screen_rect, (x, y));
        if self.dock.is_visible() != dock_was_visible {
            need_redraw = true;
            self.mark_window_scene_dirty();
        }

        // Context menu hover
        if self.context_menu_visible {
            let menu_padding = self.menu_padding();
            let menu_item_height = self.menu_item_height();
            let context_menu_width = self.context_menu_width();
            let ctx_items = ContextMenuItem::defaults();
            let ctx_h = menu_padding * 2.0 + ctx_items.len() as f32 * menu_item_height;
            let ctx_x = self
                .context_menu_pos
                .x
                .min(self.screen_rect.width - context_menu_width - 4.0)
                .max(0.0);
            let ctx_y = self
                .context_menu_pos
                .y
                .min(self.screen_rect.height - ctx_h - 4.0)
                .max(0.0);
            let ctx_bounds = Rect::new(ctx_x, ctx_y, context_menu_width, ctx_h);
            let prev_hover = self.context_menu_hover_index;
            if ctx_bounds.contains(pt) {
                let rel_y = y - ctx_y - menu_padding;
                if rel_y >= 0.0 {
                    let idx = (rel_y / menu_item_height) as usize;
                    self.context_menu_hover_index = if idx < ctx_items.len() {
                        Some(idx)
                    } else {
                        None
                    };
                } else {
                    self.context_menu_hover_index = None;
                }
            } else {
                self.context_menu_hover_index = None;
            }
            if self.context_menu_hover_index != prev_hover {
                need_redraw = true;
            }
        }

        // Session menu hover
        if self.session_menu_visible {
            let menu_padding = self.menu_padding();
            let menu_item_height = self.menu_item_height();
            let menu_bounds = self.session_menu_bounds();
            let prev_hover = self.session_menu_hover_index;
            if menu_bounds.contains(pt) {
                let rel_y = y - menu_bounds.y - menu_padding;
                if rel_y >= 0.0 {
                    let idx = (rel_y / menu_item_height) as usize;
                    self.session_menu_hover_index = if idx < self.session_menu_items.len() {
                        Some(idx)
                    } else {
                        None
                    };
                } else {
                    self.session_menu_hover_index = None;
                }
            } else {
                self.session_menu_hover_index = None;
            }
            if self.session_menu_hover_index != prev_hover {
                need_redraw = true;
            }
        }

        // App menu hover
        if self.app_menu_open.is_some() {
            let menu_padding = self.menu_padding();
            let menu_item_height = self.menu_item_height();
            let menu_item_count = 5usize; // Minimize, Maximize, Close, Settings, About
            let prev_hover = self.app_menu_hover_index;
            if let Some(menu_bounds) = self.app_menu_bounds(menu_item_count) {
                if menu_bounds.contains(pt) {
                    let rel_y = y - menu_bounds.y - menu_padding;
                    if rel_y >= 0.0 {
                        let idx = (rel_y / menu_item_height) as usize;
                        self.app_menu_hover_index = if idx < menu_item_count {
                            Some(idx)
                        } else {
                            None
                        };
                    } else {
                        self.app_menu_hover_index = None;
                    };
                } else {
                    self.app_menu_hover_index = None;
                }
            } else {
                self.app_menu_hover_index = None;
            }
            if self.app_menu_hover_index != prev_hover {
                need_redraw = true;
            }
        }

        // Cursor shape determination
        let prev_cursor = self.cursor_shape;
        self.cursor_shape = CursorShape::Arrow;
        if self.dock.hover_index().is_some() {
            self.cursor_shape = CursorShape::Pointer;
        } else if self.context_menu_hover_index.is_some()
            || self.session_menu_hover_index.is_some()
            || self.app_menu_hover_index.is_some()
        {
            self.cursor_shape = CursorShape::Pointer;
        } else if self.hovered_button.is_some() {
            self.cursor_shape = CursorShape::Pointer;
        } else {
            for window in self.visible_windows().into_iter().rev() {
                if !window.flags.contains(WindowFlags::DECORATED) {
                    continue;
                }
                let client = Rect::new(
                    window.bounds.x,
                    window.bounds.y + tbh,
                    window.bounds.width,
                    (window.bounds.height - tbh).max(0.0),
                );
                let zone = hit_test_decoration(client, &self.decoration_style, x, y);
                match zone {
                    HitZone::Outside => continue,
                    HitZone::TitleBar | HitZone::Client => break,
                    zone => {
                        self.cursor_shape = Self::cursor_for_hit_zone(zone);
                        break;
                    }
                }
            }
        }
        if self.cursor_shape != prev_cursor {
            need_redraw = true;
        }

        if need_redraw {
            Some(ShellAction::Redraw)
        } else {
            None
        }
    }

    /// Route a typed character into the focused window. When a live app view is
    /// registered (t70-s6), the character is forwarded into the app's model via
    /// `handle_text` so keyboard text reaches the real app; otherwise it falls
    /// back to the local typed-text buffer (the shell side of the t57-fG
    /// text-input seam). Either way the window scene is invalidated so the
    /// change repaints.
    fn route_char_to_focused_app(&mut self, wid: WindowId, ch: char) {
        if let Some(view) = self.app_views.get_mut(&wid) {
            let mut buf = [0u8; 4];
            let _changed = view.handle_text(ch.encode_utf8(&mut buf));
            // Always repaint: even a "no change" key may move the caret, and the
            // app content revision must advance so the cache re-renders.
            self.mark_app_content_dirty(wid);
            return;
        }
        let buf = self.focused_app_text.entry(wid).or_default();
        buf.push(ch);
        self.mark_window_scene_dirty();
    }

    /// Route a non-printable logical key into the focused window's live app view
    /// (t70-s6). Returns `true` if a view was registered and consumed the key
    /// (so the caller should stop and request a redraw); `false` lets the key
    /// fall through to the shell's local buffer / shortcut handling.
    fn route_key_to_focused_app(&mut self, wid: WindowId, key: &liquide_interop::AppKey) -> bool {
        if let Some(view) = self.app_views.get_mut(&wid) {
            let _changed = view.handle_key(key);
            self.mark_app_content_dirty(wid);
            return true;
        }
        false
    }

    /// Map a platform [`KeyCode`](liquide_input::keyboard::KeyCode) to a logical
    /// [`AppKey`](liquide_interop::AppKey) for forwarding into an app view. Only
    /// keys an app model cares about are mapped; everything else returns `None`
    /// so it stays available for the shell's own handling.
    fn keycode_to_app_key(
        key: liquide_input::keyboard::KeyCode,
    ) -> Option<liquide_interop::AppKey> {
        use liquide_input::keyboard::KeyCode;
        use liquide_interop::AppKey;
        Some(match key {
            KeyCode::Enter => AppKey::Enter,
            KeyCode::Backspace => AppKey::Backspace,
            KeyCode::Tab => AppKey::Tab,
            KeyCode::Escape => AppKey::Escape,
            KeyCode::Delete => AppKey::Delete,
            KeyCode::ArrowLeft => AppKey::Left,
            KeyCode::ArrowRight => AppKey::Right,
            KeyCode::ArrowUp => AppKey::Up,
            KeyCode::ArrowDown => AppKey::Down,
            KeyCode::Home => AppKey::Home,
            KeyCode::End => AppKey::End,
            KeyCode::PageUp => AppKey::PageUp,
            KeyCode::PageDown => AppKey::PageDown,
            _ => return None,
        })
    }

    /// Whether a title-bar press on `wid` at `pt` is the second click of a
    /// double-click: same window, within [`DOUBLE_CLICK_MS`] of the recorded
    /// previous title-bar press, and within [`DOUBLE_CLICK_DIST_PX`] of it.
    fn is_titlebar_double_click(&self, wid: WindowId, pt: Point) -> bool {
        match self.last_titlebar_click {
            Some((prev_id, prev_pt, prev_at)) => {
                prev_id == wid
                    && prev_at.elapsed().as_millis() <= super::DOUBLE_CLICK_MS
                    && (pt.x - prev_pt.x).abs() <= super::DOUBLE_CLICK_DIST_PX
                    && (pt.y - prev_pt.y).abs() <= super::DOUBLE_CLICK_DIST_PX
            }
            None => false,
        }
    }

    fn handle_mouse_button(
        &mut self,
        button: MouseButton,
        state: ButtonState,
        x: f32,
        y: f32,
    ) -> Option<ShellAction> {
        if state == ButtonState::Released {
            if let Some(drag) = self.drag_state {
                // If a move drag ended over an active snap zone, tile the window
                // into that zone (drives `liquide_tiling` snap geometry).
                if let DragState::Moving { window_id, .. } = drag {
                    if !self.apply_snap_on_release(window_id) {
                        self.clear_snap_preview();
                    }
                } else {
                    self.clear_snap_preview();
                }
                self.drag_state = None;
                self.cursor_shape = CursorShape::Arrow;
                return Some(ShellAction::Redraw);
            }
            return None;
        }

        let pt = Point::new(x, y);

        // Right-click
        if button == MouseButton::Right {
            self.session_menu_visible = false;
            self.session_menu_hover_index = None;
            self.app_menu_open = None;
            self.app_menu_hover_index = None;
            let bar_bounds = self.status_bar.compute_bounds(self.screen_rect);
            let dock_bounds = self.dock.compute_bounds(self.screen_rect);
            if bar_bounds.contains(pt) {
                self.context_menu_visible = true;
                self.context_menu_hover_index = None;
                self.context_menu_pos = pt;
                return Some(ShellAction::Redraw);
            }
            if dock_bounds.contains(pt) {
                self.context_menu_visible = true;
                self.context_menu_hover_index = None;
                self.context_menu_pos = pt;
                return Some(ShellAction::Redraw);
            }
            let tbh = self.decoration_style.title_bar_height;
            let titlebar_window = self
                .visible_windows()
                .into_iter()
                .rev()
                .find(|w| {
                    let title_rect = Rect::new(w.bounds.x, w.bounds.y, w.bounds.width, tbh);
                    title_rect.contains(pt) && w.flags.contains(WindowFlags::DECORATED)
                })
                .map(|w| w.id);
            if let Some(wid) = titlebar_window {
                // Show the app menu (Minimize/Maximize/Close) instead of generic context menu
                let win_id_str = format!("window-{}", wid.0);
                self.app_menu_open = Some(win_id_str);
                self.app_menu_hover_index = Some(0);
                self.context_menu_visible = false;
                self.context_menu_hover_index = None;
                return Some(ShellAction::Redraw);
            }
            let on_window = self
                .visible_windows()
                .iter()
                .rev()
                .any(|w| w.bounds.contains(pt));
            if !on_window {
                self.context_menu_visible = true;
                self.context_menu_pos = pt;
                return Some(ShellAction::Redraw);
            }
            return None;
        }

        if button != MouseButton::Left {
            return None;
        }

        // Context menu click
        if self.context_menu_visible {
            let menu_padding = self.menu_padding();
            let menu_item_height = self.menu_item_height();
            let context_menu_width = self.context_menu_width();
            let ctx_items = ContextMenuItem::defaults();
            let ctx_h = menu_padding * 2.0 + ctx_items.len() as f32 * menu_item_height;
            let ctx_x = self
                .context_menu_pos
                .x
                .min(self.screen_rect.width - context_menu_width - 4.0)
                .max(0.0);
            let ctx_y = self
                .context_menu_pos
                .y
                .min(self.screen_rect.height - ctx_h - 4.0)
                .max(0.0);
            let ctx_bounds = Rect::new(ctx_x, ctx_y, context_menu_width, ctx_h);
            if ctx_bounds.contains(pt) {
                let rel_y = y - ctx_y - menu_padding;
                self.context_menu_visible = false;
                self.context_menu_hover_index = None;
                if rel_y >= 0.0 {
                    let idx = (rel_y / menu_item_height) as usize;
                    if idx < ctx_items.len() {
                        return Some(ctx_items[idx].action.clone());
                    }
                }
                return Some(ShellAction::Redraw);
            }
            self.context_menu_visible = false;
            self.context_menu_hover_index = None;
            return Some(ShellAction::Redraw);
        }

        // Session menu click
        if self.session_menu_visible {
            let menu_padding = self.menu_padding();
            let menu_item_height = self.menu_item_height();
            let menu_bounds = self.session_menu_bounds();
            if menu_bounds.contains(pt) {
                let rel_y = y - menu_bounds.y - menu_padding;
                self.session_menu_visible = false;
                self.session_menu_hover_index = None;
                if rel_y >= 0.0 {
                    let idx = (rel_y / menu_item_height) as usize;
                    if idx < self.session_menu_items.len() {
                        return Some(self.session_menu_items[idx].action.clone());
                    }
                }
                return Some(ShellAction::Redraw);
            }
            self.session_menu_visible = false;
            self.session_menu_hover_index = None;
            return Some(ShellAction::Redraw);
        }

        // App menu click (window-specific: Minimize/Maximize/Close/Settings/About)
        if self.app_menu_open.is_some() {
            let menu_padding = self.menu_padding();
            let menu_item_height = self.menu_item_height();
            let menu_item_count = 5usize;
            if let Some(menu_bounds) = self.app_menu_bounds(menu_item_count) {
                if menu_bounds.contains(pt) {
                    let rel_y = y - menu_bounds.y - menu_padding;
                    if rel_y >= 0.0 {
                        let idx = (rel_y / menu_item_height) as usize;
                        if idx < menu_item_count {
                            return self.activate_app_menu_index(idx);
                        }
                    }
                    self.app_menu_open = None;
                    self.app_menu_hover_index = None;
                    return Some(ShellAction::Redraw);
                }
            }
            self.app_menu_open = None;
            self.app_menu_hover_index = None;
            return Some(ShellAction::Redraw);
        }

        // Launcher click
        if self.launcher.is_visible() {
            let screen = self.screen_rect;
            let panel_w = 480.0_f32; // matches CSS: launcher { width: 480 }
            let panel_h = 600.0_f32; // matches CSS: launcher { max-height: 600 }
            let panel_x = screen.x + (screen.width - panel_w) / 2.0;
            let panel_y = screen.y + (screen.height - panel_h) / 2.0;
            let panel_bounds = Rect::new(panel_x, panel_y, panel_w, panel_h);
            if !panel_bounds.contains(pt) {
                self.launcher.close();
                self.hook_manager.dispatch(&ShellHookEvent::LauncherClosed);
                return Some(ShellAction::Redraw);
            }
            // padding(16) + search height(36) + search margin-bottom(8) = 60
            let item_start_y = panel_y + 60.0;
            let item_height = 40.0_f32;
            let item_gap = 2.0_f32; // matches CSS: launcher-results { gap: 2 }
            if y >= item_start_y {
                let rel_y = y - item_start_y;
                let idx = (rel_y / (item_height + item_gap)) as usize;
                self.launcher.select_index(idx);
                if let Some(kind) = self.launcher.activate_selected().cloned() {
                    self.launcher.close();
                    self.hook_manager.dispatch(&ShellHookEvent::LauncherClosed);
                    if let SearchResultKind::Application { ref app_id } = kind {
                        self.open_app_window(app_id);
                    }
                }
                return Some(ShellAction::Redraw);
            }
            return None;
        }

        // Status bar click
        let bar_bounds = self.status_bar.compute_bounds(self.screen_rect);
        if bar_bounds.contains(pt) {
            if self
                .status_bar_item_bounds("launcher")
                .map_or(false, |bounds| bounds.contains(pt))
            {
                let was_visible = self.launcher.is_visible();
                self.launcher.toggle();
                if was_visible {
                    self.hook_manager.dispatch(&ShellHookEvent::LauncherClosed);
                } else {
                    self.hook_manager.dispatch(&ShellHookEvent::LauncherOpened);
                }
                return Some(ShellAction::Redraw);
            }
            if self
                .status_bar_item_bounds("session")
                .map_or(false, |bounds| bounds.contains(pt))
            {
                // Single-owner toggle contract (t59-shell): the click handler must
                // NOT mutate `session_menu_visible` here — it only returns the
                // action, and `execute_action(OpenSessionMenu)` (tick.rs) performs
                // the one-and-only toggle. Mutating here AND returning the action
                // caused a DOUBLE-TOGGLE (open-then-instantly-close) once the
                // integrated input path runs `execute_action` on the result.
                return Some(ShellAction::OpenSessionMenu);
            }
            // The notification indicator occupies a fixed 36..80 px hit
            // region from the right edge of the status bar, regardless of
            // the dynamically-computed item layout.  This matches the spec
            // exercised by `notification_indicator_click_toggles_panel`.
            let from_right = bar_bounds.x + bar_bounds.width - x;
            let has_notification_indicator = self.status_bar.items().iter().any(|item| {
                item.visible
                    && matches!(
                        item.kind,
                        liquide_statusbar::StatusBarItemKind::NotificationIndicator { .. }
                    )
            });
            if has_notification_indicator && (36.0..=80.0).contains(&from_right) {
                // Route through the canonical notification center (t51-e14):
                // the panel that dom_sync now renders reads the live
                // (daemon-mirrored) notification set rather than a dead flag, so
                // this opens a real center (fixes t49-e5-F03).
                //
                // Single-owner toggle contract (t59-shell): do NOT call
                // `toggle_notification_center()` here — only return the action.
                // `execute_action(OpenNotificationCenter)` (tick.rs) is the single
                // owner of the toggle. Toggling here AND returning the action
                // caused a DOUBLE-TOGGLE that cancelled the click.
                return Some(ShellAction::OpenNotificationCenter);
            }
            if self
                .status_bar_item_bounds("notifications")
                .map_or(false, |bounds| bounds.contains(pt))
            {
                // Single-owner toggle: return the action only (see above).
                return Some(ShellAction::OpenNotificationCenter);
            }
            return None;
        }

        // Dock click
        let dock_bounds = self.dock.compute_bounds(self.screen_rect);
        if dock_bounds.contains(pt) {
            let item_rects = self.dock.compute_item_rects(self.screen_rect);
            for (i, (_, rect)) in item_rects.iter().enumerate() {
                if rect.contains(pt) {
                    let items = self.dock.items();
                    if i < items.len() {
                        let app_id = items[i].app_id.clone();
                        if !app_id.is_empty() {
                            self.open_app_window(&app_id);
                            return Some(ShellAction::Redraw);
                        }
                    }
                    break;
                }
            }
            return None;
        }

        // Window click with decoration hit-testing.
        // Use resize_tolerance (not border_width) so the expanded rect covers
        // the full area where hit_test_decoration can return resize zones.
        let mut clicked = None;
        let tbh = self.decoration_style.title_bar_height;
        for window in self.visible_windows().into_iter().rev() {
            let rt = self.decoration_style.resize_tolerance;
            let expanded = Rect::new(
                window.bounds.x - rt,
                window.bounds.y - rt,
                window.bounds.width + rt * 2.0,
                window.bounds.height + rt * 2.0,
            );
            if expanded.contains(pt) {
                clicked = Some(window.id);
                break;
            }
        }

        if let Some(wid) = clicked {
            let is_decorated = self
                .windows
                .get(&wid)
                .map(|w| w.flags.contains(WindowFlags::DECORATED))
                .unwrap_or(false);
            let is_resizable = self
                .windows
                .get(&wid)
                .map(|w| w.flags.contains(WindowFlags::RESIZABLE))
                .unwrap_or(false);
            if is_decorated {
                let bounds = self.windows[&wid].bounds;
                let client = Rect::new(
                    bounds.x,
                    bounds.y + tbh,
                    bounds.width,
                    (bounds.height - tbh).max(0.0),
                );
                let zone = hit_test_decoration(client, &self.decoration_style, x, y);
                match zone {
                    HitZone::CloseButton => {
                        let _ = self.set_focus(wid);
                        return Some(ShellAction::CloseWindow);
                    }
                    HitZone::MaximizeButton => {
                        let _ = self.set_focus(wid);
                        return Some(ShellAction::MaximizeWindow);
                    }
                    HitZone::MinimizeButton => {
                        let _ = self.set_focus(wid);
                        return Some(ShellAction::MinimizeWindow);
                    }
                    HitZone::AlwaysOnTopButton => {
                        let _ = self.set_focus(wid);
                        return Some(ShellAction::ToggleAlwaysOnTop);
                    }
                    HitZone::TitleBar => {
                        let _ = self.set_focus(wid);
                        let _ = self.raise_window(wid);
                        // Double-click detection (t57-fG feature 1): a second
                        // title-bar press on the SAME window within the
                        // double-click time + distance window toggles maximize/
                        // restore instead of starting a drag.
                        if self.is_titlebar_double_click(wid, pt) {
                            self.last_titlebar_click = None;
                            let is_maximized = self
                                .windows
                                .get(&wid)
                                .map(|w| w.state == WindowState::Maximized)
                                .unwrap_or(false);
                            if is_maximized {
                                let _ = self.restore(wid);
                            } else {
                                let _ = self.maximize(wid);
                            }
                            return Some(ShellAction::Redraw);
                        }
                        // First (or stale) title-bar press: record it for
                        // double-click detection and begin a move drag. A single
                        // click/drag is unaffected (the drag still starts here).
                        self.last_titlebar_click = Some((wid, pt, std::time::Instant::now()));
                        self.drag_state = Some(DragState::Moving {
                            window_id: wid,
                            offset_x: x - bounds.x,
                            offset_y: y - bounds.y,
                        });
                        return Some(ShellAction::Redraw);
                    }
                    HitZone::ResizeTop
                    | HitZone::ResizeBottom
                    | HitZone::ResizeLeft
                    | HitZone::ResizeRight
                    | HitZone::ResizeTopLeft
                    | HitZone::ResizeTopRight
                    | HitZone::ResizeBottomLeft
                    | HitZone::ResizeBottomRight
                        if is_resizable =>
                    {
                        let _ = self.set_focus(wid);
                        let _ = self.raise_window(wid);
                        self.drag_state = Some(DragState::Resizing {
                            window_id: wid,
                            edge: zone,
                            start_bounds: bounds,
                            start_x: x,
                            start_y: y,
                        });
                        return Some(ShellAction::Redraw);
                    }
                    _ => {
                        let _ = self.set_focus(wid);
                        let _ = self.raise_window(wid);
                    }
                }
            } else {
                let _ = self.set_focus(wid);
                let _ = self.raise_window(wid);
            }
        }
        None
    }
}
