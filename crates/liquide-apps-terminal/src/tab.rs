//! Tab management for multi-tab terminal.

use crate::grid::Grid;
use crate::pty::{PtyBackend, PtySize};
use crate::scrollback::ScrollbackBuffer;
use crate::shell_integration::ShellIntegration;

/// A terminal tab containing a grid, PTY, and scrollback.
pub struct Tab {
    id: u32,
    title: String,
    grid: Grid,
    scrollback: ScrollbackBuffer,
    pty: PtyBackend,
    shell_integration: ShellIntegration,
    closed: bool,
}

impl Tab {
    /// Create a new tab.
    #[must_use]
    pub fn new(id: u32, shell: &str, rows: u32, cols: u32, scrollback_lines: u32) -> Self {
        let size = PtySize::new(rows, cols);
        Self {
            id,
            title: format!("Tab {id}"),
            grid: Grid::new(rows, cols),
            scrollback: ScrollbackBuffer::new(scrollback_lines as usize),
            pty: PtyBackend::new(shell.to_string(), size),
            shell_integration: ShellIntegration::new(),
            closed: false,
        }
    }

    /// Tab ID.
    #[must_use]
    pub fn id(&self) -> u32 { self.id }

    /// Tab title.
    #[must_use]
    pub fn title(&self) -> &str { &self.title }

    /// Set tab title.
    pub fn set_title(&mut self, title: String) { self.title = title; }

    /// Get the character grid.
    #[must_use]
    pub fn grid(&self) -> &Grid { &self.grid }

    /// Get a mutable reference to the grid.
    pub fn grid_mut(&mut self) -> &mut Grid { &mut self.grid }

    /// Get the scrollback buffer.
    #[must_use]
    pub fn scrollback(&self) -> &ScrollbackBuffer { &self.scrollback }

    /// Get a mutable reference to the scrollback.
    pub fn scrollback_mut(&mut self) -> &mut ScrollbackBuffer { &mut self.scrollback }

    /// Get the PTY backend.
    #[must_use]
    pub fn pty(&self) -> &PtyBackend { &self.pty }

    /// Get a mutable PTY reference.
    pub fn pty_mut(&mut self) -> &mut PtyBackend { &mut self.pty }

    /// Get shell integration state.
    #[must_use]
    pub fn shell_integration(&self) -> &ShellIntegration { &self.shell_integration }

    /// Get mutable shell integration.
    pub fn shell_integration_mut(&mut self) -> &mut ShellIntegration { &mut self.shell_integration }

    /// Whether this tab has been closed.
    #[must_use]
    pub fn is_closed(&self) -> bool { self.closed }

    /// Mark this tab as closed.
    pub fn close(&mut self) {
        self.closed = true;
        self.pty.kill();
    }

    /// Get the display title (from shell integration or manual title).
    #[must_use]
    pub fn display_title(&self) -> String {
        let si_title = self.shell_integration.tab_title();
        if si_title != "terminal" {
            si_title
        } else {
            self.title.clone()
        }
    }

    /// Resize this tab.
    pub fn resize(&mut self, rows: u32, cols: u32) {
        self.grid.resize(rows, cols);
        self.pty.resize(PtySize::new(rows, cols));
    }
}

/// Tab manager holding all tabs.
pub struct TabManager {
    tabs: Vec<Tab>,
    active_tab_id: u32,
    next_id: u32,
}

impl TabManager {
    /// Create a new empty tab manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_tab_id: 0,
            next_id: 1,
        }
    }

    /// Create a new tab and return its ID.
    pub fn new_tab(&mut self, shell: &str, rows: u32, cols: u32, scrollback: u32) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        let tab = Tab::new(id, shell, rows, cols, scrollback);
        self.tabs.push(tab);
        if self.tabs.len() == 1 {
            self.active_tab_id = id;
        }
        id
    }

    /// Close a tab by ID.
    pub fn close_tab(&mut self, id: u32) -> crate::Result<()> {
        let tab = self.tabs.iter_mut().find(|t| t.id == id)
            .ok_or(crate::TerminalError::TabNotFound { id })?;
        tab.close();
        self.tabs.retain(|t| !t.is_closed());
        if self.active_tab_id == id {
            self.active_tab_id = self.tabs.first().map(|t| t.id()).unwrap_or(0);
        }
        Ok(())
    }

    /// Set the active tab.
    pub fn set_active(&mut self, id: u32) -> crate::Result<()> {
        if !self.tabs.iter().any(|t| t.id == id) {
            return Err(crate::TerminalError::TabNotFound { id });
        }
        self.active_tab_id = id;
        Ok(())
    }

    /// Get the active tab.
    #[must_use]
    pub fn active_tab(&self) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id == self.active_tab_id)
    }

    /// Get a mutable reference to the active tab.
    pub fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        let id = self.active_tab_id;
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    /// Get a tab by ID.
    #[must_use]
    pub fn get(&self, id: u32) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id == id)
    }

    /// Get a mutable reference to a tab.
    pub fn get_mut(&mut self, id: u32) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    /// Active tab ID.
    #[must_use]
    pub fn active_id(&self) -> u32 { self.active_tab_id }

    /// Total tab count.
    #[must_use]
    pub fn count(&self) -> usize { self.tabs.len() }

    /// List tab IDs and titles.
    #[must_use]
    pub fn list(&self) -> Vec<(u32, String)> {
        self.tabs.iter().map(|t| (t.id(), t.display_title())).collect()
    }
}

impl Default for TabManager {
    fn default() -> Self { Self::new() }
}
