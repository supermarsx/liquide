//! Text buffer with line-oriented storage.

/// A text buffer storing content as a vector of lines.
#[derive(Debug, Clone)]
pub struct TextBuffer {
    lines: Vec<String>,
    modified: bool,
}

impl TextBuffer {
    /// Create an empty buffer with one empty line.
    #[must_use]
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            modified: false,
        }
    }

    /// Create a buffer from a vector of lines.
    #[must_use]
    pub fn from_lines(mut lines: Vec<String>) -> Self {
        if lines.is_empty() {
            lines.push(String::new());
        }
        Self {
            lines,
            modified: false,
        }
    }

    /// Create a buffer from text content.
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        let lines: Vec<String> = if text.is_empty() {
            vec![String::new()]
        } else {
            text.lines().map(String::from).collect()
        };
        Self {
            lines,
            modified: false,
        }
    }

    /// Number of lines.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Get a line by index.
    #[must_use]
    pub fn line(&self, index: usize) -> Option<&str> {
        self.lines.get(index).map(String::as_str)
    }

    /// Get line length in characters.
    #[must_use]
    pub fn line_len(&self, index: usize) -> usize {
        self.lines.get(index).map_or(0, |l| l.len())
    }

    /// Whether the buffer has been modified since last mark.
    #[must_use]
    pub fn is_modified(&self) -> bool {
        self.modified
    }

    /// Clear the modified flag.
    pub fn mark_saved(&mut self) {
        self.modified = false;
    }

    /// Total character count across all lines.
    #[must_use]
    pub fn char_count(&self) -> usize {
        self.lines.iter().map(|l| l.len()).sum::<usize>() + self.lines.len().saturating_sub(1) // newlines
    }

    /// Insert a character at the given line and column.
    pub fn insert_char(&mut self, line: usize, col: usize, ch: char) -> crate::Result<()> {
        let total = self.lines.len();
        let l = self
            .lines
            .get_mut(line)
            .ok_or(crate::EditorError::LineOutOfRange { line, total })?;
        if col > l.len() {
            return Err(crate::EditorError::ColumnOutOfRange { col, len: l.len() });
        }
        l.insert(col, ch);
        self.modified = true;
        Ok(())
    }

    /// Delete a character at the given line and column.
    pub fn delete_char(&mut self, line: usize, col: usize) -> crate::Result<char> {
        let total = self.lines.len();
        let l = self
            .lines
            .get_mut(line)
            .ok_or(crate::EditorError::LineOutOfRange { line, total })?;
        if col >= l.len() {
            return Err(crate::EditorError::ColumnOutOfRange { col, len: l.len() });
        }
        let ch = l.remove(col);
        self.modified = true;
        Ok(ch)
    }

    /// Insert a new line, splitting the current line at the given column.
    pub fn insert_newline(&mut self, line: usize, col: usize) -> crate::Result<()> {
        let l = self
            .lines
            .get(line)
            .ok_or(crate::EditorError::LineOutOfRange {
                line,
                total: self.lines.len(),
            })?;
        if col > l.len() {
            return Err(crate::EditorError::ColumnOutOfRange { col, len: l.len() });
        }
        let remainder = self.lines[line][col..].to_string();
        self.lines[line].truncate(col);
        self.lines.insert(line + 1, remainder);
        self.modified = true;
        Ok(())
    }

    /// Delete a line, joining it with the previous line.
    pub fn join_line_up(&mut self, line: usize) -> crate::Result<usize> {
        if line == 0 || line >= self.lines.len() {
            return Err(crate::EditorError::LineOutOfRange {
                line,
                total: self.lines.len(),
            });
        }
        let removed = self.lines.remove(line);
        let join_col = self.lines[line - 1].len();
        self.lines[line - 1].push_str(&removed);
        self.modified = true;
        Ok(join_col)
    }

    /// Insert text at a position, handling multi-line inserts.
    pub fn insert_text(
        &mut self,
        line: usize,
        col: usize,
        text: &str,
    ) -> crate::Result<(usize, usize)> {
        let l = self
            .lines
            .get(line)
            .ok_or(crate::EditorError::LineOutOfRange {
                line,
                total: self.lines.len(),
            })?;
        if col > l.len() {
            return Err(crate::EditorError::ColumnOutOfRange { col, len: l.len() });
        }

        let after = self.lines[line][col..].to_string();
        self.lines[line].truncate(col);

        let new_lines: Vec<&str> = text.lines().collect();
        if new_lines.is_empty() {
            self.lines[line].push_str(&after);
            return Ok((line, col));
        }

        self.lines[line].push_str(new_lines[0]);

        for (i, nl) in new_lines.iter().enumerate().skip(1) {
            self.lines.insert(line + i, nl.to_string());
        }

        let end_line = line + new_lines.len() - 1;
        let end_col = self.lines[end_line].len();
        self.lines[end_line].push_str(&after);

        self.modified = true;
        Ok((end_line, end_col))
    }

    /// Delete a range of text.
    pub fn delete_range(
        &mut self,
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
    ) -> crate::Result<String> {
        if start_line >= self.lines.len() {
            return Err(crate::EditorError::LineOutOfRange {
                line: start_line,
                total: self.lines.len(),
            });
        }
        if end_line >= self.lines.len() {
            return Err(crate::EditorError::LineOutOfRange {
                line: end_line,
                total: self.lines.len(),
            });
        }

        if start_line == end_line {
            let l = &mut self.lines[start_line];
            let deleted: String = l[start_col..end_col].to_string();
            l.replace_range(start_col..end_col, "");
            self.modified = true;
            return Ok(deleted);
        }

        let mut deleted = String::new();
        deleted.push_str(&self.lines[start_line][start_col..]);
        deleted.push('\n');

        for i in (start_line + 1)..end_line {
            deleted.push_str(&self.lines[i]);
            deleted.push('\n');
        }
        deleted.push_str(&self.lines[end_line][..end_col]);

        let after = self.lines[end_line][end_col..].to_string();
        self.lines[start_line].truncate(start_col);
        self.lines[start_line].push_str(&after);

        self.lines.drain((start_line + 1)..=end_line);
        self.modified = true;
        Ok(deleted)
    }

    /// Get the full text content.
    #[must_use]
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    /// Get all lines.
    #[must_use]
    pub fn lines(&self) -> &[String] {
        &self.lines
    }
}

impl Default for TextBuffer {
    fn default() -> Self {
        Self::new()
    }
}
