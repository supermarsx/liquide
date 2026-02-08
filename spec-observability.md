# LiquiDE — Observability: Metrics, Traces, Logs & Runbooks

> **Status**: Living document
> **Related specs**: [Main Spec](spec.md) · [Performance](spec-performance.md) · [Threat Model](spec-threat-model.md) · [Normative Conventions](spec-normative.md)

---

## 1) Purpose

This document defines the complete observability stack for LiquiDE: a **metrics catalog** with cardinality rules, **distributed trace spans** for request flow tracking, a **structured log schema** consistent across all components, and **troubleshooting runbooks** for common operational issues.

---

## 2) Metrics Catalog

All metrics are exposed via Prometheus exposition format on a per-component HTTP endpoint.

### 2.1 Metric Endpoints

| Component | Default Endpoint | Auth |
|-----------|-----------------|------|
| `liquid-desktopd` (supervisor) | `http://127.0.0.1:9400/metrics` | Optional bearer token |
| `liquid-session` (per-session) | `http://127.0.0.1:9401/metrics` (port per session) | Optional bearer token |
| `liquid-gateway` | `http://127.0.0.1:9402/metrics` | Optional bearer token |
| `liquid-manager` | `http://127.0.0.1:9403/metrics` | Optional bearer token |

### 2.2 Metric Naming Convention

All metrics use the prefix `liquide_` and follow Prometheus naming conventions:

```
liquide_<subsystem>_<metric_name>_<unit>
```

Examples: `liquide_compositor_frame_time_seconds`, `liquide_transport_bytes_sent_total`, `liquide_session_count`.

### 2.3 Supervisor Metrics (`liquid-desktopd`)

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `liquide_supervisor_sessions_total` | gauge | `state` (running, suspended, crashed, failed) | Current session count by state |
| `liquide_supervisor_session_starts_total` | counter | `result` (success, failure) | Total session spawn attempts |
| `liquide_supervisor_session_crashes_total` | counter | `signal`, `exit_code` | Total session crashes |
| `liquide_supervisor_session_restarts_total` | counter | | Total session restart attempts |
| `liquide_supervisor_session_uptime_seconds` | histogram | | Session uptime distribution |
| `liquide_supervisor_heartbeat_misses_total` | counter | `session_id` | Heartbeat misses per session |
| `liquide_supervisor_auth_attempts_total` | counter | `method`, `result` (success, failure) | Authentication attempts |
| `liquide_supervisor_connections_active` | gauge | `transport` | Active client connections |
| `liquide_supervisor_connections_total` | counter | `transport`, `result` | Total connection attempts |
| `liquide_supervisor_cgroup_memory_bytes` | gauge | `session_id` | Per-session cgroup memory usage |
| `liquide_supervisor_cgroup_cpu_usage_seconds_total` | counter | `session_id` | Per-session cgroup CPU time |
| `liquide_supervisor_process_cpu_seconds_total` | counter | | Supervisor process CPU time |
| `liquide_supervisor_process_memory_bytes` | gauge | | Supervisor process RSS |

### 2.4 Session Metrics (`liquid-session`)

#### Compositor

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `liquide_compositor_frame_time_seconds` | histogram | `profile` | Frame composite + effect time |
| `liquide_compositor_frame_rate` | gauge | | Current effective FPS |
| `liquide_compositor_frame_drops_total` | counter | | Frames skipped due to budget overrun |
| `liquide_compositor_damage_tiles` | histogram | | Damaged tiles per frame |
| `liquide_compositor_damage_ratio` | gauge | | Fraction of screen damaged per frame |
| `liquide_compositor_degradation_level` | gauge | | Current degradation level (0=L0, 13=L13) |
| `liquide_compositor_blur_time_seconds` | histogram | `quality` | Backdrop blur time |
| `liquide_compositor_blur_cache_hits_total` | counter | | Blur cache hit count |
| `liquide_compositor_blur_cache_misses_total` | counter | | Blur cache invalidations |
| `liquide_compositor_shadow_cache_size` | gauge | | Cached shadow textures count |
| `liquide_compositor_surfaces_total` | gauge | `type` (toplevel, popup, subsurface, layer) | Wayland surfaces by type |

#### Encoding

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `liquide_encode_time_seconds` | histogram | `codec`, `mode` (video, tile) | Per-frame/batch encode time |
| `liquide_encode_bytes_total` | counter | `codec`, `mode` | Total encoded bytes |
| `liquide_encode_frames_total` | counter | `codec`, `type` (key, delta) | Encoded frames by type |
| `liquide_encode_tile_skip_ratio` | gauge | | Fraction of tiles skipped (unchanged) |
| `liquide_encode_tile_delta_ratio` | gauge | | Fraction of tiles sent as XOR delta |
| `liquide_encode_tile_full_ratio` | gauge | | Fraction of tiles sent as full |
| `liquide_encode_tile_copy_ratio` | gauge | | Fraction of tiles sent as copy |
| `liquide_encode_tile_solid_ratio` | gauge | | Fraction of tiles sent as solid fill |
| `liquide_encode_bandwidth_savings_ratio` | gauge | | Delta savings vs full frames |
| `liquide_encode_quality_score` | gauge | `codec` | Encoder quality metric (SSIM or VMAF) |

#### Transport

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `liquide_transport_bytes_sent_total` | counter | `channel`, `transport` | Bytes sent per channel |
| `liquide_transport_bytes_received_total` | counter | `channel`, `transport` | Bytes received per channel |
| `liquide_transport_packets_sent_total` | counter | `transport` | Packets sent |
| `liquide_transport_packets_lost_total` | counter | `transport` | Packets lost (detected) |
| `liquide_transport_rtt_seconds` | histogram | `transport` | Round-trip time |
| `liquide_transport_send_queue_bytes` | gauge | | Current send queue size |
| `liquide_transport_send_queue_ratio` | gauge | | Send queue fullness (0.0–1.0) |
| `liquide_transport_backpressure_events_total` | counter | `level` (80, 90, 95, 100) | Backpressure threshold triggers |
| `liquide_transport_bandwidth_estimate_bps` | gauge | | Estimated available bandwidth |
| `liquide_transport_switches_total` | counter | `from`, `to` | Transport switches |

#### Input

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `liquide_input_events_total` | counter | `type` (key, mouse_move, mouse_button, touch, scroll) | Input events processed |
| `liquide_input_processing_time_seconds` | histogram | | Input event processing latency |
| `liquide_input_coalesce_total` | counter | `type` | Coalesced input events |

#### Clipboard

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `liquide_clipboard_transfers_total` | counter | `direction` (s2c, c2s), `result` (success, blocked, cancelled, timeout) | Clipboard transfer outcomes |
| `liquide_clipboard_transfer_bytes_total` | counter | `direction` | Total clipboard bytes transferred |
| `liquide_clipboard_transfer_time_seconds` | histogram | `direction` | Transfer duration |

#### Plugins

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `liquide_plugin_loaded` | gauge | `plugin_id`, `version` | Currently loaded plugins |
| `liquide_plugin_calls_total` | counter | `plugin_id`, `extension_point` | Plugin invocations |
| `liquide_plugin_call_time_seconds` | histogram | `plugin_id`, `extension_point` | Plugin call duration |
| `liquide_plugin_faults_total` | counter | `plugin_id`, `fault_type` | Plugin faults |
| `liquide_plugin_memory_bytes` | gauge | `plugin_id` | Plugin WASM memory usage |
| `liquide_plugin_fuel_consumed_total` | counter | `plugin_id` | CPU fuel consumed |

#### Audio

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `liquide_audio_buffer_level_seconds` | gauge | `direction` (playback, capture) | Jitter buffer fill level |
| `liquide_audio_underruns_total` | counter | `direction` | Buffer underrun count |
| `liquide_audio_overruns_total` | counter | `direction` | Buffer overrun count |
| `liquide_audio_latency_seconds` | gauge | `direction` | End-to-end audio latency |

### 2.5 Gateway Metrics (`liquid-gateway`)

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `liquide_gateway_connections_active` | gauge | `server` | Active connections per backend |
| `liquide_gateway_connections_total` | counter | `server`, `result` | Total connection attempts |
| `liquide_gateway_bytes_relayed_total` | counter | `direction` (client_to_server, server_to_client) | Bytes proxied |
| `liquide_gateway_auth_attempts_total` | counter | `method`, `result` | Auth attempts at gateway |
| `liquide_gateway_routing_time_seconds` | histogram | | Time to route connection |
| `liquide_gateway_tls_handshake_seconds` | histogram | | TLS handshake time |
| `liquide_gateway_backend_health` | gauge | `server` | Backend health (0=down, 1=up) |
| `liquide_gateway_rate_limit_rejects_total` | counter | | Connections rejected by rate limiter |

### 2.6 Cardinality Rules

High-cardinality labels can overwhelm Prometheus. The following rules MUST be followed:

| Rule | Description |
|------|-------------|
| **No `session_id` on high-frequency metrics** | Metrics emitted per-frame or per-event MUST NOT include `session_id` as a label. Use per-session metric endpoints instead. |
| **No `user` label on session metrics** | User identity is implicit in the per-session endpoint. |
| **No unbounded string labels** | Labels MUST use a fixed enum of values (e.g., `codec="h264"`, not `codec="libx264 -preset veryfast"`). |
| **Plugin ID cardinality** | `plugin_id` is allowed because plugins are admin-controlled (bounded). Max 50 plugins per session. |
| **IP address labels forbidden** | Source IP is never a metric label (use audit logs instead). |
| **Max labels per metric** | ≤ 4 labels per metric. |

---

## 3) Distributed Trace Spans

Traces follow [OpenTelemetry](https://opentelemetry.io/) conventions and can be exported to Jaeger, Zipkin, or OTLP-compatible backends.

### 3.1 Trace Context Propagation

- A `trace_id` is generated at the client when an action begins (e.g., key press, clipboard paste).
- The `trace_id` is carried in the protocol message headers (CBOR field `trace_id` in control messages).
- Server components create child spans linked to the incoming `trace_id`.

### 3.2 Standard Spans

| Span Name | Component | Parent | Description |
|-----------|-----------|--------|-------------|
| `client.input.send` | Client | — (root) | Client captures input and sends to server |
| `server.input.process` | Session | `client.input.send` | Server processes input event |
| `compositor.frame.render` | Session | `server.input.process` | Compositor renders frame |
| `compositor.effects.blur` | Session | `compositor.frame.render` | Blur computation |
| `compositor.effects.shadow` | Session | `compositor.frame.render` | Shadow computation |
| `compositor.damage.compute` | Session | `compositor.frame.render` | Damage tracking |
| `encoder.frame.encode` | Session | `compositor.frame.render` | Encode frame/tile |
| `transport.frame.send` | Session | `encoder.frame.encode` | Packetize and send |
| `client.frame.receive` | Client | `transport.frame.send` | Receive over network |
| `client.frame.decode` | Client | `client.frame.receive` | Decode frame |
| `client.frame.render` | Client | `client.frame.decode` | Render to screen |
| `auth.login` | Supervisor | — (root) | Authentication flow |
| `auth.pam.verify` | Supervisor | `auth.login` | PAM verification |
| `auth.mfa.check` | Supervisor | `auth.login` | MFA verification |
| `session.spawn` | Supervisor | `auth.login` | Fork session process |
| `plugin.call` | Session | varies | Plugin invocation |
| `clipboard.transfer` | Session | — (root) | Clipboard data transfer |
| `gateway.route` | Gateway | — (root) | Connection routing |
| `gateway.tls.handshake` | Gateway | `gateway.route` | TLS negotiation |

### 3.3 Span Attributes

All spans carry standard attributes:

| Attribute | Type | Description |
|-----------|------|-------------|
| `liquide.session_id` | string | Session identifier |
| `liquide.component` | string | Component name |
| `liquide.version` | string | Component version |
| `liquide.channel` | string | Protocol channel (if applicable) |

### 3.4 Sampling Strategy

| Environment | Sampling Rate | Notes |
|-------------|--------------|-------|
| Development | 100% | All traces captured |
| Staging | 10% | Representative sample |
| Production | 1% (default), 100% for errors | Configurable. Error spans always captured. |

Configuration:

```toml
[observability.tracing]
enabled = false                        # disabled by default
exporter = "otlp"                     # otlp, jaeger, zipkin, stdout
endpoint = "http://localhost:4317"    # OTLP gRPC endpoint
sampling_rate = 0.01                  # 1% in production
always_sample_errors = true
```

---

## 4) Structured Log Schema

All LiquiDE components emit **structured JSON logs** with a consistent schema.

### 4.1 Log Format

```json
{
  "ts": "2025-06-15T14:22:31.847Z",
  "level": "info",
  "component": "liquid-session",
  "session_id": "s-001",
  "target": "liquide::compositor::render",
  "message": "Frame rendered",
  "fields": {
    "frame_id": 42381,
    "render_time_ms": 4.2,
    "damage_tiles": 12,
    "degradation_level": 0,
    "mode": "hybrid"
  }
}
```

### 4.2 Schema Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `ts` | string (ISO 8601 UTC) | Yes | Timestamp with microsecond precision |
| `level` | enum | Yes | `trace`, `debug`, `info`, `warn`, `error` |
| `component` | string | Yes | `liquid-desktopd`, `liquid-session`, `liquid-gateway`, `liquid-manager` |
| `session_id` | string | Conditional | Present in session-scoped logs |
| `target` | string | Yes | Rust module path (e.g., `liquide::compositor::render`) |
| `message` | string | Yes | Human-readable message |
| `fields` | object | Yes | Structured key-value pairs (metric data, identifiers, etc.) |
| `span_id` | string | Optional | OpenTelemetry span ID (if tracing enabled) |
| `trace_id` | string | Optional | OpenTelemetry trace ID |

### 4.3 Log Levels

| Level | Usage | Examples |
|-------|-------|---------|
| `error` | Unrecoverable errors requiring attention | Session crash, auth system failure, cgroup OOM |
| `warn` | Recoverable issues, potential problems | Plugin fault, SLO violation, degradation descent, rate limit hit |
| `info` | Normal operational events | Session start/stop, auth success, transport switch, plugin load |
| `debug` | Detailed operational data | Frame timing, encode stats, damage counts, cache hits |
| `trace` | Very verbose, for debugging | Every input event, every tile encoded, CBOR decode details |

### 4.4 Log Configuration

```toml
[logging]
# Default log level
level = "info"

# Per-module log levels
[logging.modules]
compositor = "info"
transport = "info"
encoder = "info"
input = "info"
clipboard = "info"
plugin = "info"
supervisor = "info"
auth = "info"
crash = "warn"
audit = "info"

# Output targets
[logging.output]
stdout = true                         # log to stdout (for systemd journal capture)
file = "/var/log/liquide/session.log" # log to file (optional)
file_max_size_mb = 50
file_max_age_days = 7
file_max_backups = 5
json_format = true                    # structured JSON (true) or human-readable (false)
```

### 4.5 Log Parity Across Components

All components MUST log these events at the specified level:

| Event | Level | Components | Required Fields |
|-------|-------|-----------|----------------|
| Component startup | `info` | All | `version`, `config_path` |
| Component shutdown | `info` | All | `reason`, `uptime_seconds` |
| Auth attempt | `info` | supervisor, gateway | `user`, `method`, `source_ip`, `result` |
| Session created | `info` | supervisor | `session_id`, `user`, `pid` |
| Session terminated | `info` | supervisor | `session_id`, `reason`, `duration_seconds` |
| Transport established | `info` | session, gateway | `transport`, `cipher`, `client_version` |
| Transport error | `warn` | session, gateway | `transport`, `error`, `retry` |
| Plugin loaded | `info` | session | `plugin_id`, `version`, `abi_version` |
| Plugin fault | `warn` | session | `plugin_id`, `fault_type`, `extension_point` |
| SLO violation | `warn` | session | `metric`, `threshold`, `actual_value`, `duration_seconds` |
| Degradation change | `info` | session | `from_level`, `to_level`, `reason` |
| Crash detected | `error` | supervisor | `session_id`, `signal`, `exit_code` |
| Backpressure activated | `warn` | session | `level`, `queue_percent` |

---

## 5) Troubleshooting Runbooks

### 5.1 High Input-to-Photon Latency

**Symptom**: User reports sluggish typing or mouse lag. `liquide_input_processing_time_seconds` p99 > 10ms or `input-to-photon` exceeds SLO.

**Diagnosis**:
1. Check `liquidctl session perf` for live latency breakdown.
2. Check compositor frame time: `liquide_compositor_frame_time_seconds`. If > 16ms, compositor is bottleneck.
3. Check encode time: `liquide_encode_time_seconds`. If > 8ms, encoder is bottleneck.
4. Check transport RTT: `liquide_transport_rtt_seconds`. If elevated above network expectation, transport congestion.
5. Check backpressure: `liquide_transport_send_queue_ratio`. If > 0.8, bandwidth-limited.
6. Check degradation level: `liquide_compositor_degradation_level`. High levels indicate CPU pressure.

**Resolution**:
- Compositor bottleneck: reduce `rendering.profile` to `performance`. Check for excessive glass surfaces.
- Encoder bottleneck: switch to faster codec (`h264` over `av1`), reduce quality preset, enable hardware encoder if GPU available.
- Transport congestion: reduce max FPS, enable `bandwidth_saver` mode, check network for issues.
- CPU pressure: check for runaway plugins (`liquide_plugin_call_time_seconds`), reduce max concurrent sessions.

### 5.2 High Memory Usage

**Symptom**: Session approaching cgroup memory limit. `liquide_supervisor_cgroup_memory_bytes` > 80% of limit.

**Diagnosis**:
1. Check plugin memory: `liquide_plugin_memory_bytes`. A plugin may be leaking.
2. Check shadow cache: `liquide_compositor_shadow_cache_size`. May need reduction.
3. Check font atlas size: large font variety causes atlas growth.
4. Check tile buffer: `performance.tile.tile_size` × grid count × buffer depth.

**Resolution**:
- Plugin leak: restart faulting plugin (`liquidctl plugins reload <plugin_id>`). If persistent, disable.
- Shadow cache: reduce `rendering.effects.shadow_cache_max` or reduce shadow quality.
- Font atlas: limit font variants, reduce font sizes.
- Tile buffer: reduce tile size or buffer depth.
- Last resort: increase cgroup memory limit or reduce session count.

### 5.3 Session Crash Loop

**Symptom**: Session repeatedly crashes and restarts. `liquide_supervisor_session_crashes_total` incrementing rapidly.

**Diagnosis**:
1. Check crash reports: `liquidctl crash list --session <id>`.
2. Check crash signal: `SIGSEGV` suggests memory corruption, `SIGABRT` suggests assertion failure.
3. Check if crash happens during specific operation (render, encode, plugin call).
4. Check plugin faults before crash: `liquide_plugin_faults_total` — a plugin may be triggering the crash.

**Resolution**:
- Plugin-related: disable suspect plugin (`liquidctl plugins disable <plugin_id>`).
- Encode crash: switch codec, disable hardware encoder.
- Render crash: force CPU rendering (`rendering.gpu_mode = "cpu"`).
- Persistent: engage support with crash report. Consider `rendering.profile = "minimal"` as interim.
- After 5 restarts in 10 minutes, supervisor stops restarting automatically. Admin intervention required.

### 5.4 Poor Visual Quality / Bandwidth Spikes

**Symptom**: Blurry text, compression artifacts, or sudden bandwidth increase.

**Diagnosis**:
1. Check encoding mode: `liquidctl session status` — verify tile mode for text regions.
2. Check tile delta ratio: `liquide_encode_tile_delta_ratio`. Low ratio means many full tiles (high bandwidth).
3. Check video quality: `liquide_encode_quality_score`. Low SSIM indicates aggressive compression.
4. Check degradation level: high degradation disables blur, reducing visual quality.
5. Check bandwidth estimate: `liquide_transport_bandwidth_estimate_bps`. Low estimate triggers aggressive compression.

**Resolution**:
- Text artifacts: ensure text regions use tile mode (lossless). Check `performance.tile.codec` is `zstd` or `png`.
- Low delta ratio: may indicate high screen churn. Check for full-screen animations or video. Consider reducing tile size.
- Low quality: increase quality preset, increase bandwidth cap.
- Bandwidth spike: typically caused by full-screen damage (window resize, slide transition). Transient — will resolve.

### 5.5 Plugin Not Loading

**Symptom**: Plugin installed but not active. `liquidctl plugins list` shows plugin as `error` state.

**Diagnosis**:
1. Check plugin load log: `journalctl -u liquide-session --grep <plugin_id>`.
2. Common failure reasons:
   - ABI version mismatch: plugin targets newer ABI than server supports.
   - Signature verification failure: `plugins.require_signatures = true` and plugin is unsigned.
   - Capability not granted: plugin requires capabilities not in server policy.
   - WASM compilation error: malformed `.wasm` file.

**Resolution**:
- ABI mismatch: update plugin or update server.
- Signature: sign plugin with trusted key, or disable `require_signatures` (not recommended).
- Capability: add required capabilities to policy.
- WASM error: re-download or rebuild plugin.

### 5.6 Gateway Connection Failures

**Symptom**: Clients cannot connect through gateway. `liquide_gateway_connections_total{result="failure"}` increasing.

**Diagnosis**:
1. Check gateway health: `liquidctl gateway status`.
2. Check backend health: `liquide_gateway_backend_health`. If 0, backend server is unreachable.
3. Check TLS: `liquide_gateway_tls_handshake_seconds`. Failures indicate cert issues.
4. Check rate limiter: `liquide_gateway_rate_limit_rejects_total`. Legitimate clients may be rate-limited.
5. Check auth: `liquide_gateway_auth_attempts_total{result="failure"}`. Auth issues.

**Resolution**:
- Backend down: check `liquid-desktopd` status on backend servers. Check network between gateway and backend.
- TLS error: verify certificates are valid and not expired. Check mTLS configuration between gateway and backend.
- Rate limit: increase rate limits if legitimate traffic. Check for DDoS.
- Auth failure: verify auth configuration matches between gateway and backend.

---

## 6) Observability Configuration Summary

```toml
[observability]
# Metrics
[observability.metrics]
enabled = true
listen = "127.0.0.1:9400"           # Prometheus scrape endpoint
auth_token = ""                      # optional bearer token for scrape auth

# Tracing
[observability.tracing]
enabled = false
exporter = "otlp"
endpoint = "http://localhost:4317"
sampling_rate = 0.01
always_sample_errors = true

# Logging (see §4.4 for full logging config)
# Logging is always enabled. Configuration in [logging] section.

# Health check
[observability.health]
enabled = true
listen = "127.0.0.1:9410/healthz"   # liveness/readiness probe
```

---

## 7) Test Plan

### Metrics
- Verify all catalogued metrics are registered and emitted.
- Verify cardinality rules: no `session_id` on high-frequency metrics, no IP labels, max 4 labels.
- Verify Prometheus scrape endpoint returns valid exposition format.
- Verify metric values are correct (e.g., `frame_rate` matches actual frames rendered).

### Tracing
- Verify trace context propagation from client → server → compositor → encoder → transport.
- Verify span relationships (parent-child) are correct.
- Verify sampling rate is respected (±10% tolerance over 1000 traces).
- Verify error spans are always captured regardless of sampling rate.

### Logging
- Verify all log parity events (§4.5) are emitted at the correct level.
- Verify JSON log format matches schema.
- Verify per-module log level overrides work.
- Verify log rotation (file size and age limits).

### Runbooks
- For each runbook, create a synthetic scenario that triggers the symptom and verify the diagnosis steps produce the expected output.
