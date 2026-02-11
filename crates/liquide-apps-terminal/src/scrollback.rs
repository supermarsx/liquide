//! Scrollback buffer for terminal history.

use crate::grid::Cell;

/// Scrollback buffer storing lines that have scrolled off the top.
pub struct ScrollbackBuffer {
    lines: Vec<Vec<Cell>>,
    max_lines: usize,
    viewport_offset: usize,
}

impl ScrollbackBuffer {
    /// Create a new scrollback buffer.
    #[must_use]
    pub fn new(max_lines: usize) -> Self {
        Self {
            lines: Vec::new(),
            max_lines,
            viewport_offset: 0,
        }
    }

    /// Push a line into the scrollback.
    pub fn push(&mut self, line: Vec<Cell>) {
        self.lines.push(line);
        if self.lines.len() > self.max_lines {
            self.lines.remove(0);
            if self.viewport_offset > 0 {
                self.viewport_offset = self.viewport_offset.saturating_sub(1);
            }
        }
    }

    /// Push multiple lines.
    pub fn push_lines(&mut self, lines: Vec<Vec<Cell>>) {
        for line in lines {
            self.push(line);
        }
    }

    /// Total lines in the buffer.
    #[must_use]
    pub fn len(&self) -> usize { self.lines.len() }

    /// Whether the buffer is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool { self.lines.is_empty() }

    /// Maximum capacity.
    #[must_use]
    pub fn capacity(&self) -> usize { self.max_lines }

    /// Get a line by index (0 = oldest).
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&[Cell]> {
        self.lines.get(index).map(|l| l.as_slice())
    }

    /// Get the text content of a line.
    #[must_use]
    pub fn line_text(&self, index: usize) -> String {
        self.lines.get(index)
            .map(|l| l.iter().map(|c| c.ch).collect::<String>().trim_end().to_string())
            .unwrap_or_default()
    }

    /// Scroll viewport up by `n` lines.
    pub fn scroll_up(&mut self, n: usize) {
        self.viewport_offset = (self.viewport_offset + n).min(self.lines.len());
    }

    /// Scroll viewport down by `n` lines.
    pub fn scroll_down(&mut self, n: usize) {
        self.viewport_offset = self.viewport_offset.saturating_sub(n);
    }

    /// Reset viewport to bottom (most recent).
    pub fn scroll_to_bottom(&mut self) {
        self.viewport_offset = 0;
    }

    /// Current viewport offset from bottom.
    #[must_use]
    pub fn viewport_offset(&self) -> usize { self.viewport_offset }

    /// Whether viewport is at the bottom.
    #[must_use]
    pub fn at_bottom(&self) -> bool { self.viewport_offset == 0 }

    /// Clear the scrollback buffer.
    pub fn clear(&mut self) {
        self.lines.clear();
        self.viewport_offset = 0;
    }

    /// Search for a string in the scrollback, returning matching line indices.
    #[must_use]
    pub fn find_lines(&self, needle: &str) -> Vec<usize> {
        self.lines.iter().enumerate()
            .filter(|(_, line)| {
                let text: String = line.iter().map(|c| c.ch).collect();
                text.contains(needle)
            })
            .map(|(i, _)| i)
            .collect()
    }
}

impl Default for ScrollbackBuffer {
    fn default() -> Self { Self::new(10_000) }
}
