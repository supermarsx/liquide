//! Tests for `export` module types.

use liquide_apps_task_manager::export::*;

// ---------------------------------------------------------------------------
// ExportFormat
// ---------------------------------------------------------------------------

#[test]
fn export_format_all_variants() {
    let variants = [
        ExportFormat::Csv,
        ExportFormat::Json,
        ExportFormat::Tsv,
        ExportFormat::Html,
        ExportFormat::Xml,
    ];
    assert_eq!(variants.len(), 5);
}

#[test]
fn export_format_display() {
    assert_eq!(ExportFormat::Csv.to_string(), "CSV");
    assert_eq!(ExportFormat::Json.to_string(), "JSON");
    assert_eq!(ExportFormat::Tsv.to_string(), "TSV");
    assert_eq!(ExportFormat::Html.to_string(), "HTML");
    assert_eq!(ExportFormat::Xml.to_string(), "XML");
}

#[test]
fn export_format_serde_roundtrip() {
    let val = ExportFormat::Html;
    let json = serde_json::to_string(&val).unwrap();
    let back: ExportFormat = serde_json::from_str(&json).unwrap();
    assert_eq!(back, val);
}

// ---------------------------------------------------------------------------
// ExportConfig
// ---------------------------------------------------------------------------

#[test]
fn export_config_default() {
    let cfg = ExportConfig::default();
    assert_eq!(cfg.format, ExportFormat::Csv);
    assert!(cfg.include_headers);
    assert!(cfg.columns.is_none());
    assert!(cfg.output_path.is_none());
}

#[test]
fn export_config_serde_roundtrip() {
    let cfg = ExportConfig {
        format: ExportFormat::Json,
        include_headers: false,
        columns: Some(vec!["name".into(), "pid".into()]),
        output_path: Some("/tmp/export.json".into()),
    };
    let json = serde_json::to_string(&cfg).unwrap();
    let back: ExportConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(back.format, ExportFormat::Json);
    assert!(!back.include_headers);
    assert_eq!(back.columns.unwrap().len(), 2);
}

// ---------------------------------------------------------------------------
// export_records with Exportable trait
// ---------------------------------------------------------------------------

struct TestRecord {
    name: String,
    value: String,
}

impl Exportable for TestRecord {
    fn export_headers(&self) -> Vec<String> {
        vec!["Name".into(), "Value".into()]
    }
    fn export_row(&self) -> Vec<String> {
        vec![self.name.clone(), self.value.clone()]
    }
}

fn sample_records() -> Vec<TestRecord> {
    vec![
        TestRecord {
            name: "firefox".into(),
            value: "12.4".into(),
        },
        TestRecord {
            name: "code".into(),
            value: "3.1".into(),
        },
    ]
}

#[test]
fn export_csv_basic() {
    let records = sample_records();
    let config = ExportConfig::default();
    let output = export_records(&records, &config).unwrap();
    assert!(output.starts_with("Name,Value\n"));
    assert!(output.contains("firefox,12.4\n"));
    assert!(output.contains("code,3.1\n"));
}

#[test]
fn export_csv_no_headers() {
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
fn export_json_basic() {
    let records = sample_records();
    let config = ExportConfig {
        format: ExportFormat::Json,
        ..Default::default()
    };
    let output = export_records(&records, &config).unwrap();
    assert!(output.starts_with("[\n"));
    assert!(output.contains("\"Name\""));
    assert!(output.contains("firefox"));
    assert!(output.ends_with(']'));
}

#[test]
fn export_tsv_basic() {
    let records = sample_records();
    let config = ExportConfig {
        format: ExportFormat::Tsv,
        ..Default::default()
    };
    let output = export_records(&records, &config).unwrap();
    assert!(output.contains("Name\tValue\n"));
}

#[test]
fn export_html_basic() {
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
fn export_xml_basic() {
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
fn export_column_filter() {
    let records = sample_records();
    let config = ExportConfig {
        columns: Some(vec!["Value".into()]),
        ..Default::default()
    };
    let output = export_records(&records, &config).unwrap();
    assert!(output.starts_with("Value\n"));
    assert!(!output.contains("Name"));
}

#[test]
fn export_empty_records_csv() {
    let records: Vec<TestRecord> = vec![];
    let config = ExportConfig::default();
    let output = export_records(&records, &config).unwrap();
    assert!(output.is_empty());
}

#[test]
fn export_empty_records_json() {
    let records: Vec<TestRecord> = vec![];
    let config = ExportConfig {
        format: ExportFormat::Json,
        ..Default::default()
    };
    let output = export_records(&records, &config).unwrap();
    assert_eq!(output, "[]");
}

#[test]
fn export_unknown_column_error() {
    let records = sample_records();
    let config = ExportConfig {
        columns: Some(vec!["NonExistent".into()]),
        ..Default::default()
    };
    let result = export_records(&records, &config);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("unknown column"));
}
