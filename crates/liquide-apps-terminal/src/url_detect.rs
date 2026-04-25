//! URL and path detection in terminal output.

/// A detected link in terminal output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedLink {
    /// Line index.
    pub line: usize,
    /// Start column.
    pub start_col: usize,
    /// End column (exclusive).
    pub end_col: usize,
    /// The detected URL or path.
    pub target: String,
    /// Link type.
    pub kind: LinkKind,
}

/// Type of detected link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    /// HTTP/HTTPS URL.
    Url,
    /// File path.
    FilePath,
    /// IP address.
    IpAddress,
    /// Email address.
    Email,
}

/// Detect links in a line of text.
#[must_use]
pub fn detect_links(line: &str, line_index: usize) -> Vec<DetectedLink> {
    let mut links = Vec::new();

    // Simple URL detection: http:// or https://
    detect_pattern(line, line_index, "http://", LinkKind::Url, &mut links);
    detect_pattern(line, line_index, "https://", LinkKind::Url, &mut links);

    // File paths: /path/to/file
    for (i, _) in line.char_indices() {
        if i == 0 || line.as_bytes().get(i.wrapping_sub(1)).copied() == Some(b' ') {
            if line[i..].starts_with('/') && line[i..].len() > 1 {
                let end = line[i..]
                    .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
                    .map(|e| i + e)
                    .unwrap_or(line.len());
                if end > i + 1 {
                    links.push(DetectedLink {
                        line: line_index,
                        start_col: i,
                        end_col: end,
                        target: line[i..end].to_string(),
                        kind: LinkKind::FilePath,
                    });
                }
            }
        }
    }

    links
}

fn detect_pattern(
    line: &str,
    line_index: usize,
    prefix: &str,
    kind: LinkKind,
    links: &mut Vec<DetectedLink>,
) {
    let mut start = 0;
    while let Some(pos) = line[start..].find(prefix) {
        let abs_start = start + pos;
        let end = line[abs_start..]
            .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == '>' || c == ')')
            .map(|e| abs_start + e)
            .unwrap_or(line.len());
        if end > abs_start + prefix.len() {
            links.push(DetectedLink {
                line: line_index,
                start_col: abs_start,
                end_col: end,
                target: line[abs_start..end].to_string(),
                kind,
            });
        }
        start = abs_start + 1;
    }
}
