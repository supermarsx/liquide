//! Mouse and keyboard event handling, click dispatch, DOM event forwarding.

use liquide_compositor::geometry::{Point, Rect};
use liquide_compositor::scene::CursorShape;
use liquide_input::mouse::{ButtonState, MouseButton, MouseEvent};
use liquide_platform::PlatformEvent;

use crate::decoration::{HitZone, hit_test_decoration};
use crate::focus::FocusPolicy;
use crate::ime::ImeOutcome;
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
            let modifiers = self.keyboard_modifiers;
            let scroll_target = {
                let hit_test = match self.hit_test_engine.as_ref() {
                    Some(ht) => ht,
                    None => return,
                };
                // Seam-2: thread the live keyboard-modifier snapshot so a widget
                // can read Ctrl/Shift off the synthesized event (e.g. Ctrl+wheel).
                self.event_dispatcher
                    .dispatch_scroll_with_modifiers(pos, dx, dy, modifiers, hit_test);
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

        let modifiers = self.keyboard_modifiers;
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
                // Seam-2: thread the live keyboard-modifier snapshot through the
                // pointer path so a widget reads Ctrl/Shift off the synthesized
                // Click for multi-select (the value matches what `dispatch_key_*`
                // already feeds the DOM key path).
                match state {
                    ButtonState::Pressed => {
                        self.event_dispatcher.dispatch_mouse_down_with_modifiers(
                            pos,
                            dom_btn,
                            modifiers,
                            &mut self.desktop_dom.doc,
                            hit_test,
                        );
                    }
                    ButtonState::Released => {
                        self.event_dispatcher.dispatch_mouse_up_with_modifiers(
                            pos,
                            dom_btn,
                            modifiers,
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

    /// Compute the on-screen bounds of the context menu panel (matching the
    /// hover hit-test geometry in [`Self::handle_platform_event`]). `None` when
    /// the context menu is not visible.
    pub(crate) fn context_menu_bounds(&self) -> Option<Rect> {
        if !self.context_menu_visible {
            return None;
        }
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
        Some(Rect::new(ctx_x, ctx_y, context_menu_width, ctx_h))
    }

    /// Whether any pop-up menu overlay (context / session / app menu) is open.
    #[must_use]
    pub fn any_menu_open(&self) -> bool {
        self.context_menu_visible || self.session_menu_visible || self.app_menu_open.is_some()
    }

    /// Upper-bound damage region for a hover interaction **while a pop-up menu is
    /// open** — the proven residual-lag / stale-pixel scenario (t79 Bug 1 & Bug
    /// 2 #1). Returns `None` when no menu is open, so the caller keeps its own
    /// conservative damage determination for ordinary hovers (a plain hover can
    /// surface a tooltip or other unbounded chrome we deliberately do not try to
    /// bound here).
    ///
    /// Returned in logical (CSS-pixel) layout coordinates — the same space the
    /// renderer rasters in (the shell's `screen_rect` is sized in physical
    /// pixels by `resize_screen`, matching the session's damage grid). This is a
    /// deliberate **superset** of what a hover can change while a menu is open:
    /// the whole menu panel (so any item-hover background flip lands inside it),
    /// the dock band when shown (dock hover/badge), and a hovered window
    /// titlebar (decoration-button hover). Each region is expanded by
    /// [`OVERLAY_BACKDROP_MARGIN`] to also cover the menu's `backdrop-filter`
    /// blur halo, which samples — and therefore repaints — pixels just outside
    /// the panel rect.
    ///
    /// A consumer must treat this as an authoritative LOWER bound on the damage
    /// set (mark EVERY returned rect; UNION with any scene diff; never narrow
    /// past it), otherwise stale pixels can be left behind.
    ///
    /// Returns a SET of disjoint regions rather than a single bounding box: a
    /// top-of-screen menu and the bottom dock band would, if merged into one
    /// bbox, cover almost the whole screen and defeat the optimization. Each
    /// region is damaged independently so the empty middle is never repainted.
    /// Empty `Vec` means no menu is open and the caller should keep its own
    /// damage determination.
    #[must_use]
    pub fn interactive_overlay_damage(&self) -> Vec<Rect> {
        /// Margin (logical px) added around each overlay region to cover the
        /// `backdrop-filter: blur(var(--blur-strong))` halo that samples
        /// neighbouring pixels. Generously larger than the strong-blur radius so
        /// the hint can never be narrower than the actually-repainted region.
        const OVERLAY_BACKDROP_MARGIN: f32 = 48.0;

        // Only engage while a menu is open. Without an open menu a hover can
        // change unbounded chrome (e.g. a tooltip popping up anywhere), which
        // this targeted hint does not cover — so we leave those frames on the
        // caller's existing full-frame path.
        if !self.any_menu_open() {
            return Vec::new();
        }

        let mut rects: Vec<Rect> = Vec::new();
        let mut add = |rect: Rect| rects.push(rect.expand(OVERLAY_BACKDROP_MARGIN));

        if let Some(bounds) = self.context_menu_bounds() {
            add(bounds);
        }
        if self.session_menu_visible {
            add(self.session_menu_bounds());
        }
        if self.app_menu_open.is_some() {
            // Item count mirrors `handle_platform_event`'s app-menu hover math.
            const APP_MENU_ITEMS: usize = 5;
            if let Some(bounds) = self.app_menu_bounds(APP_MENU_ITEMS) {
                add(bounds);
            }
        }
        if self.dock.is_visible() {
            add(self.dock.compute_bounds(self.screen_rect));
        }
        // A hovered window-decoration button (close/maximize/minimize/pin)
        // repaints in the window's title bar on hover. Include that band so a
        // titlebar-button hover-highlight under an open menu is not under-damaged.
        if let Some((window_id, _zone)) = self.hovered_button {
            if let Some(window) = self.windows.get(&window_id) {
                let tbh = self.decoration_style.title_bar_height;
                add(Rect::new(
                    window.bounds.x,
                    window.bounds.y,
                    window.bounds.width,
                    tbh,
                ));
            }
        }

        rects
    }

    /// Margin (logical px) added around the dragged window's footprint to cover
    /// its drop-shadow / glass halo / decoration so the confined drag damage is
    /// a true SUPERSET of every pixel the move touches. Mirrors the
    /// `OVERLAY_BACKDROP_MARGIN` precedent used by [`Self::interactive_overlay_damage`]
    /// (and `BACKDROP_MARGIN` in `scene.rs`).
    pub const DRAG_FOOTPRINT_MARGIN: f32 = 48.0;

    /// Begin a window MOVE drag programmatically (the same `DragState::Moving`
    /// that a title-bar press installs). `grab` is the pointer position at grab
    /// time; the offset is recorded so subsequent `MouseEvent::Move`s relocate
    /// the window under the cursor. Returns `false` if the window is unknown.
    ///
    /// Exposed so the session-level drag-damage plumbing can be exercised
    /// end-to-end without reconstructing a CSS title-bar hit-test.
    pub fn begin_move_drag(&mut self, window_id: WindowId, grab: Point) -> bool {
        let Some(window) = self.windows.get(&window_id) else {
            return false;
        };
        self.drag_state = Some(DragState::Moving {
            window_id,
            offset_x: grab.x - window.bounds.x,
            offset_y: grab.y - window.bounds.y,
        });
        true
    }

    /// Targeted damage for a window MOVE drag-frame: the union of the dragged
    /// window's OLD footprint (where it was before this move) and its NEW
    /// footprint (where it is now), each expanded by [`Self::DRAG_FOOTPRINT_MARGIN`]
    /// to cover shadow/blur/decoration.
    ///
    /// `old_bounds` is the dragged window's `bounds` captured BEFORE the move
    /// event was handled (the caller snapshots it in `dispatch_platform_event`,
    /// symmetric with the overlay-damage before/after capture). The OLD rect MUST
    /// be in the union or the window's previous position is never repainted and
    /// leaves a stale ghost (the smear/disappear class).
    ///
    /// Returns a disjoint SET (two rects, unioned downstream into the
    /// `DamageSet`), NOT a single bbox: a long fling leaves a wide gap between
    /// old and new and merging them would re-raster the untouched middle.
    ///
    /// Returns an EMPTY `Vec` when this is not a window move-drag, or when the
    /// dragged window's current bounds are unavailable — the caller then keeps
    /// its conservative full-frame path (no regression / no under-damage).
    #[must_use]
    pub fn drag_move_damage(&self, old_bounds: Rect) -> Vec<Rect> {
        let Some(window_id) = self.dragged_window() else {
            return Vec::new();
        };
        // Only window MOVE drags are confined here; resize drags keep the
        // existing full-frame path (follow-up).
        if !matches!(self.drag_state, Some(DragState::Moving { .. })) {
            return Vec::new();
        }
        let Some(window) = self.windows.get(&window_id) else {
            return Vec::new();
        };
        let new_bounds = window.bounds;
        vec![
            old_bounds.expand(Self::DRAG_FOOTPRINT_MARGIN),
            new_bounds.expand(Self::DRAG_FOOTPRINT_MARGIN),
        ]
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
                // Seam-2: keep the live keyboard-modifier snapshot current so the
                // pointer path (`dispatch_*_with_modifiers`) can read Ctrl/Shift
                // for widget multi-select. Same opaque `u32` the DOM key path uses.
                self.keyboard_modifiers = ke.modifiers.bits() as u32;
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

                // Modal grab (t94-e4 gap #5b): while a modal owns input, keys
                // must not activate or switch background windows. The DOM/app
                // key dispatch above already gave the modal overlay (and any
                // `preventDefault` it wants) its chance; here we swallow the
                // remainder so Alt-Tab window switching, the text-input seam to
                // a background window, and global shortcuts cannot fire behind
                // the modal. Respects the nested-modal stack.
                if self.has_active_modal() {
                    return None;
                }

                // Input-method step (t73-input §1): drive the IME engine BEFORE
                // the text-input seam so CJK / accent / emoji composition works.
                // The engine is inactive by default (Direct mode → Forward), so
                // an ASCII-input session falls straight through with no behavior
                // change; it only intercepts once activated (Ctrl+Space) or
                // switched to a composing mode. A committed string is routed into
                // the focused window exactly like typed text; a consumed
                // (preedit/candidate) key requests a redraw and stops here.
                match self.drive_input_method(ke) {
                    ImeOutcome::Commit(text) => {
                        if let Some(wid) = self.focus.focused() {
                            if self.windows.contains_key(&wid) {
                                for ch in text.chars() {
                                    self.route_char_to_focused_app(wid, ch);
                                }
                                return Some(ShellAction::Redraw);
                            }
                        }
                        return Some(ShellAction::Redraw);
                    }
                    ImeOutcome::Consumed => return Some(ShellAction::Redraw),
                    ImeOutcome::Forward => {}
                }

                // Widget-host keyboard seam (t108-p8 Seam-2): when the focused
                // window is widget-backed AND a widget owns DOM focus (the host's
                // focused slot), route the key into the host (queued for the
                // per-frame drive's `on_keyboard`) instead of the app text path.
                // The focused window's host owns DOM focus while a widget is
                // focused, so this composes with the existing shell DOM focus.
                if let Some(wid) = self.focus.focused() {
                    let widget_focused = self
                        .app_widget_hosts
                        .get(&wid)
                        .map_or(false, |h| h.focused().is_some());
                    if widget_focused {
                        if let Some(key) =
                            Self::keycode_to_widget_key(ke.key, ke.modifiers)
                        {
                            self.pending_widget_keys.push(key);
                            return Some(ShellAction::Redraw);
                        }
                    }
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

        // Decoration button hover detection. Resolve the topmost window at the
        // cursor through the SINGLE canonical router (t93-e3) so hover highlights
        // the same window a click would pick — a button-hover on a window that is
        // actually occluded at this point can no longer fire. Only that window's
        // own title-bar decoration is then hit-tested.
        let prev_hover = self.hovered_button;
        self.hovered_button = None;
        let tbh = self.decoration_style.title_bar_height;
        // Resolve the topmost window through the SAME canonical router the click
        // path uses (`pick_window_at`, incl. the off-edge resize ring) so a hover
        // and a click agree on which window — and so a button laid out a hair
        // beyond the window's exact bounds (CSS flex overflow) is still reached.
        if let Some(wid) = self.pick_window_at(x, y) {
            let is_decorated = self
                .windows
                .get(&wid)
                .map(|w| w.flags.contains(WindowFlags::DECORATED))
                .unwrap_or(false);
            if is_decorated {
                // Button hover from the LAID-OUT CSS boxes (t103-p6 / t86): the
                // CSS button box IS the hover zone (no rect-math window-bounds
                // gate — that gate could exclude a button whose CSS box overflows
                // the window edge). Fall back to the rect-based hit-test only when
                // the decoration is not laid out yet (first frame).
                let zone = self.window_button_zone_from_css(wid, x, y).or_else(|| {
                    self.windows.get(&wid).map(|window| {
                        let client = Rect::new(
                            window.bounds.x,
                            window.bounds.y + tbh,
                            window.bounds.width,
                            (window.bounds.height - tbh).max(0.0),
                        );
                        hit_test_decoration(client, &self.decoration_style, x, y)
                    })
                });
                match zone {
                    Some(HitZone::CloseButton)
                    | Some(HitZone::MaximizeButton)
                    | Some(HitZone::MinimizeButton)
                    | Some(HitZone::AlwaysOnTopButton) => {
                        self.hovered_button = Some((wid, zone.unwrap()));
                    }
                    _ => {}
                }
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
        if let Some(ctx_bounds) = self.context_menu_bounds() {
            let menu_padding = self.menu_padding();
            let menu_item_height = self.menu_item_height();
            let ctx_items = ContextMenuItem::defaults();
            let ctx_y = ctx_bounds.y;
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
        } else if let Some(wid) = self.pick_window_at(x, y) {
            // Resolve the cursor shape from the SAME canonical window the click
            // path would pick (t93-e3): `pick_window_at` resolves the topmost
            // window including its off-edge resize ring, and we hit-test only
            // that window's decoration — so the resize cursor and the resize
            // grab always agree on which window is being targeted.
            if let Some(window) = self.windows.get(&wid) {
                if window.flags.contains(WindowFlags::DECORATED) {
                    let client = Rect::new(
                        window.bounds.x,
                        window.bounds.y + tbh,
                        window.bounds.width,
                        (window.bounds.height - tbh).max(0.0),
                    );
                    match hit_test_decoration(client, &self.decoration_style, x, y) {
                        HitZone::Outside | HitZone::TitleBar | HitZone::Client => {}
                        zone => {
                            self.cursor_shape = Self::cursor_for_hit_zone(zone);
                        }
                    }
                }
            }
        }
        if self.cursor_shape != prev_cursor {
            need_redraw = true;
        }

        // Focus-follows-mouse (t94-e4 gap #5a). Opt-in via the focus policy —
        // click-to-focus is the DEFAULT (FocusManager is constructed with
        // FocusPolicy::ClickToFocus), so this block is inert unless the policy
        // is switched to FocusFollowsMouse. When enabled, a pointer move that
        // crosses into a *different* window focuses that window WITHOUT raising
        // it (classic FFM: focus tracks the pointer; auto-raise stays a
        // click-only behavior). Thrash guards:
        //   • an active drag/resize already returned above, so we never refocus
        //     mid-drag;
        //   • a move that stays over the same window is a no-op (only a genuine
        //     change calls set_focus);
        //   • while a modal grab is active, FFM is suppressed entirely so input
        //     stays with the modal.
        // Hit-testing goes through the SINGLE canonical tree router
        // (`window_at_point`, t93-e3) — no flat z-scan is reintroduced. We use
        // the exact-bounds `window_at_point` (not the resize-ring `pick_window_at`)
        // so the off-frame resize tolerance does not bleed focus to a window the
        // pointer is not actually over.
        if self.focus.policy() == FocusPolicy::FocusFollowsMouse && !self.has_active_modal() {
            let target = self.window_at_point(x, y);
            if let Some(wid) = target {
                if self.focus.focused() != Some(wid) {
                    let _ = self.set_focus(wid);
                    need_redraw = true;
                }
            }
        }

        if need_redraw {
            Some(ShellAction::Redraw)
        } else {
            None
        }
    }

    /// The active focus policy (t94-e4 gap #5a). Defaults to
    /// [`FocusPolicy::ClickToFocus`].
    #[must_use]
    pub fn focus_policy(&self) -> FocusPolicy {
        self.focus.policy()
    }

    /// Set the focus policy (t94-e4 gap #5a). Opt in to focus-follows-mouse via
    /// `FocusPolicy::FocusFollowsMouse`; the default is click-to-focus. This is
    /// the config/consumer entry point for the FFM behavior wired into
    /// [`Self::handle_mouse_move`].
    pub fn set_focus_policy(&mut self, policy: FocusPolicy) {
        self.focus.set_policy(policy);
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

    /// Map a platform `KeyCode` + `Modifiers` to a `liquide_widgets::KeyInput`
    /// for routing into a focused widget host (t108-p8 Seam-2). Mirrors
    /// [`Self::keycode_to_app_key`] but targets the toolkit's self-contained key
    /// encoding (`liquide_widgets::keys`): printable keys become their Unicode
    /// codepoint (shift uppercases letters), named keys map to the toolkit's
    /// high-range constants, and the modifier bits are translated into the
    /// toolkit's modifier bit layout. Returns `None` for keys the toolkit has no
    /// encoding for, so they fall through to the shell's own handling.
    fn keycode_to_widget_key(
        key: liquide_input::keyboard::KeyCode,
        modifiers: liquide_input::keyboard::Modifiers,
    ) -> Option<liquide_widgets::KeyInput> {
        use liquide_input::keyboard::KeyCode;
        use liquide_widgets::keys;

        // Toolkit modifier bit layout (matches `liquide_widgets::keys::modifiers`).
        let mut mods = 0u32;
        if modifiers.shift() {
            mods |= keys::modifiers::SHIFT;
        }
        if modifiers.ctrl() {
            mods |= keys::modifiers::CTRL;
        }
        if modifiers.alt() {
            mods |= keys::modifiers::ALT;
        }
        if modifiers.super_key() {
            mods |= keys::modifiers::SUPER;
        }

        let code = match key {
            KeyCode::Enter => keys::ENTER,
            KeyCode::Tab => keys::TAB,
            KeyCode::Backspace => keys::BACKSPACE,
            KeyCode::Delete => keys::DELETE,
            KeyCode::Escape => keys::ESCAPE,
            KeyCode::ArrowLeft => keys::ARROW_LEFT,
            KeyCode::ArrowRight => keys::ARROW_RIGHT,
            KeyCode::ArrowUp => keys::ARROW_UP,
            KeyCode::ArrowDown => keys::ARROW_DOWN,
            KeyCode::Home => keys::HOME,
            KeyCode::End => keys::END,
            KeyCode::PageUp => keys::PAGE_UP,
            KeyCode::PageDown => keys::PAGE_DOWN,
            other => {
                let ch = Self::keycode_to_char(other)?;
                // Shift uppercases ASCII letters for the inserted character; other
                // shifted symbols keep their base codepoint (the toolkit reads the
                // SHIFT modifier bit for non-text semantics like range-select).
                let ch = if modifiers.shift() && ch.is_ascii_alphabetic() {
                    ch.to_ascii_uppercase()
                } else {
                    ch
                };
                ch as u32
            }
        };
        Some(liquide_widgets::KeyInput::new(code, mods))
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
                    // Re-resolve the window's monitor from its dropped position
                    // (t73-multimon §3.3). No-op when no layout is installed.
                    self.assign_window_to_monitor(window_id);
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

        // Lock screen (t95-p4). When the session is locked the DOM/CSS lock
        // overlay is topmost and modal: every press is consumed here so it
        // cannot leak to windows/chrome behind the scrim. A press inside the
        // CSS-laid-out password field box focuses it (Clock → PasswordEntry)
        // via the canonical lock-screen logic — the hit-test geometry comes
        // from the laid-out `#lockscreen-password` box, NOT a hardcoded
        // constant (t86 hit-test-from-CSS-geometry contract).
        if self.is_session_locked() {
            if button == MouseButton::Left {
                self.lockscreen_press(x, y);
            }
            return Some(ShellAction::Redraw);
        }

        // Overview / exposé (t101-p5 full-CSS migration). While the overview is
        // open it is topmost and modal: every press is consumed here so it
        // cannot leak to windows/chrome behind the scrim. A left press inside a
        // tile's CSS-laid-out box focuses + raises that window and closes the
        // overview; an empty-scrim press dismisses it. The picked window comes
        // from the laid-out `#overview-tile-<id>` box (see `overview_adapter`),
        // NOT hardcoded grid geometry — the t86 hit-test-from-CSS contract.
        if self.overview_visible {
            if button == MouseButton::Left {
                return Some(self.overview_press(x, y));
            }
            return Some(ShellAction::Redraw);
        }

        // Modal grab (t94-e4 gap #5b). While a modal dialog/window owns input,
        // a press anywhere outside the modal surface must NOT focus, raise, or
        // start a drag on any background window, nor open the desktop/window
        // context menus. The modal surface itself (the `dialog-overlay` and its
        // buttons) is a DOM/CSS overlay handled by `dispatch_dom_mouse_event`
        // (run before this handler in `handle_platform_event`), so it keeps
        // receiving its clicks; here we simply swallow the press so it cannot
        // leak through the scrim to the windows behind it. The grab respects the
        // modal STACK (nested modals): it stays in effect until every modal is
        // dismissed. We keep the scrim up by requesting a redraw (a v1 stand-in
        // for an optional bell/flash).
        if self.has_active_modal() {
            return Some(ShellAction::Redraw);
        }

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
            // Resolve the topmost window under the cursor through the SINGLE
            // canonical router (t93-e3) — same source of truth the left-click /
            // hover paths use — then refine: a press on its title bar opens the
            // app menu, a press elsewhere on it is consumed, and a press on no
            // window opens the desktop context menu.
            let tbh = self.decoration_style.title_bar_height;
            let on_window = self.window_at_point(x, y);
            let titlebar_window = on_window.filter(|wid| {
                self.windows.get(wid).is_some_and(|w| {
                    w.flags.contains(WindowFlags::DECORATED)
                        && Rect::new(w.bounds.x, w.bounds.y, w.bounds.width, tbh).contains(pt)
                })
            });
            if let Some(wid) = titlebar_window {
                // Show the app menu (Minimize/Maximize/Close) instead of generic context menu
                let win_id_str = format!("window-{}", wid.0);
                self.app_menu_open = Some(win_id_str);
                self.app_menu_hover_index = Some(0);
                self.context_menu_visible = false;
                self.context_menu_hover_index = None;
                return Some(ShellAction::Redraw);
            }
            if on_window.is_none() {
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
        if let Some(ctx_bounds) = self.context_menu_bounds() {
            let menu_padding = self.menu_padding();
            let menu_item_height = self.menu_item_height();
            let ctx_items = ContextMenuItem::defaults();
            let ctx_y = ctx_bounds.y;
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

        // Window click. Route through the SINGLE canonical window hit-test
        // (`pick_window_at`, t93-e3): the tree-routed `window_at_point` resolves
        // the topmost window honoring z-order + the always-on-top band +
        // child-over-parent + visibility, and an off-edge resize-ring fallback
        // (same band order as paint) widens the hit area by `resize_tolerance`
        // so a grab just outside a window's frame still starts a resize. This
        // retires the old flat z-scan that duplicated (and could diverge from)
        // the canonical tree pick.
        let tbh = self.decoration_style.title_bar_height;
        let clicked = self.pick_window_at(x, y);

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
                // Buttons + titlebar drag come from the LAID-OUT CSS boxes
                // (t103-p6 / t86 hit-test-from-CSS contract): a theme change
                // that moves a button moves its click zone. Resize-edge zones
                // extend outside the DOM box and have no CSS element, so they —
                // and the first frame before layout — fall back to the
                // rect-based `hit_test_decoration`.
                //
                // PRECEDENCE (t115-titlebar fix): the CSS `window-titlebar` box
                // spans the whole title row, so `window_decoration_zone_from_css`
                // returns `TitleBar` even for points inside the resize-CORNER
                // tolerance at the titlebar's top-left/top-right (the corners the
                // rect model treats as `ResizeTopLeft`/`ResizeTopRight`). The CSS
                // adapter knows nothing about resize edges, so before the P6
                // migration these corners started a resize; afterwards the CSS
                // `TitleBar` shadowed them and they started a MOVE instead — i.e.
                // a resizable window could no longer be grabbed for resize at its
                // top corners. Fix: a rect-based resize zone takes precedence over
                // a CSS `TitleBar` (but NEVER over a CSS button — you don't resize
                // from the close button). So: CSS button > rect resize edge/corner
                // > CSS titlebar/zone > rect fallback.
                let css_zone = self.window_decoration_zone_from_css(wid, x, y);
                let rect_zone = hit_test_decoration(client, &self.decoration_style, x, y);
                let zone = match css_zone {
                    // A CSS button always wins (resize never overrides a button).
                    Some(
                        z @ (HitZone::CloseButton
                        | HitZone::MaximizeButton
                        | HitZone::MinimizeButton
                        | HitZone::AlwaysOnTopButton),
                    ) => z,
                    // CSS says titlebar drag — but if the rect model says this
                    // point is actually a resize edge/corner AND the window is
                    // resizable, prefer the resize (the CSS titlebar box overlaps
                    // the corner tolerance). For a non-resizable window the corner
                    // stays a drag (TitleBar), as before.
                    Some(HitZone::TitleBar) if is_resizable && rect_zone.is_resize() => rect_zone,
                    Some(z) => z,
                    // Decoration not laid out yet (first frame): rect fallback,
                    // which also owns the resize-edge zones outside the DOM box.
                    None => rect_zone,
                };
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
