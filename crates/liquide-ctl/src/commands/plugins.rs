use crate::cli::PluginsCommand;
use crate::client::Client;
use crate::error::Result;
use crate::output::Output;

pub async fn execute(_client: &Client, output: &Output, cmd: &PluginsCommand) -> Result<()> {
    match cmd {
        PluginsCommand::List(_args) => {
            output.message("No plugins installed.");
        }
        PluginsCommand::Info(args) => {
            output.message(&format!(
                "Plugin '{}' information not available (not connected).",
                args.plugin_id
            ));
        }
        PluginsCommand::Install(args) => {
            output.message(&format!("Installing plugin from '{}'...", args.source));
        }
        PluginsCommand::Uninstall(args) => {
            output.message(&format!("Uninstalling plugin '{}'...", args.plugin_id));
        }
        PluginsCommand::Enable(args) => {
            output.message(&format!("Enabling plugin '{}'...", args.plugin_id));
        }
        PluginsCommand::Disable(args) => {
            output.message(&format!("Disabling plugin '{}'...", args.plugin_id));
        }
        PluginsCommand::Reload(args) => {
            output.message(&format!("Reloading plugin '{}'...", args.plugin_id));
        }
        PluginsCommand::Config(args) => {
            output.message(&format!(
                "Plugin '{}' config not available (not connected).",
                args.plugin_id
            ));
        }
    }
    Ok(())
}
