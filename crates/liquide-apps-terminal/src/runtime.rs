//! Terminal runtime coordinator.

use crate::config::TerminalConfig;
use crate::grid::CellAttrs;
use crate::grid::Grid;
use crate::search::SearchState;
use crate::tab::TabManager;
use crate::vt::{Action, CsiAction, EraseMode, OscAction, Parser, SgrParam};

/// Central coordinator for the terminal emulator.
pub struct TerminalRuntime {
    config: TerminalConfig,
    tabs: TabManager,
    parser: Parser,
    search: SearchState,
}

impl TerminalRuntime {
    fn set_tab_title(&mut self, id: u32, title: Option<String>) {
        if let Some(title) = title {
            if let Some(tab) = self.tabs.get_mut(id) {
                tab.set_title(title);
            }
        }
    }

    /// Create a new terminal runtime.
    #[must_use]
    pub fn new(config: TerminalConfig) -> Self {
        Self {
            config,
            tabs: TabManager::new(),
            parser: Parser::new(),
            search: SearchState::new(),
        }
    }

    /// Get the config.
    #[must_use]
    pub fn config(&self) -> &TerminalConfig {
        &self.config
    }

    /// Create a new tab, returning its ID.
    pub fn new_tab(&mut self, title: Option<String>) -> crate::Result<u32> {
        let id = self.tabs.new_tab(
            &self.config.shell,
            self.config.rows,
            self.config.cols,
            self.config.scrollback_lines,
        )?;
        self.set_tab_title(id, title);
        Ok(id)
    }

    #[cfg(test)]
    pub(crate) fn new_stub_tab(&mut self, title: Option<String>) -> crate::Result<u32> {
        let id = self.tabs.new_stub_tab(
            self.config.rows,
            self.config.cols,
            self.config.scrollback_lines,
        )?;
        self.set_tab_title(id, title);
        Ok(id)
    }

    /// Close a tab.
    pub fn close_tab(&mut self, id: u32) -> crate::Result<()> {
        self.tabs.close_tab(id)
    }

    /// Set active tab.
    pub fn set_active_tab(&mut self, id: u32) -> crate::Result<()> {
        self.tabs.set_active(id)
    }

    /// Tab count.
    #[must_use]
    pub fn tab_count(&self) -> usize {
        self.tabs.count()
    }

    /// List tabs.
    #[must_use]
    pub fn tab_list(&self) -> Vec<(u32, String)> {
        self.tabs.list()
    }

    /// Get the active grid (read-only).
    #[must_use]
    pub fn active_grid(&self) -> &Grid {
        self.tabs
            .active_tab()
            .map(|t| t.grid())
            .expect("no active tab")
    }

    /// Feed raw PTY output into the parser and apply to the active tab.
    pub fn process_output(&mut self, data: &[u8]) {
        let mut actions = Vec::new();
        self.parser.feed(data, &mut actions);

        for action in actions {
            self.apply_action(action);
        }
    }

    /// Send keyboard input to the active PTY.
    pub fn send_input(&mut self, data: &[u8]) -> crate::Result<()> {
        let tab = self
            .tabs
            .active_tab_mut()
            .ok_or(crate::TerminalError::TabNotFound { id: 0 })?;
        tab.pty_mut().write(data)
    }

    /// Resize the active tab.
    pub fn resize(&mut self, rows: u32, cols: u32) {
        if let Some(tab) = self.tabs.active_tab_mut() {
            tab.resize(rows, cols);
        }
    }

    /// Get the search state.
    #[must_use]
    pub fn search(&self) -> &SearchState {
        &self.search
    }

    /// Search mutable access.
    pub fn search_mut(&mut self) -> &mut SearchState {
        &mut self.search
    }

    fn apply_action(&mut self, action: Action) {
        let Some(tab) = self.tabs.active_tab_mut() else {
            return;
        };
        let grid = tab.grid_mut();

        match action {
            Action::Print(ch) => {
                grid.put_char(ch);
            }
            Action::Execute(byte) => match byte {
                0x08 => grid.cursor_back(1),            // BS
                0x09 => grid.cursor_tab(),              // HT
                0x0a | 0x0b | 0x0c => grid.line_feed(), // LF/VT/FF
                0x0d => grid.carriage_return(),         // CR
                _ => {}
            },
            Action::CsiDispatch(csi) => self.apply_csi(csi),
            Action::OscDispatch(osc) => self.apply_osc(osc),
            Action::EscDispatch(_) => {}
        }
    }

    fn apply_csi(&mut self, csi: CsiAction) {
        let Some(tab) = self.tabs.active_tab_mut() else {
            return;
        };
        let grid = tab.grid_mut();

        match csi {
            CsiAction::CursorUp(n) => grid.cursor_up(n),
            CsiAction::CursorDown(n) => grid.cursor_down(n),
            CsiAction::CursorForward(n) => grid.cursor_forward(n),
            CsiAction::CursorBack(n) => grid.cursor_back(n),
            CsiAction::CursorPosition { row, col } => {
                grid.set_cursor(row.saturating_sub(1), col.saturating_sub(1));
            }
            CsiAction::EraseDisplay(mode) => match mode {
                EraseMode::ToEnd => grid.erase_display_to_end(),
                EraseMode::ToBeginning => grid.erase_display_to_beginning(),
                EraseMode::All => grid.erase_display_all(),
            },
            CsiAction::EraseLine(mode) => match mode {
                EraseMode::ToEnd => grid.erase_line_to_end(),
                EraseMode::ToBeginning => grid.erase_line_to_beginning(),
                EraseMode::All => grid.erase_line_all(),
            },
            CsiAction::Sgr(params) => {
                let mut attrs = grid.current_attrs();
                for param in params {
                    apply_sgr(&mut attrs, param);
                }
                grid.set_attrs(attrs);
            }
            CsiAction::ScrollUp(n) => {
                let scrolled = grid.scroll_up(n);
                tab.scrollback_mut().push_lines(scrolled);
            }
            CsiAction::ScrollDown(n) => grid.scroll_down(n),
            CsiAction::SetScrollRegion { top, bottom } => grid.set_scroll_region(top, bottom),
            CsiAction::InsertLines(n) => grid.insert_lines(n),
            CsiAction::DeleteLines(n) => grid.delete_lines(n),
            CsiAction::InsertChars(n) => grid.insert_chars(n),
            CsiAction::DeleteChars(n) => grid.delete_chars(n),
            CsiAction::DeviceStatusReport | CsiAction::Unknown(_) => {}
        }
    }

    fn apply_osc(&mut self, osc: OscAction) {
        let Some(tab) = self.tabs.active_tab_mut() else {
            return;
        };
        match osc {
            OscAction::SetTitle(title) => {
                tab.shell_integration_mut().set_title(title);
            }
            OscAction::SetWorkingDirectory(cwd) => {
                tab.shell_integration_mut().set_cwd(cwd);
            }
            OscAction::CommandStart => {
                tab.shell_integration_mut().command_start();
            }
            OscAction::CommandEnd(code) => {
                tab.shell_integration_mut().command_end(code);
            }
            OscAction::Hyperlink { .. } | OscAction::Unknown(_) => {}
        }
    }
}

/// A rendered terminal line with attributed text spans.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderedLine {
    /// The text content of the line.
    pub text: String,
    /// Styled spans within the line.
    pub spans: Vec<TextSpan>,
}

/// A styled span within a rendered line.
#[derive(Debug, Clone, PartialEq)]
pub struct TextSpan {
    /// Start column (inclusive).
    pub start: u32,
    /// End column (exclusive).
    pub end: u32,
    /// Foreground color: palette index.
    pub fg: Option<u8>,
    /// Background color: palette index.
    pub bg: Option<u8>,
    /// Foreground RGB override.
    pub fg_rgb: Option<(u8, u8, u8)>,
    /// Background RGB override.
    pub bg_rgb: Option<(u8, u8, u8)>,
    /// Bold attribute.
    pub bold: bool,
    /// Italic attribute.
    pub italic: bool,
    /// Underline attribute.
    pub underline: bool,
    /// Dim attribute.
    pub dim: bool,
    /// Strikethrough attribute.
    pub strikethrough: bool,
    /// Reverse video attribute.
    pub reverse: bool,
}

impl TextSpan {
    fn from_attrs(start: u32, end: u32, attrs: &CellAttrs) -> Self {
        Self {
            start,
            end,
            fg: attrs.fg,
            bg: attrs.bg,
            fg_rgb: attrs.fg_rgb,
            bg_rgb: attrs.bg_rgb,
            bold: attrs.bold,
            italic: attrs.italic,
            underline: attrs.underline,
            dim: attrs.dim,
            strikethrough: attrs.strikethrough,
            reverse: attrs.reverse,
        }
    }
}

impl TerminalRuntime {
    /// Process one iteration of the terminal event loop.
    ///
    /// Reads available output from the active tab's PTY, feeds it through
    /// the VT parser, and applies the resulting actions to the grid.
    /// Returns `true` if the grid was updated (i.e., needs a redraw).
    pub fn tick(&mut self) -> bool {
        let data = {
            let Some(tab) = self.tabs.active_tab_mut() else {
                return false;
            };
            let output = tab.pty_mut().read();
            if output.is_empty() {
                return false;
            }
            output
        };
        self.process_output(&data);
        true
    }

    /// to the appropriate terminal escape sequences.
    pub fn send_key(&mut self, key: &str) -> crate::Result<()> {
        let bytes: &[u8] = match key {
            "Enter" => b"\r",
            "Backspace" => b"\x7f",
            "Tab" => b"\t",
            "Escape" => b"\x1b",
            "ArrowUp" => b"\x1b[A",
            "ArrowDown" => b"\x1b[B",
            "ArrowRight" => b"\x1b[C",
            "ArrowLeft" => b"\x1b[D",
            "Home" => b"\x1b[H",
            "End" => b"\x1b[F",
            "PageUp" => b"\x1b[5~",
            "PageDown" => b"\x1b[6~",
            "Delete" => b"\x1b[3~",
            "Insert" => b"\x1b[2~",
            "F1" => b"\x1bOP",
            "F2" => b"\x1bOQ",
            "F3" => b"\x1bOR",
            "F4" => b"\x1bOS",
            "F5" => b"\x1b[15~",
            "F6" => b"\x1b[17~",
            "F7" => b"\x1b[18~",
            "F8" => b"\x1b[19~",
            "F9" => b"\x1b[20~",
            "F10" => b"\x1b[21~",
            "F11" => b"\x1b[23~",
            "F12" => b"\x1b[24~",
            other => other.as_bytes(),
        };
        self.send_input(bytes)
    }

    /// Send a single character to the active tab's PTY.
    pub fn send_char(&mut self, ch: char) -> crate::Result<()> {
        let mut buf = [0u8; 4];
        let encoded = ch.encode_utf8(&mut buf);
        self.send_input(encoded.as_bytes())
    }

    /// Get the visible lines for rendering.
    ///
    /// Returns a `RenderedLine` per grid row, containing the text content
    /// and a list of styled spans with attribute changes.
    #[must_use]
    pub fn visible_lines(&self) -> Vec<RenderedLine> {
        let Some(tab) = self.tabs.active_tab() else {
            return Vec::new();
        };
        let grid = tab.grid();
        let rows = grid.rows();
        let cols = grid.cols();
        let mut lines = Vec::with_capacity(rows as usize);

        for row in 0..rows {
            let mut text = String::with_capacity(cols as usize);
            let mut spans = Vec::new();
            let mut current_attrs = CellAttrs::default();
            let mut span_start = 0u32;

            for col in 0..cols {
                let cell = match grid.cell(row, col) {
                    Some(c) => c,
                    None => break,
                };
                if cell.attrs != current_attrs && col > span_start {
                    spans.push(TextSpan::from_attrs(span_start, col, &current_attrs));
                    current_attrs = cell.attrs;
                    span_start = col;
                } else if cell.attrs != current_attrs {
                    current_attrs = cell.attrs;
                    span_start = col;
                }
                text.push(cell.ch);
            }
            // Final span for the remainder of the row.
            if cols > span_start {
                spans.push(TextSpan::from_attrs(span_start, cols, &current_attrs));
            }

            lines.push(RenderedLine { text, spans });
        }
        lines
    }

    /// Current cursor position (row, col) for cursor rendering.
    #[must_use]
    pub fn cursor_position(&self) -> (u32, u32) {
        self.tabs
            .active_tab()
            .map(|t| t.grid().cursor())
            .unwrap_or((0, 0))
    }
}

fn apply_sgr(attrs: &mut CellAttrs, param: SgrParam) {
    match param {
        SgrParam::Reset => *attrs = CellAttrs::default(),
        SgrParam::Bold => attrs.bold = true,
        SgrParam::Dim => attrs.dim = true,
        SgrParam::Italic => attrs.italic = true,
        SgrParam::Underline => attrs.underline = true,
        SgrParam::Blink => attrs.blink = true,
        SgrParam::Reverse => attrs.reverse = true,
        SgrParam::Hidden => attrs.hidden = true,
        SgrParam::Strikethrough => attrs.strikethrough = true,
        SgrParam::Foreground(idx) => {
            attrs.fg = Some(idx);
            attrs.fg_rgb = None;
        }
        SgrParam::Background(idx) => {
            attrs.bg = Some(idx);
            attrs.bg_rgb = None;
        }
        SgrParam::ForegroundRgb(r, g, b) => {
            attrs.fg_rgb = Some((r, g, b));
            attrs.fg = None;
        }
        SgrParam::BackgroundRgb(r, g, b) => {
            attrs.bg_rgb = Some((r, g, b));
            attrs.bg = None;
        }
        SgrParam::DefaultForeground => {
            attrs.fg = None;
            attrs.fg_rgb = None;
        }
        SgrParam::DefaultBackground => {
            attrs.bg = None;
            attrs.bg_rgb = None;
        }
    }
}
