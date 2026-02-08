# LiquiDE — Protocol Formal Specification

> **Status**: Living document (wire format frozen at `proto/1`)
> **Depends on**: [spec.md](spec.md) (core server), [spec-client.md](spec-client.md) (client), [Normative Conventions](spec-normative.md)

---

## 1) Overview

This document formally specifies the LiquiDE wire protocol: message framing, channel architecture, compression rules, ordering guarantees, canonical schemas, state machines, the emergency channel, and conformance testing.

The LiquiDE protocol is a **multiplexed, multi-channel binary protocol** designed for low-latency remote desktop streaming with reliability guarantees on control data and best-effort delivery on media streams.

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
| `0x12` | Tile | Server → Client | Reliable | Ordered per-batch | High |
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
| TCP (TLS) | `0x00` Control, `0x01` Emergency, `0x12` Tile, `0x30` Clipboard, `0x31` File Transfer, `0x40` USB/IP, `0x50` Input, `0xF0` Plugin IPC |
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
| Magic | 2 bytes | `0x4C44` ("LD" for LiquiDE) |
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
| `0x0030` | `SecureAttention` | C → S | Secure Attention Sequence (privileged command, see below) |
| `0x0031` | `SecureAttentionAck` | S → C | Server acknowledges SAS command with result |

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

#### Video Frame Loss Recovery

Keyframes are sacred — loss of a keyframe or corruption of the decoder state results in visual corruption until the next keyframe. The protocol defines explicit recovery mechanisms:

| Scenario | Detection | Recovery | Max Black-Screen Time |
|----------|-----------|----------|-----------------------|
| **Delta frame lost** (unreliable transport) | Client detects sequence gap in `FrameHeader.seq` | Client sends `KeyFrameRequest`. Server generates IDR within 1 frame period. Client discards delta frames until IDR arrives. | 1–2 frame periods (~16–66ms) |
| **Keyframe lost** (unreliable transport) | Client detects gap AND `FrameHeader.is_keyframe = true` in the missing range | Client sends `KeyFrameRequest` with `urgent = true`. Server generates IDR immediately (bypasses encoder queue). | 1 frame period (~16ms) |
| **Decoder error** (corrupt bitstream) | Client decoder returns error | Client sends `KeyFrameRequest` with `reason = "decode_error"`. Client resets decoder state. Server generates IDR. | 1–2 frame periods |
| **Prolonged loss** (>500ms without valid frame) | Client timeout on frame arrival | Client sends `KeyFrameRequest`. If no response in 1s, requests transport-level reconnect. Shows "Reconnecting..." overlay after 2s. | 500ms (overlay at 2s) |
| **Reconnect / resume** | New transport established | Server sends IDR as first frame on reconnect (mandatory). No client request needed. | 0 (first frame is always IDR) |

**Keyframe protection strategy by transport:**

| Transport | Keyframe Protection | Rationale |
|-----------|-------------------|-----------|
| QUIC (unreliable datagrams) | Keyframes sent on a **reliable QUIC stream** (not datagrams). Delta frames use datagrams. | Keyframes are too large and important to lose. Reliable delivery adds ~1 RTT worst-case but guarantees arrival. |
| UDP (raw) | Keyframes use **FEC (Forward Error Correction)**: Reed-Solomon with 20% redundancy (configurable). Delta frames have no FEC. | FEC protects against loss without RTT penalty. 20% overhead is acceptable for keyframes (~1/sec). |
| TCP / TLS-TCP | Inherently reliable — no special handling needed. | TCP retransmits all lost segments. |
| WebRTC data channels | Keyframes sent on a **reliable** data channel. Delta frames on unreliable channel (`maxRetransmits: 0`). | Same split as QUIC: reliability for keyframes, speed for deltas. |

**IDR generation constraints:**

- Server MUST generate an IDR frame within 1 frame period of receiving `KeyFrameRequest`.
- Server MUST NOT rely solely on periodic keyframe intervals (I-frame interval) for recovery — explicit client-requested IDR is required.
- Server SHOULD insert periodic IDR frames at configurable intervals (default: every 5 seconds) as a safety net even without client requests.
- IDR frames are always sent as complete (not fragmented across unreliable packets) on reliable channels.

**Maximum black-screen-time SLO:** the client MUST NOT display a corrupted or frozen frame for more than **500ms** before either recovering (via IDR) or showing a user-visible "Reconnecting..." overlay. This is a hard enterprise SLO — visual corruption is never silently tolerated.

### 5.4 Tile Channel Messages (0x12)

The tile channel carries bitmap-based screen updates. It is used when the session (or a region of the session) operates in tile/bitmap mode. The tile channel is **reliable** — tile data must arrive intact because XOR deltas depend on the client having the correct previous tile state.

| Type Code | Name | Direction | Description |
|-----------|------|-----------|-------------|
| `0x1201` | `TileConfig` | S → C | Tile grid configuration (tile size, grid dimensions, color depth) |
| `0x1202` | `TileBatch` | S → C | Batch of tile updates for a single frame |
| `0x1203` | `TileBatchAck` | C → S | Client acknowledges a tile batch (for flow control) |
| `0x1204` | `TileScroll` | S → C | Scroll optimization: shift the tile grid by a vector |
| `0x1205` | `TileKeyFrame` | S → C | Full tile grid snapshot (all tiles as full, no deltas) |
| `0x1206` | `TileKeyFrameRequest` | C → S | Client requests a full tile refresh (after desync or reconnect) |
| `0x1207` | `TileModeSwitch` | S → C | Server switches region between video and tile mode |

**Message details:**

- **`TileConfig`** — sent once when the tile channel opens, and again if the tile grid changes (e.g., resize). Tells the client the tile size, grid width/height (in tiles), and pixel format.
- **`TileBatch`** — the primary data message. Contains a sequence of tile updates for a single compositor frame. Each tile update in the batch carries its grid coordinates and encoding type (`full`, `delta`, `copy`, `solid`, `skip`). Tiles flagged `skip` are omitted from the batch (implicitly unchanged).
- **`TileBatchAck`** — sent by the client after processing a `TileBatch`. Includes the batch sequence number. The server uses this for flow control (avoids sending more batches than the client can process).
- **`TileScroll`** — an optimization message: the server detected a scroll event and sends a scroll vector (dx, dy in tiles). The client shifts its tile buffer by this vector before applying the follow-up `TileBatch` (which contains only the newly exposed tiles).
- **`TileKeyFrame`** — a full refresh of the entire tile grid. Sent on initial connection, after reconnect, or when the client sends `TileKeyFrameRequest`. All tiles are `full` type (no deltas). This is the tile-mode equivalent of a video key frame.
- **`TileModeSwitch`** — informs the client that a rectangular region of the screen is switching between video mode and tile mode (for hybrid encoding). Contains the region bounds and the target mode.

#### Tile Channel Loss Recovery

The tile channel uses a **reliable** transport (ordered, retransmitted) because XOR deltas are stateful — a single lost tile corrupts all subsequent deltas for that grid position. Recovery mechanisms:

| Scenario | Detection | Recovery |
|----------|-----------|----------|
| **Transport-level loss** (TCP/QUIC stream retransmit) | Handled by transport | Automatic retransmit. No protocol-level action needed. Adds latency equal to ~1 RTT. |
| **Tile grid desync** (client state diverges from server's expectation) | Client detects delta that produces visual artifacts (optional CRC check per tile) | Client sends `TileKeyFrameRequest`. Server responds with full `TileKeyFrame` (all tiles as `full`, no deltas). |
| **Reconnect / resume** | New transport established | Server MUST send `TileConfig` + `TileKeyFrame` as the first tile-channel messages. Client discards all buffered tile state. |
| **Mode switch (video → tile)** | `TileModeSwitch` received | Server sends full tiles for the switched region (no delta possible since client has no prior tile state for that region). |

**Tile CRC verification (optional, configurable):**

Each `TileBatch` optionally includes a per-tile CRC-32 of the expected client-side tile state after applying the update. The client can verify the CRC after applying deltas. If a mismatch is detected, the client sends `TileKeyFrameRequest` for the affected region. This catches silent corruption from bugs or memory errors, not just transport loss. Enabled by default in debug/test builds, disabled in production for performance (adds ~2% CPU overhead).

### 5.5 Cursor Channel Messages (0x11)

| Type Code | Name | Direction | Description |
|-----------|------|-----------|-------------|
| `0x1101` | `CursorPosition` | S → C | Cursor position update (x, y) |
| `0x1102` | `CursorShape` | S → C | Cursor image/shape change |
| `0x1103` | `CursorVisibility` | S → C | Cursor show/hide |

### 5.6 Audio Channel Messages (0x20, 0x21)

| Type Code | Name | Direction | Description |
|-----------|------|-----------|-------------|
| `0x2001` | `AudioConfig` | Both | Audio format negotiation (sample rate, channels, codec) |
| `0x2002` | `AudioData` | Both | Encoded audio frame |
| `0x2003` | `AudioMute` | Both | Mute/unmute |
| `0x2004` | `AudioVolume` | Both | Volume level change |

### 5.7 Clipboard Channel Messages (0x30)

| Type Code | Name | Direction | Description |
|-----------|------|-----------|-------------|
| `0x3001` | `ClipboardOffer` | Both | Announce available clipboard formats |
| `0x3002` | `ClipboardRequest` | Both | Request clipboard data in specific format |
| `0x3003` | `ClipboardData` | Both | Clipboard content (possibly fragmented) |
| `0x3004` | `ClipboardDataEnd` | Both | End of clipboard data transfer |
| `0x3005` | `ClipboardClear` | Both | Clipboard cleared |
| `0x3006` | `ClipboardProgress` | Both | Transfer progress for large items |
| `0x3007` | `ClipboardCancel` | Both | Cancel ongoing transfer |

### 5.8 Input Channel Messages (0x50)

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
| `0x500C` | `TextInput` | C → S | Committed UTF-8 text from client IME (bypasses scancode-to-char mapping) |
| `0x500D` | `CompositionUpdate` | C → S | IME composition state (start/update/cancel with preedit string + cursor position) |
| `0x500E` | `CompositionRequest` | S → C | Server requests client to activate/deactivate IME composition (e.g., text field focused) |

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
    server_name: text,                         ; "LiquiDE"
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

### 8.5 CBOR Schema: Tile Channel

```cddl
TileConfig = {
    tile_size: uint,                           ; tile dimension in pixels (32, 64, 128, 256)
    grid_width: uint,                          ; number of tiles horizontally
    grid_height: uint,                         ; number of tiles vertically
    pixel_format: text,                        ; "rgb888", "rgba8888", "rgb565"
    codec: text,                               ; "zstd", "lz4", "png", "qoi", "webp", "raw"
    delta_enabled: bool,                       ; whether XOR deltas are used
    screen_width: uint,                        ; actual screen width in pixels
    screen_height: uint,                       ; actual screen height in pixels
}

TileBatch = {
    batch_id: uint,                            ; monotonic batch counter
    timestamp_us: uint,                        ; capture timestamp
    tile_count: uint,                          ; number of tile updates in this batch
    tiles: [+ TileUpdate],                     ; the tile updates
    ? scroll_precede: TileScrollVector,        ; if set, apply scroll before tiles
}

TileUpdate = {
    x: uint,                                   ; tile grid column (0-based)
    y: uint,                                   ; tile grid row (0-based)
    encoding: text,                            ; "full", "delta", "copy", "solid"
    ? data: bytes,                             ; compressed tile data (full or XOR delta)
    ? copy_source: uint,                       ; index into this batch's tile list (for "copy")
    ? solid_color: bytes,                      ; 3 or 4 bytes RGBA (for "solid")
    ? data_size: uint,                         ; uncompressed size hint (for pre-allocation)
}

; Encoding type semantics:
; "full"  — `data` contains the full tile bitmap, codec-compressed.
;           Client replaces tile buffer at (x, y) with decoded data.
; "delta" — `data` contains XOR of (current tile ^ previous tile), codec-compressed.
;           Client decompresses, XORs with its buffered tile at (x, y), stores result.
; "copy"  — tile is identical to another tile in this batch.
;           `copy_source` is the index of the source tile in the `tiles` array.
;           Client copies the decoded tile from that index.
; "solid" — tile is a single solid color.
;           `solid_color` is the RGBA value. Client fills tile buffer at (x, y).
; Tiles that are unchanged ("skip") are NOT included in the batch.

TileBatchAck = {
    batch_id: uint,                            ; batch being acknowledged
    decode_time_us: uint,                      ; time to decode + apply this batch
}

TileScrollVector = {
    dx: int,                                   ; horizontal scroll in tiles (positive = right)
    dy: int,                                   ; vertical scroll in tiles (positive = down)
}

TileScroll = {
    scroll: TileScrollVector,
    timestamp_us: uint,
}

TileKeyFrame = {
    batch_id: uint,
    timestamp_us: uint,
    tile_count: uint,
    tiles: [+ TileUpdate],                     ; all tiles, all "full" encoding
}

TileKeyFrameRequest = {
    reason: text,                              ; "reconnect", "desync", "user"
}

TileModeSwitch = {
    region: Rect,                              ; screen region in pixels
    mode: text,                                ; "video", "tile"
    timestamp_us: uint,
}
```

### 8.6 CBOR Schema: Input Events

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

### 8.7 CBOR Schema: Asset Cache Messages

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

### 8.8 CBOR Schema: Secure Attention Sequence

The Secure Attention Sequence (SAS) is a **privileged command channel** for operations that must not be spoofable by applications running inside the remote session. SAS commands are sent on the control channel (`0x00`), which terminates at the supervisor daemon — not at the session process. This ensures that a compromised session cannot intercept or forge SAS commands.

```cddl
SecureAttention = {
    command: text,                             ; SAS command (see table below)
    ? params: {* text => any},                 ; command-specific parameters
    nonce: uint,                               ; unique per-request, for ack correlation
    timestamp_us: uint,                        ; client timestamp
}

SecureAttentionAck = {
    nonce: uint,                               ; correlates to SecureAttention.nonce
    result: text,                              ; "ok", "denied", "error", "unsupported"
    ? reason: text,                            ; human-readable reason (on deny/error)
    ? data: {* text => any},                   ; command-specific response data
}
```

**SAS Commands:**

| Command | Description | Supervisor Action |
|---------|-------------|-------------------|
| `lock_session` | Lock the session (equivalent to Ctrl+Alt+L or Win+L) | Supervisor sends lock signal to session via IPC. Session shows lock screen. |
| `ctrl_alt_delete` | Send Ctrl+Alt+Delete to the session (SAS on Windows guests, task manager) | Supervisor injects the key combination directly into the session's input queue, bypassing any application-level key grabbing. |
| `switch_user` | Request user switch (show login screen for another user without terminating session) | Supervisor triggers VT switch or shows greeter. |
| `terminate_session` | Force-terminate the session process | Supervisor sends SIGTERM → SIGKILL to session. Client shows crash/disconnect screen. |
| `reboot_session` | Restart the session process | Supervisor terminates and respawns session. Client shows "Restarting..." overlay. |
| `screenshot` | Capture a screenshot of the current session (admin/support tool) | Supervisor captures framebuffer, returns as PNG in `SecureAttentionAck.data.image`. Policy-gated. |
| `change_password` | Request password change dialog (handled by supervisor, not session) | Supervisor invokes PAM password change flow. Credentials never pass through session process. |

**Security properties:**
- SAS commands are **never routed through the session process**. They are delivered to the supervisor daemon via the control channel, which the supervisor owns directly.
- A compromised session process cannot intercept, block, or forge SAS commands because it has no access to the control channel's transport endpoint.
- The client triggers SAS via a **dedicated key combination** (default: Ctrl+Alt+End, configurable) that is captured at the client's lowest input layer — before any application-level key handling.
- Each SAS command is audit-logged (`admin.action` event with `action = "sas.<command>"`).

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

### 10.3 Tile Channel State Machine

```
                    ┌────────────┐
                    │  Inactive  │ (channel not opened)
                    └──────┬─────┘
                           │ ChannelOpen
                           ▼
                    ┌────────────┐
                    │ Configuring│ (TileConfig sent, awaiting ack)
                    └──────┬─────┘
                           │ ChannelOpenAck
                           ▼
                    ┌────────────┐
                    │ Key Frame  │ (sending initial TileKeyFrame)
                    └──────┬─────┘
                           │ TileKeyFrame sent + ack
                           ▼
                    ┌────────────┐
                    │ Streaming  │ (TileBatch flow)
                    └──┬──┬──┬──┘
                       │  │  │
      TileKeyFrameReq │  │  │ TileConfig (resize)
                       │  │  ▼
                       │  │ ┌────────────┐
                       │  │ │Reconfiguring│ (new grid size, sends TileConfig)
                       │  │ └──────┬─────┘
                       │  │        │ Ack → Key Frame → Streaming
                       │  │        ▼
                       ▼  │   Streaming
                 Key Frame│
            (full refresh)│ ChannelClose
                       │  ▼
                       ▼ ┌────────┐
                  Streaming│ Closed │
                          └────────┘
```

**Key behaviors:**
- On entering **Key Frame** state, the server sends a `TileKeyFrame` containing every tile as `full`. The client replaces its entire tile buffer. This synchronizes client and server tile state.
- During **Streaming**, the server sends `TileBatch` messages with adaptive delta encoding. `TileScroll` messages can precede a `TileBatch` when a scroll is detected.
- A `TileKeyFrameRequest` from the client (e.g., after detecting tile corruption or on reconnect) transitions the server back to **Key Frame** state.
- On **Reconfiguring** (triggered by window resize), the tile grid is re-computed. The server sends a new `TileConfig` followed by a `TileKeyFrame` for the new grid dimensions.

### 10.4 Audio Channel State Machine

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

### 10.5 Clipboard Channel State Machine

```
                    ┌────────────┐
                    │   Idle     │ (channel open, no transfer active)
                    └──┬──┬──┬──┘
                       │  │  │
          ClipboardOffer│  │  │ ChannelClose
            (from peer) │  │  ▼
                       │  │ ┌────────┐
                       │  │ │ Closed │
                       │  │ └────────┘
                       ▼  │
                ┌──────────────┐
                │ Offer Pending│ (peer announced formats, awaiting local request)
                └──────┬───┬──┘
                       │   │
        ClipboardRequest│   │ timeout / ClipboardClear
                       │   ▼
                       │  Idle
                       ▼
                ┌──────────────┐
                │ Transferring │ (data chunks flowing)
                └──┬──┬──┬────┘
                   │  │  │
   ClipboardDataEnd│  │  │ ClipboardCancel
                   │  │  ▼
                   │  │ Idle (transfer aborted)
                   ▼  │
                 Idle  │ ChannelClose
                       ▼
                    ┌────────┐
                    │ Closed │
                    └────────┘
```

**Key behaviors:**
- The clipboard channel is **bidirectional** — both client and server can send `ClipboardOffer`.
- Only **one transfer** can be active at a time in each direction. A new `ClipboardOffer` while in **Transferring** state implicitly cancels the current transfer.
- **Policy enforcement** occurs at two points: (1) when an offer is received, the policy engine checks `clipboard.enabled` and `clipboard.direction`; (2) when data arrives, the policy engine checks `clipboard.max_size` and `clipboard.allowed_mime_types`.
- `ClipboardProgress` messages are informational during **Transferring** and do not change state.
- **Timeout**: if no `ClipboardRequest` arrives within 30 seconds of an offer, the offer expires and state returns to **Idle**.
- **Large clipboard transfers** (> 1 MB) are fragmented into `ClipboardData` chunks with `ClipboardDataEnd` marking the final chunk.

### 10.6 Input Channel State Machine

```
                    ┌────────────┐
                    │  Inactive  │
                    └──────┬─────┘
                           │ ChannelOpen
                           ▼
                    ┌────────────┐
                    │  Syncing   │ (InputSyncRequest/Response exchange)
                    └──────┬─────┘
                           │ InputSyncResponse received
                           ▼
                    ┌────────────┐
                    │   Active   │ (input events flowing C → S)
                    └──┬──┬──┬──┘
                       │  │  │
           reconnect   │  │  │ ChannelClose
           (sync lost) │  │  ▼
                       │  │ ┌────────┐
                       │  │ │ Closed │
                       │  │ └────────┘
                       ▼  │
                    Syncing│
                           │ ChannelSuspend
                           ▼
                    ┌────────────┐
                    │ Suspended  │ (session locked or minimized)
                    └──────┬─────┘
                           │ ChannelResume
                           ▼
                       Syncing → Active
```

**Key behaviors:**
- On channel open or reconnect, the client sends `InputSyncRequest`. The server responds with `InputSyncResponse` containing current modifier and button state. This prevents phantom keystrokes (e.g., a stuck Ctrl key after reconnect).
- During **Active** state, the client streams input events (key, mouse, touch) to the server. The server does not acknowledge individual input events (low-latency, fire-and-forget over reliable transport).
- **Input coalescing**: when the server is backpressured, mouse move events MAY be coalesced (latest position wins). Key events are NEVER coalesced.
- **Channel suspension** occurs when the session is locked or the client window is minimized. The server discards input events received during suspension.
- The input channel is **client-to-server only** for event data. The only server-to-client message is `InputSyncResponse`.

### 10.7 Cursor Channel State Machine

```
                    ┌────────────┐
                    │  Inactive  │
                    └──────┬─────┘
                           │ ChannelOpen
                           ▼
                    ┌────────────┐
                    │   Active   │ (cursor updates S → C)
                    └──┬──┬──┬──┘
                       │  │  │
     CursorVisibility  │  │  │ ChannelClose
     (hidden)          │  │  ▼
                       │  │ ┌────────┐
                       │  │ │ Closed │
                       │  │ └────────┘
                       ▼  │
                 ┌────────────┐
                 │   Hidden   │ (cursor invisible, position updates suppressed)
                 └──────┬─────┘
                        │ CursorVisibility (visible)
                        ▼
                     Active
```

**Key behaviors:**
- The cursor channel is **server-to-client only**. The server sends position updates, shape changes, and visibility toggles.
- Position updates are sent at a higher priority than video frames (low-latency cursor movement).
- **Cursor shape caching**: the server sends `CursorShape` with a shape hash. If the client has the shape cached, it uses the cache. New shapes include the full image data. The asset cache (§8.7) can pre-load common cursor shapes.
- **Rate limiting**: cursor position updates are capped at the frame rate (no more than one update per frame interval). During idle periods, no updates are sent.
- **Client-side prediction**: the client MAY render the local cursor position immediately (client-side cursor) and reconcile with server position updates. This is configurable via `cursor.client_side = true`.

---

## 11) Backpressure & Flow Control

All channels in the LiquiDE protocol implement backpressure mechanisms to prevent the sender from overwhelming the receiver. The specific mechanism varies by channel characteristics.

### 11.1 Flow Control Model

```
Sender                                  Receiver
  │                                         │
  │  ──── Data Message ──────────────────►  │
  │  ──── Data Message ──────────────────►  │
  │  ──── Data Message ──────────────────►  │
  │                                         │
  │  ◄──── Acknowledgment ────────────────  │ (credit-based)
  │        {received_seq, window_size}      │
  │                                         │
  │  ──── Data Message ──────────────────►  │ (sender respects window)
  │                                         │
```

### 11.2 Per-Channel Flow Control

| Channel | Mechanism | Window Size | Behavior When Full |
|---------|-----------|-------------|-------------------|
| **Control (0x00)** | No explicit flow control | N/A | Control messages are small and rate-limited by design. Sender-side rate limit: 100 messages/sec. |
| **Emergency (0x01)** | No flow control | N/A | Emergency channel is best-effort, zero-copy. Messages are never dropped by sender. |
| **Video (0x10)** | `FrameAck`-based | 3 frames in-flight | Server pauses encoding if 3 unacknowledged frames are in-flight. Drops to lower FPS or switches to tile mode. |
| **Tile (0x12)** | `TileBatchAck`-based | 2 batches in-flight | Server waits for ack before sending 3rd batch. Client reports `decode_time_us` in ack for adaptive tuning. |
| **Cursor (0x11)** | Rate-limited (no acks) | N/A | Server caps at 1 update per frame interval. During congestion, updates are coalesced (latest position wins). |
| **Audio (0x20/0x21)** | Jitter buffer feedback | 200ms buffer | Client reports buffer level. Server adjusts bitrate. If buffer overflows, oldest packets are dropped (audio glitch preferred over latency). |
| **Clipboard (0x30)** | `ClipboardProgress` + `ClipboardDataEnd` | 1 transfer at a time | Only one clipboard transfer per direction at a time. `ClipboardCancel` aborts. Max size enforced before transfer starts. |
| **Input (0x50)** | No acks (fire-and-forget) | N/A | Reliable transport guarantees delivery. Server coalesces mouse movements during overload. Key events are never dropped. |

### 11.3 Global Connection Backpressure

When the total send queue across all channels exceeds a high-water mark, global backpressure activates:

| Threshold | Action |
|-----------|--------|
| Send queue > 80% of `transport.send_buffer_size` | Video FPS reduced by 50%. Tile batch rate halved. |
| Send queue > 90% of `transport.send_buffer_size` | Video encoding paused. Only tile key updates, cursor, audio, and control messages proceed. |
| Send queue > 95% of `transport.send_buffer_size` | Audio bitrate reduced to minimum. Clipboard transfers paused. |
| Send queue = 100% | Only control and emergency channel messages proceed. All data channels stalled. |

Recovery is hysteretic: backpressure relaxes when the queue drops below 70% of the triggering threshold.

### 11.4 Bandwidth Estimation & Adaptation

The transport layer continuously estimates available bandwidth using:

1. **ACK timing** — RTT measurement from Ping/Pong and data acks.
2. **Packet loss** — detected via QUIC loss detection or TCP retransmit counters.
3. **Send buffer drain rate** — how fast the OS sends pending data.

Bandwidth estimate feeds into:
- Video encoder bitrate target.
- Tile compression level selection.
- Audio codec bitrate.
- Decision to switch between transmission modes (video ↔ tile ↔ client-side render).

### 11.5 Connection-Level Resource Limits

| Resource | Default Limit | Configurable |
|----------|---------------|-------------|
| Max concurrent channels | 16 | `transport.max_channels` |
| Max message size (pre-fragmentation) | 16 MB | `transport.max_message_size` |
| Max frame rate (video) | 60 fps | `performance.max_fps` |
| Max tile batch rate | 60 batches/sec | `performance.tile.max_batch_rate` |
| Max clipboard transfer size | 50 MB | `clipboard.max_size` |
| Max pending clipboard transfers | 1 per direction | Fixed |
| Input event rate limit | 1000 events/sec | `input.max_rate` |
| Control message rate limit | 100 messages/sec | Fixed |

---

## 12) Protocol Extension Mechanism

### 12.1 Capability Negotiation

Post-handshake, either side can announce additional capabilities via the `Capabilities` message (type `0x0018`).

#### CBOR Schema: Capabilities

```cddl
Capabilities = {
    action: text,                   ; "advertise" | "request" | "confirm" | "reject"
    capabilities: {* text => any},  ; capability_id => capability_value
    ? request_id: uint,             ; correlates request/confirm/reject
}
```

#### Negotiation Flow

```
Client                                  Server
  │                                         │
  │  ──── Capabilities ─────────────────►  │
  │       {action: "advertise",             │
  │        capabilities: {                  │
  │          "file_transfer": true,         │
  │          "usb_redirect": true,          │
  │          "webcam": true                 │
  │        }}                               │
  │                                         │
  │  ◄──── Capabilities ────────────────── │
  │        {action: "confirm",              │
  │         capabilities: {                 │
  │           "file_transfer": true,        │ (server supports it too)
  │           "usb_redirect": false,        │ (server denies — policy)
  │           "webcam": true                │
  │         }}                              │
  │                                         │
  │  At this point, file_transfer and       │
  │  webcam channels can be opened.         │
  │  usb_redirect is unavailable.           │
```

#### Known Capability Keys

| Capability Key | Value Type | Meaning |
|---------------|-----------|---------|
| `file_transfer` | bool | File transfer channel support |
| `usb_redirect` | bool | USB/IP device redirection |
| `webcam` | bool | Camera passthrough |
| `seamless_windows` | bool | Seamless window mode |
| `audio_capture` | bool | Microphone input |
| `clipboard_files` | bool | File list clipboard support |
| `clipboard_images` | bool | Image clipboard support |
| `clipboard_richtext` | bool | Rich text clipboard support |
| `tile_encoding` | bool | Tile/bitmap channel support |
| `client_render_offload` | bool | Mode C client-side rendering |
| `multi_monitor` | bool | Multi-monitor virtual screens |
| `pen_input` | bool | Stylus/pen input events |
| `gamepad_input` | bool | Gamepad input forwarding |
| `plugin_ipc` | bool | Plugin-to-client communication |

New capability keys MAY be introduced in any MINOR version. Unknown capability keys MUST be ignored by the receiver (respond with `false` or omit from the confirm message).

### 12.2 Protocol Version Extensions

Each protocol version MAY introduce:
- **New message types**: assigned type codes from reserved ranges (see §12.3).
- **New fields in existing CBOR schemas**: receivers MUST ignore unknown CBOR fields (forward compatibility).
- **New channels**: channel IDs from the reserved range.

### 12.3 Reserved Ranges

| Range | Allocation |
|-------|-----------|
| `0x0000–0x00FF` | Control channel messages |
| `0x0100–0x01FF` | Emergency channel messages |
| `0x1000–0x10FF` | Video channel messages |
| `0x1100–0x11FF` | Cursor channel messages |
| `0x1200–0x12FF` | Tile channel messages |
| `0x2000–0x21FF` | Audio channel messages |
| `0x3000–0x30FF` | Clipboard channel messages |
| `0x4000–0x40FF` | File transfer channel messages (reserved) |
| `0x5000–0x50FF` | Input channel messages |
| `0x6000–0x60FF` | USB channel messages (reserved) |
| `0x7000–0x70FF` | Webcam channel messages (reserved) |
| `0x8000–0x80FF` | Plugin IPC channel messages (reserved) |
| `0xE000–0xEFFF` | Vendor extensions (private use) |
| `0xF000–0xFFFF` | Experimental / testing (MUST NOT be used in production) |

### 12.4 Unknown Message Handling

When a receiver encounters a message type it does not recognize:

1. **Known channel, unknown type**: The message MUST be silently discarded. An optional `debug`-level log entry MAY be emitted. The receiver MUST NOT close the channel or connection.
2. **Unknown channel ID**: The `ChannelOpen` for the unknown channel MUST be rejected with `ChannelOpenReject` (reason: `unsupported_channel`). If data arrives on a channel that was never opened, it MUST be silently discarded.
3. **Unknown CBOR fields**: Receivers MUST ignore unknown fields in CBOR structures. This is the primary mechanism for forward compatibility.
4. **Malformed messages**: Messages that fail CBOR decoding or violate schema constraints MUST be discarded. If malformed messages exceed a threshold (10 per minute per channel), the channel MAY be closed with an error.

### 12.5 Vendor Extensions

The `0xE000–0xEFFF` message type range is reserved for vendor-specific extensions. Vendors MUST use the `Capabilities` mechanism to negotiate vendor extension support before sending vendor messages. The capability key format for vendor extensions is `vendor.<vendor_id>.<extension_name>`.

```cddl
; Example vendor extension capability
"vendor.acme.screenshare_annotations": {
    version: uint,          ; extension version
    max_annotations: uint,  ; max simultaneous annotations
}
```

---

## 13) Canonical Schema (CDDL)

The LiquiDE protocol uses **CBOR (RFC 8949)** as its payload encoding format. All message schemas are formally defined using **CDDL (RFC 8610)** — the Concise Data Definition Language. The CDDL source files are the **authoritative** schema definition; the prose descriptions in §5 and §8 are informative.

### 13.1 Schema Files

The canonical schemas live in the repository at `crates/liquide-protocol/schema/`:

| File | Contents |
|------|----------|
| `control.cddl` | Control channel messages (ClientHello, ServerHello, LoginPrompt, etc.) |
| `video.cddl` | Video channel messages (FrameHeader, FrameData, FrameAck) |
| `tile.cddl` | Tile channel messages (TileConfig, TileBatch, TileUpdate, etc.) |
| `cursor.cddl` | Cursor channel messages (CursorShape, CursorPosition) |
| `audio.cddl` | Audio channel messages (AudioConfig, AudioData) |
| `clipboard.cddl` | Clipboard channel messages (ClipboardOffer, ClipboardData, etc.) |
| `input.cddl` | Input channel messages (KeyEvent, MouseEvent, TouchEvent, etc.) |
| `emergency.cddl` | Emergency channel messages (EmergencyHello, CrashInfo, CrashLog) |
| `common.cddl` | Shared type definitions (session_id, error codes, enums) |

### 13.2 Schema Conventions

| Convention | Rule |
|-----------|------|
| **Integer keys** | CBOR maps use integer keys (not string keys) on the wire for compactness. String key names in CDDL are documentation only — the encoded form uses the integer mappings defined in each schema. |
| **Optional fields** | Optional fields (`? key`) MUST be omitted (not set to null) when absent. Receivers MUST accept both omission and explicit null for optional fields. |
| **Unknown fields** | Receivers MUST ignore unknown integer keys in CBOR maps (forward compatibility). Senders MUST NOT send fields not defined in the current protocol version's schema. |
| **Enums** | Enum values are encoded as unsigned integers, not strings. Mapping tables are defined per schema. |
| **Byte strings** | Binary data (frame payloads, tile pixels, audio samples) uses CBOR byte strings (`bstr`), not base64-encoded text. |
| **Timestamps** | All timestamps are `uint` microseconds since session start. Absolute timestamps (audit, logs) use ISO 8601 text strings. |

### 13.3 Schema Excerpt: Control Channel

```cddl
; common.cddl — shared types
session-id = tstr .size (1..64)
error-code = uint
protocol-version = tstr           ; e.g., "proto/1"

; control.cddl — control channel messages
ClientHello = {
    0: protocol-version,           ; protocol_version
    1: [+ tstr],                   ; supported_transports
    2: [+ tstr],                   ; supported_codecs
    3: {* tstr => any},            ; client_capabilities
    4: tstr,                       ; client_version
    5: tstr,                       ; client_platform ("linux-x86_64", "windows-x86_64", etc.)
    ? 6: bstr,                     ; session_resume_token (for reconnect)
}

ServerHello = {
    0: protocol-version,           ; selected protocol version
    1: tstr,                       ; selected_transport
    2: session-id,                 ; session_id
    3: {* tstr => any},            ; server_capabilities
    4: tstr,                       ; server_version
    ? 5: bstr,                     ; session_resume_token
    ? 6: uint,                     ; heartbeat_interval_ms
}

LoginPrompt = {
    0: [+ tstr],                   ; available_methods ("password", "totp", "fido2", etc.)
    ? 1: bstr,                     ; avatar_png (JPEG/PNG image bytes, ≤ 32KB)
    ? 2: bool,                     ; session_resume_available
    ? 3: tstr,                     ; server_greeting
}

LoginResponse = {
    0: tstr,                       ; method ("password", "totp", "fido2", etc.)
    1: bstr,                       ; credential (encrypted under TLS)
    ? 2: bstr,                     ; mfa_token (second-factor response)
}

LoginSuccess = {
    0: session-id,
    1: bstr,                       ; session_token
    2: {* tstr => any},            ; session_features (negotiated capabilities)
    ? 3: uint,                     ; token_lifetime_sec
}

LoginFailure = {
    0: error-code,
    1: tstr,                       ; reason ("invalid_credentials", "account_locked", "mfa_required")
    ? 2: uint,                     ; retry_after_sec
    ? 3: uint,                     ; remaining_attempts
}

Disconnect = {
    0: error-code,
    1: tstr,                       ; reason
    ? 2: bool,                     ; reconnect_allowed
}
```

### 13.4 Decode Strictness

Implementations MUST follow these decode rules:

| Rule | Strict Mode (default) | Lax Mode (optional, config) |
|------|----------------------|---------------------------|
| Unknown CBOR map keys | Silently ignored | Silently ignored |
| Missing required field | Reject message, emit error metric | Reject message |
| Wrong field type | Reject message | Attempt coercion (uint↔int), reject if impossible |
| Duplicate map keys | Reject message | Last value wins |
| Trailing bytes after CBOR | Reject message | Ignore trailing bytes |
| Indefinite-length CBOR | Reject (not supported) | Reject |
| CBOR tags | Ignored (strip) | Ignored (strip) |
| Nested depth > 8 | Reject message | Reject message |
| Single value > 16 MB | Reject message | Reject message |

Strict mode is the default for all production deployments. Lax mode MAY be enabled for interoperability testing with third-party clients. Lax mode MUST NOT be used in production as it masks protocol violations.

### 13.5 Schema Validation Tooling

```bash
# Validate a CBOR capture against the canonical schemas
liquide-conformance --validate-capture capture.bin --schema crates/liquide-protocol/schema/

# Generate Rust encode/decode code from CDDL schemas
cargo run --bin gen-protocol -- --schema crates/liquide-protocol/schema/ --output crates/liquide-protocol/src/generated/

# Validate that generated code matches the schema (CI check)
cargo test --package liquide-protocol -- schema_roundtrip
```

---

## 14) Test Vectors & Golden Captures

Test vectors provide known-good protocol message sequences that implementations MUST parse correctly. Golden captures provide known-good byte sequences for specific messages.

### 14.1 Golden Captures

Each message type has a canonical byte sequence stored in `crates/liquide-protocol/test-vectors/`:

| File | Contents | Format |
|------|----------|--------|
| `clienthello_basic.bin` | Minimal ClientHello with proto/1, QUIC, H.264 | Raw frame (header + CBOR payload) |
| `serverhello_basic.bin` | ServerHello response | Raw frame |
| `login_password_flow.bin` | LoginPrompt → LoginResponse → LoginSuccess | Concatenated frames |
| `login_failure.bin` | LoginPrompt → LoginResponse → LoginFailure | Concatenated frames |
| `tile_batch_mixed.bin` | TileBatch with skip, full, delta, copy, solid tiles | Raw frame |
| `tile_keyframe_1080p.bin` | TileKeyFrame for 1920×1080 at 64×64 tiles | Raw frame |
| `clipboard_text_roundtrip.bin` | ClipboardOffer → Request → Data → DataEnd | Concatenated frames |
| `disconnect_clean.bin` | Graceful Disconnect | Raw frame |
| `capability_negotiation.bin` | Client advertise → Server confirm | Concatenated frames |
| `emergency_crash.bin` | EmergencyHello → CrashInfo → CrashLogRequest → CrashLogChunk | Concatenated frames |
| `reconnect_resume.bin` | ClientHello with resume token → ServerHello → SessionInfo | Concatenated frames |

### 14.2 Golden Capture Format

Each `.bin` file is accompanied by a `.json` description file:

```json
{
    "vector_id": "clienthello_basic",
    "protocol_version": "proto/1",
    "description": "Minimal ClientHello with default options",
    "frames": [
        {
            "offset": 0,
            "length": 142,
            "channel": "0x00",
            "message_type": "0x0001",
            "description": "ClientHello",
            "decoded": {
                "protocol_version": "proto/1",
                "supported_transports": ["quic", "tcp"],
                "supported_codecs": ["h264", "tile-zstd"],
                "client_capabilities": {},
                "client_version": "0.3.0",
                "client_platform": "linux-x86_64"
            }
        }
    ],
    "sha256": "a1b2c3d4..."
}
```

### 14.3 Test Vector Requirements

| Requirement | Description |
|-------------|-------------|
| **Platform-independent** | Test vectors use fixed byte order (network byte order), fixed timestamps (0), and deterministic CBOR encoding (canonical form per RFC 8949 §4.2). |
| **Version-tagged** | Each vector specifies the minimum protocol version that supports it. |
| **CI-gated** | `cargo test --package liquide-protocol -- test_vector` runs all test vectors. New vectors MUST be added when new message types are introduced. |
| **Cross-platform** | Test vectors MUST produce identical parse results on all target platforms (x86_64, ARM64, WASM). |
| **Fuzz corpus seed** | Golden captures are automatically added to the fuzz corpus for frame parser and CBOR decoder targets. |

### 14.4 Compatibility Test Matrix

Client-server combinations tested with test vectors:

| Client Version | Server Version | Expected Behavior |
|---------------|----------------|-------------------|
| Current | Current | Full feature set, all vectors pass |
| Current | Current - 1 minor | Full feature set, server ignores unknown client capabilities |
| Current - 1 minor | Current | Full feature set, client ignores unknown server capabilities |
| Current | Current + 1 minor (future) | Server ignores unknown fields/capabilities from client |
| proto/1 client | proto/2 server (future) | Version negotiation falls back to proto/1 |

---

## 15) Wire Compatibility Policy

### 15.1 Protocol Version Contract

| Property | Rule |
|----------|------|
| **Frame header format** | Frozen. The 20/24-byte frame header (§4) MUST NOT change within a major protocol version. Adding fields requires a new major version. |
| **Magic number** | `0x4C44` is permanent. A different magic indicates a non-LiquiDE protocol. |
| **Channel IDs** | Channel ID assignments (§3.1) are frozen within a major protocol version. New channels use reserved IDs (§12.3). Existing channel IDs MUST NOT be reassigned. |
| **Message type codes** | Existing type codes are frozen. New message types use unallocated codes from reserved ranges (§12.3). Existing codes MUST NOT be reassigned or change semantics. |
| **CBOR field numbering** | Existing integer keys in CBOR maps are frozen. New optional fields use the next available integer key. Existing keys MUST NOT change meaning. |

### 15.2 Version Negotiation Rules

1. Client sends `ClientHello` with its highest supported protocol version.
2. Server selects the highest version it supports that is ≤ the client's version.
3. If no compatible version exists, server sends `LoginFailure` with reason `"version_incompatible"` and its supported version range.
4. The selected version applies to all channels for the session duration.
5. Mid-session version changes are NOT supported. Upgrade requires reconnect.

### 15.3 Forward Compatibility Rules

| Sender Action | Receiver Behavior |
|--------------|-------------------|
| Sends unknown CBOR field | Receiver ignores it |
| Sends unknown message type on known channel | Receiver silently discards message |
| Sends unknown capability key | Receiver responds with `false` or omits from confirm |
| Sends ChannelOpen for unknown channel | Receiver sends ChannelOpenReject |
| Uses a new compression algorithm ID | Receiver falls back to uncompressed and logs warning |

### 15.4 Breaking Change Policy

A protocol **major version bump** (`proto/1` → `proto/2`) is required for:

- Removing or changing the semantics of an existing message type.
- Removing or retyping an existing CBOR field.
- Changing the frame header layout.
- Changing the semantics of an existing flag bit.
- Reassigning a channel ID.

A protocol major version bump is **NOT** required for:

- Adding new optional CBOR fields to existing messages.
- Adding new message types in reserved ranges.
- Adding new channels in reserved ranges.
- Adding new capability keys.
- Adding new compression algorithm IDs.
- Adding new values to existing enums.

### 15.5 Deprecation Process

1. **Announce**: deprecated feature is documented in release notes with the version where it will be removed.
2. **Warn**: server and client emit `warn`-level log when a deprecated feature is used. Deprecation warning is included in `ServerHello.server_capabilities["deprecated_features"]`.
3. **Grace period**: minimum 2 minor versions between announcement and removal.
4. **Remove**: feature removed in the announced version. Clients using the deprecated feature receive a clear error.

---

## 16) Operational SLOs & Performance Targets

### 16.1 Latency Budgets

| Metric | Target (1080p, same-datacenter) | Target (1080p, WAN 50ms RTT) | Target (4K, same-datacenter) |
|--------|-------------------------------|------------------------------|------------------------------|
| Input-to-display (total) | < 16ms | < 50ms + RTT | < 25ms |
| Input processing | < 1ms | < 1ms | < 1ms |
| Compositor render | < 5ms | < 5ms | < 10ms |
| Encode (H.264) | < 5ms | < 5ms | < 10ms |
| Encode (AV1) | < 8ms | < 8ms | < 15ms |
| Tile batch encode (64×64) | < 3ms | < 3ms | < 5ms |
| Tile XOR delta (64×64) | < 0.1ms | < 0.1ms | < 0.1ms |
| Transport (packetize + send) | < 2ms | < 2ms | < 3ms |
| Client decode | < 3ms | < 3ms | < 5ms |
| Cursor update | < 5ms | < 5ms + RTT | < 5ms |
| Audio end-to-end | < 30ms | < 30ms + RTT | < 30ms |

### 16.2 Throughput Targets

| Metric | Target  |
|--------|---------|
| Frame rate (1080p, balanced) | 60 FPS sustained |
| Frame rate (4K, balanced) | 30 FPS sustained, 60 FPS achievable |
| Frame rate (idle, no damage) | 0 FPS (no frames sent when nothing changes) |
| Tile batch rate (1080p, active typing) | 30–60 batches/sec |
| Tile skip ratio (static screen) | > 99% (only cursor blink tiles sent) |
| Tile delta savings (vs. full tile, typical UI) | 60–90% bandwidth reduction |
| Audio stream | 48kHz stereo, Opus, < 128kbps |
| Clipboard (text, < 1MB) | < 100ms end-to-end |
| File transfer | Limited by network bandwidth |

### 16.3 Resource Budget (Server, per session)

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

### 16.4 CI Regression Thresholds

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

### 16.5 Network Emulation Scenarios

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

## 17) Fuzzing Targets

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

### 17.1 Fuzzing Infrastructure

- Fuzzing uses `cargo-fuzz` (libFuzzer) for Rust components.
- Corpus seeded from protocol conformance test recordings.
- CI runs fuzzing for a minimum of 1 hour per target on every release.
- Crashes are triaged as security issues (P0) until proven otherwise.

---

## 18) Conformance Tests

### 18.1 Test Categories

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
| Tile key frame | TileConfig → TileKeyFrame, all tiles full | Client reconstructs full screen from tiles |
| Tile delta | TileBatch with XOR deltas | Client applies XOR, result matches server bitmap |
| Tile scroll | TileScroll + TileBatch for exposed strip | Client shifts buffer, exposed tiles are correct |
| Tile copy/solid | TileBatch with copy and solid tiles | Client fills/copies correctly, pixel-perfect match |
| Tile mode switch | TileModeSwitch video↔tile | Client transitions regions without artifacts |
| Tile resize | Resize → TileConfig → TileKeyFrame | Client reconfigures grid, no desync |
| Tile key frame request | Client sends TileKeyFrameRequest | Server responds with full TileKeyFrame |
| Clipboard lifecycle | ClipboardOffer → Request → Data → DataEnd | Complete transfer, state returns to Idle |
| Clipboard cancel | ClipboardCancel during transfer | Transfer aborted, state returns to Idle |
| Clipboard timeout | No ClipboardRequest within 30s of offer | Offer expires, state returns to Idle |
| Clipboard policy block | Transfer violating direction policy | Transfer rejected, audit event emitted |
| Input sync | InputSyncRequest/Response on channel open | Client receives correct modifier state |
| Input coalescing | Rapid mouse moves under backpressure | Server coalesces to latest position, no key drops |
| Cursor shape cache | Repeated cursor shape changes | Client uses cached shapes, new shapes transferred |
| Backpressure video | 3 unacked frames in-flight | Server pauses encoding, resumes on ack |
| Backpressure tile | 2 unacked batches in-flight | Server waits for ack |
| Backpressure global | Send queue exceeds 90% | Video paused, cursor/audio/control continue |
| Capability negotiation | Client advertises, server confirms/rejects | Only confirmed capabilities activate channels |
| Unknown message type | Send unrecognized message type on known channel | Message silently discarded, channel stays open |
| Unknown CBOR field | Add extra field to known message | Receiver ignores field, processes message |
| Unknown channel | ChannelOpen for unrecognized channel ID | ChannelOpenReject with unsupported_channel |
| Vendor extension | Negotiate vendor cap, send vendor messages | Messages processed only after capability confirmed |

### 18.2 Conformance Test Runner

A standalone conformance test tool (`liquide-conformance`) can be run against any LiquiDE server to verify protocol compliance:

```bash
liquide-conformance --server <address> --username <user> --password <pass> --suite all
```

Outputs a pass/fail report per test case.

---

## 19) Test Plan

### Protocol Correctness
- Frame parsing: all field combinations, max sizes, truncated frames.
- CBOR encoding/decoding: round-trip all message types.
- State machine: all transition paths, including error paths.
- Sequence numbering: duplicate detection, wrap-around at 2^32.
- Timestamp: monotonicity, wrap-around handling.
- Tile: TileBatch round-trip with all encoding types (full, delta, copy, solid).
- Tile: XOR delta produces pixel-identical result to full tile on client.
- Tile: TileScroll + TileBatch produces correct shifted buffer.
- Tile: TileKeyFrame fully resynchronizes client after induced desync.

### Security
- TLS: verify only TLS 1.3 is accepted. Downgrade attack rejected.
- Authentication: brute-force rate limiting. Invalid credentials rejected.
- Channel injection: verify a client cannot send messages on server-only channels.
- Emergency channel: verify it cannot be used to bypass authentication.

### Performance
- All SLOs (§16) met under each network scenario (§16.5).
- Regression thresholds (§16.4) enforced in CI.

### Interoperability
- Conformance tests pass for: Linux client, Windows client, macOS client, browser client.
- Version mismatch: older client with newer server and vice versa.
- Backpressure: verify all channels respect flow control limits (§11).
- Extension negotiation: verify unknown capabilities are ignored, known capabilities activate features.
- Unknown messages: verify unknown message types are silently discarded (§12).
- Test vectors: verify all golden captures (§14) decode correctly on all client platforms.
- Compatibility: verify version negotiation rules (§15) are enforced.
