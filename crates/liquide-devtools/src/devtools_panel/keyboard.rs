//! Keyboard event handling for the DevTools panel.

use std::time::Instant;

use liquide_dom::Document;
use liquide_layout::tree::LayoutTree;
use liquide_style_engine::StyleMap;

use super::{DevToolsPanel, DevToolsTab};

impl DevToolsPanel {
    // ─── Keyboard shortcuts ───────────────────────────────────

    /// Process a key event. Returns `true` if the event was handled.
    ///
    /// Supported shortcuts:
    /// - F12: Toggle devtools panel
    /// - Ctrl+Shift+C: Toggle element picker
    /// - Ctrl+Shift+I: Toggle devtools panel
    /// - Tab (when devtools focused): Cycle tabs
    pub fn handle_key(&mut self, key: &str, ctrl: bool, shift: bool, _alt: bool) -> bool {
        // Escape always closes context menu first.
        if key == "Escape" && self.context_menu.is_visible() {
            self.context_menu.hide();
            return true;
        }

        // F12 always toggles devtools.
        match key {
            "F12" => {
                self.toggle();
                return true;
            }
            "I" | "i" if ctrl && shift => {
                self.toggle();
                return true;
            }
            "C" | "c" if ctrl && shift => {
                if !self.visible {
                    self.show();
                }
                self.toggle_picker();
                return true;
            }
            _ => {}
        }

        // If the style editor is actively editing a property, route keys there.
        if self.style_editor.editing_property().is_some() {
            match key {
                "Escape" => {
                    self.style_editor.cancel_edit();
                    return true;
                }
                "Enter" | "Return" => {
                    // Confirm edit — returns a StyleEdit that we should apply.
                    if let Some(edit) = self.style_editor.confirm_edit() {
                        // Queue the edit — the host will apply it via
                        // apply_pending_style_edits() on the next frame.
                        self.style_edit_queue.push(edit);
                    }
                    return true;
                }
                "Backspace" => {
                    self.style_editor.backspace();
                    return true;
                }
                "ArrowLeft" | "Left" => {
                    self.style_editor.cursor_left();
                    return true;
                }
                "ArrowRight" | "Right" => {
                    self.style_editor.cursor_right();
                    return true;
                }
                "Tab" => {
                    // Confirm current and move focus to next property (if any).
                    if let Some(edit) = self.style_editor.confirm_edit() {
                        self.style_edit_queue.push(edit);
                    }
                    return true;
                }
                _ if key.len() == 1 && !ctrl => {
                    if let Some(c) = key.chars().next() {
                        self.style_editor.insert_char(c);
                        // Auto-apply: queue the edit immediately if enabled.
                        if self.style_editor.auto_apply() {
                            if let (Some(node_id), Some(prop)) = (
                                self.style_editor.target(),
                                self.style_editor.editing_property().map(|s| s.to_string()),
                            ) {
                                let value = self.style_editor.editing_value().to_string();
                                self.style_edit_queue.push(crate::style_editor::StyleEdit {
                                    node_id,
                                    property: prop,
                                    original_value: String::new(),
                                    new_value: value,
                                    applied: false,
                                });
                            }
                        }
                    }
                    return true;
                }
                _ => {}
            }
        }

        // If console is focused, route keys there (except global shortcuts above).
        if self.console_focused && self.active_tab == DevToolsTab::Console {
            // Any keystroke resets the caret blink so it stays solid while typing.
            let reset_blink = |s: &mut Self| {
                s.caret_blink_epoch = Instant::now();
            };
            match key {
                "Escape" => {
                    self.console_focused = false;
                    return true;
                }
                "Enter" | "Return" => {
                    // Submit will need doc/layout/styles — handled by the desktop layer.
                    // For now, we mark it as consumed and the desktop
                    // will call handle_console_key with context.
                    reset_blink(self);
                    return true;
                }
                "Backspace" => {
                    self.console.backspace();
                    reset_blink(self);
                    return true;
                }
                "Delete" => {
                    self.console.delete();
                    reset_blink(self);
                    return true;
                }
                "ArrowLeft" | "Left" => {
                    self.console.cursor_left();
                    reset_blink(self);
                    return true;
                }
                "ArrowRight" | "Right" => {
                    self.console.cursor_right();
                    reset_blink(self);
                    return true;
                }
                "ArrowUp" | "Up" => {
                    self.console.history_up();
                    reset_blink(self);
                    return true;
                }
                "ArrowDown" | "Down" => {
                    self.console.history_down();
                    reset_blink(self);
                    return true;
                }
                "Home" => {
                    self.console.cursor_home();
                    reset_blink(self);
                    return true;
                }
                "End" => {
                    self.console.cursor_end();
                    reset_blink(self);
                    return true;
                }
                _ if key.len() == 1 && !ctrl => {
                    if let Some(c) = key.chars().next() {
                        self.console.insert_char(c);
                    }
                    reset_blink(self);
                    return true;
                }
                _ => {}
            }
        }

        if self.visible && !ctrl && !shift && key == "Tab" {
            self.next_tab();
            return true;
        }

        false
    }

    /// Handle a keyboard event when the console is focused.
    pub fn handle_console_key(
        &mut self,
        key: &str,
        _ctrl: bool,
        _shift: bool,
        doc: &Document,
        layout: &LayoutTree,
        styles: &StyleMap,
    ) -> bool {
        if !self.console_focused {
            return false;
        }

        match key {
            "Escape" => {
                self.console_focused = false;
                true
            }
            "Enter" | "Return" => {
                self.console.submit(doc, layout, styles);
                true
            }
            "Backspace" => {
                self.console.backspace();
                true
            }
            "Delete" => {
                self.console.delete();
                true
            }
            "ArrowLeft" | "Left" => {
                self.console.cursor_left();
                true
            }
            "ArrowRight" | "Right" => {
                self.console.cursor_right();
                true
            }
            "ArrowUp" | "Up" => {
                self.console.history_up();
                true
            }
            "ArrowDown" | "Down" => {
                self.console.history_down();
                true
            }
            "Home" => {
                self.console.cursor_home();
                true
            }
            "End" => {
                self.console.cursor_end();
                true
            }
            _ if key.len() == 1 => {
                if let Some(c) = key.chars().next() {
                    self.console.insert_char(c);
                }
                true
            }
            _ => false,
        }
    }
}
