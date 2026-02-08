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

| Method | Description |
|--------|-------------|
| **OS package manager** | `.deb`, `.rpm` via standard repositories (preferred) |
| **Flatpak** (client only) | Client distributed as Flatpak |
| **Self-update** | `liquidctl update check` / `liquidctl update apply` for standalone installs |
| **Manual** | Download + install from release artifacts |

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

## 10) Release Lifecycle

### 10.1 Release Cadence

| Channel | Cadence | Support |
|---------|---------|---------|
| Stable | Every 3–4 months | Until next stable release + 1 month overlap |
| LTS | Annually | 2 years of security fixes |
| Patch | As needed | Backported to current stable + current LTS |

### 10.2 End-of-Life

When a version reaches end-of-life:
- No further patches are released.
- The update server returns a warning: "Your version is no longer supported."
- The server continues to function (no kill switch).
- The management UI and `liquidctl` show an EOL warning banner.

---

## 11) Test Plan

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
