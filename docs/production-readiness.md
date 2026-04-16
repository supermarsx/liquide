# Liquide — Production Readiness

**Workspace**: 141 crates, 1,973 files, 448,101 LOC  
**Tests**: 12,339 (12,138 baseline + 201 from remediation)  
**Last Review**: 2026-04-16 (Rev 2) · Remediation: 2025-07-18

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Verdicts Overview](#verdicts-overview)
3. [BLOCKING Crates](#blocking-crates)
4. [NEEDS WORK Crates](#needs-work-crates)
5. [Critical Issues — Ranked by Impact](#critical-issues--ranked-by-impact)
6. [Quick Wins](#quick-wins)
7. [Implementation Plans](#implementation-plans)
   - [liquide-renderer-wgpu](#liquide-renderer-wgpu)
   - [liquide-ctl](#liquide-ctl)
   - [liquide-policy](#liquide-policy)
   - [liquide-client](#liquide-client)
   - [liquide-gateway](#liquide-gateway)
   - [liquide-render-coordinator](#liquide-render-coordinator)
   - [liquide-render-thread](#liquide-render-thread)
   - [liquide-plugin-abi](#liquide-plugin-abi)
   - [liquide-font-rasterizer](#liquide-font-rasterizer)
   - [liquide-theme-css](#liquide-theme-css)
   - [liquide-devtools](#liquide-devtools)
   - [liquide-shell](#liquide-shell)
   - [liquide-hotkeys](#liquide-hotkeys)
   - [liquide-gestures](#liquide-gestures)
   - [liquide-telemetry-viewer](#liquide-telemetry-viewer)
   - [liquide-common](#liquide-common)
   - [liquide-fonts](#liquide-fonts)
8. [Remediation Results](#remediation-results)
9. [Remaining Work](#remaining-work)
10. [Supply Chain & Toolchain](#supply-chain--toolchain)

---

## Executive Summary

| Metric | Rev 1 | Rev 2 | Post-Remediation |
|--------|-------|-------|------------------|
| Crates | 130+ | 141 | 141 |
| Lines of code | ~444K | 448,101 | ~448K |
| Test functions | 11,984 | 12,138 | ~12,339 |
| TODOs/FIXMEs | — | 91 | — |
| Prod unwraps | — | 4,259 | — |
| Unsafe blocks | — | 949 | — |
| Prod panics | — | 227 | — |

**Known flaky test**: `liquide-dpi::scale_manager_scale_for_window_spanning_monitors` — passes in isolation, fails in workspace-wide runs (test harness interaction).

**Excluded from CI**: `liquide-apps-terminal` PTY tests (require terminal environment).

---

## Verdicts Overview

### After Review (Rev 2)

| Verdict | Count | % |
|---------|-------|---|
| READY | 115 | 81.6% |
| NEEDS WORK | 18 | 12.8% |
| BLOCKING | 8 | 5.7% |

### After Remediation

| Verdict | Count | % | Delta |
|---------|-------|---|-------|
| READY | 124 | 87.9% | +9 |
| NEEDS WORK | 13 | 9.2% | -5 |
| BLOCKING | 4 | 2.8% | **-4 resolved** |

### Test Coverage Distribution (139 crates > 200 LOC)

| Category | Count | % |
|----------|-------|---|
| Zero tests | 4 | 2.9% |
| Under-tested (< 10 tests/KLOC) | 14 | 10.1% |
| Adequately tested (≥ 10 tests/KLOC) | 121 | 87.1% |

---

## BLOCKING Crates

### Still Blocking After Remediation (4)

| Crate | LOC | Tests | Issue |
|-------|-----|-------|-------|
| **liquide-ctl** | 2,652 | 0 | All 5 `Client` HTTP methods `bail!()`. 28 command handlers print fake output. No `reqwest` client. |
| **liquide-client** | 3,711 | 57 | No TLS, no protocol handshake, no auth, no capability negotiation. Plaintext credentials. |
| **liquide-gateway** | 3,869 | 31 | `handle_tcp_connection()` accepts and rate-limits, then drops. No TLS/auth/routing/forwarding. |
| **liquide-session** | 7,460 | 120 | IPC fixed (SA5), but desktop/window management stubs remain. |

### Resolved by Remediation (4)

| Crate | Was | Now | Fix |
|-------|-----|-----|-----|
| **liquide-render-coordinator** | BLOCKING | READY | Replaced `sleep(100µs)` stub with real `RenderTaskKind` dispatch |
| **liquide-protocol** | BLOCKING | READY | Added 92 tests (serialization, codecs, state machines) |
| **liquide-plugin-abi** | BLOCKING | READY | Added 29 tests (layouts, manifests, host functions) |
| **liquide-fonts** | BLOCKING | READY | Added 60 tests across catalog, family, index, install |

---

## NEEDS WORK Crates

### Previously Flagged — Still Open (11)

| Crate | LOC | Tests | Issue | Severity |
|-------|-----|-------|-------|----------|
| **liquide-renderer-wgpu** | 3,230 | 0 | 6/10 scene node types stub. 0 tests. Text+image works. | High |
| **liquide-policy** | 260 | 10 | Loader implemented (SA7), but evaluation edge cases remain. | Medium |
| **liquide-theme-css** | 6,220 | 42 | `var()` not resolved. 6 prod unwraps. | Medium |
| **liquide-font-rasterizer** | 3,417 | 29 | No pixel correctness validation. | Medium |
| **liquide-hotkeys** | 1,968 | 18 | macOS `poll()` returns empty — no `InstallEventHandler` callback. | Medium |
| **liquide-devtools** | 6,898 | 44 | 7 modules (2,357 LOC) with zero tests. | Low |
| **liquide-common** | 223 | 0 | Foundation crate with 0 tests. Low risk given size. | Low |

### Newly Flagged in Rev 2 (6)

| Crate | LOC | Tests | Issue | Severity |
|-------|-----|-------|-------|----------|
| **liquide-service-manager** | 2,653 | 88 | 95 prod unwraps (1 per 28 LOC). 2 worst fixed by SA8. | Critical |
| **liquide-plugins** | 2,416 | 115 | 76 prod unwraps (1 per 32 LOC). | Critical |
| **liquide-platform** | 9,931 | 50 | 206 unsafe blocks, 5.0 tests/KLOC. | Critical |
| **liquide-authorization** | 4,745 | 184 | 44 prod unwraps in security-sensitive code (confirmed test-only by SA8). | High |
| **liquide-apps-task-manager** | 15,269 | 77 | 28 unwraps + 27 unsafe + 1 `bail!()`. 5.0 tests/KLOC. | High |
| **liquide-render-thread** | 1,049 | 8 | Damage tracking fixed (SA3), but 2 TODOs remain. | Medium |

### Resolved by Remediation (5)

| Crate | Was | Now | Fix |
|-------|-----|-----|-----|
| **liquide-gestures** | NEEDS WORK | READY | Replaced 2 `.unwrap()` with `if let` guards |
| **liquide-telemetry-viewer** | NEEDS WORK | READY | `.partial_cmp().unwrap()` → `.total_cmp()` |
| **liquide-transport** | NEEDS WORK | READY | `.unwrap()` → `?` propagation |
| **liquide-dpi** | NEEDS WORK | READY | `total_cmp()` + SAFETY docs on 7 Win32 FFI blocks |
| **liquide-shell** | NEEDS WORK | READY | Frame delta as named constant, hardcoded `16.0` replaced |

---

## Critical Issues — Ranked by Impact

### Tier 1: Security / Correctness

| # | Issue | Crate | Impact |
|---|-------|-------|--------|
| 1 | No TLS/auth on remote connections | liquide-client, liquide-gateway | Plaintext credentials and session data |
| 2 | IPC stubs — sessions unmanageable | liquide-session | Cannot shutdown/lock/suspend from supervisor |
| 3 | 95 unwraps in service manager | liquide-service-manager | Daemon crash on unexpected state |
| 4 | 76 unwraps in plugin host | liquide-plugins | Plugin operations crash the host |

### Tier 2: Performance / Reliability

| # | Issue | Crate | Impact |
|---|-------|-------|--------|
| 5 | 6/10 wgpu node types stub | liquide-renderer-wgpu | Gradients, shadows, filters not GPU-rendered |
| 6 | CLI tool non-functional | liquide-ctl | No server management capability |
| 7 | 206 unsafe blocks, 5 tests/KLOC | liquide-platform | Massive unsafe surface with minimal testing |

### Tier 3: Stability / Quality

| # | Issue | Crate | Impact |
|---|-------|-------|--------|
| 8 | CSS `var()` not resolved | liquide-theme-css | All theme customization broken |
| 9 | macOS hotkeys dead | liquide-hotkeys | Platform gap |
| 10 | Font rasterizer output unvalidated | liquide-font-rasterizer | Correctness risk |

---

## Quick Wins

Fixable in < 1 day each:

| Crate | Fix | LOC |
|-------|-----|-----|
| liquide-policy | Implement TOML `load_from_dir()` | ~120 |
| liquide-plugin-abi | Add layout + serde round-trip tests | ~150 |
| liquide-dpi | SAFETY comments on 7 Win32 FFI blocks, `total_cmp()` | ~30 |
| liquide-transport | Replace `Endpoint::client().unwrap()` with `?` | ~5 |

> **Note**: Several quick wins were already completed during remediation (gestures, telemetry-viewer, shell, transport, dpi).

---

## Implementation Plans

### liquide-renderer-wgpu

**Status**: 3,230 LOC · 0 tests · 71 unsafe blocks · 6 stub render-loop methods  
**Detailed plan**: [docs/wgpu-implementation-plan.md](wgpu-implementation-plan.md)

5 phases, ~1,895 LOC, 27 tests:

| Phase | Scope | LOC | Tests |
|-------|-------|-----|-------|
| P1: Background/Tint/Surface | Rect pipeline dispatch, bind group layout storage, surface blit | ~235 | — |
| P2: Shadow/Gradient | Shadow SDF dispatch, gradient stop buffers, corner radius masking | ~220 | — |
| P3: Filter/Blur/Blend | Compute filter pipeline, 2-pass blur, RenderLayer compositing | ~540 | — |
| P4: Tests | 22 unit + 5 visual parity tests, headless harness | ~670 | 27 |
| P5: Cleanup | Wire `render_frame_filtered()`, remaining node types | ~230 | — |

Key risk: Filter compute shader read/write conflicts require scratch textures.

---

### liquide-ctl

**Status**: 2,652 LOC · 0 tests · 20 TODOs · All 5 Client methods bail  

6 phases, ~1,170 net LOC, 100 tests:

| Phase | Scope | LOC | Tests |
|-------|-------|-----|-------|
| P1: Client Rewrite | `reqwest` HTTP client, auth headers, error mapping | +200 | 15 |
| P2: Core Commands | sessions, users, lock/unlock | +160 | 15 |
| P3: Config & Policy | config CRUD, policy show/set | +120 | 10 |
| P4: Infrastructure | 11 command files (service, gateway, monitors, etc.) | +240 | 15 |
| P5: Plugins & Packages | plugins, crash, supervisor, cache, package managers | +270 | 25 |
| P6: Integration Suite | `wiremock` mock server + `assert_cmd` binary tests | +180 | 20 |

Client architecture: `reqwest::Client` with Bearer auth, Unix socket option, HTTP→`LiquidctlError` mapping. All 29 commands map to `/api/v1/` REST endpoints on `liquide-manager`.

---

### liquide-policy

**Status**: 260 LOC · 10 tests (post-remediation) · Loader implemented  

Remaining: edge-case hardening and reload API.

| Phase | Scope | LOC |
|-------|-------|-----|
| P1: Unknown Key Warnings | `KNOWN_POLICY_KEYS` constant, load-time validation | ~25 |
| P2: Reload API | `PolicyEngine::reload(&mut self, dir)` | ~8 |
| P3: Edge-case Tests | Type mismatches, empty rulesets, defaults | ~100 |

Directory layout: `/etc/liquide/policy.d/` with `server.toml`, `group/*.toml`, `user/*.toml`, `session/*.toml`.

---

### liquide-client

**Status**: 3,711 LOC · 57 tests · 4 TODOs in connection establishment  

5 phases, ~355 LOC:

| Phase | Scope | LOC |
|-------|-------|-----|
| P1: TLS Handshake | `rustls` client config, cert validation, ALPN | ~80 |
| P2: Protocol Handshake | `ClientHello`/`ServerHello` exchange | ~80 |
| P3: Auth Exchange | `LoginPrompt`/`LoginResponse` flow | ~60 |
| P4: Capability Negotiation | Encode/decode client capabilities | ~75 |
| P5: Runtime Integration | Wire into state machine, timeouts, reconnect | ~60 |

---

### liquide-gateway

**Status**: 3,869 LOC · 31 tests · Steps 4-25 in connection handler stubbed  

4 phases, ~2,830 LOC:

| Phase | Scope | LOC |
|-------|-------|-----|
| P1: Frame I/O | TLS acceptor (rustls), frame codec, connection lifecycle | ~600 |
| P2: Protocol Handshake | Server-side `ServerHello`, version negotiation, capabilities | ~400 |
| P3: Auth & Session | PAM integration, JWT issuance, session routing | ~800 |
| P4: Relay & Broker | Frame relay, session multiplexing, health checks | ~1,030 |

Architecture: `Client → [TLS] → Gateway → [Auth] → Session Router → Backend Server`

---

### liquide-render-coordinator

**Status**: READY (post-remediation)  

Stub `execute_task()` replaced with real `RenderTaskKind` dispatch. Poll-sleep replaced with blocking `recv_timeout` + batch drain.

Future work: `RenderBackend` trait for pluggable CPU/GPU/wgpu backends.

---

### liquide-render-thread

**Status**: 1,049 LOC · 8 tests · Damage tracking fixed (SA3)  

Remaining phases:

| Phase | Scope | LOC | Tests |
|-------|-------|-----|-------|
| P2: Buffer Pooling | Pre-allocate tile buffers, reuse across frames | ~250 | 4 |
| P3: Panic Recovery | Catch panics in render workers, restart thread | ~180 | 4 |
| P4: Frame Pairing | Timeout for frame completion, dropped frame tracking | ~150 | 3 |
| P5: Metrics | Per-worker render time, frame drop counters | ~150 | 3 |

---

### liquide-plugin-abi

**Status**: READY (post-remediation) — 29 tests added for layouts, manifests, host functions.

Future work: `#[repr(C)]` on WASM-boundary structs, explicit layout assertions.

---

### liquide-font-rasterizer

**Status**: 3,417 LOC · 29 tests · No pixel correctness validation  

3 phases, ~1,483 LOC tests, 88 tests:

| Phase | Scope | Tests |
|-------|-------|-------|
| P1: Core Rasterization | Glyph output validation, subpixel modes | 35 |
| P2: Color Fonts | COLR/CPAL tables, emoji rasterization, SVG fonts | 25 |
| P3: Text Shaping | `rustybuzz` integration, ligatures, kerning, BiDi | 28 |

---

### liquide-theme-css

**Status**: 6,220 LOC · 42 tests · CSS `var()` not resolved  

6 phases, ~5,600 LOC, 168 tests:

| Phase | Scope | Tests |
|-------|-------|-------|
| P1: CSS Variables | `var()` resolution, fallback values, cycle detection | 30 |
| P2: Specificity | Calculation for all selector types, `!important` | 25 |
| P3: Complex Selectors | Combinators, `:not()`, `:is()`, `:where()` | 28 |
| P4: Cascade | Full cascade algorithm, layer ordering | 25 |
| P5: Media/Supports | `@media` conditions, `@supports` | 30 |
| P6: Color Spaces | HSL, HWB, Lab, LCH, oklch, `color-mix()` | 30 |

---

### liquide-devtools

**Status**: 6,898 LOC · 44 tests · 7 modules with zero tests  

3 phases, ~120-150 new tests:

| Phase | Tests | Scope |
|-------|-------|-------|
| P1: Critical Modules | 50 | `live_reload`, console, rendering |
| P2: Input Handling | 40 | Keyboard, mouse, panel focus |
| P3: Integration | 30-60 | Inspector lifecycle, DOM tree, style panel |

Zero-test modules: `live_reload` (~400 LOC), `keyboard`, `mouse`, `rendering`, `scene`, `side_panels`, `types`.

---

### liquide-shell

**Status**: READY (post-remediation) — frame delta fixed, 910 tests.

Remaining lower-priority items:
- Pipeline cache `.unwrap()` → `.unwrap_or_default()` in `stages.rs:138-140`
- Focus history `.unwrap()` → `None` return in `focus.rs:73`
- Animation durations → configurable

---

### liquide-hotkeys

**Status**: 1,968 LOC · 18 tests · macOS non-functional  

4 phases, ~750 LOC, 12 tests:

| Phase | Scope | LOC |
|-------|-------|-----|
| P1: Core Event Handler | `InstallEventHandler` FFI, Carbon callback, `mpsc` bridge | ~140 |
| P2: Event Polling | Wire channel receiver to `poll()` | ~65 |
| P3: Lifecycle | App target validation, thread safety | ~95 |
| P4: Testing | Platform-specific + cross-platform regression | ~450 |

Root cause: `RegisterEventHotKey` succeeds but no `InstallEventHandler` callback is installed, so `poll()` always returns empty.

---

### liquide-gestures

**Status**: READY (post-remediation) — 2 hot-path unwraps replaced with `if let` guards.

---

### liquide-telemetry-viewer

**Status**: READY (post-remediation) — NaN-safe sorting via `total_cmp()`.

---

### liquide-common

**Status**: 223 LOC · 0 tests  

5 phases, ~1,650 LOC tests, 65 tests:

| Phase | Scope | Tests |
|-------|-------|-------|
| P1: Error Types | `LiquideError` variants, `Display`, `From` impls | 12 |
| P2: Config Loading | `load_toml()` happy path, missing file, invalid TOML | 15 |
| P3: Path Resolution | `dirs_fallback()`, XDG, Windows special folders | 10 |
| P4: Logging | `init_logging()`, `RUST_LOG` env interaction | 8 |
| P5: Poison Recovery | Mutex/RwLock poison recovery, thread panic scenarios | 20 |

---

### liquide-fonts

**Status**: READY (post-remediation) — 60 tests added by SA7.

Remaining: `hot_reload` FS watcher implementation (currently poll skeleton only).

---

## Remediation Results

8 focused subagents deployed in sequence:

| Subagent | Targets | Tests Added | Key Fixes |
|----------|---------|-------------|-----------|
| SA1: Quick Crash Fixes | gestures, telemetry-viewer, transport, dpi | — | 9 crash paths eliminated |
| SA2: Shell Frame Delta | shell | — | Named constant, correct 60 Hz value |
| SA3: Render-Thread Damage | compositor, render-thread | — | `mark_rect()` per rectangle, incremental rendering |
| SA4: Render-Coordinator | render-coordinator | — | Real `RenderTaskKind` dispatch, blocking recv |
| SA5: Session IPC | session | 10 | `mpsc` channels, `SupervisorHandle`, send/receive |
| SA6: Protocol + ABI Tests | protocol, plugin-abi | 121 | 92 protocol + 29 ABI tests |
| SA7: Policy + Fonts | policy, fonts | 70 | TOML loader + 60 font tests |
| SA8: Hardening | service-manager, authorization | — | 2 prod unwraps → error propagation |
| **Total** | | **~201** | |

---

## Remaining Work

### Aggregate Effort Estimates

| Category | Crates | New Code LOC | New Test LOC | New Tests |
|----------|--------|-------------|-------------|-----------|
| Still BLOCKING | 4 | ~4,355 | ~1,200 | ~135 |
| NEEDS WORK | 13 | ~3,000 | ~12,000 | ~440 |
| **Total** | **17** | **~7,355** | **~13,200** | **~575** |

### Priority Order

| Priority | Crate | Justification |
|----------|-------|---------------|
| P0 | liquide-client + liquide-gateway | Security: plaintext credentials |
| P0 | liquide-ctl | Operations: no CLI management |
| P1 | liquide-renderer-wgpu | 6/10 node types stubbed |
| P1 | liquide-service-manager | 95 prod unwraps in daemon |
| P1 | liquide-plugins | 76 prod unwraps in host |
| P1 | liquide-platform | 206 unsafe blocks, 5 tests/KLOC |
| P2 | liquide-theme-css | `var()` breaks theming |
| P2 | liquide-hotkeys | macOS platform gap |
| P3 | liquide-font-rasterizer | Output correctness |
| P3 | liquide-devtools | Low coverage |
| P3 | liquide-common | Foundation crate |

---

## Supply Chain & Toolchain

- **Rust**: Stable channel, rustfmt + clippy
- **cargo-deny**: Vulnerability deny, copyleft deny, unknown registry/git deny, wildcard deny
- **License allowlist**: MIT, Apache-2.0, BSD-2/3, ISC, Zlib, MPL-2.0, Unicode

---

## READY Crates — Highlights (124)

Recently hardened (no regressions):

| Crate | LOC | Tests | Notes |
|-------|-----|-------|-------|
| liquide-compositor | 5,075 | 116 | Scene graph depth guards (MAX=512), framebuffer bounds |
| liquide-style-engine | 15,984 | 60 | `:has()` depth guard, selector bounds, `var()` cycle detection |
| liquide-layout | 11,910 | 63 | Grid growth caps (10K), flex redistribution fix |
| liquide-dom | 6,205 | 141 | Observer panic safety, orphan warnings |
| liquide-renderer-cpu | 16,881 | 172 | Panic reduction, overflow guards |
| liquide-client-renderer | 2,179 | 77 | Cleanest crate — 0 unwraps, 0 unsafe, 0 TODOs |
| liquide-encoder | 2,719 | 84 | 0 prod panic points |

### Lowest Test Density (> 200 LOC)

| Crate | LOC | Tests | Density |
|-------|-----|-------|---------|
| liquide-renderer-wgpu | 2,914 | 0 | 0.0 |
| liquide-ctl | 2,652 | 0 | 0.0 |
| liquide-common | 223 | 0 | 0.0 |
| liquide-renderer-css | 1,304 | 3 | 2.3 |
| liquide-ui-window | 730 | 2 | 2.7 |
| liquide-platform | 9,931 | 50 | 5.0 |
| liquide-dock | 975 | 5 | 5.1 |
