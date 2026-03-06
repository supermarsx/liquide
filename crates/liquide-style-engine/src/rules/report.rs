//! Validation report summarizing rule violations.

use super::types::Violation;

/// Summary report of validation results.
#[derive(Debug)]
pub struct ValidationReport {
    /// All violations found.
    pub violations: Vec<Violation>,
    /// Number of critical violations.
    pub critical_count: usize,
    /// Number of error violations.
    pub error_count: usize,
    /// Number of warning violations.
    pub warning_count: usize,
}

impl ValidationReport {
    /// Returns true if there are no critical or error violations.
    pub fn is_valid(&self) -> bool {
        self.critical_count == 0 && self.error_count == 0
    }

    /// Returns true if there are no violations at all.
    pub fn is_perfect(&self) -> bool {
        self.violations.is_empty()
    }

    /// Format report as human-readable string.
    pub fn to_string_report(&self) -> String {
        let mut report = String::new();
        report.push_str(&format!(
            "Validation Report: {} critical, {} errors, {} warnings\n",
            self.critical_count, self.error_count, self.warning_count
        ));
        report.push_str("═".repeat(60).as_str());
        report.push('\n');

        for v in &self.violations {
            report.push_str(&format!("[{}] <{}> {}\n", v.severity, v.element, v.message));
            if let Some(suggestion) = &v.suggestion {
                report.push_str(&format!("    ↳ Fix: {}\n", suggestion));
            }
        }

        if self.violations.is_empty() {
            report.push_str("✓ All elements pass validation\n");
        }

        report
    }
}
