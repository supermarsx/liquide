# LiquidDE Gateway — Specification

> **Language**: Rust
> **License**: MIT
> **Related specs**: [Server/DE](spec.md) · [Client](spec-client.md) · [Management UI](spec-manager.md) · [liquidctl CLI](spec-liquidctl.md) · [Design Language](spec-design.md)

---

## 0) Overview

**liquid-gateway** is a connection broker and relay server for LiquidDE deployments where direct client-to-server connectivity is not possible — typically when servers sit behind NAT, firewalls, or in private networks.

The gateway is a lightweight, standalone Rust binary that runs on a publicly reachable host. It handles connection brokering, optional traffic relay, authentication passthrough, and load distribution.

---

## 1) Use Cases

### Primary
- **NAT traversal**: servers behind NAT/firewall, clients on the internet.
- **Enterprise edge**: single public entry point to multiple internal LiquidDE servers.
- **Zero-trust access**: gateway as the only exposed endpoint, servers never directly reachable.

### Secondary
- **Load balancing**: distribute connections across a pool of servers.
- **Geographic routing**: connect clients to the nearest server.
- **Connection auditing**: centralized logging of all connection attempts.

---

## 2) Architecture

```
Internet                          │  Private Network
                                  │
┌──────────┐    QUIC/TLS    ┌─────┴──────┐    Internal    ┌─────────────┐
│  Client   │ ────────────→ │  Gateway    │ ────────────→ │  LiquidDE   │
│           │ ←──────────── │             │ ←──────────── │  Server(s)  │
└──────────┘    Session     └─────┬──────┘    Relay or    └─────────────┘
                stream            │           Direct
                                  │
                            ┌─────┴──────┐
                            │  Another   │
                            │  Server    │
                            └────────────┘
```

### Connection Modes

#### 1. Broker-Only (Preferred)
- Gateway authenticates the client and provides connection details for the target server.
- Client establishes a direct connection to the server (if network allows).
- Gateway is not in the data path after brokering.
- Lowest latency, minimal gateway load.

#### 2. Full Relay
- All traffic between client and server passes through the gateway.
- Used when direct connectivity is not possible (strict NAT, firewall rules).
- Gateway forwards encrypted traffic (does not decrypt session data).
- Higher latency, gateway must handle bandwidth.

#### 3. TURN-Style Relay
- Gateway provides relay candidates.
- Client and server attempt direct connection first (ICE-like).
- Fall back to relay only if direct connection fails.
- Automatic, transparent to the user.

#### 4. Reverse Connection
- Server registers with the gateway and keeps a persistent control channel open.
- When a client requests a session, the gateway instructs the server to connect back.
- Server initiates the outbound data connection (bypasses NAT without port forwarding).
- Client connects to the gateway, gateway splices the connections.

---

## 3) Server Registration

Servers register with the gateway to advertise availability.

### Registration Flow
1. Server starts with gateway configuration enabled.
2. Server connects to gateway using a **registration token** (shared secret or certificate).
3. Server sends a registration message:
   - Server ID / hostname.
   - Capabilities (encoders, transports, max sessions).
   - Current load (active sessions, CPU, memory).
   - Supported auth methods.
4. Gateway acknowledges registration.
5. Server maintains a persistent keepalive connection.
6. Server periodically updates its load status.

### Registration Configuration (Server Side)

```toml
# In server.toml
[gateway]
enabled = true
gateway_url = "wss://gateway.example.com:443"
reverse_connect = true
registration_token = "secret-token-here"
keepalive_interval_sec = 30
reconnect_on_failure = true
reconnect_delay_sec = 5
```

### Server Health Checks
- Gateway pings registered servers periodically.
- Unresponsive servers are marked "unhealthy" after configurable timeout.
- Unhealthy servers are excluded from client routing.
- Servers automatically re-register when connectivity is restored.

---

## 4) Client Connection Flow

### Step-by-Step

1. **Client connects to gateway** (QUIC or TLS/TCP).
2. **Authentication**:
   - Gateway can perform its own authentication (gateway-level auth).
   - Or pass through to the target server's auth system.
   - Or combine both (gateway auth + server auth).
3. **Server selection**:
   - Client specifies a target server by name/ID.
   - Or gateway presents a list of available servers.
   - Or gateway auto-selects based on load balancing / routing rules.
4. **Connection brokering**:
   - Gateway determines the best connection mode (direct, relay, reverse).
   - Sends connection instructions to client (and server, for reverse connections).
5. **Session establishment**:
   - In broker mode: client connects directly to server.
   - In relay mode: gateway forwards traffic.
   - In reverse mode: server connects back through gateway.
6. **Ongoing**:
   - Gateway monitors connection health.
   - Can force disconnect if policy requires.
   - Logs connection metadata.

### Client Configuration

```toml
# In client config.toml
[gateway]
enabled = true
url = "wss://gateway.example.com:443"
auth_token = ""                    # or interactive auth
auto_discover = true               # mDNS / DNS-SD for LAN gateways
prefer_direct = true               # attempt direct connection first
```

---

## 5) Authentication

### Gateway-Level Authentication
- Gateway can enforce its own auth layer before allowing server access.
- Methods:
  - **Token-based**: pre-shared bearer tokens.
  - **Username/password**: gateway-local accounts.
  - **OIDC/OAuth2**: integrate with identity providers (Okta, Auth0, Azure AD, etc.).
  - **Client certificates**: mTLS.
  - **API keys**: for programmatic access.

### Authentication Passthrough
- Gateway can pass authentication through to the target server.
- Client authenticates with the server's auth system (PAM, LDAP, etc.) through the gateway tunnel.
- Gateway does not see credentials (encrypted end-to-end).

### Combined Authentication
- Gateway auth first (is this user allowed to use the gateway?).
- Server auth second (is this user allowed to access this specific server?).
- Two-step process presented as a single flow to the user.

---

## 6) Routing & Load Balancing

### Routing Strategies

| Strategy | Description |
|----------|-------------|
| **Direct** | Client specifies exact server. |
| **Round-robin** | Distribute across all healthy servers. |
| **Least-load** | Route to server with fewest active sessions. |
| **Least-latency** | Route to server with lowest RTT from client (if measurable). |
| **Geographic** | Route based on client IP geolocation. |
| **Tag-based** | Route to servers matching specific tags (e.g., `gpu=true`, `team=engineering`). |
| **Sticky** | Return client to the same server they used previously (for session resume). |

### Server Tags
- Servers can register with arbitrary tags:
  ```toml
  [gateway]
  tags = { gpu = "true", team = "engineering", location = "us-west" }
  ```
- Clients can request routing based on tags: `liquidclient --gateway-tag gpu=true`.

---

## 7) Transport

### Client ↔ Gateway
- **QUIC** (preferred): multiplexed, encrypted, low latency.
- **TLS/TCP**: fallback for restrictive networks.
- **WebSocket (TLS)**: for web clients or corporate proxies.

### Gateway ↔ Server
- **QUIC** (preferred): same advantages.
- **TLS/TCP**: when server network doesn't support UDP.
- **Internal plaintext**: optional for trusted internal networks (policy-guarded).

### Relay Mode Transport
- Gateway **does not decrypt** session data in relay mode.
- Gateway operates at the transport level — it forwards encrypted byte streams.
- This means the gateway adds latency but does not compromise session encryption.
- Optional **connection splicing** for zero-copy relay on supported platforms.

---

## 8) Multiple Listening Modes

```toml
[[listen]]
address = "0.0.0.0:443"
transport = "quic"
tls_cert = "/etc/liquid-gateway/cert.pem"
tls_key = "/etc/liquid-gateway/key.pem"

[[listen]]
address = "0.0.0.0:443"
transport = "tls-tcp"
tls_cert = "/etc/liquid-gateway/cert.pem"
tls_key = "/etc/liquid-gateway/key.pem"

[[listen]]
address = "0.0.0.0:8443"
transport = "websocket-tls"
tls_cert = "/etc/liquid-gateway/cert.pem"
tls_key = "/etc/liquid-gateway/key.pem"

# Internal management API (localhost only)
[[listen]]
address = "127.0.0.1:9090"
transport = "http"
role = "management-api"
```

---

## 9) Reverse Connection

### How It Works
1. Server registers with gateway and maintains a persistent control channel.
2. Client connects to gateway and requests a session.
3. Gateway sends a "connect-back" command to the server via the control channel.
4. Server initiates an outbound connection to the gateway (or directly to the client if possible).
5. Gateway splices the client and server connections.

### Benefits
- **No port forwarding** on the server's network.
- **No public IP** needed for the server.
- Server only makes outbound connections (easier firewall rules).

### Reverse Connection Settings

```toml
# Server-side
[gateway]
reverse_connect = true
reverse_connect_timeout_sec = 10   # max time for server to establish back-connection

# Gateway-side
[reverse_connection]
enabled = true
max_pending_requests = 100
timeout_sec = 10
```

---

## 10) Security

### Transport Security
- All connections (client ↔ gateway, gateway ↔ server) use TLS 1.3.
- Certificate management:
  - ACME/Let's Encrypt for public gateways.
  - Enterprise PKI.
  - Self-signed with fingerprint verification.

### Server Authentication
- Servers must authenticate to register with the gateway.
- Methods:
  - Registration token (shared secret).
  - Client certificate (mTLS).
  - Both (token + certificate).

### Access Control
- Gateway can restrict:
  - Which clients can connect (IP allowlists, auth requirements).
  - Which servers a client can access (per-user or per-group ACLs).
  - Time-of-day restrictions.
  - Concurrent connection limits per user.

### Audit Logging
- All events logged:
  - Client connection attempts (success/failure).
  - Server registrations/deregistrations.
  - Session brokering events.
  - Routing decisions.
  - Policy enforcement actions.
- Log format: structured JSON.
- Export: file, syslog, or webhook.

---

## 11) Configuration

### Configuration File

`/etc/liquid-gateway/gateway.toml`

```toml
# ─── General ────────────────────────────────────────────────
[general]
hostname = "liquid-gateway-01"
log_level = "info"
log_format = "json"
log_file = "/var/log/liquid-gateway/gateway.log"
data_dir = "/var/lib/liquid-gateway"

# ─── Listening ──────────────────────────────────────────────
[[listen]]
address = "0.0.0.0:443"
transport = "quic"
tls_cert = "/etc/liquid-gateway/cert.pem"
tls_key = "/etc/liquid-gateway/key.pem"

[[listen]]
address = "0.0.0.0:443"
transport = "tls-tcp"
tls_cert = "/etc/liquid-gateway/cert.pem"
tls_key = "/etc/liquid-gateway/key.pem"

# ─── TLS ────────────────────────────────────────────────────
[tls]
acme_enabled = true
acme_domain = "gateway.example.com"
acme_email = "admin@example.com"
min_tls_version = "1.3"

# ─── Authentication ─────────────────────────────────────────
[auth]
# Gateway-level client authentication
mode = "oidc"                        # none, token, password, oidc, mtls
oidc_issuer = "https://auth.example.com"
oidc_client_id = "liquid-gateway"
oidc_client_secret_file = "/etc/liquid-gateway/oidc-secret"

# Server registration authentication
server_auth_mode = "token"           # token, mtls, both
server_registration_tokens = [
  "server-token-alpha",
  "server-token-beta",
]

# ─── Routing ────────────────────────────────────────────────
[routing]
strategy = "least-load"              # direct, round-robin, least-load, least-latency, geographic, tag-based, sticky
sticky_sessions = true               # remember client → server mapping
sticky_ttl_sec = 86400               # how long sticky mapping persists

# ─── Relay ──────────────────────────────────────────────────
[relay]
enabled = true
prefer_direct = true                 # try direct connection first
max_relay_bandwidth_mbps = 1000      # total relay bandwidth cap
per_session_bandwidth_mbps = 100     # per-session relay bandwidth cap
connection_splicing = true           # zero-copy relay where supported

# ─── Reverse Connection ─────────────────────────────────────
[reverse_connection]
enabled = true
max_pending_requests = 100
timeout_sec = 10

# ─── Limits ─────────────────────────────────────────────────
[limits]
max_concurrent_clients = 500
max_concurrent_servers = 50
max_sessions_per_user = 5
connection_rate_limit = 100          # new connections per second
client_idle_timeout_sec = 300

# ─── Health Check ───────────────────────────────────────────
[health_check]
interval_sec = 15
timeout_sec = 5
unhealthy_threshold = 3              # consecutive failures before marking unhealthy
healthy_threshold = 2                # consecutive successes before marking healthy again

# ─── Access Control ─────────────────────────────────────────
[access_control]
client_ip_allowlist = []             # empty = allow all
client_ip_blocklist = []
user_server_acl_enabled = false
user_server_acl_file = "/etc/liquid-gateway/acls.toml"

# ─── Audit ──────────────────────────────────────────────────
[audit]
enabled = true
log_file = "/var/log/liquid-gateway/audit.log"
log_format = "json"
syslog_enabled = false
webhook_url = ""

# ─── Metrics ────────────────────────────────────────────────
[metrics]
prometheus_enabled = true
prometheus_listen = "127.0.0.1:9101"

# ─── Management API ────────────────────────────────────────
[management_api]
enabled = true
listen = "127.0.0.1:9090"
api_key = "gateway-admin-key"
```

### ACL Configuration (`acls.toml`)

```toml
# User-to-server access control
[user.alice]
allowed_servers = ["server-01", "server-02"]
allowed_tags = { team = "engineering" }

[user.bob]
allowed_servers = ["*"]              # all servers

[group.guests]
allowed_servers = ["demo-server"]
max_sessions = 1
```

---

## 12) Management API

The gateway exposes a REST API for management and monitoring:

### Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/v1/status` | Gateway health and status |
| `GET` | `/api/v1/servers` | List registered servers and their status |
| `GET` | `/api/v1/servers/{id}` | Get specific server details |
| `DELETE` | `/api/v1/servers/{id}` | Force-deregister a server |
| `GET` | `/api/v1/sessions` | List active sessions |
| `GET` | `/api/v1/sessions/{id}` | Get specific session details |
| `DELETE` | `/api/v1/sessions/{id}` | Force-disconnect a session |
| `GET` | `/api/v1/users` | List connected users |
| `GET` | `/api/v1/metrics` | Current metrics snapshot |
| `GET` | `/api/v1/config` | Current configuration (redacted secrets) |
| `PUT` | `/api/v1/config` | Update configuration (hot-reload supported) |

### Authentication
- API key in header: `X-API-Key: <key>`.
- Or mTLS for programmatic access.

---

## 13) Observability

### Prometheus Metrics
- `liquid_gateway_active_sessions` — gauge.
- `liquid_gateway_active_servers` — gauge.
- `liquid_gateway_connections_total` — counter (by status: success, failed, rejected).
- `liquid_gateway_relay_bytes_total` — counter (by direction: in, out).
- `liquid_gateway_relay_bandwidth_bps` — gauge.
- `liquid_gateway_auth_attempts_total` — counter (by status, method).
- `liquid_gateway_server_health` — gauge (1=healthy, 0=unhealthy).
- `liquid_gateway_brokering_latency_seconds` — histogram.

### Structured Logs
- JSON format for machine parsing.
- Includes: timestamp, event type, client IP, server ID, session ID, user, result.
- Correlation IDs link events across the brokering flow.

---

## 14) High Availability

### Multi-Gateway Deployment
- Multiple gateway instances can run behind a load balancer.
- Shared state (server registry, sticky sessions) via:
  - Redis.
  - Embedded distributed KV (e.g., etcd-like, raft-based).
  - Or stateless mode (no sticky sessions, DNS-based server registry).

### Failover
- If a gateway instance fails, clients reconnect to another instance.
- Server re-registration happens automatically.
- Active relay sessions are dropped (client auto-reconnects through surviving gateway).

---

## 15) Deployment

### Binary
- Single static Rust binary: `liquid-gateway`.
- No runtime dependencies beyond TLS libraries.

### Platforms
- Linux x86_64 (primary).
- Linux ARM64.

### Installation
- Systemd service file included.
- Docker/OCI container image available.
- Configuration templates with sensible defaults.

---

## 16) Test Plan

### Functional
- Client connects through gateway to server.
- All connection modes (broker, relay, reverse).
- Authentication (all methods).
- Routing strategies.
- Server registration/deregistration.
- Health checks and unhealthy handling.

### Performance
- Relay throughput (target: line-rate for the gateway's NIC).
- Brokering latency (target: <50ms overhead).
- Concurrent session capacity.
- Memory usage under load.

### Reliability
- Gateway restart with active sessions.
- Server disconnect/reconnect during active sessions.
- Network partition between gateway and server.
- Load balancer failover between gateway instances.

### Security
- TLS enforcement.
- Auth bypass attempts.
- ACL enforcement.
- Rate limiting effectiveness.
