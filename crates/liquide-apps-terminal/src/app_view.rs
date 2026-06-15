//! Implementation of the shell↔app seam ([`liquide_interop::AppView`]) for the
//! terminal, plus the real content view that replaces the `Label` placeholder.

use liquide_interop::{
    AppContentProvider, AppContentView, AppKey, AppTextInput, AppView, ContentKind, ContentRow,
    ContentSpan,
};

use crate::runtime::{TerminalRuntime, TextSpan};

/// Map a terminal cell palette/RGB span to a packed `0xRRGGBBAA` color.
fn span_color(span: &TextSpan) -> Option<u32> {
    if let Some((r, g, b)) = span.fg_rgb {
        return Some(u32::from_be_bytes([r, g, b, 0xFF]));
    }
    // Map the 16-color ANSI palette index to an approximate RGB so the shell
    // can paint colored output even without a full palette table.
    let idx = span.fg?;
    let (r, g, b) = ansi_palette_rgb(idx);
    Some(u32::from_be_bytes([r, g, b, 0xFF]))
}

/// Approximate RGB for the low ANSI palette indices (0-15). Higher indices fall
/// back to a neutral light gray so text remains visible.
fn ansi_palette_rgb(idx: u8) -> (u8, u8, u8) {
    match idx {
        0 => (0x00, 0x00, 0x00),
        1 => (0xCD, 0x00, 0x00),
        2 => (0x00, 0xCD, 0x00),
        3 => (0xCD, 0xCD, 0x00),
        4 => (0x00, 0x00, 0xEE),
        5 => (0xCD, 0x00, 0xCD),
        6 => (0x00, 0xCD, 0xCD),
        7 => (0xE5, 0xE5, 0xE5),
        8 => (0x7F, 0x7F, 0x7F),
        9 => (0xFF, 0x00, 0x00),
        10 => (0x00, 0xFF, 0x00),
        11 => (0xFF, 0xFF, 0x00),
        12 => (0x5C, 0x5C, 0xFF),
        13 => (0xFF, 0x00, 0xFF),
        14 => (0x00, 0xFF, 0xFF),
        15 => (0xFF, 0xFF, 0xFF),
        _ => (0xD0, 0xD0, 0xD0),
    }
}

impl AppTextInput for TerminalRuntime {
    fn handle_text(&mut self, text: &str) -> bool {
        let mut changed = false;
        for ch in text.chars() {
            if self.send_char(ch).is_ok() {
                changed = true;
            }
        }
        changed
    }

    fn handle_key(&mut self, key: &AppKey) -> bool {
        match key {
            AppKey::Char(c) => self.send_char(*c).is_ok(),
            // Named/navigation keys map onto the terminal escape-sequence
            // protocol via `send_key`.
            other => self.send_key(other.name()).is_ok(),
        }
    }
}

impl AppContentProvider for TerminalRuntime {
    fn content_view(&self, _cols: u32, _rows: u32) -> AppContentView {
        let mut view = AppContentView::new(ContentKind::Terminal);
        for line in self.visible_lines() {
            let spans = line
                .spans
                .iter()
                .map(|s| ContentSpan {
                    start_col: s.start,
                    end_col: s.end,
                    color: span_color(s),
                    bold: s.bold,
                })
                .collect();
            view.rows.push(ContentRow {
                text: line.text,
                spans,
                gutter: None,
                active: false,
            });
        }
        let (row, col) = self.cursor_position();
        view.cursor = Some((row, col));
        view
    }
}

impl AppView for TerminalRuntime {
    fn app_id(&self) -> &str {
        crate::TERMINAL_APP_ID
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TerminalConfig;

    /// Build a runtime backed by a stub PTY so input/render is deterministic
    /// and offline. The stub PTY echoes writes back through its read buffer.
    fn stub_runtime() -> TerminalRuntime {
        let mut rt = TerminalRuntime::new(TerminalConfig::default());
        rt.new_stub_tab(None).expect("stub tab");
        rt
    }

    #[test]
    fn content_view_is_non_placeholder_grid() {
        let rt = stub_runtime();
        let view: &dyn AppView = &rt;
        let content = view.content_view(80, 24);
        assert_eq!(content.kind, ContentKind::Terminal);
        // A grid view has one row per terminal line — far more than a Label.
        assert!(content.rows.len() >= 2, "expected full grid, got {}", content.rows.len());
        assert!(content.cursor.is_some());
    }

    #[test]
    fn typed_text_routes_into_grid_via_echo() {
        let mut rt = stub_runtime();
        // Stub PTY echoes input back; route text then drain it into the grid.
        assert!(rt.handle_text("echo"));
        // Pump the echoed bytes through the VT parser.
        for _ in 0..8 {
            rt.tick();
        }
        let joined: String = rt
            .content_view(80, 24)
            .rows
            .iter()
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>()
            .join("");
        assert!(joined.contains("echo"), "typed text not visible in grid: {joined:?}");
    }

    #[test]
    fn key_routes_through_send_key() {
        let mut rt = stub_runtime();
        // Enter is delivered as a CR escape via send_key — should not error.
        assert!(rt.handle_key(&AppKey::Enter));
        assert!(rt.handle_key(&AppKey::Char('x')));
    }
}
