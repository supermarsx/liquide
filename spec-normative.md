# LiquiDE — Normative Conventions & Governance

> **Status**: Living document
> **Related specs**: [Main Spec](spec.md) · [Protocol](spec-protocol-formal.md) · [Client](spec-client.md) · [Gateway](spec-gateway.md) · [Design Language](spec-design.md)

---

## 1) Purpose

This document defines the shared terminology, trust model, normative language rules, versioning contract, and architectural decision record (ADR) index that apply across **all** LiquiDE specification documents. Every other spec implicitly imports this document.

---

## 2) Terminology

The following terms have precise meanings throughout the LiquiDE specification suite.

### Core Components

| Term | Definition |
|------|-----------|
| **LiquiDE** | The server-side desktop environment: compositor, shell, session manager, policy engine, plugin host, and all supporting services. |
| **LiquidClient** | The native cross-platform client application that connects to a LiquiDE server and presents the remote desktop to the user. Binary name: `liquidclient`. |
| **Gateway** | An optional reverse-proxy/load-balancer (`liquid-gateway`) that terminates TLS, authenticates clients, and routes connections to one or more LiquiDE server instances. See [spec-gateway.md](spec-gateway.md). |
| **Manager** | The web-based management UI (`liquid-manager`) for fleet administration. See [spec-manager.md](spec-manager.md). |
| **liquidctl** | The command-line administration tool for LiquiDE servers. See [spec-liquidctl.md](spec-liquidctl.md). |
| **liquid-desktopd** | The server supervisor daemon. Manages `liquid-session` child processes, monitors health, enforces cgroup limits, handles crash recovery. |
| **liquid-session** | A per-user session process spawned by `liquid-desktopd`. Contains the compositor, shell, plugin host, and all worker threads for one user session. |

### Session & Connection

| Term | Definition |
|------|-----------|
| **Session** | A logical user desktop instance managed by `liquid-session`. A session has an identity (session ID), a lifecycle (Created → Running → Suspended → Terminated), virtual monitors, running applications, and persistent state. Sessions can outlive connections. |
| **Connection** | A network-level link between a LiquidClient instance and a LiquiDE server (or gateway). A connection carries one session's traffic. Connections are ephemeral — they may drop and reconnect without destroying the session. |
| **Channel** | A multiplexed logical stream within a connection. Each channel carries a specific data type (video, audio, input, clipboard, etc.) and has its own reliability/ordering semantics. See [spec-protocol-formal.md](spec-protocol-formal.md). |
| **Transport** | The underlying network protocol carrying channels: QUIC, TCP+TLS, UDP, or WebRTC. Transports can be switched or hybridized mid-session. |

### Architecture

| Term | Definition |
|------|-----------|
| **Compositor** | The Wayland-compatible compositing engine within `liquid-session`. Manages surfaces, applies effects (blur, shadows, translucency), produces damage-tracked frames. |
| **Shell** | The desktop shell UI rendered by the compositor: dock, status bar, launcher, notification center, overview, lock screen. |
| **Policy Engine** | The subsystem that evaluates hierarchical policy rules (server → group → user → session) to determine effective permissions and limits. |
| **Plugin** | A WASM module that extends LiquiDE through one or more of the nine extension points. Runs in a sandboxed wasmtime instance with resource limits. |
| **Plugin Host** | The wasmtime-based runtime within `liquid-session` that loads, executes, and isolates plugins. |
| **Supervisor** | Synonym for `liquid-desktopd` when referring to its process-management role (spawning, monitoring, restarting sessions). |

### Rendering & Encoding

| Term | Definition |
|------|-----------|
| **Frame** | A single compositor output representing the current state of all visible surfaces. Frames are produced at a target cadence (e.g., 60 fps) but only when damage exists. |
| **Damage** | The set of pixel regions that changed between consecutive frames. Damage tracking occurs at surface, tile, and pixel levels. |
| **Tile** | A fixed-size rectangular block (default 64×64 px) used for bitmap-mode encoding. Tiles are the unit of delta comparison, encoding, and transmission. |
| **Mode A (Video Stream)** | Encoding mode where damaged regions are encoded as video codec frames (H.264, H.265, AV1, etc.). Suitable for motion-heavy content. |
| **Mode B (Tile/Bitmap Stream)** | Encoding mode where the screen is divided into tiles, each independently encoded with delta detection (skip, XOR delta, full, copy, solid fill). Suitable for mostly-static content with scattered changes. |
| **Mode C (Client-Side Render Offload)** | Encoding mode where the server sends structured render commands (scene graph, text runs, vector paths) instead of pixels. The client renders locally. Suitable for text-heavy and UI-heavy content. |
| **Adaptive Switching** | The server's continuous process of selecting Mode A, B, or C (or combinations) per screen region based on content analysis, network conditions, and client capabilities. |

### Security

| Term | Definition |
|------|-----------|
| **Trust Boundary** | A point where data crosses between components with different privilege levels. Every trust boundary requires authentication, authorization, input validation, and potentially encryption. |
| **mTLS** | Mutual TLS — both client and server present certificates. Used for client certificate authentication and inter-component communication. |
| **Session Jail** | A set of OS-level isolation mechanisms (cgroup v2, namespaces, seccomp, landlock) applied to each `liquid-session` process. |
| **WASM Sandbox** | The wasmtime execution environment for a plugin: isolated linear memory, CPU fuel metering, no ambient host access. |

### Policy

| Term | Definition |
|------|-----------|
| **Policy Source** | A layer in the policy hierarchy: `server`, `group`, `user`, or `session`. |
| **Effective Policy** | The computed result of merging all applicable policy sources for a given session, after applying precedence and conflict resolution rules. |
| **Policy Key** | A dot-separated identifier for a single policy setting (e.g., `clipboard.enabled`, `audio.direction`). |

---

## 3) Trust Boundaries

The following diagram shows all trust boundaries in a LiquiDE deployment. Each boundary line represents a point where data must be validated, authenticated, and (where applicable) encrypted.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         UNTRUSTED NETWORK                              │
│                                                                        │
│   ┌────────────────┐          TLS 1.3           ┌────────────────────┐ │
│   │  LiquidClient  │ ◄════════════════════════► │    Gateway         │ │
│   │  (user device)  │    ══ Trust Boundary 1 ══  │  (liquid-gateway)  │ │
│   └────────────────┘                             └────────┬───────────┘ │
│                                                           │             │
│                         ══ Trust Boundary 2 ══            │ mTLS /      │
│                         (Gateway ↔ Server)                │ unix sock   │
│                                                           ▼             │
│   ┌───────────────────────────────────────────────────────────────────┐ │
│   │                    SERVER HOST (trusted)                          │ │
│   │                                                                   │ │
│   │   ┌──────────────────┐     ══ Trust Boundary 3 ══               │ │
│   │   │  liquid-desktopd │     (supervisor ↔ session)                │ │
│   │   │   (supervisor)   │──────────┐                                │ │
│   │   └──────────────────┘          │ fork + cgroup + namespace      │ │
│   │                                  ▼                                │ │
│   │   ┌──────────────────────────────────────────────────────┐       │ │
│   │   │  liquid-session (per-user jail)                      │       │ │
│   │   │                                                      │       │ │
│   │   │  ┌────────────┐  ┌────────────┐  ┌───────────────┐  │       │ │
│   │   │  │ Compositor  │  │   Shell    │  │ Policy Engine │  │       │ │
│   │   │  └────────────┘  └────────────┘  └───────────────┘  │       │ │
│   │   │                                                      │       │ │
│   │   │  ┌─────────────────────────────────────────────┐     │       │ │
│   │   │  │  Plugin Host (wasmtime)                     │     │       │ │
│   │   │  │  ┌──────────┐ ┌──────────┐ ┌──────────┐    │     │       │ │
│   │   │  │  │ Plugin A │ │ Plugin B │ │ Plugin C │    │     │       │ │
│   │   │  │  │ (sandbox)│ │ (sandbox)│ │ (sandbox)│    │     │       │ │
│   │   │  │  └──────────┘ └──────────┘ └──────────┘    │     │       │ │
│   │   │  │  ══ Trust Boundary 4 (host ↔ plugin) ══    │     │       │ │
│   │   │  └─────────────────────────────────────────────┘     │       │ │
│   │   │                                                      │       │ │
│   │   │  ┌──────────────────────────────────────────┐        │       │ │
│   │   │  │  Wayland Clients (user applications)     │        │       │ │
│   │   │  │  ══ Trust Boundary 5 (compositor ↔ app) ══       │       │ │
│   │   │  └──────────────────────────────────────────┘        │       │ │
│   │   └──────────────────────────────────────────────────────┘       │ │
│   │                                                                   │ │
│   │   ┌──────────────────┐                                            │ │
│   │   │  liquid-manager  │ ══ Trust Boundary 6 ══                    │ │
│   │   │  (web UI)        │ (admin browser ↔ manager API, HTTPS+auth) │ │
│   │   └──────────────────┘                                            │ │
│   └───────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────┘
```

### Trust Boundary Details

| # | Boundary | Transport | Authentication | Encryption | Validation |
|---|----------|-----------|---------------|------------|------------|
| 1 | Client ↔ Gateway | QUIC / TCP | Password, MFA, mTLS, OIDC | TLS 1.3 (AES-256-GCM or ChaCha20-Poly1305) | Protocol message schema validation, size limits, rate limiting |
| 2 | Gateway ↔ Server | Unix socket or mTLS over TCP | Pre-shared token or mTLS certificate | TLS 1.3 (if TCP) or OS-level (if unix socket) | Gateway-injected auth headers validated by server |
| 3 | Supervisor ↔ Session | Unix IPC (pipe / socketpair) | Implicit (parent-child fork) | Not encrypted (same host, process isolation via cgroup/namespace) | Heartbeat protocol, exit code classification |
| 4 | Plugin Host ↔ Plugin | WASM ABI function calls | Manifest signature verification at load time | N/A (same process, isolated linear memory) | All host function arguments validated. Return values bounds-checked. Fuel + memory limits enforced. |
| 5 | Compositor ↔ Application | Wayland protocol (unix socket) | Session UID match (SO_PEERCRED) | N/A (same host) | Wayland protocol validation, buffer size limits, request rate limiting |
| 6 | Admin ↔ Manager | HTTPS | Username/password + MFA, or OIDC SSO | TLS 1.3 | CSRF tokens, input sanitization, RBAC authorization |

### Data Flow at Each Boundary

- **Boundary 1 (Client ↔ Gateway/Server)**: All LiquiDE protocol channels (video, audio, input, clipboard, cursor, tile, control). Untrusted — all input MUST be validated.
- **Boundary 2 (Gateway ↔ Server)**: Proxied client traffic plus gateway-injected metadata (authenticated user, client IP, connection ID). Server validates gateway identity and auth claims.
- **Boundary 3 (Supervisor ↔ Session)**: Heartbeat pings, configuration delivery, shutdown signals, crash context capture. Low-frequency control traffic.
- **Boundary 4 (Host ↔ Plugin)**: Host function calls with structured arguments. Every argument is validated. Plugins cannot access host memory directly. CPU and memory limits enforced by wasmtime.
- **Boundary 5 (Compositor ↔ App)**: Wayland protocol messages and shared-memory buffers. Compositor validates all messages and rejects malformed input. Buffer overruns are prevented by SHM pool bounds checking.
- **Boundary 6 (Admin ↔ Manager)**: REST API calls over HTTPS. All endpoints require authentication. Destructive operations require elevated privilege.

---

## 4) Normative Language

This specification suite uses requirement-level keywords as defined in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) and [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174).

### Keywords

| Keyword | Meaning |
|---------|---------|
| **MUST** / **MUST NOT** | Absolute requirement or prohibition. An implementation that fails to satisfy a MUST is non-conformant. |
| **SHALL** / **SHALL NOT** | Synonymous with MUST / MUST NOT. |
| **REQUIRED** | The item is an absolute requirement of the specification. |
| **SHOULD** / **SHOULD NOT** | There may exist valid reasons in particular circumstances to ignore this requirement, but the full implications must be understood and carefully weighed before doing so. |
| **RECOMMENDED** | Synonymous with SHOULD. |
| **MAY** | The item is truly optional. An implementation may include or omit the feature. |
| **OPTIONAL** | Synonymous with MAY. |

### Usage Rules

1. **Capitalization**: These keywords are normative **only** when they appear in **ALL CAPS**. Lowercase uses (e.g., "the server must handle...") are descriptive prose, not requirements.
2. **Scope**: Normative keywords apply to conformance targets. Each spec section identifies its target: server, client, gateway, plugin, or management tool.
3. **Informative text**: Sections explicitly marked "> **Informative**" or placed in blockquotes contain explanatory guidance that carries no conformance requirement.
4. **Examples**: Code examples and TOML configuration blocks illustrate defaults and typical usage. Unless a normative keyword appears in the surrounding prose, examples are informative.
5. **Diagrams**: ASCII diagrams and wireframes are illustrative. The normative text adjacent to them governs behavior.

### Conformance Targets

| Target | Identifier | Description |
|--------|-----------|-------------|
| Server | `[S]` | The LiquiDE server (`liquid-desktopd` + `liquid-session`). |
| Client | `[C]` | The LiquidClient application. |
| Gateway | `[G]` | The `liquid-gateway` reverse proxy. |
| Plugin | `[P]` | A WASM plugin conforming to the plugin ABI. |
| Management Tool | `[M]` | The `liquid-manager` web UI and `liquidctl` CLI. |
| Protocol | `[PROTO]` | The LiquiDE wire protocol as defined in [spec-protocol-formal.md](spec-protocol-formal.md). |

---

## 5) Versioning & Compatibility Contract

### Specification Versioning

The specification suite follows **calendar versioning** in the format `YYYY.MM.patch`:

| Component | Meaning |
|-----------|---------|
| `YYYY` | Year of release |
| `MM` | Month of release (zero-padded) |
| `patch` | Incremental patch within the same month (starts at 0) |

Example: `2025.06.0` → first release of June 2025; `2025.06.1` → a correction to that release.

### Software Versioning

LiquiDE software components (server, client, gateway, liquidctl, manager) follow **Semantic Versioning 2.0.0** (`MAJOR.MINOR.PATCH`):

| Level | Guarantees |
|-------|-----------|
| **MAJOR** (X.0.0) | Breaking changes to the wire protocol, configuration format, or public API. Requires migration. |
| **MINOR** (0.X.0) | New features, new protocol channels, new configuration keys. Backwards-compatible with the same MAJOR. Clients/servers with the same MAJOR MUST interoperate (new features degrade gracefully on older peers). |
| **PATCH** (0.0.X) | Bug fixes, security patches, performance improvements. No functional changes. Drop-in replacement. |

### Pre-1.0 Exception

While version is `0.x.y`, MINOR bumps MAY contain breaking changes. Once `1.0.0` is released, the full semver contract takes effect.

### Wire Protocol Compatibility

1. **Protocol version negotiation**: During connection handshake, client and server exchange their protocol version. The highest mutually supported version is used.
2. **Forward compatibility**: Receivers MUST ignore unknown message types and unknown fields in CBOR structures (see §4 of spec-protocol-formal.md). This allows newer senders to include new data without breaking older receivers.
3. **Backward compatibility**: Senders MUST NOT require receivers to understand messages or fields introduced after the negotiated protocol version.
4. **Channel capability advertisement**: During handshake, both sides advertise supported channels. Unsupported channels are simply not opened — the session degrades gracefully.
5. **Minimum supported version**: The server MAY configure a minimum client protocol version via `min_client_version` in `server.toml`. Connections from older clients are rejected with a descriptive error.

### Plugin ABI Compatibility

| Rule | Description |
|------|-------------|
| **ABI version independence** | Each ABI version (v1, v2, ...) is a self-contained contract. |
| **Additive changes within an ABI** | New host functions MAY be added to an existing ABI version. Plugins that don't call them are unaffected. |
| **Breaking changes require new ABI** | Removing or changing the signature of a host function creates a new ABI version. |
| **Deprecation runway** | A deprecated ABI version MUST remain supported for at least **2 major releases** of LiquiDE (or 12 months, whichever is longer). |
| **Runtime multi-version support** | The plugin host MUST support loading plugins targeting different ABI versions concurrently. |

### Configuration Compatibility

1. **Unknown keys are ignored**: New configuration keys MAY be added in any release. Older software MUST ignore keys it does not recognize (warning in log, not an error).
2. **Key removal**: Removing a configuration key is a MAJOR version change. Deprecated keys SHOULD be accepted (with a deprecation warning) for at least one MAJOR version.
3. **Default values**: Every configuration key MUST have a documented default. Omitting a key is valid and uses the default.

### Deprecation Policy

| Phase | Duration | Behavior |
|-------|----------|----------|
| **Active** | Current | Feature is fully supported. |
| **Deprecated** | ≥ 1 MAJOR release or 12 months | Feature works but emits a deprecation warning in logs. Documentation marks it as deprecated with migration guidance. |
| **Removed** | After deprecation period | Feature is removed. Using it produces an error with a pointer to the replacement. |

---

## 6) Architecture Decision Records (ADR) Index

ADRs document significant architectural decisions, their context, options considered, and rationale. Each ADR is immutable once accepted — superseding decisions create new ADRs that reference the original.

### ADR Format

Each ADR follows this structure:

```
# ADR-NNNN: Title

**Status**: Proposed | Accepted | Deprecated | Superseded by ADR-XXXX
**Date**: YYYY-MM-DD
**Deciders**: [names or roles]

## Context
[What is the issue? What forces are at play?]

## Decision
[What was decided?]

## Consequences
[What are the positive, negative, and neutral outcomes?]

## Alternatives Considered
[What was rejected and why?]
```

### ADR Index

| ADR | Title | Status | Summary |
|-----|-------|--------|---------|
| ADR-0001 | Rust as sole implementation language | Accepted | Memory safety, performance, and single-language simplicity outweigh ecosystem maturity concerns. |
| ADR-0002 | Custom compositor over existing (wlroots, smithay) | Accepted | Full control of rendering pipeline needed for remote-optimized damage tracking and encoding integration. Existing compositors optimize for local display. |
| ADR-0003 | QUIC as primary transport | Accepted | Multiplexed streams, 0-RTT reconnect, built-in encryption. UDP fallback and TCP fallback for restricted networks. |
| ADR-0004 | CBOR for control messages, not Protobuf/FlatBuffers | Accepted | Schema-free evolution, compact encoding, first-class Rust support (ciborium). Forward-compatible by design (unknown fields ignored). |
| ADR-0005 | CSS-driven theming engine | Accepted | Web developers can contribute themes. Declarative styling separates design from code. Hot-reload without restart. |
| ADR-0006 | wasmtime for plugin runtime | Accepted | Rust-native, fuel-based CPU metering, memory isolation, async support, active maintenance, WASI preview 2. |
| ADR-0007 | Supervisor process model (liquid-desktopd + liquid-session) | Accepted | Session crash isolation — one user's crash does not affect others. Enables cgroup enforcement, clean restart, and crash diagnostics. |
| ADR-0008 | liquid-ui for built-in apps (not GTK) | Accepted | Built-in apps render directly to Wayland surfaces via a Rust-native toolkit, avoiding GTK overhead and ensuring visual consistency with the compositor theme. |
| ADR-0009 | Flatpak as primary third-party app mechanism | Accepted | Sandboxing, portal routing, Flathub ecosystem, runtime deduplication. Avoids distro-specific package management for user-facing apps. |
| ADR-0010 | Adaptive bitmap delta encoding for Mode B | Accepted | XOR-diff between consecutive tiles before compression yields 60-90% bandwidth savings for mostly-static content. Five encoding strategies (skip, delta, full, copy, solid) selected per-tile. |
| ADR-0011 | Three-mode adaptive encoding (Video / Tile / Client-Side) | Accepted | No single encoding strategy is optimal for all content. Per-region mode selection enables best quality-bandwidth tradeoff. |
| ADR-0012 | Gateway as optional component | Accepted | Single-server deployments should not require a gateway. Gateway adds TLS termination, load balancing, and multi-server routing for scale-out. |

> **Note**: ADRs are maintained alongside the specification. New ADRs are added as architectural decisions are made during implementation.

---

## 7) Cross-Reference Conventions

### Spec File Index

| File | Scope |
|------|-------|
| [spec.md](spec.md) | Main server & DE specification |
| [spec-client.md](spec-client.md) | LiquidClient specification |
| [spec-protocol-formal.md](spec-protocol-formal.md) | Wire protocol, channels, CBOR schemas, state machines |
| [spec-gateway.md](spec-gateway.md) | Gateway reverse proxy & load balancer |
| [spec-manager.md](spec-manager.md) | Web management UI |
| [spec-liquidctl.md](spec-liquidctl.md) | CLI administration tool |
| [spec-design.md](spec-design.md) | Liquid Glass design language & CSS |
| [spec-settings.md](spec-settings.md) | Settings application & configuration UI |
| [spec-addons.md](spec-addons.md) | Built-in applications |
| [spec-interop.md](spec-interop.md) | OS integration, portals, Flatpak, D-Bus |
| [spec-system.md](spec-system.md) | Packaging, installation, system integration |
| [spec-updates.md](spec-updates.md) | Update mechanism & release lifecycle |
| [spec-accessibility.md](spec-accessibility.md) | Accessibility features |
| [spec-normative.md](spec-normative.md) | This document — conventions & governance |
| [spec-threat-model.md](spec-threat-model.md) | Threat model, STRIDE analysis, key lifecycle |
| [spec-rendering-software.md](spec-rendering-software.md) | Software rendering pipeline & compositor contract |
| [spec-performance.md](spec-performance.md) | SLOs, benchmarks, CI performance gating |
| [spec-observability.md](spec-observability.md) | Metrics, traces, logs, runbooks |
| [spec-build.md](spec-build.md) | Repository layout, build system, CI |
| [spec-theme-night.md](spec-theme-night.md) | Night theme CSS variables |
| [spec-theme-sunset.md](spec-theme-sunset.md) | Sunset theme CSS variables |
| [spec-theme-midday.md](spec-theme-midday.md) | Midday theme CSS variables |

### Internal Cross-Reference Format

When referencing another section within the spec suite, use the format:

- **Same file**: `§N` (e.g., "see §13 Session Supervisor")
- **Different file**: `[spec-file.md §N]` (e.g., "see [spec-protocol-formal.md §5.4] Tile Channel Messages")

---

## 8) Document Conventions

### Configuration Examples

All TOML configuration examples show **default values** unless explicitly noted. Commented-out keys indicate optional settings. Example:

```toml
[section]
required_key = "default_value"        # always present
# optional_key = "example"            # omitted by default
```

### ASCII Diagrams

Diagrams use box-drawing characters for structure and arrows for data flow:
- `→` / `←` / `↑` / `↓` — data flow direction
- `══` — trust boundary crossing
- `├──` / `└──` — tree hierarchy
- `◄════►` — bidirectional encrypted link

### Units

| Quantity | Unit | Notes |
|----------|------|-------|
| Time (latency, timeout) | Milliseconds (ms) | Unless otherwise specified |
| Time (duration, uptime) | Seconds (s) | |
| Bandwidth | Megabits per second (Mbps) | |
| Memory / disk | Megabytes (MB) or Gigabytes (GB) | Powers of 1024 (MiB) when referring to OS memory; powers of 1000 (MB) when referring to network data volumes |
| Resolution | Pixels (px) | Width × Height |
| Frame rate | Frames per second (fps) | |
| CPU usage | Percentage of one core (%) | 100% = one core fully utilized |

---

## 9) Test Plan

### Normative Compliance
- Verify all spec documents use RFC 2119 keywords only in ALL CAPS.
- Verify configuraton files accept and ignore unknown keys without error.
- Verify protocol handshake includes version negotiation.
- Verify minimum client version rejection produces descriptive error.
- Verify deprecated features emit log warnings.
- Verify plugin ABI version check rejects incompatible plugins with clear error message.
