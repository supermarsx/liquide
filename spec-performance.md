# LiquiDE — Performance SLOs, Benchmarks & CI Gating

> **Status**: Living document
> **Related specs**: [Main Spec](spec.md) · [Protocol](spec-protocol-formal.md) · [Rendering](spec-rendering-software.md) · [Observability](spec-observability.md) · [Build](spec-build.md)

---

## 1) Purpose

This document defines explicit **Service Level Objectives (SLOs)** for LiquiDE, a reproducible **benchmark harness** for measuring performance, **reference workload profiles** for testing, and **CI gating rules** that prevent performance regressions from merging.

---

## 2) Service Level Objectives

### 2.1 End-to-End Latency SLOs

| Metric | SLO (LAN, <1ms RTT) | SLO (WAN, 50ms RTT) | SLO (Degraded, 100ms RTT, 2% loss) | Measurement Point |
|--------|---------------------|---------------------|--------------------------------------|-------------------|
| **Input-to-photon** (key press → pixel change visible on client) | p50 < 16ms, p99 < 25ms | p50 < 70ms, p99 < 120ms | p50 < 150ms, p99 < 300ms | Client-side instrumented tap-to-pixel |
| **First frame after connect** (auth complete → first frame rendered) | < 500ms | < 1000ms | < 2000ms | Client timestamp from AuthResult to first FrameData |
| **Reconnect-to-first-frame** (transport drop → first frame) | < 300ms | < 500ms | < 1500ms | Client timestamp from disconnect detection to first frame |
| **Cursor responsiveness** (mouse move → cursor position update) | p50 < 5ms, p99 < 10ms | p50 < 5ms + RTT, p99 < 15ms + RTT | p50 < 10ms + RTT | Client-side cursor position delta |
| **Clipboard (text <1MB)** | < 50ms | < 100ms + RTT | < 300ms + RTT | Client-side paste-complete time |
| **Audio end-to-end** | < 30ms | < 30ms + RTT | < 50ms + RTT | Audio loopback measurement |

### 2.2 Throughput SLOs

| Metric | SLO | Conditions |
|--------|-----|-----------|
| Frame rate (1080p, active) | ≥ 60 fps sustained | LAN, balanced profile, standard workload |
| Frame rate (4K, active) | ≥ 30 fps sustained | LAN, balanced profile |
| Frame rate (1080p, idle) | 0 fps (no frames when no damage) | All conditions |
| Tile skip ratio (static screen) | > 99% | Cursor blink only |
| Tile delta bandwidth savings | ≥ 60% vs full tiles | Typical UI workload |
| Encode throughput (H.264, 1080p) | ≥ 60 fps | Single-core software, balanced quality |
| Encode throughput (AV1, 1080p) | ≥ 30 fps | Multi-core software, balanced quality |
| Frame rate (1080p, 10-bit WCG) | ≥ 30 fps sustained | LAN, H.265 Main 10 or AV1 10-bit, software encode |
| Frame rate (1080p, 10-bit WCG, HW) | ≥ 60 fps sustained | LAN, hardware encoder (VAAPI/NVENC), 10-bit profile |
| Frame rate (4K, 10-bit WCG) | ≥ 24 fps sustained | LAN, hardware encoder recommended |
| Encode throughput (H.265 Main 10, 1080p) | ≥ 30 fps | Hardware encoder (VAAPI/NVENC). Software ≥ 15 fps. |
| Encode throughput (AV1 10-bit, 1080p) | ≥ 30 fps | Multi-core software (SVT-AV1). Hardware ≥ 60 fps. |

### 2.3 Resource Consumption SLOs

| Resource | SLO (1080p, 60fps, balanced) | SLO (idle, no damage) | SLO (4K, 30fps, balanced) | Max Hard Limit |
|----------|--------|------|--------|-----|
| **CPU** (server, per session) | < 2 cores | < 1% of 1 core | < 4 cores | 6 cores (cgroup) |
| **Memory** (server, per session, no apps) | < 200 MB | < 100 MB | < 350 MB | 512 MB (cgroup) |
| **Memory** (10-bit WCG/HDR session, no apps) | < 300 MB | < 150 MB | < 500 MB | 768 MB (cgroup) |
| **Memory** (per WASM plugin) | < 16 MB typical | < 2 MB | < 16 MB | 256 MB (configurable) |
| **Network bandwidth** (1080p, balanced) | 2–8 Mbps | < 10 Kbps | 8–20 Mbps | 50 Mbps |
| **Disk I/O** (session runtime) | < 1 MB/s sustained | ~0 | < 2 MB/s | 10 MB/s |
| **File descriptors** (per session) | < 200 | < 100 | < 300 | 1024 (ulimit) |

### 2.3a Memory Pool Architecture

| Pool | Budget (1080p, 60fps) | Budget (4K, 30fps) | Eviction Policy | Notes |
|------|----------------------|--------------------|--------------------|-------|
| Frame buffers (double-buffered) | 16 MB (2 × 1920×1080×4) | 64 MB (2 × 3840×2160×4) | N/A (fixed allocation) | Pre-allocated at session start. Never freed during session. |
| Glyph atlas | 4 MB initial, 64 MB max | 8 MB initial, 64 MB max | LRU per (font, size, hinting) slab | Auto-grows when cache miss rate > 1% per frame. Shrinks under memory pressure. |
| Shadow cache | 8 MB (LRU, 64 entries) | 8 MB | LRU by last-use time | Geometry-keyed. Shared across identical shadow params. |
| Blur scratch buffers | 2 MB | 4 MB | N/A (reused per-frame) | Allocated once, reused. Sized for largest active blur surface. |
| Tile hash table | 32 KB (1080p: 510 tiles × 8B) | 128 KB (4K: 2040 tiles × 8B) | N/A (fixed per grid) | CRC-32C per tile for damage detection. Resized on resolution change. |
| Compression scratch (per thread) | 2 MB × encode_threads | 2 MB × encode_threads | N/A (fixed) | Zstd/LZ4 working memory. One buffer per encoder thread. |
| Channel send/receive queues | 10 MB total | 15 MB total | Oldest-first drop under pressure | Partitioned across channels by priority. |
| **Total baseline** | **~42 MB** | **~101 MB** | | Excludes application memory. |

**Memory pressure response:**

| RSS Threshold | Action |
|--------------|--------|
| > 80% of cgroup limit | Evict LRU shadow cache entries. Shrink glyph atlas to 50% of current size. Emit `liquide_memory_pressure` metric. |
| > 90% of cgroup limit | Shrink glyph atlas to minimum (4 MB). Disable compression scratch (fall back to single-threaded encode). Reduce channel queue sizes by 50%. |
| > 95% of cgroup limit | Kill lowest-priority WASM plugins. Disable all blur effects (free scratch buffers). Alert via `liquide_memory_critical` metric. |

### 2.3b CPU Budget Partitioning

| Thread Class | Budget (1080p, 60fps) | Budget (4K, 30fps) | Priority | Overbudget Action |
|-------------|----------------------|--------------------|----------|--------------------|
| **Compositor** (scene walk + composite) | 80% of 1 core (~13ms at 60fps) | 80% of 2 cores | Normal | Triggers degradation ladder (see [spec-rendering-software.md](spec-rendering-software.md) §7) |
| **Encoder** (video/tile encode) | 2 threads, 150% combined | 4 threads, 300% combined | Normal-1 | ABR control loop reduces FPS or quality (see spec.md §9 ABR) |
| **Transport I/O** (send/recv, TLS, framing) | 30% of 1 core | 30% of 1 core | Normal | Backpressure propagates to encoder (pause frames) |
| **Audio** (encode/decode, jitter buffer) | 10% of 1 core | 10% of 1 core | RT (SCHED_RR) | Audio never yields to other threads. Underrun metrics emitted. |
| **Plugin runtime** (WASM execution) | Per-plugin fuel limit | Per-plugin fuel limit | Normal-2 | Plugin faulted (fuel exhausted), disabled for session duration |

**Guardrails:**
- If the encoder consistently exceeds budget (>3 frames), the ABR control loop reduces target FPS or increases quantization.
- If the compositor consistently exceeds budget (>3 frames), the degradation ladder descends one level.
- Audio thread uses real-time scheduling (`SCHED_RR`, priority 50) to avoid starvation. If the system cannot provide RT scheduling (container without `CAP_SYS_NICE`), audio uses `SCHED_FIFO` as fallback, or normal priority with elevated niceness.

### 2.4 Startup & Shutdown SLOs

| Metric | SLO | Notes |
|--------|-----|-------|
| Server daemon start (`liquid-desktopd` ready) | < 2s | From `systemctl start` to accepting connections |
| Session spawn (supervisor → session ready) | < 500ms | From auth success to first frame capable |
| Session shutdown (graceful) | < 1s | From logout to process exit |
| Session crash restart | < 2s (first attempt) | From crash detection to new session ready |
| Plugin load (per plugin) | < 200ms | WASM compile + init |
| Benchmark calibration | < 5s | Auto-calibration on session start |

---

## 3) Reference Workload Profiles

The benchmark harness uses standardized workload profiles that simulate real user activity.

### 3.1 Workload Definitions

| Profile | Description | Activity Pattern | Expected Characteristics |
|---------|-------------|-----------------|------------------------|
| **idle** | Login, no interaction | No input, no surface commits | 0 fps, <1% CPU, <10 Kbps network |
| **text-editing** | Terminal or code editor with continuous typing | 120 WPM typing, cursor blink, scrollback buffer, syntax highlighting updates | 30-60 fps during typing bursts, tile mode dominant, high skip ratio during pauses |
| **web-browsing** | Browser with page loads, scrolling, video embed | Page loads (full-screen damage), smooth scroll (tile scroll optimization), embedded video (hybrid mode) | Mixed video+tile, 30-60 fps, bandwidth bursts on page load |
| **document** | Word processor / PDF viewer | Typing, page scroll, format changes | Tile mode dominant, moderate bandwidth |
| **video-playback** | Full-screen video (1080p, 30fps source) | Continuous motion, full-screen damage | Video mode, 30 fps lock, 5-15 Mbps sustained |
| **desktop-workflow** | Multi-window: terminal + browser + file manager | Mixed interaction across windows, window moves/resizes, launcher usage, notification toasts | Hybrid mode, damage in multiple regions, window management overhead |
| **dashboard** | Monitoring dashboard with live charts | Periodic small updates (chart ticks), mostly static layout | High skip ratio, small tile batches, very low bandwidth |
| **presentation** | Slide deck in presentation mode | Full-screen slide transitions, embedded media, pointer movement | Full-screen damage on transitions, low bandwidth between slides |

### 3.2 Workload Replay Format

Workloads are recorded as `.lqw` (LiquiDE Workload) files — a sequence of timestamped events:

```
# Format: timestamp_us event_type event_data
0         surface_commit shell-dock 1920×48 damage=full
0         surface_commit desktop-bg 1920×1080 damage=none
16666     input key_down scancode=31 keysym=0x0073  # 's' key
16700     surface_commit terminal 800×600 damage=rect(0,580,800,20)
33333     input key_up scancode=31
33400     surface_commit terminal 800×600 damage=rect(0,580,800,20)
# ... continues for duration of workload
```

The benchmark harness replays these events into the compositor using a synthetic Wayland client that commits surfaces according to the recorded pattern.

### 3.3 Network Emulation Profiles

| Profile Name | RTT | Bandwidth | Packet Loss | Jitter | Use Case |
|-------------|-----|-----------|-------------|--------|----------|
| `lan` | 1ms | 1 Gbps | 0% | 0ms | Office LAN |
| `datacenter` | 5ms | 1 Gbps | 0% | 1ms | Same-region cloud |
| `wan-good` | 30ms | 100 Mbps | 0.1% | 5ms | Same-continent WAN |
| `wan-cross` | 100ms | 50 Mbps | 0.5% | 10ms | Cross-continent |
| `cellular-4g` | 50ms | 20 Mbps | 1% | 20ms | Mobile tethering |
| `cellular-3g` | 100ms | 2 Mbps | 2% | 50ms | Degraded mobile |
| `hotel-wifi` | 50ms | 5 Mbps | 3% | 30ms | Congested hotel Wi-Fi |
| `satellite` | 600ms | 10 Mbps | 1% | 10ms | Satellite link |

Network emulation is applied via `tc` (Linux traffic control) or `netem`.

> **Cross-reference:** The test harness implementation details, scenario configuration, and success criteria are specified in [spec.md](spec.md) §8 Network Condition Test Harness. The `liquide-bench` CLI invokes these profiles via `--network <profile>`.

---

## 4) Benchmark Harness

### 4.1 Architecture

```
┌──────────────────────────────────────────────────────────┐
│                    liquide-bench                          │
│                                                          │
│  ┌────────────┐  ┌────────────────┐  ┌───────────────┐  │
│  │  Workload  │  │  Network       │  │  Measurement  │  │
│  │  Replayer  │  │  Emulator      │  │  Collector    │  │
│  │  (synthetic│  │  (tc/netem)    │  │  (metrics     │  │
│  │   Wayland  │  │                │  │   aggregator) │  │
│  │   client)  │  │                │  │               │  │
│  └─────┬──────┘  └───────┬────────┘  └──────┬────────┘  │
│        │                 │                   │           │
│        ▼                 ▼                   ▼           │
│  ┌─────────────────────────────────────────────────────┐ │
│  │  liquid-session (under test)                        │ │
│  └─────────────────────────────────────────────────────┘ │
│        │                                                  │
│        ▼                                                  │
│  ┌─────────────────────────────────────────────────────┐ │
│  │  liquidclient (instrumented, headless)               │ │
│  │  - Measures decode time, render time                 │ │
│  │  - Measures input-to-photon (screen capture)         │ │
│  │  - Records bandwidth, frame rate, skip ratio         │ │
│  └─────────────────────────────────────────────────────┘ │
│                                                          │
│  ┌────────────────┐                                      │
│  │  Report        │  JSON + HTML report                  │
│  │  Generator     │  CI-parseable metrics                │
│  └────────────────┘                                      │
└──────────────────────────────────────────────────────────┘
```

### 4.2 CLI Interface

```bash
# Run full benchmark suite
liquide-bench --suite all --network lan --output report.json

# Run specific workload
liquide-bench --workload text-editing --network wan-good --duration 60s

# Run regression test (compare against baseline)
liquide-bench --suite ci --baseline baseline.json --output comparison.json

# Run with custom server config
liquide-bench --suite all --server-config /path/to/server.toml

# List available workloads and network profiles
liquide-bench --list
```

### 4.3 Output Format

```json
{
  "benchmark_version": "1.0",
  "timestamp": "2025-06-15T14:00:00Z",
  "system": {
    "cpu": "AMD EPYC 7763 64-Core",
    "cores": 8,
    "ram_gb": 32,
    "gpu": "none",
    "os": "Ubuntu 24.04",
    "kernel": "6.5.0-44-generic",
    "liquide_version": "0.3.0",
    "liquide_commit": "abc1234"
  },
  "results": [
    {
      "workload": "text-editing",
      "network": "lan",
      "duration_sec": 60,
      "metrics": {
        "input_to_photon_p50_ms": 12.3,
        "input_to_photon_p99_ms": 18.7,
        "fps_avg": 58.2,
        "fps_min": 42,
        "fps_p1": 45,
        "cpu_avg_percent": 85,
        "cpu_peak_percent": 145,
        "memory_avg_mb": 156,
        "memory_peak_mb": 198,
        "bandwidth_avg_mbps": 3.2,
        "bandwidth_peak_mbps": 12.1,
        "tile_skip_ratio": 0.87,
        "tile_delta_ratio": 0.65,
        "encode_time_p50_ms": 2.1,
        "encode_time_p99_ms": 4.8,
        "degradation_level_max": "L0",
        "frame_drops": 3,
        "errors": 0
      }
    }
  ]
}
```

### 4.4 Input-to-Photon Measurement

The most critical SLO — measured via the **screen-change detection method**:

1. Benchmark client sends a keystroke (e.g., type character 'X').
2. Client starts a high-resolution timer.
3. Client polls/waits for the decoded frame that contains the character 'X' at the expected cursor position.
4. Timer stops when the pixel at the expected position changes to match the expected glyph.
5. Delta = input-to-photon latency.

This method captures the **true end-to-end latency** including: client input encoding → network → server input processing → compositor render → encode → network → client decode → client render.

---

## 5) CI Performance Gating

### 5.1 CI Benchmark Configuration

The CI pipeline runs a reduced benchmark suite on every PR and full suite on merge to main:

| Trigger | Suite | Network Profiles | Workloads | Duration |
|---------|-------|-----------------|-----------|----------|
| Pull Request | `ci-quick` | `lan` only | `text-editing`, `idle`, `desktop-workflow` | 15s each |
| Merge to main | `ci-full` | `lan`, `wan-good`, `hotel-wifi` | All 8 profiles | 60s each |
| Nightly | `ci-nightly` | All 8 profiles | All 8 profiles | 120s each |
| Release candidate | `ci-release` | All 8 profiles | All 8 profiles + stress variants | 300s each |

### 5.2 Regression Detection

Each metric is compared against a **rolling baseline** (last 10 merge-to-main benchmark results, trimmed mean).

| Metric Category | Regression Threshold (PR Block) | Warning Threshold |
|----------------|-------------------------------|-------------------|
| Input-to-photon p50 | > 15% increase | > 8% increase |
| Input-to-photon p99 | > 25% increase | > 12% increase |
| FPS average | > 10% decrease | > 5% decrease |
| CPU average | > 20% increase | > 10% increase |
| Memory average | > 20% increase | > 10% increase |
| Memory peak | > 30% increase | > 15% increase |
| Bandwidth average | > 25% increase | > 12% increase |
| Encode time p50 | > 15% increase | > 8% increase |
| Binary size | > 10% increase | > 5% increase |
| Session start time | > 20% increase | > 10% increase |
| Plugin load time | > 30% increase | > 15% increase |

### 5.3 Gating Rules

1. **PR blocking**: if any metric exceeds the regression threshold, the PR is blocked with a clear message identifying which metrics regressed and by how much.
2. **Warning**: if any metric exceeds the warning threshold but not the regression threshold, a comment is posted on the PR with the warning. Merge is allowed.
3. **Baseline update**: on merge to main, the benchmark result is added to the rolling baseline window (last 10 runs).
4. **Flake detection**: if the same PR passes/fails inconsistently across 2 re-runs, it is flagged as a potential flake. The baseline is investigated.
5. **Override**: a maintainer can override a regression gate with a comment containing `[perf-override: reason]`. The override is logged.

### 5.4 CI Hardware Requirements

| Component | Specification | Notes |
|-----------|--------------|-------|
| CPU | 8-core x86_64 with AVX2 | Representative of typical server VM |
| RAM | 16 GB | Sufficient for session + benchmark overhead |
| Disk | SSD (NVMe preferred) | Consistent I/O for reproducible results |
| GPU | None | Software rendering path (primary target) |
| Network | Loopback + tc/netem | Emulated network conditions |
| OS | Ubuntu 24.04 LTS (or latest LTS) | Consistent baseline |

Performance numbers are **not portable** across different CI hardware. Baselines are hardware-specific.

---

## 6) Auto-Calibration Benchmark

On session start, the server runs a fast calibration benchmark to determine hardware capability:

### 6.1 Calibration Tests

| Test | Duration | Measures | Used For |
|------|----------|----------|----------|
| Single-core blur throughput | <1s | Blur ops/sec at 1080p | Effect budget calculation |
| Multi-core composite throughput | <1s | Composite ops/sec | Frame budget, max FPS |
| Encode throughput (H.264, software) | <1s | Frames/sec at target resolution | Codec selection, quality preset |
| Memory bandwidth | <0.5s | GB/s | Tile copy optimization |
| SIMD capability detection | <0.1s | AVX2/AVX512/NEON availability | Code path selection |

### 6.2 Profile Selection

Based on calibration results, the server auto-selects a rendering profile:

| Calibration Score | Selected Profile | Expected Hardware |
|-------------------|-----------------|-------------------|
| blur ≥ 500 ops/s, composite ≥ 120 fps | `quality` | 8+ cores, modern CPU |
| blur ≥ 200 ops/s, composite ≥ 60 fps | `balanced` | 4-8 cores, mid-range CPU |
| blur ≥ 50 ops/s, composite ≥ 30 fps | `performance` | 2-4 cores, budget CPU |
| blur < 50 ops/s | `minimal` | 1-2 cores, low-power |

Admin can override auto-detection via `rendering.profile = "balanced"` in config.

---

## 7) Performance Monitoring Integration

Real-time performance data is exposed via:

- **`liquidctl session perf`** — live performance dashboard in terminal
- **Stream analysis overlay** — client-side transparent overlay showing frame rate, latency, bandwidth, degradation level
- **Prometheus metrics** — all SLO metrics exported (see [spec-observability.md](spec-observability.md))
- **Structured logs** — performance events logged at `debug` level with measurement data

### 7.1 SLO Violation Alerts

When SLOs are violated during runtime:

| Violation | Log Level | Action |
|-----------|-----------|--------|
| Input-to-photon p99 > SLO for 30s | `warn` | Emit metric, log warning |
| FPS < 50% of target for 10s | `warn` | Emit metric, trigger degradation ladder |
| CPU > 90% of cgroup limit for 60s | `warn` | Emit metric, consider reducing max FPS |
| Memory > 80% of cgroup limit | `warn` | Emit metric, trigger plugin memory audit |
| Memory > 95% of cgroup limit | `error` | Emit metric, kill lowest-priority plugins, reduce caches |
| Bandwidth > 90% of estimated link | `info` | Emit metric, increase compression |

---

## 7a) Capacity Planning Reference

### Per-Session Resource Cost by Workload Profile

| Workload Profile | CPU (p50) | CPU (p95) | Memory (p50) | Memory (p95) | Bandwidth (p50) | Bandwidth (p95) |
|-----------------|-----------|-----------|-------------|-------------|-----------------|-----------------|
| **idle** | 0.01 cores | 0.02 cores | 80 MB | 100 MB | 5 Kbps | 10 Kbps |
| **text-editing** | 0.8 cores | 1.5 cores | 150 MB | 200 MB | 2 Mbps | 5 Mbps |
| **web-browsing** | 1.2 cores | 2.0 cores | 180 MB | 250 MB | 5 Mbps | 12 Mbps |
| **document** | 0.6 cores | 1.2 cores | 140 MB | 190 MB | 1.5 Mbps | 4 Mbps |
| **video-playback** | 1.5 cores | 2.5 cores | 200 MB | 280 MB | 8 Mbps | 15 Mbps |
| **desktop-workflow** | 1.0 cores | 2.0 cores | 170 MB | 230 MB | 3 Mbps | 8 Mbps |
| **dashboard** | 0.3 cores | 0.8 cores | 120 MB | 160 MB | 0.5 Mbps | 2 Mbps |
| **presentation** | 0.5 cores | 1.5 cores | 160 MB | 220 MB | 2 Mbps | 10 Mbps |

### Sizing Formula

```
Required CPU cores = sum(sessions × cpu_p95_per_profile) × 1.2 (reconnect storm headroom)
Required RAM       = sum(sessions × mem_p95_per_profile) × 1.3 (reconnect storm headroom + OS/daemon overhead)
Required bandwidth = sum(sessions × bw_p95_per_profile) × 1.1 (protocol overhead)
```

The **reconnect storm headroom** accounts for the burst when a network event causes many clients to reconnect simultaneously: all sessions momentarily demand peak CPU (re-encoding keyframes) and peak memory (re-allocating decode buffers). Budget 20% extra CPU and 30% extra memory above steady-state peak.

### Quick-Reference Sizing Table

| Server Spec | Max Sessions (text-editing) | Max Sessions (desktop-workflow) | Max Sessions (video-playback) |
|------------|---------------------------|-------------------------------|------------------------------|
| 8 cores / 32 GB | 5 | 4 | 3 |
| 16 cores / 64 GB | 10 | 8 | 6 |
| 32 cores / 128 GB | 21 | 16 | 12 |
| 64 cores / 256 GB | 42 | 32 | 25 |

These estimates assume: balanced rendering profile, 1080p, software encoding, no GPU. Hardware encoder availability roughly doubles session density for video-heavy workloads. See also [spec.md](spec.md) §14 Capacity Planning Formulas for the detailed per-session cost model.

---

## 8) Test Plan

### Benchmark Harness
- Verify `liquide-bench` runs all workload profiles without error.
- Verify JSON output format matches schema.
- Verify input-to-photon measurement accuracy against known injected latency (±2ms tolerance).
- Verify network emulation profiles produce expected RTT and loss characteristics.
- Verify regression detection correctly identifies regressions above threshold.
- Verify regression detection ignores variations below threshold.

### SLO Validation
- Verify all SLOs are met on reference hardware (8-core, AVX2, no GPU) under LAN profile for each workload.
- Verify SLO violation alerts are emitted when thresholds are exceeded.
- Verify auto-calibration selects appropriate profile for different hardware capabilities.

### CI Integration
- Verify PR pipeline runs `ci-quick` suite and blocks on regression.
- Verify merge pipeline runs `ci-full` suite and updates baseline.
- Verify override mechanism works with `[perf-override: reason]` comment.
- Verify flake detection flags inconsistent pass/fail results.
