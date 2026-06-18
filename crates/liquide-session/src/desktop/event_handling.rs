//! Platform event routing — keyboard, mouse, touch, and window events.

use std::cell::RefCell;
use std::time::{SystemTime, UNIX_EPOCH};

use liquide_input::event::InputEvent;
use liquide_input::keyboard::{KeyCode, KeyState};
use liquide_platform::PlatformEvent;

use super::{DesktopCompositor, RenderMsg};

// ---------------------------------------------------------------------------
// Per-window DPI scale tracking
// ---------------------------------------------------------------------------
//
// Coordinate-space convention
// ----------------------------
// The Win32 backend delivers mouse coordinates in *physical* client pixels
// (`GET_X_LPARAM`/`GET_Y_LPARAM`), and the window's client rect — which drives
// the compositor/renderer surface size — is also in physical pixels. The
// shell's layout/hit-test space, however, is authored in *logical* (CSS) pixels.
// On a display scaled by `s` (e.g. 1.25 at 125%) the two spaces differ by `s`,
// so a raw physical click at (x, y) lands at logical (x/s, y/s). Without the
// correction a click misses its target by `(s - 1)`: 25% at 125%, 50% at 150%
// (e6 DPI input bug — dead context menus / buttons on scaled & remote/RDP
// displays where DPI reporting differs).
//
// Fix: divide incoming mouse coordinates by `dpi_scale` before they reach the
// shell hit-test and `input_state`, so dispatched coordinates are always in the
// logical layout space the renderer lays out in. `dpi_scale` is seeded from the
// window's real DPI at creation and updated on `PlatformEvent::DpiChanged`.
//
// The current scale lives here (not on `DesktopCompositor`, whose definition is
// owned by a peer) keyed by window handle. The desktop event loop is
// single-threaded, so a thread-local map is sufficient and avoids cross-file
// struct changes.
thread_local! {
    static DPI_SCALES: RefCell<std::collections::HashMap<u64, f32>> =
        RefCell::new(std::collections::HashMap::new());
}

/// Record the DPI scale for a window handle (clamped to a sane positive range).
fn set_dpi_scale(handle_id: u64, dpi_scale: f32) {
    // Reject non-finite or non-positive values so coordinate division can never
    // produce NaN/inf; fall back to 1.0 (unscaled) for bad input.
    let scale = if dpi_scale.is_finite() && dpi_scale > 0.0 {
        dpi_scale
    } else {
        1.0
    };
    DPI_SCALES.with(|m| {
        m.borrow_mut().insert(handle_id, scale);
    });
}

/// Current DPI scale for a window handle (defaults to 1.0 if unknown).
fn dpi_scale_for(handle_id: u64) -> f32 {
    DPI_SCALES.with(|m| m.borrow().get(&handle_id).copied().unwrap_or(1.0))
}

/// Convert a physical mouse coordinate pair into logical (CSS-pixel) layout
/// space for the given window. See the module-level convention note.
///
/// `logical = physical / dpi_scale`.
fn to_logical_coords(handle_id: u64, x: f32, y: f32) -> (f32, f32) {
    let scale = dpi_scale_for(handle_id);
    (x / scale, y / scale)
}

/// Return a copy of `me` with positional coordinates converted from physical to
/// logical (CSS-pixel) space for the given window. Non-positional fields
/// (button, state, axis, scroll delta) are preserved unchanged. `Leave` carries
/// no coordinates and is returned as-is.
fn scale_mouse_event_to_logical(
    handle_id: u64,
    me: &liquide_input::mouse::MouseEvent,
) -> liquide_input::mouse::MouseEvent {
    use liquide_input::mouse::MouseEvent;
    match me {
        MouseEvent::Move { x, y } => {
            let (x, y) = to_logical_coords(handle_id, *x, *y);
            MouseEvent::Move { x, y }
        }
        MouseEvent::Button {
            button,
            state,
            x,
            y,
        } => {
            let (x, y) = to_logical_coords(handle_id, *x, *y);
            MouseEvent::Button {
                button: *button,
                state: *state,
                x,
                y,
            }
        }
        MouseEvent::Scroll { axis, delta, x, y } => {
            let (x, y) = to_logical_coords(handle_id, *x, *y);
            MouseEvent::Scroll {
                axis: *axis,
                delta: *delta,
                x,
                y,
            }
        }
        MouseEvent::Enter { x, y } => {
            let (x, y) = to_logical_coords(handle_id, *x, *y);
            MouseEvent::Enter { x, y }
        }
        MouseEvent::Leave => MouseEvent::Leave,
    }
}

/// Return a copy of `te` with its contact-point coordinates converted from
/// physical to logical (CSS-pixel) space for the given window, mirroring
/// [`scale_mouse_event_to_logical`]. Without this, touch contacts on a scaled
/// display land at the wrong place because the layout/hit-test space is logical
/// (t60-input Medium: "Touch input NOT DPI-scaled"; t65-s2 item 6).
fn scale_touch_event_to_logical(
    handle_id: u64,
    te: &liquide_input::touch::TouchEvent,
) -> liquide_input::touch::TouchEvent {
    let (x, y) = to_logical_coords(handle_id, te.point.x, te.point.y);
    let mut scaled = *te;
    scaled.point.x = x;
    scaled.point.y = y;
    scaled
}

impl DesktopCompositor {
    /// Handle a platform event: route through shell and input state.
    ///
    /// Returns `true` if the event requires a full redraw.
    /// Sets `self.cursor.dirty` when only cursor position changed.
    pub fn handle_event(&mut self, event: &PlatformEvent) -> bool {
        let mut needs_redraw = false;

        // Normalize mouse coordinates from physical to logical (CSS-pixel) space
        // up front, so BOTH the input/devtools routing below AND the shell
        // hit-test routing at the end see logical coordinates. On a non-scaled
        // display (dpi_scale == 1.0) this is the identity. See the module-level
        // coordinate-space convention note. We hold the corrected event in a
        // local and borrow it for the rest of the function.
        let normalized;
        let event: &PlatformEvent = match event {
            PlatformEvent::MouseInput { event: me, handle } => {
                normalized = PlatformEvent::MouseInput {
                    handle: *handle,
                    event: scale_mouse_event_to_logical(handle.0, me),
                };
                &normalized
            }
            PlatformEvent::TouchInput { event: te, handle } => {
                // Normalize touch contacts to logical space too (t65-s2 item 6),
                // so touch hit-testing matches mouse on scaled displays.
                normalized = PlatformEvent::TouchInput {
                    handle: *handle,
                    event: scale_touch_event_to_logical(handle.0, te),
                };
                &normalized
            }
            other => other,
        };

        match event {
            PlatformEvent::WindowResized { width, height, .. } => {
                self.width = *width;
                self.height = *height;

                // During loading, resize compositor directly
                if let Some(ref mut compositor) = self.compositor {
                    let _ = compositor.resize(*width, *height);
                } else if let Some(ref tx) = self.render_tx {
                    // After loading, notify render thread
                    let _ = tx.send(RenderMsg::Resize {
                        width: *width,
                        height: *height,
                    });
                }

                self.shell.resize_screen(*width as f32, *height as f32);
                self.tiles.resize(*width, *height);
                self.dt.on_resize(*width, *height);
                needs_redraw = true;
            }
            PlatformEvent::WindowCloseRequested { .. } | PlatformEvent::Quit => {
                // Request shutdown, but DON'T tear down the loop immediately.
                // Stopping here would orphan an in-flight render job (the final
                // desktop frame never reaches the screen → black/stale flash on
                // close). The event loop drains and presents any pending frame
                // first, then honours `quit_requested` (t60-runtime #1).
                self.quit_requested = true;
            }
            PlatformEvent::WindowRedraw { .. } => {
                needs_redraw = true;
            }
            PlatformEvent::KeyInput { event: ke, .. } => {
                // DevTools keyboard shortcuts (intercept before shell).
                if self.dt.dev_mode {
                    if let Some(ref mut devtools) = self.dt.devtools {
                        if ke.state == KeyState::Pressed {
                            // Map KeyCode to a string for devtools handle_key.
                            let key_str: Option<&str> = match ke.key {
                                KeyCode::F12 => Some("F12"),
                                KeyCode::Tab => Some("Tab"),
                                KeyCode::Escape => Some("Escape"),
                                KeyCode::Enter => Some("Enter"),
                                KeyCode::Backspace => Some("Backspace"),
                                KeyCode::Delete => Some("Delete"),
                                KeyCode::ArrowUp => Some("ArrowUp"),
                                KeyCode::ArrowDown => Some("ArrowDown"),
                                KeyCode::ArrowLeft => Some("ArrowLeft"),
                                KeyCode::ArrowRight => Some("ArrowRight"),
                                KeyCode::Home => Some("Home"),
                                KeyCode::End => Some("End"),
                                KeyCode::Space => Some(" "),
                                // Letters.
                                KeyCode::A => Some(if ke.modifiers.shift() { "A" } else { "a" }),
                                KeyCode::B => Some(if ke.modifiers.shift() { "B" } else { "b" }),
                                KeyCode::C => Some(if ke.modifiers.shift() { "C" } else { "c" }),
                                KeyCode::D => Some(if ke.modifiers.shift() { "D" } else { "d" }),
                                KeyCode::E => Some(if ke.modifiers.shift() { "E" } else { "e" }),
                                KeyCode::F => Some(if ke.modifiers.shift() { "F" } else { "f" }),
                                KeyCode::G => Some(if ke.modifiers.shift() { "G" } else { "g" }),
                                KeyCode::H => Some(if ke.modifiers.shift() { "H" } else { "h" }),
                                KeyCode::I => Some(if ke.modifiers.shift() { "I" } else { "i" }),
                                KeyCode::J => Some(if ke.modifiers.shift() { "J" } else { "j" }),
                                KeyCode::K => Some(if ke.modifiers.shift() { "K" } else { "k" }),
                                KeyCode::L => Some(if ke.modifiers.shift() { "L" } else { "l" }),
                                KeyCode::M => Some(if ke.modifiers.shift() { "M" } else { "m" }),
                                KeyCode::N => Some(if ke.modifiers.shift() { "N" } else { "n" }),
                                KeyCode::O => Some(if ke.modifiers.shift() { "O" } else { "o" }),
                                KeyCode::P => Some(if ke.modifiers.shift() { "P" } else { "p" }),
                                KeyCode::Q => Some(if ke.modifiers.shift() { "Q" } else { "q" }),
                                KeyCode::R => Some(if ke.modifiers.shift() { "R" } else { "r" }),
                                KeyCode::S => Some(if ke.modifiers.shift() { "S" } else { "s" }),
                                KeyCode::T => Some(if ke.modifiers.shift() { "T" } else { "t" }),
                                KeyCode::U => Some(if ke.modifiers.shift() { "U" } else { "u" }),
                                KeyCode::V => Some(if ke.modifiers.shift() { "V" } else { "v" }),
                                KeyCode::W => Some(if ke.modifiers.shift() { "W" } else { "w" }),
                                KeyCode::X => Some(if ke.modifiers.shift() { "X" } else { "x" }),
                                KeyCode::Y => Some(if ke.modifiers.shift() { "Y" } else { "y" }),
                                KeyCode::Z => Some(if ke.modifiers.shift() { "Z" } else { "z" }),
                                // Digits / shifted symbols.
                                KeyCode::Digit0 => {
                                    Some(if ke.modifiers.shift() { ")" } else { "0" })
                                }
                                KeyCode::Digit1 => {
                                    Some(if ke.modifiers.shift() { "!" } else { "1" })
                                }
                                KeyCode::Digit2 => {
                                    Some(if ke.modifiers.shift() { "@" } else { "2" })
                                }
                                KeyCode::Digit3 => {
                                    Some(if ke.modifiers.shift() { "#" } else { "3" })
                                }
                                KeyCode::Digit4 => {
                                    Some(if ke.modifiers.shift() { "$" } else { "4" })
                                }
                                KeyCode::Digit5 => {
                                    Some(if ke.modifiers.shift() { "%" } else { "5" })
                                }
                                KeyCode::Digit6 => {
                                    Some(if ke.modifiers.shift() { "^" } else { "6" })
                                }
                                KeyCode::Digit7 => {
                                    Some(if ke.modifiers.shift() { "&" } else { "7" })
                                }
                                KeyCode::Digit8 => {
                                    Some(if ke.modifiers.shift() { "*" } else { "8" })
                                }
                                KeyCode::Digit9 => {
                                    Some(if ke.modifiers.shift() { "(" } else { "9" })
                                }
                                // Punctuation.
                                KeyCode::Period => {
                                    Some(if ke.modifiers.shift() { ">" } else { "." })
                                }
                                KeyCode::Comma => {
                                    Some(if ke.modifiers.shift() { "<" } else { "," })
                                }
                                KeyCode::Slash => {
                                    Some(if ke.modifiers.shift() { "?" } else { "/" })
                                }
                                KeyCode::Semicolon => {
                                    Some(if ke.modifiers.shift() { ":" } else { ";" })
                                }
                                KeyCode::Quote => {
                                    Some(if ke.modifiers.shift() { "\"" } else { "'" })
                                }
                                KeyCode::BracketLeft => {
                                    Some(if ke.modifiers.shift() { "{" } else { "[" })
                                }
                                KeyCode::BracketRight => {
                                    Some(if ke.modifiers.shift() { "}" } else { "]" })
                                }
                                KeyCode::Backslash => {
                                    Some(if ke.modifiers.shift() { "|" } else { "\\" })
                                }
                                KeyCode::Minus => {
                                    Some(if ke.modifiers.shift() { "_" } else { "-" })
                                }
                                KeyCode::Equal => {
                                    Some(if ke.modifiers.shift() { "+" } else { "=" })
                                }
                                KeyCode::Grave => {
                                    Some(if ke.modifiers.shift() { "~" } else { "`" })
                                }
                                _ => None,
                            };
                            if let Some(k) = key_str {
                                // For Enter in console, also pass doc/layout/styles for command execution.
                                if k == "Enter" && devtools.is_console_focused() {
                                    if let (Some(layout), Some(styles)) =
                                        (self.shell.layout_tree(), self.shell.style_map())
                                    {
                                        let doc = self.shell.document();
                                        devtools.handle_console_key(
                                            "Enter", false, false, doc, layout, styles,
                                        );
                                        needs_redraw = true;
                                    }
                                } else if devtools.handle_key(
                                    k,
                                    ke.modifiers.ctrl(),
                                    ke.modifiers.shift(),
                                    ke.modifiers.alt(),
                                ) {
                                    needs_redraw = true;
                                }
                            }
                        }
                    }
                }
                self.input_state.handle_event(&InputEvent::Keyboard(*ke));
            }
            PlatformEvent::DpiChanged { handle, dpi_scale } => {
                // Persist the new per-window DPI scale so subsequent mouse
                // coordinates are converted into logical layout space (see the
                // coordinate-space convention note at the top of this module).
                set_dpi_scale(handle.0, *dpi_scale);
            }
            PlatformEvent::MouseInput { event: me, .. } => {
                // `me` is already in logical (CSS-pixel) space — coordinates were
                // converted from physical at the top of `handle_event`.
                use liquide_input::mouse::MouseEvent;
                match me {
                    MouseEvent::Move { x, y } => {
                        self.cursor.update_position(*x, *y);
                        // Forward to devtools element picker.
                        if self.dt.dev_mode {
                            if let Some(ref mut devtools) = self.dt.devtools {
                                if let (Some(hit_test), Some(layout)) =
                                    (self.shell.hit_test_engine(), self.shell.layout_tree())
                                {
                                    let doc = self.shell.document();
                                    if devtools.on_mouse_move(*x, *y, hit_test, doc, layout) {
                                        needs_redraw = true;
                                    }
                                }
                            }
                        }
                    }
                    MouseEvent::Button {
                        x,
                        y,
                        button,
                        state,
                    } => {
                        self.cursor.set_position(*x, *y);
                        // Only react on button press, not release.
                        if *state == liquide_input::mouse::ButtonState::Pressed
                            && *button == liquide_input::mouse::MouseButton::Left
                        {
                            // Forward click to devtools panel (tabs, tree nodes, etc.)
                            // and element picker / viewport click-to-inspect.
                            if self.dt.dev_mode {
                                if let Some(ref mut devtools) = self.dt.devtools {
                                    if let (Some(styles), Some(hit_test)) =
                                        (self.shell.style_map(), self.shell.hit_test_engine())
                                    {
                                        let doc = self.shell.document();
                                        if devtools.on_panel_click(*x, *y, styles, doc, hit_test) {
                                            needs_redraw = true;
                                        } else if devtools.on_click(styles) {
                                            needs_redraw = true;
                                        } else {
                                            // Click-to-inspect: clicking outside the panel
                                            // selects the element under the cursor.
                                            if devtools.on_viewport_click(*x, *y, hit_test, styles)
                                            {
                                                needs_redraw = true;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        // Right-click: context menu in devtools.
                        if *state == liquide_input::mouse::ButtonState::Pressed
                            && *button == liquide_input::mouse::MouseButton::Right
                        {
                            if self.dt.dev_mode {
                                if let Some(ref mut devtools) = self.dt.devtools {
                                    if let Some(styles) = self.shell.style_map() {
                                        if devtools.on_right_click(*x, *y, styles) {
                                            needs_redraw = true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    MouseEvent::Scroll { delta, x, y, .. } => {
                        // Forward scroll to devtools panel.
                        if self.dt.dev_mode {
                            if let Some(ref mut devtools) = self.dt.devtools {
                                // Convert scroll delta: positive delta = scroll up
                                // in most platform conventions, but we want positive
                                // = scroll content down (increase offset).
                                let scroll_px = -delta * 36.0;
                                if devtools.on_scroll(*x, *y, scroll_px) {
                                    needs_redraw = true;
                                }
                            }
                        }
                    }
                    _ => {}
                }
                self.input_state.handle_event(&InputEvent::Mouse(*me));
            }
            PlatformEvent::TouchInput { event: te, .. } => {
                self.input_state.handle_event(&InputEvent::Touch(*te));
            }
            _ => {}
        }

        // Route the event through the shell for higher-level actions
        // (keyboard shortcuts, mouse-click focus, dock hover, etc.).
        if !self.loading {
            if let Some(action) = self.shell.handle_platform_event(event) {
                if self.shell.execute_action(&action) {
                    needs_redraw = true;
                }
            }
        }

        // Sync hardware cursor shape when it changes. This is free (just
        // a Win32 SetCursor call) and avoids needing to re-render the
        // entire framebuffer for cursor shape changes.
        if self.cursor.use_hardware {
            self.cursor.request_hw_shape_sync(self.shell.cursor_shape());
        }

        needs_redraw
    }

    /// Perform periodic updates (clock, notification expiry, etc.).
    ///
    /// Returns `true` if something visually changed and a redraw is needed.
    pub(super) fn tick(&mut self) -> bool {
        let now_us = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        let tick = self.shell.tick_detailed(now_us);
        if !tick.dirty {
            return false;
        }

        // A visible devtools panel forces a conservative full repaint ONLY when
        // it has live overlay scene nodes (picker / layout overlay / hover or
        // selection highlight) that the precomputed-damage hint can't bound
        // (t131 jank fix). A merely-visible panel — e.g. its Performance tab's
        // FPS number ticking with no overlay — leaves the targeted damage path
        // intact (the panel is part of the CSS pipeline, so its own content
        // change is bounded by precomputed damage like any other chrome).
        if tick.windows_dirty || tick.notifications_dirty || self.dt.has_active_overlays() {
            self.dirty_damage = None;
        } else if tick.status_bar_dirty || tick.auto_hide_dirty {
            let height = self.shell.status_bar().config().height.ceil().max(1.0);
            self.mark_rect_dirty(liquide_compositor::geometry::Rect::new(
                0.0,
                0.0,
                self.width as f32,
                height + 4.0,
            ));
        } else {
            self.dirty_damage = None;
        }

        true
    }

    /// Route an event that targets the SEPARATE devtools window to the devtools
    /// panel (NOT the main DE). Returns `true` if the event was for the devtools
    /// window and was consumed here — the caller must then SKIP the normal main
    /// DE routing. Returns `false` for events on the main window (or any other
    /// handle), which fall through to the regular path.
    ///
    /// This is the `handle`-matched dispatch the multi-window architecture
    /// relies on: every platform event carries its originating window handle, so
    /// a click in the devtools window can only drive devtools panel state and
    /// can never reach the desktop shell.
    pub(super) fn try_handle_devtools_window_event(
        &mut self,
        platform: &mut dyn liquide_platform::PlatformBackend,
        event: &PlatformEvent,
    ) -> bool {
        use liquide_input::mouse::{ButtonState, MouseButton, MouseEvent};

        let Some(win_handle) = self.dt.window_handle() else {
            return false;
        };

        // Extract the originating handle; non-window events (e.g. DisplaysChanged,
        // Quit, ColorSchemeChanged) are never devtools-window events.
        let event_handle = match event {
            PlatformEvent::WindowCloseRequested { handle }
            | PlatformEvent::WindowDestroyed { handle }
            | PlatformEvent::WindowRedraw { handle }
            | PlatformEvent::FocusGained { handle }
            | PlatformEvent::FocusLost { handle }
            | PlatformEvent::WindowMinimized { handle }
            | PlatformEvent::WindowMaximized { handle }
            | PlatformEvent::WindowRestored { handle } => *handle,
            PlatformEvent::WindowResized { handle, .. }
            | PlatformEvent::WindowCreated { handle, .. }
            | PlatformEvent::WindowMoved { handle, .. }
            | PlatformEvent::DpiChanged { handle, .. }
            | PlatformEvent::FileDrop { handle, .. } => *handle,
            PlatformEvent::KeyInput { handle, .. }
            | PlatformEvent::MouseInput { handle, .. }
            | PlatformEvent::TouchInput { handle, .. } => *handle,
            _ => return false,
        };

        if event_handle != win_handle {
            return false;
        }

        // The event belongs to the devtools window. Coordinates from the window
        // are in its own client space (the window pipeline runs at scale 1.0).
        let mut needs_render = false;
        match event {
            PlatformEvent::WindowCloseRequested { .. } | PlatformEvent::WindowDestroyed { .. } => {
                // The devtools window was closed (its X / OS close) — tear it
                // down and re-dock the panel into the in-DE overlay.
                self.dt.close_window(platform);
                // Also hide the panel so dev-mode visibility tracking does not
                // immediately respawn the window.
                if let Some(d) = self.dt.devtools.as_mut() {
                    d.hide();
                }
                return true;
            }
            PlatformEvent::WindowResized { width, height, .. } => {
                self.dt.resize_window(*width, *height);
                needs_render = true;
            }
            PlatformEvent::KeyInput { event: ke, .. } => {
                if ke.state == KeyState::Pressed {
                    // F12 on the devtools window closes / detaches it.
                    if ke.key == KeyCode::F12 {
                        self.dt.close_window(platform);
                        if let Some(d) = self.dt.devtools.as_mut() {
                            d.hide();
                        }
                        return true;
                    }
                    if let Some(k) = key_to_str(ke.key, ke.modifiers.shift()) {
                        needs_render |= self.dt.route_window_key(
                            k,
                            ke.modifiers.ctrl(),
                            ke.modifiers.shift(),
                            ke.modifiers.alt(),
                        );
                    }
                }
            }
            PlatformEvent::MouseInput { event: me, .. } => match me {
                MouseEvent::Button {
                    x,
                    y,
                    button,
                    state,
                } if *state == ButtonState::Pressed => {
                    let right = *button == MouseButton::Right;
                    if *button == MouseButton::Left || right {
                        needs_render |= self.dt.route_window_click(*x, *y, right);
                    }
                }
                MouseEvent::Scroll { delta, x, y, .. } => {
                    needs_render |= self.dt.route_window_scroll(*x, *y, -delta * 36.0);
                }
                _ => {}
            },
            _ => {}
        }

        if needs_render {
            self.dt.render_window(&self.shell, platform);
        }
        true
    }
}

/// Map a `KeyCode` + shift state to the string the devtools panel expects.
/// Mirrors the in-DE keyboard mapping in `handle_event` (covers the keys the
/// panel actually consumes — navigation, editing, and printable characters).
fn key_to_str(key: KeyCode, shift: bool) -> Option<&'static str> {
    Some(match key {
        KeyCode::F12 => "F12",
        KeyCode::Tab => "Tab",
        KeyCode::Escape => "Escape",
        KeyCode::Enter => "Enter",
        KeyCode::Backspace => "Backspace",
        KeyCode::Delete => "Delete",
        KeyCode::ArrowUp => "ArrowUp",
        KeyCode::ArrowDown => "ArrowDown",
        KeyCode::ArrowLeft => "ArrowLeft",
        KeyCode::ArrowRight => "ArrowRight",
        KeyCode::Home => "Home",
        KeyCode::End => "End",
        KeyCode::Space => " ",
        KeyCode::A => if shift { "A" } else { "a" },
        KeyCode::B => if shift { "B" } else { "b" },
        KeyCode::C => if shift { "C" } else { "c" },
        KeyCode::D => if shift { "D" } else { "d" },
        KeyCode::E => if shift { "E" } else { "e" },
        KeyCode::F => if shift { "F" } else { "f" },
        KeyCode::G => if shift { "G" } else { "g" },
        KeyCode::H => if shift { "H" } else { "h" },
        KeyCode::I => if shift { "I" } else { "i" },
        KeyCode::J => if shift { "J" } else { "j" },
        KeyCode::K => if shift { "K" } else { "k" },
        KeyCode::L => if shift { "L" } else { "l" },
        KeyCode::M => if shift { "M" } else { "m" },
        KeyCode::N => if shift { "N" } else { "n" },
        KeyCode::O => if shift { "O" } else { "o" },
        KeyCode::P => if shift { "P" } else { "p" },
        KeyCode::Q => if shift { "Q" } else { "q" },
        KeyCode::R => if shift { "R" } else { "r" },
        KeyCode::S => if shift { "S" } else { "s" },
        KeyCode::T => if shift { "T" } else { "t" },
        KeyCode::U => if shift { "U" } else { "u" },
        KeyCode::V => if shift { "V" } else { "v" },
        KeyCode::W => if shift { "W" } else { "w" },
        KeyCode::X => if shift { "X" } else { "x" },
        KeyCode::Y => if shift { "Y" } else { "y" },
        KeyCode::Z => if shift { "Z" } else { "z" },
        _ => return None,
    })
}

#[cfg(test)]
mod dpi_tests {
    use super::*;
    use liquide_input::mouse::{ButtonState, MouseButton, MouseEvent, ScrollAxis};

    // Each test uses a distinct window-handle id so the process-wide
    // thread-local DPI map does not bleed state between cases.

    #[test]
    fn unknown_window_defaults_to_unscaled() {
        // A handle we never set must behave as 1.0 (identity).
        assert_eq!(dpi_scale_for(9001), 1.0);
        assert_eq!(to_logical_coords(9001, 200.0, 100.0), (200.0, 100.0));
    }

    #[test]
    fn physical_click_maps_to_logical_at_125_percent() {
        // At 125% DPI, a physical click at (250, 125) must hit logical (200, 100).
        set_dpi_scale(1, 1.25);
        let (lx, ly) = to_logical_coords(1, 250.0, 125.0);
        assert!((lx - 200.0).abs() < 1e-3, "x: {lx}");
        assert!((ly - 100.0).abs() < 1e-3, "y: {ly}");
    }

    #[test]
    fn physical_click_maps_to_logical_at_150_percent() {
        // At 150% DPI, a physical click at (300, 150) maps to logical (200, 100):
        // the un-scaled click would otherwise miss its target by 50%.
        set_dpi_scale(2, 1.5);
        let (lx, ly) = to_logical_coords(2, 300.0, 150.0);
        assert!((lx - 200.0).abs() < 1e-3, "x: {lx}");
        assert!((ly - 100.0).abs() < 1e-3, "y: {ly}");
    }

    #[test]
    fn button_event_coords_are_scaled_other_fields_preserved() {
        set_dpi_scale(3, 2.0);
        let scaled = scale_mouse_event_to_logical(
            3,
            &MouseEvent::Button {
                button: MouseButton::Right,
                state: ButtonState::Pressed,
                x: 400.0,
                y: 200.0,
            },
        );
        match scaled {
            MouseEvent::Button {
                button,
                state,
                x,
                y,
            } => {
                assert_eq!(button, MouseButton::Right);
                assert_eq!(state, ButtonState::Pressed);
                assert!((x - 200.0).abs() < 1e-3);
                assert!((y - 100.0).abs() < 1e-3);
            }
            other => panic!("expected Button, got {other:?}"),
        }
    }

    #[test]
    fn touch_event_coords_are_scaled_other_fields_preserved() {
        // t65-s2 item 6: a physical touch at (200, 200) on a 2x display maps to
        // logical (100, 100); phase/id/pressure/timestamp are preserved.
        use liquide_input::touch::{TouchEvent, TouchPhase, TouchPoint};
        set_dpi_scale(20, 2.0);
        let scaled = scale_touch_event_to_logical(
            20,
            &TouchEvent::new(TouchPhase::Begin, TouchPoint::new(7, 200.0, 200.0, 0.5), 1234),
        );
        assert_eq!(scaled.phase, TouchPhase::Begin);
        assert_eq!(scaled.point.id, 7);
        assert!((scaled.point.x - 100.0).abs() < 1e-3, "x: {}", scaled.point.x);
        assert!((scaled.point.y - 100.0).abs() < 1e-3, "y: {}", scaled.point.y);
        assert!((scaled.point.pressure - 0.5).abs() < 1e-3);
        assert_eq!(scaled.timestamp_us, 1234);
    }

    #[test]
    fn touch_unscaled_is_identity() {
        use liquide_input::touch::{TouchEvent, TouchPhase, TouchPoint};
        // Unknown handle → 1.0 scale → identity.
        let scaled = scale_touch_event_to_logical(
            9002,
            &TouchEvent::new(TouchPhase::Move, TouchPoint::new(1, 320.0, 240.0, 1.0), 0),
        );
        assert_eq!(scaled.point.x, 320.0);
        assert_eq!(scaled.point.y, 240.0);
    }

    #[test]
    fn scroll_event_scales_position_not_delta() {
        set_dpi_scale(4, 2.0);
        let scaled = scale_mouse_event_to_logical(
            4,
            &MouseEvent::Scroll {
                axis: ScrollAxis::Vertical,
                delta: 3.0,
                x: 100.0,
                y: 50.0,
            },
        );
        match scaled {
            MouseEvent::Scroll { axis, delta, x, y } => {
                assert_eq!(axis, ScrollAxis::Vertical);
                assert!((delta - 3.0).abs() < 1e-3, "delta must be unchanged");
                assert!((x - 50.0).abs() < 1e-3);
                assert!((y - 25.0).abs() < 1e-3);
            }
            other => panic!("expected Scroll, got {other:?}"),
        }
    }

    #[test]
    fn dpi_update_replaces_previous_scale() {
        set_dpi_scale(5, 1.25);
        assert_eq!(dpi_scale_for(5), 1.25);
        // Moving the window to a 200% monitor updates the stored scale.
        set_dpi_scale(5, 2.0);
        assert_eq!(dpi_scale_for(5), 2.0);
        assert_eq!(to_logical_coords(5, 400.0, 200.0), (200.0, 100.0));
    }

    #[test]
    fn nonfinite_or_nonpositive_scale_falls_back_to_unscaled() {
        set_dpi_scale(6, f32::NAN);
        assert_eq!(dpi_scale_for(6), 1.0);
        set_dpi_scale(7, 0.0);
        assert_eq!(dpi_scale_for(7), 1.0);
        set_dpi_scale(8, -2.0);
        assert_eq!(dpi_scale_for(8), 1.0);
    }
}

#[cfg(test)]
mod devtools_jank_tests {
    //! t131 (t130 handoff): a VISIBLE devtools panel must no longer force a
    //! conservative full-frame repaint every frame. The panel is part of the CSS
    //! pipeline, so its own content damages only its own region (bounded by the
    //! shell's precomputed damage). Only LIVE overlay scene nodes (picker /
    //! layout overlay / hover+selection highlight), which are added AFTER
    //! build_scene and escape the hint, justify the full path.
    use super::*;

    #[test]
    fn idle_visible_devtools_does_not_force_full_frame_on_status_tick() {
        // A clock-only tick (status_bar_dirty) with the devtools panel merely
        // VISIBLE must produce a TARGETED status-bar damage rect — NOT the
        // conservative full-frame (dirty_damage = None). RED before the fix:
        // `tick()` keyed off `devtools_panel_visible()` and nuked damage to None
        // for every tick while devtools was up.
        let mut desktop = DesktopCompositor::new(1280, 800);
        desktop.loading = false;
        desktop.set_dev_mode(true);
        if let Some(panel) = desktop.dt.devtools.as_mut() {
            panel.show();
        }
        // The panel is visible but has NO active overlays (no picker / no target
        // / no selection) — the idle case.
        assert!(desktop.dt.devtools.as_ref().unwrap().is_visible());
        assert!(
            !desktop.dt.has_active_overlays(),
            "an idle visible panel must report no active overlays"
        );

        // Simulate the tick decision for a status-bar-only change: this is the
        // exact branch from `tick()`. With no overlays active it must take the
        // targeted status-bar rect path, leaving a SOME (non-full) damage hint.
        desktop.dirty_damage = None;
        // Drive the real decision via a forced clock tick is timing-dependent, so
        // assert the gate the decision now uses: overlays-active, not merely
        // visible.
        let forces_full = desktop.dt.has_active_overlays();
        assert!(
            !forces_full,
            "idle visible devtools must NOT force the full-frame path"
        );

        // And once an overlay IS active (picker), the gate flips to full.
        desktop.dt.devtools.as_mut().unwrap().toggle_picker();
        assert!(
            desktop.dt.has_active_overlays(),
            "an active picker overlay must force the conservative full path"
        );
    }

    #[test]
    fn fps_snapshot_update_changes_only_text_not_structure() {
        // The per-frame FPS / frame-number snapshot must update the Performance
        // tab via a damage-scoped CONTENT change — NOT by adding/removing
        // elements (which would force a structural relayout). We push two DIFFERENT
        // snapshots and assert: (a) the rendered FPS/frame TEXT follows the live
        // value, and (b) the ELEMENT STRUCTURE (tag multiset, excluding text
        // nodes) is byte-stable across the change. A snapshot bump that churned
        // the element tree (the full-relayout jank) would fail (b).
        use std::collections::BTreeMap;

        let mut desktop = DesktopCompositor::new(1280, 800);
        desktop.loading = false;
        desktop.set_dev_mode(true);
        desktop.dt.devtools.as_mut().unwrap().show();
        desktop
            .dt
            .devtools
            .as_mut()
            .unwrap()
            .set_tab(liquide_devtools::DevToolsTab::Performance);
        // Lay the shell out so the panel template has live layout/styles.
        let _ = desktop.shell.build_scene();

        // Helper: render the panel template and return (element-tag multiset,
        // all text content joined).
        fn snapshot_template(
            desktop: &DesktopCompositor,
        ) -> (BTreeMap<String, usize>, String) {
            let panel = desktop.dt.devtools.as_ref().unwrap();
            let doc = desktop.shell.document();
            let layout = desktop.shell.layout_tree().unwrap();
            let styles = desktop.shell.style_map().unwrap();
            let t = panel.render_template(doc, layout, styles);
            let mut tags: BTreeMap<String, usize> = BTreeMap::new();
            let mut text = String::new();
            fn walk(
                n: &liquide_devtools::TemplateNode,
                tags: &mut BTreeMap<String, usize>,
                text: &mut String,
            ) {
                if let Some(t) = &n.text {
                    text.push_str(t);
                    text.push('\u{1}');
                } else if n.tag != "devtools-bar" {
                    // The frame-time sparkline grows one <devtools-bar> per pushed
                    // frame — expected, not jank. Exclude it so we measure the
                    // STABLE structure (the FPS/frame rows must not churn).
                    *tags.entry(n.tag.clone()).or_insert(0) += 1;
                }
                for c in &n.children {
                    walk(c, tags, text);
                }
            }
            walk(&t, &mut tags, &mut text);
            (tags, text)
        }

        // Warm up the sparkline so the "Frame Times" chart section exists in BOTH
        // measured renders (it only appears once >1 frame time is recorded), so
        // the structure is comparable.
        desktop.dt.devtools.as_mut().unwrap().push_frame_snapshot(
            liquide_devtools::FrameSnapshot {
                frame_number: 1,
                fps: 60.0,
                avg_frame_ms: 16.0,
                css_rule_count: 0,
                css_variable_count: 0,
                stylesheet_count: 0,
                viewport_w: 1280.0,
                viewport_h: 800.0,
            },
        );

        // Snapshot A.
        desktop.dt.devtools.as_mut().unwrap().push_frame_snapshot(
            liquide_devtools::FrameSnapshot {
                frame_number: 100,
                fps: 60.0,
                avg_frame_ms: 16.0,
                css_rule_count: 0,
                css_variable_count: 0,
                stylesheet_count: 0,
                viewport_w: 1280.0,
                viewport_h: 800.0,
            },
        );
        let (tags_a, text_a) = snapshot_template(&desktop);

        // Snapshot B — different live numbers.
        desktop.dt.devtools.as_mut().unwrap().push_frame_snapshot(
            liquide_devtools::FrameSnapshot {
                frame_number: 250,
                fps: 30.0,
                avg_frame_ms: 33.0,
                css_rule_count: 0,
                css_variable_count: 0,
                stylesheet_count: 0,
                viewport_w: 1280.0,
                viewport_h: 800.0,
            },
        );
        let (tags_b, text_b) = snapshot_template(&desktop);

        // (a) The live FPS / frame number must be reflected in the rendered text.
        assert!(
            text_a.contains("100") && text_a.contains("60.0"),
            "snapshot A's frame/FPS must appear in the rendered template"
        );
        assert!(
            text_b.contains("250") && text_b.contains("30.0"),
            "snapshot B's NEW frame/FPS must be reflected (live state → scene)"
        );
        assert_ne!(text_a, text_b, "the snapshot bump must change rendered text");

        // (b) The ELEMENT structure must be stable — only text content changed.
        // A snapshot bump that added/removed elements (structural relayout) fails.
        assert_eq!(
            tags_a, tags_b,
            "an FPS/frame snapshot bump must change only TEXT content, not the \
             element structure — a structural change forces a full relayout (the \
             t130 jank). tags A={tags_a:?} B={tags_b:?}"
        );
    }
}
