# LiquidDE Server & Desktop Environment — Full Specification

> **Language**: Rust
> **License**: MIT
> **Related specs**: [Client](spec-client.md) · [Gateway](spec-gateway.md) · [Management UI](spec-manager.md) · [liquidctl CLI](spec-liquidctl.md) · [Design Language](spec-design.md)

---

## 0) Concept

A **remote-first, native desktop environment** built entirely in Rust, designed specifically for remote desktop use-cases:

- **Runs well with *no GPU*** (headless servers, VMs, iGPU-less boxes, cloud instances) but **supports common GPU acceleration** when available.
- **Feels like a modern "liquid glass" OS** (depth, translucency, blur, vibrancy) while remaining bandwidth/CPU efficient.
- **Users customize the DE appearance with CSS** — every visual element is themeable through a well-documented CSS system.
- **Multi-threaded architecture** where the main thread is strictly an orchestrator — all heavy work (rendering, encoding, I/O, effects) runs on dedicated worker threads to prevent hangs.
- **Implements aggressive performance optimizations end-to-end** (render → capture → encode → transport → decode → present).
- Works as a **complete desktop session** (shell + compositor + core apps + dock), not a "screenshare your existing GNOME/KDE" solution.
- **Reimplements the full graphics pipeline** — no dependency on existing compositors or display servers.
- **Multi-platform server**: x86_64 and ARM64 (Linux), with ARM64 support for macOS-hosted VMs and ARM Linux boards.
- Supports **multiple transport strategies** including QUIC, UDP, TCP, TLS, switchable and hybridizable on the fly.

Working name: **LiquidDE** (server) + **LiquidClient** (client, see [spec-client.md](spec-client.md)).

---

## 1) Product Goals

### Primary Goals

1. **Remote experience that feels local**
   - Sub-500ms adaptive fast frame pacing for input-to-photon response.
   - Smooth motion under real networks (Wi-Fi, LTE, congested WAN).
   - High text clarity for terminals/IDEs.

2. **Zero-GPU server requirement with optional GPU acceleration**
   - Compositor and UI run on pure CPU by default.
   - Optional GPU acceleration for encoding/compositing when available (Vulkan, OpenCL, VAAPI, NVENC, AMF).
   - No hard dependency on OpenGL/Vulkan being functional.

3. **Multi-threaded, hang-free architecture**
   - Main thread is strictly an orchestrator/event loop.
   - All rendering, encoding, I/O, effects processing, and transport run on dedicated worker threads.
   - No single operation can block the session.

4. **Dynamic resize and multi-monitor virtualization**
   - Client window resize adjusts the remote virtual monitor(s) without reconnect.
   - On-demand virtual screens per user and session with variable dimensions.
   - Support: single monitor, multi-monitor, fractional scaling.

5. **Full media passthrough**
   - Bidirectional audio (playback + microphone capture).
   - Camera/webcam passthrough.
   - USB device passthrough/redirection.

6. **Clipboard that actually works**
   - Bi-directional text by default.
   - Optional image + file list + rich text.
   - Extensive policy control (disable, whitelist, size limits, audit, direction control).

7. **Secure-by-default**
   - TLS everywhere with multiple encryption options.
   - AES encryption as alternative for low-latency local deployments.
   - Modern auth options.
   - Reasonable hardening and audit logs.

8. **CSS-customizable desktop environment**
   - Every visual element of the DE is themeable through CSS.
   - Comprehensive CSS documentation (see [spec-design.md](spec-design.md)).
   - User-centric configuration — each user has their own DE config.

### Non-Goals (initially)
- Perfect pixel parity with local GNOME/KDE.
- Gaming / high-end 3D rendering (can be added later with extended GPU acceleration).

---

## 2) Target Users & Scenarios

### Personas
- **IT / homelab / MSP**: wants stable remote Linux sessions with predictable behavior.
- **Developers**: terminal/IDE-heavy usage, needs crisp text, low latency, reliable clipboard.
- **Headless servers**: wants a "real desktop session" without GPU.
- **ARM deployments**: Raspberry Pi clusters, ARM cloud instances, Apple Silicon VMs.
- **Enterprise**: centralized management, policy enforcement, gateway deployments behind NAT.

### Scenarios
- Remote session into a VM on a Proxmox/ESXi host.
- Remote session into an Arch box running headless.
- Remote session into a machine that crashes when a GPU/driver path is touched.
- ARM64 Linux board serving desktop sessions.
- Behind-NAT deployment via gateway server.
- Multi-user managed environment with centralized policies.

---

## 3) System Requirements

### Server
- **Linux** (first-class). Must run on:
  - Bare metal, VMs, containers.
  - No GPU (CPU-only rendering) or with optional GPU acceleration.
- **CPU architectures**: x86_64, ARM64 (aarch64).
- **RAM**: 1–2 GB for minimal session; 4 GB recommended.
- **Optional GPU**: Vulkan-capable GPU for hardware-accelerated encoding/compositing.

### Client
- See [spec-client.md](spec-client.md) for full client specification.
- Windows (x86_64, ARM64), Linux (x86_64, ARM64), macOS (ARM64, x86_64).
- Optional web client (WebRTC) for "no install".

---

## 4) Architecture Overview

LiquidDE is built from cleanly separated layers, all in Rust, with a strict multi-threaded design.

### Thread Model

```
Main Thread (Orchestrator)
├── Event Loop — receives all events, dispatches to workers
├── Session Lifecycle — start/stop/resume coordination
└── Thread Health Monitor — detects hung workers, restarts

Worker Thread Pool
├── Render Workers (1–N) — compositing, rasterization, effects
├── Encode Workers (1–N per codec) — frame encoding
├── Transport Workers — packet assembly, send/receive, channel mux
├── Input Worker — keyboard, mouse, touch event processing
├── Audio Worker — bidirectional audio mixing/capture (dedicated channel)
├── Clipboard Worker — clipboard sync (dedicated channel)
├── USB/IP Worker — USB device forwarding (dedicated channel, disabled by default)
├── Media Worker — camera passthrough
├── Policy Engine — evaluates client/server policy rules
├── Logging Worker — async structured log writing
└── Metrics Worker — telemetry collection, stream analysis
```

The main thread **never** performs blocking work. It runs an async event loop (tokio or equivalent) that dispatches work items to worker threads via lock-free channels. If any worker thread hangs or exceeds a deadline, the orchestrator can kill and restart it without crashing the session.

### Layer Architecture

1. **Session Manager**
   - Auth, user session lifecycle, per-user isolation.
   - Multi-session support with per-session virtual screens.

2. **Custom Compositor + Shell (Reimplemented)**
   - Full Wayland-compatible compositor written from scratch in Rust.
   - Shell UI: dock, launcher, notifications, overview, status bar.
   - CSS-driven theming engine.

3. **Custom Graphics Pipeline (Reimplemented)**
   - CPU rasterizer with SIMD acceleration (AVX2/AVX512/NEON).
   - Optional GPU compute path (Vulkan compute shaders).
   - Damage tracking at surface + tile level.
   - Frame graph produces minimal updates.

4. **Encoder Pipeline**
   - 10+ codec support (see §8).
   - Multi-threaded encode with CPU affinity.
   - Adaptive bitrate + latency tuning.

5. **Transport Layer**
   - Multiple simultaneous transport strategies.
   - On-the-fly transport switching and hybridization.
   - MTU-optimized packetization.

6. **Media Subsystem**
   - Bidirectional audio (playback + microphone).
   - Camera/webcam passthrough.
   - USB device redirection.

7. **Policy Engine**
   - Server-side and client-side policy evaluation.
   - Per-user, per-group, per-session granularity.

### High-Level Data Flow

```
Client Input → Transport → Input Worker → Compositor
Compositor → Render Workers → Frame Graph → Damage Tracker
Damage Tracker → Encode Workers → Transport Workers → Client

Dedicated Channels:
  Audio (bidirectional) ←→ Audio Worker ←→ Transport [audio channel]
  Clipboard ←→ Clipboard Worker ←→ Transport [clipboard channel]
  USB/IP ←→ USB/IP Worker ←→ Transport [usb channel]
  Camera ←→ Media Worker ←→ Transport [video channel]
  Cursor ←→ separate cursor channel (out-of-band)
```

---

## 5) Core Design Decision: Reimplemented Graphics

### Why reimplement?
- Existing compositors (wlroots, smithay) are optimized for local display, not remote streaming.
- Full control over the render pipeline enables remote-specific optimizations.
- Damage tracking, tile-based encoding, and frame pacing can be deeply integrated.
- No hidden GPU dependencies or driver requirements.
- CSS theming is a first-class concern, not bolted on.

### Wayland Compatibility
- Implements core Wayland protocols: `wl_compositor`, `wl_shm`, `wl_seat`, `xdg_shell`, `layer_shell`.
- Applications see a standard Wayland compositor.
- XWayland bridge available for legacy X11 apps.

---

## 6) Rendering Stack

### Custom CPU Rasterizer
- **Written entirely in Rust** — no dependency on Skia, Pixman, or Cairo.
- SIMD-accelerated compositing:
  - **x86_64**: SSE4.2, AVX2, AVX-512 (runtime detection).
  - **ARM64**: NEON, SVE where available.
- Rendering primitives:
  - Rectangles, rounded rectangles, circles, paths.
  - Anti-aliased edges.
  - Alpha compositing (Porter-Duff operations).
  - Gradient fills (linear, radial, conic).
  - Text rendering (FreeType + HarfBuzz).
  - Image blitting with bilinear/bicubic filtering.

### Optional GPU Acceleration
- **Vulkan compute** path for compositing and blur when GPU available.
- **Hardware encoder** support:
  - VAAPI (Intel, AMD).
  - NVENC (NVIDIA).
  - AMF (AMD).
  - V4L2 M2M (ARM SoCs).
  - VideoToolbox (macOS ARM64 — client-side decode).
- GPU is **never required** — CPU path is always available and complete.

### Font Stack
- **FreeType** for rasterization.
- **HarfBuzz** for shaping.
- **Fontconfig** for font discovery.
- Subpixel rendering configurable (may be disabled for remote to avoid codec artifacts).
- Font hinting modes: none, slight, medium, full.

### Client-Assisted Font Rendering
- The server can **offload font rendering to the client** whenever the client supports it.
- Instead of rasterizing text into pixels on the server, the server sends:
  - Font data (or font references if the client has the font locally).
  - Glyph IDs, positions, sizes, colors, and shaping results.
  - Layout metadata (line breaks, text direction, decorations).
- The client rasterizes text locally using its own GPU or CPU, producing **pixel-perfect, sharp text** at the client's native DPI with no codec artifacts.
- Benefits:
  - **Eliminates text compression artifacts** — text is never encoded through a lossy video codec.
  - **Reduces bandwidth** — glyph data is far smaller than pixel data for text-heavy content.
  - **Perfect subpixel rendering** — client renders with its native subpixel layout.
  - **DPI independence** — text re-renders cleanly at any scale without server involvement.
- Font offload modes:
  - `auto` (default) — offload when client supports it and bandwidth savings are significant.
  - `always` — always offload text rendering to client.
  - `never` — always server-render text (traditional mode).
  - `hybrid` — offload static/UI text, server-render dynamic/application text.
- Font synchronization:
  - Server sends a font manifest at session start listing required fonts.
  - Client reports which fonts it has locally.
  - Missing fonts are transferred once and cached on the client.
  - Font cache is persistent across sessions (configurable max size).
- Configurable in `server.toml`:
  ```toml
  [offload]
  font_rendering = "auto"         # auto, always, never, hybrid
  font_cache_max_mb = 200         # max font cache size on client
  font_sync_on_connect = true     # sync fonts at connection time
  ```

### "Liquid Glass" Rendering Without Melting the CPU

#### 1. Blur Caching
- Glass surfaces maintain a **cached blurred backdrop texture**.
- Recompute blur **only** when the background behind that surface actually changes (tracked via damage regions).
- Cache is invalidated per-surface, not globally.
- **User-configurable**: can be set to `auto`, `always-cache`, or `always-recompute`.

#### 2. Downsampled Blur
- Blur computed at **1/4, 1/8, or 1/16 resolution**, then upsampled.
- Downsample ratio is:
  - **Auto**: system chooses based on CPU budget and blur radius.
  - **Override**: user or admin can set a fixed downsample level.
- Optional multi-pass separable Gaussian blur.
- Box blur fast path for bandwidth-saver profiles.

#### 3. CPU Effect Budgets
- Each frame has a **CPU time budget** (in milliseconds).
- On session start, a **benchmark** runs to calibrate the budget to the hardware:
  - Measures single-core and multi-core blur throughput.
  - Measures compositing throughput.
  - Measures encode throughput.
- Budget modes:
  - **Auto**: system-determined from benchmark results.
  - **Defined**: administrator sets a fixed budget (e.g., `effect_budget_ms = 8`).
  - **Specific setting**: choose a named profile (`minimal`, `balanced`, `quality`).
- When over budget:
  - Reduce blur radius progressively.
  - Reduce shadow samples.
  - Skip non-essential highlights and specular effects.
  - Fall back to solid-color tinted panels instead of glass.

#### 4. Background / Wallpaper Caching
- Wallpapers are rendered once and cached as a pre-composited texture.
- Wallpaper blur (for glass over desktop) is pre-computed at session start and cached.
- **Wallpaper can be disabled entirely** for maximum performance (solid color or gradient fallback).
- When wallpaper is disabled, glass surfaces use a configurable solid tint.
- Wallpaper changes invalidate the cache and trigger a one-time recompute.

#### 5. Client-Side Wallpaper Caching
- The server can instruct the client to **cache wallpaper assets locally**.
- On connection, the server sends a wallpaper manifest (hash, dimensions, last modified).
- If the client already has the wallpaper cached, no transfer is needed — the client confirms the hash.
- If the wallpaper has changed, the server sends the new wallpaper once; the client caches it persistently.
- The client composites the cached wallpaper locally behind glass surfaces, reducing server-side render cost and bandwidth.
- Fully configurable behavior:
  ```toml
  [performance]
  wallpaper_client_cache = "auto"     # auto, always, never
  wallpaper_client_cache_max_mb = 500 # max local wallpaper cache size
  wallpaper_client_composite = true   # client composites wallpaper behind glass
  wallpaper_client_blur = true        # client computes wallpaper blur locally
  wallpaper_cache_ttl_days = 30       # expire cached wallpapers after N days
  wallpaper_preload_on_connect = true # send wallpaper during connection handshake
  ```
- Cache modes:
  - `auto` (default) — cache on client when bandwidth savings are significant (wallpaper size > threshold).
  - `always` — always cache on client regardless of size.
  - `never` — server always renders wallpaper, no client caching.
- The client wallpaper cache persists across sessions and across server reconnections.
- Cache eviction: LRU with configurable max size. Expired entries purged automatically.

#### 6. Partial Caches for Static Regions
- **Status bars**, dock backgrounds, and other rarely-changing regions maintain cached rasterizations.
- Cache hit: blit from cache (nearly free).
- Cache invalidation: only on content change (clock tick, notification badge, etc.).
- **Fast bounce from idle**: when the session goes idle, caches are preserved in memory. On wake (input event), the first frame is assembled from caches with near-zero render cost, then incremental updates resume.
- **Configurable**: each auto-caching behavior can be:
  - `enabled` (default) — system manages cache lifecycle.
  - `disabled` — always re-render (useful for debugging or specific use cases).
  - `level:<N>` — set cache aggressiveness (1 = minimal caching, 5 = aggressive caching).

#### 7. Animation Policy
- Default animations are **event-driven** (input/transition) rather than constant.
- Frame rate caps for UI-only animation (e.g., 30 fps) while cursor/input stays responsive.
- Idle state: 1–2 fps or pure "only-on-change" mode.
- All animation durations and curves configurable via CSS.

---

## 7) Remote Display Model

### Virtual Monitors
- Each session owns one or more **virtual monitors**.
- **On-demand creation**: virtual screens are created per user and session as needed.
- **Variable dimensions**: each virtual screen has independently configurable resolution, DPI, and refresh rate.
- Client can:
  - Add/remove monitors at any time.
  - Resize monitors dynamically.
  - Set DPI scale per monitor.
  - Arrange monitors spatially (left-of, right-of, above, below).

### Resize Behavior
- When client window resizes, client sends a **Display Update** message.
- Server updates the virtual monitor(s), re-layouts UI, and continues without reconnect.
- For clients lacking true dynamic-res support:
  - Fall back to **smart scaling** (server fixed resolution, client scales) with optional "fit to window".

### Multi-Monitor Mapping
- Supports:
  - **"Match local monitors"** (1:1 mapping).
  - **"Single large canvas"** (panorama mode).
  - **"Tabbed monitors"** (fast switch between virtual screens in a single client window).
  - **"Multi-window"** (each virtual monitor in a separate client window, spanning across different physical client screens).
- Mode switchable at runtime without reconnection.

### Multiple Screens Per Session
- A single session can have N virtual screens.
- Each virtual screen has its own:
  - Resolution and DPI.
  - Damage tracker and encode pipeline.
  - Transport stream (can be multiplexed or separate).

---

## 8) Transport & Codec Strategy

### Supported Encoders (10+)

| Encoder | Type | Notes |
|---------|------|-------|
| **H.264 / AVC** | Video | Fast, broadest compatibility, default for interactive |
| **H.265 / HEVC** | Video | Better compression, higher CPU cost |
| **AV1** | Video | Best compression, CPU-heavy (SVT-AV1 for speed) |
| **VP8** | Video | WebRTC compatibility |
| **VP9** | Video | Good compression, moderate CPU |
| **MJPEG** | Video | Per-frame, no temporal dependency, lowest latency |
| **Zstd tiles** | Tile | Lossless compressed bitmap tiles |
| **LZ4 tiles** | Tile | Ultra-fast lossless tile compression |
| **PNG tiles** | Tile | Lossless, best for text-heavy content |
| **QOI tiles** | Tile | Fast lossless image format |
| **WebP tiles** | Tile | Lossy/lossless hybrid tiles |
| **Raw bitmap** | Tile | Uncompressed, for LAN/localhost |

- Hardware-accelerated variants (VAAPI, NVENC, AMF, V4L2 M2M) used automatically when available.
- Encoders are selectable per-session, per-monitor, or auto-negotiated.

### Encoding Modes

#### Mode A — Video Stream (general UI, video playback)
- Encode dirty regions as video frames.
- **Separate cursor channel** (never encoded into video stream).
- **Damage-aware encoding**: only encode dirty regions via ROI or region refresh.
- **Adaptive frame pacing** (sub-500ms response target):
  - Idle: 1–5 fps (or "only-on-change").
  - Interaction: ramp to 30–60 fps within one frame period.
  - Configurable min/max FPS, ramp speed.
- **GOP tuning** for ultra-low latency (all-intra or very short GOPs).
- **FPS limiter**: configurable per session, per policy, or auto-adaptive.

#### Mode B — Tile / Bitmap Stream (crisp text, static content)
- Screen partitioned into tiles (configurable: 32×32 to 256×256).
- Per tile:
  - Hash-based change detection.
  - Compress changed tiles with selected tile codec.
- Best for: terminals, code editors, dashboards, documents.

#### Hybrid Mode
- The system **automatically hybridizes**: video for large moving regions, tiles for text regions.
- Content-type heuristics detect:
  - Text / UI → tile mode bias.
  - Video playback / animation → video mode bias.
  - Mixed → hybrid with per-region decisions.
- Hybridization ratio configurable or fully auto.

### Frame Buffer Caps
- Maximum frame buffer memory configurable per session.
- Frame buffer dimensions capped by policy.
- FPS hard cap configurable (server or client policy).

### Encryption

| Scheme | Use Case | Notes |
|--------|----------|-------|
| **TLS 1.3** | Default for all connections | ChaCha20-Poly1305 or AES-256-GCM |
| **AES-128-GCM** | Local/LAN deployments | Lower overhead alternative |
| **AES-256-GCM** | High-security deployments | Maximum encryption strength |
| **ChaCha20-Poly1305** | ARM64 / no AES-NI | Faster on platforms without AES hardware |
| **None (plaintext)** | Localhost only | Must be explicitly enabled, policy-guarded |

- Encryption is **per-transport-stream**, allowing different encryption for control vs. media channels.
- Encryption scheme negotiated at connection or set by policy.

### Transport Strategies

#### Available Transports

| Transport | Description | Default Priority |
|-----------|-------------|-----------------|
| **QUIC (over UDP)** | Preferred. Multiplexed streams, built-in encryption, congestion control. | 1 (highest) |
| **Pure UDP** | Lowest latency, no ordering guarantees. Good for LAN. Requires app-level reliability. | 2 |
| **TLS over TCP** | Reliable, high compatibility. Higher latency than QUIC. | 3 |
| **Pure TCP** | Fallback. No encryption (use with external tunnel). | 4 |
| **WebSocket (TLS)** | For web clients and restrictive firewalls. | 5 |
| **WebRTC** | For browser-based clients. | 6 |

#### Transport Negotiation
- **Auto-negotiation** (default): client and server probe available transports and select the best one based on network conditions.
- **Priority list**: administrator or user defines an ordered preference list.
- **Specific override**: force a single transport (e.g., `transport = "quic"`).
- **On-the-fly switching**: transport can change mid-session based on network condition changes (e.g., QUIC fails, fall back to TLS/TCP without disconnecting).
- **Hybrid transport**: different data channels can use different transports simultaneously:
  - Control channel on TLS/TCP (reliable).
  - Video stream on QUIC/UDP (low latency).
  - Audio on UDP (minimal latency).
  - File transfer on TCP (reliable).

#### MTU-Optimized Packetization
- Automatic MTU discovery (PMTUD) on connection.
- Packets sized to avoid IP fragmentation.
- For video: NAL units split to fit within MTU.
- For tiles: tiles grouped or split to maximize MTU utilization.
- Configurable MTU override for known network topologies.

#### Congestion Control
- QUIC: uses built-in congestion control (BBR or Cubic, configurable).
- UDP: custom congestion control with:
  - RTT estimation.
  - Packet loss detection.
  - Bandwidth probing.
  - Interactive-traffic-optimized pacing.

### Multiple Listening Modes
- Server can listen on **multiple addresses and ports simultaneously**.
- Different listeners can use different transports:
  ```toml
  [[listen]]
  address = "0.0.0.0:3389"
  transport = "quic"

  [[listen]]
  address = "0.0.0.0:3390"
  transport = "tls-tcp"

  [[listen]]
  address = "[::]:3389"
  transport = "quic"
  ```
- Supports **reverse connection** mode for gateway-brokered sessions (server connects out to gateway instead of listening).

---

## 9) Performance Optimizations

### Idle Loop & CPU Efficiency
- When idle (no input, no screen changes):
  - Render thread **parks** (zero CPU).
  - Encode thread parks.
  - Transport sends nothing (or periodic keepalives only).
  - Event loop uses `epoll`/`io_uring` — no busy-waiting, no polling.
- **Fast wake**: on any input event, the pipeline resumes within 1ms (pre-warmed caches, pre-allocated buffers).
- All worker threads use **work-stealing schedulers** — idle threads are truly idle, not spinning.

### Capture / Render
- Damage tracking at surface + tile level.
- Occlusion culling: don't composite fully covered surfaces.
- Partial present: update only changed tiles.
- Cursor out-of-band (separate channel, never re-encodes the frame).
- Double/triple buffering with frame pacing.
- **Background/wallpaper caching** — rendered once, cached indefinitely until changed.
- **Partial region caching** — status bars, dock, and static panels cached.
- **Fast bounce from idle states** — first frame after idle assembled from caches.

### Encode
- Multi-threaded encode with CPU affinity.
- Adaptive bitrate (ABR) based on:
  - Packet loss.
  - RTT.
  - Decode queue depth.
  - Input activity.
- Encoder presets:
  - **"Interactive"** (lowest latency).
  - **"Balanced"**.
  - **"Bandwidth saver"**.
  - **"LAN"** (maximum quality, minimal compression).

### Transport
- MTU-optimized packetization (see §8).
- Forward error correction (FEC) — optional, configurable redundancy level.
- Congestion control tuned for interactive traffic.
- Prioritized packet scheduling (input > cursor > video > audio > file transfer).

### Client-Side Rendering Offload
- Server can optionally offload certain rendering to the client:
  - **Cursor rendering**: client draws cursor locally (already default).
  - **Window chrome**: server sends window geometry + theme data, client renders chrome locally.
  - **Simple animations**: server sends animation parameters, client interpolates.
  - **Text rendering**: server sends glyph data + positions, client rasterizes (see §6 Client-Assisted Font Rendering).
  - **Full offload**: all UI chrome and text rendered client-side, only application content streamed.
- Offload level configurable: `none`, `cursor-only` (default), `chrome`, `text`, `full`.
- Reduces bandwidth and improves perceived latency for UI elements.

### Benchmarks on Start
- On session start (or on-demand via `liquidctl benchmark`):
  - **CPU compositing throughput** — measures SIMD-accelerated blend rates.
  - **Blur throughput** — measures Gaussian blur at various radii and resolutions.
  - **Encode throughput** — measures each available encoder's speed.
  - **Memory bandwidth** — measures buffer copy rates.
- Results stored and used to auto-configure:
  - Effect budgets.
  - Default encoder selection.
  - Blur downsample ratios.
  - Animation complexity.

---

## 10) Bidirectional Audio & Media

### Audio
- **Playback** (server → client): PulseAudio/PipeWire virtual sink captures server audio.
- **Microphone** (client → server): client captures mic, sends to virtual source on server.
- **Audio can be entirely disabled** for lightweight mode — no audio threads, no audio processing, no bandwidth usage.

#### Audio Codecs
| Codec | Type | Notes |
|-------|------|-------|
| **Opus** | Lossy | Default. Best quality/latency balance for voice and music. 6–510 kbps. |
| **AAC** (LC, HE, HEv2) | Lossy | Wide compatibility. Good for music at higher bitrates. |
| **MP3** (LAME) | Lossy | Legacy compatibility. Decode-only on server, full on client. |
| **Vorbis** (OGG) | Lossy | Open, good quality. Alternative to AAC. |
| **FLAC** | Lossless | LAN mode or high-fidelity requirements. |
| **ALAC** | Lossless | Apple ecosystem compatibility. |
| **PCM** (raw) | Uncompressed | LAN/localhost mode. Zero latency, high bandwidth. |
| **G.711 (μ-law/A-law)** | Lossy | Narrowband voice. Ultra-low CPU. |
| **G.722** | Lossy | Wideband voice. Low CPU, good for voice-only. |
| **Speex** | Lossy | Legacy voice codec. Low CPU. |
| **WMA** | Lossy | Windows ecosystem compatibility. |

- Codec selection per direction (playback/microphone can use different codecs).
- Codec auto-negotiation based on content type:
  - Voice detected → low-bitrate voice codec (Opus voice mode, G.722).
  - Music/media detected → high-quality codec (Opus music mode, AAC, FLAC).
  - Silence detected → codec pauses, zero bandwidth.

#### Audio Configuration
- Configurable:
  - Sample rate (8kHz – 48kHz).
  - Channels (mono, stereo, 5.1, 7.1).
  - Bitrate (per direction).
  - Buffer size / latency target.
  - Codec preference order.
- Audio transport uses a **dedicated channel** (separate from video) for independent QoS.
- Mute/volume controls exposed to both client and server policies.
- **Disable audio entirely**:
  ```toml
  [audio]
  enabled = false                    # disables all audio subsystem
  ```
  When disabled, no audio threads are started, no virtual sinks are created, and no audio bandwidth is consumed.

### Camera / Webcam Passthrough
- Client webcam forwarded to a virtual V4L2 device on the server.
- Server applications see a standard camera.
- Encoding: MJPEG or H.264 for camera stream (negotiated).
- Resolution and FPS negotiated between client capability and server policy.
- Privacy: camera passthrough requires explicit client approval per session.
- **Fully configurable on both sides**:
  - Server policy controls whether camera passthrough is allowed, per user/group.
  - Client controls which camera device to share and when.
  - Either side can disable at any time during the session.
  - Configuration:
    ```toml
    # Server-side (server.toml)
    [camera]
    passthrough_enabled = false           # server allows camera passthrough
    max_resolution = "1920x1080"
    max_fps = 30
    allowed_codecs = ["mjpeg", "h264"]
    default_codec = "mjpeg"
    require_client_approval = true        # client must explicitly approve

    # Client-side (config.toml)
    [camera]
    passthrough_enabled = false           # client allows camera passthrough
    device = "auto"
    resolution = "auto"
    fps = 30
    auto_enable_on_connect = false        # never auto-enable, always ask
    preview_before_share = true           # show preview before sharing
    ```

### USB Device Redirection (USB/IP)
- USB devices on the client can be forwarded to the server via a **USB/IP-based protocol**.
- **Disabled by default** — must be explicitly enabled in server configuration.
- **Runs as its own dedicated thread** — USB/IP processing is fully isolated from the render/encode pipeline:
  - Dedicated USB Worker thread handles device enumeration, attach/detach, data transfer.
  - Uses a **dedicated transport channel** (separate from video, audio, clipboard).
  - Thread can be disabled entirely when USB redirection is off (zero overhead).
- Supports:
  - Storage devices (USB drives, SD cards).
  - Printers.
  - Smart card readers.
  - Security keys (FIDO2/U2F).
  - Generic USB devices (via USB/IP protocol).
  - Composite devices (multi-function).
- USB/IP implementation:
  - Compatible with Linux kernel `usbip` protocol for device import/export.
  - Client runs USB/IP device server, server runs USB/IP client (VHCI).
  - Encrypted channel — USB data tunneled through the session's transport encryption.
  - Latency-sensitive devices (smart cards, security keys) get priority scheduling.
- Policy-controlled:
  - Whitelist/blacklist by VID/PID.
  - Whitelist/blacklist by device class.
  - Per-user permissions.
  - Per-group permissions.
  - Audit logging of device attach/detach and data transfer statistics.
- Configuration:
  ```toml
  [usb]
  enabled = false                      # disabled by default
  transport_channel = "dedicated"      # dedicated, shared
  allowed_device_classes = ["mass-storage", "printer", "smartcard", "security-key"]
  allowed_vid_pid = []                 # empty = allow all (when enabled)
  blocked_vid_pid = []
  max_devices_per_session = 5
  max_bandwidth_mbps = 50              # per-session USB bandwidth cap
  audit_log = true
  ```

---

## 11) Clipboard & Data Channels

### Dedicated Transport Channels
LiquidDE uses **separate, dedicated transport channels** for different data types. Each channel operates independently with its own QoS, priority, and flow control:

| Channel | Priority | Reliability | Notes |
|---------|----------|-------------|-------|
| **Control** | Highest | Reliable | Session control, auth, keepalives |
| **Input** | Highest | Reliable | Keyboard, mouse, touch events |
| **Cursor** | High | Unreliable (latest-wins) | Cursor position, shape updates |
| **Video** | Medium | Semi-reliable | Encoded frames, tiles |
| **Clipboard** | Medium | Reliable | Clipboard sync data |
| **Audio** | Medium | Unreliable (jitter-buffered) | Playback and microphone streams |
| **USB** | Low-Medium | Reliable | USB/IP device data |
| **File Transfer** | Low | Reliable | File uploads/downloads |

- Each channel can use a different transport if hybrid transport is enabled.
- Channels can be independently enabled/disabled without affecting others.
- Bandwidth allocation is configurable per channel with priority-based scheduling.
- Channel multiplexing: all channels can share a single transport connection (QUIC streams) or use separate connections.

### Clipboard Channel

#### Clipboard Types
- **Text** (UTF-8) — default, always enabled.
- **Rich text** (HTML/RTF) — optional.
- **Images** — optional (size limit configurable).
- **File list** — optional (maps to file transfer channel).

#### Clipboard Policy Engine
Configurable per session/user/group with extensive options:

- **Direction control**:
  - `client → server` enable/disable.
  - `server → client` enable/disable.
  - Bidirectional enable/disable.
- **Size limits**: max bytes per clipboard item, per transfer.
- **MIME type filtering**: whitelist or blacklist specific types.
- **Rate limiting**: max clipboard operations per minute.
- **Content inspection**: optional regex-based blocking (e.g., block SSN patterns).
- **Audit logging**: metadata-only by default, optional content hashing.
- **Delay / confirmation**: optional user confirmation for large clipboard transfers.

### File Transfer Channel
- Optional, policy-controlled.
- Two modes:
  1. **"Drag & drop into session"** (client uploads to server).
  2. **"Browse server files"** (read-only or read-write).
- Size limits, rate limits, and allowed file type filters all configurable.

---

## 12) Input System

### Keyboard
- **Layout-aware scancodes**: server-side layout mapping.
- **Extensive keyboard layout support**: 50+ layouts selectable per session.
  - QWERTY (US, UK, etc.), AZERTY, QWERTZ, Dvorak, Colemak, etc.
  - CJK input method support (IME).
  - Custom layout definition files.
- **Keystroke capture and forward**: client captures all keystrokes (including system shortcuts like Alt+Tab, Super, Ctrl+Alt+Del) and forwards them to the remote session when the client window has focus.
- **No 'sticky modifiers' under latency**: modifier key state tracked precisely.
- **Dead keys and compose sequences** supported.

### Mouse
- Relative and absolute mode.
- High-precision scroll (smooth scrolling).
- Button forward: all buttons including back/forward.
- **Cursor fluidity** — see [spec-client.md](spec-client.md) §7 for cursor settings, including the dual cursor mode (local dot + server-authoritative position).

### Touch & Tablet Support
- **Full touchscreen input forwarding**: touch events from the client are forwarded to the server as native touch events.
- Gestures mapped to shell actions:
  - Swipe up from bottom — open app launcher.
  - Swipe down from top — notification shade.
  - Three-finger swipe — workspace switch.
  - Pinch-to-zoom — mapped to Ctrl+scroll or native zoom (configurable).
  - Long press — right-click.
  - Two-finger tap — right-click (alternative).
- **Pen/stylus support**: pressure, tilt, and button events forwarded for drawing applications.
- Multi-touch events forwarded with full touch point tracking (up to 10 simultaneous points).

### Tablet Mode (Server-Side)
- **Disabled by default** — must be explicitly enabled.
- When enabled, the DE adapts its layout for touch-friendly operation:
  - Larger hit targets (minimum 56×56px, up from 44×44px).
  - On-screen keyboard auto-shows when text input is focused.
  - Dock switches to a tablet-friendly bottom bar with larger icons.
  - Window management simplifies: windows default to maximized, swipe gestures for switching.
  - Status bar becomes taller (40px) with larger touch areas.
  - App launcher uses a grid layout with larger icons.
  - Notification shade becomes swipe-accessible.
- Tablet mode can be toggled at runtime without session restart.
- Tablet mode CSS classes applied to root: `.liquid-tablet-mode`.
- Configuration:
  ```toml
  [tablet_mode]
  enabled = false                      # disabled by default
  auto_detect = false                  # auto-enable when touch-only client connects
  on_screen_keyboard = true            # show on-screen keyboard in tablet mode
  gesture_navigation = true            # enable swipe gestures
  larger_hit_targets = true            # increase minimum touch target size
  default_maximized = true             # new windows open maximized
  dock_style = "tablet"                # tablet, desktop (use desktop dock in tablet mode)
  ```

---

## 13) Session Management & Isolation

### Session Manager Responsibilities
- Auth handshake.
- Start/stop/resume sessions.
- Allocate virtual monitors (on-demand, variable dimensions).
- Assign policies (clipboard, file transfer, media, USB).
- Manage per-user DE configuration.

### Isolation Model
Each user session runs in one of:
- `systemd --user` session (default).
- User namespace sandbox.
- Optional container (bubblewrap or similar).

### Persistence
- "Resume last session" optional.
- Idle timeout + disconnect behavior:
  - Lock session.
  - Keep running in background.
  - Terminate after configurable timeout.
- Session state survives client disconnect (reconnect continues where left off).

### User-Centric DE Configuration
- Each user has their own DE configuration directory: `~/.config/liquidde/`.
- Contains:
  - `theme.css` — user's CSS customizations.
  - `session.toml` — DE preferences (dock position, wallpaper, layout).
  - `keybindings.toml` — custom keyboard shortcuts.
  - `keyboard-layout.toml` — preferred keyboard layout and variants.
- Defaults inherited from system config, user overrides take priority.
- Config changes apply live (no session restart needed for most settings).

---

## 14) Desktop Environment: Shell & Dock

### Dock (Bottom by Default, macOS-style)
- **Position**: bottom (default), left, right, or top. Configurable per user.
- **Fully configurable**:
  - Icon size (adjustable, with magnification on hover).
  - Auto-hide with configurable delay and animation.
  - Show/hide running indicators.
  - Pinned apps, running apps, and trash/minimized section.
  - Separator/spacer items.
  - Badge notifications on dock icons.
- **Glass aesthetic**: translucent background with blur, matching the liquid glass theme.
- **Multi-monitor aware**: dock can appear on primary screen only, all screens, or follow focus.
- **CSS-themeable**: dock appearance fully controlled by CSS class `.liquid-dock`.

### Top Bar / Status Bar
- Optional (can be replaced by dock-integrated status area).
- System tray, clock, notification indicators, connection quality badge.
- Cached rendering — only redraws on content change.
- CSS class: `.liquid-status-bar`.

### App Launcher
- Activated by dock icon, keyboard shortcut, or hot corner.
- Search-as-you-type for applications.
- Categories and recent apps.
- Glass panel with blur backdrop.

### Window Management
- Tiling and floating hybrid (configurable default).
- Snap to edges/corners.
- Alt+Tab window switcher optimized for remote (shows live thumbnails if bandwidth allows, else icons).
- Window animations governed by CSS transitions and effect budget.

### Extensive Window Tiling
LiquidDE includes a full-featured tiling window manager that coexists with the floating mode:

#### Tiling Layouts
- **Split horizontal** (side-by-side) — default for 2-window tiling.
- **Split vertical** (top/bottom).
- **Quadrant** — 4 equal quadrants, snap windows to any corner.
- **Three-column** — center master + side columns.
- **Spiral/Fibonacci** — each new window takes half the remaining space.
- **Stacking** — tabbed windows within a tile region.
- **Custom grid** — user-defined row/column layouts with configurable ratios.

#### Tiling Behavior
- **Mode switching**: toggle between tiling and floating per workspace or globally.
  - `floating` (default) — traditional free-form window management.
  - `tiling` — all new windows auto-tile into the active layout.
  - `hybrid` — manual tiling (snap to tile zones) with floating as default.
- **Snap zones**: when dragging a window, semi-transparent zone previews appear at screen edges and corners.
- **Keyboard-driven tiling**:
  - `Super + Arrow` — tile to half/quarter.
  - `Super + Enter` — toggle focused window fullscreen within tile.
  - `Super + Shift + Arrow` — swap tiled window positions.
  - `Super + [1-9]` — switch tile layout preset.
  - All shortcuts configurable in `keybindings.toml`.
- **Resize handles**: drag tile borders to resize adjacent tiles proportionally.
- **Gap/margin between tiles**: configurable `--liquid-tile-gap` (default: 8px), set to 0 for gapless.
- **Per-workspace layouts**: each virtual workspace can have a different tiling layout.
- **Window rules**: per-application rules for tiling behavior:
  ```toml
  [[window_rules]]
  app_id = "firefox"
  tile_behavior = "tiling"          # tiling, floating, force-floating
  default_tile_zone = "master"

  [[window_rules]]
  app_id = "dialog-*"
  tile_behavior = "force-floating"  # dialogs always float
  ```
- **Saved layouts**: save and restore complete tiling arrangements with named presets.

#### Tiling Configuration
```toml
[tiling]
enabled = true
default_mode = "hybrid"             # floating, tiling, hybrid
default_layout = "split-horizontal" # split-horizontal, split-vertical, quadrant, three-column, spiral, stacking, custom
gap = 8                             # pixels between tiles
outer_gap = 8                       # pixels between tiles and screen edges
snap_threshold = 32                 # pixels from edge to trigger snap preview
animate_tile = true                 # animate window tile transitions
master_ratio = 0.55                 # master window width ratio (for three-column, spiral)
respect_min_size = true             # respect window minimum size hints
tiling_indicator = true             # show visual indicator when tiling mode is active

[tiling.custom_grid]
rows = 2
columns = 3
ratios_cols = [0.25, 0.50, 0.25]
ratios_rows = [0.5, 0.5]
```

#### Tiling CSS Classes
- `.liquid-window.tiled` — window in tiled mode.
- `.liquid-window.tiled.master` — master tile window.
- `.liquid-tile-preview` — snap zone preview overlay.
- `.liquid-tile-indicator` — tiling mode indicator.
- Full CSS reference in [spec-design.md](spec-design.md).

### Notifications
- "Stacking" to avoid animation storms.
- Do-not-disturb mode.
- Notification history panel.

---

## 15) Security

### Transport Security
- TLS 1.3 (default).
- AES-256-GCM or ChaCha20-Poly1305.
- **AES-128-GCM** available for local/LAN deployments where lower overhead is preferred.
- Server certificate management:
  - Self-signed bootstrap with fingerprint verification.
  - ACME/Let's Encrypt (optional).
  - Enterprise PKI import.

### Authentication
- Options:
  - Local accounts.
  - PAM.
  - LDAP/AD via PAM.
  - OIDC (optional).
  - Multi-factor authentication (see below).
  - Certificate-based authentication (see below).

#### Multi-Factor Authentication (MFA)
- **TOTP** (Time-based One-Time Password) — Google Authenticator, Authy, etc.
- **Hardware security tokens** — FIDO2/U2F (YubiKey, SoloKey, etc.).
  - WebAuthn protocol for token registration and challenge/response.
  - Multiple tokens can be registered per user (backup tokens).
- **Smart card authentication** — PIV/CAC smart cards.
  - PKCS#11 interface for smart card access.
  - Certificate-based identity verification.
  - Supported reader types forwarded via USB/IP when needed.
- **Platform biometrics** (client-side):
  - **Windows**: Windows Hello (fingerprint, face recognition, PIN).
  - **macOS**: Touch ID.
  - **Linux**: fprintd (fingerprint), polkit integration.
  - Biometric verification happens on the client; an attestation token is sent to the server.
  - Server never receives raw biometric data.
- MFA configuration:
  ```toml
  [auth]
  mfa_enabled = true
  mfa_required = false                 # true = MFA mandatory for all users
  mfa_methods = ["totp", "fido2", "smartcard", "biometric"]
  mfa_grace_period_sec = 0             # seconds before MFA is required after password
  mfa_remember_device_days = 30        # 0 = always require MFA

  [auth.fido2]
  relying_party_id = "liquidde.example.com"
  attestation = "none"                 # none, indirect, direct

  [auth.smartcard]
  pkcs11_module = "/usr/lib/opensc-pkcs11.so"
  ca_certificates = ["/etc/liquidde/smartcard-ca.pem"]
  require_pin = true
  ```

#### Certificate-Based Authentication
- Mutual TLS (mTLS) for client authentication.
- Client presents a certificate signed by a trusted CA.
- Certificate fields mapped to user identity:
  - Common Name (CN) → username.
  - Subject Alternative Names (SAN) → additional identity attributes.
  - Organization (O) → group membership.
- Certificate revocation checking: CRL or OCSP.
- Configuration:
  ```toml
  [auth.certificate]
  enabled = false
  client_ca_file = "/etc/liquidde/client-ca.pem"
  crl_file = "/etc/liquidde/crl.pem"
  ocsp_enabled = false
  ocsp_responder_url = ""
  username_field = "CN"                # CN, SAN:email, SAN:upn
  ```

### Authorization & Policy Engine

#### Server Policies
Server-wide policies set by administrators:
- Max concurrent sessions.
- Allowed client platforms.
- Allowed transports.
- Allowed encoders.
- Clipboard policy defaults.
- USB redirection policy.
- Audio/video passthrough policy.
- Max resolution and FPS caps.
- Connection rate limiting.
- Session duration limits.

#### Client Policies
Policies enforced on the client side:
- Certificate pinning requirements.
- Minimum encryption level.
- Local clipboard integration permissions.
- Screenshot/recording restrictions.
- Keyboard capture scope.

#### Per-User Policies
- Override server defaults per user or group:
  - Clipboard allowed?
  - File transfer allowed?
  - Max sessions.
  - Allowed features.
  - Performance profile.

### Audit Logging
- Log:
  - Login success/failure with source IP.
  - Session start/stop with duration.
  - Policy changes.
  - Clipboard/file transfer events (metadata).
  - USB device attach/detach.
  - Administrative actions.

### Intrusion Prevention (fail2ban Integration)
- LiquidDE integrates with **fail2ban** for automated intrusion prevention.
- Server emits structured authentication events to a log file or syslog that fail2ban can monitor.
- **Built-in fail2ban jails** ship with the server:
  - `liquidde-auth` — ban IPs after repeated authentication failures.
  - `liquidde-brute` — ban IPs attempting rapid connection attempts.
  - `liquidde-proto` — ban IPs sending malformed protocol messages.
- Jail configuration (shipped as `/etc/fail2ban/jail.d/liquidde.conf`):
  ```ini
  [liquidde-auth]
  enabled = true
  filter = liquidde-auth
  logpath = /var/log/liquidde/auth.log
  maxretry = 5
  findtime = 600
  bantime = 3600
  action = iptables-multiport[name=liquidde, port="3389,3390"]

  [liquidde-brute]
  enabled = true
  filter = liquidde-brute
  logpath = /var/log/liquidde/auth.log
  maxretry = 20
  findtime = 60
  bantime = 86400

  [liquidde-proto]
  enabled = true
  filter = liquidde-proto
  logpath = /var/log/liquidde/server.log
  maxretry = 3
  findtime = 60
  bantime = 86400
  ```
- Built-in rate limiting (independent of fail2ban):
  - Configurable max login attempts per IP per time window.
  - Progressive delay after failed attempts (exponential backoff).
  - IP lockout duration configurable.
- Server configuration:
  ```toml
  [security]
  fail2ban_log = "/var/log/liquidde/auth.log"
  fail2ban_log_format = "syslog"       # syslog, json
  rate_limit_enabled = true
  rate_limit_max_attempts = 5
  rate_limit_window_sec = 300
  rate_limit_lockout_sec = 600
  progressive_delay = true
  ```

### Session Jails (Sandboxing)
- Each user session can be **jailed** in an isolated environment for security hardening.
- Jail types:
  - `none` — no additional isolation (trusts OS user separation).
  - `namespace` — Linux user/mount/PID/network namespaces for lightweight isolation.
  - `seccomp` — syscall filtering to restrict session capabilities.
  - `container` — full container isolation (bubblewrap, systemd-nspawn).
  - `combined` — namespace + seccomp + limited capabilities.
- Configurable per user, per group, or globally:
  ```toml
  [sessions]
  jail_type = "namespace"               # none, namespace, seccomp, container, combined
  jail_allowed_paths = ["/home", "/tmp", "/usr", "/bin", "/lib"]
  jail_denied_syscalls = ["ptrace", "mount", "reboot"]
  jail_network = "host"                 # host, isolated, none
  jail_max_processes = 200
  jail_max_memory_mb = 4096
  jail_max_disk_mb = 10240
  ```
- Resource limits enforced via cgroups v2:
  - CPU shares / quota.
  - Memory limit (hard and soft).
  - I/O bandwidth limits.
  - Process count limits.

### Service Obfuscation
- LiquidDE supports hiding its identity from network scanners and unauthorized probes.
- **Protocol obfuscation**:
  - The initial handshake can be disguised to not reveal the service type.
  - Connection attempts without a valid protocol version header receive no response (silent drop).
  - Configurable banner/identification:
    - `default` — identifies as LiquidDE (standard).
    - `minimal` — returns only a protocol version, no product name.
    - `hidden` — no identification whatsoever; unknown clients get connection reset.
    - `custom` — administrator-defined response string.
- **Port knocking** (optional):
  - Before the service port accepts connections, the client must send a specific sequence of packets to other ports.
  - Sequence configurable, shared as part of the connection profile.
- **Service fingerprint reduction**:
  - TLS certificate does not include product-specific fields by default.
  - Server timing responses are randomized to prevent fingerprinting.
  - Error responses are generic (no stack traces, no version numbers).
- Configuration:
  ```toml
  [security.obfuscation]
  service_banner = "hidden"            # default, minimal, hidden, custom
  custom_banner = ""
  silent_drop_unknown = true           # silently drop non-LiquidDE connections
  port_knocking_enabled = false
  port_knocking_sequence = [7331, 8442, 9553]
  port_knocking_timeout_sec = 10
  timing_randomization = true
  fingerprint_reduction = true
  ```

---

## 16) Stream Analysis

- **Real-time stream statistics** available to users and admins:
  - Current FPS (render, encode, transport, client present).
  - Current bandwidth (in/out, per channel).
  - Latency (input-to-photon estimate, RTT).
  - Packet loss percentage.
  - Encoder performance (encode time per frame, queue depth).
  - Transport mode in use (QUIC, TCP, etc.).
  - Encryption scheme in use.
  - Dirty region ratio (% of screen changing per frame).
  - Cache hit rates (blur, wallpaper, partial regions).
  - Effect budget utilization.
- Accessible via:
  - `liquidctl stats` CLI command.
  - Client-side overlay (see [spec-client.md](spec-client.md)).
  - Prometheus metrics endpoint (for monitoring infrastructure).
  - Management UI (see [spec-manager.md](spec-manager.md)).

---

## 17) Observability & Operations

### Metrics (Prometheus)
- FPS (server render, encode, client present).
- Latency (input-to-photon estimate).
- Bandwidth in/out (total, per channel).
- Packet loss / RTT.
- Dirty region ratio.
- Encode time per frame.
- Active sessions count.
- CPU and memory usage per session.
- Cache hit rates.

### Logs
- Structured logs (JSON option).
- Per-session log correlation ID.
- Configurable log levels per subsystem.

### Extensive Logging System
LiquidDE has a comprehensive, per-component logging system designed for production debugging, auditing, and monitoring:

#### Log Subsystems
Each subsystem logs independently with its own configurable log level:

| Subsystem | Log File | Contents |
|-----------|----------|----------|
| `server` | `server.log` | Server lifecycle, config changes, listener events |
| `session` | `session.log` | Session start/stop/resume, user activity |
| `auth` | `auth.log` | Login attempts, MFA events, cert validation, lockouts |
| `render` | `render.log` | Compositor events, cache hits/misses, effect budget |
| `encode` | `encode.log` | Encoder selection, frame timing, bitrate changes |
| `transport` | `transport.log` | Connection events, transport switches, packet loss |
| `audio` | `audio.log` | Audio device events, codec negotiation, buffer underruns |
| `clipboard` | `clipboard.log` | Clipboard sync events (metadata only by default) |
| `usb` | `usb.log` | Device attach/detach, transfer statistics |
| `input` | `input.log` | Input device events, layout changes (no keystrokes logged) |
| `policy` | `policy.log` | Policy evaluation, enforcement actions |
| `metrics` | `metrics.log` | Periodic metric snapshots |
| `audit` | `audit.log` | Security-relevant events (immutable, append-only) |

#### Log Configuration
```toml
[logging]
base_dir = "/var/log/liquidde"
format = "json"                       # json, text, syslog
max_file_size_mb = 100                # per log file
max_files = 10                        # rotation count
compress_rotated = true               # gzip rotated logs
unified_log = false                   # true = all subsystems to one file
syslog_enabled = false
syslog_facility = "local0"
syslog_address = "127.0.0.1:514"

# Per-subsystem log levels
[logging.levels]
server = "info"
session = "info"
auth = "info"                          # always at least "info" for security
render = "warn"
encode = "warn"
transport = "info"
audio = "warn"
clipboard = "info"
usb = "info"
input = "warn"
policy = "info"
metrics = "warn"
audit = "info"                         # always at least "info"
```

#### Log Features
- **Correlation IDs**: every log entry includes a session ID and request ID for tracing across subsystems.
- **Structured fields**: all log entries are key-value structured, not free-form text.
- **Log rotation**: automatic rotation by size or time with configurable retention.
- **Compression**: rotated logs compressed with gzip automatically.
- **Syslog forwarding**: all logs can be forwarded to a remote syslog server (RFC 5424).
- **Log streaming**: real-time log streaming via `liquidctl logs tail` (see [spec-liquidctl.md](spec-liquidctl.md)).
- **Audit log immutability**: audit logs are append-only, with HMAC integrity verification.
- **Sensitive data redaction**: passwords, tokens, and keys are never logged; clipboard content is hashed, not stored.
- **Performance**: logging is async and uses lock-free buffers — logging never blocks the render/encode pipeline.

### Admin Tools
- See [spec-liquidctl.md](spec-liquidctl.md) for the full `liquidctl` CLI specification.
- Quick reference:
  - `liquidctl status` — sessions, bandwidth, latency.
  - `liquidctl sessions list/kill` — manage active sessions.
  - `liquidctl policy set ...` — manage policies.
  - `liquidctl benchmark` — run performance benchmarks.
  - `liquidctl config validate` — validate configuration files.

---

## 18) RDP Compatibility Layer

- **Disabled by default**. Enabled via configuration:
  ```toml
  [rdp_compat]
  enabled = false
  listen = "0.0.0.0:3389"
  ```
- When enabled, provides an RDP endpoint for standard RDP clients (mstsc, FreeRDP, etc.).
- Supported RDP features:
  - Basic display (bitmap updates).
  - Keyboard and mouse input.
  - Clipboard (text).
  - Audio playback.
  - Drive redirection (basic).
- Limitations documented: not all LiquidDE features available via RDP (e.g., no hybrid tile+video, no transport switching).
- Useful for environments where installing LiquidClient is not possible.

---

## 19) Server Configuration

### Configuration Files
- `/etc/liquidde/server.toml` — server-wide configuration.
- `/etc/liquidde/policies.toml` — policy definitions.
- `~/.config/liquidde/session.toml` — per-user session preferences.
- `~/.config/liquidde/theme.css` — per-user CSS theme.

### Server Configuration Structure (`server.toml`)

```toml
# ─── General ────────────────────────────────────────────────
[general]
hostname = "liquid-server-01"
log_level = "info"                    # trace, debug, info, warn, error
log_format = "json"                   # json, text
data_dir = "/var/lib/liquidde"

# ─── Listening ──────────────────────────────────────────────
[[listen]]
address = "0.0.0.0:3389"
transport = "quic"

[[listen]]
address = "0.0.0.0:3390"
transport = "tls-tcp"

# ─── TLS ────────────────────────────────────────────────────
[tls]
cert = "/etc/liquidde/cert.pem"
key = "/etc/liquidde/key.pem"
acme_enabled = false
acme_domain = ""
acme_email = ""

# ─── Encryption ─────────────────────────────────────────────
[encryption]
default_scheme = "tls13"              # tls13, aes-128-gcm, aes-256-gcm, chacha20
allow_plaintext_localhost = true

# ─── Authentication ─────────────────────────────────────────
[auth]
provider = "pam"                      # local, pam, ldap, oidc
mfa_enabled = false
mfa_required = false
mfa_methods = ["totp", "fido2", "smartcard", "biometric"]
mfa_remember_device_days = 30
max_login_attempts = 5
lockout_duration_sec = 300

[auth.certificate]
enabled = false
client_ca_file = ""
username_field = "CN"

[auth.fido2]
relying_party_id = ""
attestation = "none"

[auth.smartcard]
pkcs11_module = ""
require_pin = true

# ─── Sessions ───────────────────────────────────────────────
[sessions]
max_concurrent = 50
idle_timeout_sec = 3600
disconnect_action = "lock"            # lock, keep, terminate
resume_enabled = true
isolation = "systemd-user"            # systemd-user, namespace, container

# ─── Display ────────────────────────────────────────────────
[display]
default_resolution = "1920x1080"
max_resolution = "7680x4320"
default_dpi = 96
max_virtual_monitors = 8
default_refresh_rate = 60

# ─── Encoding ───────────────────────────────────────────────
[encoding]
default_encoder = "h264"
allowed_encoders = ["h264", "h265", "av1", "vp9", "vp8", "mjpeg", "zstd", "lz4", "png", "qoi", "webp", "raw"]
hardware_encoding = "auto"            # auto, force-cpu, force-gpu
default_preset = "interactive"        # interactive, balanced, bandwidth-saver, lan

# ─── Performance ────────────────────────────────────────────
[performance]
effect_budget = "auto"                # auto, <ms value>, minimal, balanced, quality
blur_downsample = "auto"              # auto, 2, 4, 8, 16
wallpaper_enabled = true
wallpaper_cache = true
partial_cache_enabled = true
partial_cache_level = 3               # 1-5
idle_fps = 2
active_fps = 60
fps_ramp_speed = "fast"               # instant, fast, smooth
animation_enabled = true
benchmark_on_start = true

# ─── Transport ──────────────────────────────────────────────
[transport]
negotiation = "auto"                  # auto, priority, specific
preferred = "quic"
priority_list = ["quic", "udp", "tls-tcp", "tcp", "websocket"]
hybrid_channels = true
mtu = "auto"                          # auto, <bytes>
fec_enabled = false
fec_redundancy = 0.1
congestion_algorithm = "bbr"          # bbr, cubic

# ─── Audio ──────────────────────────────────────────────────
[audio]
enabled = true                        # false = disable audio entirely
playback_enabled = true
microphone_enabled = true
default_codec = "opus"                # opus, aac, vorbis, flac, alac, pcm, g711, g722, speex, mp3, wma
allowed_codecs = ["opus", "aac", "vorbis", "flac", "pcm", "g722"]
codec_negotiation = "auto"            # auto, fixed
sample_rate = 48000
channels = "stereo"                   # mono, stereo, 5.1, 7.1
buffer_ms = 20
playback_bitrate = 128000             # bps
microphone_bitrate = 64000            # bps
silence_detection = true              # pause codec on silence
transport_channel = "dedicated"       # dedicated, shared

# ─── Camera ─────────────────────────────────────────────────
[camera]
passthrough_enabled = false
max_resolution = "1280x720"
max_fps = 30
codec = "mjpeg"

# ─── USB/IP ──────────────────────────────────────────────────
[usb]
enabled = false                       # disabled by default
redirection_enabled = false
transport_channel = "dedicated"       # dedicated, shared
allowed_device_classes = ["mass-storage", "printer", "smartcard", "security-key"]
allowed_vid_pid = []                  # empty = allow all (when enabled)
blocked_vid_pid = []
max_devices_per_session = 5
max_bandwidth_mbps = 50
audit_log = true

# ─── Clipboard ──────────────────────────────────────────────
[clipboard]
enabled = true
direction = "bidirectional"           # bidirectional, client-to-server, server-to-client, disabled
max_size_bytes = 10485760             # 10 MB
allowed_types = ["text/plain", "text/html", "image/png"]
rate_limit_per_min = 60
content_inspection = false
audit_log = true

# ─── RDP Compatibility ─────────────────────────────────────
[rdp_compat]
enabled = false
listen = "0.0.0.0:3389"

# ─── Client Rendering Offload ──────────────────────────────
[offload]
level = "cursor-only"                 # none, cursor-only, chrome, text, full
font_rendering = "auto"              # auto, always, never, hybrid
font_cache_max_mb = 200
font_sync_on_connect = true

# ─── Gateway ────────────────────────────────────────────────
[gateway]
enabled = false
gateway_url = ""
reverse_connect = false
registration_token = ""

# ─── Tiling ────────────────────────────────────────────────
[tiling]
enabled = true
default_mode = "hybrid"
default_layout = "split-horizontal"
gap = 8
outer_gap = 8
master_ratio = 0.55

# ─── Tablet Mode ───────────────────────────────────────────
[tablet_mode]
enabled = false
auto_detect = false
on_screen_keyboard = true
gesture_navigation = true

# ─── Security ──────────────────────────────────────────────
[security]
fail2ban_log = "/var/log/liquidde/auth.log"
rate_limit_enabled = true
rate_limit_max_attempts = 5
rate_limit_window_sec = 300

[security.obfuscation]
service_banner = "default"
silent_drop_unknown = false
port_knocking_enabled = false

# ─── Logging ───────────────────────────────────────────────
[logging]
base_dir = "/var/log/liquidde"
format = "json"
max_file_size_mb = 100
max_files = 10
compress_rotated = true
syslog_enabled = false

[logging.levels]
server = "info"
session = "info"
auth = "info"
render = "warn"
encode = "warn"
transport = "info"

# ─── Metrics ────────────────────────────────────────────────
[metrics]
prometheus_enabled = true
prometheus_listen = "127.0.0.1:9100"
stream_analysis = true
```

### Policy Configuration (`policies.toml`)

```toml
# Default policy (applies to all users unless overridden)
[default]
clipboard = "bidirectional"
file_transfer = true
audio_playback = true
audio_microphone = false
camera = false
usb_redirection = false
max_sessions = 3
max_resolution = "3840x2160"
max_fps = 60
allowed_encoders = ["h264", "h265", "av1"]
allowed_transports = ["quic", "tls-tcp"]

# Group overrides
[group.developers]
clipboard = "bidirectional"
file_transfer = true
max_sessions = 5

[group.guests]
clipboard = "server-to-client"
file_transfer = false
usb_redirection = false
max_resolution = "1920x1080"
max_fps = 30

# User overrides
[user.admin]
max_sessions = 10
```

---

## 20) CSS Theming System

### Overview
LiquidDE uses a CSS-like styling language for all visual elements. Users and administrators can customize the appearance of the entire desktop environment through CSS files.

### CSS Scope
- **System theme**: `/etc/liquidde/theme.css` — base theme, ships with Liquid Glass defaults.
- **User theme**: `~/.config/liquidde/theme.css` — user overrides, merged on top of system theme.

### Supported Properties
The theming engine supports a subset of CSS3 + custom properties:

- **Colors**: `color`, `background-color`, `border-color`, `accent-color`.
- **Backgrounds**: `background`, gradients, `background-image` (local paths).
- **Borders**: `border`, `border-radius`, `outline`.
- **Shadows**: `box-shadow` (including inset shadows).
- **Blur**: `backdrop-filter: blur()`, `filter: blur()`.
- **Opacity**: `opacity`.
- **Spacing**: `padding`, `margin`, `gap`.
- **Typography**: `font-family`, `font-size`, `font-weight`, `line-height`, `letter-spacing`.
- **Layout**: `display` (flex, grid basics), `align-items`, `justify-content`.
- **Transitions**: `transition` (property, duration, easing).
- **Custom properties**: `--liquid-*` namespace for theme variables.

### CSS Selectors (Shell Elements)
```css
/* Top-level containers */
.liquid-desktop { }
.liquid-dock { }
.liquid-status-bar { }
.liquid-launcher { }
.liquid-notification { }

/* Window chrome */
.liquid-window { }
.liquid-window.focused { }
.liquid-window .titlebar { }
.liquid-window .close-btn { }
.liquid-window .minimize-btn { }
.liquid-window .maximize-btn { }

/* Dock items */
.liquid-dock .dock-item { }
.liquid-dock .dock-item.active { }
.liquid-dock .dock-item:hover { }
.liquid-dock .dock-separator { }

/* Panels */
.liquid-panel { }
.liquid-panel.glass { }

/* Notifications */
.liquid-notification { }
.liquid-notification.urgent { }

/* System tray */
.liquid-tray { }
.liquid-tray .tray-icon { }
```

### Default Theme Variables
```css
:root {
  --liquid-glass-blur: 20px;
  --liquid-glass-opacity: 0.7;
  --liquid-glass-tint: rgba(255, 255, 255, 0.1);
  --liquid-accent: #007AFF;
  --liquid-text: #FFFFFF;
  --liquid-text-secondary: rgba(255, 255, 255, 0.7);
  --liquid-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
  --liquid-border: 1px solid rgba(255, 255, 255, 0.15);
  --liquid-radius: 12px;
  --liquid-radius-lg: 16px;
  --liquid-dock-height: 64px;
  --liquid-statusbar-height: 28px;
  --liquid-transition-speed: 200ms;
  --liquid-transition-easing: cubic-bezier(0.4, 0, 0.2, 1);
}
```

Full CSS documentation in [spec-design.md](spec-design.md).

---

## 21) Built-in Core Apps (Remote-Friendly)

1. **Liquid Terminal**
   - CPU-rendered terminal emulator.
   - Text-only fast path (tile encoding).
   - Optional ligatures.
   - Configurable via CSS.

2. **File Manager**
   - Large preview generation done lazily.
   - Network mounts optional.
   - Drag-and-drop file transfer integration.

3. **Settings**
   - Performance profiles.
   - Clipboard policy.
   - Display & scaling.
   - Keyboard layout selection.
   - Theme/CSS editor with live preview.
   - Dock configuration.

4. **Task Monitor**
   - Shows CPU, RAM, encode load, FPS, latency.
   - Stream analysis dashboard.
   - Per-session resource usage.

---

## 22) Implementation Plan

### Language
- **Everything in Rust**: server, compositor, shell, renderer, encoder bindings, transport, CLI tools.
- C FFI bindings for: FreeType, HarfBuzz, Fontconfig, codec libraries (x264, x265, SVT-AV1, etc.).
- Client also in Rust (see [spec-client.md](spec-client.md)).

### Platform Support

| Platform | Architecture | Role | Status |
|----------|-------------|------|--------|
| Linux | x86_64 | Server + Client | Primary |
| Linux | ARM64 | Server + Client | Primary |
| macOS | ARM64 | Client | Primary |
| macOS | x86_64 | Client | Secondary |
| Windows | x86_64 | Client | Primary |
| Windows | ARM64 | Client | Secondary |

### MVP Scope
- Headless server session (single virtual monitor).
- Native client (Linux, macOS, Windows).
- Clipboard (text, bidirectional).
- Dynamic resize.
- Video mode (H.264) + cursor channel.
- QUIC transport.
- Basic shell: dock, launcher, terminal.
- CSS theming (basic).
- CPU-only rendering.

### v1 Scope
- Multi-monitor with on-demand virtual screens.
- Hybrid tile/video encoding.
- All 10+ encoders.
- Bidirectional audio.
- Full policy engine.
- Stream analysis.
- Server configuration tool.
- Full CSS theming.
- Multiple transport strategies.
- GPU acceleration (optional).
- Gateway support.

### vNext
- Web client (WebRTC).
- Camera passthrough.
- USB redirection.
- RDP compatibility layer.
- OIDC authentication.
- Management UI.
- Client rendering offload (full).

---

## 23) Compatibility & Interop

### Wayland Protocol Support
- `wl_compositor`, `wl_shm`, `wl_seat`, `wl_output`, `wl_data_device`.
- `xdg_shell` (toplevel, popup).
- `zwlr_layer_shell_v1` (panels, overlays).
- `wp_fractional_scale_v1`.
- `wp_viewporter`.
- `xdg_decoration_unstable_v1`.
- XWayland for legacy X11 applications.

### RDP Compatibility
- See §18.

---

## 24) Test Plan

### Functional
- Connect/disconnect/reconnect behavior.
- Clipboard bidirectional (all types).
- Resize storms (rapid dragging).
- Multi-monitor add/remove/resize.
- Transport switching mid-session.
- Audio bidirectional.
- Keyboard layout switching.

### Performance
- Measure:
  - Input-to-photon latency.
  - Encode time per frame.
  - Bandwidth usage per encoder.
  - Idle CPU usage (target: <1%).
  - Cache hit rates.
  - Benchmark calibration accuracy.
- Network emulation:
  - High RTT (50ms, 100ms, 200ms, 500ms).
  - Packet loss (1%, 5%, 10%).
  - Bandwidth caps (1Mbps, 5Mbps, 20Mbps, 100Mbps).

### Reliability
- Fuzz protocol decoding.
- Long-run session soak tests (24h+).
- Worker thread crash recovery tests.
- Transport failover tests.

### Security
- TLS configuration validation.
- Policy enforcement tests.
- Authentication brute-force protection.
- Audit log completeness.

---

## 25) Failure Modes & Mishap Hardening

This DE is explicitly designed to avoid common remote-desktop pain:

- **No hidden GPU dependency**: renderer stays CPU-only unless GPU explicitly enabled and available.
- **No hanging**: main thread is an orchestrator only; stuck workers are killed and restarted.
- **Clipboard is a first-class feature**: not an afterthought.
- **Dynamic resizing is built-in**: not a best-effort.
- **Transport resilience**: automatic failover between transport strategies.
- **Cache corruption recovery**: if any cache is detected as corrupted, it's rebuilt automatically.
- **Graceful degradation**: if CPU budget is exceeded, effects degrade gracefully rather than dropping frames.

---

## 26) Deliverables

- **Server**:
  - `liquid-desktopd` — main server daemon.
  - `liquid-session` — per-user session runner.
  - `liquidctl` — admin CLI (see [spec-liquidctl.md](spec-liquidctl.md)).

- **Client**:
  - `LiquidClient` — native client for Windows/macOS/Linux (see [spec-client.md](spec-client.md)).
  - Optional web client.

- **Gateway**:
  - `liquid-gateway` — NAT traversal gateway (see [spec-gateway.md](spec-gateway.md)).

- **Management**:
  - `liquid-manager` — web-based management UI (see [spec-manager.md](spec-manager.md)).

- **Docs**:
  - Admin guide.
  - Security guide.
  - Performance tuning guide.
  - CSS theming guide.
  - Troubleshooting playbook.
  - API reference.

---

## 27) Performance Profiles (Defaults)

### Interactive (default)
- Target latency: lowest.
- FPS: 60 when active, 2 when idle.
- Codec: H.264 low-latency preset.
- Effect budget: auto (benchmark-calibrated).
- Blur: downsampled, cached.
- Transport: QUIC preferred.

### Balanced
- Target: good quality.
- FPS: 45 active, 2 idle.
- Hybrid tile/video enabled.
- Effect budget: balanced.
- Full glass effects.

### Bandwidth Saver
- Target: minimum bandwidth.
- FPS: 30 active, on-change idle.
- Aggressive tile dedupe and compression.
- Reduced blur (box blur or disabled).
- Wallpaper optionally disabled.

### LAN
- Target: maximum quality.
- FPS: 60 active, 5 idle.
- Minimal compression (raw or LZ4 tiles).
- Full glass effects at maximum quality.
- AES-128-GCM encryption (lower overhead).

---

## 28) Open Questions (Answered with Sensible Defaults)

- Default protocol: **native QUIC protocol**.
- RDP compatibility: **disabled by default, available in config**.
- Shell toolkit: **minimal, custom-built, CSS-driven**.
- Default encryption: **TLS 1.3 with AES-256-GCM**.
- Default encoder: **H.264 (CPU) or VAAPI H.264 (GPU)**.
- Default dock position: **bottom**.
- Default transport: **QUIC with auto-negotiation**.
- Management UI: **disabled by default** (see [spec-manager.md](spec-manager.md)).
