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
  - **ACME/Let's Encrypt** (automatic):
    - Gateway can automatically obtain and renew TLS certificates via ACME protocol.
    - Supports HTTP-01 and TLS-ALPN-01 challenge types.
    - Auto-renewal before expiry (configurable threshold, default: 30 days before).
    - Staging environment support for testing.
    - Multiple domain names and SANs supported.
  - Enterprise PKI.
  - Self-signed with fingerprint verification.
- Auto TLS configuration:
  ```toml
  [tls]
  acme_enabled = true
  acme_provider = "letsencrypt"        # letsencrypt, letsencrypt-staging, zerossl, custom
  acme_domain = "gateway.example.com"
  acme_email = "admin@example.com"
  acme_challenge = "tls-alpn-01"       # http-01, tls-alpn-01
  acme_renew_before_days = 30
  acme_http_listen = "0.0.0.0:80"     # only for http-01 challenge
  acme_additional_domains = []         # SANs
  min_tls_version = "1.3"

  # Manual certificate (used when acme_enabled = false)
  cert = "/etc/liquid-gateway/cert.pem"
  key = "/etc/liquid-gateway/key.pem"
  ```

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

### Intrusion Prevention (fail2ban Integration)
- Gateway emits structured authentication events for fail2ban monitoring.
- **Built-in fail2ban jails** ship with the gateway:
  - `liquid-gateway-auth` — ban IPs after repeated client authentication failures.
  - `liquid-gateway-brute` — ban IPs attempting rapid connection attempts.
  - `liquid-gateway-scan` — ban IPs probing service ports without valid protocol.
- Jail configuration (shipped as `/etc/fail2ban/jail.d/liquid-gateway.conf`):
  ```ini
  [liquid-gateway-auth]
  enabled = true
  filter = liquid-gateway-auth
  logpath = /var/log/liquid-gateway/auth.log
  maxretry = 5
  findtime = 600
  bantime = 3600

  [liquid-gateway-brute]
  enabled = true
  filter = liquid-gateway-brute
  logpath = /var/log/liquid-gateway/auth.log
  maxretry = 20
  findtime = 60
  bantime = 86400

  [liquid-gateway-scan]
  enabled = true
  filter = liquid-gateway-scan
  logpath = /var/log/liquid-gateway/gateway.log
  maxretry = 3
  findtime = 60
  bantime = 86400
  ```
- Built-in rate limiting (independent of fail2ban):
  - Configurable per-IP connection rate limits.
  - Progressive delay after failed authentication.
  - Automatic IP blocking after threshold.

### Service Obfuscation
- The gateway supports hiding its identity from network scanners and unauthorized probes.
- **Protocol obfuscation**:
  - Connection attempts without a valid protocol header receive no response (silent drop).
  - Configurable banner/identification:
    - `default` — identifies as liquid-gateway.
    - `minimal` — returns only protocol version.
    - `hidden` — no identification; unknown clients get connection reset.
    - `custom` — administrator-defined response.
  - TLS SNI validation: only accept connections for configured domain names.
- **Port knocking** (optional):
  - Client must send a specific packet sequence before the gateway accepts connections.
  - Sequence shared as part of the client profile.
- **Fingerprint reduction**:
  - TLS certificate does not include product-specific fields.
  - Server timing responses randomized.
  - Error responses are generic.
- Configuration:
  ```toml
  [security.obfuscation]
  service_banner = "hidden"            # default, minimal, hidden, custom
  custom_banner = ""
  silent_drop_unknown = true
  sni_validation = true                # reject connections with wrong SNI
  allowed_sni_domains = ["gateway.example.com"]
  port_knocking_enabled = false
  port_knocking_sequence = [7331, 8442, 9553]
  port_knocking_timeout_sec = 10
  timing_randomization = true
  fingerprint_reduction = true
  ```

### Honeypot & Tarpit (Automatic)

The gateway can automatically detect unambiguously malicious traffic and respond with tarpit/honeypot tactics. Since the gateway is the public-facing entry point, it is the primary defense layer.

#### Triggers (Zero False-Positive Only)

Only patterns that no legitimate client would ever produce:

| Trigger | Response |
|---------|----------|
| **Invalid protocol magic** — non-LiquidDE/non-WebSocket probes (HTTP, SSH, random bytes) | Tarpit: accept, drip-feed data at 1 byte/sec |
| **Known exploit payloads** — RDP/VNC/SSH CVE exploit signatures | Honeypot: fake service, log full payload |
| **Post-ban reconnection** — IP already banned by rate limiter, continues attempts | Tarpit: accept TCP, drip-feed indefinitely |
| **Credential stuffing** — >10 distinct usernames from single IP in 60 seconds | Tarpit: 5-30s fake auth processing per attempt |
| **TLS downgrade attack** — attempts null ciphers or TLS <1.2 after 1.3 was offered | Tarpit: slow handshake, then reject |
| **Port scan follow-up** — IP probed 3+ closed ports in last 60s, now connecting | Honeypot: fake service, log all interaction |
| **Malformed packet flood** — >100 malformed packets/sec from single IP | Tarpit: throttle to 1 response/sec |

**Does NOT trigger** (to avoid false positives): wrong passwords (typos), slow connections (poor network), expired certificates (stale config), old client versions, single auth failures followed by success, unusual connection times.

#### Gateway Tarpit Implementation

- **TCP tarpit**: tiny TCP window (1-10 bytes), 1 byte/sec throughput. Ties up attacker sockets.
- **TLS tarpit**: ServerHello sent one extension per second. Attacker waits 30-60s before timeout.
- **Auth tarpit**: accept credentials, simulate 5-30s "processing" with jitter, always reject.
- **Relay decoy**: for post-ban IPs, pretend to broker a session to a fake server, then stall.
- Dedicated thread pool for tarpit connections (does not affect legitimate traffic).
- Configurable max concurrent tarpit slots (default: 200, higher than server default since gateway is public-facing).

#### Gateway Honeypot

- **Intelligence logging**: all honeypot traffic logged to `honeypot.log` with source IP, payloads, timing, tool fingerprints.
- **IOC export**: indicators of compromise exported in STIX, CSV, or JSON format.
- **Shared threat intelligence**: gateway can push IOC lists to all registered servers so they pre-block known attackers.
- Honeypot is passive — never initiates outbound connections.

#### Configuration

```toml
[security.honeypot]
enabled = true
mode = "both"                            # tarpit, honeypot, both, disabled
tarpit_max_connections = 200             # higher for public-facing gateway
tarpit_byte_rate = 1
tarpit_tls_delay_ms = 1000
tarpit_auth_delay_sec = 15
tarpit_thread_pool_size = 8
honeypot_log = "/var/log/liquid-gateway/honeypot.log"
honeypot_capture_payloads = true
honeypot_max_capture_mb = 200
honeypot_retention_days = 90
trigger_on_invalid_protocol = true
trigger_on_exploit_signatures = true
trigger_on_post_ban_attempts = true
trigger_on_credential_stuffing = true
credential_stuffing_threshold = 10
trigger_on_downgrade_attacks = true
trigger_on_port_scan_followup = true
trigger_on_malformed_floods = true
notify_on_trigger = true
webhook_url = ""
export_iocs = true
ioc_export_format = "stix"
share_iocs_with_servers = true           # push IOC blocklists to registered servers
```

### Extensive Logging
The gateway has a comprehensive logging system:

#### Log Subsystems

| Subsystem | Log File | Contents |
|-----------|----------|----------|
| `gateway` | `gateway.log` | Gateway lifecycle, config changes, listener events |
| `auth` | `auth.log` | Client & server authentication attempts, failures, bans |
| `routing` | `routing.log` | Routing decisions, server selection, load balancing |
| `relay` | `relay.log` | Relay session events, bandwidth, connection splicing |
| `server-reg` | `server-reg.log` | Server registration, deregistration, health checks |
| `health` | `health.log` | Health check results, server status changes |
| `audit` | `audit.log` | Security events (immutable, append-only) |

#### Log Configuration
```toml
[logging]
base_dir = "/var/log/liquid-gateway"
format = "json"                        # json, text, syslog
max_file_size_mb = 100
max_files = 10
compress_rotated = true
syslog_enabled = false
syslog_facility = "local1"
syslog_address = "127.0.0.1:514"

[logging.levels]
gateway = "info"
auth = "info"
routing = "info"
relay = "warn"
server_reg = "info"
health = "warn"
audit = "info"
```

#### Log Features
- **Correlation IDs**: every log entry includes a connection ID and session ID.
- **Structured fields**: all entries are key-value structured, not free-form text.
- **Log rotation**: automatic rotation by size with configurable retention.
- **Syslog forwarding**: RFC 5424 syslog support.
- **Webhook forwarding**: send log events to external systems via HTTP webhook.
- **Audit log immutability**: append-only with HMAC integrity verification.
- **Sensitive data redaction**: tokens, keys, and credentials are never logged.

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
acme_provider = "letsencrypt"
acme_domain = "gateway.example.com"
acme_email = "admin@example.com"
acme_challenge = "tls-alpn-01"
acme_renew_before_days = 30
min_tls_version = "1.3"
# Manual certificate (when acme_enabled = false)
cert = "/etc/liquid-gateway/cert.pem"
key = "/etc/liquid-gateway/key.pem"

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

# ─── Security ─────────────────────────────────────────────
[security]
rate_limit_enabled = true
rate_limit_max_attempts = 5
rate_limit_window_sec = 300
rate_limit_lockout_sec = 600
progressive_delay = true

[security.obfuscation]
service_banner = "hidden"            # default, minimal, hidden, custom
custom_banner = ""
silent_drop_unknown = true
sni_validation = true                # reject connections with wrong SNI
allowed_sni_domains = ["gateway.example.com"]
port_knocking_enabled = false
port_knocking_sequence = [7331, 8442, 9553]
port_knocking_timeout_sec = 10
timing_randomization = true
fingerprint_reduction = true

[security.honeypot]
enabled = true
mode = "both"                            # tarpit, honeypot, both, disabled
tarpit_max_connections = 200
tarpit_auth_delay_sec = 15
tarpit_thread_pool_size = 8
honeypot_log = "/var/log/liquid-gateway/honeypot.log"
honeypot_capture_payloads = true
trigger_on_invalid_protocol = true
trigger_on_exploit_signatures = true
trigger_on_post_ban_attempts = true
trigger_on_credential_stuffing = true
share_iocs_with_servers = true
export_iocs = true
ioc_export_format = "stix"

# ─── Logging ──────────────────────────────────────────────
[logging]
base_dir = "/var/log/liquid-gateway"
format = "json"                        # json, text, syslog
max_file_size_mb = 100
max_files = 10
compress_rotated = true
syslog_enabled = false
syslog_facility = "local1"
syslog_address = "127.0.0.1:514"

[logging.levels]
gateway = "info"
auth = "info"
routing = "info"
relay = "warn"
server_reg = "info"
health = "warn"
audit = "info"

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
