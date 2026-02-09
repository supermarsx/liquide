use crate::cli::UsbCommand;
use crate::client::Client;
use crate::error::Result;
use crate::output::Output;

pub async fn execute(_client: &Client, output: &Output, cmd: &UsbCommand) -> Result<()> {
    match cmd {
        UsbCommand::Status => {
            output.message("USB/IP subsystem status not available (not connected).");
        }
        UsbCommand::List(args) => {
            output.message(&format!(
                "USB devices for session {} not available (not connected).",
                args.session
            ));
        }
        UsbCommand::Disconnect(args) => {
            output.message(&format!(
                "Disconnecting USB device {} from session {}...",
                args.device_id, args.session_id
            ));
        }
        UsbCommand::DisconnectAll(args) => {
            output.message(&format!(
                "Disconnecting all USB devices from session {}...",
                args.session_id
            ));
        }
    }
    Ok(())
}
