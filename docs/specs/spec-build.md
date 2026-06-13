# LiquiDE — Repository Layout, Build System & CI

> **Status**: Living document
> **Related specs**: [Main Spec](spec.md) · [Performance](spec-performance.md) · [Normative Conventions](spec-normative.md)

---

## 1) Purpose

This document defines the monorepo layout, crate structure, build matrix, dependency policy, shared protocol crate, test harness location, and CI pipeline for the LiquiDE project.

---

## 2) Repository Layout

LiquiDE uses a **Cargo workspace monorepo**. All components share a single repository, enabling atomic cross-component changes and unified versioning.

```text
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
│   ├── blur_simd.rs                # unregistered placeholder; SIMD blur is benchmarked in crate-local benches
│   ├── layout_cache.rs
│   ├── tile_encode.rs
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
│   ├── deb/                        # Debian/Ubuntu packaging (.deb)
│   ├── rpm/                        # Fedora/RHEL packaging (.rpm)
│   ├── arch/                       # Arch Linux PKGBUILD
│   ├── flatpak/                    # Flatpak manifest (client only)
│   ├── snap/                       # Snapcraft recipe (client + server)
│   ├── brew/                       # Homebrew formula (macOS + Linux)
│   ├── nix/                        # Nix derivation + NixOS module
│   ├── appimage/                   # AppImage recipe (client only, Linux)
│   ├── dmg/                        # macOS .dmg + .pkg installer (client, macOS)
│   ├── docker/                     # Dockerfile / OCI container build (server)
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

```text
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

Target tiers are assigned per target triple. If the same target triple is used by more than one product role, it keeps one support tier and one release artifact identity. Current automated coverage is listed in [4.4 Current Workflow Coverage](#44-current-workflow-coverage); a tier does not imply that every workflow builds every target.

### 4.1 Server Targets

| Target Triple | Tier | Notes |
| ------------- | ---- | ----- |
| `x86_64-unknown-linux-gnu` | 1 (primary) | Primary development target. PR Linux release build, nightly build, and release artifact. |
| `aarch64-unknown-linux-gnu` | 1 | ARM64 Linux server target. Nightly and release cross-build. |
| `x86_64-unknown-linux-musl` | 2 | Static Linux binary for container/Alpine deployments. Nightly and release build. |
| `aarch64-unknown-linux-musl` | 2 | Static ARM64 Linux binary. Nightly and release cross-build. |

### 4.2 Client Targets

| Target Triple | Tier | Notes |
| ------------- | ---- | ----- |
| `x86_64-unknown-linux-gnu` | 1 | Linux client. Shares the Tier 1 Linux GNU release target. |
| `x86_64-pc-windows-msvc` | 1 | Windows client. Windows E2E runs in CI/nightly; release artifact is built on tag pushes. |
| `aarch64-apple-darwin` | 1 | macOS ARM64 client. Release artifact is built on tag pushes. No current PR/nightly matrix entry. |
| `x86_64-apple-darwin` | 2 | macOS Intel client. Release artifact is built on tag pushes. No current PR/nightly matrix entry. |
| `aarch64-unknown-linux-gnu` | 1 | Linux ARM64 client. Same target triple and support tier as the ARM64 Linux server target. |
| `aarch64-pc-windows-msvc` | 2 | Windows ARM64 client. Release artifact is built on tag pushes. No current PR/nightly matrix entry. |

### 4.3 Mobile Client Targets

Mobile target triples are product targets, but they are not in the current `ci.yml`, `nightly.yml`, or `release.yml` matrices.

| Target Triple | Platform | Current Workflow Coverage | Notes |
| ------------- | -------- | ------------------------- | ----- |
| `aarch64-apple-ios` | iOS / iPadOS | Not currently built in GitHub Actions | Mobile core library (.xcframework) target. |
| `aarch64-apple-ios-sim` | iOS Simulator | Not currently built in GitHub Actions | Simulator build target. |
| `aarch64-linux-android` | Android ARM64 | Not currently built in GitHub Actions | Mobile core library (.so) target. |
| `x86_64-linux-android` | Android x86_64 | Not currently built in GitHub Actions | Emulator build target. |

### 4.4 Current Workflow Coverage

| Workflow | Target Coverage | Test / Validation Coverage | Artifacts |
| -------- | --------------- | -------------------------- | --------- |
| `ci.yml` | Linux release build for `x86_64-unknown-linux-gnu`; Windows hosted E2E job. No current PR build for macOS, mobile, Windows release binaries, or Tier 2 targets. | Rustfmt, clippy, Ubuntu workspace tests, Ubuntu integration tests, Windows manifest E2E, `cargo deny check`. | Linux `liquid*` binaries and Windows E2E logs. |
| `nightly.yml` | Linux GNU builds for `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`; Linux musl builds for `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`; Windows hosted E2E job. | Ubuntu full workspace tests with all features, integration tests, doc tests, `cargo outdated`, `cargo audit`, `cargo deny check`. | Nightly artifacts are uploaded for Linux GNU targets and Windows E2E logs. |
| `release.yml` | Release matrix targets: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`, `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`, `x86_64-apple-darwin`, and `aarch64-pc-windows-msvc`. Each target appears once. | Ubuntu `cargo test --workspace --all-features` release test job. | One archive artifact per release target plus `SHA256SUMS.txt`. |

### 4.5 Tier Definitions

| Tier | Current Workflow Expectation | Release Artifact | Support Level |
| ---- | ---------------------------- | ---------------- | ------------- |
| **Tier 1** | Target is part of the configured release matrix. PR and nightly coverage are explicit in [4.4 Current Workflow Coverage](#44-current-workflow-coverage), not automatic for every Tier 1 target. | Yes, for desktop/server targets listed in `release.yml`. | Fully supported, bugs are P1. |
| **Tier 2** | Best-effort target. Built where explicitly listed in nightly or release workflows; not built on every PR unless the workflow matrix is widened. | Yes, for desktop/server targets listed in `release.yml`. | Best-effort, bugs are P2. |
| **Tier 3** | No current GitHub Actions matrix entry unless one is added explicitly. | No. | Community-supported or roadmap. |

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
| ---- | ----------- |
| **Minimize** | Prefer Rust standard library over external crates where feasible. Each dependency is a maintenance and supply-chain risk. |
| **Audit** | All dependencies must pass `cargo-audit` with no known vulnerabilities. CI blocks on advisory-db hits. |
| **License** | Only MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, and MPL-2.0 licenses accepted. No LGPL or GPL dependencies (copyleft). Exception: system libraries linked dynamically (FreeType, HarfBuzz, Fontconfig — these are LGPL but dynamically linked). |
| **Pin** | `Cargo.lock` is committed. All builds are reproducible from the lockfile. |
| **Review** | New dependencies require PR review. Reviewer checks: crate maturity, maintainer reputation, transitive dependency count, license. |
| **Minimum versions** | Where possible, use `>=` version constraints rather than `^` to avoid unnecessary breakage. |
| **No `unsafe` in deps** | Prefer crates that avoid `unsafe`, or that have been audited (listed in `cargo-crev`). `unsafe` in LiquiDE crates requires reviewer approval with a safety comment. |

### 5.2 Key Dependencies

| Crate | Purpose | Version Policy |
| ----- | ------- | -------------- |
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

Runs on pull requests to `main` / `release/**` and on pushes to `main`.

| Job | Host | Current Commands | Blocks PR |
| --- | ---- | ---------------- | --------- |
| `lint` | Ubuntu | `cargo fmt --all --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Yes |
| `build` | Ubuntu | `cargo build --release --target x86_64-unknown-linux-gnu` | Yes |
| `windows-e2e` | Windows | `pwsh -NoProfile -File scripts/e2e.ps1 -Suite check,apps,shell,session -CargoTargetDir ""` | Yes |
| `test` | Ubuntu | `cargo test --workspace`; `cargo test --workspace --test '*'` | Yes |
| `audit` | Ubuntu | `cargo deny check` | Yes |

The current PR pipeline does not build macOS targets, mobile targets, Windows release binaries, or Tier 2 targets.

### 6.2 Merge Pipeline

Pushes to `main` run the same `ci.yml` jobs listed above. There is no separate all-target merge workflow or merge-time benchmark workflow at present.

| Step | Current Coverage |
| ---- | ---------------- |
| Linux build | `x86_64-unknown-linux-gnu` release build. |
| Tests | Ubuntu workspace tests and integration tests, plus Windows manifest E2E. |
| Dependency policy | `cargo deny check`. |

### 6.3 Nightly Pipeline (`nightly.yml`)

| Job | Current Coverage |
| --- | ---------------- |
| `build-tier1` | Builds `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu` on Ubuntu. |
| `build-tier2` | Builds `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl` on Ubuntu. |
| `test-extended` | Runs `cargo test --workspace --all-features`, integration tests, and doc tests on Ubuntu. |
| `windows-e2e` | Runs the manifest E2E suite on Windows and uploads logs. |
| `outdated` | Runs `cargo outdated --workspace --exit-code 0`. |
| `audit` | Runs `cargo audit` and `cargo deny check`. |

Nightly does not currently build macOS, mobile, Windows release binaries, or Tier 3 targets. Fuzzing, benchmark suites, and SAST are roadmap items unless a dedicated workflow is added.

### 6.4 Release Pipeline (`release.yml`)

Triggered on git tag `v*`.

1. Run the Ubuntu release test job: `cargo test --workspace --all-features`.
2. Build one release artifact for each configured release target: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`, `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`, `x86_64-apple-darwin`, and `aarch64-pc-windows-msvc`.
3. Generate SHA-256 checksums for the packaged artifacts.
4. Generate a changelog from git commits.
5. Publish the GitHub release with artifacts and checksums.

Distribution packages such as deb, rpm, Arch, Flatpak, Snap, Homebrew, Nix, AppImage, DMG, and Docker images are not currently built by `release.yml`.

---

## 7) Test Harness Location

| Test Type | Location | Runner |
| --------- | -------- | ------ |
| Unit tests | Inline in each crate (`#[cfg(test)]` modules) | `cargo test` |
| Integration tests | `tests/integration/` | `cargo test --test <name>` |
| End-to-end tests | `tests/e2e/` | `cargo test --test <name>` (requires full server + client setup) |
| Fuzz tests | `tests/fuzz/<target>/` | `cargo fuzz run <target>` from the target manifest directory; CI time-budgeted runs are roadmap |
| Root Criterion benchmarks | `benches/` | `cargo bench --bench <name>` for registered root targets (`compositor_render`, `layout_cache`, `tile_encode`, `transport_throughput`) |
| Crate-local Criterion benchmarks | `crates/*/benches/` | `cargo bench -p <crate> --bench <name>`; SIMD blur coverage lives in `liquide-simd`'s `simd_bench` and `liquide-renderer-cpu`'s `renderer_bench` |
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
| ------ | ------- | ---------- |
| `main` | Stable development. All PRs merge here. | CI must pass. 1 approval required. |
| `release/X.Y` | Release branch for version X.Y. Cherry-picks from main. | CI must pass. 2 approvals required. |
| `feature/*` | Feature development branches. | None (developer's branch). |
| `fix/*` | Bug fix branches. | None. |

---

## 9) Test Plan

### Build System

- Verify `cargo build` succeeds for each target in the current release matrix with a clean checkout.
- Verify the PR `ci.yml` Linux build/test jobs and Windows E2E job pass.
- Verify the nightly Linux GNU and Linux musl target builds pass.
- Verify `cargo test` passes for all crates.
- Verify `cargo clippy -- -D warnings` produces zero warnings.
- Verify `cargo fmt --check` produces no diffs.
- Verify `cargo deny check` passes.
- Verify `cargo audit` reports no vulnerabilities.
- Verify release profile produces stripped, optimized binaries.
- Verify cross-compilation for Tier 2 release matrix targets succeeds.
- Verify `liquide-protocol` crate is a dependency of both server and client binaries.
- Verify all binaries are statically linked (musl targets) or have minimal dynamic deps (gnu targets).
