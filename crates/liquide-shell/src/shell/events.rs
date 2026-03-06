//! Mouse and keyboard event handling, click dispatch, DOM event forwarding.

use liquide_compositor::geometry::{Point, Rect};
use liquide_compositor::scene::CursorShape;
use liquide_input::mouse::{ButtonState, MouseButton, MouseEvent};
use liquide_platform::PlatformEvent;

use crate::decoration::{HitZone, hit_test_decoration};
use crate::launcher::SearchResultKind;
use crate::shortcuts::ShellAction;
use crate::window::{WindowFlags, WindowState};
use liquide_hit_test::event::{DomEventKind, MouseButton as DomMouseButton};
use liquide_hit_test::EventDispatcher;

use super::{ContextMenuItem, DragState, Shell};

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
                        if layout.get(box_id).map_or(false, |b| b.scroll_size.is_some()) {
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
            MouseEvent::Button { x, y, button, state } => {
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
                            pos, dom_btn, &mut self.desktop_dom.doc, hit_test,
                        );
                    }
                    ButtonState::Released => {
                        self.event_dispatcher.dispatch_mouse_up(
                            pos, dom_btn, &mut self.desktop_dom.doc, hit_test,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    /// Register an event handler on a DOM node.
    pub fn add_event_handler(
        &mut self,
        node: liquide_dom::NodeId,
        kind_filter: Option<DomEventKind>,
        handler: liquide_hit_test::dispatch::EventHandler,
    ) {
        self.event_dispatcher.add_handler(node, kind_filter, handler);
    }

    /// Get a reference to the DOM event dispatcher.
    pub fn event_dispatcher(&self) -> &EventDispatcher {
        &self.event_dispatcher
    }

    /// Get a mutable reference to the DOM event dispatcher.
    pub fn event_dispatcher_mut(&mut self) -> &mut EventDispatcher {
        &mut self.event_dispatcher
    }

    /// Handle a platform event and return any resulting shell action.
    pub fn handle_platform_event(&mut self, event: &PlatformEvent) -> Option<ShellAction> {
        use liquide_input::keyboard::{KeyCode, KeyState};

        match event {
            PlatformEvent::KeyInput { event: ke, .. } => {
                if ke.state != KeyState::Pressed {
                    return None;
                }
                if self.launcher.is_visible() {
                    match ke.key {
                        KeyCode::Escape => { self.launcher.close(); return Some(ShellAction::Redraw); }
                        KeyCode::ArrowUp => { self.launcher.select_prev(); return Some(ShellAction::Redraw); }
                        KeyCode::ArrowDown => { self.launcher.select_next(); return Some(ShellAction::Redraw); }
                        KeyCode::Enter => {
                            if let Some(kind) = self.launcher.activate_selected().cloned() {
                                self.launcher.close();
                                if let SearchResultKind::Application { ref app_id } = kind {
                                    self.open_app_window(app_id);
                                }
                            } else {
                                self.launcher.close();
                            }
                            return Some(ShellAction::Redraw);
                        }
                        KeyCode::Backspace => {
                            let q = self.launcher.query().to_string();
                            if !q.is_empty() {
                                let new_q = if let Some((idx, _)) = q.char_indices().last() { &q[..idx] } else { "" };
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
                if self.context_menu_visible && ke.key == KeyCode::Escape {
                    self.context_menu_visible = false;
                    return Some(ShellAction::Redraw);
                }
                if self.session_menu_visible && ke.key == KeyCode::Escape {
                    self.session_menu_visible = false;
                    return Some(ShellAction::Redraw);
                }
                self.shortcuts.handle_key_event(ke).cloned()
            }
            PlatformEvent::MouseInput { event: me, .. } => {
                self.dispatch_dom_mouse_event(me);
                match me {
                    MouseEvent::Move { x, y } => self.handle_mouse_move(*x, *y),
                    MouseEvent::Button { button, state, x, y } => {
                        self.handle_mouse_button(*button, *state, *x, *y)
                    }
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

        // Active drag handling
        if let Some(drag) = self.drag_state {
            match drag {
                DragState::Moving { window_id, offset_x, offset_y } => {
                    self.cursor_shape = CursorShape::Move;
                    if let Some(window) = self.windows.get_mut(&window_id) {
                        window.bounds.x = x - offset_x;
                        window.bounds.y = y - offset_y;
                        if window.state == WindowState::Maximized {
                            window.state = WindowState::Normal;
                        }
                    }
                    return Some(ShellAction::Redraw);
                }
                DragState::Resizing { window_id, edge, start_bounds, start_x, start_y } => {
                    self.cursor_shape = Self::cursor_for_hit_zone(edge);
                    let dx = x - start_x;
                    let dy = y - start_y;
                    let min_w = self.windows.get(&window_id).and_then(|w| w.min_size).map(|(mw, _)| mw).unwrap_or(120.0);
                    let min_h = self.windows.get(&window_id).and_then(|w| w.min_size).map(|(_, mh)| mh).unwrap_or(80.0);
                    if let Some(window) = self.windows.get_mut(&window_id) {
                        match edge {
                            HitZone::ResizeRight => { window.bounds.width = (start_bounds.width + dx).max(min_w); }
                            HitZone::ResizeBottom => { window.bounds.height = (start_bounds.height + dy).max(min_h); }
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
            if !window.flags.contains(WindowFlags::DECORATED) { continue; }
            if y >= window.bounds.y && y < window.bounds.y + tbh
                && x >= window.bounds.x && x < window.bounds.x + window.bounds.width
            {
                let client = Rect::new(window.bounds.x, window.bounds.y + tbh, window.bounds.width, (window.bounds.height - tbh).max(0.0));
                let zone = hit_test_decoration(client, &self.decoration_style, x, y);
                match zone {
                    HitZone::CloseButton | HitZone::MaximizeButton | HitZone::MinimizeButton | HitZone::AlwaysOnTopButton => {
                        self.hovered_button = Some((window.id, zone));
                    }
                    _ => {}
                }
                break;
            }
        }
        if self.hovered_button != prev_hover { need_redraw = true; }

        // Dock hover
        let dock_bounds = self.dock.compute_bounds(self.screen_rect);
        if dock_bounds.contains(pt) {
            let item_rects = self.dock.compute_item_rects(self.screen_rect);
            let mut found = None;
            for (i, (_, rect)) in item_rects.iter().enumerate() {
                if rect.contains(pt) { found = Some(i); break; }
            }
            let prev = self.dock.hover_index();
            if let Some(idx) = found { self.dock.on_hover(idx); } else { self.dock.on_hover_leave(); }
            if self.dock.hover_index() != prev { need_redraw = true; }
        } else {
            if self.dock.hover_index().is_some() { need_redraw = true; }
            self.dock.on_hover_leave();
        }

        // Context menu hover
        if self.context_menu_visible {
            let ctx_items = ContextMenuItem::defaults();
            let ctx_item_h = 36.0_f32;
            let ctx_w = 260.0_f32;
            let ctx_h = 16.0 + ctx_items.len() as f32 * ctx_item_h;
            let ctx_x = self.context_menu_pos.x.min(self.screen_rect.width - ctx_w - 4.0).max(0.0);
            let ctx_y = self.context_menu_pos.y.min(self.screen_rect.height - ctx_h - 4.0).max(0.0);
            let ctx_bounds = Rect::new(ctx_x, ctx_y, ctx_w, ctx_h);
            let prev_hover = self.context_menu_hover_index;
            if ctx_bounds.contains(pt) {
                let rel_y = y - ctx_y - 8.0;
                if rel_y >= 0.0 {
                    let idx = (rel_y / ctx_item_h) as usize;
                    self.context_menu_hover_index = if idx < ctx_items.len() { Some(idx) } else { None };
                } else { self.context_menu_hover_index = None; }
            } else { self.context_menu_hover_index = None; }
            if self.context_menu_hover_index != prev_hover { need_redraw = true; }
        }

        // Session menu hover
        if self.session_menu_visible {
            let item_h = 36.0_f32;
            let menu_w = 180.0_f32;
            let menu_h = 16.0 + self.session_menu_items.len() as f32 * item_h;
            let bar_h = self.status_bar.config().height;
            let menu_x = self.screen_rect.width - menu_w - 8.0;
            let menu_y = bar_h + 4.0;
            let menu_bounds = Rect::new(menu_x, menu_y, menu_w, menu_h);
            let prev_hover = self.session_menu_hover_index;
            if menu_bounds.contains(pt) {
                let rel_y = y - menu_y - 8.0;
                if rel_y >= 0.0 {
                    let idx = (rel_y / item_h) as usize;
                    self.session_menu_hover_index = if idx < self.session_menu_items.len() { Some(idx) } else { None };
                } else { self.session_menu_hover_index = None; }
            } else { self.session_menu_hover_index = None; }
            if self.session_menu_hover_index != prev_hover { need_redraw = true; }
        }

        // Cursor shape determination
        let prev_cursor = self.cursor_shape;
        self.cursor_shape = CursorShape::Arrow;
        if self.dock.hover_index().is_some() {
            self.cursor_shape = CursorShape::Pointer;
        } else if self.context_menu_hover_index.is_some() || self.session_menu_hover_index.is_some() {
            self.cursor_shape = CursorShape::Pointer;
        } else if self.hovered_button.is_some() {
            self.cursor_shape = CursorShape::Pointer;
        } else {
            for window in self.visible_windows().into_iter().rev() {
                if !window.flags.contains(WindowFlags::DECORATED) { continue; }
                let client = Rect::new(window.bounds.x, window.bounds.y + tbh, window.bounds.width, (window.bounds.height - tbh).max(0.0));
                let zone = hit_test_decoration(client, &self.decoration_style, x, y);
                match zone {
                    HitZone::Outside => continue,
                    HitZone::TitleBar | HitZone::Client => break,
                    zone => { self.cursor_shape = Self::cursor_for_hit_zone(zone); break; }
                }
            }
        }
        if self.cursor_shape != prev_cursor { need_redraw = true; }

        if need_redraw { Some(ShellAction::Redraw) } else { None }
    }

    fn handle_mouse_button(&mut self, button: MouseButton, state: ButtonState, x: f32, y: f32) -> Option<ShellAction> {
        if state == ButtonState::Released {
            if self.drag_state.is_some() {
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
            let bar_bounds = self.status_bar.compute_bounds(self.screen_rect);
            let dock_bounds = self.dock.compute_bounds(self.screen_rect);
            if bar_bounds.contains(pt) {
                self.context_menu_visible = !self.context_menu_visible;
                self.context_menu_pos = pt;
                return Some(ShellAction::Redraw);
            }
            if dock_bounds.contains(pt) {
                self.context_menu_visible = !self.context_menu_visible;
                self.context_menu_pos = pt;
                return Some(ShellAction::Redraw);
            }
            let tbh = self.decoration_style.title_bar_height;
            let on_titlebar = self.visible_windows().iter().rev().any(|w| {
                let title_rect = Rect::new(w.bounds.x, w.bounds.y, w.bounds.width, tbh);
                title_rect.contains(pt) && w.flags.contains(WindowFlags::DECORATED)
            });
            if on_titlebar {
                self.context_menu_visible = !self.context_menu_visible;
                self.context_menu_pos = pt;
                return Some(ShellAction::Redraw);
            }
            let on_window = self.visible_windows().iter().rev().any(|w| w.bounds.contains(pt));
            if !on_window {
                self.context_menu_visible = !self.context_menu_visible;
                self.context_menu_pos = pt;
                return Some(ShellAction::Redraw);
            }
            return None;
        }

        if button != MouseButton::Left { return None; }

        // Context menu click
        if self.context_menu_visible {
            let ctx_items = ContextMenuItem::defaults();
            let ctx_item_h = 36.0_f32;
            let ctx_w = 260.0_f32;
            let ctx_h = 16.0 + ctx_items.len() as f32 * ctx_item_h;
            let ctx_x = self.context_menu_pos.x.min(self.screen_rect.width - ctx_w - 4.0).max(0.0);
            let ctx_y = self.context_menu_pos.y.min(self.screen_rect.height - ctx_h - 4.0).max(0.0);
            let ctx_bounds = Rect::new(ctx_x, ctx_y, ctx_w, ctx_h);
            if ctx_bounds.contains(pt) {
                let rel_y = y - ctx_y - 8.0;
                let idx = (rel_y / ctx_item_h) as usize;
                self.context_menu_visible = false;
                if idx < ctx_items.len() { return Some(ctx_items[idx].action.clone()); }
                return None;
            }
            self.context_menu_visible = false;
        }

        // Session menu click
        if self.session_menu_visible {
            let menu_w = 180.0_f32;
            let item_h = 36.0_f32;
            let menu_h = 16.0 + self.session_menu_items.len() as f32 * item_h;
            let bar_h = self.status_bar.config().height;
            let menu_x = self.screen_rect.width - menu_w - 8.0;
            let menu_y = bar_h + 4.0;
            let menu_bounds = Rect::new(menu_x, menu_y, menu_w, menu_h);
            if menu_bounds.contains(pt) {
                let rel_y = y - menu_y - 8.0;
                let idx = (rel_y / item_h) as usize;
                self.session_menu_visible = false;
                if idx < self.session_menu_items.len() { return Some(self.session_menu_items[idx].action.clone()); }
                return None;
            }
            self.session_menu_visible = false;
        }

        // Launcher click
        if self.launcher.is_visible() {
            let screen = self.screen_rect;
            let panel_w = screen.width * 0.6;
            let panel_h = screen.height * 0.7;
            let panel_x = screen.x + (screen.width - panel_w) / 2.0;
            let panel_y = screen.y + (screen.height - panel_h) / 2.0;
            let panel_bounds = Rect::new(panel_x, panel_y, panel_w, panel_h);
            if !panel_bounds.contains(pt) {
                self.launcher.close();
                return Some(ShellAction::Redraw);
            }
            let item_start_y = panel_y + 65.0;
            let item_height = 40.0_f32;
            let item_gap = 4.0_f32;
            if y >= item_start_y {
                let rel_y = y - item_start_y;
                let idx = (rel_y / (item_height + item_gap)) as usize;
                self.launcher.select_index(idx);
                if let Some(kind) = self.launcher.activate_selected().cloned() {
                    self.launcher.close();
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
            let session_x = self.screen_rect.width - 36.0;
            if x >= session_x {
                self.session_menu_visible = !self.session_menu_visible;
                return Some(ShellAction::OpenSessionMenu);
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

        // Window click with decoration hit-testing
        let mut clicked = None;
        let tbh = self.decoration_style.title_bar_height;
        for window in self.visible_windows().into_iter().rev() {
            let bw = self.decoration_style.border_width;
            let expanded = Rect::new(
                window.bounds.x - bw, window.bounds.y - bw,
                window.bounds.width + bw * 2.0, window.bounds.height + bw * 2.0,
            );
            if expanded.contains(pt) { clicked = Some(window.id); break; }
        }

        if let Some(wid) = clicked {
            let is_decorated = self.windows.get(&wid).map(|w| w.flags.contains(WindowFlags::DECORATED)).unwrap_or(false);
            let is_resizable = self.windows.get(&wid).map(|w| w.flags.contains(WindowFlags::RESIZABLE)).unwrap_or(false);
            if is_decorated {
                let bounds = self.windows[&wid].bounds;
                let client = Rect::new(bounds.x, bounds.y + tbh, bounds.width, (bounds.height - tbh).max(0.0));
                let zone = hit_test_decoration(client, &self.decoration_style, x, y);
                match zone {
                    HitZone::CloseButton => { let _ = self.set_focus(wid); return Some(ShellAction::CloseWindow); }
                    HitZone::MaximizeButton => { let _ = self.set_focus(wid); return Some(ShellAction::MaximizeWindow); }
                    HitZone::MinimizeButton => { let _ = self.set_focus(wid); return Some(ShellAction::MinimizeWindow); }
                    HitZone::AlwaysOnTopButton => { let _ = self.set_focus(wid); return Some(ShellAction::ToggleAlwaysOnTop); }
                    HitZone::TitleBar => {
                        let _ = self.set_focus(wid);
                        let _ = self.raise_window(wid);
                        self.drag_state = Some(DragState::Moving { window_id: wid, offset_x: x - bounds.x, offset_y: y - bounds.y });
                        return Some(ShellAction::Redraw);
                    }
                    HitZone::ResizeTop | HitZone::ResizeBottom | HitZone::ResizeLeft | HitZone::ResizeRight
                    | HitZone::ResizeTopLeft | HitZone::ResizeTopRight | HitZone::ResizeBottomLeft | HitZone::ResizeBottomRight
                        if is_resizable =>
                    {
                        let _ = self.set_focus(wid);
                        let _ = self.raise_window(wid);
                        self.drag_state = Some(DragState::Resizing { window_id: wid, edge: zone, start_bounds: bounds, start_x: x, start_y: y });
                        return Some(ShellAction::Redraw);
                    }
                    _ => { let _ = self.set_focus(wid); let _ = self.raise_window(wid); }
                }
            } else {
                let _ = self.set_focus(wid);
                let _ = self.raise_window(wid);
            }
        }
        None
    }
}
