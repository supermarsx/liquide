//! Auto-indent logic.

/// Indentation style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentStyle {
    Spaces(usize),
    Tabs,
}

impl IndentStyle {
    /// Get the indent string for one level.
    #[must_use]
    pub fn indent_str(&self) -> String {
        match self {
            Self::Spaces(n) => " ".repeat(*n),
            Self::Tabs => "\t".to_string(),
        }
    }
}

/// Compute the leading whitespace of a line.
#[must_use]
pub fn leading_whitespace(line: &str) -> &str {
    let trimmed = line.trim_start();
    &line[..line.len() - trimmed.len()]
}

/// Compute the indent level of a line.
#[must_use]
pub fn indent_level(line: &str, tab_width: usize) -> usize {
    let mut level = 0;
    for ch in line.chars() {
        match ch {
            ' ' => level += 1,
            '\t' => level += tab_width,
            _ => break,
        }
    }
    level / tab_width
}

/// Determine the auto-indent for the next line.
#[must_use]
pub fn auto_indent(current_line: &str, style: IndentStyle) -> String {
    let base = leading_whitespace(current_line).to_string();
    let trimmed = current_line.trim_end();

    // Increase indent after opening braces / colons.
    if trimmed.ends_with('{') || trimmed.ends_with(':') || trimmed.ends_with('(') {
        return base + &style.indent_str();
    }

    base
}

/// Detect the indentation style used in a set of lines.
#[must_use]
pub fn detect_indent(lines: &[String]) -> IndentStyle {
    let mut tab_count = 0;
    let mut space_count = 0;
    let mut space_widths: [usize; 9] = [0; 9]; // index 1..8

    for line in lines {
        if line.is_empty() { continue; }
        let first = line.chars().next().unwrap_or(' ');
        if first == '\t' {
            tab_count += 1;
        } else if first == ' ' {
            let ws_len = line.len() - line.trim_start().len();
            if ws_len > 0 && ws_len <= 8 {
                space_widths[ws_len] += 1;
            }
            space_count += 1;
        }
    }

    if tab_count > space_count {
        return IndentStyle::Tabs;
    }

    // Find most common space width.
    let mut best_width = 4;
    let mut best_count = 0;
    for width in [2, 4, 8, 3] {
        if space_widths.get(width).copied().unwrap_or(0) > best_count {
            best_count = space_widths[width];
            best_width = width;
        }
    }

    IndentStyle::Spaces(best_width)
}
