# LiquiDE — Updates, Versioning & Migrations Specification

> **Status**: Draft
> **Depends on**: [spec.md](spec.md) (core server), [spec-system.md](spec-system.md) (system integration)

---

## 1) Overview

This document specifies LiquiDE's versioning scheme, compatibility guarantees, update mechanisms, cryptographic signing, rollback procedures, and data migration strategies.

---

## 2) Versioning Scheme

### 2.1 Semantic Versioning

LiquiDE follows [Semantic Versioning 2.0.0](https://semver.org/):

```
MAJOR.MINOR.PATCH[-prerelease][+build]
```

| Component | Meaning | Example |
|-----------|---------|---------|
| `MAJOR` | Breaking changes to protocol, config format, or plugin ABI | `2.0.0` |
| `MINOR` | New features, backward-compatible additions | `1.3.0` |
| `PATCH` | Bug fixes, security patches, no feature changes | `1.3.2` |
| Pre-release | Development stage | `2.0.0-alpha.1`, `2.0.0-beta.3`, `2.0.0-rc.1` |
| Build | Build metadata (informational) | `1.3.2+20250215.git.a1b2c3d` |

### 2.2 Version Components

Each LiquiDE component has its own version, but all components in a release share the same version number:

| Component | Binary | Example Version |
|-----------|--------|----------------|
| Supervisor daemon | `liquid-desktopd` | `1.3.2` |
| Session process | `liquid-session` | `1.3.2` |
| CLI tool | `liquidctl` | `1.3.2` |
| Client | `liquidclient` | `1.3.2` |
| Portal backend | `xdg-desktop-portal-liquide` | `1.3.2` |
| Management UI | `liquid-manager` | `1.3.2` |
| Plugin SDK | `liquide-plugin-sdk` | `1.3.2` |

### 2.3 Protocol Version

The wire protocol has an independent version number negotiated during the TLS handshake:

| Protocol Version | LiquiDE Versions | Description |
|-----------------|-------------------|-------------|
| `proto/1` | `1.0.0` – `1.x.y` | Initial protocol |
| `proto/2` | `2.0.0` – `2.x.y` | (future) Breaking protocol changes |

Minor protocol extensions (new message types, optional fields) are backward-compatible within a major protocol version.

### 2.4 Plugin ABI Version

Plugin ABI versions (see spec.md §14b) follow an independent scheme: `v1`, `v2`, etc. ABI versions are supported concurrently — `liquid-session` can load plugins targeting `v1` or `v2`.

---

## 3) Compatibility Guarantees

### 3.1 Compatibility Matrix

| Component A | Component B | Compatibility Rule |
|-------------|-------------|-------------------|
| Server `1.x` | Client `1.y` | Compatible (any `x` with any `y` within `1.*`) |
| Server `1.x` | Client `2.y` | Client negotiates down to `proto/1`; warn on missing features |
| Server `2.x` | Client `1.y` | Server offers `proto/1` fallback; limited feature set |
| Server `1.x` | `liquidctl` `1.y` | Compatible |
| Server `1.x` | `liquidctl` `2.y` | Management API may require fallback; warn if features unavailable |
| Server `1.x` | Plugin ABI `v1` | Fully supported |
| Server `2.x` | Plugin ABI `v1` | Supported with deprecation warning (see §3.2) |

### 3.2 Deprecation Policy

| Item | Deprecation Window | Process |
|------|-------------------|---------|
| Protocol version | 1 major release | `proto/N-1` supported for 1 major release after `proto/N` ships |
| Plugin ABI version | 2 major releases | `v(N-1)` supported for 2 major releases after `v(N)` ships |
| Config keys | 1 major release | Deprecated keys produce warnings, still functional |
| CLI commands | 1 major release | Deprecated commands warn, still function |
| D-Bus interfaces | 1 major release | Old interfaces kept as aliases |

Deprecation warnings are logged at `warn` level and surfaced in the management UI.

### 3.3 Configuration Forward Compatibility

When a newer LiquiDE version reads an older config file:
- Unknown sections are ignored (with a debug log).
- Missing keys use defaults.
- No config file modification occurs automatically.

When an older LiquiDE version reads a newer config file:
- Unknown sections and keys are ignored.
- No errors for unknown keys.

### 3.4 Database Schema Compatibility

Internal databases (`sessions.db`, `permissions.db`, etc.) use a schema version table:

```sql
CREATE TABLE schema_version (
    component TEXT PRIMARY KEY,
    version   INTEGER NOT NULL,
    migrated  TEXT NOT NULL  -- ISO 8601 timestamp
);
```

On startup, `liquid-desktopd` checks schema versions and runs migrations if needed (see §7).

---

## 4) Update Mechanism

### 4.1 Update Channels

| Channel | Description | Audience |
|---------|-------------|----------|
| `stable` | Production releases, fully tested | Default |
| `lts` | Long-term support (security fixes only, 2 years) | Enterprise |
| `beta` | Pre-release testing | Opt-in testers |
| `nightly` | Automated builds from `main` branch | Developers |

### 4.2 Update Sources

LiquiDE supports multiple update delivery methods:

| Method | Platforms | Description |
|--------|-----------|-------------|
| **apt / dnf / pacman** | Linux | `.deb`, `.rpm`, Arch packages via official or third-party repos (preferred for server) |
| **Homebrew** | macOS, Linux | `brew install liquide` / `brew install --cask liquidclient` |
| **Snap** | Ubuntu/Linux | `snap install liquidclient` (client) or `snap install liquide-server` (server) |
| **Nix** | NixOS/any | `nix profile install nixpkgs#liquide` or NixOS module via `services.liquide.enable` |
| **Flatpak** (client only) | Linux | Client distributed as Flatpak from Flathub |
| **AppImage** (client only) | Linux | Portable single-file client, no installation required |
| **DMG / pkg installer** | macOS | Disk image with drag-to-Applications or `.pkg` installer |
| **Docker / OCI** | Any | `docker pull ghcr.io/liquide/liquide-server` (server only) |
| **Self-update** | Any | `liquidctl update check` / `liquidctl update apply` for standalone installs |
| **Manual** | Any | Download from release artifacts page |

### 4.3 Update Check

```bash
# Check for available updates
liquidctl update check

# Example output:
Current version: 1.3.2
Latest stable:   1.4.0
Latest LTS:      1.2.8

Available update: 1.4.0
  - New: Plugin hot-reload improvements
  - Fixed: Session crash on XWayland resize (#1234)
  - Security: TLS certificate validation fix (CVE-2025-XXXX)
```

### 4.4 Update Application

For standalone installations (not managed by OS package manager):

```bash
# Download and apply update (requires restart)
liquidctl update apply --version 1.4.0

# Dry-run (download + verify, don't install)
liquidctl update apply --version 1.4.0 --dry-run

# Auto-update configuration
liquidctl config set updates.auto_check true
liquidctl config set updates.auto_download true
liquidctl config set updates.auto_apply false     # require manual apply
liquidctl config set updates.channel stable
```

### 4.5 Zero-Downtime Updates (Server)

For server updates while sessions are active:

1. `liquidctl update apply --graceful` initiates a rolling update:
   - Downloads and verifies the new version.
   - Signals `liquid-desktopd` to enter "drain mode" (no new sessions accepted).
   - Waits for active sessions from current sessions to disconnect or for admin to force-disconnect. Admin can set a timer: `--timeout 3600`.
   - Replaces binaries.
   - Restarts `liquid-desktopd` via `systemctl restart`.
   - New sessions use the new version.
2. Alternatively, `liquidctl update apply --force-restart` restarts immediately. Active sessions are terminated with a crash screen showing "Server is restarting for an update."

### 4.6 Upgrade Safety Guarantees

Server upgrades MUST NOT corrupt session state, policies, or configuration. The following invariants are enforced:

#### State Preservation

| State | Location | Upgrade Behavior |
|-------|----------|-----------------|
| **Server configuration** (`server.toml`) | `/etc/liquide/server.toml` | Never modified by the updater. Config is read-only to the update process. If a new version introduces new config keys, they use defaults. Deprecated keys are silently ignored (with `warn`-level log). |
| **Policy files** (`policies.toml`, per-user overrides) | `/etc/liquide/policies/` | Never modified by the updater. Policy format is forward-compatible (unknown keys ignored). |
| **TLS certificates** | `/etc/liquide/certs/` | Never modified. Certificate lifecycle is independent of software version. |
| **Session tokens** (in-memory) | `liquid-desktopd` process memory | Lost on daemon restart. This is by design — all sessions must re-authenticate after upgrade. Clients reconnect automatically via session resume (if within token lifetime). |
| **Active sessions** (graceful mode) | Running `liquid-session` processes | In graceful mode, active sessions continue running the **old binary** until they disconnect. The supervisor drains sessions from the old version. New sessions use the new binary. |
| **Crash reports** | `/var/log/liquide/crashes/` | Preserved. Not part of the update payload. |
| **Audit logs** | `/var/log/liquide/audit.log` | Preserved. The updater itself emits an audit event: `admin.action { action: "update_apply", before_version: "1.3.0", after_version: "1.4.0" }`. |
| **Plugin state** | `/var/lib/liquide/plugins/` | Plugin WASM binaries are NOT updated by the server updater. Plugins have their own update lifecycle. Plugin ABI compatibility is checked at load time (see spec.md §14b). |

#### Rollback

If the new version fails to start (e.g., binary crash, config incompatibility):

1. systemd `Restart=on-failure` triggers up to 3 restart attempts (with 5s `RestartSec`).
2. If all restarts fail, systemd stops the unit and marks it as failed.
3. The administrator can rollback by:
   ```bash
   liquidctl update rollback         # restores previous binary from /var/lib/liquide/rollback/
   systemctl start liquid-desktopd   # start with previous version
   ```
4. The rollback binary is preserved for exactly one version (the version that was replaced). Updating from 1.3 → 1.4 → 1.5 means 1.3 is no longer available for rollback; only 1.4 is.

#### Version Compatibility Matrix

| Component Pair | Compatibility Rule |
|---------------|-------------------|
| Supervisor (new) ↔ Session (old) | Supervisor MUST support sessions from the previous minor version. Supervisor uses internal ABI version check on session spawn. |
| Client (new) ↔ Server (old) | Protocol version negotiation handles this (spec-protocol-formal.md §15). Client falls back to server's protocol version. |
| Client (old) ↔ Server (new) | Same — server falls back to client's protocol version. New server features are unavailable. |
| Plugins (old ABI) ↔ Host (new) | Plugin ABI version checked at load time. Plugins with unsupported ABI are disabled with a clear error, not crashed. |
| Config (old format) ↔ Binary (new) | New config keys use defaults. Removed keys are silently ignored. No config migration step needed for minor version updates. |

#### Pre-Upgrade Validation

`liquidctl update apply --dry-run` performs:

1. Downloads and verifies the update package (signature, checksum).
2. Checks binary compatibility (architecture, glibc version).
3. Validates current config against new version's schema (warns about deprecated keys).
4. Checks plugin ABI compatibility with new host version.
5. Estimates downtime (graceful: 0 for new sessions, existing sessions continue; force: ~5–10s).
6. Reports pass/fail. Does not modify any files.

---

## 5) Signed Updates & Integrity

### 5.1 Signing Keys

| Key | Algorithm | Purpose |
|-----|-----------|---------|
| Release signing key | Ed25519 | Signs release manifests |
| Binary signing key | Ed25519 | Signs individual binaries |
| Plugin signing key | Ed25519 | Signs WASM plugin bundles (see spec.md §14b) |

The release public key is embedded in the `liquid-desktopd` and `liquidctl` binaries at compile time. Additional trusted keys can be configured in `server.toml`:

```toml
[updates]
trusted_keys = [
    "ed25519:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",  # built-in
    "ed25519:BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB=",  # org key
]
```

### 5.2 Release Manifest

Each release includes a signed manifest:

```json
{
  "version": "1.4.0",
  "channel": "stable",
  "timestamp": "2025-02-15T12:00:00Z",
  "artifacts": [
    {
      "name": "liquid-desktopd",
      "platform": "linux-amd64",
      "sha256": "a1b2c3d4...",
      "size": 15234567
    },
    {
      "name": "liquid-session",
      "platform": "linux-amd64",
      "sha256": "e5f6g7h8...",
      "size": 12345678
    }
  ],
  "migrations": ["1.3.0-to-1.4.0"],
  "min_upgrade_from": "1.2.0",
  "changelog_url": "https://docs.liquide.dev/changelog/1.4.0"
}
```

Manifest signature: detached Ed25519 signature in `manifest.sig`.

### 5.3 Verification Process

1. Download `manifest.json` and `manifest.sig`.
2. Verify signature against trusted keys.
3. For each artifact:
   - Download the artifact.
   - Verify SHA-256 matches manifest.
   - Optionally verify individual binary signature.
4. Only proceed with installation if all verifications pass.

### 5.4 Transparency Log (Optional)

For organizations requiring auditability, LiquiDE can publish releases to a [Sigstore](https://sigstore.dev/)-compatible transparency log. Configuration:

```toml
[updates]
transparency_log_enabled = false
transparency_log_url = ""
```

---

## 6) Rollback

### 6.1 Rollback Strategy

LiquiDE supports rollback to the previous version in case an update causes issues:

| Method | Description |
|--------|-------------|
| **OS package manager** | `apt install liquide=1.3.2-1`, `dnf downgrade liquide` |
| **Standalone** | `liquidctl update rollback` (keeps one previous version) |
| **Snapshot** | For VM/container deployments: filesystem snapshot before update |

### 6.2 Standalone Rollback

When `liquidctl update apply` is used, the previous binaries are preserved in `/var/lib/liquide/rollback/`:

```
/var/lib/liquide/rollback/
├── version.txt        (previous version: "1.3.2")
├── liquid-desktopd
├── liquid-session
├── xdg-desktop-portal-liquide
└── liquidctl
```

Rolling back:

```bash
# Rollback to previous version
liquidctl update rollback

# Verify rollback version
liquidctl update rollback --dry-run

# Delete rollback data (after confirming new version is stable)
liquidctl update rollback --purge
```

### 6.3 Database Rollback

Database migrations (§7) include both `up` and `down` migration scripts. On rollback:
1. `liquid-desktopd` detects that the binary version is older than the schema version.
2. It runs `down` migrations to revert the schema.
3. If a `down` migration fails, the daemon refuses to start and logs an error with instructions for manual intervention.

### 6.4 Config Rollback

A backup of the config files is created before any migration modifies them:
- `/etc/liquide/server.toml.bak.<timestamp>`
- Backups older than 30 days are automatically cleaned up.

---

## 7) Database Migrations

### 7.1 Migration Framework

Migrations are embedded in the `liquid-desktopd` binary. Each migration is a pair of SQL scripts:

```
migrations/
├── 001_initial_schema.up.sql
├── 001_initial_schema.down.sql
├── 002_add_plugin_tables.up.sql
├── 002_add_plugin_tables.down.sql
├── 003_add_crash_reports.up.sql
├── 003_add_crash_reports.down.sql
└── ...
```

### 7.2 Migration Execution

1. On startup, `liquid-desktopd` reads `schema_version` from each database.
2. If the current code version requires a higher schema version, pending `up` migrations are run in order within a transaction.
3. If any migration fails, the transaction is rolled back and the daemon refuses to start.
4. Successful migrations update the `schema_version` table.

### 7.3 Migration Safety

- All migrations run within a transaction (rollback on failure).
- Migrations are idempotent (safe to re-run if interrupted).
- Large data migrations use batched operations to avoid locking.
- A pre-flight check (`liquidctl update migrate --dry-run`) validates migrations without applying them.

### 7.4 Config Migration

When config file format changes between major versions:

1. `liquid-desktopd` detects the config version (via `config_version` key, default: `1`).
2. A config migration transforms the old format to the new format.
3. The migrated config is written to the original path; the old config is backed up.
4. Config migrations are logged at `info` level.

Example config migration:
```toml
# Old (v1):
[video]
codec = "h264"

# New (v2): codec moved under [encoding]
[encoding]
codec = "h264"
```

---

## 8) Client Updates

### 8.1 Client Update Flow

The LiquidClient application checks for updates independently:

1. On startup (if `check_updates = true` in client config).
2. Client queries the update server (HTTPS GET to `updates.liquide.dev/client/<platform>/<channel>/latest`).
3. If a newer version is available, a non-intrusive notification appears: "LiquidClient X.Y.Z is available. [Update Now] [Later] [Skip This Version]".
4. "Update Now" downloads the installer and launches it (platform-specific).
5. "Skip This Version" suppresses the notification for that specific version.

### 8.2 Client-Server Version Mismatch

When a client connects to a server with a different version:

| Scenario | Behavior |
|----------|----------|
| Client newer than server | Client negotiates down. Features unavailable on server are greyed out in the client UI. |
| Server newer than client | Server offers backward-compatible protocol. A banner suggests: "A newer client version is available for the best experience." |
| Major version mismatch | Connection allowed if protocol versions overlap (see §3.1). If no overlap: connection refused with error `VERSION_INCOMPATIBLE`. |

---

## 9) Flatpak Application Updates

Flatpak applications installed via the Software Center (or `liquidctl flatpak install`) are updated through a dedicated pipeline that integrates with the LiquiDE update infrastructure.

### 9.1 Update Check

Flatpak update checks are triggered by:

1. **systemd timer** — `liquide-flatpak-update.timer` (see spec-system.md §14.3) for system-wide installs.
2. **Session login** — `liquid-session` checks for per-user Flatpak updates on session start.
3. **Manual** — `liquidctl flatpak update --check` or the Software Center "Updates" tab.
4. **Periodic** — controlled by `flatpak.auto_update_schedule` policy (default: `daily`).

```bash
# Check for available Flatpak updates
liquidctl flatpak update --check

# Example output:
Available Flatpak updates:
  org.mozilla.firefox          124.0 → 124.0.1   (12 MB)
  org.gimp.GIMP                2.10.36 → 2.10.38 (45 MB)
  org.freedesktop.Platform     23.08.15 → 23.08.16 (runtime, 200 MB)
Total download: 257 MB
```

### 9.2 Update Application

```bash
# Update all Flatpak apps
liquidctl flatpak update

# Update a specific app
liquidctl flatpak update org.mozilla.firefox

# Update system-wide installs (requires polkit)
liquidctl flatpak update --system

# Non-interactive (for systemd service / scripting)
liquidctl flatpak update --system --noninteractive
```

**Update behavior:**
- Updates are downloaded as OSTree deltas (bandwidth-efficient).
- Running apps are **not** interrupted — the update is deployed alongside the current version. The new version takes effect on next launch.
- The Software Center shows a toast: "Firefox was updated to 124.0.1" with a "Restart app" action if the app is currently running.

### 9.3 Auto-Update

When `flatpak.auto_update = true`:

1. LiquiDE downloads and applies Flatpak updates in the background.
2. A notification is shown: "N applications were updated" with an action to view details in the Software Center.
3. No app restarts are forced — users launch the updated version next time.

**Bandwidth awareness:** Auto-updates respect the session's network state. If the connection is metered (detected via NetworkManager `metered` property or policy `flatpak.auto_update_on_metered = false`), auto-updates are deferred until an unmetered connection is available.

### 9.4 Runtime Updates

Flatpak runtimes (`org.freedesktop.Platform`, `org.kde.Platform`, etc.) are updated alongside application updates. If a runtime update would break a pinned version (see `flatpak.pinned_runtimes` policy), the pinned runtime is preserved and the update is skipped with a warning.

### 9.5 Rollback

Flatpak supports per-app rollback to the previous version:

```bash
# Rollback Firefox to the previous commit
liquidctl flatpak rollback org.mozilla.firefox

# List available commits for an app
liquidctl flatpak history org.mozilla.firefox
```

The Software Center also provides a "Revert to previous version" option in the app detail page's version history.

---

## 10) Homebrew Updates

### 10.1 Package Names

| Package | Type | Platform | Description |
|---------|------|----------|-------------|
| `liquide` | Formula | macOS, Linux | Server daemon, session process, CLI tools |
| `liquidclient` | Cask | macOS | GUI client application (`.app` bundle) |

For pre-release channels (beta, nightly), use the LiquiDE tap:

```bash
brew tap liquide/tap
brew install liquide/tap/liquide --HEAD   # nightly
```

### 10.2 Update Check

```bash
# Check for available Homebrew updates
liquidctl brew update --check

# Example output:
Available Homebrew updates:
  liquide          1.3.2 → 1.4.0   (formula)
  liquidclient     1.3.2 → 1.4.0   (cask)
```

### 10.3 Update Application

```bash
# Update via Homebrew directly
brew update && brew upgrade liquide
brew upgrade --cask liquidclient

# Update via liquidctl wrapper
liquidctl brew update
liquidctl brew update liquide              # specific package
```

### 10.4 Auto-Update

Homebrew auto-update integrates with `brew autoupdate` (macOS) or a systemd timer (Linux):

```bash
# Enable auto-update via brew autoupdate (macOS, requires homebrew-autoupdate)
brew autoupdate start --upgrade

# Linux: systemd timer installed by dev-setup.sh
# Timer runs brew update && brew upgrade daily
```

### 10.5 Version Pinning

```bash
# Pin to prevent automatic upgrades
brew pin liquide

# Unpin to resume upgrades
brew unpin liquide
```

### 10.6 Rollback

```bash
# Rollback to previous version
liquidctl brew rollback liquide

# Install a specific version (requires tap with versioned formulae)
brew install liquide/tap/liquide@1.3.2
```

---

## 11) Snap Updates

### 11.1 Snap Names

| Snap | Confinement | Description |
|------|-------------|-------------|
| `liquidclient` | strict | Client application (GUI) |
| `liquide-server` | classic | Server daemon (needs system access) |

### 11.2 Channel Mapping

| LiquiDE Channel | Snap Channel | Description |
|-----------------|-------------|-------------|
| `stable` | `stable` | Production releases |
| `beta` | `beta` | Pre-release testing |
| `nightly` | `edge` | Automated daily builds |

### 11.3 Update Check

```bash
# Check for available Snap updates
liquidctl snap update --check

# Example output:
Available Snap updates:
  liquidclient     1.3.2 → 1.4.0   (stable channel)
  liquide-server   1.3.2 → 1.4.0   (stable channel)
```

### 11.4 Update Application

```bash
# Update via snap directly
snap refresh liquidclient
snap refresh liquidclient --channel=beta   # switch channel

# Update via liquidctl wrapper
liquidctl snap update
liquidctl snap update liquidclient         # specific snap
```

### 11.5 Automatic Refresh

Snap updates are managed by `snapd` and occur automatically. To defer:

```bash
# Hold refresh for 72 hours
snap refresh --hold=72h liquidclient

# Remove hold
snap refresh --unhold liquidclient

# Set maintenance window
snap set system refresh.timer=sat,04:00-06:00
```

### 11.6 Rollback

```bash
# Revert to previous revision
snap revert liquidclient

# Revert via liquidctl
liquidctl snap revert liquidclient
```

### 11.7 Interfaces

Client snap connections:

| Interface | Purpose | Auto-connected |
|-----------|---------|---------------|
| `network` | Network access | Yes |
| `audio-playback` | Audio output | Yes |
| `audio-record` | Microphone input | No |
| `desktop` | Desktop integration | Yes |
| `wayland` | Wayland display | Yes |
| `x11` | X11 fallback | Yes |
| `opengl` | GPU rendering | Yes |

Server snap connections (classic confinement):

| Interface | Purpose |
|-----------|---------|
| `network-bind` | Listen for connections |
| `system-observe` | Process monitoring |
| `process-control` | Session management |

---

## 12) Nix Updates

### 12.1 Package Names

| Package | Description |
|---------|-------------|
| `nixpkgs#liquide` | Server + CLI + client (full package) |
| `nixpkgs#liquidclient` | Client only |

### 12.2 NixOS Module

Declarative NixOS configuration:

```nix
# /etc/nixos/configuration.nix
{
  services.liquide = {
    enable = true;
    settings = {
      general.hostname = "liquid-server-01";
      tls.cert = "/etc/liquide/cert.pem";
      tls.key = "/etc/liquide/key.pem";
      performance.active_fps = 60;
      # ... all server.toml keys available as typed Nix options
    };
  };
}
```

Apply changes:

```bash
nixos-rebuild switch
```

### 12.3 Imperative Install

```bash
# Install
nix profile install nixpkgs#liquide

# Update
nix profile upgrade nixpkgs#liquide

# Check via liquidctl
liquidctl nix update --check
liquidctl nix update
```

### 12.4 Flake Input

```nix
# flake.nix
{
  inputs.liquide.url = "github:liquide/liquide";

  outputs = { self, nixpkgs, liquide }: {
    nixosConfigurations.myhost = nixpkgs.lib.nixosSystem {
      modules = [
        liquide.nixosModules.default
        { services.liquide.enable = true; }
      ];
    };
  };
}
```

### 12.5 Rollback

```bash
# Imperative rollback
nix profile rollback

# NixOS rollback
nixos-rebuild switch --rollback

# Via liquidctl
liquidctl nix rollback
```

### 12.6 Binary Cache

Pre-built binaries are served from `cache.liquide.dev`:

```nix
# configuration.nix
nix.settings.substituters = [ "https://cache.liquide.dev" ];
nix.settings.trusted-public-keys = [ "cache.liquide.dev-1:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=" ];
```

### 12.7 Development Shell

Contributors can enter a development environment with all dependencies:

```bash
nix develop github:liquide/liquide
```

---

## 13) AppImage Updates

### 13.1 Distribution

| File | Platform | Description |
|------|----------|-------------|
| `LiquidClient-x86_64.AppImage` | Linux x86_64 | Portable client, no installation required |
| `LiquidClient-aarch64.AppImage` | Linux ARM64 | Portable client for ARM |

### 13.2 First Run

```bash
chmod +x LiquidClient-x86_64.AppImage
./LiquidClient-x86_64.AppImage
```

### 13.3 Desktop Integration

```bash
# Via appimaged (automatic, watches ~/Applications/)
mkdir -p ~/Applications
mv LiquidClient-x86_64.AppImage ~/Applications/

# Via liquidctl
liquidctl appimage integrate LiquidClient-x86_64.AppImage
```

### 13.4 Update

AppImage updates use the AppImageUpdate delta mechanism for bandwidth-efficient downloads:

```bash
# Check for updates
liquidctl appimage update --check

# Apply update (downloads delta, replaces AppImage in-place)
liquidctl appimage update

# Auto-update on launch (configurable in client.toml)
# [updates]
# appimage_auto_check = true
```

### 13.5 Signature Verification

Each AppImage contains an embedded Ed25519 signature:

```bash
# Verify before execution
liquidctl appimage verify LiquidClient-x86_64.AppImage

# Output:
Signature: valid (signed by LiquiDE release key)
Version:   1.4.0
SHA-256:   a1b2c3d4...
```

### 13.6 Rollback

AppImage does not support automatic rollback. Users should keep the previous AppImage file for manual rollback.

---

## 14) DMG / pkg Installer (macOS)

### 14.1 Artifacts

| File | Architecture | Description |
|------|-------------|-------------|
| `LiquidClient-arm64.dmg` | Apple Silicon | DMG disk image for ARM64 Macs |
| `LiquidClient-x86_64.dmg` | Intel | DMG disk image for Intel Macs |
| `LiquidClient-universal.dmg` | Universal | Fat binary (arm64 + x86_64) |
| `LiquidClient.pkg` | Universal | Installer package for scripted/MDM deployment |

### 14.2 DMG Installation

1. Open the `.dmg` file.
2. Drag `LiquidClient.app` to the Applications folder (symlink provided in DMG).
3. Eject the DMG.

### 14.3 pkg Installation

```bash
# Interactive
open LiquidClient.pkg

# Scripted / MDM
sudo installer -pkg LiquidClient.pkg -target /
```

### 14.4 Code Signing & Notarization

- All binaries are signed with an Apple Developer ID certificate.
- The app and pkg are notarized via `notarytool` and stapled.
- Gatekeeper passes verification without user override.

### 14.5 MDM Deployment

The `.pkg` installer supports managed deployment via:
- Apple Business Manager / MDM push.
- `installer -pkg` command for scripted installs.
- Configuration profiles (`.mobileconfig`) for pre-configuring server URL.

### 14.6 In-App Updates (Sparkle)

The macOS client optionally integrates the Sparkle framework for in-app update checks:

| Property | Value |
|----------|-------|
| `SUFeedURL` | `https://updates.liquide.dev/mac/appcast.xml` |
| `SUPublicEDKey` | Ed25519 public key for Sparkle signature verification |
| `SUEnableAutomaticChecks` | `true` (configurable in preferences) |

### 14.7 Uninstall

```bash
# Drag app to Trash (removes app bundle only)

# Full cleanup (remove preferences, caches, login items)
liquidctl uninstall --purge
# Removes: ~/Library/Preferences/dev.liquide.client.plist
#          ~/Library/Caches/dev.liquide.client/
#          ~/Library/Application Support/LiquidClient/
```

---

## 15) Release Lifecycle

### 15.1 Release Cadence

| Channel | Cadence | Support |
|---------|---------|---------|
| Stable | Every 3–4 months | Until next stable release + 1 month overlap |
| LTS | Annually | 2 years of security fixes |
| Patch | As needed | Backported to current stable + current LTS |

### 15.2 End-of-Life

When a version reaches end-of-life:
- No further patches are released.
- The update server returns a warning: "Your version is no longer supported."
- The server continues to function (no kill switch).
- The management UI and `liquidctl` show an EOL warning banner.

---

## 16) Test Plan

### Functional
- Version negotiation between all component combinations (see §3.1 matrix).
- Update check returns correct version information.
- Update apply downloads, verifies, and installs correctly.
- Rollback restores previous version and reverts database schema.
- Config migration transforms old format correctly and creates backups.
- Database migrations run in order, are transactional, and are idempotent.
- Flatpak update check lists available app and runtime updates with correct versions and sizes.
- Flatpak update apply downloads and installs updates without interrupting running apps.
- Flatpak auto-update fires on schedule and respects metered network policy.
- Flatpak rollback reverts an app to the previous OSTree commit.
- Flatpak runtime pinning prevents unwanted runtime updates.

### Security
- Signature verification rejects tampered manifests.
- Signature verification rejects tampered binaries.
- Unknown signing keys are rejected.
- Update downloads use TLS; MITM is detected.

### Edge Cases
- Update interrupted mid-download (resume or clean restart).
- Rollback when no previous version exists (clear error message).
- Database migration failure (daemon refuses to start, logs instructions).
- Config migration from version 1 directly to version 3 (chained migrations).
- Client newer than server by 2 major versions (graceful degradation).

### Package Manager Updates
- Homebrew formula install, upgrade, tap switch, pin/unpin, and rollback.
- Snap install, refresh across channels, revert, interface connections.
- Nix imperative install/upgrade/rollback and NixOS module enable/rebuild.
- AppImage download, delta update, desktop integration, signature verification.
- DMG drag-install, pkg scripted install, notarization verification, Sparkle update check.
