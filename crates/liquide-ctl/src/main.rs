mod cli;
mod client;
mod commands;
mod config;
mod error;
mod output;

use clap::Parser;
use cli::{Cli, Command};
use client::Client;
use error::ExitCode;
use output::{Output, should_colorize};

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let cli = Cli::parse();

    // Initialize tracing
    let filter = if cli.verbose { "debug" } else { "warn" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(filter)),
        )
        .with_writer(std::io::stderr)
        .init();

    // Handle completions immediately (no server connection needed)
    if let Command::Completions(args) = &cli.command {
        generate_completions(args.shell);
        return ExitCode::Success.into();
    }

    // Load config
    let ctl_config = match config::CtlConfig::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error loading config: {e}");
            return ExitCode::GeneralError.into();
        }
    };

    // Resolve server address
    let server = match ctl_config.resolve_server(cli.server.as_deref()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error resolving server: {e}");
            return ExitCode::GeneralError.into();
        }
    };

    // Resolve API key
    let api_key = ctl_config.resolve_api_key(cli.api_key.as_deref(), cli.server.as_deref());

    // Build client and output
    let client = Client::new(server, api_key);
    let color = should_colorize(&cli.color);
    let output = Output::new(cli.format.clone(), color, cli.quiet);

    // Dispatch command
    let result = dispatch(&cli.command, &client, &output).await;

    match result {
        Ok(()) => ExitCode::Success.into(),
        Err(e) => {
            output.error(&e.to_string());
            e.exit_code().into()
        }
    }
}

async fn dispatch(command: &Command, client: &Client, output: &Output) -> error::Result<()> {
    match command {
        Command::Status(args) => commands::status::execute(client, output, args).await,
        Command::Sessions(cmd) => commands::sessions::execute(client, output, cmd).await,
        Command::Users(cmd) => commands::users::execute(client, output, cmd).await,
        Command::Stats(args) => commands::stats::execute(client, output, args).await,
        Command::Benchmark(args) => commands::benchmark::execute(client, output, args).await,
        Command::Config(cmd) => commands::config::execute(client, output, cmd).await,
        Command::Policy(cmd) => commands::policy::execute(client, output, cmd).await,
        Command::Monitors(cmd) => commands::monitors::execute(client, output, cmd).await,
        Command::Transport(cmd) => commands::transport::execute(client, output, cmd).await,
        Command::Audio(cmd) => commands::audio::execute(client, output, cmd).await,
        Command::Encoder(cmd) => commands::encoder::execute(client, output, cmd).await,
        Command::Usb(cmd) => commands::usb::execute(client, output, cmd).await,
        Command::Logs(cmd) => commands::logs::execute(client, output, cmd).await,
        Command::Audit(cmd) => commands::audit::execute(client, output, cmd).await,
        Command::Honeypot(cmd) => commands::honeypot::execute(client, output, cmd).await,
        Command::Lock(args) => commands::lock::execute_lock(client, output, args).await,
        Command::Unlock(args) => commands::lock::execute_unlock(client, output, args).await,
        Command::Gateway(cmd) => commands::gateway::execute(client, output, cmd).await,
        Command::Service(cmd) => commands::service::execute(client, output, cmd).await,
        Command::Cache(cmd) => commands::cache::execute(client, output, cmd).await,
        Command::Rdp(cmd) => commands::rdp::execute(client, output, cmd).await,
        Command::Plugins(cmd) => commands::plugins::execute(client, output, cmd).await,
        Command::Crash(cmd) => commands::crash::execute(client, output, cmd).await,
        Command::Supervisor(cmd) => commands::supervisor::execute(client, output, cmd).await,
        Command::Flatpak(cmd) => commands::flatpak::execute(client, output, cmd).await,
        Command::Brew(cmd) => commands::brew::execute(client, output, cmd).await,
        Command::Snap(cmd) => commands::snap::execute(client, output, cmd).await,
        Command::Nix(cmd) => commands::nix::execute(client, output, cmd).await,
        Command::Appimage(cmd) => commands::appimage::execute(client, output, cmd).await,
        Command::Completions(_) => unreachable!("handled above"),
    }
}

/// Generate shell completions and print to stdout.
fn generate_completions(shell: clap_complete::Shell) {
    use clap::CommandFactory;
    let mut cmd = Cli::command();
    clap_complete::generate(shell, &mut cmd, "liquidctl", &mut std::io::stdout());
}
