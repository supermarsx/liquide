use anyhow::Result;
use clap::Parser;
use tracing::info;

use liquide_gateway::{
    GatewayConfig, RoutingConfig, RelayConfig, LimitsConfig,
    HealthCheckConfig, ManagementApiConfig, ClusterConfig,
    ListenConfig, GatewayRuntime, TransportListener,
};

/// Network gateway for the Liquide desktop environment.
///
/// `liquid-gateway` accepts incoming client connections over the network,
/// performs TLS termination and authentication hand-off, then routes each
/// client to the appropriate session process.
#[derive(Parser, Debug)]
#[command(name = "liquid-gateway", version, about)]
struct Cli {
    /// Path to the gateway configuration file.
    #[arg(long, default_value = "/etc/liquide/gateway.toml")]
    config: String,

    /// Address and port to listen on for incoming client connections.
    #[arg(long, default_value = "0.0.0.0:3900")]
    listen_addr: String,

    /// Address and port to listen on for management API.
    #[arg(long, default_value = "127.0.0.1:3901")]
    management_addr: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    run(cli).await
}

async fn run(cli: Cli) -> Result<()> {
    info!(config = %cli.config, listen_addr = %cli.listen_addr, "Starting liquid-gateway");

    // Load configuration (in production, parse from the config file).
    info!(path = %cli.config, "Loading configuration...");

    let gateway_config = GatewayConfig::default();
    let routing_config = RoutingConfig::default();
    let relay_config = RelayConfig::default();
    let limits_config = LimitsConfig::default();
    let health_config = HealthCheckConfig::default();
    let management_config = ManagementApiConfig {
        enabled: true,
        listen_addr: cli.management_addr.clone(),
        ..ManagementApiConfig::default()
    };
    let cluster_config = ClusterConfig::default();

    // Create the runtime coordinator.
    let mut runtime = GatewayRuntime::new(
        gateway_config,
        routing_config,
        relay_config,
        limits_config,
        health_config,
        management_config,
        cluster_config,
    );

    // Set up and bind the TCP listener (kept outside runtime to avoid
    // borrow conflicts between accept() and handle_tcp_connection()).
    let listen_config = ListenConfig {
        address: cli.listen_addr.clone(),
        ..ListenConfig::default()
    };
    let mut listener = TransportListener::new("listener-1".into(), listen_config);
    listener.start().await.expect("failed to bind listener");

    info!(addr = %cli.listen_addr, "Listener bound");
    info!(
        management_addr = %cli.management_addr,
        "Management API endpoint configured"
    );
    info!(
        hostname = %runtime.hostname(),
        "Gateway ready — accepting connections"
    );

    // Event loop.
    let health_interval = tokio::time::Duration::from_secs(10);
    let cleanup_interval = tokio::time::Duration::from_secs(60);

    let mut health_tick = tokio::time::interval(health_interval);
    let mut cleanup_tick = tokio::time::interval(cleanup_interval);

    // Consume the first immediate tick.
    health_tick.tick().await;
    cleanup_tick.tick().await;

    loop {
        tokio::select! {
            // Accept new TCP connections when the listener is active.
            result = listener.accept(), if listener.is_listening() => {
                match result {
                    Ok((stream, peer_addr)) => {
                        runtime.handle_tcp_connection(&stream, peer_addr);
                    }
                    Err(e) => {
                        tracing::warn!(err = %e, "accept error");
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Received shutdown signal — draining connections");
                break;
            }
            _ = health_tick.tick() => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                runtime.health_check_tick(now);

                // Flush audit events.
                for event in runtime.drain_audit_events() {
                    info!(event = %event.event_name(), "audit");
                }
            }
            _ = cleanup_tick.tick() => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                runtime.cleanup_tick(now);
            }
        }
    }

    let status = runtime.status();
    info!(
        servers = status.registered_servers,
        connections = status.active_connections,
        relays = status.active_relays,
        "Final gateway status"
    );

    Ok(())
}
