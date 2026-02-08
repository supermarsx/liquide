# LiquidDE — Protocol Formal Specification

> **Status**: Draft
> **Depends on**: [spec.md](spec.md) (core server), [spec-client.md](spec-client.md) (client)

---

## 1) Overview

This document formally specifies the LiquidDE wire protocol: message framing, channel architecture, compression rules, ordering guarantees, canonical schemas, state machines, the emergency channel, and conformance testing.

The LiquidDE protocol is a **multiplexed, multi-channel binary protocol** designed for low-latency remote desktop streaming with reliability guarantees on control data and best-effort delivery on media streams.

---

## 2) Transport Layer

### 2.1 Supported Transports

| Transport | Use Case | Properties |
|-----------|----------|------------|
| TLS 1.3 over TCP | Control channel, file transfer | Reliable, ordered |
| DTLS 1.3 over UDP | Video, audio, cursor | Low-latency, unordered, lossy allowed |
| QUIC (HTTP/3) | Hybrid (all channels) | Reliable + unreliable streams, multiplexed |
| WebSocket over TLS | Browser client fallback | Reliable, ordered |

### 2.2 Transport Negotiation

1. Client connects to server on the configured port.
2. TLS handshake completes (the control channel is always TLS).
3. Client sends `ClientHello` message containing:
   - Protocol version: `proto/1`.
   - Supported transports: `["quic", "tcp+udp", "tcp-only", "websocket"]`.
   - Supported codecs, features, capabilities.
4. Server responds with `ServerHello`:
   - Selected protocol version.
   - Selected transport strategy.
   - Assigned channel IDs.
   - Session parameters.
5. Additional transport connections are opened (e.g., UDP for video if `tcp+udp` selected).

---

## 3) Channel Architecture

### 3.1 Channel Table

Each logical data stream is assigned a **channel ID**. Channels are multiplexed over the underlying transport.

| Channel ID | Name | Direction | Reliability | Ordering | Priority |
|-----------|------|-----------|-------------|----------|----------|
| `0x00` | Control | Bidirectional | Reliable | Ordered | Highest |
| `0x01` | Emergency | Bidirectional | Reliable | Ordered | Highest (parallel to control) |
| `0x10` | Video | Server → Client | Unreliable (lossy OK) | Ordered per-frame | High |
| `0x11` | Cursor | Server → Client | Unreliable | Latest-wins | Highest media |
| `0x20` | Audio (playback) | Server → Client | Unreliable | Ordered | High |
| `0x21` | Audio (capture) | Client → Server | Unreliable | Ordered | High |
| `0x30` | Clipboard | Bidirectional | Reliable | Ordered | Medium |
| `0x31` | File Transfer | Bidirectional | Reliable | Ordered | Low |
| `0x40` | USB/IP | Bidirectional | Reliable | Ordered | Medium |
| `0x50` | Input | Client → Server | Reliable | Ordered | Highest |
| `0x60` | Camera | Client → Server | Unreliable | Ordered | Medium |
| `0xF0` | Plugin IPC | Bidirectional | Reliable | Ordered | Low |

### 3.2 Channel Lifecycle

```
                    ┌───────────┐
                    │  Closed   │
                    └─────┬─────┘
                          │ ChannelOpen
                          ▼
                    ┌───────────┐
              ┌─────│  Opening  │
              │     └─────┬─────┘
              │           │ ChannelOpenAck
    ChannelOpenReject     ▼
              │     ┌───────────┐
              │     │  Active   │◄─────── ChannelResume
              │     └──┬──┬──┬──┘
              │        │  │  │
              │        │  │  └── ChannelSuspend ──► Suspended
              │        │  └───── ChannelReset ────► Opening (re-negotiate)
              │        │
              │        └──────── ChannelClose
              │                       │
              ▼                       ▼
        ┌───────────┐          ┌───────────┐
        │  Rejected │          │  Closed   │
        └───────────┘          └───────────┘
```

### 3.3 Channel Multiplexing

On a single TCP/QUIC connection, channels are multiplexed using the frame header's `channel_id` field. On `tcp+udp` transport, the mapping is:

| Transport | Channels |
|-----------|----------|
| TCP (TLS) | `0x00` Control, `0x01` Emergency, `0x30` Clipboard, `0x31` File Transfer, `0x40` USB/IP, `0x50` Input, `0xF0` Plugin IPC |
| UDP (DTLS) | `0x10` Video, `0x11` Cursor, `0x20`/`0x21` Audio, `0x60` Camera |

On QUIC transport, all channels use QUIC streams. Unreliable channels use QUIC datagrams (RFC 9221) when available.

---

## 4) Message Framing

### 4.1 Frame Format

Every message on the wire is wrapped in a frame:

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
├─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┤
│  Magic (0x4C44)   │Version│ Flags │        Channel ID             │
├─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┤
│                     Sequence Number                               │
├─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┤
│                     Timestamp (µs)                                │
│                                                                   │
├─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┤
│       Message Type            │        Payload Length             │
├─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┤
│                                                                   │
│                     Payload (variable length)                     │
│                                                                   │
├─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┼─┤
│                     CRC-32C (optional, if flag set)               │
└─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┘
```

### 4.2 Frame Header Fields

| Field | Size | Description |
|-------|------|-------------|
| Magic | 2 bytes | `0x4C44` ("LD" for LiquidDE) |
| Version | 4 bits | Protocol frame version (`1`) |
| Flags | 4 bits | See flag table below |
| Channel ID | 2 bytes | Logical channel (see §3.1) |
| Sequence Number | 4 bytes | Per-channel monotonic sequence number |
| Timestamp | 8 bytes | Microseconds since session start (64-bit unsigned) |
| Message Type | 2 bytes | Message type code (see §5) |
| Payload Length | 2 bytes | Payload size in bytes (max 65535; for larger payloads, use fragmentation) |
| Payload | variable | Message-specific payload |
| CRC-32C | 4 bytes | Optional checksum (Castagnoli CRC) |

**Total header size**: 20 bytes (without CRC) or 24 bytes (with CRC).

### 4.3 Frame Flags

| Bit | Name | Description |
|-----|------|-------------|
| 0 | `COMPRESSED` | Payload is compressed (see §6) |
| 1 | `FRAGMENTED` | This frame is part of a multi-frame message |
| 2 | `CRC` | CRC-32C checksum is appended |
| 3 | `PRIORITY` | High-priority frame (skip normal queue) |

### 4.4 Fragmentation

Messages larger than 65535 bytes are fragmented:

| Fragment Type | Flags | Sequence | Payload |
|--------------|-------|----------|---------|
| First | `FRAGMENTED` | N | First chunk + `fragment_total` (4 bytes prepended) |
| Middle | `FRAGMENTED` | N+1, N+2... | Continuation chunks |
| Last | (no `FRAGMENTED` flag) | N+K | Final chunk |

The receiver reassembles fragments by channel and sequence number. Fragments must arrive in order on reliable channels. On unreliable channels, missing fragments cause the entire message to be dropped.

### 4.5 Maximum Frame Sizes

| Transport | Max Frame Size | Notes |
|-----------|---------------|-------|
| TCP | 65559 bytes | Header (20) + max payload (65535) + CRC (4) |
| UDP | MTU - DTLS overhead | Typically ~1200 bytes; fragmentation above this |
| QUIC stream | 65559 bytes | Same as TCP |
| QUIC datagram | QUIC max_datagram_frame_size | Usually ~1200 bytes |

---

## 5) Message Types

### 5.1 Control Channel Messages (0x00)

| Type Code | Name | Direction | Description |
|-----------|------|-----------|-------------|
| `0x0001` | `ClientHello` | C → S | Initial handshake |
| `0x0002` | `ServerHello` | S → C | Handshake response |
| `0x0003` | `Ping` | Both | Keepalive / latency measurement |
| `0x0004` | `Pong` | Both | Ping response |
| `0x0005` | `ChannelOpen` | Both | Open a logical channel |
| `0x0006` | `ChannelOpenAck` | Both | Accept channel open |
| `0x0007` | `ChannelOpenReject` | Both | Reject channel open |
| `0x0008` | `ChannelClose` | Both | Close a channel |
| `0x0009` | `ChannelSuspend` | Both | Suspend a channel (pause data) |
| `0x000A` | `ChannelResume` | Both | Resume a suspended channel |
| `0x0010` | `LoginPrompt` | S → C | Request authentication input |
| `0x0011` | `LoginResponse` | C → S | Authentication input from user |
| `0x0012` | `LoginSuccess` | S → C | Authentication succeeded |
| `0x0013` | `LoginFailure` | S → C | Authentication failed |
| `0x0014` | `SessionInfo` | S → C | Session metadata (user, ID, features) |
| `0x0015` | `Disconnect` | Both | Graceful disconnect with reason |
| `0x0016` | `ConfigUpdate` | S → C | Server config change notification |
| `0x0017` | `PolicyUpdate` | S → C | Policy change affecting this session |
| `0x0018` | `Capabilities` | Both | Feature negotiation (post-handshake) |
| `0x0019` | `Resize` | C → S | Client viewport resize |
| `0x001A` | `ResizeAck` | S → C | Server acknowledges resize |
| `0x001B` | `SessionLock` | S → C | Session locked |
| `0x001C` | `SessionUnlock` | C → S | Unlock request (with credentials) |
| `0x0020` | `AssetManifest` | S → C | List of session assets with content hashes (icon/cursor/theme cache) |
| `0x0021` | `AssetRequest` | C → S | Client requests specific assets (cache misses) |
| `0x0022` | `AssetData` | S → C | Asset payload (icon, cursor, theme resource) |
| `0x0023` | `AssetManifestAck` | C → S | Client confirms manifest received with list of cache hits |

### 5.2 Emergency Channel Messages (0x01)

The emergency channel operates *independently* of the control channel. It is designed to function even when the session process has crashed or is unresponsive. The supervisor daemon maintains this channel directly.

| Type Code | Name | Direction | Description |
|-----------|------|-----------|-------------|
| `0x0101` | `CrashInfo` | S → C | Session crash notification (BSOD data) |
| `0x0102` | `CrashLogChunk` | S → C | Chunk of crash log text (streamed) |
| `0x0103` | `CrashLogEnd` | S → C | End of crash log stream |
| `0x0104` | `CrashReportRequest` | C → S | Client requests full crash report |
| `0x0105` | `CrashReportChunk` | S → C | Chunk of crash report data |
| `0x0106` | `CrashReportEnd` | S → C | End of crash report stream |
| `0x0107` | `SupervisorStatus` | S → C | Supervisor session status update |
| `0x0108` | `RestartRequest` | C → S | Client requests session restart |
| `0x0109` | `RestartStatus` | S → C | Session restart progress/result |
| `0x010A` | `HeartbeatEmergency` | Both | Emergency heartbeat (supervisor ↔ client) |
| `0x010B` | `ServerShutdown` | S → C | Server is shutting down gracefully |
| `0x010C` | `SessionLogStream` | S → C | Real-time log forwarding (emergency) |
| `0x010D` | `DiagnosticRequest` | C → S | Client requests diagnostic data |
| `0x010E` | `DiagnosticResponse` | S → C | Diagnostic data (memory, CPU, etc.) |

### 5.3 Video Channel Messages (0x10)

| Type Code | Name | Direction | Description |
|-----------|------|-----------|-------------|
| `0x1001` | `FrameHeader` | S → C | Frame metadata (codec, size, damage rects) |
| `0x1002` | `FrameData` | S → C | Encoded frame data (possibly fragmented) |
| `0x1003` | `FrameAck` | C → S | Client acknowledge frame receipt |
| `0x1004` | `QualityHint` | C → S | Client hints about desired quality/fps |
| `0x1005` | `CodecSwitch` | S → C | Server switching codecs |
| `0x1006` | `KeyFrameRequest` | C → S | Client requests a key frame (after packet loss) |

### 5.4 Cursor Channel Messages (0x11)

| Type Code | Name | Direction | Description |
|-----------|------|-----------|-------------|
| `0x1101` | `CursorPosition` | S → C | Cursor position update (x, y) |
| `0x1102` | `CursorShape` | S → C | Cursor image/shape change |
| `0x1103` | `CursorVisibility` | S → C | Cursor show/hide |

### 5.5 Audio Channel Messages (0x20, 0x21)

| Type Code | Name | Direction | Description |
|-----------|------|-----------|-------------|
| `0x2001` | `AudioConfig` | Both | Audio format negotiation (sample rate, channels, codec) |
| `0x2002` | `AudioData` | Both | Encoded audio frame |
| `0x2003` | `AudioMute` | Both | Mute/unmute |
| `0x2004` | `AudioVolume` | Both | Volume level change |

### 5.6 Clipboard Channel Messages (0x30)

| Type Code | Name | Direction | Description |
|-----------|------|-----------|-------------|
| `0x3001` | `ClipboardOffer` | Both | Announce available clipboard formats |
| `0x3002` | `ClipboardRequest` | Both | Request clipboard data in specific format |
| `0x3003` | `ClipboardData` | Both | Clipboard content (possibly fragmented) |
| `0x3004` | `ClipboardDataEnd` | Both | End of clipboard data transfer |
| `0x3005` | `ClipboardClear` | Both | Clipboard cleared |
| `0x3006` | `ClipboardProgress` | Both | Transfer progress for large items |
| `0x3007` | `ClipboardCancel` | Both | Cancel ongoing transfer |

### 5.7 Input Channel Messages (0x50)

| Type Code | Name | Direction | Description |
|-----------|------|-----------|-------------|
| `0x5001` | `KeyDown` | C → S | Key press (scancode + keysym + modifiers) |
| `0x5002` | `KeyUp` | C → S | Key release |
| `0x5003` | `MouseMove` | C → S | Mouse position (absolute or relative) |
| `0x5004` | `MouseButton` | C → S | Mouse button press/release |
| `0x5005` | `MouseScroll` | C → S | Scroll event (axis, delta, discrete/smooth) |
| `0x5006` | `TouchDown` | C → S | Touch start (id, x, y) |
| `0x5007` | `TouchMove` | C → S | Touch move |
| `0x5008` | `TouchUp` | C → S | Touch end |
| `0x5009` | `TouchCancel` | C → S | Touch sequence cancelled |
| `0x500A` | `InputSyncRequest` | C → S | Request input state sync (after reconnect) |
| `0x500B` | `InputSyncResponse` | S → C | Current modifier/button state |

---

## 6) Compression

### 6.1 Compression Algorithms

| Algorithm | ID | Use Case | Notes |
|-----------|----|----------|-------|
| None | `0x00` | Small messages, already-compressed data | Default for frames < 64 bytes |
| LZ4 | `0x01` | Control messages, clipboard text | Very fast, moderate ratio |
| Zstd | `0x02` | File transfer, crash reports | Good ratio, configurable level |

### 6.2 Compression Rules

| Channel | Default Compression | Rationale |
|---------|-------------------|-----------|
| Control | LZ4 (for messages > 128 bytes) | JSON/CBOR messages benefit from compression |
| Emergency | LZ4 | Log text compresses well |
| Video | None | Already codec-compressed |
| Audio | None | Already codec-compressed |
| Cursor | None | Small messages |
| Clipboard (text) | LZ4 | Text compresses well |
| Clipboard (binary) | None or Zstd | Depends on MIME type |
| File Transfer | Zstd (level 3) | Good ratio for general files |
| Input | None | Tiny messages, latency-critical |

### 6.3 Compression Negotiation

Supported compression algorithms are exchanged in `ClientHello`/`ServerHello`. Both sides must support LZ4 (mandatory). Zstd is optional.

---

## 7) Ordering Guarantees

### 7.1 Per-Channel Ordering

| Channel | Ordering | Guarantee |
|---------|----------|-----------|
| Control | Strictly ordered | Messages processed in sequence number order |
| Emergency | Strictly ordered | Independent sequence from control |
| Video | Frame-ordered | Frames delivered in order; individual packets within a frame may arrive out-of-order on UDP (reassembled before delivery) |
| Cursor | Latest-wins | Only the most recent cursor position matters; older positions can be dropped |
| Audio | Stream-ordered | Audio frames delivered in timestamp order; late frames beyond jitter buffer are dropped |
| Clipboard | Strictly ordered | Operations are sequential |
| Input | Strictly ordered | Input events must preserve order |

### 7.2 Cross-Channel Ordering

There are **no** ordering guarantees between different channels. A video frame and an input event may arrive at the server/client in any relative order. Timestamps (microseconds since session start) allow correlation when needed.

### 7.3 Duplicate Detection

Sequence numbers are monotonically increasing per channel. Receivers discard frames with a sequence number ≤ the last processed sequence number for that channel (duplicate suppression).

---

## 8) Control Channel Schema

Control channel payloads use **CBOR** (Concise Binary Object Representation, [RFC 8949](https://tools.ietf.org/html/rfc8949)) encoding. CBOR was chosen over JSON for compactness and over Protobuf for schema flexibility.

### 8.1 CBOR Schema: ClientHello

```cddl
ClientHello = {
    protocol_version: text,                    ; "proto/1"
    client_name: text,                         ; "LiquidClient"
    client_version: text,                      ; "1.3.2"
    client_platform: text,                     ; "linux-x86_64", "windows-x86_64", "macos-aarch64"
    supported_transports: [+ text],            ; ["quic", "tcp+udp", "tcp-only", "websocket"]
    supported_codecs: [+ text],                ; ["h264", "h265", "av1", "vp9"]
    supported_audio_codecs: [+ text],          ; ["opus", "aac"]
    supported_compressions: [+ text],          ; ["lz4", "zstd"]
    capabilities: {* text => bool},            ; {"clipboard": true, "audio": true, "usb": false, ...}
    display: {
        width: uint,
        height: uint,
        scale_factor: float32,
        refresh_rate: uint,                    ; Hz
    },
    ? resume_token: bytes,                     ; session resume token (if reconnecting)
}
```

### 8.2 CBOR Schema: ServerHello

```cddl
ServerHello = {
    protocol_version: text,
    server_name: text,                         ; "LiquidDE"
    server_version: text,
    selected_transport: text,
    selected_video_codec: text,
    selected_audio_codec: text,
    channels: {* uint => ChannelConfig},       ; channel_id => config
    session_id: text,
    ? resume_accepted: bool,
    features: {* text => bool},
}

ChannelConfig = {
    name: text,
    direction: text,                           ; "s2c", "c2s", "bidirectional"
    reliable: bool,
    compression: text,                         ; "none", "lz4", "zstd"
}
```

### 8.3 CBOR Schema: CrashInfo (Emergency Channel)

```cddl
CrashInfo = {
    error_code: text,                          ; "SESSION_PROCESS_CRASH", "SESSION_OOM", etc.
    description: text,                         ; human-readable description
    severity: text,                            ; "session", "connection", "server"
    ? stack_trace: [+ text],                   ; stack frames (if available)
    ? session_id: text,
    ? user: text,
    ? uptime_seconds: uint,
    ? crash_report_id: text,                   ; for downloading full report
    ? exit_code: int,
    ? signal_name: text,                       ; "SIGSEGV", "SIGABRT", etc.
    recovery_options: [+ text],                ; ["restart_session", "download_report", "disconnect"]
    restart_available: bool,
    timestamp: text,                           ; ISO 8601
    ? log_tail: [+ text],                      ; last N log lines
}
```

### 8.4 CBOR Schema: FrameHeader (Video Channel)

```cddl
FrameHeader = {
    frame_id: uint,                            ; monotonic frame counter
    codec: text,                               ; "h264", "h265", "av1", "vp9"
    frame_type: text,                          ; "key", "delta"
    width: uint,
    height: uint,
    data_size: uint,                           ; total encoded data bytes
    ? damage_rects: [+ Rect],                  ; damaged regions (if delta frame)
    ? quantizer: uint,                         ; quantizer value for quality metrics
    timestamp_us: uint,                        ; capture timestamp
    ? metadata: {* text => any},               ; extensible metadata
}

Rect = {
    x: uint,
    y: uint,
    width: uint,
    height: uint,
}
```

### 8.5 CBOR Schema: Input Events

```cddl
KeyEvent = {
    type: text,                                ; "down" or "up"
    scancode: uint,                            ; hardware scancode
    keysym: uint,                              ; XKB keysym
    modifiers: uint,                           ; bitmask: shift=1, ctrl=2, alt=4, super=8, capslock=16
    ? text: text,                              ; UTF-8 text produced by this keypress
    timestamp_us: uint,
}

MouseMoveEvent = {
    mode: text,                                ; "absolute" or "relative"
    x: float32,                                ; position or delta
    y: float32,
    timestamp_us: uint,
}

MouseButtonEvent = {
    type: text,                                ; "down" or "up"
    button: uint,                              ; 1=left, 2=middle, 3=right, 4+=extra
    x: float32,
    y: float32,
    timestamp_us: uint,
}

ScrollEvent = {
    axis: text,                                ; "vertical" or "horizontal"
    delta: float32,                            ; scroll amount (positive = down/right)
    discrete: bool,                            ; true for click-wheel, false for smooth
    timestamp_us: uint,
}

TouchEvent = {
    type: text,                                ; "down", "move", "up", "cancel"
    id: uint,                                  ; touch point ID
    x: float32,
    y: float32,
    timestamp_us: uint,
}
```

### 8.6 CBOR Schema: Asset Cache Messages

```cddl
AssetManifest = {
    manifest_version: uint,                    ; monotonic version for diff detection
    assets: [+ AssetEntry],
}

AssetEntry = {
    asset_id: text,                            ; e.g., "icon:firefox:48", "cursor:default:left_ptr"
    category: text,                            ; "icon", "cursor", "theme", "avatar", "shell"
    content_hash: bytes,                       ; SHA-256 truncated to 16 bytes
    size: uint,                                ; size in bytes
    mime_type: text,                           ; "image/png", "image/svg+xml", "image/x-icon"
    ? inline_data: bytes,                      ; present if size < inline_threshold (saves round-trip)
    ? metadata: {* text => any},               ; optional: icon sizes, theme name, etc.
}

AssetManifestAck = {
    manifest_version: uint,
    cached_assets: [+ text],                   ; asset_ids the client already has (cache hits)
    requested_assets: [+ text],                ; asset_ids the client needs (cache misses)
}

AssetRequest = {
    asset_ids: [+ text],                       ; batch request for multiple assets
    ? preferred_format: text,                  ; optional format preference ("png", "svg", "ico")
    ? preferred_sizes: [+ uint],               ; optional size preference (e.g., [48, 64])
}

AssetData = {
    asset_id: text,
    content_hash: bytes,
    mime_type: text,
    data: bytes,                               ; asset content
    ? is_last: bool,                           ; true if this is the last asset in a batch response
}
```

---

## 9) Emergency Channel

### 9.1 Purpose

The emergency channel (`0x01`) is a dedicated reliable channel that operates **independently** of the session process. It serves as the communication path for:

1. **Crash notifications**: when the session process crashes, the supervisor sends `CrashInfo` via the emergency channel.
2. **Crash log streaming**: the client can request and stream crash logs and full crash reports.
3. **Session restart coordination**: the client requests a restart; the supervisor reports progress.
4. **Supervisor heartbeat**: ensures the client knows whether the server is alive even if the session is dead.
5. **Server shutdown**: graceful shutdown notifications.
6. **Emergency diagnostics**: the client can request server-side diagnostic data (memory, CPU, session list).
7. **Emergency log forwarding**: real-time session log streaming for debugging during degraded operation.

### 9.2 Architecture

```
Client ◄═══════ Emergency Channel (0x01) ═══════► liquid-desktopd (supervisor)
                                                         │
Client ◄═══════ Control Channel (0x00) ═════════► liquid-session (user session)
                                                         │
                                              [crash: session dies]
                                                         │
Client ◄═══════ Emergency Channel (0x01) ═══════► liquid-desktopd (supervisor)
                     [still alive]                 [detects crash, sends CrashInfo]
```

The key property: the emergency channel is **not** routed through the session process. It terminates directly at the supervisor daemon. This means:

- If `liquid-session` crashes, hangs, or OOMs, the emergency channel remains operational.
- The client can still communicate with the server to get crash info, request restarts, and download reports.
- The emergency channel is established during the TLS handshake (before session process spawn) and persists for the lifetime of the TCP connection.

### 9.3 Emergency Channel State Machine

```
                    ┌─────────────┐
                    │   Idle      │  (session running normally)
                    └──────┬──────┘
                           │ CrashInfo received
                           ▼
                    ┌─────────────┐
                    │  Crash      │  (session crashed, client shows crash screen)
                    └──┬──┬──┬────┘
                       │  │  │
     CrashReportRequest│  │  │ RestartRequest
                       ▼  │  ▼
               ┌──────────┐│ ┌──────────────┐
               │Streaming ││ │  Restarting   │
               │Report    ││ └───────┬──┬────┘
               └────┬─────┘│         │  │
         ReportEnd  │      │  RestartStatus(success)
                    ▼      │         │  │
               ┌──────────┐│         │  │ RestartStatus(failed)
               │  Crash   │◄─────────  │
               └──────────┘           ▼
                                ┌──────────────┐
                                │  Failed      │  (all restarts exhausted)
                                └──────────────┘
```

### 9.4 Crash Log Grab Protocol

After receiving `CrashInfo`, the client can request the crash log:

1. Client sends `CrashReportRequest` on the emergency channel:
   ```cddl
   CrashReportRequest = {
       crash_report_id: text,
       include_log_tail: bool,     ; include last N log lines
       include_stack_trace: bool,
       include_system_info: bool,
       include_coredump: bool,     ; large; only on explicit request
   }
   ```

2. Server streams the report as chunks:
   ```cddl
   CrashReportChunk = {
       crash_report_id: text,
       chunk_index: uint,
       total_chunks: uint,         ; 0 = unknown (streaming)
       data: bytes,                ; chunk of report data (JSON or tar.gz)
   }
   ```

3. Server sends `CrashReportEnd`:
   ```cddl
   CrashReportEnd = {
       crash_report_id: text,
       total_size: uint,
       sha256: bytes,              ; hash for integrity verification
   }
   ```

4. The client reassembles the chunks and offers the report for download/viewing.

### 9.5 Emergency Log Streaming

For real-time debugging, the client can request live log forwarding via the emergency channel:

1. Client sends `DiagnosticRequest` with `type: "log_stream"`.
2. Server begins sending `SessionLogStream` messages:
   ```cddl
   SessionLogStream = {
       session_id: text,
       timestamp: text,            ; ISO 8601
       level: text,                ; "trace", "debug", "info", "warn", "error"
       subsystem: text,            ; "compositor", "plugin", "transport", etc.
       message: text,
   }
   ```
3. Streaming continues until the client sends another `DiagnosticRequest` with `type: "log_stream_stop"` or the session is restarted.

### 9.6 Emergency Channel Keepalive

The emergency channel has its own heartbeat independent of the control channel:

- `HeartbeatEmergency` is sent every 10 seconds by both sides.
- If 3 consecutive heartbeats are missed (30 seconds), the emergency channel is considered dead.
- If the emergency channel dies while the session is in crash state, the client shows "Server Unreachable" crash screen (the most severe variant).

---

## 10) Channel State Machines

### 10.1 Control Channel State Machine

```
                    ┌─────────────┐
                    │ Connecting  │ (TLS handshake in progress)
                    └──────┬──────┘
                           │ TLS complete
                           ▼
                    ┌─────────────┐
                    │ Handshake   │ (ClientHello sent, awaiting ServerHello)
                    └──────┬──────┘
                           │ ServerHello received
                           ▼
                    ┌─────────────┐
                    │ Authenticating │ (LoginPrompt/LoginResponse exchange)
                    └──────┬──────┘
                           │ LoginSuccess
                           ▼
                    ┌─────────────┐
                    │ Active      │ (session running, normal operation)
                    └──┬──┬──┬────┘
                       │  │  │
            Disconnect │  │  │ Connection lost
                       │  │  ▼
                       │  │ ┌──────────────┐
                       │  │ │ Reconnecting │ (client auto-reconnect)
                       │  │ └──────┬──┬────┘
                       │  │        │  │ Timeout
                       │  │ Resume │  ▼
                       │  │   OK   │ ┌────────┐
                       │  │        │ │Disconnected│
                       │  ◄────────┘ └────────┘
                       │
                       ▼
                 ┌────────────┐
                 │  Closed    │
                 └────────────┘
```

### 10.2 Video Channel State Machine

```
                    ┌────────────┐
                    │  Inactive  │ (channel not opened)
                    └──────┬─────┘
                           │ ChannelOpen
                           ▼
                    ┌────────────┐
                    │ Negotiating│ (codec/resolution negotiation)
                    └──────┬─────┘
                           │ ChannelOpenAck
                           ▼
                    ┌────────────┐
                    │ Streaming  │ (frames being sent)
                    └──┬──┬──┬───┘
                       │  │  │
          ChannelSuspend│  │  │ CodecSwitch
                       │  │  ▼
                       │  │ ┌────────────┐
                       │  │ │ Switching  │ (codec change, key frame pending)
                       │  │ └──────┬─────┘
                       │  │        │ Key frame sent
                       │  │        ▼
                       │  │   Streaming (resumed)
                       │  │
                       ▼  │
                  ┌────────┐│
                  │Suspended││ ChannelClose
                  └────┬───┘│
        ChannelResume  │    ▼
                       ▼ ┌────────┐
                  Streaming│ Closed │
                          └────────┘
```

### 10.3 Audio Channel State Machine

```
                    ┌────────────┐
                    │  Inactive  │
                    └──────┬─────┘
                           │ ChannelOpen
                           ▼
                    ┌────────────┐
                    │ Negotiating│ (AudioConfig exchange: sample rate, channels, codec)
                    └──────┬─────┘
                           │ AudioConfig agreed
                           ▼
                    ┌────────────┐
                    │ Streaming  │ (audio frames flowing)
                    └──┬──┬──────┘
                       │  │
              AudioMute│  │ ChannelClose
                       ▼  ▼
                  ┌────────┐ ┌────────┐
                  │ Muted  │ │ Closed │
                  └────┬───┘ └────────┘
            AudioMute  │
            (unmute)   ▼
                  Streaming
```

---

## 11) Operational SLOs & Performance Targets

### 11.1 Latency Budgets

| Metric | Target (1080p, same-datacenter) | Target (1080p, WAN 50ms RTT) | Target (4K, same-datacenter) |
|--------|-------------------------------|------------------------------|------------------------------|
| Input-to-display (total) | < 16ms | < 50ms + RTT | < 25ms |
| Input processing | < 1ms | < 1ms | < 1ms |
| Compositor render | < 5ms | < 5ms | < 10ms |
| Encode (H.264) | < 5ms | < 5ms | < 10ms |
| Encode (AV1) | < 8ms | < 8ms | < 15ms |
| Transport (packetize + send) | < 2ms | < 2ms | < 3ms |
| Client decode | < 3ms | < 3ms | < 5ms |
| Cursor update | < 5ms | < 5ms + RTT | < 5ms |
| Audio end-to-end | < 30ms | < 30ms + RTT | < 30ms |

### 11.2 Throughput Targets

| Metric | Target  |
|--------|---------|
| Frame rate (1080p, balanced) | 60 FPS sustained |
| Frame rate (4K, balanced) | 30 FPS sustained, 60 FPS achievable |
| Frame rate (idle, no damage) | 0 FPS (no frames sent when nothing changes) |
| Audio stream | 48kHz stereo, Opus, < 128kbps |
| Clipboard (text, < 1MB) | < 100ms end-to-end |
| File transfer | Limited by network bandwidth |

### 11.3 Resource Budget (Server, per session)

| Resource | Target | Maximum |
|----------|--------|---------|
| CPU (1080p, 60fps, balanced) | 1–2 cores | 4 cores |
| CPU (4K, 30fps, balanced) | 2–3 cores | 6 cores |
| CPU (idle, no damage) | < 1% of 1 core | — |
| Memory (session process) | 100–200 MB | 512 MB (without apps) |
| Memory (per WASM plugin) | 2–32 MB | 256 MB (configurable cap) |
| Network (1080p, 60fps, quality) | 5–15 Mbps | 50 Mbps |
| Network (1080p, 60fps, balanced) | 2–8 Mbps | 20 Mbps |
| Network (4K, 30fps, balanced) | 8–20 Mbps | 50 Mbps |
| Network (idle) | < 10 Kbps | — |

### 11.4 CI Regression Thresholds

Automated performance tests run in CI. A regression is flagged if:

| Metric | Regression Threshold |
|--------|---------------------|
| Input-to-display latency (p50) | > 10% increase |
| Input-to-display latency (p99) | > 20% increase |
| Frame rate (sustained, same workload) | > 5% decrease |
| Memory usage (idle session) | > 15% increase |
| CPU usage (idle session) | > 20% increase |
| Binary size | > 10% increase |
| Startup time (session ready) | > 15% increase |

### 11.5 Network Emulation Scenarios

Release gating includes tests under simulated network conditions:

| Scenario | RTT | Bandwidth | Packet Loss | Jitter |
|----------|-----|-----------|-------------|--------|
| LAN | 1ms | 1 Gbps | 0% | 0ms |
| Datacenter (same region) | 5ms | 1 Gbps | 0% | 1ms |
| WAN (same continent) | 30ms | 100 Mbps | 0.1% | 5ms |
| WAN (cross-continent) | 100ms | 50 Mbps | 0.5% | 10ms |
| Cellular (4G) | 50ms | 20 Mbps | 1% | 20ms |
| Cellular (3G) | 100ms | 2 Mbps | 2% | 50ms |
| Degraded (hotel Wi-Fi) | 50ms | 5 Mbps | 3% | 30ms |
| Satellite | 600ms | 10 Mbps | 1% | 10ms |

For each scenario, verify:
- Session is usable (subjective, by test operator).
- No crashes or protocol errors.
- Graceful degradation (quality reduction, not hangs).
- Reconnection succeeds after brief network outage (5s).

---

## 12) Fuzzing Targets

The following components are fuzzing targets for security and robustness:

| Target | Fuzzer Input | Goal |
|--------|-------------|------|
| Frame parser | Random bytes as frame data | No crash, no undefined behavior |
| CBOR decoder | Random bytes as CBOR payload | No crash, correct error returns |
| Video decoder (all codecs) | Malformed encoded bitstreams | No crash, graceful error |
| Clipboard parser | Arbitrary MIME data | No crash, correct MIME validation |
| Protocol state machine | Random message sequences | No invalid state transitions |
| TLS handshake | Malformed TLS records | No crash, correct TLS error |
| Session resume token | Malformed tokens | No bypass, correct auth error |

### 12.1 Fuzzing Infrastructure

- Fuzzing uses `cargo-fuzz` (libFuzzer) for Rust components.
- Corpus seeded from protocol conformance test recordings.
- CI runs fuzzing for a minimum of 1 hour per target on every release.
- Crashes are triaged as security issues (P0) until proven otherwise.

---

## 13) Conformance Tests

### 13.1 Test Categories

| Category | Description | Pass Criteria |
|----------|-------------|---------------|
| Handshake | ClientHello/ServerHello exchange | Both sides reach Active state |
| Channel lifecycle | Open/close/suspend/resume all channels | State machine transitions are correct |
| Authentication | All PAM flows (password, MFA, failure) | Correct state after each flow |
| Video streaming | Key frame + delta frames | Client can decode and display |
| Input round-trip | Key/mouse/touch → server → response | Input is processed and frame reflects it |
| Emergency channel | Simulate crash, verify CrashInfo delivery | Client receives crash data |
| Reconnection | Drop TCP, verify reconnect + resume | Session resumes without data loss |
| Codec switching | Mid-stream codec switch | Client switches decoder, no artifacts |
| Compression | Verify all compression modes | Data integrity after decompress |
| Fragmentation | Send messages > 65535 bytes | Reassembly produces correct data |
| Ordering | Verify per-channel ordering | No out-of-order processing |
| Rate limiting | Exceed notification/clipboard limits | Correct error codes returned |
| Asset caching | AssetManifest/Request/Data round-trip | Client receives and caches assets correctly; cache hits skip transfer |

### 13.2 Conformance Test Runner

A standalone conformance test tool (`liquidde-conformance`) can be run against any LiquidDE server to verify protocol compliance:

```bash
liquidde-conformance --server <address> --username <user> --password <pass> --suite all
```

Outputs a pass/fail report per test case.

---

## 14) Test Plan

### Protocol Correctness
- Frame parsing: all field combinations, max sizes, truncated frames.
- CBOR encoding/decoding: round-trip all message types.
- State machine: all transition paths, including error paths.
- Sequence numbering: duplicate detection, wrap-around at 2^32.
- Timestamp: monotonicity, wrap-around handling.

### Security
- TLS: verify only TLS 1.3 is accepted. Downgrade attack rejected.
- Authentication: brute-force rate limiting. Invalid credentials rejected.
- Channel injection: verify a client cannot send messages on server-only channels.
- Emergency channel: verify it cannot be used to bypass authentication.

### Performance
- All SLOs (§11) met under each network scenario (§11.5).
- Regression thresholds (§11.4) enforced in CI.

### Interoperability
- Conformance tests pass for: Linux client, Windows client, macOS client, browser client.
- Version mismatch: older client with newer server and vice versa.
