use anyhow::{Context, Result};
use clap::Parser;
use tracing::info;

use liquide_gateway::{
    ClusterConfig, GatewayConfig, GatewayRuntime, HealthCheckConfig, LimitsConfig, ListenConfig,
    ManagementApiConfig, RelayConfig, RoutingConfig, TransportListener,
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

    /// Path to the PEM TLS certificate chain for client connections.
    #[arg(long, default_value = "/etc/liquide/certs/gateway.crt")]
    tls_cert: String,

    /// Path to the PEM TLS private key for client connections.
    #[arg(long, default_value = "/etc/liquide/certs/gateway.key")]
    tls_key: String,

    /// Backend session server address to register at startup
    /// (`host:port`). May be repeated. Without at least one routable backend
    /// the gateway has no session target.
    #[arg(long = "backend")]
    backends: Vec<String>,
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

    // Configure TLS so the gateway can actually terminate client connections.
    // Without a valid cert/key the binary refuses to start rather than silently
    // dropping every client at the TLS step.
    let tls_config = liquide_gateway::load_server_tls_config(&cli.tls_cert, &cli.tls_key)
        .map_err(|e| anyhow::anyhow!("failed to load TLS config: {e}"))
        .with_context(|| {
            format!(
                "cert={}, key={} — provide --tls-cert/--tls-key",
                cli.tls_cert, cli.tls_key
            )
        })?;
    runtime.set_tls_config(tls_config);
    info!(cert = %cli.tls_cert, "TLS configured");

    // Register backend session servers so routing has a real target. A gateway
    // with no routable backend rejects every connection at the routing step.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    for backend in &cli.backends {
        match runtime.handle_server_registration(
            backend.clone(),
            liquide_gateway::ServerCapabilities::default(),
            now,
        ) {
            Ok(server_id) => {
                // Mark the freshly registered backend healthy so routing can
                // select it immediately; subsequent health-check ticks maintain
                // this based on heartbeats.
                runtime
                    .server_registry_mut()
                    .update_health(&server_id, liquide_gateway::ServerHealth::Healthy);
                info!(server_id = %server_id, addr = %backend, "registered backend session server");
            }
            Err(e) => {
                tracing::warn!(addr = %backend, err = %e, "failed to register backend");
            }
        }
    }
    if cli.backends.is_empty() {
        tracing::warn!(
            "no backend session servers registered (--backend); routing will reject clients"
        );
    }

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
                        runtime.handle_tcp_connection(stream, peer_addr).await;
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
