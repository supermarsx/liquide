# Liquide — Production Readiness Implementation Plans

**Generated**: 2026-04-15  
**Scope**: All 17 crates flagged during production readiness audit  
**Workspace**: 130+ crates, ~444K LOC, 11,984 tests (all passing)  
**Overall Verdict**: 113 READY (87%) · 14 NEEDS WORK (11%) · 3 BLOCKING (2%)

---

## Table of Contents

1. [BLOCKING — liquide-renderer-wgpu](#1-blocking--liquide-renderer-wgpu)
2. [BLOCKING — liquide-ctl](#2-blocking--liquide-ctl)
3. [BLOCKING — liquide-policy](#3-blocking--liquide-policy)
4. [NEEDS WORK — liquide-common](#4-needs-work--liquide-common)
5. [NEEDS WORK — liquide-fonts](#5-needs-work--liquide-fonts)
6. [NEEDS WORK — liquide-client](#6-needs-work--liquide-client)
7. [NEEDS WORK — liquide-gateway](#7-needs-work--liquide-gateway)
8. [NEEDS WORK — liquide-render-coordinator](#8-needs-work--liquide-render-coordinator)
9. [NEEDS WORK — liquide-render-thread](#9-needs-work--liquide-render-thread)
10. [NEEDS WORK — liquide-plugin-abi](#10-needs-work--liquide-plugin-abi)
11. [NEEDS WORK — liquide-font-rasterizer](#11-needs-work--liquide-font-rasterizer)
12. [NEEDS WORK — liquide-theme-css](#12-needs-work--liquide-theme-css)
13. [NEEDS WORK — liquide-devtools](#13-needs-work--liquide-devtools)
14. [NEEDS WORK — liquide-shell](#14-needs-work--liquide-shell)
15. [NEEDS WORK — liquide-hotkeys](#15-needs-work--liquide-hotkeys)
16. [NEEDS WORK — liquide-gestures](#16-needs-work--liquide-gestures)
17. [NEEDS WORK — liquide-telemetry-viewer](#17-needs-work--liquide-telemetry-viewer)
18. [Aggregate Summary](#18-aggregate-summary)

---

## 1. BLOCKING — liquide-renderer-wgpu

**Status**: 2914 LOC · 0 tests · 71 unsafe blocks · 8 TODOs · 6 stub render-loop methods  
**Severity**: BLOCKING — No functional GPU rendering via wgpu backend  

### Problem Summary

The wgpu renderer has structural scaffolding but no functional render loop. All 6 core render methods are stubs that return early without producing frames. The 71 unsafe blocks (FFI to wgpu/GPU) are unaudited. Zero tests exist for a safety-critical rendering backend.

### Implementation Plan (5 Phases, ~1895 LOC, 27 tests)

| Phase | Scope | LOC | Tests |
|-------|-------|-----|-------|
| **P1: Device & Surface** | wgpu instance/adapter/device creation, surface configuration, swap chain setup | ~400 | 5 |
| **P2: Pipeline & Shaders** | Render pipeline creation, shader module compilation (WGSL), vertex/index buffer layouts, bind group layouts | ~450 | 5 |
| **P3: Render Loop** | Command encoder, render pass, draw calls for tile regions, texture atlas management, frame presentation | ~500 | 7 |
| **P4: Damage Tracking** | Incremental redraw — accept damage rects from compositor, only re-render affected tiles, double-buffer management | ~300 | 5 |
| **P5: Safety Audit & Tests** | Audit all 71 unsafe blocks, add SAFETY comments, integration tests with headless wgpu adapter | ~245 | 5 |

### Key Technical Details

- Replace stub `render_frame()` with real wgpu command encoder pipeline
- Implement `WgpuTileAtlas` for texture atlas management (tile packing, eviction)
- Add `WgpuDamageTracker` to skip clean regions
- Target headless `wgpu::Backends::GL` for CI testing (no GPU required)
- All 71 unsafe blocks require `// SAFETY:` annotations per Rust convention

### Risk Assessment

| Risk | Impact | Mitigation |
|------|--------|------------|
| No CI GPU access | Can't run render tests in CI | Use wgpu headless backend (GL/Vulkan software renderer) |
| Unsafe audit reveals UB | Critical | Phase 5 dedicated to safety review |
| Performance unknown | May not hit 60fps tile composition | Benchmark in Phase 3, optimize in Phase 4 |

---

## 2. BLOCKING — liquide-ctl

**Status**: ~2600 LOC · 0 tests · 20 TODOs · All 5 Client methods bail · 29 commands print fake output  
**Severity**: BLOCKING — CLI tool is entirely non-functional  

### Problem Summary

The crate has a well-structured CLI framework (clap, 29 top-level commands, rich arg types, 4 output formats, typed error→exit-code mapping) but a completely non-functional backend: all 5 `Client` methods bail with `todo!()`; every command handler prints fake messages; 20 TODOs; 0 tests. The server counterpart (`liquide-manager`, 189 tests) provides a fully-defined REST API.

### Implementation Plan (6 Phases, ~1170 net LOC, 100 tests)

| Phase | Scope | LOC | Tests |
|-------|-------|-----|-------|
| **P1: Client Rewrite** | `reqwest` async HTTP client, URL normalization, auth header injection, error mapping (HTTP → `LiquidctlError`) | +200 | 15 |
| **P2: Core Commands** | sessions (list/show/disconnect), users (list/show/kick/avatar), lock/unlock, `TextDisplay` impls | +160 | 15 |
| **P3: Config & Policy** | config (show/validate/set/diff/export/import), policy (show/set/effective) | +120 | 10 |
| **P4: Infrastructure** | service, gateway, monitors, transport, audio, encoder, USB, audit, logs, honeypot — 11 command files | +240 | 15 |
| **P5: Plugins & Packages** | plugins (8 actions), crash (5), supervisor (4), cache (2), RDP (3), flatpak/brew/snap/nix/appimage | +270 | 25 |
| **P6: Integration Suite** | `wiremock` mock HTTP server tests, `assert_cmd` binary tests, output format validation | +180 | 20 |

### Client Architecture

```rust
pub struct Client {
    http: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}
```

- **Auth**: `Authorization: Bearer {key}` on every request
- **Unix socket**: `#[cfg(unix)]` for local connections (bypass TLS+auth)
- **Error mapping**: HTTP 401→`Authentication`, 403→`PermissionDenied`, 404→`NotFound`, 5xx→`Other`

### Shared Types Strategy

Direct dependency on `liquide-manager` for shared response types: `DashboardData`, `SessionSummary`, `SessionDetail`, `UserSummary`, `ApiResponse<T>`, `PaginatedResponse<T>`.

### Command → API Mapping (29 commands)

All commands map to `liquide-manager` REST endpoints under `/api/v1/`. Examples:
- `status` → `GET /api/v1/dashboard`
- `sessions list` → `GET /api/v1/sessions?user=&sort=`
- `config validate` → `POST /api/v1/config/validate`
- `policy effective` → `GET /api/v1/policies/effective/{username}`

### Test Strategy

| Layer | Tool | Count |
|-------|------|-------|
| Unit (client) | `#[cfg(test)]` | ~15 |
| Unit (types) | `#[cfg(test)]` | ~10 |
| Integration (mock) | `wiremock` | ~50 |
| Binary (E2E) | `assert_cmd` | ~25 |

---

## 3. BLOCKING — liquide-policy

**Status**: ~230 LOC · 0 tests · Stub `load_from_dir()` returns empty engine  
**Severity**: BLOCKING — Policy enforcement is non-functional  

### Problem Summary

5 source files with well-defined types (`PolicySource`, `EffectivePolicy`, `Rule`, `RuleAction`) and a working evaluation module, but the loader (`engine.rs:load_from_dir()`) is a 14-line stub returning an empty engine. Downstream consumers (`liquide-supervisor`, `liquide-manager`, etc.) call it expecting real policy enforcement.

### Implementation Plan (4 Phases, ~611 LOC, 48 tests)

| Phase | Scope | LOC | Tests |
|-------|-------|-----|-------|
| **P1: Loader + Format** | TOML schema, serde models, `load_from_dir()` implementation — walk `server.toml`, `group/*.toml`, `user/*.toml`, `session/*.toml` | ~128 | — |
| **P2: Unknown Key Warnings** | `KNOWN_POLICY_KEYS` constant, load-time key validation with `tracing::warn!` | ~25 | — |
| **P3: Tests** | 48 tests across 8 categories (source ordering, hierarchy, eval boolean, eval set-value, priority resolution, load happy, load errors, edge cases) | ~450 | 48 |
| **P4: Reload API** | `PolicyEngine::reload(&mut self, dir)` for supervisor-driven reloads | ~8 | — |

### Directory Layout

```
/etc/liquide/policy.d/
├── server.toml           # PolicySource::Server defaults
├── group/*.toml          # PolicySource::Group
├── user/*.toml           # PolicySource::User
└── session/*.toml        # PolicySource::Session
```

### TOML Schema

```toml
[[rules]]
key = "clipboard.enabled"
action = "allow"

[[rules]]
key = "display.max_width"
action = { set = "1920" }
```

### Error Handling

| Condition | Behavior |
|-----------|----------|
| Base dir missing | `warn!`, return empty engine |
| TOML parse error | `warn!` + skip file, continue loading |
| Invalid action string | `Err(PolicyError::Parse)` — hard fail on schema violation |

### Test Plan (48 Tests)

- **Source ordering** (4): Verify `Server < Group < User < Session`
- **Hierarchy** (3): `can_override()` directional checks
- **Boolean eval** (12): Allow/Deny for all 6 boolean policy keys
- **Set-value eval** (6): `display.max_width`, `max_height`, `session.idle_timeout` with valid/invalid values
- **Priority resolution** (6): Multi-layer cascade, override semantics
- **Load happy path** (6): Single files, all layers, empty dir
- **Load error handling** (6): Missing dir, malformed TOML, invalid actions, non-TOML files
- **Edge cases** (5): Unknown keys, empty rulesets, type mismatches, defaults

---

## 4. NEEDS WORK — liquide-common

**Status**: ~180 LOC production · 0 tests · Foundation crate used by entire workspace  
**Issue**: Zero test coverage for error types, config loading, path resolution, logging setup  

### Implementation Plan (5 Phases, ~1650 LOC tests, 65 tests)

| Phase | Scope | Tests |
|-------|-------|-------|
| **P1: Error Types** | `LiquideError` variants, `Display` output, `From` impls, `#[non_exhaustive]` | 12 |
| **P2: Config Loading** | `load_toml()` happy path, missing file, invalid TOML, permission errors, context messages | 15 |
| **P3: Path Resolution** | `dirs_fallback()` behavior, XDG paths, Windows special folders, fallback to `/` | 10 |
| **P4: Logging** | `init_logging()` levels, subscriber setup, `RUST_LOG` env interaction | 8 |
| **P5: Poison Recovery** | `Mutex`/`RwLock` poison recovery helpers, thread panic scenarios | 20 |

### Critical Fixes

- `dirs_fallback()` returning `/` as fallback — needs documented rationale or platform-specific default
- `load_toml()` error messages need context (file path in error)
- Add `#[non_exhaustive]` to `LiquideError` for future extensibility

---

## 5. NEEDS WORK — liquide-fonts

**Status**: ~1500 LOC · 0 tests · Font management subsystem  
**Issue**: Zero test coverage across catalog, families, config, index, install, collections, hot-reload  

### Implementation Plan (6 Phases, ~4100 LOC tests, 76 tests)

| Phase | Scope | Tests |
|-------|-------|-------|
| **P1: Catalog Core** | Add/remove families, dedup, lookup by name/style | 15 |
| **P2: Family & Matching** | Style matching, weight/width/slant selection, fallback chains | 12 |
| **P3: Config** | Font preferences TOML loading, default font stacks, DPI scaling | 10 |
| **P4: Index** | Font file scanning, metadata extraction, index persistence | 12 |
| **P5: Install & Collections** | User font installation, collection grouping, Google Fonts stub | 15 |
| **P6: Hot Reload** | File watcher integration, catalog refresh, subscriber notification | 12 |

### Critical Issues

- `catalog.remove_family()` — potential index corruption if family not found (no bounds check)
- `hot_reload` module — declared but not implemented
- `google_fonts` — no network code; stub only

---

## 6. NEEDS WORK — liquide-client

**Status**: Remote desktop client with stub protocol handshake  
**Issue**: 4 TODOs in connection establishment — TLS, protocol handshake, auth, capability negotiation  

### Implementation Plan (5 Phases, ~355 LOC)

| Phase | Scope | LOC |
|-------|-------|-----|
| **P1: TLS Handshake** | Complete `rustls` client config, certificate validation, ALPN negotiation | ~80 |
| **P2: Protocol Handshake** | `ClientHello` / `ServerHello` exchange using `liquide-protocol` wire types | ~80 |
| **P3: Auth Exchange** | `LoginPrompt` / `LoginResponse` flow, credential forwarding | ~60 |
| **P4: Capability Negotiation** | Encode/decode client capabilities (display, audio, input, clipboard, USB) | ~75 |
| **P5: Runtime Integration** | Wire handshake into connection state machine, timeout handling, reconnection | ~60 |

### Wire Protocol Types (from `liquide-protocol`)

```
ClientHello { version, capabilities, display_info }
ServerHello { server_version, session_id, accepted_capabilities }
LoginPrompt { methods: Vec<AuthMethod> }
LoginResponse { method, credentials }
```

---

## 7. NEEDS WORK — liquide-gateway

**Status**: ~2100 LOC · Gateway relay server with stub `handle_tcp_connection()`  
**Issue**: Steps 4-25 in connection handler are entirely stubbed  

### Implementation Plan (4 Phases, ~2830 LOC)

| Phase | Scope | LOC |
|-------|-------|-----|
| **P1: Frame I/O Layer** | TLS acceptor (rustls), frame codec (length-prefixed), connection lifecycle | ~600 |
| **P2: Protocol Handshake** | Server-side `ServerHello` response, version negotiation, capability exchange | ~400 |
| **P3: Auth & Session** | PAM integration, JWT token issuance, session routing to backend servers | ~800 |
| **P4: Relay & Broker** | Frame relay mode (transparent forwarding), broker mode (session multiplexing), health checks | ~1030 |

### Architecture

```
Client → [TLS] → Gateway → [Auth] → Session Router → Backend Server
                          → [Relay] → Direct Forward
                          → [Broker] → Multiplexed Sessions
```

### Key Design Decisions

- PAM for local auth, JWT for token-based session auth
- Frame relay mode for single-server deployments
- Broker mode for multi-server farms with session affinity

---

## 8. NEEDS WORK — liquide-render-coordinator

**Status**: Render task coordinator with stub `execute_task()`  
**Issue**: `execute_task()` just sleeps for 100μs instead of dispatching to actual renderers  

### Implementation Plan (4 Phases)

| Phase | Scope |
|-------|-------|
| **P1: RenderBackend Trait** | Define `trait RenderBackend { fn render_tile(...) -> TileResult; }` |
| **P2: Backend Integration** | Implement trait for CPU (liquide-renderer-cpu), GPU (liquide-renderer-gpu), wgpu (liquide-renderer-wgpu) |
| **P3: Task Dispatch** | Replace stub `execute_task()` with real dispatch: damage rect → tile list → backend.render_tile() → collect results |
| **P4: Scene Graph** | Accept scene graph from compositor, convert to render tasks, handle layer composition order |

### Critical Code

```rust
// CURRENT (stub):
fn execute_task(&self, _task: RenderTask) -> RenderResult {
    std::thread::sleep(Duration::from_micros(100));
    RenderResult::default()
}

// TARGET:
fn execute_task(&self, task: RenderTask) -> RenderResult {
    self.backend.render_tile(task.tile, task.damage, &task.scene)
}
```

---

## 9. NEEDS WORK — liquide-render-thread

**Status**: ~1200 LOC · Render worker threads  
**Issue**: Incremental damage tracking broken — `mark_all()` ignores damage parameter  

### Implementation Plan (6 Phases, ~1080 LOC, 22+ tests)

| Phase | Scope | LOC | Tests |
|-------|-------|-----|-------|
| **P1: Damage Tracking** | Fix `chrome_worker` and `content_worker` — both call `damage.mark_all()` ignoring the passed damage rects | ~200 | 5 |
| **P2: Buffer Pooling** | Pre-allocate tile buffers, reuse across frames to avoid allocation churn | ~250 | 4 |
| **P3: Panic Recovery** | Catch panics in render workers, restart thread, report error to coordinator | ~180 | 4 |
| **P4: Frame Pairing** | Add timeout for frame completion — if worker doesn't respond within deadline, mark frame as dropped | ~150 | 3 |
| **P5: Metrics** | Per-worker render time tracking, frame drop counters, backpressure signals | ~150 | 3 |
| **P6: Tests** | Integration tests with mock render backend, damage propagation validation | ~150 | 3 |

### Critical Bug

```rust
// CURRENT (broken):
fn chrome_worker(&self, damage: DamageRegion) {
    self.damage_tracker.mark_all(); // Ignores `damage` parameter!
}

// TARGET:
fn chrome_worker(&self, damage: DamageRegion) {
    self.damage_tracker.mark_region(damage); // Incremental
}
```

**Impact**: Every frame re-renders everything, wasting ~90% of GPU/CPU time on unchanged regions.

---

## 10. NEEDS WORK — liquide-plugin-abi

**Status**: ~400 LOC · Plugin ABI contract (FFI boundary)  
**Issue**: Zero tests for ABI stability, layout guarantees, or FFI safety  

### Implementation Plan (5 Phases, ~1500 LOC tests)

| Phase | Scope | Tests |
|-------|-------|-------|
| **P1: Layout Tests** | `repr(i32)` for `PluginResult`, `ResourceHandle(u64)` size/align assertions | 8 |
| **P2: ABI Compatibility** | Enum discriminant values, struct field offsets, function pointer signatures | 10 |
| **P3: JSON Round-Trip** | Serialize/deserialize all ABI types, verify no data loss | 8 |
| **P4: FFI Safety** | Null pointer handling, invalid enum variants, buffer overflow guards | 12 |
| **P5: Robustness** | Fuzzing with random bytes, stress test with rapid plugin load/unload | 6 |

### Key Assertions

```rust
#[test]
fn plugin_result_is_repr_i32() {
    assert_eq!(std::mem::size_of::<PluginResult>(), 4);
    assert_eq!(std::mem::align_of::<PluginResult>(), 4);
}

#[test]
fn resource_handle_is_u64() {
    assert_eq!(std::mem::size_of::<ResourceHandle>(), 8);
}
```

---

## 11. NEEDS WORK — liquide-font-rasterizer

**Status**: ~1800 LOC · Font rasterization engine  
**Issue**: Rasterization output never validated; color fonts and text shaping untested  

### Implementation Plan (3 Phases, ~1483 LOC tests, 88 tests)

| Phase | Scope | Tests |
|-------|-------|-------|
| **P1: Core Rasterization** | Glyph rendering output validation (non-empty bitmaps, correct dimensions, baseline alignment), subpixel rendering modes | 35 |
| **P2: Color Fonts** | COLR/CPAL table parsing, layered glyph rendering, emoji rasterization, SVG font handling | 25 |
| **P3: Text Shaping** | `rustybuzz` integration, ligature formation, kerning, BiDi text, combining marks, cluster analysis | 28 |

### Critical Gaps

- Rasterized glyph bitmaps are produced but never validated for correctness
- COLR/CPAL (color font) code paths have zero test coverage
- `rustybuzz` text shaping integration is exercised only through higher-level crates

---

## 12. NEEDS WORK — liquide-theme-css

**Status**: ~4200 LOC · CSS engine for desktop theming  
**Issue**: CSS variable resolution NOT IMPLEMENTED; specificity untested; complex selectors under-tested  

### Implementation Plan (6 Phases, ~5600 LOC, 168 tests)

| Phase | Scope | Tests |
|-------|-------|-------|
| **P1: CSS Variables** | `var()` function resolution, fallback values, cyclic detection, inherited custom properties | 30 |
| **P2: Specificity** | Specificity calculation for all selector types, `!important` override, cascade ordering | 25 |
| **P3: Complex Selectors** | Descendant/child/sibling combinators, attribute selectors, `:not()`, `:is()`, `:where()` | 28 |
| **P4: Cascade** | Full cascade algorithm — origin, specificity, order, layer ordering | 25 |
| **P5: Media/Supports** | `@media` condition evaluation (width, height, prefers-color-scheme, prefers-reduced-motion), `@supports` | 30 |
| **P6: Color Spaces** | HSL↔RGB, HWB, Lab, LCH, oklch, color-mix(), relative color syntax | 30 |

### Critical Gap: CSS Variable Resolution

```css
:root { --primary: #2196F3; }
.button { color: var(--primary); }  /* NOT RESOLVED */
```

The `var()` function is parsed but never substituted during style resolution. This breaks all theme customization.

### Code Quality Issues (6 categories)

1. Hardcoded color values instead of using theme variables
2. Missing `!important` handling in cascade
3. Selector specificity not computed (always 0)
4. No `@layer` support
5. `color-mix()` parsed but not evaluated
6. `@supports` conditions always return `true`

---

## 13. NEEDS WORK — liquide-devtools

**Status**: ~6200 LOC · 44 existing tests · Developer tools suite  
**Issue**: 7 modules have ZERO tests; test coverage insufficient for crate size  

### Implementation Plan (3 Phases, ~120-150 new tests)

| Phase | Scope | Tests |
|-------|-------|-------|
| **P1: Critical Modules** | `live_reload` (0 tests), console command execution (0 tests), rendering submodules (0 tests) | 50 |
| **P2: Input Handling** | Keyboard shortcuts, mouse interactions, panel focus management | 40 |
| **P3: Integration** | Full inspector lifecycle, DOM tree traversal, style panel updates, network panel recording | 30-60 |

### Modules with Zero Tests

| Module | LOC | Risk |
|--------|-----|------|
| `live_reload` | ~400 | HIGH — file watcher + hot swap |
| `keyboard` | ~200 | MEDIUM — input handling |
| `mouse` | ~150 | MEDIUM — input handling |
| `rendering` | ~300 | HIGH — visual output |
| `scene` | ~250 | MEDIUM — scene graph |
| `side_panels` | ~180 | LOW — UI layout |
| `types` | ~100 | LOW — data structures |

---

## 14. NEEDS WORK — liquide-shell

**Status**: Desktop shell with hardcoded values and unwrap panics in hot paths  
**Issue**: Frame delta hardcoded, pipeline cache unwraps, focus history unwrap, notification panel stub  

### Implementation Plan (4 Phases)

| Phase | Scope | Priority |
|-------|-------|----------|
| **P1: Fix Panics** | Replace 3 unwrap() calls in hot paths with proper error handling | P0 — Critical |
| **P2: Implement Stubs** | Notification panel toggle (no-op), mouse hover reveal | P1 — High |
| **P3: Extract Config** | Hardcoded UI dimensions → config constants | P2 — Medium |
| **P4: Parameterize** | Animation durations → configurable | P3 — Low |

### P1 Critical Fixes

| Location | Issue | Fix |
|----------|-------|-----|
| `threading.rs:146` | Frame delta hardcoded to `16ms` | Use `1_000_000 / refresh_rate` |
| `scene.rs:47` | Frame delta hardcoded to `16ms` | Same — derive from monitor refresh rate |
| `stages.rs:138-140` | Pipeline cache `.unwrap()` | Use `.unwrap_or_default()` or `if let` |
| `focus.rs:73` | Focus history `.unwrap()` | Return `None` if history empty |
| `workspace.rs:135` | Workspace count invariant violated | Add bounds check before access |

### P2 Stub Implementations

- **Notification panel toggle**: Currently a no-op comment. Wire to panel visibility state.
- **Mouse hover reveal**: Dock auto-hide on hover — connect to gesture/input events.

---

## 15. NEEDS WORK — liquide-hotkeys

**Status**: Cross-platform hotkey system — Windows ✅, Linux ✅, macOS ❌  
**Issue**: macOS event handler callback not installed — hotkeys register but never fire  

### Implementation Plan (4 Phases, ~750 LOC, 12 tests)

| Phase | Scope | LOC | Tests |
|-------|-------|-----|-------|
| **P1: Core Event Handler** | Add `InstallEventHandler` FFI, Carbon event callback, `mpsc::channel` bridge | ~140 | 4 |
| **P2: Event Polling** | Wire channel receiver to `poll()` — drain pending events | ~65 | 4 |
| **P3: Lifecycle** | App target validation, thread safety, multi-manager support | ~95 | 2 |
| **P4: Testing** | Platform-specific tests, cross-platform regression, documentation | ~450 | 2 |

### Platform Status Matrix

| Feature | Windows | Linux | macOS |
|---------|---------|-------|-------|
| Registration | ✅ RegisterHotKey | ✅ XGrabKey | ✅ RegisterEventHotKey |
| Event Polling | ✅ PeekMessageW | ✅ XCheckTypedEvent | ❌ **NOT IMPLEMENTED** |
| Media Keys | ✅ | ✅ XF86 keysyms | ❌ Not in Carbon API |

### Root Cause (macOS)

```rust
// macos.rs:351
// TODO: Install kEventHotKeyPressed handler via InstallEventHandler
// and use a channel or Arc<Mutex<Vec>> to collect triggered IDs.
```

`RegisterEventHotKey` succeeds but no `InstallEventHandler` callback is installed, so `poll()` always returns empty. Fix: Add Carbon event handler callback + `mpsc::channel` to bridge events.

### Fix Architecture

```
RegisterEventHotKey ──→ Carbon Runtime
                              │
InstallEventHandler ──→ hotkey_event_handler() callback
                              │
                        mpsc::Sender.send(hotkey_id)
                              │
poll() ←── mpsc::Receiver.try_recv() ──→ Vec<(HotkeyId, Action)>
```

---

## 16. NEEDS WORK — liquide-gestures

**Status**: ~1500 LOC · 50+ tests · Touch gesture recognition  
**Issue**: 2 production `unwrap()` calls in `recognizer.rs` process() hot path  

### Implementation Plan (4 Phases, ~230 LOC, 20+ tests)

| Phase | Scope | LOC | Tests |
|-------|-------|-----|-------|
| **P1: Fix Unwraps** | Replace 2x `.map().unwrap()` with pattern match + early return | ~35 | 6 |
| **P2: Test Coverage** | Malformed input tests, orphaned touch IDs, state machine invariants | ~80 | 10 |
| **P3: Error Infrastructure** | `GestureError` enum, diagnostic logging | ~60 | — |
| **P4: Integration** | Regression tests, fuzzing, performance validation | ~55 | 4+ |

### Critical Unwraps

**Unwrap #1** — `recognizer.rs:197` (EdgeSwiping state):
```rust
let start = self.active_touches.get(&point.id).map(|t| t.start).unwrap();
```

**Unwrap #2** — `recognizer.rs:218` (Tracking/Scrolling/Pinching/Swiping states):
```rust
let start = self.active_touches.get(&point.id).map(|t| t.start).unwrap();
```

**Fix**: Eliminate second lookup — store `start` from first `get_mut()` check, reuse in calculations. If touch ID not found, silently skip event (valid for malformed input from device drivers).

### Panic Scenarios

- Device driver sends `Move(id=3)` while only `id=1` and `id=2` are registered
- Race condition in multi-touch stream produces orphaned touch IDs
- Accessibility driver corruption sends out-of-order events

---

## 17. NEEDS WORK — liquide-telemetry-viewer

**Status**: ~2500 LOC · Performance monitoring with TUI/web/report modes  
**Issue**: NaN panics in report generation and dashboard rendering  

### Implementation Plan (4 Phases, ~166 LOC, 15+ tests)

| Phase | Scope | LOC | Tests |
|-------|-------|-----|-------|
| **P1: Fix NaN Panics** | Replace `.partial_cmp().unwrap()` with `total_cmp()` in export.rs, add infinity validation | ~13 | 5 |
| **P2: Dashboard Fix** | Filter NaN/infinity in y_max fold calculation | ~6 | 3 |
| **P3: Tests** | NaN handling, infinity, empty data, single value, sort stability | ~120 | 7+ |
| **P4: Documentation** | Module-level docs on floating-point edge case handling | ~27 | — |

### Critical Panics

**export.rs:101-102** — Sorting frame times:
```rust
// PANICS on NaN:
frame_times.sort_by(|a, b| a.partial_cmp(b).unwrap());

// FIX (Rust 1.62+):
frame_times.sort_by(|a, b| a.total_cmp(b));
```

**dashboard.rs:275** — Y-axis calculation:
```rust
// Silent data corruption with NaN:
let y_max = history.iter().cloned().fold(0.0f64, f64::max).max(20.0);

// FIX:
let y_max = history.iter().filter(|v| v.is_finite()).cloned().fold(20.0, f64::max);
```

### NaN Sources

- Frame time of 0μs → `1_000_000.0 / 0.0` = ∞ → stored in history → NaN propagation
- Custom telemetry sources reporting unavailable metrics as NaN
- Precision loss in microsecond-to-millisecond conversions

---

## 18. Aggregate Summary

### Total Effort Estimates

| Category | Crates | New Code LOC | New Test LOC | Total New LOC | New Tests |
|----------|--------|-------------|-------------|--------------|-----------|
| **BLOCKING** | 3 | ~2,585 | ~1,520 | ~4,105 | 175 |
| **NEEDS WORK** | 14 | ~5,440 | ~15,680 | ~21,120 | 545+ |
| **TOTAL** | **17** | **~8,025** | **~17,200** | **~25,225** | **720+** |

### Priority Order (Recommended)

| Priority | Crate | Justification |
|----------|-------|---------------|
| **P0** | liquide-renderer-wgpu | No GPU rendering — blocks visual output |
| **P0** | liquide-ctl | No CLI management — blocks operations |
| **P0** | liquide-policy | No policy enforcement — security gap |
| **P1** | liquide-render-coordinator | Stub dispatch — connected to renderer |
| **P1** | liquide-render-thread | Broken damage tracking — performance critical |
| **P1** | liquide-shell | Unwrap panics in hot paths — crash risk |
| **P1** | liquide-gestures | Unwrap panics in hot paths — crash risk |
| **P1** | liquide-telemetry-viewer | NaN panics — crash risk |
| **P2** | liquide-client | Protocol stubs — blocks remote desktop |
| **P2** | liquide-gateway | Protocol stubs — blocks remote desktop |
| **P2** | liquide-theme-css | CSS variables not resolved — breaks theming |
| **P2** | liquide-hotkeys | macOS non-functional — platform gap |
| **P3** | liquide-common | Zero tests — foundation crate risk |
| **P3** | liquide-fonts | Zero tests — font subsystem risk |
| **P3** | liquide-font-rasterizer | Output not validated — correctness risk |
| **P3** | liquide-plugin-abi | ABI not tested — plugin stability risk |
| **P3** | liquide-devtools | Low coverage — development tool risk |

### Risk Matrix

| Risk Level | Count | Crates |
|------------|-------|--------|
| 🔴 Critical | 3 | renderer-wgpu, ctl, policy |
| 🟠 High | 5 | render-coordinator, render-thread, shell, gestures, telemetry-viewer |
| 🟡 Medium | 4 | client, gateway, theme-css, hotkeys |
| 🟢 Low | 5 | common, fonts, font-rasterizer, plugin-abi, devtools |

### Quick Wins (< 1 day each)

1. **liquide-gestures** — 2 unwrap fixes (~35 LOC)
2. **liquide-telemetry-viewer** — NaN fixes (~19 LOC)
3. **liquide-shell** — 3 unwrap fixes + hardcoded frame delta (~50 LOC)
4. **liquide-render-thread** — `mark_all()` → `mark_region()` (~20 LOC)
