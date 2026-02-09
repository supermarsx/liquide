# liquidctl — CLI Tool Specification

> **Language**: Rust
> **License**: MIT
> **Related specs**: [Server/DE](spec.md) · [Client](spec-client.md) · [Gateway](spec-gateway.md) · [Management UI](spec-manager.md) · [Design Language](spec-design.md)

---

## 0) Overview

**liquidctl** is the unified command-line tool for administering, monitoring, and troubleshooting LiquiDE servers. It is a single static binary written in Rust that communicates with the LiquiDE server daemon via a local Unix socket or a remote API endpoint.

`liquidctl` is self-documenting, versioned, and designed to never produce "unknown subcommand" dead ends.

---

## 1) Design Principles

- **Single binary** — one tool for everything.
- **Self-documenting** — `liquidctl help` and `liquidctl <command> --help` always work.
- **Structured output** — all commands support `--format json` for scripting.
- **Human-friendly defaults** — colored, formatted output for interactive use.
- **Safe by default** — destructive operations require `--confirm` or interactive confirmation.
- **Remote capable** — can manage local or remote servers via API.

---

## 2) Global Options

```
liquidctl [global-options] <command> [command-options]

Global options:
  --server <address>      Server address (default: local Unix socket)
  --api-key <key>         API key for remote authentication
  --format <format>       Output format: text (default), json, csv, table
  --color <when>          Colorize output: auto (default), always, never
  --quiet                 Suppress non-essential output
  --verbose               Increase output verbosity
  --help                  Show help
  --version               Show version
```

---

## 3) Command Reference

### `liquidctl status`

Display overall server status.

```
$ liquidctl status

LiquiDE Server v0.1.0
  Status:       running
  Uptime:       3d 14h 22m
  Architecture: x86_64
  GPU:          none (CPU-only mode)
  Sessions:     12 active / 50 max
  CPU:          23% (4 cores)
  Memory:       1.8 GB / 8.0 GB
  Bandwidth:    142 Mbps out / 8 Mbps in
  Transport:    QUIC (primary), TLS/TCP (fallback active: 2 sessions)
  Listeners:    0.0.0.0:3389 (quic), 0.0.0.0:3390 (tls-tcp)
  Gateway:      registered (gateway.example.com)
```

Options:
- `--watch` — continuous refresh (default: every 2s).
- `--watch-interval <seconds>` — custom refresh interval.

---

### `liquidctl sessions`

Manage active sessions.

#### `liquidctl sessions list`

```
$ liquidctl sessions list

ID       User     Monitor(s)   Resolution     Encoder   Transport  Latency  FPS   Bandwidth   Duration
s-001    alice    1            1920x1080      h264/cpu  quic       12ms     60    28 Mbps     2h 14m
s-002    bob      2            3840x2160×2    h265/gpu  quic       18ms     45    85 Mbps     0h 42m
s-003    carol    1            1280x720       av1/cpu   tls-tcp    45ms     30    4 Mbps      5h 01m
```

Options:
- `--user <name>` — filter by user.
- `--sort <field>` — sort by column.
- `--watch` — live updating.

#### `liquidctl sessions show <session-id>`

Show detailed session information.

```
$ liquidctl sessions show s-001

Session s-001
  User:           alice
  Started:        2025-01-15 14:22:31 UTC
  Duration:       2h 14m 33s
  Client:         LiquidClient 0.1.0 (macOS ARM64)
  Client IP:      203.0.113.42
  Transport:      QUIC (v1)
  Encryption:     TLS 1.3 (AES-256-GCM)

  Monitors:
    #0: 1920x1080 @ 60Hz, 96 DPI

  Encoding:
    Video:        H.264 (CPU, x264)
    Preset:       interactive
    Tile mode:    hybrid (auto)

  Performance:
    FPS:          60 render / 60 encode / 59 present
    Latency:      12ms RTT, ~18ms input-to-photon
    Bandwidth:    28.4 Mbps out / 0.8 Mbps in
    Packet loss:  0.01%
    Encode time:  4.2ms avg / 8.1ms p99
    Cache hits:   blur=94%, wallpaper=100%, partial=87%
    Effect budget: 62% utilized (5.0ms / 8.0ms)

  Features:
    Clipboard:    bidirectional (text, images)
    Audio:        playback (opus), microphone (off)
    Camera:       off
    USB:          none

  Policy:
    Group:        developers
    Overrides:    none
```

#### `liquidctl sessions disconnect <session-id>`

Disconnect a session.

```
$ liquidctl sessions disconnect s-001
Disconnect session s-001 (user: alice)? [y/N] y
Session s-001 disconnected.
```

Options:
- `--confirm` — skip interactive confirmation.
- `--message <msg>` — send a message to the user before disconnecting.

#### `liquidctl sessions disconnect-all`

Disconnect all sessions.

Options:
- `--user <name>` — disconnect only sessions for a specific user.
- `--confirm` — skip interactive confirmation.
- `--drain` — stop accepting new sessions and wait for existing to end gracefully.

---

### `liquidctl users`

Manage connected users.

#### `liquidctl users list`

```
$ liquidctl users list

User     Sessions  Last Login            Policy Group
alice    1         2025-01-15 14:22 UTC  developers
bob      2         2025-01-15 15:40 UTC  developers
carol    1         2025-01-15 10:15 UTC  default
```

#### `liquidctl users show <username>`

Detailed user information including active sessions, policy, and history.

#### `liquidctl users kick <username>`

Disconnect all sessions for a user.

#### `liquidctl users avatar set <username> <path>`

Set or replace a user's avatar image. Supported formats: PNG, JPEG, WebP, SVG.

- SVG files are sanitized (scripts, external references, and entity expansions removed) and rasterized to PNG before storage. The original SVG is not retained.
- Images are resized to fit within 256×256px and stored as PNG regardless of input format.
- Maximum upload size: configurable via `[avatar] max_size_kb` (default 256 KB).

```
$ liquidctl users avatar set alice /tmp/alice-photo.png
Avatar updated for user 'alice' (256×256 PNG, 42 KB).

$ liquidctl users avatar set bob /tmp/logo.svg
SVG sanitized and rasterized. Avatar updated for user 'bob' (256×256 PNG, 38 KB).
```

#### `liquidctl users avatar remove <username>`

Remove a user's avatar, reverting to initial-based fallback.

```
$ liquidctl users avatar remove alice
Avatar removed for user 'alice'.
```

#### `liquidctl users avatar show <username>`

Display avatar metadata for a user.

```
$ liquidctl users avatar show alice
User:       alice
Has Avatar: yes
Format:     PNG (stored)
Size:       256×256
File Size:  42 KB
Uploaded:   2025-01-15 14:30 UTC
Source:     SVG (rasterized on upload)
```

---

### `liquidctl stats`

Display real-time stream statistics.

```
$ liquidctl stats

Aggregate Statistics
  Sessions:       12 active
  Total FPS:      avg 52, min 30, max 60
  Total Output:   142 Mbps
  Total Input:    12 Mbps
  Avg Latency:    22ms RTT
  Packet Loss:    0.02% avg
  Cache Hits:     blur=91%, wallpaper=99%, partial=85%

Per-Encoder Distribution:
  h264/cpu:    8 sessions (67%)
  h265/gpu:    2 sessions (17%)
  av1/cpu:     1 session  (8%)
  tiles/zstd:  1 session  (8%)

Transport Distribution:
  quic:        10 sessions (83%)
  tls-tcp:     2 sessions  (17%)
```

Options:
- `--session <id>` — show stats for a specific session.
- `--watch` — live updating.
- `--interval <ms>` — update interval (default: 1000ms).
- `--format json` — machine-readable output for monitoring scripts.

---

### `liquidctl benchmark`

Run performance benchmarks.

```
$ liquidctl benchmark

Running LiquiDE Performance Benchmark...

CPU Information:
  Model:          AMD EPYC 7763 64-Core
  Cores:          64 (128 threads)
  Architecture:   x86_64
  SIMD:           SSE4.2, AVX2, AVX-512

Compositing Throughput:
  Single-core:    2.4 Gpixels/s (AVX2)
  Multi-core:     38.7 Gpixels/s (16 threads)
  1080p compose:  0.8ms per frame

Blur Throughput:
  Gaussian r=20, 1080p:     12.4ms (single-core)
  Gaussian r=20, 1080p:     1.6ms (8 threads)
  Gaussian r=20, 1080p/4:   0.4ms (downsampled, 8 threads)
  Box blur r=20, 1080p:     0.2ms (8 threads)

Encoder Throughput (1080p):
  x264 (ultrafast):     2.1ms  /  480 fps
  x264 (veryfast):      4.8ms  /  210 fps
  x265 (ultrafast):     8.2ms  /  122 fps
  SVT-AV1 (preset 12):  12.1ms /  83 fps
  VAAPI H.264:          not available (no GPU)

Tile Compression (1080p, 128x128 tiles):
  Zstd (level 1):    0.3ms / 3200 fps
  LZ4:               0.1ms / 9600 fps
  PNG:               2.8ms / 360 fps
  QOI:               0.2ms / 4800 fps

Memory Bandwidth:
  Sequential read:    42 GB/s
  Sequential write:   38 GB/s
  Buffer copy:        19 GB/s

Recommended Settings:
  Effect budget:     8ms (auto)
  Blur downsample:   4x (auto)
  Default encoder:   h264 (x264 ultrafast)
  Max concurrent:    ~25 sessions @ 1080p60
```

Options:
- `--quick` — abbreviated benchmark (compositing + top encoder only).
- `--full` — full benchmark (all encoders, all blur modes, all tile codecs).
- `--save` — save results to file for later comparison.

---

### `liquidctl config`

Configuration management.

#### `liquidctl config show`

Display current server configuration (redacted secrets).

```
$ liquidctl config show

[general]
hostname = "liquid-server-01"
log_level = "info"
...

[tls]
cert = "/etc/liquide/cert.pem"
key = "***REDACTED***"
...
```

Options:
- `--section <name>` — show only a specific section.
- `--raw` — show without redacting secrets (requires admin).
- `--defaults` — show default values for all settings.

#### `liquidctl config validate`

Validate configuration files.

```
$ liquidctl config validate

Validating /etc/liquide/server.toml...
  ✓ Syntax valid
  ✓ All required fields present
  ✓ TLS certificate found and readable
  ✓ TLS key found and readable
  ✓ Listen addresses valid
  ⚠ [encoding] hardware_encoding = "auto" but no GPU detected — will use CPU
  ✓ Policy file /etc/liquide/policies.toml valid

Validation passed (1 warning).
```

#### `liquidctl config set <key> <value>`

Set a configuration value (hot-reload if supported).

```
$ liquidctl config set performance.active_fps 45
Set performance.active_fps = 45
Configuration reloaded.
```

Options:
- `--no-reload` — write to file but don't hot-reload.

#### `liquidctl config diff`

Show differences between running config and on-disk config.

#### `liquidctl config export`

Export current config to stdout (for backup or transfer).

#### `liquidctl config import <file>`

Import and apply a configuration file.

---

### `liquidctl policy`

Policy management.

#### `liquidctl policy show`

Display current policies.

```
$ liquidctl policy show

[default]
  clipboard:        bidirectional
  file_transfer:    true
  audio_playback:   true
  audio_microphone: false
  camera:           false
  usb_redirection:  false
  max_sessions:     3
  max_resolution:   3840x2160
  max_fps:          60

[group.developers]
  clipboard:        bidirectional
  file_transfer:    true
  max_sessions:     5

[group.guests]
  clipboard:        server-to-client
  file_transfer:    false
  max_resolution:   1920x1080
  max_fps:          30
```

#### `liquidctl policy set <scope> <key> <value>`

Set a policy value.

```
$ liquidctl policy set group.guests max_fps 15
Set group.guests.max_fps = 15
Policy reloaded. Affects 3 active sessions.
```

#### `liquidctl policy effective <username>`

Show the effective policy for a specific user (after all inheritance and overrides).

```
$ liquidctl policy effective alice

Effective policy for alice (group: developers):
  clipboard:        bidirectional           (from: group.developers)
  file_transfer:    true                    (from: group.developers)
  audio_playback:   true                    (from: default)
  audio_microphone: false                   (from: default)
  camera:           false                   (from: default)
  usb_redirection:  false                   (from: default)
  max_sessions:     5                       (from: group.developers)
  max_resolution:   3840x2160              (from: default)
  max_fps:          60                      (from: default)
```

---

### `liquidctl monitors`

Manage virtual monitors.

#### `liquidctl monitors list`

```
$ liquidctl monitors list --session s-001

Session s-001 (alice):
  Monitor #0: 1920x1080 @ 60Hz, 96 DPI (primary)
```

#### `liquidctl monitors add <session-id>`

Add a virtual monitor to a session.

```
$ liquidctl monitors add s-001 --resolution 1920x1080 --dpi 96
Added monitor #1 (1920x1080 @ 60Hz, 96 DPI) to session s-001.
```

#### `liquidctl monitors remove <session-id> <monitor-id>`

Remove a virtual monitor.

#### `liquidctl monitors resize <session-id> <monitor-id> <resolution>`

Resize a virtual monitor.

---

### `liquidctl transport`

Manage transport settings and view transport status.

#### `liquidctl transport status`

```
$ liquidctl transport status

Active Transports:
  QUIC (0.0.0.0:3389):     10 connections, 128 Mbps
  TLS/TCP (0.0.0.0:3390):  2 connections, 14 Mbps

Transport Negotiation: auto
Preferred: quic
Priority: quic > udp > tls-tcp > tcp > websocket
Hybrid channels: enabled
MTU: 1400 (discovered)
FEC: disabled
Congestion: BBR
```

#### `liquidctl transport switch <session-id> <transport>`

Force a session to switch transport.

```
$ liquidctl transport switch s-003 quic
Switching session s-003 from tls-tcp to quic...
Transport switched successfully. New latency: 28ms (was 45ms).
```

---

### `liquidctl audio`

Manage audio subsystem.

#### `liquidctl audio status`

```
$ liquidctl audio status

Audio Subsystem: active
  Playback:     12 sessions using playback
  Microphone:   2 sessions using microphone
  Codec:        opus (default)
  Sample rate:  48000 Hz

Backend: PipeWire
```

---

### `liquidctl encoder`

Manage encoders.

#### `liquidctl encoder list`

```
$ liquidctl encoder list

Available Encoders:
  Name          Type     HW Accel   Status
  h264/x264     video    CPU        active (8 sessions)
  h265/x265     video    CPU        active (1 session)
  av1/svt-av1   video    CPU        active (1 session)
  vp9/libvpx    video    CPU        available
  vp8/libvpx    video    CPU        available
  mjpeg/turbo   video    CPU        available
  h264/vaapi    video    GPU        not available (no GPU)
  h265/vaapi    video    GPU        not available (no GPU)
  zstd          tile     CPU        active (hybrid tiles)
  lz4           tile     CPU        available
  png           tile     CPU        available
  qoi           tile     CPU        available
  webp          tile     CPU        available
  raw           tile     CPU        available
```

#### `liquidctl encoder benchmark <encoder>`

Benchmark a specific encoder.

---

### `liquidctl usb`

Manage USB device forwarding.

#### `liquidctl usb status`

```
$ liquidctl usb status

USB/IP Subsystem: enabled
  Active forwards:   3 devices across 2 sessions
  Bandwidth:         12 Mbps total

Per-Session:
  s-001 (alice):     2 devices (USB drive, YubiKey)
  s-002 (bob):       1 device (printer)
```

#### `liquidctl usb list`

List USB devices forwarded in a session.

```
$ liquidctl usb list --session s-001

Session s-001 (alice):
  ID       VID:PID      Class           Device Name              Status
  usb-01   0781:5583    mass-storage    SanDisk Ultra USB 3.0    connected
  usb-02   1050:0407    security-key    YubiKey 5 NFC            connected
```

#### `liquidctl usb disconnect <session-id> <device-id>`

Disconnect a forwarded USB device.

```
$ liquidctl usb disconnect s-001 usb-01
Disconnect USB device usb-01 (SanDisk Ultra USB 3.0) from session s-001? [y/N] y
Device usb-01 disconnected. Safely ejected.
```

Options:
- `--confirm` — skip interactive confirmation.
- `--force` — force disconnect without safe eject.

#### `liquidctl usb disconnect-all <session-id>`

Disconnect all forwarded USB devices from a session.

---

### `liquidctl logs`

View and manage logs.

#### `liquidctl logs tail`

Stream live logs.

```
$ liquidctl logs tail --level info

2025-01-15T16:22:31Z INFO  [session] Session s-013 started: user=dave, client=LiquidClient/0.1.0
2025-01-15T16:22:31Z INFO  [transport] QUIC connection established: s-013, RTT=14ms
2025-01-15T16:22:32Z INFO  [encoder] Encoder selected: h264/x264 (ultrafast) for s-013
```

Options:
- `--level <level>` — filter by log level.
- `--session <id>` — filter by session.
- `--subsystem <name>` — filter by subsystem (server, session, auth, render, encode, transport, audio, clipboard, usb, input, policy, metrics, audit).
- `--since <time>` — show logs since a time.
- `--follow` — stay attached and stream new logs.

#### `liquidctl logs search <pattern>`

Search historical logs.

```
$ liquidctl logs search "login_failure" --since 24h --subsystem auth

2025-01-15 09:12:44  WARN  [auth] login_failure user=unknown IP=198.51.100.99 reason=invalid_password
2025-01-15 09:12:47  WARN  [auth] login_failure user=unknown IP=198.51.100.99 reason=invalid_password
2025-01-15 09:12:49  WARN  [auth] login_failure user=unknown IP=198.51.100.99 reason=invalid_password
2025-01-15 09:12:49  WARN  [auth] rate_limit_lockout IP=198.51.100.99 duration=600s
```

Options:
- `--subsystem <name>` — filter by subsystem.
- `--session <id>` — filter by session correlation ID.
- `--since <time>` — time range start.
- `--until <time>` — time range end.
- `--limit <n>` — max entries.

#### `liquidctl logs config`

View and modify per-subsystem log levels at runtime.

```
$ liquidctl logs config

Log Subsystems:
  Subsystem     Level    Log File
  server        info     /var/log/liquide/server.log
  session       info     /var/log/liquide/session.log
  auth          info     /var/log/liquide/auth.log
  render        warn     /var/log/liquide/render.log
  encode        warn     /var/log/liquide/encode.log
  transport     info     /var/log/liquide/transport.log
  audio         warn     /var/log/liquide/audio.log
  clipboard     info     /var/log/liquide/clipboard.log
  usb           info     /var/log/liquide/usb.log
  input         warn     /var/log/liquide/input.log
  policy        info     /var/log/liquide/policy.log
  metrics       warn     /var/log/liquide/metrics.log
  audit         info     /var/log/liquide/audit.log (immutable)

General:
  Format:       json
  Base dir:     /var/log/liquide
  Max file:     100 MB
  Rotation:     10 files, compressed
  Syslog:       disabled
```

#### `liquidctl logs level <subsystem> <level>`

Change log level for a subsystem at runtime (hot-reload, no restart).

```
$ liquidctl logs level render debug
Set render log level to debug.
Configuration reloaded.
```

#### `liquidctl logs rotate`

Force log rotation for all or specific subsystems.

```
$ liquidctl logs rotate --subsystem auth
Rotated auth.log → auth.log.1.gz
```

---

### `liquidctl audit`

View audit events.

#### `liquidctl audit list`

```
$ liquidctl audit list --since 24h

Time                    Event              User     Details
2025-01-15 10:15:31     login_success      carol    IP: 198.51.100.5
2025-01-15 10:15:31     session_start      carol    s-003, 1280x720
2025-01-15 10:16:02     clipboard_sync     carol    text, 142 bytes, server→client
2025-01-15 14:22:31     login_success      alice    IP: 203.0.113.42
...
```

Options:
- `--event <type>` — filter by event type.
- `--user <name>` — filter by user.
- `--since <time>` — time range.
- `--limit <n>` — max entries.

---

### `liquidctl honeypot`

Monitor and manage the honeypot/tarpit system.

#### `liquidctl honeypot status`

```
$ liquidctl honeypot status

Honeypot & Tarpit Status: active (mode: both)
  Active tarpits:     7 / 100 max
  Honeypot sessions:  2 active

  Tarpit Breakdown:
    TCP tarpit:       3 connections (1 byte/sec drip)
    TLS tarpit:       2 connections (slow handshake)
    Auth tarpit:      2 connections (fake processing)

  Today's Statistics:
    Tarpitted IPs:     23
    Honeypot captures: 8
    Exploit attempts:  3
    Credential stuffing detected: 5
    Post-ban reconnects: 12
    IOCs exported:     15

  Top Attacker IPs (24h):
    198.51.100.99      142 attempts (tarpitted, post-ban)
    203.0.113.77        87 attempts (credential stuffing)
    192.0.2.15          34 attempts (exploit payloads)
```

Options:
- `--watch` — live updating.

#### `liquidctl honeypot list`

List active tarpit and honeypot connections.

```
$ liquidctl honeypot list

ID        IP               Type         Started           Duration    Trigger
hp-001    198.51.100.99    tcp-tarpit   16:22:31 UTC      12m 04s     post-ban reconnect
hp-002    203.0.113.77     auth-tarpit  16:28:14 UTC      5m 51s      credential stuffing
hp-003    192.0.2.15       honeypot     16:30:02 UTC      3m 03s      exploit signature (CVE-2019-0708)
hp-004    198.51.100.22    tls-tarpit   16:31:45 UTC      1m 20s      TLS downgrade
hp-005    10.0.0.99        tcp-tarpit   16:33:01 UTC      0m 04s      invalid protocol magic
```

#### `liquidctl honeypot drop <connection-id>`

Release a tarpit/honeypot connection immediately.

#### `liquidctl honeypot drop-all`

Release all active tarpit/honeypot connections.

#### `liquidctl honeypot iocs`

List or export collected indicators of compromise.

```
$ liquidctl honeypot iocs --since 24h --format table

IP               First Seen           Last Seen            Type                 Attempts   Payloads
198.51.100.99    2025-01-15 04:12     2025-01-15 16:34     brute-force          142        0
203.0.113.77     2025-01-15 14:00     2025-01-15 16:28     credential-stuffing  87         0
192.0.2.15       2025-01-15 16:30     2025-01-15 16:33     exploit              3          3
```

Options:
- `--since <time>` — time range.
- `--format <format>` — text, json, csv, stix.
- `--export <file>` — export to file.

#### `liquidctl honeypot triggers`

Show which triggers are enabled and their thresholds.

```
$ liquidctl honeypot triggers

Trigger                      Enabled   Threshold           Response
Invalid protocol magic       yes       any                 tarpit
Known exploit signatures     yes       any                 honeypot
Post-ban reconnection        yes       after IP ban        tarpit
Credential stuffing          yes       10 users/60s        tarpit
TLS downgrade attack         yes       any                 tarpit
Port scan follow-up          yes       3 ports/60s         honeypot
Malformed packet flood       yes       100 pkt/s           tarpit
```

---

### `liquidctl lock`

Manage session locks.

#### `liquidctl lock status`

Show lock state for all sessions.

```
$ liquidctl lock status

Session  User     State      Locked Since          Idle Time   Escalation
s-001    alice    unlocked   —                     2m 14s      —
s-002    bob      locked     2025-01-15 15:40 UTC  1h 22m      disconnect in 2h 38m
s-003    carol    locked     2025-01-15 12:00 UTC  5h 02m      background (terminate in 18h 58m)
s-004    dave     blank      —                     8m 30s      lock in 6m 30s
```

#### `liquidctl lock <session-id>`

Lock a specific session immediately (admin action).

```
$ liquidctl lock s-001
Lock session s-001 (user: alice)? [y/N] y
Session s-001 locked.
```

Options:
- `--confirm` — skip interactive confirmation.
- `--message <msg>` — show a custom message on the lock screen.
- `--reason <reason>` — reason for lock (logged in audit).

#### `liquidctl lock all`

Lock all active sessions.

Options:
- `--confirm` — skip interactive confirmation.
- `--message <msg>` — custom lock screen message for all sessions.

#### `liquidctl unlock <session-id>`

Unlock a locked session (admin override, bypasses user auth).

```
$ liquidctl unlock s-002
Admin-unlock session s-002 (user: bob)? This bypasses user authentication. [y/N] y
Session s-002 unlocked.
```

Options:
- `--confirm` — skip interactive confirmation.

#### `liquidctl lock policy <username>`

Show the effective lock policy for a user.

```
$ liquidctl lock policy alice

Effective lock policy for alice (group: developers):
  Idle lock:            enabled, 30 min          (from: user.alice.lock)
  Screen blank:         enabled, 10 min          (from: default)
  Disconnect action:    lock                     (from: default)
  Background timeout:   480 min (8h)             (from: user.alice.lock)
  Suspend:              disabled                 (from: default)
  Terminate timeout:    1440 min (24h)           (from: default)
  Schedule lock:        disabled                 (from: default)
  Lock clipboard:       pause                    (from: default)
  Lock audio:           continue                 (from: default)
  Lock USB:             continue                 (from: default)
  Lock camera:          pause                    (from: default)
```

#### `liquidctl lock config`

Show current lock configuration.

```
$ liquidctl lock config

Lock Configuration:
  Idle lock:            enabled (15 min)
  Screen blank:         enabled (10 min)
  Disconnect action:    lock
  Background timeout:   240 min (4h)
  Suspend:              disabled
  Schedule lock:        disabled
  Lock screen wallpaper: blur
  Lock screen message:  (none)
```

---

### `liquidctl gateway`

Manage gateway connection.

#### `liquidctl gateway status`

```
$ liquidctl gateway status

Gateway: registered
  URL:        liquide://gateway.example.com
  Status:     connected
  Uptime:     3d 14h
  Mode:       reverse-connect
  Sessions:   5 brokered through gateway
```

#### `liquidctl gateway register`

Manually trigger gateway registration.

#### `liquidctl gateway deregister`

Deregister from gateway.

---

### `liquidctl service`

Manage the LiquiDE service.

#### `liquidctl service status`

Service health check.

#### `liquidctl service restart`

Restart the server daemon (with graceful session handling).

#### `liquidctl service stop`

Stop the server daemon.

Options:
- `--drain` — stop accepting new sessions, wait for existing to end (up to timeout).
- `--force` — immediate stop, disconnect all sessions.
- `--timeout <seconds>` — drain timeout (default: 300).

---

### `liquidctl cache`

Manage rendering caches.

#### `liquidctl cache status`

```
$ liquidctl cache status

Cache Status (all sessions):
  Blur cache:       94% hit rate, 12 entries, 48 MB
  Wallpaper cache:  100% hit rate, 12 entries, 96 MB
  Partial cache:    87% hit rate, 156 entries, 24 MB
  Font cache:       99% hit rate, 342 glyphs, 8 MB
  Total cache:      176 MB
```

#### `liquidctl cache clear`

Clear all or specific caches.

Options:
- `--type <type>` — blur, wallpaper, partial, font, all.
- `--session <id>` — clear caches for a specific session only.

---

### `liquidctl rdp`

Manage RDP compatibility layer.

#### `liquidctl rdp status`

```
$ liquidctl rdp status

RDP Compatibility: disabled
  To enable: liquidctl config set rdp_compat.enabled true
```

#### `liquidctl rdp enable` / `liquidctl rdp disable`

Toggle RDP compatibility without editing config files.

---

### 3.12 `liquidctl plugins` — WASM Plugin Management

#### `liquidctl plugins list [--session <id>] [--format json|table|csv]`

List installed plugins and their status.

```
$ liquidctl plugins list
PLUGIN ID              VERSION   STATUS    MEMORY    CPU       EXTENSION POINTS
clipboard-transform    1.2.0     active    4.2 MB    0.3%      clipboard-transformer
custom-panel           0.8.1     active    8.1 MB    1.2%      panel-widget
shell-ext-git          2.0.0     suspended 0.0 MB    0.0%      shell-extension
theme-gen-solarized    1.0.0     active    2.1 MB    0.1%      theme-generator
```

#### `liquidctl plugins info <plugin-id> [--session <id>]`

Show detailed plugin information: manifest, resource usage, fault history, configuration.

#### `liquidctl plugins install <source> [--signature-check] [--dry-run]`

Install a plugin from a local `.wasm` file, directory, or registry URL.
- `--signature-check` — require Ed25519 signature validation (overrides server config).
- `--dry-run` — validate manifest and signature without installing.

#### `liquidctl plugins uninstall <plugin-id> [--purge]`

Remove an installed plugin. `--purge` removes plugin configuration and stored data.

#### `liquidctl plugins enable <plugin-id> [--session <id>]`

Enable a plugin for the specified session (or all sessions if `--session` omitted).

#### `liquidctl plugins disable <plugin-id> [--session <id>]`

Disable a plugin. The plugin is deactivated and unloaded from any running sessions.

#### `liquidctl plugins reload <plugin-id> [--session <id>]`

Hot-reload a plugin: suspend → unload old → load new → resume. State is preserved if the plugin supports it.

#### `liquidctl plugins config <plugin-id> [key] [value]`

Get or set per-plugin configuration values.

```bash
# List all plugin config
liquidctl plugins config clipboard-transform

# Set a value
liquidctl plugins config clipboard-transform strip_formatting true
```

---

### 3.13 `liquidctl crash` — Crash Report Management

#### `liquidctl crash list [--limit N] [--session <id>] [--since <date>] [--format json|table|csv]`

List crash reports.

```
$ liquidctl crash list --limit 5
REPORT ID    SESSION    USER     TIMESTAMP                ERROR CODE               EXIT
cr-001       s-042      alice    2025-01-15T16:22:31Z     SESSION_PROCESS_CRASH     SIGSEGV
cr-002       s-038      bob      2025-01-15T14:01:12Z     SESSION_OOM              OOM
cr-003       s-042      alice    2025-01-15T12:55:48Z     SESSION_PROCESS_CRASH     SIGABRT
```

#### `liquidctl crash show <report-id> [--format json|text]`

Display full crash report details: error code, stack trace, session metadata, system info, last log lines.

#### `liquidctl crash export <report-id> [--output <path>] [--include-coredump]`

Export a crash report to a file. JSON by default. `--include-coredump` bundles the coredump in a `.tar.gz` archive.

#### `liquidctl crash delete <report-id> [--all] [--older-than <days>]`

Delete crash reports. `--all` deletes all reports. `--older-than` deletes reports older than N days.

#### `liquidctl crash stats [--since <date>]`

Show crash statistics: total crashes, crashes by error code, mean time between failures, sessions affected.

---

### 3.14 `liquidctl supervisor` — Session Supervisor Management

#### `liquidctl supervisor status [--format json|table]`

Show supervisor status and all managed session processes.

```
$ liquidctl supervisor status
SUPERVISOR: running (PID 1234, uptime 14d 3h 22m)

SESSION    USER     PID      STATE      UPTIME       RESTARTS   MEMORY     CPU
s-042      alice    5678     running    2h 15m       0          312 MB     4.2%
s-038      bob      5901     running    6h 02m       1          256 MB     2.1%
s-051      charlie  —        failed     —            5/5        —          —
```

#### `liquidctl supervisor restart <session-id> [--force]`

Request the supervisor to restart a session process. `--force` kills the current process immediately (SIGKILL) before restarting.

#### `liquidctl supervisor reset-restarts <session-id>`

Reset the restart counter for a session, allowing it to be restarted again after hitting the maximum.

#### `liquidctl supervisor logs [--session <id>] [--lines N] [--follow]`

View supervisor logs. `--session` filters to a specific session. `--follow` tails the log in real-time.

---

### 3.15 `liquidctl flatpak` — Flatpak Application Management

Manage Flatpak applications, runtimes, and remotes. These commands proxy to the system Flatpak installation with LiquiDE policy enforcement and structured output.

#### `liquidctl flatpak search <query>`

Search Flathub (and other configured remotes) for applications.

```
$ liquidctl flatpak search firefox

ID                       Name            Version   Remote    Description
org.mozilla.firefox      Firefox         124.0.1   flathub   Fast, private & safe web browser
org.mozilla.Thunderbird  Thunderbird     115.8.0   flathub   Email, calendar, and contacts
io.gitlab.librewolf      LibreWolf       124.0     flathub   Privacy-focused Firefox fork
```

Options:
- `--remote <name>` — search only a specific remote.

#### `liquidctl flatpak install <app-id> [--user|--system] [--noninteractive] [--no-deps]`

Install a Flatpak application.

```
$ liquidctl flatpak install org.mozilla.firefox

Installing org.mozilla.firefox from flathub...

Permissions requested:
  ✓ Network access
  ✓ Wayland
  ✓ GPU acceleration (DRI)
  ⚠ Filesystem: home (read/write)
  ⚠ X11 fallback

Proceed? [Y/n] y

Downloading:  org.mozilla.firefox  [=========>          ]  65%  (156/241 MB)
Installing:   Done.
```

Options:
- `--user` — install for current user only (default).
- `--system` — install system-wide (requires polkit).
- `--noninteractive` — skip permission review prompt (for scripting).
- `--no-deps` — do not install runtime dependencies (advanced).

#### `liquidctl flatpak remove <app-id> [--user|--system] [--delete-data] [--noninteractive]`

Remove a Flatpak application.

```
$ liquidctl flatpak remove org.mozilla.firefox

Remove Firefox?
  App data in ~/.var/app/org.mozilla.firefox/ will be kept.
  Use --delete-data to also remove app data.

Proceed? [Y/n] y
Removing:     Done.
```

Options:
- `--delete-data` — also remove `~/.var/app/<app-id>/`.
- `--noninteractive` — skip confirmation.

#### `liquidctl flatpak list [--user|--system|--all] [--columns <cols>]`

List installed Flatpak applications.

```
$ liquidctl flatpak list

ID                       Name            Version   Size      Remote    Scope
org.mozilla.firefox      Firefox         124.0.1   241 MB    flathub   user
org.gimp.GIMP            GIMP            2.10.36   312 MB    flathub   system
org.videolan.VLC         VLC             3.0.20    145 MB    flathub   user
```

Options:
- `--user` — show only per-user installs.
- `--system` — show only system-wide installs.
- `--all` — show all (default).
- `--runtimes` — also show installed runtimes.
- `--columns <cols>` — customize output columns.

#### `liquidctl flatpak update [<app-id>] [--user|--system] [--check] [--noninteractive]`

Update Flatpak applications and runtimes.

```
$ liquidctl flatpak update

Checking for updates...
  org.mozilla.firefox     124.0 → 124.0.1   (12 MB)
  org.gimp.GIMP           2.10.36 → 2.10.38 (45 MB)

Downloading: [===================>]  100%
Updated 2 applications.
```

Options:
- `<app-id>` — update a specific app only.
- `--check` — check for updates without applying.
- `--system` — update system-wide installs.
- `--noninteractive` — do not prompt.

#### `liquidctl flatpak permissions <app-id>`

Show the effective permissions of a Flatpak application.

```
$ liquidctl flatpak permissions org.mozilla.firefox

Permissions for org.mozilla.firefox (Firefox):

  Filesystem:
    ✓ home         (read/write)
    ✓ /tmp         (read/write)
    ✗ host         (denied by override)

  Network:
    ✓ network

  Sockets:
    ✓ wayland
    ✗ x11          (denied by override)
    ✓ pulseaudio

  Devices:
    ✓ dri

  D-Bus (session):
    ✓ org.freedesktop.Notifications
    ✓ org.freedesktop.portal.*

  Source: manifest + user override (~/.local/share/flatpak/overrides/org.mozilla.firefox)
```

#### `liquidctl flatpak override <app-id> [options]`

Set permission overrides for a Flatpak application.

```bash
# Grant filesystem access
liquidctl flatpak override org.mozilla.firefox --filesystem=~/Downloads

# Deny network
liquidctl flatpak override org.mozilla.firefox --no-network

# Deny X11
liquidctl flatpak override org.mozilla.firefox --nosocket=x11

# Reset all overrides to manifest defaults
liquidctl flatpak override org.mozilla.firefox --reset
```

Options mirror `flatpak override` flags: `--filesystem`, `--nofilesystem`, `--socket`, `--nosocket`, `--device`, `--nodevice`, `--share`, `--unshare`, `--talk-name`, `--no-talk-name`, `--reset`.

#### `liquidctl flatpak remote-list`

List configured Flatpak remotes.

```
$ liquidctl flatpak remote-list

Name          URL                                          Scope    Enabled
flathub       https://dl.flathub.org/repo/               system   yes
flathub-beta  https://dl.flathub.org/beta-repo/           system   no
myrepo        https://example.com/repo/                   user     yes
```

#### `liquidctl flatpak remote-add <name> <url|.flatpakrepo> [--user|--system]`

Add a Flatpak remote repository.

```bash
liquidctl flatpak remote-add myrepo https://example.com/repo.flatpakrepo
```

#### `liquidctl flatpak remote-remove <name> [--force]`

Remove a Flatpak remote. `--force` also removes all apps installed from that remote.

#### `liquidctl flatpak rollback <app-id>`

Rollback a Flatpak app to the previous OSTree commit.

```
$ liquidctl flatpak rollback org.mozilla.firefox

Rollback org.mozilla.firefox:
  Current: 124.0.1 (commit a1b2c3d)
  Target:  124.0   (commit e4f5g6h)

Proceed? [Y/n] y
Rolled back to 124.0.
```

#### `liquidctl flatpak history <app-id> [--limit N]`

Show version/commit history for a Flatpak app.

```
$ liquidctl flatpak history org.mozilla.firefox

Commit      Version   Date                 Size
a1b2c3d     124.0.1   2025-02-05 10:00     241 MB   (current)
e4f5g6h     124.0     2025-02-01 08:30     240 MB
i7j8k9l     123.0.1   2025-01-15 12:15     238 MB
```

#### `liquidctl flatpak gc [--unused-runtimes] [--dry-run]`

Garbage-collect unused Flatpak data.

```bash
# Remove unused runtimes
liquidctl flatpak gc --unused-runtimes

# Preview what would be removed
liquidctl flatpak gc --unused-runtimes --dry-run
```

---

### 3.16 `liquidctl brew` — Homebrew Package Management

Manage Homebrew formulae and casks. These commands proxy to the system Homebrew installation with LiquiDE policy enforcement and structured output.

#### `liquidctl brew search <query>`

Search Homebrew for formulae and casks.

```bash
$ liquidctl brew search liquide

Name              Type       Version   Description
liquide           formula    1.4.0     LiquiDE remote desktop server + CLI tools
liquidclient      cask       1.4.0     LiquiDE client application
```

Options:
- `--formula` — search only formulae.
- `--cask` — search only casks.

#### `liquidctl brew install <formula|cask> [--cask] [--formula]`

Install a Homebrew formula or cask.

```bash
$ liquidctl brew install liquide
Installing liquide (formula) via Homebrew...
==> Downloading liquide-1.4.0.tar.gz
==> Installing dependencies: openssl@3
==> Installing liquide
Done. Installed liquide 1.4.0.

$ liquidctl brew install liquidclient --cask
Installing liquidclient (cask) via Homebrew...
==> Downloading LiquidClient-1.4.0.dmg
==> Installing Cask liquidclient
Done. LiquidClient.app installed to /Applications.
```

#### `liquidctl brew remove <formula|cask> [--cask] [--formula]`

Remove a Homebrew formula or cask.

#### `liquidctl brew list [--formula] [--cask] [--json]`

List installed Homebrew formulae and casks.

```bash
$ liquidctl brew list

Name              Type       Version   Size      Outdated
liquide           formula    1.3.2     45 MB     yes (1.4.0)
liquidclient      cask       1.4.0     128 MB    no
openssl@3         formula    3.2.1     12 MB     no
```

#### `liquidctl brew update [<formula|cask>] [--check] [--cask] [--formula]`

Update Homebrew packages.

```bash
$ liquidctl brew update --check

Available Homebrew updates:
  liquide          1.3.2 → 1.4.0   (formula)
  liquidclient     1.3.2 → 1.4.0   (cask)

$ liquidctl brew update
Updating liquide (1.3.2 → 1.4.0)...
Updating liquidclient (1.3.2 → 1.4.0)...
Updated 2 packages.
```

#### `liquidctl brew info <formula|cask>`

Show detailed information about a Homebrew formula or cask.

#### `liquidctl brew tap <tap-name>`

Add a Homebrew tap.

```bash
$ liquidctl brew tap liquide/tap
Tapped liquide/tap (3 formulae, 1 cask).
```

#### `liquidctl brew untap <tap-name>`

Remove a Homebrew tap.

#### `liquidctl brew pin <formula>`

Pin a formula to prevent automatic upgrades.

```bash
$ liquidctl brew pin liquide
Pinned liquide (1.4.0).
```

#### `liquidctl brew unpin <formula>`

Unpin a formula to resume automatic upgrades.

#### `liquidctl brew rollback <formula|cask>`

Rollback to the previous version.

```bash
$ liquidctl brew rollback liquide

Rollback liquide:
  Current: 1.4.0
  Target:  1.3.2

Proceed? [Y/n] y
Rolled back to 1.3.2.
```

---

### 3.17 `liquidctl snap` — Snap Package Management

Manage Snap packages. These commands proxy to `snapd` with LiquiDE policy enforcement and structured output.

#### `liquidctl snap search <query>`

Search the Snap Store for packages.

```bash
$ liquidctl snap search liquidclient

Name              Publisher     Version   Summary
liquidclient      liquide✓      1.4.0     LiquiDE remote desktop client
liquide-server    liquide✓      1.4.0     LiquiDE remote desktop server
```

#### `liquidctl snap install <snap> [--channel <channel>] [--classic] [--devmode]`

Install a snap package.

```bash
$ liquidctl snap install liquidclient
Installing liquidclient from stable channel...
liquidclient 1.4.0 from LiquiDE (liquide✓) installed.

$ liquidctl snap install liquide-server --channel=beta
Installing liquide-server from beta channel...
liquide-server 1.5.0-beta.2 from LiquiDE (liquide✓) installed.
```

#### `liquidctl snap remove <snap> [--purge]`

Remove a snap package. `--purge` also removes snapshots and data.

#### `liquidctl snap list [--all]`

List installed snaps.

```bash
$ liquidctl snap list

Name              Version   Rev    Channel   Publisher     Confinement
liquidclient      1.4.0     142    stable    liquide✓      strict
liquide-server    1.4.0     98     stable    liquide✓      classic
```

Options:
- `--all` — include disabled revisions.

#### `liquidctl snap update [<snap>] [--check] [--channel <channel>]`

Update snap packages.

```bash
$ liquidctl snap update --check

Available Snap updates:
  liquidclient     1.3.2 → 1.4.0   (stable channel)
  liquide-server   1.3.2 → 1.4.0   (stable channel)

$ liquidctl snap update liquidclient
Refreshing liquidclient (1.3.2 → 1.4.0)...
liquidclient 1.4.0 refreshed.
```

#### `liquidctl snap info <snap>`

Show detailed information about a snap.

#### `liquidctl snap connections <snap>`

List interface connections for a snap.

```bash
$ liquidctl snap connections liquidclient

Interface        Plug                          Slot              Status
audio-playback   liquidclient:audio-playback   :audio-playback   connected
desktop          liquidclient:desktop          :desktop          connected
network          liquidclient:network          :network          connected
opengl           liquidclient:opengl           :opengl           connected
wayland          liquidclient:wayland          :wayland          connected
x11              liquidclient:x11              :x11              connected
audio-record     liquidclient:audio-record     -                 disconnected
```

#### `liquidctl snap connect <snap> <interface>`

Connect a snap interface plug.

```bash
$ liquidctl snap connect liquidclient audio-record
Connected liquidclient:audio-record to :audio-record.
```

#### `liquidctl snap disconnect <snap> <interface>`

Disconnect a snap interface plug.

#### `liquidctl snap revert <snap>`

Revert a snap to the previous revision.

```bash
$ liquidctl snap revert liquidclient

Revert liquidclient:
  Current: 1.4.0 (rev 142)
  Target:  1.3.2 (rev 135)

Proceed? [Y/n] y
Reverted liquidclient to revision 135 (1.3.2).
```

#### `liquidctl snap refresh-hold <snap> --duration <hours>`

Hold automatic snap refreshes for a specified duration.

```bash
$ liquidctl snap refresh-hold liquidclient --duration 72
Holding refresh for liquidclient for 72 hours (until 2025-01-18 16:00 UTC).
```

#### `liquidctl snap channels <snap>`

Show available channels for a snap.

```bash
$ liquidctl snap channels liquidclient

Channel        Version       Published
stable         1.4.0         2025-02-01
candidate      1.4.0         2025-01-28
beta           1.5.0-beta.1  2025-02-05
edge           1.5.0-dev.42  2025-02-08
```

---

### 3.18 `liquidctl nix` — Nix Package Management

Manage Nix packages and profiles. These commands proxy to the Nix CLI with LiquiDE policy enforcement and structured output.

#### `liquidctl nix search <query>`

Search nixpkgs for packages.

```bash
$ liquidctl nix search liquide

Package             Version   Description
nixpkgs#liquide     1.4.0     LiquiDE remote desktop (server + CLI + client)
nixpkgs#liquidclient 1.4.0    LiquiDE client only
```

Options:
- `--flake <ref>` — search a specific flake instead of nixpkgs.

#### `liquidctl nix install <package> [--profile <name>]`

Install a Nix package to the current profile.

```bash
$ liquidctl nix install nixpkgs#liquide
Installing liquide 1.4.0...
Done. Added to profile.

$ liquidctl nix install github:liquide/liquide --profile server
Installing from flake github:liquide/liquide...
Done. Added to profile 'server'.
```

#### `liquidctl nix remove <package>`

Remove a Nix package from the current profile.

#### `liquidctl nix list [--profile <name>] [--json]`

List installed Nix packages.

```bash
$ liquidctl nix list

Index   Package             Version   Store Path
0       nixpkgs#liquide     1.4.0     /nix/store/abc123...-liquide-1.4.0
1       nixpkgs#git         2.43.0    /nix/store/def456...-git-2.43.0
```

#### `liquidctl nix update [<package>] [--profile <name>] [--check]`

Update Nix packages.

```bash
$ liquidctl nix update --check

Available Nix updates:
  liquide     1.3.2 → 1.4.0

$ liquidctl nix update
Updating profile...
Upgraded liquide (1.3.2 → 1.4.0).
```

#### `liquidctl nix rollback [--profile <name>]`

Rollback to the previous profile generation.

```bash
$ liquidctl nix rollback

Rollback profile:
  Current: generation 42
  Target:  generation 41

Proceed? [Y/n] y
Rolled back to generation 41.
```

#### `liquidctl nix gc [--older-than <days>] [--dry-run]`

Garbage-collect unused Nix store paths.

```bash
$ liquidctl nix gc --older-than 30 --dry-run

Would remove 847 store paths (12.4 GB).

$ liquidctl nix gc --older-than 30
Removing 847 store paths...
Freed 12.4 GB.
```

#### `liquidctl nix develop [--flake <ref>]`

Enter a Nix development shell with all LiquiDE build dependencies.

```bash
$ liquidctl nix develop
Entering development shell for github:liquide/liquide...
[nix-develop]$
```

---

### 3.19 `liquidctl appimage` — AppImage Management

Manage AppImage files for the LiquiDE client. These commands handle desktop integration, updates, and signature verification.

#### `liquidctl appimage list`

List integrated AppImage files.

```bash
$ liquidctl appimage list

Name              Version   Path                                    Integrated
LiquidClient      1.4.0     ~/Applications/LiquidClient-x86_64.AppImage   yes
```

#### `liquidctl appimage update [<app>] [--check]`

Check for and apply AppImage updates using the AppImageUpdate delta mechanism.

```bash
$ liquidctl appimage update --check

Available AppImage updates:
  LiquidClient   1.3.2 → 1.4.0   (delta: 18 MB)

$ liquidctl appimage update
Downloading delta update for LiquidClient...
  [===================>]  100%  (18 MB)
Updated LiquidClient to 1.4.0.
```

#### `liquidctl appimage integrate <file>`

Integrate an AppImage into the desktop (create `.desktop` entry and icon).

```bash
$ liquidctl appimage integrate ~/Downloads/LiquidClient-x86_64.AppImage

Moving to ~/Applications/...
Creating desktop entry...
Extracting icon...
Integrated LiquidClient (1.4.0). Available in application launcher.
```

#### `liquidctl appimage remove <app>`

Remove an integrated AppImage and its desktop entry.

#### `liquidctl appimage verify <file>`

Verify the Ed25519 signature embedded in an AppImage.

```bash
$ liquidctl appimage verify ~/Applications/LiquidClient-x86_64.AppImage

Signature: valid (signed by LiquiDE release key)
Version:   1.4.0
SHA-256:   a1b2c3d4e5f6...
```

---

## 4) Shell Completion

`liquidctl` generates shell completions:

```bash
# Bash
liquidctl completions bash > /etc/bash_completion.d/liquidctl

# Zsh
liquidctl completions zsh > /usr/local/share/zsh/site-functions/_liquidctl

# Fish
liquidctl completions fish > ~/.config/fish/completions/liquidctl.fish

# PowerShell
liquidctl completions powershell > $PROFILE/liquidctl.ps1
```

---

## 5) Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Invalid arguments / usage error |
| 3 | Connection error (cannot reach server) |
| 4 | Authentication error |
| 5 | Permission denied |
| 6 | Resource not found (session, user, etc.) |
| 7 | Operation cancelled by user |
| 8 | Timeout |
| 9 | Plugin error (install failed, invalid manifest, signature mismatch) |
| 10 | Supervisor error (session process management failure) |
| 11 | Crash report error (report not found, export failed) |

---

## 6) Configuration

`liquidctl` reads its own config from:
- `~/.config/liquidctl/config.toml` (Linux/macOS)
- `%APPDATA%\liquidctl\config.toml` (Windows)

```toml
[default]
server = "unix:///run/liquide/ctl.sock"   # default local socket
format = "text"                             # text, json, csv, table
color = "auto"                              # auto, always, never

[remote.prod]
server = "https://liquid-server.example.com:9100"
api_key = "remote-api-key"

[remote.staging]
server = "https://staging.example.com:9100"
api_key = "staging-api-key"
```

Use remote profiles: `liquidctl --server @prod sessions list`.

---

## 7) Man Page

`liquidctl` installs a man page at `liquidctl(1)` covering all commands, options, exit codes, and examples.

---

## 8) Test Plan

### Functional
- Every command and subcommand produces correct output.
- `--format json` produces valid JSON for every command.
- `--help` works for every command and subcommand.
- Shell completions work for bash, zsh, fish, PowerShell.
- Remote management (via API) works with authentication.
- Destructive commands require confirmation.

### Edge Cases
- Server not running.
- Invalid session/user/monitor ID.
- Permission denied for non-admin operations.
- Network timeout for remote management.
- Concurrent `liquidctl` invocations.

### Integration
- `liquidctl benchmark` results match observed performance.
- `liquidctl config set` + hot-reload actually changes running behavior.
- `liquidctl policy set` affects active sessions in real-time.
- `liquidctl transport switch` performs seamless switch.
- `liquidctl sessions disconnect` cleanly terminates sessions.
- `liquidctl plugins install` downloads and installs a plugin.
- `liquidctl plugins enable`/`disable` toggles plugin state for a session.
- `liquidctl plugins info` shows accurate resource usage and status.
- `liquidctl crash list` shows recent crash reports with correct metadata.
- `liquidctl crash show` displays full crash report details.
- `liquidctl crash export` produces valid, importable JSON/tar archives.
- `liquidctl supervisor status` reports accurate session process states.
- `liquidctl supervisor restart` successfully restarts a failed session.
- `liquidctl flatpak search` returns matching Flathub results.
- `liquidctl flatpak install` installs app with runtime, shows permissions, respects policy.
- `liquidctl flatpak remove` removes app, `--delete-data` clears `~/.var/app/`.
- `liquidctl flatpak list` shows installed apps with correct metadata.
- `liquidctl flatpak update` downloads and applies updates, `--check` shows available only.
- `liquidctl flatpak permissions` shows effective permissions with override sources.
- `liquidctl flatpak override` modifies per-app sandbox permissions.
- `liquidctl flatpak remote-add/remote-remove/remote-list` manages remotes correctly.
- `liquidctl flatpak rollback` reverts to previous commit.
- `liquidctl flatpak history` shows commit history with versions and dates.
- `liquidctl flatpak gc` removes unused runtimes, `--dry-run` previews without acting.
- `liquidctl brew search` returns matching formulae and casks from Homebrew.
- `liquidctl brew install` installs formula/cask with dependency resolution.
- `liquidctl brew update` upgrades installed packages, `--check` shows available only.
- `liquidctl brew tap/untap` manages custom taps correctly.
- `liquidctl brew pin/unpin` prevents and resumes automatic upgrades.
- `liquidctl brew rollback` restores previous version.
- `liquidctl snap search` returns matching snaps from the Snap Store.
- `liquidctl snap install` installs snap with correct confinement, `--channel` selects track.
- `liquidctl snap update` refreshes installed snaps, `--check` shows available only.
- `liquidctl snap connections` lists interface connections accurately.
- `liquidctl snap connect/disconnect` manages interface plugs correctly.
- `liquidctl snap revert` rolls back to previous snap revision.
- `liquidctl snap refresh-hold` defers automatic refresh for specified duration.
- `liquidctl nix search` returns matching packages from nixpkgs.
- `liquidctl nix install` installs package to profile, resolves dependencies.
- `liquidctl nix update` upgrades installed packages or full profile.
- `liquidctl nix rollback` reverts to previous profile generation.
- `liquidctl nix gc` collects unused store paths.
- `liquidctl nix develop` enters development shell with all deps.
- `liquidctl appimage list` shows integrated AppImages with version info.
- `liquidctl appimage update` downloads delta update, replaces AppImage in-place.
- `liquidctl appimage integrate` creates desktop entry and icon.
- `liquidctl appimage verify` checks Ed25519 signature and reports status.
