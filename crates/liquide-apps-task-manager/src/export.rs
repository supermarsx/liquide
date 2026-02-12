//! Data export utilities for the task manager.
//!
//! Provides export format selection, export configuration, an exportable
//! trait for tabular data, and a generic export function that renders
//! records in CSV, TSV, JSON, HTML, or XML. Corresponds to spec
//! section 23 (API Surface) and appendix A.2.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Supported export file formats.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    /// Comma-separated values.
    Csv,
    /// JSON array of objects.
    Json,
    /// Tab-separated values.
    Tsv,
    /// Basic HTML table.
    Html,
    /// Basic XML elements.
    Xml,
}

impl ExportFormat {
    /// Return a human-readable name for this format.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Csv => "CSV",
            Self::Json => "JSON",
            Self::Tsv => "TSV",
            Self::Html => "HTML",
            Self::Xml => "XML",
        }
    }
}

impl fmt::Display for ExportFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Configuration controlling how records are exported.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportConfig {
    /// The output format.
    pub format: ExportFormat,
    /// Whether to include a header row (CSV/TSV) or equivalent.
    pub include_headers: bool,
    /// If set, only export these columns (by key). `None` means all columns.
    pub columns: Option<Vec<String>>,
    /// Optional file path to write the output to. `None` means return as string.
    pub output_path: Option<String>,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            format: ExportFormat::Csv,
            include_headers: true,
            columns: None,
            output_path: None,
        }
    }
}

/// Trait for types that can be exported as tabular rows.
pub trait Exportable {
    /// Return the column headers for this type.
    fn export_headers(&self) -> Vec<String>;

    /// Return a single row of string-formatted values, one per column.
    fn export_row(&self) -> Vec<String>;
}

/// Render a slice of exportable records into a string using the given config.
///
/// # Errors
///
/// Returns `Err` if the records slice is empty and headers cannot be
/// determined, or if a column filter references a non-existent header.
pub fn export_records<T: Exportable>(
    records: &[T],
    config: &ExportConfig,
) -> Result<String, String> {
    if records.is_empty() {
        return Ok(match config.format {
            ExportFormat::Json => "[]".to_string(),
            ExportFormat::Html => {
                "<table>\n</table>".to_string()
            }
            ExportFormat::Xml => {
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<records>\n</records>".to_string()
            }
            _ => String::new(),
        });
    }

    let all_headers = records[0].export_headers();

    // Determine which column indices to include.
    let indices: Vec<usize> = if let Some(ref cols) = config.columns {
        let mut idx = Vec::new();
        for col in cols {
            let pos = all_headers
                .iter()
                .position(|h| h == col)
                .ok_or_else(|| format!("unknown column: {col}"))?;
            idx.push(pos);
        }
        idx
    } else {
        (0..all_headers.len()).collect()
    };

    let headers: Vec<&str> = indices.iter().map(|&i| all_headers[i].as_str()).collect();

    match config.format {
        ExportFormat::Csv => render_separated(records, &headers, &indices, ',', config.include_headers),
        ExportFormat::Tsv => render_separated(records, &headers, &indices, '\t', config.include_headers),
        ExportFormat::Json => render_json(records, &headers, &indices),
        ExportFormat::Html => render_html(records, &headers, &indices, config.include_headers),
        ExportFormat::Xml => render_xml(records, &headers, &indices),
    }
}

/// Render CSV or TSV output.
fn render_separated<T: Exportable>(
    records: &[T],
    headers: &[&str],
    indices: &[usize],
    sep: char,
    include_headers: bool,
) -> Result<String, String> {
    let mut out = String::new();

    if include_headers {
        let header_line: Vec<String> = headers.iter().map(|h| escape_field(h, sep)).collect();
        out.push_str(&header_line.join(&sep.to_string()));
        out.push('\n');
    }

    for record in records {
        let row = record.export_row();
        let selected: Vec<String> = indices
            .iter()
            .map(|&i| {
                let val = row.get(i).map(|s| s.as_str()).unwrap_or("");
                escape_field(val, sep)
            })
            .collect();
        out.push_str(&selected.join(&sep.to_string()));
        out.push('\n');
    }

    Ok(out)
}

/// Escape a field value for CSV/TSV. If the field contains the separator,
/// a double quote, or a newline, wrap it in quotes and double any internal
/// quotes.
fn escape_field(value: &str, sep: char) -> String {
    if value.contains(sep) || value.contains('"') || value.contains('\n') {
        let escaped = value.replace('"', "\"\"");
        format!("\"{escaped}\"")
    } else {
        value.to_string()
    }
}

/// Render JSON array of objects.
fn render_json<T: Exportable>(
    records: &[T],
    headers: &[&str],
    indices: &[usize],
) -> Result<String, String> {
    let mut out = String::from("[\n");

    for (ri, record) in records.iter().enumerate() {
        let row = record.export_row();
        out.push_str("  {");
        for (hi, &idx) in indices.iter().enumerate() {
            let key = headers[hi];
            let val = row.get(idx).map(|s| s.as_str()).unwrap_or("");
            let escaped_val = json_escape(val);
            out.push_str(&format!("\"{key}\": \"{escaped_val}\""));
            if hi + 1 < indices.len() {
                out.push_str(", ");
            }
        }
        out.push('}');
        if ri + 1 < records.len() {
            out.push(',');
        }
        out.push('\n');
    }

    out.push(']');
    Ok(out)
}

/// Escape special characters for JSON string values.
fn json_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

/// Render a basic HTML table.
fn render_html<T: Exportable>(
    records: &[T],
    headers: &[&str],
    indices: &[usize],
    include_headers: bool,
) -> Result<String, String> {
    let mut out = String::from("<table>\n");

    if include_headers {
        out.push_str("  <thead>\n    <tr>");
        for h in headers {
            out.push_str(&format!("<th>{}</th>", html_escape(h)));
        }
        out.push_str("</tr>\n  </thead>\n");
    }

    out.push_str("  <tbody>\n");
    for record in records {
        let row = record.export_row();
        out.push_str("    <tr>");
        for &idx in indices {
            let val = row.get(idx).map(|s| s.as_str()).unwrap_or("");
            out.push_str(&format!("<td>{}</td>", html_escape(val)));
        }
        out.push_str("</tr>\n");
    }
    out.push_str("  </tbody>\n</table>");

    Ok(out)
}

/// Escape HTML special characters.
fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Render basic XML elements.
fn render_xml<T: Exportable>(
    records: &[T],
    headers: &[&str],
    indices: &[usize],
) -> Result<String, String> {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<records>\n");

    for record in records {
        let row = record.export_row();
        out.push_str("  <record>\n");
        for (hi, &idx) in indices.iter().enumerate() {
            let tag = xml_tag_name(headers[hi]);
            let val = row.get(idx).map(|s| s.as_str()).unwrap_or("");
            out.push_str(&format!("    <{tag}>{}</{tag}>\n", xml_escape(val)));
        }
        out.push_str("  </record>\n");
    }

    out.push_str("</records>");
    Ok(out)
}

/// Convert a header string into a valid XML tag name by replacing
/// non-alphanumeric characters with underscores.
fn xml_tag_name(header: &str) -> String {
    header
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect()
}

/// Escape XML special characters.
fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyRecord {
        name: String,
        value: String,
    }

    impl Exportable for DummyRecord {
        fn export_headers(&self) -> Vec<String> {
            vec!["Name".to_string(), "Value".to_string()]
        }

        fn export_row(&self) -> Vec<String> {
            vec![self.name.clone(), self.value.clone()]
        }
    }

    fn sample_records() -> Vec<DummyRecord> {
        vec![
            DummyRecord {
                name: "firefox".to_string(),
                value: "12.4".to_string(),
            },
            DummyRecord {
                name: "code".to_string(),
                value: "3.1".to_string(),
            },
        ]
    }

    #[test]
    fn test_csv_export() {
        let records = sample_records();
        let config = ExportConfig::default();
        let output = export_records(&records, &config).unwrap();
        assert!(output.starts_with("Name,Value\n"));
        assert!(output.contains("firefox,12.4\n"));
        assert!(output.contains("code,3.1\n"));
    }

    #[test]
    fn test_csv_no_headers() {
        let records = sample_records();
        let config = ExportConfig {
            include_headers: false,
            ..Default::default()
        };
        let output = export_records(&records, &config).unwrap();
        assert!(!output.contains("Name,Value"));
        assert!(output.starts_with("firefox,12.4\n"));
    }

    #[test]
    fn test_tsv_export() {
        let records = sample_records();
        let config = ExportConfig {
            format: ExportFormat::Tsv,
            ..Default::default()
        };
        let output = export_records(&records, &config).unwrap();
        assert!(output.contains("Name\tValue\n"));
    }

    #[test]
    fn test_json_export() {
        let records = sample_records();
        let config = ExportConfig {
            format: ExportFormat::Json,
            ..Default::default()
        };
        let output = export_records(&records, &config).unwrap();
        assert!(output.starts_with("[\n"));
        assert!(output.contains("\"Name\": \"firefox\""));
        assert!(output.ends_with(']'));
    }

    #[test]
    fn test_html_export() {
        let records = sample_records();
        let config = ExportConfig {
            format: ExportFormat::Html,
            ..Default::default()
        };
        let output = export_records(&records, &config).unwrap();
        assert!(output.contains("<table>"));
        assert!(output.contains("<th>Name</th>"));
        assert!(output.contains("<td>firefox</td>"));
        assert!(output.contains("</table>"));
    }

    #[test]
    fn test_xml_export() {
        let records = sample_records();
        let config = ExportConfig {
            format: ExportFormat::Xml,
            ..Default::default()
        };
        let output = export_records(&records, &config).unwrap();
        assert!(output.contains("<records>"));
        assert!(output.contains("<Name>firefox</Name>"));
        assert!(output.contains("</records>"));
    }

    #[test]
    fn test_column_filter() {
        let records = sample_records();
        let config = ExportConfig {
            columns: Some(vec!["Value".to_string()]),
            ..Default::default()
        };
        let output = export_records(&records, &config).unwrap();
        assert!(output.starts_with("Value\n"));
        assert!(!output.contains("Name"));
    }

    #[test]
    fn test_empty_records() {
        let records: Vec<DummyRecord> = vec![];
        let config = ExportConfig::default();
        let output = export_records(&records, &config).unwrap();
        assert!(output.is_empty());
    }

    #[test]
    fn test_empty_records_json() {
        let records: Vec<DummyRecord> = vec![];
        let config = ExportConfig {
            format: ExportFormat::Json,
            ..Default::default()
        };
        let output = export_records(&records, &config).unwrap();
        assert_eq!(output, "[]");
    }
}
