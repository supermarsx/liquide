//! Terminal runtime coordinator.

use crate::config::TerminalConfig;
use crate::grid::Grid;
use crate::search::SearchState;
use crate::tab::TabManager;
use crate::vt::{Action, CsiAction, EraseMode, OscAction, Parser, SgrParam};
use crate::grid::CellAttrs;

/// Central coordinator for the terminal emulator.
pub struct TerminalRuntime {
    config: TerminalConfig,
    tabs: TabManager,
    parser: Parser,
    search: SearchState,
}

impl TerminalRuntime {
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
    pub fn config(&self) -> &TerminalConfig { &self.config }

    /// Create a new tab, returning its ID.
    pub fn new_tab(&mut self, title: Option<String>) -> u32 {
        let id = self.tabs.new_tab(
            &self.config.shell,
            self.config.rows,
            self.config.cols,
            self.config.scrollback_lines,
        );
        if let Some(title) = title {
            if let Some(tab) = self.tabs.get_mut(id) {
                tab.set_title(title);
            }
        }
        id
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
    pub fn tab_count(&self) -> usize { self.tabs.count() }

    /// List tabs.
    #[must_use]
    pub fn tab_list(&self) -> Vec<(u32, String)> { self.tabs.list() }

    /// Get the active grid (read-only).
    #[must_use]
    pub fn active_grid(&self) -> &Grid {
        self.tabs.active_tab()
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
        let tab = self.tabs.active_tab_mut()
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
    pub fn search(&self) -> &SearchState { &self.search }

    /// Search mutable access.
    pub fn search_mut(&mut self) -> &mut SearchState { &mut self.search }

    fn apply_action(&mut self, action: Action) {
        let Some(tab) = self.tabs.active_tab_mut() else { return };
        let grid = tab.grid_mut();

        match action {
            Action::Print(ch) => {
                grid.put_char(ch);
            }
            Action::Execute(byte) => match byte {
                0x08 => grid.cursor_back(1),      // BS
                0x09 => grid.cursor_forward(8),    // HT (simplified)
                0x0a | 0x0b | 0x0c => grid.line_feed(), // LF/VT/FF
                0x0d => grid.carriage_return(),    // CR
                _ => {}
            },
            Action::CsiDispatch(csi) => self.apply_csi(csi),
            Action::OscDispatch(osc) => self.apply_osc(osc),
            Action::EscDispatch(_) => {}
        }
    }

    fn apply_csi(&mut self, csi: CsiAction) {
        let Some(tab) = self.tabs.active_tab_mut() else { return };
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
            CsiAction::InsertLines(_)
            | CsiAction::DeleteLines(_)
            | CsiAction::InsertChars(_)
            | CsiAction::DeleteChars(_)
            | CsiAction::DeviceStatusReport
            | CsiAction::Unknown(_) => {}
        }
    }

    fn apply_osc(&mut self, osc: OscAction) {
        let Some(tab) = self.tabs.active_tab_mut() else { return };
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
        SgrParam::Foreground(idx) => { attrs.fg = Some(idx); attrs.fg_rgb = None; }
        SgrParam::Background(idx) => { attrs.bg = Some(idx); attrs.bg_rgb = None; }
        SgrParam::ForegroundRgb(r, g, b) => { attrs.fg_rgb = Some((r, g, b)); attrs.fg = None; }
        SgrParam::BackgroundRgb(r, g, b) => { attrs.bg_rgb = Some((r, g, b)); attrs.bg = None; }
        SgrParam::DefaultForeground => { attrs.fg = None; attrs.fg_rgb = None; }
        SgrParam::DefaultBackground => { attrs.bg = None; attrs.bg_rgb = None; }
    }
}
