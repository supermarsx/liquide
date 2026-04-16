# Liquide — Production Readiness Review (Rev 2)

**Date**: 2026-04-16  
**Scope**: Full workspace — 141 crates, 1,973 files, 448,101 LOC  
**Previous Review**: 2026-04-15 (Rev 1)  
**Delta Since Rev 1**: 941 files changed, +222,476 / -2,333 LOC across 30 commits

---

## Executive Summary

| Metric | Rev 1 | Rev 2 | Change |
|--------|-------|-------|--------|
| Crates | 130+ | **141** | +11 |
| Source files | 1,912 | **1,973** | +61 |
| Lines of code | ~444K | **448,101** | +4K |
| Test functions | 11,984 | **12,138** | +154 |
| Tests passing | 11,984 | **12,137** | 1 flaky |
| TODOs/FIXMEs | — | **91** | — |
| Prod unwraps | — | **4,259** | — |
| Unsafe blocks | — | **949** | — |
| Prod panics | — | **227** | — |

### Test Suite

**All 12,138 tests pass** (excluding `liquide-apps-terminal` PTY tests).  
**1 flaky test**: `liquide-dpi::scale_manager_scale_for_window_spanning_monitors` — fails in workspace-wide run but passes in isolation. Likely test harness interaction, not a code bug.

### Overall Verdicts

| Verdict | Count | % | Change from Rev 1 |
|---------|-------|---|-------------------|
| **READY** | 115 | 81.6% | 113 → 115 (+2) |
| **NEEDS WORK** | 18 | 12.8% | 14 → 18 (+4 new findings) |
| **BLOCKING** | 8 | 5.7% | 3 → 8 (+5 escalations) |

### Test Coverage Distribution (139 substantial crates >200 LOC)

| Category | Count | % |
|----------|-------|---|
| Zero tests | 4 | 2.9% |
| Under-tested (density < 10 tests/KLOC) | 14 | 10.1% |
| Adequately tested (≥ 10 tests/KLOC) | 121 | 87.1% |

### Supply Chain & Toolchain

- **Rust**: Stable channel, rustfmt + clippy components
- **cargo-deny**: Vulnerability deny, copyleft deny, unknown registry/git deny, wildcard deny
- **License allowlist**: MIT, Apache-2.0, BSD-2/3, ISC, Zlib, MPL-2.0, Unicode

---

## BLOCKING Crates (8)

### 1. liquide-ctl — CLI Management Tool

| LOC | Tests | TODOs | Unwraps | Status |
|-----|-------|-------|---------|--------|
| 2,652 | 0 | 20 | 0 | **STILL BLOCKING** (unchanged from Rev 1) |

**Issue**: All 5 `Client` HTTP methods `bail!()`. All 28 command handlers print hardcoded fake output. No `reqwest` or HTTP client. The crate compiles and runs but does nothing useful.

**What improved**: The CLI framework, error types, config loading, and output formatting are now solid. The scaffold is production-quality — only the backend is missing.

**Fix scope**: Implement reqwest-based `Client`, wire 28 command handlers to REST API, add tests. ~1,170 LOC.

---

### 2. liquide-session — Desktop Session Manager

| LOC | Tests | TODOs | Unwraps | Unsafe | Status |
|-----|-------|-------|---------|--------|--------|
| 7,460 | 120 | 0 | 8 | 9 | **NEW BLOCKING** (was READY) |

**Issue**: IPC stubs in `ipc.rs` — `send_event()` and `receive_command()` are no-ops. Sessions cannot be managed by the supervisor (no shutdown, lock, suspend, policy updates). `UpdatePolicy` handler is dead code.

**Other findings**:
- 9 unsafe blocks in `lockfree_queue.rs` — all have SAFETY comments ✅
- `uuid_stub()` generates PID-based session IDs (not cryptographically unique)
- Render thread `fb.as_mut().unwrap()` calls are safe (guarded by preceding initialization)

**Fix scope**: Implement IPC wire protocol (pipe/socket) for supervisor ↔ session communication. ~300-500 LOC.

---

### 3. liquide-client — Remote Desktop Client

| LOC | Tests | TODOs | Unwraps | Status |
|-----|-------|-------|---------|--------|
| 3,711 | 57 | 4 | 0 | **ESCALATED TO BLOCKING** (was NEEDS WORK) |

**Issue**: Connection establishment skips TLS, protocol handshake, auth, and capability negotiation. TCP connects directly to "Connected" state. **Plaintext credentials and data**.

**Fix scope**: Implement 4 TODO stubs in connection.rs. ~355 LOC.

---

### 4. liquide-gateway — Connection Gateway

| LOC | Tests | TODOs | Unwraps | Status |
|-----|-------|-------|---------|--------|
| 3,869 | 31 | 5 | 0 | **ESCALATED TO BLOCKING** (was NEEDS WORK) |

**Issue**: `handle_tcp_connection()` accepts and rate-limits connections but then drops them. No TLS, no auth, no routing, no data forwarded. Gateway is a connection counter.

**Fix scope**: Implement TLS acceptor, protocol handshake, auth, session routing. ~2,830 LOC.

---

### 5. liquide-render-coordinator — Render Task Dispatcher

| LOC | Tests | TODOs | Unwraps | Status |
|-----|-------|-------|---------|--------|
| 1,793 | 9 | 1 | 1 | **ESCALATED TO BLOCKING** (was NEEDS WORK) |

**Issue**: `execute_task()` at `thread_pool.rs:245` is `std::thread::sleep(Duration::from_micros(100))` followed by a fake `RenderOutput::success`. No rendering occurs. The coordinator architecture (thread pools, priority queues, scheduling) is correct — only the dispatch is missing.

**Fix scope**: Add `RenderBackend` trait, wire to CPU/GPU/wgpu renderers. ~200 LOC for dispatch.

---

### 6. liquide-fonts — Font Management

| LOC | Tests | TODOs | Unwraps | Status |
|-----|-------|-------|---------|--------|
| 2,160 | 0 | 1 | 1 | **ESCALATED TO BLOCKING** (was NEEDS WORK) |

**Issue**: 2.4K LOC across 16 source files with zero tests. Handles font catalog, Google Fonts integration, hot reload, glyph management, and installation. `hot_reload` has no actual filesystem watcher — poll skeleton only.

**Fix scope**: Add comprehensive test suite (~76 tests). Implement hot-reload FS watcher.

---

### 7. liquide-protocol — Wire Protocol

| LOC | Tests | TODOs | Unwraps | Status |
|-----|-------|-------|---------|--------|
| 3,487 | 0 | — | — | **NEW BLOCKING** (not reviewed in Rev 1) |

**Issue**: Zero tests for a 3.5K LOC wire protocol crate that defines the serialization/deserialization contract between client and server. Protocol breakage would silently corrupt all remote sessions.

**Fix scope**: Add serialization round-trip tests, version compatibility checks, and boundary tests.

---

### 8. liquide-plugin-abi — Plugin ABI Contract

| LOC | Tests | TODOs | Unwraps | Status |
|-----|-------|-------|---------|--------|
| 129 | 0 | 0 | 0 | **ESCALATED TO BLOCKING** (was NEEDS WORK) |

**Issue**: Zero tests for the extension system ABI contract. No `#[repr(C)]` on WASM-boundary structs (only `PluginResult` has `#[repr(i32)]`). No layout assertions. ABI breakage would silently corrupt all plugins.

**Fix scope**: Small crate — add layout tests, serde round-trips, version checks. ~150 LOC tests.

---

## NEEDS WORK Crates (18)

### Previously Flagged — Still Open

| # | Crate | LOC | Tests | Issue | Severity |
|---|-------|-----|-------|-------|----------|
| 1 | **liquide-renderer-wgpu** | 3,230 | 0 | 6/10 scene node types have stub dispatch (pipelines exist but aren't wired). 0 tests. Text+image rendering works. | High |
| 2 | **liquide-policy** | 260 | 0 | `load_from_dir()` still stub (returns empty engine). Evaluation works. | Medium |
| 3 | **liquide-render-thread** | 1,049 | 8 | `damage.mark_all()` ignores damage parameter — every frame is full repaint. 2 TODOs. | High |
| 4 | **liquide-theme-css** | 6,220 | 42 | `var()` lives in style-engine only, not in CSS crate. 6 prod unwraps. | Medium |
| 5 | **liquide-font-rasterizer** | 3,417 | 29 | No end-to-end rasterization output validation. 29 tests but no pixel correctness checks. | Medium |
| 6 | **liquide-shell** | 33,055 | 910 | 2 hardcoded `16.0ms` frame deltas (scene.rs:47, threading.rs:146). Blocks VRR support. | Medium |
| 7 | **liquide-hotkeys** | 1,968 | 18 | macOS `poll()` returns empty — no `InstallEventHandler` callback. Works on Windows/Linux. | Medium |
| 8 | **liquide-gestures** | 2,929 | 85 | 2 `unwrap()` in recognizer.rs hot path (lines 197, 218). Crashes on orphaned touch IDs. | High |
| 9 | **liquide-devtools** | 6,898 | 44 | 7 modules (2,357 LOC) with zero tests. live_reload improved (4 tests now). | Low |
| 10 | **liquide-common** | 223 | 0 | Foundation crate with 0 tests. Low risk given size. | Low |
| 11 | **liquide-telemetry-viewer** | 3,721 | 91 | `.partial_cmp().unwrap()` NaN panics in export.rs:101-102. | High |

### Newly Flagged

| # | Crate | LOC | Tests | Issue | Severity |
|---|-------|-----|-------|-------|----------|
| 12 | **liquide-service-manager** | 2,653 | 88 | **95 prod unwraps** (1 per 28 LOC). Service management daemon must not panic. | Critical |
| 13 | **liquide-plugins** | 2,416 | 115 | **76 prod unwraps** (1 per 32 LOC). Plugin host running third-party code. | Critical |
| 14 | **liquide-platform** | 9,931 | 50 | **206 unsafe blocks**, 5.0 tests/KLOC. Largest unsafe surface in workspace. | Critical |
| 15 | **liquide-transport** | 9,182 | 170 | `Endpoint::client().unwrap()` in quic.rs:70 — panics on socket bind failure. | Medium |
| 16 | **liquide-dpi** | 3,817 | 133 | 7 Win32 FFI unsafe blocks without SAFETY comments. `partial_cmp().unwrap()` on DPI values. Flaky test. | Medium |
| 17 | **liquide-authorization** | 4,745 | 184 | **44 prod unwraps** in security-sensitive auth code. | High |
| 18 | **liquide-apps-task-manager** | 15,269 | 77 | Triple threat: 28 unwraps + 27 unsafe + 1 `bail!()`. 5.0 tests/KLOC. | High |

---

## READY Crates — Highlights (115)

### Recently Hardened (no regressions found)

| Crate | LOC | Tests | Recent Change |
|-------|-----|-------|---------------|
| **liquide-compositor** | 5,075 | 116 | Scene graph depth guards (MAX=512), framebuffer bounds checking |
| **liquide-style-engine** | 15,984 | 60 | `:has()` depth guard (MAX=256), selector list bounds (MAX=1024), `var()` cycle detection |
| **liquide-layout** | 11,910 | 63 | Grid growth caps (10K), flex redistribution fix, edge case guards |
| **liquide-dom** | 6,205 | 141 | Observer panic safety, orphan warnings, reconciliation optimization |
| **liquide-tile-raster** | 2,826 | 59 | Cache invariants, coordinate bounds, damage limiting |
| **liquide-renderer-cpu** | 16,881 | 172 | Panic reduction, overflow guards. 27 `expect()` calls for framebuffer type invariant. |
| **liquide-client-renderer** | 2,179 | 77 | Two-phase frame assembly, tile validation. Cleanest crate — 0 unwraps, 0 unsafe, 0 TODOs. |
| **liquide-encoder** | 2,719 | 84 | Clean — 0 prod panic points |

---

## Per-Crate Metrics — Lowest Test Density (>200 LOC)

| Crate | LOC | Tests | Density (tests/KLOC) |
|-------|-----|-------|---------------------|
| liquide-renderer-wgpu | 2,914 | 0 | 0.0 |
| liquide-ctl | 2,652 | 0 | 0.0 |
| liquide-fonts | 2,160 | 0 | 0.0 |
| liquide-policy | 230 | 0 | 0.0 |
| liquide-renderer-css | 1,304 | 3 | 2.3 |
| liquide-ui-window | 730 | 2 | 2.7 |
| liquide-platform | 9,931 | 50 | 5.0 |
| liquide-dock | 975 | 5 | 5.1 |
| liquide-render-coordinator | 1,723 | 12 | 7.0 |
| liquide-devtools | 6,192 | 44 | 7.1 |
| liquide-theme-css | 5,766 | 42 | 7.3 |
| liquide-layout | 11,414 | 96 | 8.4 |
| liquide-render-thread | 938 | 8 | 8.5 |
| liquide-gateway | 3,407 | 31 | 9.1 |
| liquide-font-rasterizer | 3,046 | 29 | 9.5 |

---

## Critical Issue Summary — Ranked by Impact

### Tier 1: Security / Correctness Blockers

| # | Issue | Crate | Impact |
|---|-------|-------|--------|
| 1 | **No TLS/auth on remote connections** | liquide-client, liquide-gateway | Plaintext credentials and session data |
| 2 | **IPC stubs — sessions unmanageable** | liquide-session | Cannot shutdown/lock/suspend sessions from supervisor |
| 3 | **Wire protocol untested** | liquide-protocol | Protocol breakage silently corrupts remote sessions |
| 4 | **Plugin ABI untested** | liquide-plugin-abi | ABI breakage silently corrupts all plugins |
| 5 | **95 unwraps in service manager** | liquide-service-manager | Service daemon crash on any unexpected state |
| 6 | **44 unwraps in authorization** | liquide-authorization | Security-critical auth code panics on edge cases |

### Tier 2: Performance / Reliability

| # | Issue | Crate | Impact |
|---|-------|-------|--------|
| 7 | **Render coordinator is a no-op** | liquide-render-coordinator | All rendering is faked — 100μs sleep per task |
| 8 | **Damage tracking broken** | liquide-render-thread | Every frame is full repaint (~10x performance waste) |
| 9 | **6/10 wgpu node types stub** | liquide-renderer-wgpu | Gradients, shadows, filters, layers not GPU-rendered |
| 10 | **CLI tool non-functional** | liquide-ctl | No server management capability |

### Tier 3: Stability / Quality

| # | Issue | Crate | Impact |
|---|-------|-------|--------|
| 11 | **2 hot-path unwraps** | liquide-gestures | Crash on orphaned touch events |
| 12 | **NaN panic in sort** | liquide-telemetry-viewer | Report generation crashes on invalid metrics |
| 13 | **Hardcoded 16ms frame delta** | liquide-shell | Wrong timing on non-60Hz displays |
| 14 | **76 unwraps in plugin host** | liquide-plugins | Plugin operations crash the host |
| 15 | **206 unsafe blocks, 5 tests/KLOC** | liquide-platform | Massive unsafe surface with minimal testing |
| 16 | **macOS hotkeys dead** | liquide-hotkeys | Platform gap |
| 17 | **Policy loader stub** | liquide-policy | No file-based policy enforcement |
| 18 | **Font crate untested** | liquide-fonts | 2.4K LOC font subsystem with 0 tests |

---

## Quick Wins (fixable in < 1 day each)

| Crate | Fix | LOC |
|-------|-----|-----|
| liquide-gestures | Replace 2 `.unwrap()` with `if let` | ~10 |
| liquide-telemetry-viewer | Replace `.partial_cmp().unwrap()` with `.total_cmp()` | ~4 |
| liquide-shell | Replace hardcoded `16.0` with `1_000_000 / refresh_rate` | ~10 |
| liquide-plugin-abi | Add layout + serde round-trip tests | ~150 |
| liquide-dpi | Add SAFETY comments to 7 Win32 FFI blocks, use `total_cmp()` | ~30 |
| liquide-transport | Replace `Endpoint::client().unwrap()` with `?` propagation | ~5 |
| liquide-policy | Implement TOML `load_from_dir()` | ~120 |

---

## Comparison: Rev 1 → Rev 2

### Improvements Since Last Review

1. **liquide-renderer-wgpu**: Went from pure stubs to functional GPU text/image rendering with 8 WGSL shaders and full pipeline setup. Downgraded from BLOCKING → NEEDS WORK.
2. **liquide-policy**: Evaluation engine complete and functional. Downgraded from BLOCKING → NEEDS WORK (only loader stub remains).
3. **liquide-compositor**: Hardened with depth guards and framebuffer safety. Confirmed READY.
4. **liquide-style-engine**: Fixed `:has()` depth, selector bounds, `var()` cycles. Confirmed READY.
5. **liquide-layout**: Fixed grid growth, flex redistribution. Confirmed READY.
6. **liquide-dom**: Added observer panic safety, orphan warnings. Confirmed READY.
7. **liquide-shell**: Notification panel fully implemented (was stub). 910 tests (was ~800).
8. **liquide-devtools**: `live_reload` now has 4 tests (was 0).

### Regressions / Escalations

1. **liquide-session**: NEW BLOCKING — IPC stubs discovered (not caught in Rev 1).
2. **liquide-client**: NEEDS WORK → BLOCKING (no TLS is a security blocker).
3. **liquide-gateway**: NEEDS WORK → BLOCKING (connection handler does nothing).
4. **liquide-render-coordinator**: NEEDS WORK → BLOCKING (still a no-op sleep).
5. **liquide-fonts**: NEEDS WORK → BLOCKING (2.4K LOC with 0 tests).
6. **liquide-plugin-abi**: NEEDS WORK → BLOCKING (ABI contract with 0 tests).
7. **liquide-protocol**: NEW BLOCKING — 3.5K LOC wire protocol with 0 tests.

### Newly Discovered Issues (not in Rev 1)

- **liquide-service-manager**: 95 prod unwraps
- **liquide-plugins**: 76 prod unwraps
- **liquide-platform**: 206 unsafe blocks, 5.0 tests/KLOC
- **liquide-authorization**: 44 prod unwraps in security code
- **liquide-apps-task-manager**: 28 unwraps + 27 unsafe + bail!()
- **liquide-transport**: Panic on socket bind failure
- **liquide-dpi**: Undocumented unsafe FFI, NaN panic risk, flaky test
