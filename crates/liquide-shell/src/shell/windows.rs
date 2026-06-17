//! Window management — create, close, resize, move, focus, z-order.

use liquide_compositor::geometry::Rect;
use liquide_window_class::{ClassRegistry, WindowClass};
use liquide_window_effects::{EffectFrame, EffectManager, Rect as EffectRect};
use liquide_window_groups::{AutoGroupPolicy, FocusReason, GroupManager};
use liquide_window_tree::{
    Rect as TreeRect, WindowExStyle as TreeExStyle, WindowId as TreeWindowId,
    WindowStyle as TreeStyle, WindowTree,
};

use crate::history::WindowEventKind;
use crate::window::{Window, WindowFlags, WindowId, WindowState};
use crate::{Result, ShellError};

use super::Shell;
use super::hooks::ShellHookEvent;

/// Module id used for the shell's window classes in the canonical
/// `liquide-window-class` registry. The shell registers one class per
/// distinct application (keyed by class name = app id), plus a shared
/// `"Window"` class for app-less windows.
const SHELL_CLASS_MODULE_ID: u64 = 0;

/// Class name used for windows that carry no application id.
const DEFAULT_WINDOW_CLASS: &str = "Window";

impl Shell {
    /// Map a window's `app_id` to its canonical window-class name.
    fn class_name_for(app_id: &str) -> &str {
        if app_id.is_empty() {
            DEFAULT_WINDOW_CLASS
        } else {
            app_id
        }
    }

    /// Register a freshly-created window with the canonical chrome managers:
    /// the `liquide-window-class` instance/class registry and the
    /// `liquide-window-groups` grouping manager (auto-group by application).
    /// The managers are constructed lazily on the first window so the shell
    /// stays inert until windows actually exist.
    fn register_window_chrome(&mut self, id: WindowId, app_id: &str) {
        self.mark_wired(crate::shell::WiringBit::WindowClass);
        self.mark_wired(crate::shell::WiringBit::WindowGroups);
        // --- Window class registry: one class per app (instance counting). ---
        let registry = self
            .chrome_window_class
            .get_or_insert_with(ClassRegistry::new);
        let class_name = Self::class_name_for(app_id);
        let atom = match registry.find_by_name(class_name, SHELL_CLASS_MODULE_ID) {
            Some(class) => class.atom,
            None => registry
                .register_class(WindowClass::new(class_name, 0, SHELL_CLASS_MODULE_ID))
                .expect("unique shell window class registers"),
        };
        registry.add_instance(atom);

        // --- Grouping: auto-group by application. ---
        let groups = self.chrome_window_groups.get_or_insert_with(|| {
            let mut g = GroupManager::new();
            g.auto_group_policy = AutoGroupPolicy::ByApplication;
            g
        });
        let app = if app_id.is_empty() {
            None
        } else {
            Some(app_id)
        };
        groups.auto_group_window(id.0, app, None);
    }

    /// Unregister a window from the canonical chrome managers on destroy:
    /// decrement the class instance count and drop it from all groups/tab
    /// groups.
    fn unregister_window_chrome(&mut self, id: WindowId, app_id: &str) {
        if let Some(registry) = self.chrome_window_class.as_mut() {
            let class_name = Self::class_name_for(app_id);
            if let Some(class) = registry.find_by_name(class_name, SHELL_CLASS_MODULE_ID) {
                let atom = class.atom;
                registry.remove_instance(atom);
            }
        }
        if let Some(groups) = self.chrome_window_groups.as_mut() {
            groups.unregister_window(id.0);
        }
    }

    /// Convert a shell (`f32`) rect into the integer rect the canonical
    /// `liquide-window-tree` uses.
    fn tree_rect(bounds: Rect) -> TreeRect {
        TreeRect::new(
            bounds.x.round() as i32,
            bounds.y.round() as i32,
            bounds.width.round().max(0.0) as i32,
            bounds.height.round().max(0.0) as i32,
        )
    }

    /// Convert a shell (`f32`) rect into the `liquide-window-effects` rect type.
    fn effect_rect(bounds: Rect) -> EffectRect {
        EffectRect::new(bounds.x, bounds.y, bounds.width, bounds.height)
    }

    /// Register a freshly-created window with the canonical
    /// [`WindowTree`](liquide_window_tree::WindowTree): inserts a top-level node
    /// mirroring the flat window's bounds/title, records the resulting tree id
    /// on the [`Window`] so the two models stay in sync, and drives the window
    /// "open" effect. The tree is lazily constructed on the first window using
    /// the current screen size as the desktop root.
    fn register_window_tree(&mut self, id: WindowId) {
        let (bounds, title) = match self.windows.get(&id) {
            Some(w) => (w.bounds, w.title.clone()),
            None => return,
        };
        self.mark_wired(crate::shell::WiringBit::WindowTree);
        self.mark_wired(crate::shell::WiringBit::WindowEffects);
        let screen = self.screen_rect;
        let tree = self.chrome_window_tree.get_or_insert_with(|| {
            WindowTree::new(
                screen.width.round().max(1.0) as i32,
                screen.height.round().max(1.0) as i32,
            )
        });
        let tree_id = tree.create_window(
            None,
            0,
            TreeStyle::OVERLAPPED_WINDOW,
            TreeExStyle::empty(),
            Self::tree_rect(bounds),
            title,
        );
        if let Some(w) = self.windows.get_mut(&id) {
            w.tree_id = Some(tree_id.0);
        }

        // Drive the canonical open animation.
        let effects = self
            .chrome_window_effects
            .get_or_insert_with(EffectManager::new);
        effects.open_window(id.0, Self::effect_rect(bounds));
    }

    /// Look up a window's tree node id, if it has been registered with the tree.
    fn tree_id_of(&self, id: WindowId) -> Option<TreeWindowId> {
        self.windows
            .get(&id)
            .and_then(|w| w.tree_id)
            .map(TreeWindowId)
    }

    /// Map a tree node id back to the shell [`WindowId`] that owns it.
    fn shell_id_for_tree(&self, tree_id: TreeWindowId) -> Option<WindowId> {
        self.windows
            .iter()
            .find(|(_, w)| w.tree_id == Some(tree_id.0))
            .map(|(id, _)| *id)
    }

    /// Remove a window from the canonical [`WindowTree`] on destroy and drive
    /// the window "close" effect. Takes the tree node id explicitly because the
    /// flat window record is already gone from `self.windows` by close time.
    fn unregister_window_tree_node(
        &mut self,
        tree_id: Option<TreeWindowId>,
        id: WindowId,
        bounds: Rect,
    ) {
        if let (Some(tree_id), Some(tree)) = (tree_id, self.chrome_window_tree.as_mut()) {
            tree.destroy_window(tree_id);
        }
        if let Some(effects) = self.chrome_window_effects.as_mut() {
            effects.close_window(id.0, Self::effect_rect(bounds));
        }
    }

    /// Mirror a window's current bounds into its tree node (keeps the tree's
    /// hit-test geometry consistent after a move/resize/state change).
    fn sync_tree_bounds(&mut self, id: WindowId) {
        let (tree_id, bounds) = match (self.tree_id_of(id), self.windows.get(&id)) {
            (Some(t), Some(w)) => (t, w.bounds),
            _ => return,
        };
        let rect = Self::tree_rect(bounds);
        if let Some(tree) = self.chrome_window_tree.as_mut() {
            if let Some(node) = tree.get_mut(tree_id) {
                node.bounds = rect;
                node.client_rect = rect;
            }
        }
    }

    /// Mirror a window's shown/hidden state into its tree node so the
    /// tree-routed hit-test skips minimized windows.
    fn sync_tree_visibility(&mut self, id: WindowId, visible: bool) {
        let tree_id = match self.tree_id_of(id) {
            Some(t) => t,
            None => return,
        };
        if let Some(tree) = self.chrome_window_tree.as_mut() {
            if let Some(node) = tree.get_mut(tree_id) {
                if visible {
                    node.flags.insert(liquide_window_tree::WindowFlags::VISIBLE);
                } else {
                    node.flags.remove(liquide_window_tree::WindowFlags::VISIBLE);
                }
            }
        }
    }

    /// Drive a canonical "transform" effect for a window moving/resizing
    /// between two rects (maximize, restore, fullscreen, tile).
    fn drive_transform_effect(&mut self, id: WindowId, from: Rect, to: Rect) {
        if from == to {
            return;
        }
        let effects = self
            .chrome_window_effects
            .get_or_insert_with(EffectManager::new);
        effects.transform_window(id.0, Self::effect_rect(from), Self::effect_rect(to));
    }

    /// Topmost window at a screen point, resolved through the canonical
    /// [`WindowTree`] hit-test (z-order-aware, child-over-parent, skips
    /// invisible/transparent nodes). Falls back to a flat top-down scan of the
    /// visible window list when the tree has not been constructed yet.
    ///
    /// Returns the shell [`WindowId`] of the hit window, or `None` on a miss.
    #[must_use]
    pub fn window_at_point(&self, x: f32, y: f32) -> Option<WindowId> {
        if let Some(tree) = self.chrome_window_tree.as_ref() {
            if let Some(tree_id) = tree.window_at_point((x.round() as i32, y.round() as i32)) {
                return self.shell_id_for_tree(tree_id);
            }
            return None;
        }
        // Fallback: flat scan, topmost (highest z) first.
        let pt = liquide_compositor::geometry::Point::new(x, y);
        self.visible_windows()
            .into_iter()
            .rev()
            .find(|w| w.bounds.contains(pt))
            .map(|w| w.id)
    }

    /// Advance all active window effects by one frame and return the per-window
    /// frames produced. Finished effects are dropped. No-op (empty) when no
    /// effect manager has been constructed yet.
    pub fn tick_window_effects(&mut self) -> Vec<EffectFrame> {
        match self.chrome_window_effects.as_mut() {
            Some(effects) => effects.tick(),
            None => Vec::new(),
        }
    }

    /// Whether a window currently has an active canonical effect animating.
    #[must_use]
    pub fn window_is_animating(&self, id: WindowId) -> bool {
        self.chrome_window_effects
            .as_ref()
            .is_some_and(|e| e.is_animating(id.0))
    }

    /// Open a new window. Returns its ID.
    pub fn open_window(&mut self, title: impl Into<String>, bounds: Rect) -> WindowId {
        let id = WindowId(self.next_window_id);
        self.next_window_id += 1;
        let window = Window::new(id, title, bounds);
        self.windows.insert(id, window);
        self.workspaces.active_mut().add_window(id);
        self.register_window_chrome(id, "");
        self.register_window_tree(id);
        let ts = self.next_timestamp();
        self.window_history
            .record_at(id, WindowEventKind::Opened, ts);
        self.hook_manager
            .dispatch(&ShellHookEvent::WindowCreated { window_id: id.0 });
        // Assign the window to the monitor under its spawn center (t73-multimon
        // §3.3). No-op when no multi-monitor layout is installed.
        self.assign_window_to_monitor(id);
        self.mark_window_scene_dirty();
        id
    }

    /// Open a new window with an application ID. Returns its ID.
    pub fn open_window_with_app(
        &mut self,
        title: impl Into<String>,
        bounds: Rect,
        app_id: impl Into<String>,
    ) -> WindowId {
        let id = WindowId(self.next_window_id);
        self.next_window_id += 1;
        let app_id_str: String = app_id.into();
        if !app_id_str.is_empty() {
            self.sandbox_manager.register_app(app_id_str.clone());
        }
        let mut window = Window::new(id, title, bounds);
        window.app_id = app_id_str.clone();
        self.windows.insert(id, window);
        self.workspaces.active_mut().add_window(id);
        self.register_window_chrome(id, &app_id_str);
        self.register_window_tree(id);
        let ts = self.next_timestamp();
        self.window_history
            .record_at(id, WindowEventKind::Opened, ts);
        if !app_id_str.is_empty() {
            self.app_history.record_open(&app_id_str, id, bounds, ts);
            self.screen_time.feed_open(&app_id_str, id, ts);
        }
        self.hook_manager
            .dispatch(&ShellHookEvent::WindowCreated { window_id: id.0 });
        // Assign the window to the monitor under its spawn center (t73-multimon
        // §3.3). No-op when no multi-monitor layout is installed.
        self.assign_window_to_monitor(id);
        self.mark_window_scene_dirty();
        id
    }

    /// Close a window. Returns the removed window.
    pub fn close_window(&mut self, id: WindowId) -> Result<Window> {
        let window = self
            .windows
            .remove(&id)
            .ok_or(ShellError::WindowNotFound { id })?;
        // Remove from the OWNING workspace (which may be inactive), not just the
        // active one — otherwise a window closed while another workspace is
        // active leaves a dangling membership entry that resurfaces on the next
        // switch (t60-windows CRITICAL-1). Falls back to the active-workspace
        // removal only if canonical ownership cannot be resolved.
        if !self.workspaces.remove_window_from_owner(id) {
            self.workspaces.active_mut().remove_window(id);
        }
        self.focus.remove_window(id);
        // Drop the window's typed-text buffer + any pending double-click state
        // so closed windows leave no stale input state (t57-fG).
        self.focused_app_text.remove(&id);
        // Drop the window's live app view + content revision (t70-s6) so the
        // app's runtime is freed and no stale view/state outlives the window.
        self.app_views.remove(&id);
        self.app_content_revs.remove(&id);
        // Drop the window's monitor assignment (t73-multimon §3.3).
        self.window_monitors.remove(&id);
        if matches!(self.last_titlebar_click, Some((wid, _, _)) if wid == id) {
            self.last_titlebar_click = None;
        }
        self.unregister_window_chrome(id, &window.app_id);
        self.unregister_window_tree_node(window.tree_id.map(TreeWindowId), id, window.bounds);
        let ts = self.next_timestamp();
        self.window_history
            .record_at(id, WindowEventKind::Closed, ts);
        if !window.app_id.is_empty() {
            self.app_history
                .record_close(&window.app_id, id, window.bounds, ts);
            self.screen_time.feed_close(&window.app_id, id, ts);
            self.dock.remove_running(&window.app_id);
            let has_other_windows = self.windows.values().any(|w| w.app_id == window.app_id);
            if !has_other_windows {
                self.sandbox_manager.unregister_app(&window.app_id);
            }
        }
        self.hook_manager
            .dispatch(&ShellHookEvent::WindowClosed { window_id: id.0 });
        self.mark_window_scene_dirty();
        Ok(window)
    }

    /// Get a window by ID.
    pub fn window(&self, id: WindowId) -> Result<&Window> {
        self.windows
            .get(&id)
            .ok_or(ShellError::WindowNotFound { id })
    }

    /// Get a window mutably by ID.
    pub fn window_mut(&mut self, id: WindowId) -> Result<&mut Window> {
        if !self.windows.contains_key(&id) {
            return Err(ShellError::WindowNotFound { id });
        }
        self.mark_window_scene_dirty();
        Ok(self
            .windows
            .get_mut(&id)
            .expect("window existence checked before mutable access"))
    }

    /// Move a window to a new position.
    pub fn move_window(&mut self, id: WindowId, x: f32, y: f32) -> Result<()> {
        let win = self
            .windows
            .get_mut(&id)
            .ok_or(ShellError::WindowNotFound { id })?;
        let from = win.bounds;
        win.bounds.x = x;
        win.bounds.y = y;
        let to = win.bounds;
        let ts = self.next_timestamp();
        self.window_history
            .record_at(id, WindowEventKind::Moved { from, to }, ts);
        self.hook_manager.dispatch(&ShellHookEvent::WindowMoved {
            window_id: id.0,
            x: x.round() as i32,
            y: y.round() as i32,
        });
        self.sync_tree_bounds(id);
        self.mark_window_scene_dirty();
        Ok(())
    }

    /// Resize a window.
    pub fn resize_window(&mut self, id: WindowId, width: f32, height: f32) -> Result<()> {
        let win = self
            .windows
            .get_mut(&id)
            .ok_or(ShellError::WindowNotFound { id })?;
        let from = win.bounds;
        win.bounds.width = width;
        win.bounds.height = height;
        let to = win.bounds;
        let ts = self.next_timestamp();
        self.window_history
            .record_at(id, WindowEventKind::Resized { from, to }, ts);
        self.hook_manager.dispatch(&ShellHookEvent::WindowResized {
            window_id: id.0,
            width: width.round() as u32,
            height: height.round() as u32,
        });
        self.sync_tree_bounds(id);
        self.mark_window_scene_dirty();
        Ok(())
    }

    /// Minimize a window.
    pub fn minimize(&mut self, id: WindowId) -> Result<()> {
        let win = self
            .windows
            .get_mut(&id)
            .ok_or(ShellError::WindowNotFound { id })?;
        let from_state = win.state;
        let from_visible = win.visible;
        win.save_bounds();
        win.state = WindowState::Minimized;
        win.visible = false;
        let ts = self.next_timestamp();
        self.window_history.record_at(
            id,
            WindowEventKind::StateChanged {
                from: from_state,
                to: WindowState::Minimized,
            },
            ts,
        );
        if from_visible {
            let ts2 = self.next_timestamp();
            self.window_history.record_at(
                id,
                WindowEventKind::VisibilityChanged {
                    from: true,
                    to: false,
                },
                ts2,
            );
        }
        self.hook_manager
            .dispatch(&ShellHookEvent::WindowMinimized { window_id: id.0 });
        self.sync_tree_visibility(id, false);
        self.mark_window_scene_dirty();
        Ok(())
    }

    /// Maximize a window to fill the work area (screen minus statusbar and dock).
    pub fn maximize(&mut self, id: WindowId) -> Result<()> {
        let work = self.work_area();
        let win = self
            .windows
            .get_mut(&id)
            .ok_or(ShellError::WindowNotFound { id })?;
        let from_state = win.state;
        let from_bounds = win.bounds;
        win.save_bounds();
        win.state = WindowState::Maximized;
        win.bounds = work;
        let ts = self.next_timestamp();
        self.window_history.record_at(
            id,
            WindowEventKind::StateChanged {
                from: from_state,
                to: WindowState::Maximized,
            },
            ts,
        );
        let ts2 = self.next_timestamp();
        self.window_history.record_at(
            id,
            WindowEventKind::Resized {
                from: from_bounds,
                to: work,
            },
            ts2,
        );
        self.hook_manager
            .dispatch(&ShellHookEvent::WindowMaximized { window_id: id.0 });
        self.drive_transform_effect(id, from_bounds, work);
        self.sync_tree_bounds(id);
        self.mark_window_scene_dirty();
        Ok(())
    }

    /// Restore a window from minimized/maximized/fullscreen.
    pub fn restore(&mut self, id: WindowId) -> Result<()> {
        let win = self
            .windows
            .get_mut(&id)
            .ok_or(ShellError::WindowNotFound { id })?;
        let from_state = win.state;
        let from_visible = win.visible;
        let from_bounds = win.bounds;
        win.restore_bounds();
        win.state = WindowState::Normal;
        win.visible = true;
        let to_bounds = win.bounds;
        let ts = self.next_timestamp();
        self.window_history.record_at(
            id,
            WindowEventKind::StateChanged {
                from: from_state,
                to: WindowState::Normal,
            },
            ts,
        );
        if !from_visible {
            let ts2 = self.next_timestamp();
            self.window_history.record_at(
                id,
                WindowEventKind::VisibilityChanged {
                    from: false,
                    to: true,
                },
                ts2,
            );
        }
        if from_bounds != to_bounds {
            let ts3 = self.next_timestamp();
            self.window_history.record_at(
                id,
                WindowEventKind::Resized {
                    from: from_bounds,
                    to: to_bounds,
                },
                ts3,
            );
        }
        self.hook_manager
            .dispatch(&ShellHookEvent::WindowRestored { window_id: id.0 });
        self.sync_tree_visibility(id, true);
        self.drive_transform_effect(id, from_bounds, to_bounds);
        self.sync_tree_bounds(id);
        self.mark_window_scene_dirty();
        Ok(())
    }

    /// Toggle fullscreen.
    pub fn toggle_fullscreen(&mut self, id: WindowId) -> Result<()> {
        let screen = self.screen_rect;
        let win = self
            .windows
            .get_mut(&id)
            .ok_or(ShellError::WindowNotFound { id })?;
        let from_state = win.state;
        let from_bounds = win.bounds;
        if win.state == WindowState::Fullscreen {
            win.restore_bounds();
            win.state = WindowState::Normal;
        } else {
            win.save_bounds();
            win.state = WindowState::Fullscreen;
            win.bounds = screen;
        }
        let to_state = win.state;
        let to_bounds = win.bounds;
        let ts = self.next_timestamp();
        self.window_history.record_at(
            id,
            WindowEventKind::StateChanged {
                from: from_state,
                to: to_state,
            },
            ts,
        );
        if from_bounds != to_bounds {
            let ts2 = self.next_timestamp();
            self.window_history.record_at(
                id,
                WindowEventKind::Resized {
                    from: from_bounds,
                    to: to_bounds,
                },
                ts2,
            );
        }
        self.drive_transform_effect(id, from_bounds, to_bounds);
        self.sync_tree_bounds(id);
        self.mark_window_scene_dirty();
        Ok(())
    }

    /// Set focus to a window.
    ///
    /// NOTE: This updates the internal focus manager but does NOT sync the
    /// `class="focused"` attribute in the DOM because windows are rendered
    /// manually via `scene.rs` rather than through the CSS pipeline.  The
    /// `DesktopDocument::set_focused_window()` helper exists for future use
    /// when/if windows are migrated to DOM-based rendering.  The scene
    /// builder already reads `self.focus.focused()` directly to determine
    /// the focused-window visual state (title-bar colour, border colour).
    pub fn set_focus(&mut self, id: WindowId) -> Result<()> {
        if !self.windows.contains_key(&id) {
            return Err(ShellError::WindowNotFound { id });
        }
        let prev_focused = self.focus.focused();
        self.focus.set_focus(id);
        if let Some(prev_id) = prev_focused {
            if prev_id != id {
                let ts = self.next_timestamp();
                self.window_history
                    .record_at(prev_id, WindowEventKind::Unfocused, ts);
                self.screen_time.feed_unfocus(ts);
                self.hook_manager
                    .dispatch(&ShellHookEvent::WindowDeactivated {
                        window_id: prev_id.0,
                    });
            }
        }
        let ts2 = self.next_timestamp();
        self.window_history
            .record_at(id, WindowEventKind::Focused, ts2);
        let app_id = self
            .windows
            .get(&id)
            .map(|w| w.app_id.clone())
            .unwrap_or_default();
        if !app_id.is_empty() {
            self.screen_time.feed_focus(&app_id, id, ts2);
        }
        // Record the focused-window context (app id + activity time) so the
        // canonical focus-stealing guard can evaluate later, non-user focus
        // requests against this baseline.
        let ctx_app = if app_id.is_empty() {
            None
        } else {
            Some(app_id.clone())
        };
        self.focus.note_focus_context(ctx_app, ts2);
        // Mirror the activation into the canonical WindowTree z-order so that
        // hit-testing favours the newly focused window. Without this, focusing a
        // background window (e.g. via a click) updates the focus manager but
        // leaves the tree's topmost entry pointing at the previous window, so
        // subsequent clicks route to the wrong (background) window
        // (t60-windows MAJOR-4). `raise_window` already does this on restack.
        if let Some(tree_id) = self.tree_id_of(id) {
            if let Some(tree) = self.chrome_window_tree.as_mut() {
                tree.bring_to_top(tree_id);
            }
        }
        // Re-assert the always-on-top band after the focus restack: focusing a
        // *normal* window must not lift it above the AOT band in the tree-routed
        // hit-test (t93-e1 / t92 gap #2). No-op when no AOT window exists.
        self.restack_tree_band_order();
        self.hook_manager
            .dispatch(&ShellHookEvent::WindowActivated { window_id: id.0 });
        // Drive the canonical focus-highlight effect.
        if let Some(bounds) = self.windows.get(&id).map(|w| w.bounds) {
            let effects = self
                .chrome_window_effects
                .get_or_insert_with(EffectManager::new);
            effects.focus_window(id.0, Self::effect_rect(bounds));
        }
        self.mark_window_scene_dirty();
        Ok(())
    }

    /// Request that a window receive focus subject to the canonical
    /// focus-stealing-prevention policy (`liquide-window-groups`).
    ///
    /// Unlike [`Self::set_focus`] (the unconditional, user-driven activation
    /// path), this consults the focus guard with the supplied [`FocusReason`].
    /// If the policy denies the steal, focus is left unchanged and `Ok(false)`
    /// is returned; on allow, focus moves and `Ok(true)` is returned. The
    /// focused window's group is also raised to the top of focus history so
    /// focus follows the canonical group policy.
    pub fn request_window_focus(&mut self, id: WindowId, reason: FocusReason) -> Result<bool> {
        if !self.windows.contains_key(&id) {
            return Err(ShellError::WindowNotFound { id });
        }
        let app_id = self
            .windows
            .get(&id)
            .map(|w| w.app_id.clone())
            .unwrap_or_default();
        let req_app = if app_id.is_empty() {
            None
        } else {
            Some(app_id.clone())
        };
        let ts = self.next_timestamp();
        let decision = self.focus.evaluate_focus_request(id, req_app, reason, ts);
        if matches!(decision, liquide_window_groups::FocusDecision::Allow) {
            self.set_focus(id)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get the number of windows.
    #[must_use]
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// Get all visible windows on the **active workspace**, sorted by z_order
    /// (ascending).
    ///
    /// Fixes t49-e5-F01 (cosmetic workspaces): a window is only rendered /
    /// hit-tested when it is BOTH flagged `visible` AND a member of the active
    /// workspace. Workspace membership is the canonical decision (see
    /// `active_workspace_members`, kept in lockstep with the canonical
    /// `liquide-workspaces` manager by the switch path in `shell/batch.rs`).
    /// Before this fix the filter ignored membership, so every workspace
    /// rendered every window and switching was purely cosmetic.
    #[must_use]
    pub fn visible_windows(&self) -> Vec<&Window> {
        let active = self.workspaces.active();
        let mut visible: Vec<&Window> = self
            .windows
            .values()
            .filter(|w| w.visible && active.contains(w.id))
            .collect();
        // Band-aware stacking (t93-e1 / t92 gap #2): always-on-top windows form a
        // top band that sorts strictly above the normal band, while relative
        // z_order within each band is preserved. This single sort feeds both the
        // paint order AND (until the tree becomes the sole live router) the flat
        // hover/click scan, so it fixes paint and hit-test in one place. A normal
        // window raised to the top of the normal band still sits below every
        // always-on-top window.
        visible.sort_by_key(|w| Self::stacking_key(w));
        visible
    }

    /// Band-aware stacking key: `(always_on_top, z_order)`.
    ///
    /// The leading band bit makes always-on-top windows sort strictly above
    /// non-AOT windows; the trailing `z_order` preserves relative order within
    /// each band. Used by every place that needs the live stacking order so the
    /// AOT band is honored consistently (paint, flat-scan hit-test fallback, and
    /// the tree restack mirror).
    fn stacking_key(w: &Window) -> (u8, i32) {
        (u8::from(w.flags.contains(WindowFlags::ALWAYS_ON_TOP)), w.z_order)
    }

    /// Whether `id` is currently flagged always-on-top.
    fn is_always_on_top(&self, id: WindowId) -> bool {
        self.windows
            .get(&id)
            .is_some_and(|w| w.flags.contains(WindowFlags::ALWAYS_ON_TOP))
    }

    /// Apply the current layout to visible windows on the **active workspace**.
    ///
    /// Mirrors the membership filter used by [`Self::visible_windows`]: layout
    /// must only position windows that are both `visible` AND members of the
    /// active workspace, otherwise it relocates windows belonging to inactive
    /// workspaces and causes flicker/disappear on switch (t60-windows
    /// CRITICAL-2).
    pub fn arrange_windows(&mut self) {
        let screen = self.screen_rect;
        let active = self.workspaces.active();
        let mut visible_ids: Vec<WindowId> = self
            .windows
            .values()
            .filter(|w| w.visible && active.contains(w.id))
            .map(|w| w.id)
            .collect();
        visible_ids.sort_by_key(|id| id.0);
        let mut window_vec: Vec<Window> = visible_ids
            .iter()
            .filter_map(|id| self.windows.get(id).cloned())
            .collect();
        self.layout.arrange(&mut window_vec, screen);
        let mut changed = false;
        for win in window_vec {
            if let Some(existing) = self.windows.get_mut(&win.id) {
                if existing.bounds != win.bounds {
                    changed = true;
                }
                existing.bounds = win.bounds;
            }
        }
        if changed {
            self.mark_window_scene_dirty();
        }
    }

    /// Raise a window to the top of **its own stacking band** (highest z_order
    /// within the band).
    ///
    /// Band-aware (t93-e1 / t92 gap #2): the max is computed over windows that
    /// share the target's always-on-top flag, so raising a *normal* window can
    /// never jump it above an always-on-top window, and raising an *AOT* window
    /// keeps it inside the AOT band (above all normals). `normalize_z_orders`
    /// then re-packs both bands so the global `z_order` stays monotonic with the
    /// band, and the tree mirror re-applies the band so the live tree-routed
    /// hit-test agrees with `visible_windows`.
    pub fn raise_window(&mut self, id: WindowId) -> Result<()> {
        if !self.windows.contains_key(&id) {
            return Err(ShellError::WindowNotFound { id });
        }
        let target_aot = self.is_always_on_top(id);
        let band_max = self
            .windows
            .values()
            .filter(|w| w.flags.contains(WindowFlags::ALWAYS_ON_TOP) == target_aot)
            .map(|w| w.z_order)
            .max()
            .unwrap_or(0);
        let win = self
            .windows
            .get_mut(&id)
            .ok_or(ShellError::WindowNotFound { id })?;
        let from_z = win.z_order;
        win.z_order = band_max + 1;
        let ts = self.next_timestamp();
        self.window_history.record_at(
            id,
            WindowEventKind::ZOrderChanged {
                from: from_z,
                to: band_max + 1,
            },
            ts,
        );
        self.normalize_z_orders();
        // Mirror the restack into the canonical tree z-order, then re-apply the
        // AOT band so the tree never buries an always-on-top window under a
        // freshly-raised normal one.
        if let Some(tree_id) = self.tree_id_of(id) {
            if let Some(tree) = self.chrome_window_tree.as_mut() {
                tree.bring_to_top(tree_id);
            }
        }
        self.restack_tree_band_order();
        self.mark_window_scene_dirty();
        Ok(())
    }

    /// Lower a window to the bottom of **its own stacking band** (lowest z_order
    /// within the band).
    ///
    /// Band-aware (t93-e1): an always-on-top window lowered to the back of its
    /// band still sits above every normal window; a normal window sinks below
    /// the other normals but stays under the AOT band.
    pub fn lower_window(&mut self, id: WindowId) -> Result<()> {
        if !self.windows.contains_key(&id) {
            return Err(ShellError::WindowNotFound { id });
        }
        let target_aot = self.is_always_on_top(id);
        let band_min = self
            .windows
            .values()
            .filter(|w| w.flags.contains(WindowFlags::ALWAYS_ON_TOP) == target_aot)
            .map(|w| w.z_order)
            .min()
            .unwrap_or(0);
        // Drop just below the band floor, then normalize re-packs both bands so
        // the AOT band still sorts entirely above the normal band.
        if let Some(win) = self.windows.get_mut(&id) {
            win.z_order = band_min - 1;
        }
        self.normalize_z_orders();
        // Mirror the restack into the canonical tree z-order, then re-apply the
        // AOT band so an AOT window sent to the back of its band stays above the
        // normals in the tree-routed hit-test.
        if let Some(tree_id) = self.tree_id_of(id) {
            if let Some(tree) = self.chrome_window_tree.as_mut() {
                tree.send_to_bottom(tree_id);
            }
        }
        self.restack_tree_band_order();
        let new_z = self.windows.get(&id).map(|w| w.z_order).unwrap_or(0);
        let ts = self.next_timestamp();
        self.window_history.record_at(
            id,
            WindowEventKind::ZOrderChanged {
                from: new_z,
                to: new_z,
            },
            ts,
        );
        self.mark_window_scene_dirty();
        Ok(())
    }

    /// Compact z_order values to sequential non-negative integers,
    /// preserving relative order. Prevents unbounded growth from
    /// repeated raise/lower operations.
    ///
    /// Band-aware (t93-e1): windows are packed by the full stacking key
    /// `(always_on_top, z_order)`, so the always-on-top band is assigned the
    /// higher ordinals and the normal band the lower ones. This makes the raw
    /// `z_order` itself monotonic with the band — every AOT window ends up with a
    /// strictly greater `z_order` than every normal window — so any consumer that
    /// sorts purely by `z_order` (and not only the tuple key) still honors the
    /// band, and within-band relative order is preserved.
    pub(crate) fn normalize_z_orders(&mut self) {
        let mut sorted: Vec<(WindowId, (u8, i32))> = self
            .windows
            .iter()
            .map(|(id, w)| (*id, Self::stacking_key(w)))
            .collect();
        sorted.sort_by_key(|(_, key)| *key);
        let mut changed = false;
        for (i, (id, _)) in sorted.iter().enumerate() {
            if let Some(w) = self.windows.get_mut(id) {
                let z_order = i as i32;
                if w.z_order != z_order {
                    w.z_order = z_order;
                    changed = true;
                }
            }
        }
        if changed {
            self.mark_window_scene_dirty();
        }
    }

    /// Re-apply the always-on-top band to the canonical [`WindowTree`] so the
    /// live tree-routed hit-test never buries an AOT window under a normal one.
    ///
    /// The tree (used by [`Self::window_at_point`] on the live path) only knows
    /// raw sibling order — it has no band concept of its own. After any restack
    /// or AOT-flag toggle we therefore re-assert the band here: bring every
    /// always-on-top window to the top of the tree in **ascending** within-band
    /// `z_order`, so the highest-z AOT window ends up topmost and the whole AOT
    /// band sits above the normal band. Within-band relative order is preserved
    /// because we re-stack them lowest-first.
    ///
    /// No-op when no AOT window (or no tree) is present, so the common case pays
    /// nothing.
    fn restack_tree_band_order(&mut self) {
        if self.chrome_window_tree.is_none() {
            return;
        }
        // Collect AOT windows in ascending stacking order; bringing each to the
        // top in this order leaves the highest-z AOT window topmost in the tree.
        let mut aot: Vec<(i32, TreeWindowId)> = self
            .windows
            .values()
            .filter(|w| w.flags.contains(WindowFlags::ALWAYS_ON_TOP))
            .filter_map(|w| w.tree_id.map(|t| (w.z_order, TreeWindowId(t))))
            .collect();
        if aot.is_empty() {
            return;
        }
        aot.sort_by_key(|(z, _)| *z);
        if let Some(tree) = self.chrome_window_tree.as_mut() {
            for (_, tree_id) in aot {
                tree.bring_to_top(tree_id);
            }
        }
    }

    /// Re-apply the always-on-top band after a window's AOT flag changed.
    ///
    /// Called from the `ToggleAlwaysOnTop` action (tick.rs). The toggled window
    /// has moved between bands, so re-pack `z_order` (band-aware
    /// `normalize_z_orders`) and re-assert the band in the canonical tree, then
    /// mark the scene dirty so the new stacking actually repaints on the live
    /// idle fast path.
    pub(crate) fn apply_always_on_top_band(&mut self) {
        self.normalize_z_orders();
        self.restack_tree_band_order();
        self.mark_window_scene_dirty();
    }

    /// Open a new window for the given application, or focus an existing one.
    pub fn open_app_window(&mut self, app_id: &str) -> WindowId {
        self.sandbox_manager.register_app(app_id.to_string());
        let can_create_windows = self
            .sandbox_manager
            .with_sandbox(app_id, |sandbox| sandbox.can_create_windows())
            .unwrap_or(false);
        if !can_create_windows {
            tracing::warn!("Sandbox denied window creation for app: {}", app_id);
        }
        if let Some(existing) = self
            .windows
            .values()
            .find(|w| w.app_id == app_id && w.visible)
        {
            let wid = existing.id;
            let _ = self.set_focus(wid);
            let _ = self.raise_window(wid);
            return wid;
        }
        let work = self.work_area();
        let (title, w, h): (&str, f32, f32) = match app_id {
            "com.liquide.settings" => ("Settings", 700.0, 500.0),
            "com.liquide.terminal" => ("Terminal", 720.0, 480.0),
            "com.liquide.files" => ("Files", 800.0, 550.0),
            "com.liquide.browser" => ("Browser", 900.0, 600.0),
            "com.liquide.calculator" => ("Calculator", 360.0, 420.0),
            _ => ("Application", 640.0, 480.0),
        };
        let x = work.x + (work.width - w) / 2.0;
        let y = work.y + (work.height - h) / 2.0;
        let id = self.open_window_with_app(title, Rect::new(x, y, w, h), app_id);
        // Set sensible minimum sizes for known applications.
        let min = match app_id {
            "com.liquide.calculator" => Some((280.0, 320.0)),
            _ => Some((200.0, 150.0)),
        };
        if let Some(win) = self.windows.get_mut(&id) {
            win.min_size = min;
            self.mark_window_scene_dirty();
        }
        // t70-s6: make the window run the real app. If the host installed an
        // app-view factory, construct + register the matching `dyn AppView` so
        // the body paints real app content and keyboard input reaches the app's
        // model. No factory (or an id the host does not back) → the legacy
        // placeholder painting path still runs (so tests/legacy hosts work).
        self.install_app_view(id, app_id);
        self.dock.add_running(app_id);
        let _ = self.set_focus(id);
        let _ = self.raise_window(id);
        id
    }
}
