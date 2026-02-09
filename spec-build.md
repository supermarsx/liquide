# LiquiDE — Repository Layout, Build System & CI

> **Status**: Living document
> **Related specs**: [Main Spec](spec.md) · [Performance](spec-performance.md) · [Normative Conventions](spec-normative.md)

---

## 1) Purpose

This document defines the monorepo layout, crate structure, build matrix, dependency policy, shared protocol crate, test harness location, and CI pipeline for the LiquiDE project.

---

## 2) Repository Layout

LiquiDE uses a **Cargo workspace monorepo**. All components share a single repository, enabling atomic cross-component changes and unified versioning.

```
liquide/
├── Cargo.toml                      # workspace root
├── Cargo.lock                      # single lockfile for all crates
├── rust-toolchain.toml             # pinned Rust toolchain version
├── .cargo/
│   └── config.toml                 # build profiles, target configs, linker settings
│
├── crates/
│   ├── liquide-protocol/           # shared protocol definitions (wire format, CBOR schemas, message types)
│   ├── liquide-transport/          # transport layer (QUIC, TCP, UDP, WebSocket, WebRTC)
│   ├── liquide-crypto/             # TLS, certificate management, token generation
│   ├── liquide-auth/               # authentication (PAM, LDAP, OIDC, MFA, certificate auth)
│   ├── liquide-policy/             # policy engine (hierarchy, resolution, evaluation)
│   ├── liquide-compositor/         # Wayland compositor, scene graph, damage tracking
│   ├── liquide-renderer-cpu/       # CPU software rasterizer (SIMD paths)
│   ├── liquide-renderer-gpu/       # GPU compute path (Vulkan)
│   ├── liquide-encoder/            # video/tile encoding (H.264, H.265, AV1, tile codecs; 10-bit profiles for H.265 Main 10 / AV1 / VP9 Profile 2)
│   ├── liquide-encoder-hw/         # hardware encoder backends (VAAPI, NVENC, AMF, V4L2; 10-bit encode support where hardware permits)
│   ├── liquide-shell/              # desktop shell (dock, status bar, launcher, notifications, overview)
│   ├── liquide-css/                # CSS parser and theming engine
│   ├── liquide-plugin-host/        # WASM plugin runtime (wasmtime integration)
│   ├── liquide-plugin-abi/         # plugin ABI definitions (shared between host and plugins)
│   ├── liquide-session/            # session process binary (liquid-session)
│   ├── liquide-supervisor/         # supervisor daemon binary (liquid-desktopd)
│   ├── liquide-gateway/            # gateway binary (liquid-gateway)
│   ├── liquide-client/             # client binary (liquidclient)
│   ├── liquide-client-renderer/    # client-side rendering (wgpu/softbuffer)
│   ├── liquide-mobile-core/        # shared Rust library for mobile clients (iOS + Android)
│   ├── liquide-recording/          # session recording engine (.lqr format, muxer, storage backends)
│   ├── liquide-assistance/         # remote assistance / shadow session engine
│   ├── liquide-manager/            # management web UI backend
│   ├── liquide-manager-frontend/   # management web UI frontend (static assets)
│   ├── liquide-ctl/                # CLI tool binary (liquidctl)
│   ├── liquide-bench/              # benchmark harness binary (liquide-bench)
│   ├── liquide-conformance/        # protocol conformance test runner
│   ├── liquide-ui/                 # liquid-ui: Rust-native UI toolkit for built-in apps
│   ├── liquide-apps-terminal/      # built-in Terminal app
│   ├── liquide-apps-files/         # built-in File Manager app
│   ├── liquide-apps-settings/      # built-in Settings app
│   ├── liquide-apps-text-editor/   # built-in Text Editor app
│   ├── liquide-apps-software-center/ # built-in Software Center app
│   ├── liquide-interop/            # OS integration (portals, D-Bus, Flatpak, XDG)
│   ├── liquide-audio/              # audio pipeline (PipeWire integration, mixing)
│   ├── liquide-clipboard/          # clipboard manager (Wayland clipboard + remote sync)
│   ├── liquide-usb/                # USB/IP device redirection
│   ├── liquide-input/              # input processing (keyboard layouts, touch, gestures)
│   ├── liquide-a11y/               # accessibility (AT-SPI2 bridge, screen reader support)
│   └── liquide-common/             # shared utilities (logging, config parsing, error types)
│
├── tests/
│   ├── integration/                # cross-crate integration tests
│   │   ├── session_lifecycle.rs
│   │   ├── protocol_round_trip.rs
│   │   ├── clipboard_sync.rs
│   │   ├── tile_encoding.rs
│   │   ├── plugin_sandbox.rs
│   │   └── ...
│   ├── e2e/                        # end-to-end tests (full server + client)
│   │   ├── connect_disconnect.rs
│   │   ├── resize_storm.rs
│   │   ├── transport_switching.rs
│   │   └── ...
│   └── fuzz/                       # fuzz targets (cargo-fuzz)
│       ├── fuzz_frame_parser/
│       ├── fuzz_cbor_decoder/
│       ├── fuzz_video_decoder/
│       └── ...
│
├── benches/                        # criterion benchmarks
│   ├── compositor_render.rs
│   ├── tile_encode.rs
│   ├── blur_simd.rs
│   ├── transport_throughput.rs
│   └── ...
│
├── specs/                          # specification documents (this repo)
│   ├── spec.md
│   ├── spec-client.md
│   ├── spec-protocol-formal.md
│   ├── spec-gateway.md
│   ├── spec-normative.md
│   └── ... (all spec-*.md files)
│
├── assets/                         # static assets
│   ├── icons/                      # app icons, shell icons, tray icons
│   ├── cursors/                    # cursor themes
│   ├── themes/                     # CSS themes (night, sunset, midday)
│   ├── fonts/                      # bundled fonts (if any)
│   ├── sounds/                     # notification sounds
│   └── wallpapers/                 # default wallpapers
│
├── packaging/                      # distribution packaging
│   ├── deb/                        # Debian/Ubuntu packaging
│   ├── rpm/                        # Fedora/RHEL packaging
│   ├── arch/                       # Arch Linux PKGBUILD
│   ├── flatpak/                    # Flatpak manifest (for client)
│   ├── docker/                     # Dockerfile / OCI container build
│   ├── systemd/                    # systemd unit files
│   └── fail2ban/                   # fail2ban jail configs
│
├── tools/                          # development tools and scripts
│   ├── dev-setup.sh                # one-shot dev environment setup
│   ├── gen-protocol.sh             # regenerate protocol constants from spec
│   ├── gen-theme-vars.sh           # extract CSS custom properties from spec
│   └── release.sh                  # build + package release artifacts
│
├── .github/
│   └── workflows/
│       ├── ci.yml                  # main CI pipeline
│       ├── nightly.yml             # nightly builds + extended tests
│       ├── release.yml             # release build + packaging
│       └── security.yml            # dependency audit + SAST
│
└── .gitignore
```

---

## 3) Crate Dependency Graph

The crate dependency graph flows from shared libraries to binaries:

```
                    liquide-common
                   /      |       \
          liquide-protocol  liquide-crypto  liquide-policy
             /    |    \         |              |
    liquide-transport   liquide-auth      liquide-css
         |        |           |              |
    liquide-compositor ◄─ liquide-shell ◄─ liquide-renderer-cpu
         |        |           |              |
    liquide-encoder     liquide-plugin-host  liquide-renderer-gpu
         |        |           |
    liquide-encoder-hw  liquide-plugin-abi
         |
         ▼
    ┌────────────────┐  ┌────────────────┐  ┌──────────────┐
    │liquide-session │  │liquide-supervisor│ │liquide-gateway│
    │ (binary)       │  │ (binary)        │ │ (binary)      │
    └────────────────┘  └────────────────┘  └──────────────┘

    ┌────────────────┐  ┌────────────────┐  ┌──────────────┐
    │liquide-client  │  │liquide-ctl     │  │liquide-manager│
    │ (binary)       │  │ (binary)        │ │ (binary)      │
    └────────────────┘  └────────────────┘  └──────────────┘

    ┌──────────────────┐  ┌───────────────────┐  ┌──────────────────┐
    │liquide-mobile-   │  │liquide-recording  │  │liquide-assistance│
    │core (lib, iOS/   │  │ (library)         │  │ (library)        │
    │Android)          │  │                   │  │                  │
    └──────────────────┘  └───────────────────┘  └──────────────────┘
```

### Key Shared Crate: `liquide-protocol`

This crate contains:
- All protocol message type codes and constants.
- CBOR encode/decode implementations for all message types.
- Channel ID definitions.
- Protocol version constants.
- Shared error types for protocol operations.

Both server and client depend on this crate, ensuring protocol compatibility is enforced at compile time.

---

## 4) Build Matrix

### 4.1 Server Targets

| Target Triple | Tier | Notes |
|--------------|------|-------|
| `x86_64-unknown-linux-gnu` | 1 (primary) | Primary development target. Full CI. |
| `aarch64-unknown-linux-gnu` | 1 | ARM64 server support. Full CI via cross-compilation or ARM runners. |
| `x86_64-unknown-linux-musl` | 2 | Static binary for container/Alpine deployments. CI build + basic test. |
| `aarch64-unknown-linux-musl` | 2 | Static ARM64 binary. CI build. |

### 4.2 Client Targets

| Target Triple | Tier | Notes |
|--------------|------|-------|
| `x86_64-unknown-linux-gnu` | 1 | Linux client. Full CI. |
| `x86_64-pc-windows-msvc` | 1 | Windows client. Full CI. |
| `aarch64-apple-darwin` | 1 | macOS ARM64 client. Full CI. |
| `x86_64-apple-darwin` | 2 | macOS Intel client. CI build + basic test. |
| `aarch64-unknown-linux-gnu` | 2 | Linux ARM64 client. CI build. |
| `aarch64-pc-windows-msvc` | 2 | Windows ARM64 client. CI build. |

### 4.3 Mobile Client Targets

| Target Triple | Platform | Tier | Notes |
|--------------|----------|------|-------|
| `aarch64-apple-ios` | iOS / iPadOS | 1 | Mobile core library (.xcframework). Full CI. |
| `aarch64-apple-ios-sim` | iOS Simulator | 1 | Simulator build for CI testing. |
| `aarch64-linux-android` | Android ARM64 | 1 | Mobile core library (.so). Full CI. |
| `x86_64-linux-android` | Android x86_64 | 2 | Emulator build. CI smoke test. |

### 4.5 Tier Definitions

| Tier | Build in CI | Tests in CI | Release Artifact | Support Level |
|------|------------|------------|-----------------|---------------|
| **Tier 1** | Yes (every PR) | Full test suite | Yes | Fully supported, bugs are P1 |
| **Tier 2** | Yes (every PR) | Build + basic smoke test | Yes | Best-effort, bugs are P2 |
| **Tier 3** | Nightly only | Build only | No | Community-supported |

### 4.6 Build Profiles

```toml
# Cargo.toml workspace [profile] section

[profile.dev]
opt-level = 0
debug = true
# SIMD intrinsics still enabled (runtime detection)

[profile.release]
opt-level = 3
lto = "thin"                         # thin LTO for fast builds, good optimization
strip = "symbols"                    # strip debug symbols from binaries
codegen-units = 1                    # max optimization (slower build)
panic = "abort"                      # smaller binary, faster panics

[profile.release-debug]
inherits = "release"
debug = true                         # release perf + debug symbols (for profiling)
strip = "none"

[profile.bench]
inherits = "release"
debug = true                         # symbols for flamegraphs

[profile.fuzz]
inherits = "dev"
opt-level = 1                        # some optimization for fuzzing speed
```

---

## 5) Dependency Policy

### 5.1 Dependency Rules

| Rule | Description |
|------|-------------|
| **Minimize** | Prefer Rust standard library over external crates where feasible. Each dependency is a maintenance and supply-chain risk. |
| **Audit** | All dependencies must pass `cargo-audit` with no known vulnerabilities. CI blocks on advisory-db hits. |
| **License** | Only MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, and MPL-2.0 licenses accepted. No LGPL or GPL dependencies (copyleft). Exception: system libraries linked dynamically (FreeType, HarfBuzz, Fontconfig — these are LGPL but dynamically linked). |
| **Pin** | `Cargo.lock` is committed. All builds are reproducible from the lockfile. |
| **Review** | New dependencies require PR review. Reviewer checks: crate maturity, maintainer reputation, transitive dependency count, license. |
| **Minimum versions** | Where possible, use `>=` version constraints rather than `^` to avoid unnecessary breakage. |
| **No `unsafe` in deps** | Prefer crates that avoid `unsafe`, or that have been audited (listed in `cargo-crev`). `unsafe` in LiquiDE crates requires reviewer approval with a safety comment. |

### 5.2 Key Dependencies

| Crate | Purpose | Version Policy |
|-------|---------|---------------|
| `tokio` | Async runtime | Pin major version |
| `wasmtime` | WASM plugin runtime | Pin minor version (ABI-sensitive) |
| `quinn` / `s2n-quic` | QUIC implementation | Pin minor version |
| `rustls` | TLS library | Pin minor version |
| `ciborium` | CBOR encode/decode | Pin major version |
| `freetype-rs` / `harfbuzz-rs` | Font rendering | Pin major version |
| `wgpu` (client only) | GPU rendering (client) | Pin minor version |
| `zstd` | Zstd compression | Pin major version |
| `lz4_flex` | LZ4 compression | Pin major version |
| `tracing` | Structured logging | Pin major version |
| `prometheus-client` | Metrics export | Pin major version |
| `serde` / `toml` | Config parsing | Pin major version |
| `zbus` | D-Bus communication | Pin major version |
| `nix` | Linux syscall wrappers | Pin major version |
| `uniffi` | FFI bindings (mobile: Swift/Kotlin) | Pin minor version |
| `ash` | Vulkan bindings (GPU server mode) | Pin minor version |

### 5.3 `cargo-deny` Configuration

```toml
# deny.toml
[advisories]
vulnerability = "deny"
unmaintained = "warn"

[licenses]
allow = ["MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Zlib", "MPL-2.0", "Unicode-DFS-2016"]
copyleft = "deny"

[bans]
multiple-versions = "warn"
wildcards = "deny"
highlight = "all"

[sources]
unknown-registry = "deny"
unknown-git = "deny"
allow-registry = ["https://github.com/rust-lang/crates.io-index"]
```

---

## 6) CI Pipeline

### 6.1 PR Pipeline (`ci.yml`)

Runs on every pull request.

```
┌─────────────┐     ┌──────────────┐     ┌──────────────┐
│  Lint &      │     │   Build      │     │   Test        │
│  Format     │────►│  (all tier 1 │────►│  (unit +      │
│  (clippy,   │     │   targets)   │     │   integration) │
│   rustfmt)  │     │              │     │               │
└─────────────┘     └──────────────┘     └──────┬────────┘
                                                │
                    ┌──────────────┐     ┌──────▼────────┐
                    │  Dependency  │     │  Performance  │
                    │  Audit       │     │  (ci-quick    │
                    │  (cargo-deny,│     │   benchmark)  │
                    │   cargo-audit)│     │               │
                    └──────────────┘     └───────────────┘
```

| Step | Duration Target | Blocks PR |
|------|----------------|-----------|
| `cargo fmt --check` | <30s | Yes |
| `cargo clippy -- -D warnings` | <3min | Yes |
| `cargo build --release` (tier 1 targets) | <10min | Yes |
| `cargo test` (unit + integration) | <5min | Yes |
| `cargo deny check` | <30s | Yes |
| `cargo audit` | <30s | Yes |
| `liquide-bench --suite ci-quick` | <5min | Yes (on regression) |
| `cargo build` (tier 2 targets) | <10min | No (warning only) |

### 6.2 Merge Pipeline

Runs after merge to main.

| Step | Duration Target |
|------|----------------|
| Full build (all targets) | <15min |
| Full test suite (unit + integration + e2e) | <15min |
| `liquide-bench --suite ci-full` | <30min |
| Update performance baseline | — |

### 6.3 Nightly Pipeline (`nightly.yml`)

| Step | Duration Target |
|------|----------------|
| Full build (all targets including tier 3) | <20min |
| Full test suite | <15min |
| `liquide-bench --suite ci-nightly` | <2h |
| Fuzzing (1h per target) | <8h |
| Dependency update check (`cargo outdated`) | <1min |
| SAST scan (semgrep or similar) | <10min |
| License compliance check | <1min |

### 6.4 Release Pipeline (`release.yml`)

Triggered on git tag `v*`.

1. Full CI pipeline (all tests, all targets).
2. Performance benchmark (`ci-release` suite, all workloads, all network profiles).
3. Build release artifacts for all tier 1 + tier 2 targets.
4. Build distribution packages (deb, rpm, Arch, Docker, Flatpak).
5. Generate changelog from git commits.
6. Publish to release page with checksums (SHA-256).

---

## 7) Test Harness Location

| Test Type | Location | Runner |
|-----------|----------|--------|
| Unit tests | Inline in each crate (`#[cfg(test)]` modules) | `cargo test` |
| Integration tests | `tests/integration/` | `cargo test --test <name>` |
| End-to-end tests | `tests/e2e/` | `cargo test --test <name>` (requires full server + client setup) |
| Fuzz tests | `tests/fuzz/` | `cargo fuzz run <target>` |
| Benchmarks | `benches/` | `cargo bench` (criterion) |
| Performance benchmarks | `crates/liquide-bench/` | `liquide-bench` binary |
| Conformance tests | `crates/liquide-conformance/` | `liquide-conformance` binary |
| Spec tests | Per spec file (§ Test Plan sections) | Manual or automated (mapped to integration tests) |

---

## 8) Development Workflow

### 8.1 Getting Started

```bash
# Clone and setup
git clone https://github.com/liquide/liquide.git
cd liquide
./tools/dev-setup.sh           # installs Rust toolchain, system deps, pre-commit hooks

# Build everything
cargo build

# Run tests
cargo test

# Run a local session (dev mode)
cargo run --bin liquid-desktopd -- --dev-mode

# Connect with client
cargo run --bin liquidclient -- --server localhost:3389
```

### 8.2 Pre-commit Hooks

Installed by `dev-setup.sh`:

1. `cargo fmt --check` — format check.
2. `cargo clippy -- -D warnings` — lint check (workspace members only, skip build time for dependencies).
3. `cargo deny check licenses` — license check.

### 8.3 Branch Strategy

| Branch | Purpose | Protection |
|--------|---------|-----------|
| `main` | Stable development. All PRs merge here. | CI must pass. 1 approval required. |
| `release/X.Y` | Release branch for version X.Y. Cherry-picks from main. | CI must pass. 2 approvals required. |
| `feature/*` | Feature development branches. | None (developer's branch). |
| `fix/*` | Bug fix branches. | None. |

---

## 9) Test Plan

### Build System
- Verify `cargo build` succeeds for all tier 1 targets with a clean checkout.
- Verify `cargo test` passes for all crates.
- Verify `cargo clippy -- -D warnings` produces zero warnings.
- Verify `cargo fmt --check` produces no diffs.
- Verify `cargo deny check` passes.
- Verify `cargo audit` reports no vulnerabilities.
- Verify release profile produces stripped, optimized binaries.
- Verify cross-compilation for tier 2 targets succeeds.
- Verify `liquide-protocol` crate is a dependency of both server and client binaries.
- Verify all binaries are statically linked (musl targets) or have minimal dynamic deps (gnu targets).
