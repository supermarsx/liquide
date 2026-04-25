use serde::{Deserialize, Serialize};

use crate::cli::ConfigCommand;
use crate::client::{ApiResponse, Client};
use crate::error::Result;
use crate::output::Output;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ValidationResult {
    pub valid: bool,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

pub async fn execute(client: &Client, output: &Output, cmd: &ConfigCommand) -> Result<()> {
    match cmd {
        ConfigCommand::Show(args) => show(client, output, args).await,
        ConfigCommand::Validate => validate(client, output).await,
        ConfigCommand::Set(args) => set(client, output, args).await,
        ConfigCommand::Diff => diff(client, output).await,
        ConfigCommand::Export => export(client, output).await,
        ConfigCommand::Import(args) => import(client, output, args).await,
    }
}

async fn show(client: &Client, output: &Output, args: &crate::cli::ConfigShowArgs) -> Result<()> {
    let path = match &args.section {
        Some(section) => format!("/api/v1/config?section={section}"),
        None => "/api/v1/config".to_string(),
    };
    let resp: ApiResponse<serde_json::Value> = client.get(&path).await?;
    match resp.data {
        Some(config) => {
            output.message(&serde_json::to_string_pretty(&config).unwrap_or_default());
        }
        None => {
            if let Some(err) = resp.error {
                output.error(&err);
            } else {
                output.message("No configuration data.");
            }
        }
    }
    Ok(())
}

async fn validate(client: &Client, output: &Output) -> Result<()> {
    let resp: ApiResponse<ValidationResult> = client
        .post("/api/v1/config/validate", &serde_json::json!({}))
        .await?;
    match resp.data {
        Some(result) => {
            if result.valid {
                output.success("Configuration is valid.");
            } else {
                for err in &result.errors {
                    output.error(err);
                }
            }
            for warning in &result.warnings {
                output.warn(warning);
            }
        }
        None => {
            if let Some(err) = resp.error {
                output.error(&err);
            }
        }
    }
    Ok(())
}

async fn set(client: &Client, output: &Output, args: &crate::cli::ConfigSetArgs) -> Result<()> {
    let path = format!("/api/v1/config/{}", args.key);
    let body = serde_json::json!({ "value": args.value });
    let resp: ApiResponse<serde_json::Value> = client.put(&path, &body).await?;
    if resp.success {
        output.success(&format!("Set {} = {}", args.key, args.value));
    } else if let Some(err) = resp.error {
        output.error(&err);
    }
    Ok(())
}

async fn diff(client: &Client, output: &Output) -> Result<()> {
    let resp: ApiResponse<serde_json::Value> = client.get("/api/v1/config/diff").await?;
    match resp.data {
        Some(diff) => {
            output.message(&serde_json::to_string_pretty(&diff).unwrap_or_default());
        }
        None => {
            if resp.success {
                output.message("No differences.");
            } else if let Some(err) = resp.error {
                output.error(&err);
            }
        }
    }
    Ok(())
}

async fn export(client: &Client, output: &Output) -> Result<()> {
    let resp: ApiResponse<serde_json::Value> = client.get("/api/v1/config/export").await?;
    match resp.data {
        Some(config) => {
            // Raw JSON output for piping to file
            println!(
                "{}",
                serde_json::to_string_pretty(&config).unwrap_or_default()
            );
        }
        None => {
            if let Some(err) = resp.error {
                output.error(&err);
            }
        }
    }
    Ok(())
}

async fn import(
    client: &Client,
    output: &Output,
    args: &crate::cli::ConfigImportArgs,
) -> Result<()> {
    let content = std::fs::read_to_string(&args.file)
        .map_err(|e| anyhow::anyhow!("Failed to read {}: {e}", args.file))?;
    let body: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Invalid config file {}: {e}", args.file))?;
    let resp: ApiResponse<serde_json::Value> = client.post("/api/v1/config/import", &body).await?;
    if resp.success {
        output.success(&format!("Configuration imported from {}.", args.file));
    } else if let Some(err) = resp.error {
        output.error(&err);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_validation_result_valid() {
        let json = r#"{
            "success": true,
            "data": {
                "valid": true,
                "errors": [],
                "warnings": ["deprecated key: foo"]
            }
        }"#;
        let resp: ApiResponse<ValidationResult> = serde_json::from_str(json).unwrap();
        assert!(resp.success);
        let data = resp.data.unwrap();
        assert!(data.valid);
        assert!(data.errors.is_empty());
        assert_eq!(data.warnings.len(), 1);
    }

    #[test]
    fn deserialize_validation_result_invalid() {
        let json = r#"{
            "success": true,
            "data": {
                "valid": false,
                "errors": ["missing required key: display.resolution"],
                "warnings": []
            }
        }"#;
        let resp: ApiResponse<ValidationResult> = serde_json::from_str(json).unwrap();
        let data = resp.data.unwrap();
        assert!(!data.valid);
        assert_eq!(data.errors.len(), 1);
    }
}
