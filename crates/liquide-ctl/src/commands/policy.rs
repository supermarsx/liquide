use crate::cli::PolicyCommand;
use crate::client::{ApiResponse, Client};
use crate::error::Result;
use crate::output::Output;

pub async fn execute(client: &Client, output: &Output, cmd: &PolicyCommand) -> Result<()> {
    match cmd {
        PolicyCommand::Show => show(client, output).await,
        PolicyCommand::Set(args) => set(client, output, args).await,
        PolicyCommand::Effective(args) => effective(client, output, args).await,
    }
}

async fn show(client: &Client, output: &Output) -> Result<()> {
    let resp: ApiResponse<serde_json::Value> = client.get("/api/v1/policy").await?;
    match resp.data {
        Some(policies) => {
            output.message(&serde_json::to_string_pretty(&policies).unwrap_or_default());
        }
        None => {
            if let Some(err) = resp.error {
                output.error(&err);
            } else {
                output.message("No policies configured.");
            }
        }
    }
    Ok(())
}

async fn set(client: &Client, output: &Output, args: &crate::cli::PolicySetArgs) -> Result<()> {
    let path = format!("/api/v1/policy/{}/{}", args.scope, args.key);
    let body = serde_json::json!({ "value": args.value });
    let resp: ApiResponse<serde_json::Value> = client.put(&path, &body).await?;
    if resp.success {
        output.success(&format!("Set {}.{} = {}", args.scope, args.key, args.value));
    } else if let Some(err) = resp.error {
        output.error(&err);
    }
    Ok(())
}

async fn effective(
    client: &Client,
    output: &Output,
    args: &crate::cli::PolicyEffectiveArgs,
) -> Result<()> {
    let path = format!("/api/v1/policy/effective/{}", args.username);
    let resp: ApiResponse<serde_json::Value> = client.get(&path).await?;
    match resp.data {
        Some(policy) => {
            output.message(&format!("Effective policy for {}:", args.username));
            output.message(&serde_json::to_string_pretty(&policy).unwrap_or_default());
        }
        None => {
            if let Some(err) = resp.error {
                output.error(&err);
            } else {
                output.message(&format!("No effective policy for {}.", args.username));
            }
        }
    }
    Ok(())
}
