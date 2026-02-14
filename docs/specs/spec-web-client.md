# LiquiDE — Web Client Specification

> **Status**: Living document
> **Language**: TypeScript + Rust (WASM)
> **License**: MIT
> **Related specs**: [Client (Native)](spec-client.md) · [Server/DE](spec.md) · [Gateway](spec-gateway.md) · [Protocol](spec-protocol-formal.md) · [Design Language](spec-design.md) · [Threat Model](spec-threat-model.md)

---

## 1) Overview

The **LiquiDE Web Client** is a browser-based client for connecting to LiquiDE remote desktop sessions. It provides a reduced-but-functional feature set compared to the native LiquidClient, targeting environments where installing a native application is impractical — corporate-managed devices, Chromebooks, kiosks, or quick-access scenarios.

The web client runs entirely in the browser with **no plugins, extensions, or local installs**. It uses WebRTC for media transport, WebAssembly for protocol decoding, and the Canvas/WebGL APIs for rendering.

### Design Goals

1. **Zero install** — works in any modern browser (Chrome, Firefox, Safari, Edge).
2. **Acceptable latency** — not as fast as the native client, but usable for typical office workflows (text editing, web browsing, document work).
3. **Security-first** — runs within the browser sandbox. No access to host filesystem, USB, or raw sockets.
4. **Feature parity where possible** — clipboard, audio, resize, multi-monitor (tabbed) all work. Features requiring OS-level access (seamless windows, USB redirect, full keyboard capture) are unavailable or degraded.
5. **Progressive enhancement** — core experience works everywhere; advanced features (WebCodecs, WebGPU, WebTransport) activate when the browser supports them.

---

## 2) Browser Support

### 2.1 Tier Matrix

| Browser | Version | Tier | Notes |
|---------|---------|------|-------|
| Chrome / Chromium | 120+ | 1 | Full feature set. WebCodecs, WebTransport, WebGPU. |
| Edge | 120+ | 1 | Chromium-based, same as Chrome. |
| Firefox | 120+ | 1 | WebCodecs behind flag (landing). WebRTC full support. |
| Safari | 17.4+ | 2 | WebCodecs partial. No WebTransport. WebRTC works. |
| Chrome Android | 120+ | 2 | Touch input. Reduced decode performance. |
| Safari iOS/iPadOS | 17.4+ | 2 | WebCodecs partial. Touch input. |
| Samsung Internet | 24+ | 3 | Chromium-based, best-effort. |

### 2.2 Tier Definitions

| Tier | Testing | Support |
|------|---------|---------|
| **Tier 1** | Full CI + manual QA per release | Bugs are P1, blocking fix |
| **Tier 2** | CI smoke tests + periodic manual QA | Bugs are P2, best-effort fix |
| **Tier 3** | No regular testing | Community-reported bugs accepted |

### 2.3 Required Browser APIs

| API | Required | Fallback |
|-----|----------|----------|
| WebRTC (`RTCPeerConnection`) | Yes | None — required for media transport |
| WebSocket | Yes | None — required for control channel |
| WebAssembly | Yes | None — required for protocol codec |
| Canvas 2D | Yes | None — minimum rendering path |
| Web Audio API | Yes (for audio) | Silent mode (no audio) |
| Clipboard API (`navigator.clipboard`) | Required for clipboard | Clipboard disabled |
| WebCodecs (`VideoDecoder`) | Optional | WASM software decoder |
| WebGPU | Optional | Canvas 2D / WebGL fallback |
| WebGL 2 | Optional | Canvas 2D fallback |
| WebTransport | Optional | WebRTC data channels / WebSocket fallback |
| Keyboard Lock API | Optional | Partial keyboard capture only |
| Pointer Lock API | Optional | Absolute mouse only (no relative/FPS mode) |
| Fullscreen API | Optional | Windowed only |
| Screen Wake Lock API | Optional | Screen may sleep during idle sessions |
| Gamepad API | Optional | No gamepad input forwarding |

---

## 3) Architecture

### 3.1 Component Diagram

```
Browser Tab
├── UI Layer (TypeScript + HTML/CSS)
│   ├── Connection dialog (Liquid Glass themed)
│   ├── Login screen (client-rendered, same as native)
│   ├── Session toolbar (top bar: pin, fullscreen, settings, disconnect)
│   ├── Settings panel
│   └── Crash screen
│
├── Session Layer (TypeScript orchestrator)
│   ├── Connection state machine
│   ├── Channel demuxer / muxer
│   ├── Input capture (keyboard, mouse, touch)
│   ├── Clipboard bridge (Clipboard API ↔ protocol)
│   ├── Audio bridge (Web Audio API ↔ protocol)
│   └── Resize / display management
│
├── Protocol Layer (Rust → WASM)
│   ├── CBOR encode / decode (all message types)
│   ├── Frame deframing / framing
│   ├── LZ4 / Zstd decompression
│   ├── Tile XOR delta application
│   ├── Tile buffer management
│   └── Sequence number / ordering logic
│
├── Decode Layer
│   ├── WebCodecs VideoDecoder (preferred, hardware-accelerated)
│   ├── WASM software decoder (fallback: H.264 baseline via OpenH264-WASM)
│   └── Tile decoder (Zstd-WASM / raw)
│
├── Render Layer
│   ├── WebGPU (preferred: texture upload + present)
│   ├── WebGL 2 (fallback: texture upload + present)
│   └── Canvas 2D (fallback: ImageBitmap draw)
│
└── Transport Layer
    ├── WebRTC (media: video, audio, cursor)
    │   ├── RTCPeerConnection (ICE + DTLS-SRTP)
    │   ├── Data channels (reliable: control, clipboard, input)
    │   └── Media tracks (unreliable: video, audio)
    ├── WebTransport (when available, replaces WebRTC data channels)
    └── WebSocket (signaling + control channel fallback)
```

### 3.2 WASM Module

The protocol encoding/decoding logic is compiled from the shared `liquide-protocol` Rust crate to WebAssembly using `wasm-pack`. This ensures **byte-identical protocol handling** between native and web clients.

The WASM module exposes:

| Export | Purpose |
|--------|---------|
| `decode_frame(bytes) → Frame` | Parse wire frame header + CBOR payload |
| `encode_frame(msg) → bytes` | Serialize message to wire format |
| `decompress_lz4(bytes) → bytes` | LZ4 decompression |
| `decompress_zstd(bytes) → bytes` | Zstd decompression |
| `apply_tile_xor_delta(prev, delta) → tile` | XOR delta for tile channel |
| `decode_cbor(bytes) → object` | CBOR → JS object |
| `encode_cbor(object) → bytes` | JS object → CBOR |

WASM module size target: < 500 KB (gzip compressed).

### 3.3 Threading Model

The web client uses Web Workers for parallelism:

| Worker | Responsibility |
|--------|---------------|
| **Main thread** | UI rendering, DOM manipulation, event listeners, session orchestrator |
| **Protocol Worker** | WASM protocol decode/encode, CBOR processing, decompression. Receives raw bytes from transport, emits decoded messages to main thread via `postMessage`. |
| **Decode Worker** | Video frame decoding (WebCodecs or WASM software decoder). Receives encoded frame data, emits decoded `VideoFrame` or `ImageBitmap`. |
| **Audio Worker** (AudioWorklet) | Audio decode + playback via `AudioWorkletProcessor`. Low-latency audio pipeline. |

Communication between workers uses `postMessage` with `Transferable` objects (`ArrayBuffer`, `VideoFrame`, `ImageBitmap`) for zero-copy transfers.

---

## 4) Transport

### 4.1 Signaling

The web client cannot open raw TCP/UDP sockets. All transport is mediated through browser APIs.

**Signaling flow** (establishing the WebRTC connection):

```
Web Client                          Gateway / Server
    │                                       │
    │  ──── WebSocket connect ──────────►  │
    │       (wss://gateway/ws)             │
    │                                       │
    │  ──── ClientHello (CBOR/WS) ─────►  │  (protocol negotiation)
    │  ◄──── ServerHello (CBOR/WS) ──────  │
    │                                       │
    │  ──── LoginResponse (CBOR/WS) ───►  │  (auth over WebSocket)
    │  ◄──── LoginSuccess (CBOR/WS) ─────  │
    │                                       │
    │  ──── SDP Offer ──────────────────►  │  (WebRTC negotiation)
    │  ◄──── SDP Answer ────────────────── │
    │  ◄───► ICE Candidates ───────────►  │
    │                                       │
    │  ═══ WebRTC PeerConnection Open ═══  │
    │                                       │
    │  WebSocket stays open for signaling  │
    │  and control channel fallback.       │
```

### 4.2 WebRTC Channel Mapping

| LiquiDE Channel | WebRTC Mechanism | Ordered | Reliable | Notes |
|----------------|------------------|---------|----------|-------|
| Control (0x00) | Data Channel (ordered, reliable) | Yes | Yes | SCTP over DTLS |
| Emergency (0x01) | Data Channel (ordered, reliable) | Yes | Yes | Separate DC from control |
| Video (0x10) | Data Channel (unordered, unreliable) | No | No | `maxRetransmits: 0` |
| Tile (0x12) | Data Channel (ordered, reliable) | Yes | Yes | Binary frames |
| Cursor (0x11) | Data Channel (unordered, unreliable) | No | No | `maxRetransmits: 0` |
| Audio playback (0x20) | Data Channel (unordered, unreliable) | No | No | `maxRetransmits: 0` |
| Audio capture (0x21) | Data Channel (unordered, unreliable) | No | No | Microphone → server |
| Clipboard (0x30) | Data Channel (ordered, reliable) | Yes | Yes | Text + binary |
| Input (0x50) | Data Channel (ordered, reliable) | Yes | Yes | Low-latency input events |

**Why not `MediaStreamTrack`?** LiquiDE uses its own framing and codec negotiation, not the browser's built-in RTP/RTCP media stack. Using raw data channels gives full control over encoding, packetization, and frame boundaries. The server sends pre-encoded video frames, not raw RTP.

### 4.3 ICE / TURN / STUN

WebRTC requires ICE for NAT traversal. The web client ICE configuration:

```typescript
const rtcConfig: RTCConfiguration = {
    iceServers: [
        // STUN: public, for NAT type discovery
        { urls: "stun:stun.example.com:3478" },
        // TURN: relay fallback (UDP + TCP + TLS)
        {
            urls: [
                "turn:turn.example.com:3478?transport=udp",
                "turn:turn.example.com:3478?transport=tcp",
                "turns:turn.example.com:5349?transport=tcp",
            ],
            username: "<session-token>",
            credential: "<session-credential>",
        },
    ],
    iceTransportPolicy: "all",      // "relay" to force TURN
    bundlePolicy: "max-bundle",     // single transport for all channels
    rtcpMuxPolicy: "require",
};
```

#### ICE Candidate Gathering

| Phase | Candidates | Latency | Reliability |
|-------|-----------|---------|-------------|
| Host | Local IP (LAN only) | <1ms | Direct LAN |
| Server-reflexive (STUN) | Public IP via STUN | NAT-dependent | Works for most NATs |
| Relay (TURN-UDP) | TURN server relay | +RTT to TURN | Works through symmetric NAT |
| Relay (TURN-TCP) | TURN server relay (TCP) | Higher | Works through UDP-blocking firewalls |
| Relay (TURNS-TLS) | TURN server relay (TLS/TCP/443) | Highest | Works through strict corporate proxies |

The web client attempts candidates in order of decreasing performance. The `iceTransportPolicy` can be set to `"relay"` in environments where direct connections are prohibited by policy.

#### Gateway as TURN Server

The LiquiDE gateway (`liquid-gateway`) can optionally run a built-in TURN server:

```toml
# gateway.toml
[turn]
enabled = true
listen_udp = "0.0.0.0:3478"
listen_tcp = "0.0.0.0:3478"
listen_tls = "0.0.0.0:5349"
tls_cert = "/etc/liquid-gateway/turn-cert.pem"
tls_key = "/etc/liquid-gateway/turn-key.pem"
realm = "liquide.example.com"
# Credentials are session-bound, auto-provisioned during signaling
auth_mode = "session-token"       # short-lived per-session credentials
max_relayed_sessions = 500
max_bandwidth_per_session_kbps = 50000
```

### 4.4 WebTransport (Progressive Enhancement)

When the browser supports WebTransport (Chrome 114+), the web client can use it as an alternative to WebRTC data channels:

| Advantage | Description |
|-----------|-------------|
| Lower overhead | No SDP negotiation, no ICE (connects directly to server/gateway) |
| Unreliable datagrams | Native unreliable transport (like UDP) for video/audio/cursor |
| Multiplexed streams | Reliable bidirectional streams for control/clipboard/input |
| HTTP/3 compatible | Passes through HTTP/3-aware proxies |

**WebTransport channel mapping:**

| LiquiDE Channel | WebTransport Mechanism |
|----------------|----------------------|
| Control (0x00) | Bidirectional stream |
| Emergency (0x01) | Bidirectional stream |
| Video (0x10) | Datagrams |
| Tile (0x12) | Bidirectional stream |
| Cursor (0x11) | Datagrams |
| Audio (0x20/0x21) | Datagrams |
| Clipboard (0x30) | Bidirectional stream |
| Input (0x50) | Bidirectional stream |

WebTransport requires the server/gateway to support HTTP/3 with the WebTransport extension. When unavailable, the client falls back to WebRTC.

### 4.5 Transport Fallback Chain

```
Attempt 1: WebTransport (if browser supports)
    │ fail
    ▼
Attempt 2: WebRTC (data channels, ICE/TURN)
    │ fail
    ▼
Attempt 3: WebSocket-only (all channels multiplexed over single WS)
    │ fail
    ▼
Connection failed — show error
```

WebSocket-only mode is the last resort. All channels are multiplexed over a single reliable, ordered WebSocket connection. This works behind the most restrictive proxies but provides the worst latency (no unreliable transport, head-of-line blocking).

---

## 5) Video Decode & Rendering

### 5.1 Decode Pipeline

```
Encoded frame (from transport)
    │
    ▼
┌──────────────────┐
│ Is WebCodecs      │──── Yes ──► WebCodecs VideoDecoder
│ available?        │              (hardware-accelerated)
└──────────────────┘              │
    │ No                          │
    ▼                             ▼
WASM Software Decoder         VideoFrame / ImageBitmap
(OpenH264 compiled to WASM)       │
    │                             │
    ▼                             ▼
ImageBitmap                   Render to canvas
```

### 5.2 WebCodecs Decode

The preferred decode path uses the WebCodecs API (`VideoDecoder`) for hardware-accelerated decoding:

```typescript
const decoder = new VideoDecoder({
    output: (frame: VideoFrame) => {
        renderFrame(frame);
        frame.close();
    },
    error: (e: DOMException) => {
        console.error("Decode error:", e);
        requestKeyFrame();
    },
});

decoder.configure({
    codec: "avc1.42E01E",      // H.264 Baseline (server negotiated)
    codedWidth: 1920,
    codedHeight: 1080,
    hardwareAcceleration: "prefer-hardware",
});
```

**Codec support via WebCodecs:**

| Codec | WebCodecs Codec String | Browser Support | Notes |
|-------|----------------------|-----------------|-------|
| H.264 Baseline | `avc1.42E01E` | All Tier 1 | Universal, required minimum |
| H.264 Main | `avc1.4D401E` | All Tier 1 | Better compression |
| H.265/HEVC | `hvc1.1.6.L93.B0` | Safari, Chrome 107+ | Patent considerations |
| VP9 | `vp09.00.10.08` | Chrome, Firefox | Royalty-free |
| AV1 | `av01.0.04M.08` | Chrome 94+, Firefox 98+ | Best compression, royalty-free |

The server negotiates the codec based on the web client's `ClientHello` capabilities, which are determined by probing `VideoDecoder.isConfigSupported()` at startup.

### 5.3 WASM Software Decoder (Fallback)

When WebCodecs is not available (older browsers, Safari <17.4):

- **OpenH264** compiled to WASM via Emscripten.
- Decodes H.264 Baseline profile only.
- Performance: ~30 fps at 720p on a modern desktop CPU. Not suitable for 1080p60.
- The server reduces resolution and frame rate when the web client reports software decode.

WASM decoder module size: ~800 KB (gzip).

### 5.4 Tile Decode

Tile channel data is decoded in the Protocol Worker (WASM):

1. Receive `TileBatch` message.
2. For each `TileUpdate` in the batch:
   - `full`: decompress (Zstd/LZ4 in WASM), store in tile buffer.
   - `delta`: decompress, XOR with previous tile in buffer, store result.
   - `copy`: copy from another tile in the batch.
   - `solid`: fill tile buffer with solid color.
3. Transfer updated tile bitmaps to main thread as `ImageBitmap` (zero-copy via `Transferable`).
4. Render updated tiles to canvas at their grid positions.

### 5.5 Rendering

| Renderer | API | Performance | Notes |
|----------|-----|-------------|-------|
| WebGPU | `GPUDevice`, `GPUTexture` | Best | Texture upload + shader-based present. Lowest CPU overhead. |
| WebGL 2 | `WebGL2RenderingContext` | Good | Texture upload + draw. Wide support. |
| Canvas 2D | `CanvasRenderingContext2D` | Acceptable | `drawImage` with `ImageBitmap`. CPU-bound. |

Renderer is selected automatically based on browser capability. User can override in settings.

### 5.6 Color Space & HDR

The web client supports the three color pipeline modes defined in the protocol. Browser API availability determines which modes the client can offer.

**Browser API Availability:**

| Capability | API | Chrome | Firefox | Safari | Notes |
|-----------|-----|--------|---------|--------|-------|
| Wide gamut canvas (P3) | `canvas.getContext('2d', {colorSpace: 'display-p3'})` | 104+ | 127+ | 16.4+ | Used for WCG-SDR tile rendering |
| WebGPU P3 texture | `GPUTexture` with `bgra8unorm` + P3 color space | 113+ | Nightly | 17.0+ | Used for WCG-SDR GPU-accelerated path |
| WebGL P3 | `drawingBufferColorSpace: 'display-p3'` | 104+ | 127+ | 16.4+ | Used for WCG-SDR WebGL path |
| 10-bit WebCodecs decode | `VideoDecoder` with 10-bit H.265/AV1 profile | 107+ | Partial | 17.0+ | Required for WCG/HDR video decode. H.265 requires platform decoder support. |
| HDR canvas | `canvas.configureHighDynamicRange({mode: 'extended'})` | Experimental | No | No | PQ/HLG output. Extremely limited browser support. |
| WebGPU HDR texture | `GPUCanvasConfiguration` with `rgba16float` | Experimental | No | No | Float16 output for HDR. |

**Fallback Behavior:**

| Server Mode | Client Has API | Behavior |
|-------------|---------------|----------|
| SDR-sRGB | Always | Direct 8-bit sRGB rendering (no special handling) |
| WCG-SDR | P3 canvas available | Render tiles/frames in P3 color space using `colorSpace: 'display-p3'` on canvas |
| WCG-SDR | No P3 canvas | Request SDR-sRGB fallback from server (re-negotiate via `Capabilities` message) |
| HDR | HDR canvas + 10-bit decode | PQ/HLG frames rendered to HDR canvas. Extremely rare in practice. |
| HDR | No HDR canvas, has P3 | Client applies software tone mapping (Reinhard in WASM) to map PQ→SDR, render in P3 |
| HDR | No P3 canvas | Request SDR-sRGB fallback from server |

**ClientHello color capabilities for the web client:**
- `color.supported_modes`: auto-detected from browser API probing. Typically `["sdr-srgb"]` on most browsers, `["sdr-srgb", "wcg-sdr"]` on modern browsers with P3 displays.
- `color.display_gamut`: probed via `matchMedia('(color-gamut: p3)')`. `"display-p3"` if true, `"srgb"` otherwise.
- `color.display_hdr`: `false` (no reliable browser API to detect HDR display yet).
- `color.preferred_bit_depth`: `8` (default), `10` if WebCodecs 10-bit decode is available.

---

## 6) Audio

### 6.1 Playback

Audio playback uses the Web Audio API with an `AudioWorklet` for low-latency output:

```
Server → Audio Data (Opus encoded)
    │
    ▼
Protocol Worker (WASM: Opus decode)
    │
    ▼
AudioWorkletProcessor (ring buffer → audio output)
    │
    ▼
AudioContext destination (speakers)
```

- Opus decoding is performed in WASM (opus-WASM or compile from opus crate).
- Jitter buffer: 100ms (configurable). Higher than native client (20ms) due to Web Audio scheduling constraints.
- Sample rate: 48 kHz stereo (downmixed to mono if needed).

### 6.2 Capture (Microphone)

Microphone capture requires user permission (`getUserMedia`):

```
Microphone → MediaStreamTrack
    │
    ▼
AudioWorkletProcessor (capture ring buffer)
    │
    ▼
Protocol Worker (WASM: Opus encode)
    │
    ▼
Transport → Server
```

- Capture is **opt-in** — the browser will prompt for microphone permission.
- Capture is disabled by default in the web client. User must explicitly enable it.

---

## 7) Input Handling

### 7.1 Keyboard

| Feature | Browser API | Status |
|---------|------------|--------|
| Basic key events | `KeyboardEvent` | Full support |
| `code` (physical key) | `KeyboardEvent.code` | Full support |
| `key` (logical key) | `KeyboardEvent.key` | Full support |
| System shortcuts (Alt+Tab, Cmd+Tab) | Keyboard Lock API | Chrome only, fullscreen only |
| IME / compose | `CompositionEvent` | Supported (see §7.4) |
| Dead keys | `KeyboardEvent` + `CompositionEvent` | Supported |

**Keyboard Lock** (Chrome, fullscreen only): when the user enters fullscreen and grants keyboard lock permission, the web client captures system-level shortcuts (Alt+Tab, Super/Win, etc.) and forwards them to the remote session. Outside fullscreen or in non-supporting browsers, these shortcuts are handled by the OS.

```typescript
// Request keyboard lock in fullscreen
if ("keyboard" in navigator && "lock" in navigator.keyboard) {
    await navigator.keyboard.lock([
        "Escape", "AltLeft", "AltRight",
        "MetaLeft", "MetaRight", "Tab",
    ]);
}
```

### 7.2 Mouse

| Feature | Browser API | Status |
|---------|------------|--------|
| Position (absolute) | `MouseEvent` | Full support |
| Buttons | `MouseEvent.buttons` | Full support |
| Scroll (discrete) | `WheelEvent` | Full support |
| Scroll (smooth/pixel) | `WheelEvent.deltaMode` | Full support |
| Pointer Lock (relative mouse) | Pointer Lock API | Full support (requires user gesture) |
| High-resolution movement | `MouseEvent.movementX/Y` | With Pointer Lock |

**Pointer Lock** is required for relative mouse input (games, FPS-style camera). Activated on user gesture (click). Released on Escape.

### 7.3 Touch

| Feature | Browser API | Status |
|---------|------------|--------|
| Touch events | `TouchEvent` | Full support (mobile) |
| Multi-touch | `TouchEvent.touches` | Up to 10 points |
| Pointer events | `PointerEvent` | Unified mouse/touch/pen |
| Pinch-to-zoom | Custom gesture recognition | Client-side zoom (not forwarded) |

Touch events are translated to LiquiDE `TouchDown/TouchMove/TouchUp` protocol messages. Pinch-to-zoom controls the client viewport zoom, not the remote session.

### 7.4 IME / Composition

The web client supports Input Method Editors (IME) for CJK and other complex scripts:

1. `compositionstart` / `compositionupdate` / `compositionend` events are tracked.
2. During composition:
   - Individual key events are **not** forwarded to the server (they are consumed by the IME).
   - The composition string (preedit) is forwarded via a `CompositionUpdate` control message so the server can display the preedit inline.
3. On `compositionend`:
   - The committed text is sent as a single `TextInput` message.
   - The server inserts the committed text at the cursor position.

This matches the IME model described in the Wayland IME section of spec.md.

---

## 8) Clipboard

### 8.1 Browser Clipboard API

The web clipboard is accessed via the Clipboard API (`navigator.clipboard`):

| Operation | API | Permission |
|-----------|-----|------------|
| Read text | `navigator.clipboard.readText()` | Requires focus + permission |
| Write text | `navigator.clipboard.writeText()` | Requires focus |
| Read rich (images, HTML) | `navigator.clipboard.read()` | Requires focus + permission |
| Write rich | `navigator.clipboard.write()` | Requires focus |

### 8.2 Clipboard Sync Flow

```
Remote clipboard change (server → client):
    Server sends ClipboardOffer on clipboard channel
    → Web client receives offer
    → User clicks "Paste" or Ctrl+V in local context
    → Web client requests data from server (ClipboardRequest)
    → Server sends ClipboardData
    → Web client writes to navigator.clipboard

Local clipboard change (client → server):
    User copies in local context
    → Web client detects focus event (cannot poll clipboard)
    → On next paste gesture OR explicit "Sync Clipboard" button press:
        → Web client reads from navigator.clipboard
        → Sends ClipboardOffer + ClipboardData to server
```

### 8.3 Limitations

| Limitation | Reason | Mitigation |
|------------|--------|------------|
| Cannot detect clipboard changes in background | Browser security model | "Sync Clipboard" button, sync on focus |
| Permission prompt on first read | Browser requires user gesture | Prompt once, permission persisted per origin |
| No file clipboard | Clipboard API doesn't support file lists | Files shown as text paths; use file transfer for actual files |
| Size limit varies by browser | Browser-imposed | Warn on large clipboard (>4 MB) |

---

## 9) Feature Parity Matrix

| Feature | Native Client | Web Client | Notes |
|---------|:------------:|:----------:|-------|
| Video streaming (H.264) | Yes | Yes | WebCodecs or WASM fallback |
| Video streaming (AV1/VP9) | Yes | Yes | WebCodecs only |
| Tile mode (bitmap) | Yes | Yes | WASM XOR delta |
| Hybrid mode (video+tile) | Yes | Yes | Same as native |
| Audio playback | Yes | Yes | Web Audio + AudioWorklet |
| Audio capture (mic) | Yes | Yes | getUserMedia (permission required) |
| Clipboard (text) | Yes | Yes | Clipboard API |
| Clipboard (images) | Yes | Partial | `read()`/`write()` with ClipboardItem |
| Clipboard (files) | Yes | No | Browser sandbox prevents file list clipboard |
| Cursor (client-side prediction) | Yes | Yes | CSS `cursor: none` + canvas overlay |
| Cursor (custom shapes) | Yes | Yes | `CursorShape` → canvas draw |
| Single monitor | Yes | Yes | Single canvas |
| Multi-monitor (tabbed) | Yes | Yes | Tab UI with canvas per tab |
| Multi-monitor (multi-window) | Yes | No | Cannot create native OS windows from web |
| Fullscreen | Yes | Yes | Fullscreen API |
| Keyboard capture (basic) | Yes | Yes | `KeyboardEvent` |
| Keyboard capture (system keys) | Yes | Partial | Keyboard Lock API (Chrome, fullscreen) |
| Mouse (absolute) | Yes | Yes | `MouseEvent` |
| Mouse (relative / pointer lock) | Yes | Yes | Pointer Lock API |
| Touch input | Yes | Yes | `TouchEvent` / `PointerEvent` |
| Gamepad input | Yes | Partial | Gamepad API (limited) |
| IME / CJK input | Yes | Yes | `CompositionEvent` bridge |
| USB/IP redirect | Yes | No | No raw USB access in browser |
| Seamless windows | Yes | No | Cannot create native OS windows |
| File transfer | Yes | Partial | Upload/download via drag-drop or file picker |
| Camera passthrough | Yes | Yes | `getUserMedia` (permission required) |
| Custom window chrome | Yes | N/A | Web client has its own toolbar |
| Session resume | Yes | Yes | Resume token in `sessionStorage` |
| Connection profiles | Yes | Yes | `localStorage` or server-synced |
| Crash screen | Yes | Yes | Client-rendered in DOM |
| Auto-reconnect | Yes | Yes | WebSocket + re-negotiate WebRTC |
| WebTransport | N/A | Yes | When available |
| QUIC (native) | Yes | N/A | Browser uses WebRTC/WebTransport instead |
| TCP + UDP (native) | Yes | N/A | Browser uses WebRTC/WebTransport instead |
| Printing | Yes | No | No remote print redirect |
| Screen Wake Lock | N/A | Yes | Prevent screen sleep during session |

---

## 10) Security

### 10.1 Browser Sandbox

The web client runs within the browser's security sandbox. It has:

- **No filesystem access** (except via File API for explicit user-selected files).
- **No raw socket access** (all networking via WebRTC, WebTransport, WebSocket, fetch).
- **No USB access** (WebUSB exists but is not used for security reasons — USB redirect is not supported).
- **No process spawning** capability.
- **Origin-isolated** — session data is scoped to the web client's origin.

### 10.2 Authentication

Authentication flows are identical to the native client:

1. WebSocket connection to gateway/server.
2. TLS (wss://) ensures transport encryption.
3. `ClientHello` → `ServerHello` → `LoginPrompt` → `LoginResponse` → `LoginSuccess/Failure`.
4. Session token stored in `sessionStorage` (cleared on tab close) or `localStorage` (for "remember me").

**OIDC/SAML SSO in the browser:**

For enterprise deployments, the web client supports browser-native SSO:

1. Server responds to `LoginPrompt` with `method: "oidc"` and an `auth_url`.
2. Web client opens a popup window to the OIDC authorization URL.
3. User authenticates with the IdP in the popup.
4. IdP redirects back to the web client origin with an authorization code.
5. Web client sends the authorization code to the server via `LoginResponse`.
6. Server exchanges the code for tokens and completes auth.

This avoids embedding credentials in the web client and leverages the browser's existing SSO session with the IdP.

### 10.3 Content Security Policy

The web client is served with a strict CSP:

```
Content-Security-Policy:
    default-src 'none';
    script-src 'self' 'wasm-unsafe-eval';
    style-src 'self' 'unsafe-inline';
    connect-src wss://*.example.com https://*.example.com;
    img-src 'self' data: blob:;
    media-src 'self' blob:;
    worker-src 'self' blob:;
    frame-src 'none';
    object-src 'none';
    base-uri 'none';
    form-action 'none';
```

`wasm-unsafe-eval` is required for WebAssembly execution. `connect-src` is configured per-deployment to allow WebSocket and WebRTC connections to the gateway/server.

### 10.4 Credential Storage

| Data | Storage | Lifetime | Encryption |
|------|---------|----------|------------|
| Session token | `sessionStorage` | Tab close | None (browser-managed) |
| "Remember me" token | `localStorage` | Explicit expiry (server-set) | None (browser-managed) |
| OIDC tokens | Handled by IdP session | Browser session | HttpOnly cookies (IdP) |
| Connection profiles | `localStorage` | Persistent | Optional: Web Crypto API (AES-GCM with user passphrase) |
| Cached wallpapers/avatars | Cache API | Persistent | None (non-sensitive) |

**No credentials are ever stored in cookies.** Cookies are not used for session management. All auth state is in `sessionStorage` or `localStorage`, scoped to the web client origin.

### 10.5 Subresource Integrity

All static assets (JS, WASM, CSS) are served with SRI hashes:

```html
<script src="/app.js" integrity="sha384-..." crossorigin="anonymous"></script>
<script src="/protocol.wasm" integrity="sha384-..." crossorigin="anonymous"></script>
```

---

## 11) Deployment

### 11.1 Hosting

The web client is a **static web application** — a set of HTML, CSS, JavaScript, and WASM files served via any HTTP server. No server-side rendering or dynamic backend is needed for the web client itself.

Deployment options:

| Option | Description |
|--------|-------------|
| **Gateway-hosted** | `liquid-gateway` serves the web client static files on its HTTPS port. Zero additional infrastructure. |
| **CDN-hosted** | Static files on a CDN (CloudFront, Cloudflare, etc.). Lowest latency for initial load. Gateway/server handles only WebSocket/WebRTC. |
| **Self-hosted** | nginx, Caddy, or any web server serves the static files. |
| **Embedded in Manager UI** | The `liquid-manager` web UI can embed the web client for quick-connect from the management interface. |

### 11.2 Build

```bash
# Build web client (from monorepo root)
cd crates/liquide-web-client

# Build WASM protocol module
wasm-pack build --target web --release ../liquide-protocol

# Build WASM decoder module (OpenH264)
wasm-pack build --target web --release ../liquide-web-decoder

# Build TypeScript + bundle
npm run build

# Output: dist/
#   ├── index.html
#   ├── app.js                    (~150 KB gzip)
#   ├── protocol.wasm             (~300 KB gzip)
#   ├── decoder.wasm              (~800 KB gzip, optional)
#   ├── audio-worklet.js          (~10 KB gzip)
#   ├── style.css                 (~30 KB gzip)
#   └── assets/
#       ├── icons/
#       └── fonts/
```

Total initial load: ~500 KB (without software decoder). Software decoder WASM is loaded on-demand only when WebCodecs is unavailable.

### 11.3 Crate Location

```
crates/
├── liquide-web-client/           # Web client (TypeScript + WASM glue)
│   ├── src/
│   │   ├── index.ts              # Entry point
│   │   ├── session.ts            # Session orchestrator
│   │   ├── transport/
│   │   │   ├── webrtc.ts         # WebRTC transport
│   │   │   ├── webtransport.ts   # WebTransport transport
│   │   │   └── websocket.ts      # WebSocket fallback
│   │   ├── decode/
│   │   │   ├── webcodecs.ts      # WebCodecs decode path
│   │   │   └── wasm-decoder.ts   # WASM fallback decoder
│   │   ├── render/
│   │   │   ├── webgpu.ts         # WebGPU renderer
│   │   │   ├── webgl.ts          # WebGL 2 renderer
│   │   │   └── canvas.ts         # Canvas 2D renderer
│   │   ├── input/
│   │   │   ├── keyboard.ts       # Keyboard capture
│   │   │   ├── mouse.ts          # Mouse + pointer lock
│   │   │   ├── touch.ts          # Touch input
│   │   │   └── ime.ts            # IME / composition
│   │   ├── audio/
│   │   │   ├── playback.ts       # Audio playback
│   │   │   ├── capture.ts        # Microphone capture
│   │   │   └── worklet.ts        # AudioWorklet processor
│   │   ├── clipboard.ts          # Clipboard bridge
│   │   ├── ui/
│   │   │   ├── connection.ts     # Connection dialog
│   │   │   ├── login.ts          # Login screen
│   │   │   ├── toolbar.ts        # Session toolbar
│   │   │   ├── settings.ts       # Settings panel
│   │   │   └── crash.ts          # Crash screen
│   │   └── workers/
│   │       ├── protocol-worker.ts
│   │       └── decode-worker.ts
│   ├── package.json
│   ├── tsconfig.json
│   └── vite.config.ts            # Bundler config
│
├── liquide-web-decoder/          # WASM software decoder (Rust → WASM)
│   ├── src/lib.rs                # OpenH264 WASM wrapper
│   └── Cargo.toml
│
└── liquide-protocol/             # Shared (also compiled to WASM for web)
    └── ...                       # Same crate used by native + web client
```

---

## 12) Performance Targets

### 12.1 Web-Specific SLOs

| Metric | Target (LAN) | Target (WAN 50ms RTT) | Notes |
|--------|-------------|----------------------|-------|
| Input-to-photon (WebCodecs) | p50 < 25ms, p99 < 40ms | p50 < 80ms + RTT | ~10ms overhead vs native |
| Input-to-photon (WASM decode) | p50 < 50ms, p99 < 80ms | p50 < 110ms + RTT | Software decode adds ~25ms |
| First frame after connect | < 1500ms | < 2500ms | WebRTC negotiation adds latency |
| Audio latency | < 80ms | < 80ms + RTT | Web Audio jitter buffer is larger |
| Initial page load | < 2s (cached) | < 4s (cached) | WASM compile time included |
| WASM compile time | < 500ms | < 500ms | Streaming compilation |

### 12.2 Performance Degradation vs Native

| Component | Native Client | Web Client | Overhead Source |
|-----------|:------------:|:----------:|----------------|
| Decode (H.264, hardware) | ~2ms | ~3ms | WebCodecs API overhead |
| Decode (H.264, software) | ~8ms | ~25ms | WASM interpreter overhead |
| Tile XOR delta | ~0.1ms | ~0.5ms | WASM + ArrayBuffer copy |
| Render (texture upload) | ~1ms | ~2ms | WebGPU/WebGL upload path |
| Input capture | ~0.1ms | ~1ms | Event loop scheduling |
| Audio (playback) | ~5ms buffer | ~50ms buffer | AudioWorklet scheduling |
| Clipboard (text) | ~1ms | ~10ms | Async Clipboard API |
| Connection setup | ~200ms | ~1000ms | ICE negotiation |

### 12.3 Adaptive Quality

The web client reports its decode capability type in `ClientHello`:

```
client_capabilities: {
    "decode_mode": "webcodecs",        // or "wasm-software"
    "max_decode_fps_estimate": 60,     // or 30 for WASM
    "max_decode_resolution": "1920x1080",  // or "1280x720" for WASM
    "web_client": true,
}
```

The server uses these capabilities to:
- Reduce resolution for WASM decode clients.
- Reduce FPS target (30 fps for WASM, 60 fps for WebCodecs).
- Prefer tile mode over video mode for WASM clients (tiles compress better in WASM).
- Select H.264 Baseline for maximum WASM decoder compatibility.

---

## 13) Configuration

### 13.1 Web Client Settings

Settings are stored in `localStorage` and exposed via a settings panel (gear icon in toolbar):

```json
{
    "display": {
        "renderer": "auto",
        "scale": 1.0,
        "max_fps": 60,
        "prefer_tile_mode": false
    },
    "audio": {
        "playback_enabled": true,
        "capture_enabled": false,
        "jitter_buffer_ms": 100
    },
    "input": {
        "keyboard_lock_fullscreen": true,
        "pointer_lock_enabled": true,
        "touch_to_mouse": true,
        "scroll_sensitivity": 1.0
    },
    "clipboard": {
        "enabled": true,
        "auto_sync_on_focus": true,
        "max_size_kb": 4096
    },
    "transport": {
        "prefer": "auto",
        "ice_transport_policy": "all",
        "custom_stun_servers": [],
        "custom_turn_servers": []
    },
    "performance": {
        "decode_mode": "auto",
        "web_workers": true,
        "wasm_threads": true
    },
    "connection": {
        "auto_reconnect": true,
        "reconnect_delay_ms": 1000,
        "max_reconnect_attempts": 10
    }
}
```

### 13.2 Server-Side Configuration

The server/gateway enables web client support with:

```toml
# gateway.toml or server.toml
[web_client]
enabled = true
serve_static = true                    # serve web client files from gateway
static_path = "/usr/share/liquide/web-client/"
allowed_origins = ["https://remote.example.com"]

# WebRTC signaling
[web_client.signaling]
path = "/ws"                           # WebSocket signaling endpoint path

# TURN (see §4.3)
[web_client.turn]
enabled = true
# ... (see TURN config above)

# Content Security Policy override
[web_client.csp]
connect_src = ["wss://remote.example.com", "https://remote.example.com"]
```

---

## 14) File Transfer (Web)

File transfer in the web client uses the HTML5 File API:

### 14.1 Upload (Client → Server)

1. User drags files onto the web client canvas, or clicks "Upload" in the toolbar.
2. `FileReader` reads the file into an `ArrayBuffer`.
3. File data is sent to the server via the file transfer channel (reliable data channel).
4. Server places the file in the session's designated upload directory.

### 14.2 Download (Server → Client)

1. Server sends a file transfer offer (filename, size, MIME type).
2. Web client prompts the user to accept.
3. File data is received via the file transfer channel.
4. Web client creates a `Blob` and triggers a download via `<a download>` link.

### 14.3 Limitations

| Limitation | Reason | Mitigation |
|------------|--------|------------|
| No folder upload | `File` API limitation | Use zip or multiple file select |
| Memory-constrained | File buffered in memory | Stream large files in chunks, warn > 100 MB |
| No drag-to-desktop | Browser sandbox | Use download link |
| No background transfer | Tab must stay open | Warn user before closing tab during transfer |

---

## 15) Accessibility

The web client UI follows WCAG 2.1 AA guidelines:

| Feature | Implementation |
|---------|---------------|
| Keyboard navigation | All toolbar controls are focusable and operable via keyboard |
| Screen reader | ARIA labels on all controls, live regions for status updates |
| High contrast | `:forced-colors` support, high-contrast crash screen variant |
| Reduced motion | `prefers-reduced-motion` disables animations |
| Focus indicators | Visible focus rings on all interactive elements |
| Text scaling | Toolbar respects browser font size preferences |

The **remote session** accessibility depends on the server's AT-SPI2 bridge (see spec.md §24). The web client faithfully relays input and displays output; it does not add an accessibility layer over the remote session.

---

## 16) Test Plan

### Transport
- Verify WebRTC data channel establishment with ICE (host, STUN, TURN-UDP, TURN-TCP, TURNS-TLS candidates).
- Verify WebTransport connection and fallback to WebRTC when unavailable.
- Verify WebSocket-only fallback when both WebTransport and WebRTC fail.
- Verify signaling (SDP offer/answer exchange) completes within 3 seconds on LAN.
- Verify ICE restart on network change (Wi-Fi → mobile).

### Protocol
- Verify WASM protocol module decodes all golden capture test vectors (§14 of spec-protocol-formal.md).
- Verify CBOR round-trip between WASM module and server for all message types.
- Verify LZ4 and Zstd decompression in WASM matches native output byte-for-byte.

### Video
- Verify WebCodecs decode path for H.264, VP9, AV1 (on supporting browsers).
- Verify WASM software decoder for H.264 Baseline at 720p30.
- Verify tile XOR delta application produces pixel-identical output to native client.
- Verify frame rendering via WebGPU, WebGL 2, and Canvas 2D paths.

### Audio
- Verify audio playback with Opus decode via AudioWorklet.
- Verify microphone capture with Opus encode.
- Verify jitter buffer handles 50ms jitter without audible glitches.

### Input
- Verify keyboard events (keydown/keyup with correct scancode/keysym mapping).
- Verify Keyboard Lock API captures system keys in fullscreen (Chrome).
- Verify Pointer Lock for relative mouse input.
- Verify touch events on mobile browsers.
- Verify IME composition events for CJK input.

### Clipboard
- Verify text clipboard sync (bidirectional) via Clipboard API.
- Verify image clipboard (PNG) on supporting browsers.
- Verify clipboard permission prompt appears and persists.
- Verify clipboard sync on focus event.

### Security
- Verify CSP headers are present and correctly configured.
- Verify SRI hashes on all static assets.
- Verify no credentials in cookies.
- Verify `sessionStorage` token is cleared on tab close.
- Verify OIDC popup flow completes and code exchange succeeds.

### Performance
- Verify input-to-photon < 40ms p99 on LAN with WebCodecs.
- Verify initial page load < 2s (cached, LAN).
- Verify WASM module compile time < 500ms.
- Verify no memory leaks after 1-hour session (heap snapshot comparison).

### Browser Compatibility
- Run full test suite on all Tier 1 browsers (Chrome, Edge, Firefox latest).
- Run smoke tests on Tier 2 browsers (Safari, Chrome Android, Safari iOS).
- Verify graceful degradation when WebCodecs/WebGPU/WebTransport unavailable.
- Verify web client works behind a corporate HTTP proxy (TURN-TLS fallback).

### Accessibility
- Verify all toolbar controls are keyboard-navigable.
- Verify screen reader announces connection status, errors, and toolbar actions.
- Verify high-contrast mode renders correctly.
- Verify `prefers-reduced-motion` disables animations.
