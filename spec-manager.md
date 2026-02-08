# LiquiDE Management UI — Specification

> **Language**: Node.js (server) + Web (frontend)
> **License**: MIT
> **Related specs**: [Server/DE](spec.md) · [Client](spec-client.md) · [Gateway](spec-gateway.md) · [liquidctl CLI](spec-liquidctl.md) · [Design Language](spec-design.md)

---

## 0) Overview

**liquid-manager** is a lightweight web-based management interface for LiquiDE deployments. It provides administrators with a visual dashboard for monitoring sessions, managing servers, configuring policies, and viewing metrics.

The management server is a **simple Node.js application** that is **disabled by default**. It runs as a separate process from the LiquiDE server and communicates with server instances via their management APIs.

---

## 1) Design Philosophy

- **Disabled by default** — must be explicitly enabled and configured.
- **Runs as a separate process** — does not affect LiquiDE server performance or stability.
- **Simple to deploy** — single `npm start` or `node server.js` command.
- **Read-mostly** — primarily for monitoring; configuration changes require explicit confirmation.
- **Secure** — requires authentication, supports HTTPS, and never stores session credentials.

---

## 2) Architecture

```
┌─────────────────┐         ┌──────────────────┐
│  Web Browser     │  HTTPS  │  liquid-manager   │
│  (Admin)         │ ──────→ │  (Node.js)        │
│                  │ ←────── │                   │
└─────────────────┘         └────────┬──────────┘
                                     │
                    ┌────────────────┤────────────────┐
                    │ API            │ API            │ API
                    ▼                ▼                ▼
            ┌──────────────┐ ┌──────────────┐ ┌──────────────┐
            │  LiquiDE    │ │  LiquiDE    │ │  liquid-      │
            │  Server 1    │ │  Server 2    │ │  gateway      │
            └──────────────┘ └──────────────┘ └──────────────┘
```

### Components
- **Node.js backend**: serves the web frontend, proxies API calls to LiquiDE servers and gateways.
- **Web frontend**: single-page application with responsive design.
- **No database**: configuration stored in TOML files, state queried from servers in real-time.

---

## 3) Features

### Dashboard
- **Overview panel**:
  - Total active sessions across all servers.
  - Total connected users.
  - Server health summary (healthy/unhealthy/offline).
  - Gateway status (if configured).
  - Aggregate bandwidth usage.
  - System alerts (unhealthy servers, auth failures, policy violations).

### Server Management
- **Server list**:
  - Name, address, status (online/offline/unhealthy).
  - Active sessions count.
  - CPU/memory usage.
  - Uptime.
- **Server detail**:
  - Live metrics (FPS, latency, bandwidth per session).
  - Active sessions with user info.
  - Server configuration viewer.
  - Encoder/transport capabilities.
  - Recent log entries.
- **Actions**:
  - View server configuration.
  - Push configuration changes (with confirmation).
  - Restart server (with confirmation).
  - Drain server (stop accepting new sessions, wait for existing to end).

### Session Management
- **Session list** (across all servers):
  - User, server, duration, status.
  - Current resolution, encoder, transport.
  - Latency, FPS, bandwidth.
- **Session detail**:
  - Live stream statistics.
  - Client info (platform, version, IP).
  - Active features (clipboard, audio, USB, camera).
  - Policy in effect.
- **Actions**:
  - Send message to session.
  - Disconnect session (with confirmation).
  - Shadow session (view-only, if supported).

### User Management
- **User list**:
  - Active sessions, last login, policy group.
- **User detail**:
  - Session history.
  - Active sessions.
  - Applied policies.
- **Actions**:
  - Disconnect all sessions.
  - Change policy group.
  - Lock account.

### Policy Management
- **Policy viewer**: display current server and client policies.
- **Policy editor**: edit policies with a form-based UI.
  - Default policy.
  - Group policies.
  - User-specific overrides.
- **Policy preview**: show what a policy change would affect before applying.
- **Policy history**: track changes with who/when/what.

### Gateway Management (if gateway is configured)
- **Gateway status**: health, connected servers, active sessions.
- **Registered servers**: list with health status.
- **Routing configuration**: view/edit routing strategy.
- **Active relays**: list of active relay sessions with bandwidth usage.

### Metrics & Monitoring
- **Real-time graphs** (WebSocket-driven):
  - Aggregate FPS across all sessions.
  - Aggregate bandwidth (in/out).
  - Session count over time.
  - Latency distribution.
  - Error rate.
- **Per-server graphs**:
  - CPU usage, memory usage.
  - Encode times.
  - Active sessions over time.
- **Historical data**: retain metrics for configurable period (default: 24 hours in-memory, or export to external time-series DB).

### Configuration
- **Server configuration viewer**: read and display `server.toml` from each managed server.
- **Configuration editor**: edit server config with syntax highlighting and validation.
- **Configuration diff**: show changes before applying.
- **Template management**: save and apply configuration templates across servers.

### Log Explorer
- **Centralized log viewer** across all managed servers and gateways.
- **Per-subsystem filtering**: select which log subsystem(s) to view (server, session, auth, render, encode, transport, audio, clipboard, usb, input, policy, metrics, audit).
- **Log level filtering**: trace, debug, info, warn, error.
- **Live streaming**: real-time log tail via WebSocket, with pause/resume.
- **Search**: full-text search across log entries with regex support.
- **Correlation view**: click a session ID to see all log entries across all subsystems for that session.
- **Log level management**: change per-subsystem log levels at runtime from the UI (per server).
- **Download**: export filtered log entries as JSON or text file.
- **Log health**: show log file sizes, rotation status, and disk usage per server.

### Audit Log Viewer
- Searchable, filterable view of audit events from all servers and gateways.
- Filters: event type, user, server, time range, severity.
- Export to CSV/JSON.
- **Integrity verification**: verify HMAC integrity of audit log entries from the UI.
- **Timeline view**: visual timeline of security events for a user or session.

### Honeypot & Tarpit Dashboard
- **Status overview**:
  - Active tarpit connections (count, type breakdown: TCP/TLS/auth).
  - Active honeypot sessions (count, triggers).
  - Tarpit pool utilization (used / max slots).
- **Live activity feed**: real-time stream of tarpit/honeypot events (WebSocket-driven).
- **Attacker table**:
  - IP address, first seen, last seen, total attempts, trigger type, current status (tarpitted/honeypotted/dropped).
  - Geolocation (if IP geolocation data available).
  - Sortable and filterable columns.
- **IOC management**:
  - View collected indicators of compromise (IPs, payload hashes, tool fingerprints).
  - Export IOCs in STIX, CSV, or JSON format.
  - Push IOC blocklists from gateway to all servers (one-click).
- **Trigger configuration**: view and edit honeypot/tarpit trigger thresholds from the UI.
- **Payload viewer**: inspect captured exploit payloads (hex dump + decoded analysis) for security research.
- **Statistics graphs**:
  - Tarpit/honeypot activations over time.
  - Attack type distribution (pie chart).
  - Top attacker IPs (bar chart).
  - Credential stuffing attempts timeline.

### Session Lock Management
- **Lock status overview**: table showing all sessions with lock state (unlocked, screen blank, locked, disconnected+locked, suspended).
- **Bulk lock/unlock**: lock or unlock all sessions or selected sessions with one action.
- **Lock policy viewer**: display effective lock policy per user/group with inheritance chain.
- **Lock timeline**: visual timeline showing lock/unlock/escalation events per session.
- **Actions**:
  - Lock session (with optional custom message).
  - Unlock session (admin override).
  - Modify lock escalation timers per session (override policy temporarily).
  - View lock screen appearance preview.

### Plugin Management
- **Plugin list**: table of all installed plugins with columns: name, ID, version, status (active/suspended/faulted/disabled), memory usage, CPU usage, extension points.
- **Plugin details**: expanded view showing:
  - Full manifest (name, version, author, description, ABI version, capabilities, extension points).
  - Resource usage graphs (memory, CPU over time).
  - Fault history with timestamps and error details.
  - Configuration editor (per-plugin settings).
- **Actions**:
  - Install plugin (upload `.wasm` file or provide registry URL).
  - Uninstall plugin (with confirmation, option to purge config/data).
  - Enable / disable plugin (per-session or globally).
  - Hot-reload plugin (with rollback indicator on failure).
  - Edit plugin configuration.
  - View plugin logs (filtered by plugin ID).
- **Signature verification status**: badge showing whether plugin is signed, unsigned, or signature invalid.
- **Resource limit overrides**: per-plugin memory and CPU limit adjustments (within server-configured maximums).

### Crash Reports & Session Supervisor
- **Crash report list**: table of recent crashes with columns: report ID, session, user, timestamp, error code, exit signal, severity.
  - Filterable by user, session, error code, date range.
  - Sortable by any column.
- **Crash report detail view**:
  - Error code and human-readable description.
  - Stack trace (syntax-highlighted, collapsible).
  - Session metadata (ID, user, uptime, restart count at time of crash).
  - System info (OS, kernel, memory, CPU at time of crash).
  - Last N log lines (configurable).
  - Download button (JSON export, optional coredump inclusion).
- **Crash statistics dashboard**:
  - Total crashes over time (line chart).
  - Crashes by error code (pie/bar chart).
  - Mean time between failures (MTBF) per session, per user.
  - Most affected sessions/users.
- **Supervisor status panel**:
  - Supervisor process status (PID, uptime).
  - Per-session table: session ID, user, PID, state (running/failed/restarting), uptime, restart count, memory, CPU.
  - Real-time updates via WebSocket.
- **Supervisor actions**:
  - Restart a session (with optional force kill).
  - Reset restart counter for a session.
  - View supervisor logs.
  - Adjust session resource limits (cgroup overrides).

---

## 4) Authentication & Security

### Access Control
- Management UI requires authentication.
- Methods:
  - **Local accounts**: defined in `manager.toml`.
  - **OIDC**: integrate with organization's identity provider.
  - **Client certificates**: mTLS.

### Roles
| Role | Permissions |
|------|-------------|
| **Viewer** | Read-only access to dashboards, metrics, session list |
| **Operator** | Viewer + disconnect sessions, send messages |
| **Admin** | Operator + edit policies, push config, restart servers |
| **Super Admin** | Admin + manage users, manage the management UI itself |

### Security Measures
- **HTTPS required** (self-signed cert generated on first run, or user-provided).
- **CSRF protection** on all state-changing actions.
- **Rate limiting** on login attempts.
- **Session timeout** (configurable, default: 30 minutes).
- **Audit logging** of all management actions.
- **No credential storage**: management UI never stores LiquiDE session credentials; it uses API keys to communicate with servers.

---

## 5) Configuration

### Configuration File

`/etc/liquid-manager/manager.toml` (or alongside the application)

```toml
# ─── General ────────────────────────────────────────────────
[general]
hostname = "liquid-manager"
port = 8443
bind_address = "127.0.0.1"         # localhost only by default
log_level = "info"
log_file = "/var/log/liquid-manager/manager.log"

# ─── TLS ────────────────────────────────────────────────────
[tls]
enabled = true
cert = "/etc/liquid-manager/cert.pem"
key = "/etc/liquid-manager/key.pem"
auto_generate_self_signed = true    # generate self-signed if no cert provided

# ─── Authentication ─────────────────────────────────────────
[auth]
mode = "local"                      # local, oidc, mtls
session_timeout_min = 30
max_login_attempts = 5
lockout_duration_min = 15

# Local accounts (only used when mode = "local")
[[auth.local_users]]
username = "admin"
password_hash = "$argon2id$..."      # argon2id hashed password
role = "super-admin"

# OIDC (only used when mode = "oidc")
[auth.oidc]
issuer = "https://auth.example.com"
client_id = "liquid-manager"
client_secret_file = "/etc/liquid-manager/oidc-secret"
role_claim = "liquid_role"           # JWT claim for role mapping

# ─── Managed Servers ────────────────────────────────────────
[[servers]]
name = "server-01"
address = "https://10.0.0.10:9100"
api_key = "server-01-api-key"

[[servers]]
name = "server-02"
address = "https://10.0.0.11:9100"
api_key = "server-02-api-key"

# ─── Managed Gateways ──────────────────────────────────────
[[gateways]]
name = "gateway-01"
address = "https://gateway.example.com:9090"
api_key = "gateway-01-api-key"

# ─── Metrics ────────────────────────────────────────────────
[metrics]
retention_hours = 24                 # in-memory metrics retention
polling_interval_sec = 5             # how often to poll servers
external_tsdb_url = ""               # optional: push to Prometheus remote write

# ─── UI ─────────────────────────────────────────────────────
[ui]
theme = "liquid-glass"               # liquid-glass, light, dark
items_per_page = 25
auto_refresh_sec = 5
show_server_logs = true
show_audit_logs = true
```

---

## 6) API

The management server exposes its own API (used by the web frontend and optionally by automation tools).

### Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/v1/auth/login` | Authenticate and get session token |
| `POST` | `/api/v1/auth/logout` | Invalidate session |
| `GET` | `/api/v1/dashboard` | Aggregate dashboard data |
| `GET` | `/api/v1/servers` | List managed servers |
| `GET` | `/api/v1/servers/{name}` | Server details + live metrics |
| `GET` | `/api/v1/servers/{name}/config` | Server configuration |
| `PUT` | `/api/v1/servers/{name}/config` | Update server configuration |
| `POST` | `/api/v1/servers/{name}/restart` | Restart server |
| `POST` | `/api/v1/servers/{name}/drain` | Drain server |
| `GET` | `/api/v1/sessions` | List all sessions |
| `GET` | `/api/v1/sessions/{id}` | Session details |
| `DELETE` | `/api/v1/sessions/{id}` | Disconnect session |
| `GET` | `/api/v1/users` | List users |
| `GET` | `/api/v1/users/{name}` | User details |
| `GET` | `/api/v1/policies` | List policies |
| `PUT` | `/api/v1/policies` | Update policies |
| `GET` | `/api/v1/gateways` | List gateways |
| `GET` | `/api/v1/gateways/{name}` | Gateway details |
| `GET` | `/api/v1/metrics` | Metrics snapshot |
| `GET` | `/api/v1/audit` | Audit log (with pagination & filters) |
| `GET` | `/api/v1/logs` | Log entries (with subsystem, level, session filters) |
| `PUT` | `/api/v1/servers/{name}/logs/levels` | Change per-subsystem log levels |
| `POST` | `/api/v1/servers/{name}/logs/rotate` | Force log rotation |
| `GET` | `/api/v1/audit/verify` | Verify audit log HMAC integrity |
| `WS` | `/ws/v1/metrics` | WebSocket stream of real-time metrics |
| `WS` | `/ws/v1/logs` | WebSocket stream of real-time log entries |
| `GET` | `/api/v1/honeypot/status` | Honeypot/tarpit status and statistics |
| `GET` | `/api/v1/honeypot/connections` | List active tarpit/honeypot connections |
| `DELETE` | `/api/v1/honeypot/connections/{id}` | Drop a tarpit/honeypot connection |
| `GET` | `/api/v1/honeypot/iocs` | List collected indicators of compromise |
| `POST` | `/api/v1/honeypot/iocs/export` | Export IOCs (STIX, CSV, JSON) |
| `POST` | `/api/v1/honeypot/iocs/push` | Push IOC blocklists to all servers |
| `GET` | `/api/v1/honeypot/triggers` | View trigger configuration |
| `PUT` | `/api/v1/honeypot/triggers` | Update trigger thresholds |
| `WS` | `/ws/v1/honeypot` | WebSocket stream of honeypot/tarpit events |
| `GET` | `/api/v1/sessions/{id}/lock` | Get lock state for a session |
| `POST` | `/api/v1/sessions/{id}/lock` | Lock a session |
| `POST` | `/api/v1/sessions/{id}/unlock` | Unlock a session (admin override) |
| `POST` | `/api/v1/sessions/lock-all` | Lock all sessions |
| `GET` | `/api/v1/lock/policies` | Get lock policies |
| `PUT` | `/api/v1/lock/policies` | Update lock policies |
| `GET` | `/api/v1/plugins` | List all installed plugins |
| `GET` | `/api/v1/plugins/{id}` | Get plugin details (manifest, status, resource usage) |
| `POST` | `/api/v1/plugins/install` | Install a plugin (multipart upload or URL) |
| `DELETE` | `/api/v1/plugins/{id}` | Uninstall a plugin |
| `POST` | `/api/v1/plugins/{id}/enable` | Enable a plugin |
| `POST` | `/api/v1/plugins/{id}/disable` | Disable a plugin |
| `POST` | `/api/v1/plugins/{id}/reload` | Hot-reload a plugin |
| `GET` | `/api/v1/plugins/{id}/config` | Get plugin configuration |
| `PUT` | `/api/v1/plugins/{id}/config` | Update plugin configuration |
| `GET` | `/api/v1/plugins/{id}/faults` | Get plugin fault history |
| `GET` | `/api/v1/crashes` | List crash reports (filterable) |
| `GET` | `/api/v1/crashes/{id}` | Get full crash report details |
| `GET` | `/api/v1/crashes/{id}/download` | Download crash report (JSON or tar.gz with coredump) |
| `DELETE` | `/api/v1/crashes/{id}` | Delete a crash report |
| `GET` | `/api/v1/crashes/stats` | Crash statistics (counts, MTBF, breakdown by code) |
| `GET` | `/api/v1/supervisor/status` | Supervisor status and all managed sessions |
| `POST` | `/api/v1/supervisor/sessions/{id}/restart` | Restart a session process |
| `POST` | `/api/v1/supervisor/sessions/{id}/reset-restarts` | Reset restart counter |
| `GET` | `/api/v1/supervisor/sessions/{id}/resources` | Session resource usage (cgroup stats) |
| `WS` | `/ws/v1/supervisor` | WebSocket stream of supervisor events (crashes, restarts, health) |

---

## 7) Frontend

### Technology
- **Framework**: vanilla JS or lightweight framework (Preact, Svelte, or similar).
- **Styling**: Liquid Glass CSS theme (translucent panels, blur effects, depth).
- **Charts**: lightweight charting library for real-time graphs.
- **Responsive**: works on desktop and tablet.

### Pages
1. **Login** — glass-themed login form.
2. **Dashboard** — overview with key metrics and alerts.
3. **Servers** — server list + detail view.
4. **Sessions** — session list + detail view.
5. **Users** — user list + detail view.
6. **Policies** — policy viewer/editor.
7. **Gateways** — gateway management (if applicable).
8. **Metrics** — full metrics dashboard with graphs.
9. **Logs** — centralized log explorer with per-subsystem filtering and live streaming.
10. **Audit** — audit log viewer with integrity verification.
11. **Honeypot** — honeypot/tarpit dashboard, attacker table, IOC management.
12. **Settings** — management UI settings, user account.
13. **Plugins** — plugin management dashboard (install, enable/disable, resource monitoring, fault history).
14. **Crash Reports** — crash report viewer, timeline, statistics, export tools.
15. **Supervisor** — session supervisor status, process health, restart controls.

### UX Principles
- Real-time updates (no manual refresh needed).
- Confirmation dialogs for destructive actions.
- Toast notifications for action results.
- Keyboard shortcuts for common actions.
- Dark mode support (system preference or manual toggle).

---

## 8) Deployment

### Requirements
- **Node.js** 18+ (LTS).
- No additional database or services required.

### Installation
```bash
# From package
npm install -g liquid-manager
liquid-manager --config /etc/liquid-manager/manager.toml

# From source
git clone <repo>
cd liquid-manager
npm install
npm run build
node server.js --config /etc/liquid-manager/manager.toml
```

### Running
- **Standalone**: `liquid-manager` command or `node server.js`.
- **Systemd**: service file included.
- **Docker**: container image available.
- **Disabled by default**: no auto-start, must be explicitly enabled and started.

### First Run
1. On first run, generates a self-signed TLS certificate (if none provided).
2. Creates a default `admin` account with a randomly generated password.
3. Prints the initial password to stdout.
4. Admin must change password on first login.

---

## 9) Test Plan

### Functional
- Authentication (all modes).
- Dashboard data accuracy.
- Server management operations.
- Session management operations.
- Policy editing and application.
- Audit log completeness.
- WebSocket real-time updates.

### Security
- HTTPS enforcement.
- CSRF protection.
- Role-based access control.
- Session timeout.
- Rate limiting.
- Brute force protection.

### Performance
- Page load times (<1s for dashboard).
- WebSocket update latency (<1s from server event to UI update).
- Memory usage (target: <256MB for managing 50 servers).
- Handles 100 concurrent admin sessions.

### Compatibility
- Chrome, Firefox, Safari, Edge (latest 2 versions).
- Responsive layout: 1024px+ desktop, 768px+ tablet.
