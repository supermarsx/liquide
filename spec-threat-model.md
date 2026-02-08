# LiquiDE — Threat Model & Security Architecture

> **Status**: Living document
> **Related specs**: [Main Spec](spec.md) · [Normative Conventions](spec-normative.md) · [Protocol](spec-protocol-formal.md) · [Gateway](spec-gateway.md) · [Client](spec-client.md)

---

## 1) Purpose

This document provides a systematic threat analysis of the LiquiDE remote desktop system using STRIDE methodology. It covers data flow diagrams (DFDs) for critical operations, a threat catalog with mitigations, key management lifecycle, and a unified audit event schema.

This document references trust boundaries defined in [spec-normative.md §3](spec-normative.md).

---

## 2) Data Flow Diagrams

> **Note**: Channel IDs referenced in these diagrams use the canonical assignments defined in [spec-protocol-formal.md §2 Channel Assignments](spec-protocol-formal.md). See that table for the complete channel ID → subsystem mapping.

### 2.1 Session Establishment

```
                                                    ┌─────────────────┐
                                                    │   Certificate   │
                                                    │   Authority     │
                                                    │   (external)    │
                                                    └────────┬────────┘
                                                             │ cert chain
                                                             ▼
┌──────────────┐     (1) TLS ClientHello      ┌────────────────────────┐
│              │ ──────────────────────────►   │                        │
│ LiquidClient │     (2) TLS ServerHello      │   Gateway / Server     │
│              │ ◄────────────────────────     │   (liquid-gateway or   │
│              │     (3) TLS Finished         │   liquid-desktopd)     │
│              │ ══════════════════════════    │                        │
│              │   [TLS 1.3 established]      │                        │
│              │                               │                        │
│              │     (4) AuthRequest           │                        │
│              │ ──────────────────────────►   │                        │
│              │     {username}                │                        │
│              │                               │     ┌──────────────┐  │
│              │     (5) AuthChallenge         │     │  PAM / LDAP  │  │
│              │ ◄────────────────────────     │     │  / OIDC      │  │
│              │     {methods, avatar}         │     └──────────────┘  │
│              │                               │                        │
│              │     (6) AuthResponse          │                        │
│              │ ──────────────────────────►   │                        │
│              │     {password + MFA token}    │                        │
│              │                               │                        │
│              │     (7) AuthResult            │                        │
│              │ ◄────────────────────────     │                        │
│              │     {session_token,           │                        │
│              │      session_id,              │                        │
│              │      capabilities}            │                        │
│              │                               │                        │
│              │     (8) ChannelSetup          │     ┌──────────────┐  │
│              │ ◄═══════════════════════►     │     │liquid-session │  │
│              │   [multiplexed channels       │     │ (spawned)     │  │
│              │    established]                │     └──────────────┘  │
└──────────────┘                               └────────────────────────┘

Data stores accessed:
  [D1] User credential store (PAM/LDAP/local)
  [D2] Session state database (in-memory / persisted)
  [D3] Certificate store (/etc/liquide/certs/)
  [D4] Audit log (/var/log/liquide/audit.log)
```

**Sensitive data in transit**: Username, password, MFA token, session token, TLS session keys.

### 2.2 Emergency / Crash Recovery

```
┌──────────────┐                         ┌──────────────────┐
│liquid-session │ ──(SIGSEGV/panic)───►  │  liquid-desktopd │
│  (crashed)   │                         │   (supervisor)   │
└──────────────┘                         │                  │
                                          │  (1) Detect crash│
                                          │      (SIGCHLD,   │
                                          │       exit code)  │
                                          │                  │
                                          │  (2) Capture     │
                                          │      context:    │
        ┌──────────────┐                 │      - coredump   │
        │ LiquidClient │ ◄──────────     │      - logs      │
        │              │  (3) crash_info │      - metadata   │
        │              │  message        │                  │
        │  [renders    │                  │  (4) Write crash │
        │   crash      │                  │      report to   │
        │   screen]    │                  │      [D5]        │
        │              │                  │                  │
        │              │ ◄──────────     │  (5) Restart      │
        │              │  (6) new        │      liquid-      │
        │              │  session ready  │      session      │
        └──────────────┘                 └──────────────────┘

Data stores:
  [D4] Audit log — crash event recorded
  [D5] Crash report store (/var/log/liquide/crashes/)
  [D6] Coredump store (configured via systemd-coredump)
```

**Sensitive data**: Coredumps may contain user data (screen buffer contents, clipboard). Stack traces may reveal internal structure. Crash reports MUST be sanitized (see §5).

### 2.3 Clipboard Transfer

```
┌──────────────┐     Clipboard channel (0x30)     ┌──────────────┐
│ LiquidClient │ ◄═══════════════════════════════► │liquid-session │
│              │           TLS-encrypted            │              │
│              │                                    │              │
│  [Local      │     (1) ClipboardOffer             │  [Wayland    │
│   clipboard  │ ◄────────────────────────          │   clipboard  │
│   API]       │     {mime_types, size}             │   (wl_data_  │
│              │                                    │    device)]  │
│              │     (2) ClipboardRequest           │              │
│              │ ────────────────────────►          │  [Plugin     │
│              │     {accepted_mime}                │   transform  │
│              │                                    │   pipeline]  │
│              │     (3) ClipboardData             │              │
│              │ ◄────────────────────────          │              │
│              │     {mime, data (≤max_size)}       │              │
│              │                                    │              │
│              │  Policy engine checks:             │              │
│              │  - clipboard.enabled               │              │
│              │  - clipboard.direction             │              │
│              │  - clipboard.max_size              │              │
│              │  - clipboard.allowed_mime_types    │              │
└──────────────┘                                    └──────────────┘
```

**Sensitive data**: Clipboard may contain passwords, PII, proprietary content. Direction and content-type policies are critical controls.

### 2.4 USB Device Redirection

```
┌──────────────┐       USB channel (0x40)         ┌──────────────┐
│ LiquidClient │ ═══════════════════════════════►  │liquid-session │
│              │          TLS-encrypted             │              │
│  [USB device │                                    │  [USB/IP     │
│   attached   │     (1) UsbDeviceAttach           │   kernel     │
│   locally]   │ ────────────────────────►          │   module]    │
│              │     {vendor_id, product_id,        │              │
│              │      device_class, serial}         │              │
│              │                                    │  Policy:     │
│              │     (2) UsbDeviceAccepted /        │  - usb.enabled│
│              │         UsbDeviceRejected          │  - usb.      │
│              │ ◄────────────────────────          │    allowed_  │
│              │                                    │    classes   │
│              │     (3) USB I/O data              │  - usb.      │
│              │ ◄═══════════════════════►          │    allowed_  │
│              │                                    │    devices   │
│              │                                    │  - usb.      │
│              │                                    │    blocked_  │
│              │                                    │    devices   │
└──────────────┘                                    └──────────────┘
```

**Sensitive data**: USB traffic may contain keystrokes (HID), storage data, or smart card secrets. Device class filtering is a critical security control.

---

## 3) STRIDE Threat Analysis

### Threat Catalog

| ID | Category | Threat | Target | Boundary | Likelihood | Impact | Mitigation | Residual Risk |
|----|----------|--------|--------|----------|-----------|--------|------------|---------------|
| T-01 | **S**poofing | Attacker impersonates a legitimate LiquiDE server | Client ↔ Server (B1) | 1 | Medium | High | TLS certificate verification, certificate pinning, TOFU with fingerprint display | Low — requires CA compromise or user ignoring warning |
| T-02 | **S**poofing | Attacker replays captured authentication token | Client ↔ Server (B1) | 1 | Medium | High | Session tokens are short-lived, bound to TLS session (channel binding), non-replayable nonce in challenge-response | Low |
| T-03 | **S**poofing | Attacker forges gateway auth header | Gateway ↔ Server (B2) | 2 | Low | Critical | mTLS between gateway and server; gateway identity verified by certificate | Low |
| T-04 | **T**ampering | MITM modifies protocol messages in transit | Client ↔ Server (B1) | 1 | Medium | High | TLS 1.3 AEAD encryption (AES-256-GCM / ChaCha20-Poly1305). All data integrity-protected. | Negligible |
| T-05 | **T**ampering | Malicious plugin modifies session state | Host ↔ Plugin (B4) | 4 | Medium | Medium | WASM sandbox: plugin has no direct access to host memory. All mutations go through validated host function calls. Plugin capabilities checked at load time. | Low |
| T-06 | **T**ampering | Compromised Wayland client sends malformed buffers | Compositor ↔ App (B5) | 5 | Medium | Medium | Buffer size validation, SHM pool bounds checking, protocol message validation, per-client rate limiting | Low |
| T-07 | **R**epudiation | User denies performing destructive action | All boundaries | All | Medium | Medium | Comprehensive audit logging with tamper-evident log chain (see §6). All security-relevant events logged with session ID, user, timestamp, source IP. | Low |
| T-08 | **R**epudiation | Admin denies modifying policy | Admin ↔ Manager (B6) | 6 | Low | High | All admin actions logged with authenticated identity and before/after state. Audit logs are append-only, shipped to syslog. | Low |
| T-09 | **I**nformation Disclosure | User enumeration via login screen timing | Client ↔ Server (B1) | 1 | High | Low | Constant-time response for all usernames (existing and non-existing). Generic avatar fallback indistinguishable from no-avatar user. | Low |
| T-10 | **I**nformation Disclosure | Clipboard data exfiltration | Client ↔ Server (B1) | 1 | Medium | High | Clipboard direction policy (server-to-client, client-to-server, both, none). Size limits. MIME type filtering. Clipboard audit logging. Plugin transformer for PII redaction. | Low — policy dependent |
| T-11 | **I**nformation Disclosure | Coredump contains user screen data | Supervisor (B3) | 3 | Medium | High | Coredumps stored with restrictive permissions (0600, root-owned). Crash reports sanitize stack traces (no variable values). Coredumps in sensitive deployments can be disabled via policy. | Medium — defense in depth |
| T-12 | **I**nformation Disclosure | Plugin reads other plugin's data | Host ↔ Plugin (B4) | 4 | Low | Medium | Each WASM instance has isolated linear memory. No shared memory between plugins. Plugin storage is namespace-scoped via host functions. | Negligible |
| T-13 | **D**enial of Service | Client floods server with input events | Client ↔ Server (B1) | 1 | High | Medium | Per-channel rate limiting. Input event coalescing. Connection-level bandwidth caps. fail2ban integration for repeated offenders. | Low |
| T-14 | **D**enial of Service | Plugin enters infinite loop | Host ↔ Plugin (B4) | 4 | High | Low | wasmtime fuel-based CPU metering. Wall-clock timeout as backstop. Faulted plugin disabled; session continues. | Negligible |
| T-15 | **D**enial of Service | Plugin exhausts memory | Host ↔ Plugin (B4) | 4 | Medium | Low | Per-plugin memory cap (default 32 MB, max 256 MB). wasmtime OOM trap at sandbox boundary. | Negligible |
| T-16 | **D**enial of Service | Session process exhausts host resources | Supervisor ↔ Session (B3) | 3 | Medium | High | cgroup v2 limits: memory, CPU, PIDs, I/O bandwidth. Supervisor monitors and kills runaway sessions. | Low |
| T-17 | **D**enial of Service | Brute-force authentication attempts | Client ↔ Server (B1) | 1 | High | Medium | fail2ban integration (auth, brute, proto jails). Progressive delays. Account lockout after N failures. Rate limiting at gateway. | Low |
| T-18 | **E**levation of Privilege | Session process escapes jail | Supervisor ↔ Session (B3) | 3 | Low | Critical | cgroup v2, PID namespace, mount namespace, seccomp-bpf, landlock LSM. Session runs as unprivileged user. No `CAP_SYS_ADMIN` on either session or supervisor (supervisor uses cgroup v2 delegation + systemd-run for namespace creation — see [spec-system.md §6.1](spec-system.md)). | Low — requires kernel exploit |
| T-19 | **E**levation of Privilege | Plugin escapes WASM sandbox | Host ↔ Plugin (B4) | 4 | Very Low | Critical | wasmtime sandbox (validated WASM bytecode, no direct syscalls). Plugin signing verification. Reduced WASI capabilities. Regular wasmtime updates. | Very Low — requires wasmtime bug |
| T-20 | **E**levation of Privilege | User escalates to admin via manager | Admin ↔ Manager (B6) | 6 | Low | Critical | RBAC with least-privilege defaults. All API endpoints check authorization. Sensitive operations require re-authentication. No default admin password. | Low |
| T-21 | **S**poofing | Rogue plugin installed by compromised admin | Host ↔ Plugin (B4) | 4 | Low | High | Plugin signature verification (ed25519). Optional policy to restrict plugins to an allowlist. Plugin install requires admin privilege. Audit log records plugin installs. | Low — requires admin compromise |
| T-22 | **I**nformation Disclosure | USB device leaks data to wrong session | Session ↔ USB (B3) | 3 | Low | High | USB device binding is per-session. Device class and vendor/product allowlists enforced by policy. USB channel disabled by default. | Low |
| T-23 | **T**ampering | Malicious gateway injects fake auth claims | Gateway ↔ Server (B2) | 2 | Low | Critical | Server validates gateway mTLS certificate. Auth claims signed by gateway. Replay protection via nonce/timestamp. | Low |

### Risk Rating Scale

| Level | Likelihood × Impact |
|-------|-------------------|
| **Critical** | Immediate action required — architecture must prevent this |
| **High** | Must have mitigation before GA release |
| **Medium** | Should have mitigation; acceptable if compensating controls exist |
| **Low** | Risk accepted with monitoring |
| **Negligible** | No further action needed |

---

## 4) Key Management & Lifecycle

### 4.1 TLS Server Certificates

```
  ┌──────────┐     ┌──────────────┐     ┌──────────────┐     ┌──────────┐
  │ Generate  │────►│   Active     │────►│   Renewal    │────►│  Active  │
  │ (or import│     │              │     │  (ACME or    │     │  (new    │
  │  from CA) │     │              │     │   manual)    │     │   cert)  │
  └──────────┘     └──────┬───────┘     └──────────────┘     └──────────┘
                          │
                          │ compromise / expiry / revocation
                          ▼
                   ┌──────────────┐
                   │   Revoked    │
                   │  (CRL/OCSP)  │
                   └──────────────┘
```

| Property | Specification |
|----------|--------------|
| **Key algorithm** | ECDSA P-256 (RECOMMENDED), RSA-2048 (minimum), Ed25519 (OPTIONAL) |
| **Lifetime** | ACME: 90 days (auto-renewed at 60 days). Manual: admin-configured, recommended ≤ 1 year. |
| **Storage** | Private key in `/etc/liquide/certs/server.key`, permissions `0600`, owned by `liquide` service account. |
| **Rotation** | Zero-downtime: new cert loaded via `SIGHUP` to `liquid-desktopd`. Active connections continue with old cert until they reconnect. |
| **Self-signed bootstrap** | On first boot with no certificate, server generates a self-signed cert and displays the SHA-256 fingerprint in logs and via `liquidctl server status`. Client shows fingerprint for TOFU verification. |
| **Revocation** | CRL or OCSP checked for client certificates (mTLS). Server's own revocation is handled by the CA. |

### 4.2 Client Certificates (mTLS)

| Property | Specification |
|----------|--------------|
| **Issuer** | Enterprise CA or LiquiDE-managed internal CA. |
| **Subject** | CN=username, SAN as configured. |
| **Lifetime** | Recommended ≤ 1 year. Short-lived certificates (hours/days) supported for automated issuance. |
| **Storage (client-side)** | OS certificate store (Windows: CertStore, macOS: Keychain, Linux: `~/.config/liquidclient/certs/`). |
| **Revocation check** | Server checks CRL or OCSP on every connection. Configurable: `fail_open = false` (default) rejects if revocation check fails. |
| **Certificate pinning** | Client MAY pin the server's certificate or CA. Configured in connection profile. |

### 4.3 Session Tokens

| Property | Specification |
|----------|--------------|
| **Format** | Opaque 256-bit random token, encoded as URL-safe base64. |
| **Lifetime** | Default 24 hours, configurable via `session.token_lifetime_sec`. |
| **Binding** | Token is bound to: (1) user identity, (2) session ID, (3) client IP (optional, configurable). |
| **Refresh** | On reconnect within the token's lifetime, the client presents the token. Server validates and issues a new token (rotating token). |
| **Revocation** | Token invalidated on: explicit logout, admin session kill, password change, policy change that affects the user. |
| **Storage (server)** | In-memory session table (process memory). Not written to disk. Lost on daemon restart (sessions must re-authenticate). |
| **Storage (client)** | In-memory during session. Optionally persisted to OS credential store for session resume across client restarts (encrypted at rest). |

### 4.4 Gateway-Server Authentication

| Property | Specification |
|----------|--------------|
| **Method** | mTLS (RECOMMENDED) or pre-shared token over TLS. |
| **Certificate issuer** | Internal CA (same deployment). |
| **Lifetime** | Gateway certificates: ≤ 1 year, auto-rotated. Pre-shared tokens: ≤ 90 days. |
| **Rotation** | Gateway loads new certificate via `SIGHUP`. Server re-reads trusted CA bundle periodically (every 60s). |

### 4.5 Plugin Signing Keys

| Property | Specification |
|----------|--------------|
| **Algorithm** | Ed25519. |
| **Purpose** | Verify plugin `.wasm` integrity and author identity at load time. |
| **Trust store** | `/etc/liquide/plugins/trusted-keys/` — one public key file per trusted author. |
| **Mandatory** | Signature verification is OPTIONAL by default but RECOMMENDED for production. Policy key `plugins.require_signatures = true` makes it mandatory. |
| **Compromise** | Remove author's public key from trust store. All plugins by that author are disabled on next load. |

### 4.6 Key Storage Summary

| Key Material | Location | Permissions | Encrypted at Rest |
|-------------|----------|-------------|-------------------|
| Server TLS private key | `/etc/liquide/certs/server.key` | `0600` liquide:liquide | OPTIONAL (via disk encryption) |
| Client CA cert | `/etc/liquide/certs/client-ca.pem` | `0644` root:root | No (public key) |
| Session tokens | Process memory (liquid-desktopd) | N/A | N/A (memory only) |
| Gateway PSK | `/etc/liquide/gateway-psk` | `0600` liquide:liquide | RECOMMENDED |
| Plugin signing public keys | `/etc/liquide/plugins/trusted-keys/` | `0644` root:root | No (public keys) |
| fail2ban shared state | `/var/run/fail2ban/` | System default | No |

---

## 5) Crash Report Sanitization

Crash reports (see [spec.md §25](spec.md)) are designed for diagnostic value while minimizing sensitive data exposure.

### Included in Crash Reports

| Field | Sensitivity | Notes |
|-------|------------|-------|
| Timestamp | Low | ISO 8601 UTC |
| Error code / signal | Low | e.g., `SESSION_PROCESS_CRASH`, `SIGSEGV` |
| Stack trace (function names + offsets) | Medium | No variable values, no arguments |
| Session metadata (uptime, server version) | Low | |
| Resource usage at crash (memory, FDs, threads) | Low | |
| Last 100 log lines (INFO level only) | Medium | Filtered: no user data, no credentials |
| System info (OS, CPU arch, memory total) | Low | |

### Excluded from Crash Reports

| Field | Reason |
|-------|--------|
| Screen buffer contents | Contains user-visible data |
| Clipboard contents | May contain passwords / PII |
| User file paths or file contents | Privacy |
| Environment variables | May contain secrets |
| Raw memory dumps / core pointers | May contain decrypted data |
| Network packet captures | May contain cleartext from other channels |
| Other users' session data | Session isolation |

### Coredump Policy

| Setting | Default | Description |
|---------|---------|-------------|
| `crash.coredump_enabled` | `true` | Allow coredumps via systemd-coredump |
| `crash.coredump_max_size_mb` | `512` | Maximum coredump size |
| `crash.coredump_retention_days` | `7` | Auto-delete after N days |
| `crash.coredump_encrypt` | `false` | Encrypt coredumps with server certificate |

---

## 6) Unified Audit Event Schema

All security-relevant events across LiquiDE components emit structured audit records in a consistent format.

### Event Format

```json
{
  "version": 1,
  "timestamp": "2025-06-15T14:22:31.847Z",
  "event_type": "auth.login.success",
  "severity": "info",
  "component": "liquid-desktopd",
  "session_id": "s-001",
  "user": "alice",
  "source_ip": "192.168.1.42",
  "source_port": 52431,
  "detail": {
    "auth_method": "password+totp",
    "mfa_type": "totp",
    "client_version": "0.2.1",
    "client_platform": "windows-x86_64",
    "gateway_id": "gw-east-01"
  },
  "outcome": "success",
  "correlation_id": "c-a8f3e2b1"
}
```

### Event Schema Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `version` | integer | Yes | Schema version (currently `1`) |
| `timestamp` | string (ISO 8601 UTC) | Yes | Event time with millisecond precision |
| `event_type` | string | Yes | Dot-separated event type (see catalog below) |
| `severity` | enum | Yes | `debug`, `info`, `warn`, `error`, `critical` |
| `component` | string | Yes | Emitting component (`liquid-desktopd`, `liquid-session`, `liquid-gateway`, `liquid-manager`) |
| `session_id` | string | Conditional | Present when event relates to a specific session |
| `user` | string | Conditional | Present when event relates to a specific user |
| `source_ip` | string | Conditional | Client IP (present for network events) |
| `source_port` | integer | Conditional | Client port |
| `detail` | object | Yes | Event-specific data (schema varies by event_type) |
| `outcome` | enum | Yes | `success`, `failure`, `error`, `denied` |
| `correlation_id` | string | Yes | Unique ID for correlating related events across components |

### Event Type Catalog

| Event Type | Severity | Trigger | Detail Fields |
|-----------|----------|---------|---------------|
| `auth.login.attempt` | info | Client submits credentials | `auth_method`, `client_version`, `client_platform` |
| `auth.login.success` | info | Authentication succeeds | `auth_method`, `mfa_type`, `session_id` |
| `auth.login.failure` | warn | Authentication fails | `auth_method`, `failure_reason` (`bad_password`, `bad_mfa`, `account_locked`, `expired_cert`) |
| `auth.mfa.challenge` | info | MFA challenge issued | `mfa_type` |
| `auth.mfa.success` | info | MFA verification succeeds | `mfa_type` |
| `auth.mfa.failure` | warn | MFA verification fails | `mfa_type`, `failure_reason` |
| `auth.logout` | info | User logs out | `session_duration_sec`, `reason` (`user`, `admin`, `timeout`, `token_expired`) |
| `auth.token.issued` | debug | Session token created/refreshed | `token_lifetime_sec`, `bound_to_ip` |
| `auth.token.revoked` | info | Session token explicitly revoked | `reason` |
| `auth.cert.verified` | info | Client certificate verified (mTLS) | `subject_cn`, `issuer`, `serial`, `expiry` |
| `auth.cert.rejected` | warn | Client certificate rejected | `subject_cn`, `reason` (`expired`, `revoked`, `untrusted_ca`, `crl_check_failed`) |
| `session.created` | info | New session process spawned | `session_id`, `pid`, `cgroup` |
| `session.started` | info | Session ready for user interaction | `session_id`, `resolution`, `monitors` |
| `session.suspended` | info | Session suspended (user idle or explicit) | `session_id`, `reason` |
| `session.resumed` | info | Session resumed | `session_id`, `resume_method` (`reconnect`, `unlock`) |
| `session.terminated` | info | Session ended | `session_id`, `reason` (`user_logout`, `admin_kill`, `timeout`, `crash`) |
| `session.crash` | error | Session process crashed | `session_id`, `signal`, `exit_code`, `crash_report_id` |
| `session.restart` | warn | Session restarted after crash | `session_id`, `restart_count`, `backoff_sec` |
| `connection.established` | info | Client connection accepted | `transport`, `tls_cipher`, `client_version` |
| `connection.closed` | info | Client connection closed | `reason` (`clean`, `timeout`, `error`, `transport_switch`), `duration_sec` |
| `connection.transport_switch` | info | Transport changed mid-session | `from_transport`, `to_transport`, `reason` |
| `clipboard.transfer` | info | Clipboard data transferred | `direction` (`s2c`, `c2s`), `mime_type`, `size_bytes` |
| `clipboard.blocked` | warn | Clipboard transfer blocked by policy | `direction`, `mime_type`, `reason` |
| `usb.device.attached` | info | USB device redirected | `vendor_id`, `product_id`, `device_class`, `serial` |
| `usb.device.rejected` | warn | USB device rejected by policy | `vendor_id`, `product_id`, `device_class`, `reason` |
| `usb.device.detached` | info | USB device disconnected | `vendor_id`, `product_id` |
| `file.transfer.start` | info | File transfer initiated | `direction`, `filename`, `size_bytes` |
| `file.transfer.complete` | info | File transfer completed | `direction`, `filename`, `size_bytes`, `duration_sec` |
| `file.transfer.blocked` | warn | File transfer blocked by policy | `direction`, `filename`, `reason` |
| `policy.changed` | warn | Policy configuration modified | `changed_by`, `policy_key`, `old_value`, `new_value` |
| `policy.override` | info | Per-user/group policy override applied | `policy_key`, `source` (`server`, `group`, `user`), `effective_value` |
| `plugin.loaded` | info | Plugin loaded successfully | `plugin_id`, `version`, `abi_version`, `signature_verified` |
| `plugin.load_failed` | warn | Plugin failed to load | `plugin_id`, `reason` (`abi_mismatch`, `signature_invalid`, `manifest_error`) |
| `plugin.faulted` | warn | Plugin trapped during execution | `plugin_id`, `fault_type` (`fuel_exhausted`, `oom`, `trap`, `timeout`), `extension_point` |
| `plugin.installed` | info | New plugin installed | `plugin_id`, `version`, `installed_by` |
| `plugin.removed` | info | Plugin uninstalled | `plugin_id`, `removed_by` |
| `admin.action` | info | Administrative action via manager/liquidctl | `action`, `target`, `actor`, `before_state`, `after_state` |
| `admin.login` | info | Admin authenticated to manager | `admin_user`, `source_ip` |
| `gateway.route` | debug | Gateway routes connection | `target_server`, `routing_reason` |
| `gateway.reject` | warn | Gateway rejects connection | `reason` (`rate_limit`, `geo_block`, `tls_error`, `auth_failure`) |
| `intrusion.banned` | warn | IP banned by fail2ban | `source_ip`, `jail`, `ban_duration_sec` |
| `intrusion.unbanned` | info | IP ban expired/removed | `source_ip`, `jail` |

### Audit Log Transport

| Destination | Method | Notes |
|-------------|--------|-------|
| Local file | Append to `/var/log/liquide/audit.log` | Default. Log rotation via logrotate. |
| syslog | RFC 5424 structured data | `facility = local0`, configurable |
| journald | `sd_journal_send` | Native on systemd systems |
| External SIEM | syslog-ng / fluentd / vector forwarding | Forward from local file or syslog |

### Audit Configuration

```toml
[audit]
enabled = true
destinations = ["file", "syslog"]       # file, syslog, journald
file_path = "/var/log/liquide/audit.log"
file_max_size_mb = 100
file_max_age_days = 90
file_max_backups = 10
syslog_facility = "local0"
syslog_tag = "liquide"

# Event filtering — only log events at or above this severity
min_severity = "info"                    # debug, info, warn, error, critical

# High-volume event throttling
clipboard_transfer_log_interval_sec = 60 # log at most one clipboard event per 60s per session
```

---

## 7) Security Configuration Baseline

The following table defines the RECOMMENDED security configuration for production deployments.

| Setting | Default | Hardened | Notes |
|---------|---------|----------|-------|
| `transport.tls_version` | `"1.3"` | `"1.3"` | TLS 1.2 not supported |
| `transport.cipher_suites` | `["aes-256-gcm", "chacha20-poly1305"]` | Same | AES-128-GCM only for LAN |
| `auth.mfa_required` | `false` | `true` | |
| `auth.mfa_remember_device_days` | `30` | `0` | Always require MFA in hardened mode |
| `session.max_idle_sec` | `3600` | `900` | |
| `session.max_duration_sec` | `86400` | `28800` | 8-hour workday |
| `clipboard.enabled` | `true` | `false` or per-user | |
| `usb.enabled` | `false` | `false` | |
| `file_transfer.enabled` | `true` | `false` or per-user | |
| `crash.coredump_enabled` | `true` | `false` | No coredumps in high-security |
| `plugins.require_signatures` | `false` | `true` | |
| `plugins.allowed_plugins` | `[]` (all) | Explicit allowlist | |
| `audit.min_severity` | `"info"` | `"debug"` | Full audit trail |
| `login_screen.show_server_info` | `true` | `false` | Don't reveal server version |
| `login_screen.show_power_menu` | `false` | `false` | |

---

## 8) Test Plan

### Threat Model Validation
- Verify TLS 1.3 is enforced; connections offering only TLS 1.2 are rejected.
- Verify session tokens cannot be replayed after revocation.
- Verify failed login timing is constant regardless of username existence.
- Verify clipboard policy blocks transfer when direction policy is violated.
- Verify USB device rejection for disallowed device classes.
- Verify plugin signing rejection when `require_signatures = true` and signature is invalid.
- Verify crash reports contain no clipboard contents, no screen buffers, no environment variables.
- Verify coredumps are stored with `0600` permissions.
- Verify audit events are emitted for all catalogued event types.
- Verify audit log format matches the schema (version, timestamp, event_type, etc.).
- Verify fail2ban jails trigger IP bans after configured threshold.
- Verify cgroup limits prevent session resource exhaustion from affecting other sessions.
- Verify WASM plugin fuel exhaustion traps at sandbox boundary without session impact.
- Verify gateway mTLS rejects connections with untrusted certificates.
- Verify plugin cannot access another plugin's storage namespace.
- Verify Wayland client buffer overrun is caught by compositor bounds check.
