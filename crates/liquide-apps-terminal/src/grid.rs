//! Terminal character grid.

use serde::{Deserialize, Serialize};

/// Graphical attributes for a single cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CellAttrs {
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub blink: bool,
    pub reverse: bool,
    pub hidden: bool,
    pub strikethrough: bool,
    pub fg: Option<u8>,
    pub bg: Option<u8>,
    pub fg_rgb: Option<(u8, u8, u8)>,
    pub bg_rgb: Option<(u8, u8, u8)>,
}

/// A single terminal cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub attrs: CellAttrs,
    pub width: u8,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            attrs: CellAttrs::default(),
            width: 1,
        }
    }
}

/// The terminal character grid.
pub struct Grid {
    rows: u32,
    cols: u32,
    cells: Vec<Vec<Cell>>,
    cursor_row: u32,
    cursor_col: u32,
    current_attrs: CellAttrs,
    scroll_top: u32,
    scroll_bottom: u32,
}

impl Grid {
    #[must_use]
    pub fn new(rows: u32, cols: u32) -> Self {
        let cells = (0..rows)
            .map(|_| (0..cols).map(|_| Cell::default()).collect())
            .collect();
        Self {
            rows, cols, cells,
            cursor_row: 0, cursor_col: 0,
            current_attrs: CellAttrs::default(),
            scroll_top: 0,
            scroll_bottom: rows.saturating_sub(1),
        }
    }

    #[must_use] pub fn rows(&self) -> u32 { self.rows }
    #[must_use] pub fn cols(&self) -> u32 { self.cols }
    #[must_use] pub fn cursor(&self) -> (u32, u32) { (self.cursor_row, self.cursor_col) }
    #[must_use] pub fn current_attrs(&self) -> CellAttrs { self.current_attrs }

    pub fn set_attrs(&mut self, attrs: CellAttrs) { self.current_attrs = attrs; }
    pub fn reset_attrs(&mut self) { self.current_attrs = CellAttrs::default(); }

    pub fn set_cursor(&mut self, row: u32, col: u32) {
        self.cursor_row = row.min(self.rows.saturating_sub(1));
        self.cursor_col = col.min(self.cols.saturating_sub(1));
    }

    pub fn put_char(&mut self, ch: char) {
        if self.cursor_row < self.rows && self.cursor_col < self.cols {
            let r = self.cursor_row as usize;
            let c = self.cursor_col as usize;
            self.cells[r][c] = Cell { ch, attrs: self.current_attrs, width: 1 };
            self.cursor_col += 1;
            if self.cursor_col >= self.cols {
                self.cursor_col = 0;
                self.cursor_down_scroll();
            }
        }
    }

    fn cursor_down_scroll(&mut self) {
        if self.cursor_row < self.scroll_bottom {
            self.cursor_row += 1;
        } else {
            self.scroll_up(1);
        }
    }

    pub fn cursor_up(&mut self, n: u32) { self.cursor_row = self.cursor_row.saturating_sub(n); }
    pub fn cursor_down(&mut self, n: u32) { self.cursor_row = (self.cursor_row + n).min(self.rows.saturating_sub(1)); }
    pub fn cursor_forward(&mut self, n: u32) { self.cursor_col = (self.cursor_col + n).min(self.cols.saturating_sub(1)); }
    pub fn cursor_back(&mut self, n: u32) { self.cursor_col = self.cursor_col.saturating_sub(n); }
    pub fn carriage_return(&mut self) { self.cursor_col = 0; }

    pub fn line_feed(&mut self) { self.cursor_down_scroll(); }

    pub fn scroll_up(&mut self, n: u32) -> Vec<Vec<Cell>> {
        let mut scrolled_out = Vec::new();
        for _ in 0..n {
            if self.scroll_top <= self.scroll_bottom {
                let row = self.cells.remove(self.scroll_top as usize);
                scrolled_out.push(row);
                let blank = (0..self.cols).map(|_| Cell::default()).collect();
                let insert_at = self.scroll_bottom as usize;
                if insert_at < self.cells.len() {
                    self.cells.insert(insert_at, blank);
                } else {
                    self.cells.push(blank);
                }
            }
        }
        scrolled_out
    }

    pub fn scroll_down(&mut self, n: u32) {
        for _ in 0..n {
            if self.scroll_top <= self.scroll_bottom && (self.scroll_bottom as usize) < self.cells.len() {
                self.cells.remove(self.scroll_bottom as usize);
                let blank = (0..self.cols).map(|_| Cell::default()).collect();
                self.cells.insert(self.scroll_top as usize, blank);
            }
        }
    }

    pub fn set_scroll_region(&mut self, top: u32, bottom: u32) {
        let t = top.saturating_sub(1);
        let b = if bottom == 0 { self.rows.saturating_sub(1) } else { (bottom.saturating_sub(1)).min(self.rows.saturating_sub(1)) };
        if t < b { self.scroll_top = t; self.scroll_bottom = b; }
    }

    pub fn erase_display_to_end(&mut self) {
        let r = self.cursor_row as usize;
        let c = self.cursor_col as usize;
        if r < self.cells.len() { for cell in &mut self.cells[r][c..] { *cell = Cell::default(); } }
        for row in &mut self.cells[(r + 1)..] { for cell in row.iter_mut() { *cell = Cell::default(); } }
    }

    pub fn erase_display_to_beginning(&mut self) {
        let r = self.cursor_row as usize;
        let c = self.cursor_col as usize;
        for row in &mut self.cells[..r] { for cell in row.iter_mut() { *cell = Cell::default(); } }
        if r < self.cells.len() { let end = (c + 1).min(self.cells[r].len()); for cell in &mut self.cells[r][..end] { *cell = Cell::default(); } }
    }

    pub fn erase_display_all(&mut self) {
        for row in &mut self.cells { for cell in row.iter_mut() { *cell = Cell::default(); } }
    }

    pub fn erase_line_to_end(&mut self) {
        let r = self.cursor_row as usize;
        let c = self.cursor_col as usize;
        if r < self.cells.len() { for cell in &mut self.cells[r][c..] { *cell = Cell::default(); } }
    }

    pub fn erase_line_to_beginning(&mut self) {
        let r = self.cursor_row as usize;
        let c = self.cursor_col as usize;
        if r < self.cells.len() { let end = (c + 1).min(self.cells[r].len()); for cell in &mut self.cells[r][..end] { *cell = Cell::default(); } }
    }

    pub fn erase_line_all(&mut self) {
        let r = self.cursor_row as usize;
        if r < self.cells.len() { for cell in &mut self.cells[r] { *cell = Cell::default(); } }
    }

    #[must_use]
    pub fn cell(&self, row: u32, col: u32) -> Option<&Cell> {
        self.cells.get(row as usize).and_then(|r| r.get(col as usize))
    }

    #[must_use]
    pub fn row_text(&self, row: u32) -> String {
        self.cells.get(row as usize)
            .map(|r| r.iter().map(|c| c.ch).collect::<String>().trim_end().to_string())
            .unwrap_or_default()
    }

    /// Move cursor to the next tab stop (every 8 columns).
    pub fn cursor_tab(&mut self) {
        let next = ((self.cursor_col / 8) + 1) * 8;
        self.cursor_col = next.min(self.cols.saturating_sub(1));
    }

    /// Insert `n` blank lines at the cursor row, pushing lines down.
    /// Lines that fall below the scroll region are discarded.
    pub fn insert_lines(&mut self, n: u32) {
        let top = self.cursor_row.max(self.scroll_top) as usize;
        let bot = self.scroll_bottom as usize;
        if top > bot || top >= self.cells.len() {
            return;
        }
        for _ in 0..n {
            if bot < self.cells.len() {
                self.cells.remove(bot);
            }
            let blank: Vec<Cell> = (0..self.cols).map(|_| Cell::default()).collect();
            self.cells.insert(top, blank);
        }
    }

    /// Delete `n` lines at the cursor row, pulling lines up.
    /// Blank lines are inserted at the bottom of the scroll region.
    pub fn delete_lines(&mut self, n: u32) {
        let top = self.cursor_row.max(self.scroll_top) as usize;
        let bot = self.scroll_bottom as usize;
        if top > bot || top >= self.cells.len() {
            return;
        }
        for _ in 0..n {
            if top < self.cells.len() {
                self.cells.remove(top);
            }
            let blank: Vec<Cell> = (0..self.cols).map(|_| Cell::default()).collect();
            let insert_at = bot.min(self.cells.len());
            self.cells.insert(insert_at, blank);
        }
    }

    /// Insert `n` blank characters at the cursor position, shifting cells right.
    /// Characters that fall off the right edge are discarded.
    pub fn insert_chars(&mut self, n: u32) {
        let r = self.cursor_row as usize;
        let c = self.cursor_col as usize;
        if r >= self.cells.len() {
            return;
        }
        let row = &mut self.cells[r];
        for _ in 0..n {
            if c < row.len() {
                row.pop(); // discard rightmost
                row.insert(c, Cell::default());
            }
        }
    }

    /// Delete `n` characters at the cursor position, shifting cells left.
    /// Blank characters are inserted at the right edge.
    pub fn delete_chars(&mut self, n: u32) {
        let r = self.cursor_row as usize;
        let c = self.cursor_col as usize;
        if r >= self.cells.len() {
            return;
        }
        let row = &mut self.cells[r];
        for _ in 0..n {
            if c < row.len() {
                row.remove(c);
                row.push(Cell::default());
            }
        }
    }

    pub fn resize(&mut self, new_rows: u32, new_cols: u32) {
        while (self.cells.len() as u32) < new_rows { self.cells.push((0..new_cols).map(|_| Cell::default()).collect()); }
        self.cells.truncate(new_rows as usize);
        for row in &mut self.cells {
            while (row.len() as u32) < new_cols { row.push(Cell::default()); }
            row.truncate(new_cols as usize);
        }
        self.rows = new_rows;
        self.cols = new_cols;
        self.scroll_bottom = new_rows.saturating_sub(1);
        self.cursor_row = self.cursor_row.min(new_rows.saturating_sub(1));
        self.cursor_col = self.cursor_col.min(new_cols.saturating_sub(1));
    }
}

impl Default for Grid {
    fn default() -> Self { Self::new(24, 80) }
}
