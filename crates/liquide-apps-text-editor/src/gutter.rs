//! Line number gutter and diagnostics.

/// Severity of a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

/// A diagnostic message attached to a line.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub line: usize,
    pub col: Option<usize>,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub source: String,
}

impl Diagnostic {
    #[must_use]
    pub fn error(line: usize, message: impl Into<String>, source: impl Into<String>) -> Self {
        Self { line, col: None, severity: DiagnosticSeverity::Error, message: message.into(), source: source.into() }
    }

    #[must_use]
    pub fn warning(line: usize, message: impl Into<String>, source: impl Into<String>) -> Self {
        Self { line, col: None, severity: DiagnosticSeverity::Warning, message: message.into(), source: source.into() }
    }
}

/// Gutter state showing line numbers, fold markers, and diagnostics.
pub struct Gutter {
    diagnostics: Vec<Diagnostic>,
    /// Which lines have breakpoints.
    breakpoints: Vec<usize>,
    /// Line number width (number of digits for max line).
    width: usize,
}

impl Gutter {
    #[must_use]
    pub fn new() -> Self {
        Self { diagnostics: Vec::new(), breakpoints: Vec::new(), width: 4 }
    }

    /// Update the gutter width based on total line count.
    pub fn update_width(&mut self, total_lines: usize) {
        self.width = format!("{total_lines}").len().max(3);
    }

    /// Gutter width in characters.
    #[must_use]
    pub fn width(&self) -> usize { self.width }

    /// Set diagnostics.
    pub fn set_diagnostics(&mut self, diagnostics: Vec<Diagnostic>) {
        self.diagnostics = diagnostics;
    }

    /// Get diagnostics for a specific line.
    #[must_use]
    pub fn diagnostics_for(&self, line: usize) -> Vec<&Diagnostic> {
        self.diagnostics.iter().filter(|d| d.line == line).collect()
    }

    /// All diagnostics.
    #[must_use]
    pub fn all_diagnostics(&self) -> &[Diagnostic] { &self.diagnostics }

    /// Number of errors.
    #[must_use]
    pub fn error_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.severity == DiagnosticSeverity::Error).count()
    }

    /// Number of warnings.
    #[must_use]
    pub fn warning_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.severity == DiagnosticSeverity::Warning).count()
    }

    /// Toggle a breakpoint on a line.
    pub fn toggle_breakpoint(&mut self, line: usize) {
        if let Some(pos) = self.breakpoints.iter().position(|&l| l == line) {
            self.breakpoints.remove(pos);
        } else {
            self.breakpoints.push(line);
        }
    }

    /// Whether a line has a breakpoint.
    #[must_use]
    pub fn has_breakpoint(&self, line: usize) -> bool {
        self.breakpoints.contains(&line)
    }

    /// All breakpoints.
    #[must_use]
    pub fn breakpoints(&self) -> &[usize] { &self.breakpoints }

    /// Clear all diagnostics.
    pub fn clear_diagnostics(&mut self) {
        self.diagnostics.clear();
    }
}

impl Default for Gutter {
    fn default() -> Self { Self::new() }
}
