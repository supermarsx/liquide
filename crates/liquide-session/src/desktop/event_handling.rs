//! Platform event routing — keyboard, mouse, touch, and window events.

use std::time::{SystemTime, UNIX_EPOCH};

use liquide_input::event::InputEvent;
use liquide_input::keyboard::{KeyCode, KeyState};
use liquide_platform::PlatformEvent;

use super::{DesktopCompositor, RenderMsg};

impl DesktopCompositor {
    /// Handle a platform event: route through shell and input state.
    ///
    /// Returns `true` if the event requires a redraw.
    pub fn handle_event(&mut self, event: &PlatformEvent) -> bool {
        let mut needs_redraw = false;

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
                if let Some(ref mut devtools) = self.devtools {
                    devtools.set_screen_size(*width as f32, *height as f32);
                }
                needs_redraw = true;
            }
            PlatformEvent::WindowCloseRequested { .. } | PlatformEvent::Quit => {
                self.running = false;
            }
            PlatformEvent::WindowRedraw { .. } => {
                needs_redraw = true;
            }
            PlatformEvent::KeyInput { event: ke, .. } => {
                // DevTools keyboard shortcuts (intercept before shell).
                if self.dev_mode {
                    if let Some(ref mut devtools) = self.devtools {
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
            PlatformEvent::MouseInput { event: me, .. } => {
                // Track cursor position for software cursor rendering.
                use liquide_input::mouse::MouseEvent;
                match me {
                    MouseEvent::Move { x, y } => {
                        // Only redraw if cursor position actually changed
                        // (avoid redundant full redraws on minor sub-pixel jitter).
                        let new_x = *x;
                        let new_y = *y;
                        if (new_x - self.cursor_x).abs() > 0.1
                            || (new_y - self.cursor_y).abs() > 0.1
                        {
                            self.cursor_x = new_x;
                            self.cursor_y = new_y;
                            needs_redraw = true;
                        }
                        // Forward to devtools element picker.
                        if self.dev_mode {
                            if let Some(ref mut devtools) = self.devtools {
                                if let (Some(hit_test), Some(layout)) =
                                    (self.shell.hit_test_engine(), self.shell.layout_tree())
                                {
                                    let doc = self.shell.document();
                                    if devtools.on_mouse_move(new_x, new_y, hit_test, doc, layout) {
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
                        self.cursor_x = *x;
                        self.cursor_y = *y;
                        // Only react on button press, not release.
                        if *state == liquide_input::mouse::ButtonState::Pressed
                            && *button == liquide_input::mouse::MouseButton::Left
                        {
                            // Forward click to devtools panel (tabs, tree nodes, etc.)
                            // and element picker / viewport click-to-inspect.
                            if self.dev_mode {
                                if let Some(ref mut devtools) = self.devtools {
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
                            if self.dev_mode {
                                if let Some(ref mut devtools) = self.devtools {
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
                        if self.dev_mode {
                            if let Some(ref mut devtools) = self.devtools {
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
        self.shell.tick(now_us)
    }
}
