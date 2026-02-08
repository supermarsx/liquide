# LiquiDE — System Integration & Session Lifecycle Specification

> **Status**: Draft
> **Depends on**: [spec.md](spec.md) (core server), [spec-interop.md](spec-interop.md) (desktop standards)

---

## 1) Overview

This document specifies how LiquiDE integrates with the host Linux system: systemd service units, socket activation, PAM authentication flows, XDG autostart, filesystem layout, environment variables, and security hardening profiles.

---

## 2) Process Architecture & Boot Order

### 2.1 Service Dependency Graph

```
systemd (PID 1)
└── liquid-desktopd.service (supervisor daemon)
    ├── Depends: dbus.service, graphical.target
    ├── After: network-online.target (if remote auth is configured)
    ├── Wants: pipewire.service, pipewire-pulse.service
    │
    ├── liquid-session@alice.service (user session, spawned on login)
    │   ├── XWayland (optional child process)
    │   ├── User applications
    │   └── xdg-desktop-portal-liquide
    │
    └── liquid-session@bob.service (another user session)
        └── ...
```

### 2.2 Boot Sequence

1. `systemd` starts `liquid-desktopd.service` (system service).
2. `liquid-desktopd` opens its management socket and begins listening for connections.
3. On client connection, the client authenticates via the configured auth backend.
4. On successful authentication, `liquid-desktopd` spawns a `liquid-session` child process for the user (or resumes an existing one).
5. `liquid-session` starts the compositor, shell, D-Bus services, PipeWire connection, and XWayland (if enabled).
6. XDG autostart entries are processed.
7. Session is ready; the client receives the first frame.

---

## 3) systemd Units

### 3.1 System Service — `liquid-desktopd.service`

```ini
[Unit]
Description=LiquiDE Desktop Environment Supervisor
Documentation=man:liquid-desktopd(8)
After=network-online.target dbus.service graphical.target
Wants=network-online.target
Requires=dbus.service

[Service]
Type=notify
ExecStart=/usr/bin/liquid-desktopd --config /etc/liquide/server.toml
ExecReload=/bin/kill -HUP $MAINPID
Restart=on-failure
RestartSec=5
WatchdogSec=30

# ─── Hardening (see §6) ────────────────────
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=true
NoNewPrivileges=false
CapabilityBoundingSet=CAP_SYS_ADMIN CAP_SETUID CAP_SETGID CAP_NET_BIND_SERVICE CAP_SYS_RESOURCE CAP_DAC_READ_SEARCH CAP_KILL CAP_SYS_PTRACE
AmbientCapabilities=CAP_SYS_ADMIN CAP_SETUID CAP_SETGID CAP_NET_BIND_SERVICE CAP_SYS_RESOURCE CAP_DAC_READ_SEARCH CAP_KILL
ReadWritePaths=/run/liquide /var/log/liquide /var/lib/liquide /tmp
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=false
RestrictRealtime=true
RestrictSUIDSGID=true
SystemCallFilter=@system-service @privileged
SystemCallArchitectures=native
LockPersonality=true
MemoryDenyWriteExecute=false
RestrictNamespaces=false
DevicePolicy=closed
DeviceAllow=/dev/dri rw
DeviceAllow=/dev/nvidia* rw
DeviceAllow=/dev/video* rw

# ─── Resource Limits ────────────────────────
LimitNOFILE=65536
LimitNPROC=4096
TasksMax=8192

# ─── Logging ────────────────────────────────
StandardOutput=journal
StandardError=journal
SyslogIdentifier=liquid-desktopd

[Install]
WantedBy=graphical.target
```

### 3.2 Socket Activation — `liquid-desktopd.socket`

```ini
[Unit]
Description=LiquiDE Supervisor Control Socket

[Socket]
ListenStream=/run/liquide/ctl.sock
SocketMode=0660
SocketGroup=liquide
Accept=false

[Install]
WantedBy=sockets.target
```

The management socket (`ctl.sock`) is used by `liquidctl` and the management UI. It is separate from the client connection ports (which `liquid-desktopd` opens directly on TCP/UDP/QUIC).

### 3.3 User Session — `liquid-session@.service`

This is a **template** systemd unit. It is instantiated by `liquid-desktopd` via `systemd-run` or direct `fork`/`exec` with cgroup placement.

```ini
[Unit]
Description=LiquiDE User Session for %i
After=dbus.service

[Service]
Type=notify
ExecStart=/usr/lib/liquide/liquid-session --user %i
User=%i
Group=%i
PAMName=liquide-session
Slice=user-%i.slice

# ─── Session Environment ────────────────────
Environment=XDG_SESSION_TYPE=wayland
Environment=XDG_CURRENT_DESKTOP=LiquiDE
Environment=DESKTOP_SESSION=liquide
Environment=WAYLAND_DISPLAY=liquide-%i
Environment=XDG_RUNTIME_DIR=/run/user/%U

# ─── Hardening ──────────────────────────────
ProtectSystem=strict
ProtectHome=false
PrivateTmp=true
NoNewPrivileges=true
ReadWritePaths=/run/user/%U /home/%i /tmp
DevicePolicy=closed
DeviceAllow=/dev/dri rw
DeviceAllow=/dev/video* rw

# ─── Resource Limits (overridable via cgroup) ─
LimitNOFILE=32768
LimitNPROC=2048
TasksMax=4096

StandardOutput=journal
StandardError=journal
SyslogIdentifier=liquid-session-%i
```

### 3.4 Portal Service — `xdg-desktop-portal-liquide.service`

```ini
[Unit]
Description=LiquiDE XDG Desktop Portal
PartOf=graphical-session.target

[Service]
Type=dbus
BusName=org.freedesktop.impl.portal.desktop.liquide
ExecStart=/usr/lib/liquide/xdg-desktop-portal-liquide

[Install]
WantedBy=default.target
```

---

## 4) PAM Integration

### 4.1 PAM Service File — `/etc/pam.d/liquide-session`

```
# LiquiDE session authentication
auth        required    pam_env.so
auth        required    pam_unix.so
auth        optional    pam_sss.so
auth        optional    pam_ldap.so
auth        optional    pam_fprintd.so

account     required    pam_unix.so
account     optional    pam_sss.so
account     optional    pam_ldap.so
account     required    pam_nologin.so

password    required    pam_unix.so sha512 shadow
password    optional    pam_sss.so
password    optional    pam_ldap.so

session     required    pam_limits.so
session     required    pam_unix.so
session     optional    pam_sss.so
session     optional    pam_ldap.so
session     required    pam_loginuid.so
session     optional    pam_systemd.so
session     optional    pam_env.so readenv=1 envfile=/etc/default/locale
```

### 4.2 Authentication Flow (Backend)

1. Client sends credentials to `liquid-desktopd` over the encrypted transport.
2. `liquid-desktopd` invokes PAM conversation:
   - `pam_start("liquide-session", username, &conversation, &handle)`
   - `pam_authenticate(handle, 0)` — password, LDAP, SSSD, or fingerprint.
   - `pam_acct_mgmt(handle, 0)` — account status, expiration, restrictions.
3. On success:
   - `pam_setcred(handle, PAM_ESTABLISH_CRED)` — establish credentials (Kerberos tickets, etc.).
   - `pam_open_session(handle, 0)` — open PAM session (triggers pam_systemd for user slice).
4. `liquid-desktopd` then spawns the session process as the authenticated user.
5. On session end:
   - `pam_close_session(handle, 0)`
   - `pam_setcred(handle, PAM_DELETE_CRED)`
   - `pam_end(handle, status)`

### 4.3 Multi-Factor Authentication

When the PAM stack returns `PAM_NEW_AUTHTOK_REQD` or includes modules that require additional interaction (e.g., TOTP via pam_oath), the LiquiDE login screen renders additional input fields dynamically based on the PAM conversation messages. The PAM conversation function relays prompts to the client login screen via the control channel.

### 4.4 PAM Session Events

LiquiDE listens for PAM session events to trigger desktop behaviors:
- `pam_open_session` → start user session process, load user config.
- `pam_close_session` → terminate session, clean up runtime files.
- Password-about-to-expire warnings from `pam_unix` → surface as notification to user.

---

## 5) Autostart

### 5.1 XDG Autostart Specification

LiquiDE processes autostart entries per the [XDG Autostart Specification](https://specifications.freedesktop.org/autostart-spec/latest/):

**Search directories (in order):**
1. `$XDG_CONFIG_HOME/autostart/` (default: `~/.config/autostart/`)
2. Each directory in `$XDG_CONFIG_DIRS/autostart/` (default: `/etc/xdg/autostart/`)

**Processing rules:**
- Only entries with `Type=Application` are considered.
- `Hidden=true` → skip.
- `OnlyShowIn` / `NotShowIn` → apply `XDG_CURRENT_DESKTOP=LiquiDE` filter (with compat list, see spec-interop.md §4.6).
- `TryExec` → check if executable exists; skip if not.
- `AutostartCondition` → evaluate condition (e.g., `GSettings org.gnome.system.proxy mode` = `manual`). LiquiDE evaluates GSettings conditions via D-Bus if available; unknown conditions default to "run".
- User entries in `$XDG_CONFIG_HOME/autostart/` override system entries with the same filename (user can set `Hidden=true` to suppress a system autostart entry).

### 5.2 Autostart Phases

LiquiDE processes autostart in two phases:

| Phase | Timing | Description |
|-------|--------|-------------|
| **Early** | After compositor + D-Bus are ready, before first frame | Entries with `X-LiquiDE-Autostart-Phase=early` (e.g., accessibility tools, input methods) |
| **Normal** | After first frame is sent to client | All remaining autostart entries |

### 5.3 Autostart Delay

LiquiDE supports `X-GNOME-Autostart-Delay=N` (GNOME compat) and `X-LiquiDE-Autostart-Delay=N` for delaying autostart by N seconds after the phase begins.

### 5.4 Autostart Management

Users can manage autostart entries via:
- Settings app → Startup Applications (see spec-settings.md).
- Manually editing files in `~/.config/autostart/`.
- `liquidctl session autostart list|enable|disable` commands.

Policy: `session.autostart.enabled` (default: `true`) — master switch. `session.autostart.max_entries` (default: `50`) — prevent runaway autostart.

---

## 6) Security Hardening

### 6.1 Capabilities

`liquid-desktopd` requires the following Linux capabilities:

| Capability | Reason |
|------------|--------|
| `CAP_SYS_ADMIN` | cgroup management, namespace creation |
| `CAP_SETUID` / `CAP_SETGID` | Spawn session processes as different users |
| `CAP_NET_BIND_SERVICE` | Bind to ports < 1024 (optional, if configured) |
| `CAP_SYS_RESOURCE` | Set resource limits for sessions |
| `CAP_DAC_READ_SEARCH` | Read user home directories for config loading |
| `CAP_KILL` | Send signals to session processes |

`liquid-session` runs with **no** elevated capabilities (`NoNewPrivileges=true`). It runs as the target user with standard user permissions.

### 6.2 Seccomp Filtering

Both `liquid-desktopd` and `liquid-session` apply seccomp-bpf filters:

**liquid-desktopd** (supervisor): allows `@system-service` + `@privileged` syscall groups (systemd shorthand).

**liquid-session** (user session): applies a restrictive allowlist:

```
Allowed syscall groups:
  @basic-io       — read, write, open, close, stat, fstat, lseek, mmap, mprotect, munmap, brk
  @file-system    — access, openat, fstatat, readlink, getcwd, chdir
  @io-event       — epoll_*, poll, select, eventfd, timerfd, signalfd
  @ipc            — pipe, pipe2, socketpair, shmget, shmat, msgget
  @network-io     — socket, bind, listen, accept, connect, sendto, recvfrom, sendmsg, recvmsg
  @process        — clone, fork, vfork, execve, wait4, kill, getpid, getuid, getgid
  @signal         — rt_sigaction, rt_sigprocmask, sigaltstack
  @timer          — clock_gettime, nanosleep, timer_*

Denied (blocked with EPERM):
  @mount          — mount, umount, swapon
  @module         — init_module, finit_module, delete_module
  @raw-io         — ioperm, iopl, out*
  @reboot         — reboot, kexec_*
  @swap           — swapon, swapoff
  @obsolete       — uselib, _sysctl

Special:
  ioctl           — allowed with argument filtering (DRI/DRM ioctls only)
  prctl           — allowed (needed by Rust runtime)
  seccomp         — blocked (no nested seccomp)
```

### 6.3 Filesystem Protection

| Directive | Supervisor | Session |
|-----------|-----------|---------|
| `ProtectSystem` | `strict` | `strict` |
| `ProtectHome` | `read-only` | `false` (user needs home) |
| `PrivateTmp` | `true` | `true` |
| `ProtectKernelTunables` | `true` | `true` |
| `ProtectKernelModules` | `true` | `true` |
| `ProtectKernelLogs` | `true` | `true` |
| `ProtectControlGroups` | `false` (needs cgroup access) | `true` |
| `ProtectClock` | `true` | `true` |
| `ProtectHostname` | `true` | `true` |

### 6.4 SELinux Policy (Optional)

LiquiDE provides an optional SELinux policy module (`liquide-selinux`):

- `liquid-desktopd` runs in the `liquide_supervisor_t` domain.
- `liquid-session` runs in the `liquide_session_t` domain.
- Session processes are confined: no access to other users' files, no raw network sockets (beyond what the session requires), no kernel module loading.
- Transitions: `liquide_supervisor_t` → `liquide_session_t` on session process exec.

### 6.5 AppArmor Profile (Optional)

LiquiDE provides optional AppArmor profiles:

```
# /etc/apparmor.d/usr.bin.liquid-desktopd
/usr/bin/liquid-desktopd {
  #include <abstractions/base>
  #include <abstractions/nameservice>

  /etc/liquide/** r,
  /var/lib/liquide/** rw,
  /var/log/liquide/** rw,
  /run/liquide/** rw,
  /run/user/*/liquide/** rw,

  # GPU access
  /dev/dri/** rw,
  /dev/nvidia* rw,

  # Spawn session processes
  /usr/lib/liquide/liquid-session px -> liquide_session,

  capability setuid,
  capability setgid,
  capability sys_admin,
  capability kill,
  capability sys_resource,
  capability dac_read_search,
  capability net_bind_service,
}
```

### 6.6 Network Hardening

- `liquid-desktopd` binds only to configured listen addresses/ports.
- All client connections require TLS 1.3 (or DTLS 1.3 for UDP).
- Management socket is Unix domain only — not exposed over network.
- Rate limiting on connection attempts (see spec.md §19 honeypot/tarpit).

---

## 7) Filesystem Layout

### 7.1 Consolidated Path Contract

#### System Paths (root-owned)

| Path | Purpose | Permissions |
|------|---------|-------------|
| `/usr/bin/liquid-desktopd` | Supervisor daemon binary | `0755 root:root` |
| `/usr/bin/liquidctl` | CLI management tool | `0755 root:root` |
| `/usr/lib/liquide/liquid-session` | Session process binary | `0755 root:root` |
| `/usr/lib/liquide/xdg-desktop-portal-liquide` | Portal backend | `0755 root:root` |
| `/usr/lib/liquide/liquid-greeter` | Greeter helper (optional) | `0755 root:root` |
| `/etc/liquide/` | System configuration | `0755 root:root` |
| `/etc/liquide/server.toml` | Main server config | `0640 root:liquide` |
| `/etc/liquide/policies.toml` | Policy definitions | `0640 root:liquide` |
| `/etc/liquide/manager.toml` | Management UI config | `0640 root:liquide` |
| `/etc/liquide/plugins/` | System-wide plugins | `0755 root:root` |
| `/etc/liquide/certs/` | TLS certificates | `0700 liquide:liquide` |

#### Runtime Paths (volatile, tmpfs)

| Path | Purpose | Permissions |
|------|---------|-------------|
| `/run/liquide/` | Supervisor runtime | `0750 root:liquide` |
| `/run/liquide/ctl.sock` | Management control socket | `0660 root:liquide` |
| `/run/liquide/sessions/` | Per-session runtime data | `0750 root:liquide` |
| `/run/liquide/sessions/<id>/` | Session-specific runtime | `0700 <user>:<user>` |
| `/run/user/<uid>/` | XDG_RUNTIME_DIR (systemd-provided) | `0700 <user>:<user>` |
| `/run/user/<uid>/liquide/` | Session Wayland socket, PipeWire | `0700 <user>:<user>` |

#### Persistent State Paths

| Path | Purpose | Permissions |
|------|---------|-------------|
| `/var/lib/liquide/` | Server persistent state | `0750 root:liquide` |
| `/var/lib/liquide/sessions.db` | Session metadata database | `0640 liquide:liquide` |
| `/var/lib/liquide/permissions.db` | App permission grants | `0640 liquide:liquide` |
| `/var/lib/liquide/plugins/` | Installed plugins (global) | `0755 root:liquide` |
| `/var/log/liquide/` | Log directory | `0750 root:liquide` |
| `/var/log/liquide/supervisor.log` | Supervisor daemon log | `0640 liquide:liquide` |
| `/var/log/liquide/sessions/` | Per-session logs | `0750 root:liquide` |
| `/var/log/liquide/sessions/<id>.log` | Session log file | `0640 <user>:liquide` |
| `/var/log/liquide/crashes/` | Crash reports | `0750 root:liquide` |
| `/var/log/liquide/audit/` | Audit log (append-only) | `0640 root:liquide` |

#### User Paths

| Path | Purpose | Permissions |
|------|---------|-------------|
| `~/.config/liquide/` | User configuration | `0700 <user>:<user>` |
| `~/.config/liquide/config.toml` | User config overrides | `0600 <user>:<user>` |
| `~/.config/liquide/plugins/` | User-installed plugins | `0700 <user>:<user>` |
| `~/.config/autostart/` | XDG autostart entries | `0700 <user>:<user>` |
| `~/.local/share/liquide/` | User data | `0700 <user>:<user>` |
| `~/.local/share/liquide/permissions.db` | Per-user permission grants | `0600 <user>:<user>` |
| `~/.local/share/liquide/notification-history.db` | Notification history | `0600 <user>:<user>` |
| `~/.local/share/liquide/crash-reports/` | User-accessible crash reports | `0700 <user>:<user>` |
| `~/.cache/liquide/` | Cache (icons, thumbnails, etc.) | `0700 <user>:<user>` |
| `~/.cache/liquide/icon-cache/` | Resolved icon cache | `0700 <user>:<user>` |
| `~/.cache/liquide/plugin-cache/` | AOT-compiled WASM cache | `0700 <user>:<user>` |

### 7.2 XDG Base Directory Compliance

LiquiDE fully respects the [XDG Base Directory Specification](https://specifications.freedesktop.org/basedir-spec/latest/):

| Variable | Default | Usage |
|----------|---------|-------|
| `XDG_CONFIG_HOME` | `~/.config` | User configuration |
| `XDG_DATA_HOME` | `~/.local/share` | User data |
| `XDG_CACHE_HOME` | `~/.cache` | Cached data |
| `XDG_RUNTIME_DIR` | `/run/user/<uid>` | Runtime files (sockets, etc.) |
| `XDG_CONFIG_DIRS` | `/etc/xdg` | System configuration search |
| `XDG_DATA_DIRS` | `/usr/local/share:/usr/share` | System data search |

All LiquiDE components use `$XDG_*` variables when set, falling back to defaults.

---

## 8) Environment Variables

### 8.1 Session Environment

When `liquid-session` starts a user session, it sets the following environment variables:

| Variable | Value | Description |
|----------|-------|-------------|
| `XDG_SESSION_TYPE` | `wayland` | Session type |
| `XDG_CURRENT_DESKTOP` | `LiquiDE` | Desktop environment name |
| `DESKTOP_SESSION` | `liquide` | Session name |
| `XDG_SESSION_DESKTOP` | `liquide` | Session desktop (GDM compat) |
| `WAYLAND_DISPLAY` | `liquide-0` | Wayland compositor socket name |
| `DISPLAY` | `:N` (if XWayland enabled) | X11 display number |
| `XDG_RUNTIME_DIR` | `/run/user/<uid>` | Runtime directory |
| `DBUS_SESSION_BUS_ADDRESS` | `unix:path=/run/user/<uid>/bus` | Session D-Bus |
| `LIQUIDE_SESSION_ID` | `<session-id>` | LiquiDE session identifier |
| `LIQUIDE_VERSION` | `<version>` | LiquiDE version string |
| `GDK_BACKEND` | `wayland` | GTK backend hint |
| `QT_QPA_PLATFORM` | `wayland` | Qt platform hint |
| `SDL_VIDEODRIVER` | `wayland` | SDL backend hint |
| `CLUTTER_BACKEND` | `wayland` | Clutter backend hint |
| `MOZ_ENABLE_WAYLAND` | `1` | Firefox Wayland mode |
| `ELECTRON_OZONE_PLATFORM_HINT` | `wayland` | Electron Wayland hint |
| `_JAVA_AWT_WM_NONREPARENTING` | `1` | Java AWT compat |

### 8.2 PAM Environment

Variables from `/etc/environment`, `/etc/default/locale`, and `~/.pam_environment` (if it exists) are loaded via `pam_env.so` before the session environment is applied.

### 8.3 User Environment Overrides

Users can set additional environment variables in:
- `~/.config/liquide/env.conf` — one `KEY=VALUE` per line.
- Session config: `[session] environment = { "FOO" = "bar" }`.

These are applied after PAM environment and LiquiDE defaults. User values override system values.

---

## 9) Service User & Group

### 9.1 System User — `liquide`

LiquiDE creates a system user and group during installation:

```bash
useradd --system --no-create-home --shell /usr/sbin/nologin --group liquide liquide
```

| Property | Value |
|----------|-------|
| Username | `liquide` |
| Group | `liquide` |
| Home | `/var/lib/liquide` (no login shell) |
| Shell | `/usr/sbin/nologin` |
| Purpose | Owns daemon files, membership grants `liquidctl` access |

### 9.2 Group Membership

- Users in the `liquide` group can access the management socket (`/run/liquide/ctl.sock`) and use `liquidctl`.
- The `video` and `render` groups may be needed for GPU access (handled by systemd `DeviceAllow`).

---

## 10) Logging Infrastructure

### 10.1 Log Destinations

| Component | Destination | Format |
|-----------|-------------|--------|
| `liquid-desktopd` | systemd journal + `/var/log/liquide/supervisor.log` | Structured JSON |
| `liquid-session` | systemd journal + `/var/log/liquide/sessions/<id>.log` | Structured JSON |
| Audit events | `/var/log/liquide/audit/audit.log` | Append-only structured JSON |
| Crash reports | `/var/log/liquide/crashes/` | Individual JSON files |

### 10.2 Log Rotation

- File logs are rotated by LiquiDE internally (configurable max size + max files).
- Default: 50 MB max per file, 10 files retained.
- Crash reports: retained for `crash_report_retention_days` (default: 30), max `crash_report_max_count` (default: 1000).
- Audit logs: append-only, rotated monthly, retained for `audit_retention_days` (default: 365).

### 10.3 journald Integration

All log output goes to stdout/stderr, which systemd captures to the journal. The journal can be queried:

```bash
# Supervisor logs
journalctl -u liquid-desktopd.service

# Session logs for a specific user
journalctl -u liquid-session@alice.service

# All LiquiDE logs
journalctl -t liquid-desktopd -t liquid-session
```

---

## 11) Greeter / Login Flow (Backend)

### 11.1 Login Sequence

The login flow involves coordination between client, supervisor, and PAM:

```
Client                          liquid-desktopd              PAM
  │                                   │                       │
  ├── TLS handshake ────────────────►│                       │
  ├── Request server info ──────────►│                       │
  │◄──── Server info + auth methods ──┤                       │
  ├── Username ─────────────────────►│                       │
  │                                   ├── pam_start() ──────►│
  │                                   ├── pam_authenticate() ►│
  │◄──── PAM prompt (password) ───────┤◄── PAM converse ─────┤
  ├── Password ─────────────────────►│── PAM response ──────►│
  │                                   │◄── PAM_SUCCESS ───────┤
  │                                   ├── pam_acct_mgmt() ──►│
  │                                   │◄── PAM_SUCCESS ───────┤
  │                                   ├── pam_open_session() ►│
  │                                   │◄── OK ────────────────┤
  │                                   ├── spawn liquid-session │
  │◄──── Session ready ──────────────┤                       │
  │◄──── First frame ────────────────┤                       │
```

### 11.2 Multi-Step Authentication

For PAM stacks that require multiple conversation rounds (e.g., password + TOTP):

1. The server sends a `login_prompt` message to the client with: `prompt_type` (password, text, otp, pin), `message` (display text), `echo` (whether input should be visible).
2. The client renders the appropriate input field in the login screen.
3. The user provides the input; client sends `login_response` back.
4. The server feeds the response to the PAM conversation function.
5. Repeat until PAM returns `PAM_SUCCESS` or a terminal error.

### 11.3 Session Resumption

When a client connects and an existing session exists for the user:
1. Server checks if the session process is still alive (heartbeat).
2. If alive: offer "Resume existing session" on the login screen (see spec-client.md login screen).
3. If dead: offer "Start new session" or attempt auto-restart per supervisor policy.

---

## 12) tmpfiles.d Configuration

LiquiDE installs a tmpfiles.d configuration to ensure runtime directories exist:

```ini
# /usr/lib/tmpfiles.d/liquide.conf
d /run/liquide 0750 root liquide -
d /run/liquide/sessions 0750 root liquide -
d /var/log/liquide 0750 root liquide -
d /var/log/liquide/sessions 0750 root liquide -
d /var/log/liquide/crashes 0750 root liquide -
d /var/log/liquide/audit 0750 root liquide -
d /var/lib/liquide 0750 root liquide -
d /var/lib/liquide/plugins 0755 root liquide -
```

---

## 13) Installation Artifacts

### 13.1 Package Contents

A LiquiDE server package installs:

| File | Description |
|------|-------------|
| `/usr/bin/liquid-desktopd` | Supervisor daemon |
| `/usr/bin/liquidctl` | CLI management tool |
| `/usr/lib/liquide/liquid-session` | Session process binary |
| `/usr/lib/liquide/xdg-desktop-portal-liquide` | Portal backend |
| `/usr/lib/systemd/system/liquid-desktopd.service` | System unit |
| `/usr/lib/systemd/system/liquid-desktopd.socket` | Socket unit |
| `/usr/lib/tmpfiles.d/liquide.conf` | Runtime directory creation |
| `/usr/lib/sysusers.d/liquide.conf` | System user creation |
| `/usr/share/dbus-1/services/org.freedesktop.impl.portal.desktop.liquide.service` | Portal D-Bus activation |
| `/usr/share/xdg-desktop-portal/portals/liquide.portal` | Portal registration |
| `/usr/share/wayland-sessions/liquide.desktop` | Session entry for display managers |
| `/etc/liquide/server.toml` | Default config (marked as conffile) |
| `/etc/liquide/policies.toml` | Default policies (marked as conffile) |
| `/etc/pam.d/liquide-session` | PAM service file |
| `/usr/share/man/man1/liquidctl.1.gz` | Man page |
| `/usr/share/man/man8/liquid-desktopd.8.gz` | Man page |

### 13.2 sysusers.d — `/usr/lib/sysusers.d/liquide.conf`

```
u liquide - "LiquiDE system user" /var/lib/liquide /usr/sbin/nologin
```

### 13.3 Display Manager Integration

LiquiDE provides a `.desktop` session entry for GDM, SDDM, and other display managers:

```ini
# /usr/share/wayland-sessions/liquide.desktop
[Desktop Entry]
Name=LiquiDE
Comment=LiquiDE Remote Desktop Environment
Exec=/usr/bin/liquid-desktopd --local-session
Type=Application
DesktopNames=LiquiDE
```

The `--local-session` flag starts a single-user local session (useful for development or when running LiquiDE as a local DE instead of a remote server).

---

## 14) Test Plan

### Functional
- `liquid-desktopd.service` starts successfully, reports ready via sd_notify.
- Socket activation via `liquid-desktopd.socket` works.
- Session processes spawn as correct UID/GID.
- PAM authentication succeeds/fails correctly for each backend (unix, LDAP, SSSD).
- Multi-factor PAM flows complete correctly.
- XDG autostart entries are processed (early and normal phases).
- File permissions match specification.
- Environment variables are set correctly in session.

### Security
- Seccomp filter blocks forbidden syscalls.
- Capabilities are dropped correctly (verify via `/proc/<pid>/status`).
- `ProtectSystem=strict` prevents writes to `/usr`, `/etc`.
- SELinux confinement works (domain transitions, denied operations).
- AppArmor profile denies out-of-profile access.
- Management socket is not accessible to non-`liquide` group users.

### Integration
- PAM + LDAP authentication works.
- PAM session events trigger correct LiquiDE behaviors.
- journald logging is structured and filterable.
- Crash reports are written to correct paths with correct permissions.
- Log rotation works at configured thresholds.
- tmpfiles.d creates directories on boot.
