# LiquidDE Server & Desktop Environment — Full Specification

> **Language**: Rust
> **License**: MIT
> **Related specs**: [Client](spec-client.md) · [Gateway](spec-gateway.md) · [Management UI](spec-manager.md) · [liquidctl CLI](spec-liquidctl.md) · [Design Language](spec-design.md) · [Night Theme](spec-theme-night.md) · [Sunset Theme](spec-theme-sunset.md) · [Midday Theme](spec-theme-midday.md)

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
├── Plugin Worker (1–N) — WASM plugin host, sandbox execution, plugin lifecycle
├── Logging Worker — async structured log writing
└── Metrics Worker — telemetry collection, stream analysis
```

The main thread **never** performs blocking work. It runs an async event loop (tokio or equivalent) that dispatches work items to worker threads via lock-free channels. If any worker thread hangs or exceeds a deadline, the orchestrator can kill and restart it without crashing the session.

The Plugin Worker hosts WASM plugin sandboxes via wasmtime. Each plugin runs in its own isolated WASM instance with independent memory and CPU fuel budgets. Plugin faults (traps, timeouts, OOM) are caught at the sandbox boundary — a crashing plugin never affects the session or other plugins.

Above the session process, a **supervisor process** (`liquid-desktopd`) manages session lifecycle. Each user session runs as a separate `liquid-session` child process. The supervisor monitors sessions via heartbeat IPC, detects crashes, captures diagnostics, and manages restart policies (see §13 Session Supervisor & Process Model).

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

8. **Plugin Engine**
   - WASM-based plugin runtime (wasmtime).
   - Sandboxed execution with memory and CPU limits.
   - 9 extension points for shell, widgets, input, themes, and more (see §14b).

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

#### Window-Level Offload

Beyond offloading individual rendering elements (cursors, chrome, text), the server can offload **entire windows** to the client for local rendering. This is particularly effective for text-heavy applications like terminals, consoles, log viewers, and code editors.

##### How Window Offload Works
1. Server identifies eligible windows (based on app type or configuration).
2. Instead of compositing the window into the framebuffer and encoding it as video/tiles, the server sends **structured window data** over a dedicated channel.
3. The client receives the data and renders the entire window locally — including chrome, content, scrollbars, and cursor.
4. The client composites the locally-rendered window into the session display alongside server-streamed windows.

##### Offload Data Modes

- **State mode** (`state`): Server sends the terminal/application state as a character grid:
  - Character grid (rows × columns) with per-cell attributes (foreground color, background color, bold, italic, underline, blink, reverse, strikethrough).
  - Cursor position, shape, and blink state.
  - Scrollback buffer (configurable depth, incrementally synced).
  - Window title and icon.
  - Selection state (if any text is selected).
  - Bell/alert state.
  - Suitable for: terminal emulators, console applications.

- **Structured mode** (`structured`): Server sends window content as structured rendering commands:
  - Text runs with font, size, color, position, and decorations.
  - Background rectangles with colors/gradients.
  - Borders, separators, and dividers.
  - Scrollbar state (position, range, thumb size).
  - Window chrome parameters (title, buttons, decorations).
  - Suitable for: text editors, log viewers, code editors, any text-dominant window.

##### Benefits
- **Eliminates encoding overhead**: text-heavy windows are not processed by the video encoder at all.
- **Pixel-perfect text**: client renders text at native DPI with local subpixel rendering.
- **Massive bandwidth savings**: a terminal character grid (~20KB) replaces a 1080p encoded frame region (~50–200KB per frame).
- **Client-native scrolling**: scrollback navigation is instant (local), no round-trip needed.
- **Scales with window count**: additional terminal windows add negligible server-side CPU or bandwidth cost.

##### Interaction with Other Offload Levels
- Window-level offload is **independent** of the general `offload.level` setting.
- A session can mix window-offloaded terminals with server-streamed graphical applications.
- Window offload automatically enables `text` offload for the offloaded windows (font cache applies).
- If the client does not support window offload, the server falls back to normal encoding for those windows.

##### Terminal Offload Protocol
- On window creation: server sends `window_offload_start` with window ID, offload mode, initial state.
- On state change: server sends incremental diffs (changed cells, cursor moves, scroll events).
- On window close: server sends `window_offload_stop`.
- Scrollback: server sends scrollback chunks on demand (client requests ranges).
- Input: keystrokes for offloaded windows are sent to the server normally; only rendering is client-side.

##### Window Offload Configuration
```toml
[offload]
window_offload = "none"                # none, terminal, all-text-windows
terminal_offload_mode = "state"        # state (character grid), structured (text runs + layout)
terminal_scrollback_sync = 1000        # max scrollback lines synced to client initially
window_offload_apps = []               # app_ids eligible for window-level offload (empty = auto-detect)
```

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
  - QWERTY (US, UK, Australian, Canadian, Irish, etc.), AZERTY (French, Belgian), QWERTZ (German, Swiss, Hungarian, Czech).
  - Dvorak, Colemak, Colemak-DH, Workman, Norman.
  - CJK input method support (IME): Chinese (Pinyin, Wubi, Cangjie, Bopomofo), Japanese (Romaji, Kana), Korean (Hangul 2-Set, 3-Set).
  - Indic scripts: Devanagari (Hindi, Marathi, Sanskrit), Bengali, Tamil, Telugu, Kannada, Malayalam, Gujarati, Punjabi (Gurmukhi).
  - Arabic, Hebrew, Persian (Farsi), Urdu.
  - Cyrillic: Russian (JCUKEN), Ukrainian, Bulgarian, Serbian.
  - Greek, Georgian, Armenian, Ethiopic (Amharic, Tigrinya).
  - Thai (Kedmanee, Pattachote), Vietnamese (Telex, VNI, VIQR).
  - Turkish (Q, F), Azerbaijani, Kazakh, Uzbek.
  - Custom layout definition files (XKB-compatible format).
- **Keystroke capture and forward**: client captures all keystrokes (including system shortcuts like Alt+Tab, Super, Ctrl+Alt+Del) and forwards them to the remote session when the client window has focus.
- **No 'sticky modifiers' under latency**: modifier key state tracked precisely.
- **Dead keys and compose sequences** supported.
- **Input method framework**: built-in lightweight IME framework for CJK and complex script input, with support for external IBus/Fcitx protocol bridging.

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

## 12b) Internationalization (i18n) & Localization

### Overview
LiquidDE provides comprehensive internationalization support. All user-facing text in the DE shell, login screen, settings, and built-in applications is translatable. The i18n system covers UI translations, keyboard layouts, date/time formatting, number formatting, and text directionality.

### Supported Languages

LiquidDE ships with translations for 40+ languages. The translation framework supports any additional language through community-contributed message catalogs.

#### Tier 1 — Full Translation + Full QA
- English (en-US, en-GB, en-AU)
- German (de-DE, de-AT, de-CH)
- French (fr-FR, fr-CA, fr-BE)
- Spanish (es-ES, es-MX, es-AR)
- Portuguese (pt-BR, pt-PT)
- Japanese (ja-JP)
- Chinese Simplified (zh-CN)
- Chinese Traditional (zh-TW, zh-HK)
- Korean (ko-KR)
- Russian (ru-RU)

#### Tier 2 — Full Translation
- Italian (it-IT)
- Dutch (nl-NL, nl-BE)
- Polish (pl-PL)
- Czech (cs-CZ)
- Hungarian (hu-HU)
- Romanian (ro-RO)
- Turkish (tr-TR)
- Arabic (ar-SA, ar-EG)
- Hebrew (he-IL)
- Hindi (hi-IN)
- Thai (th-TH)
- Vietnamese (vi-VN)
- Indonesian (id-ID)
- Malay (ms-MY)
- Swedish (sv-SE)
- Norwegian (nb-NO, nn-NO)
- Danish (da-DK)
- Finnish (fi-FI)
- Ukrainian (uk-UA)
- Greek (el-GR)

#### Tier 3 — Community Translation
- Any additional language supported through community-contributed `.ftl` (Fluent) message catalogs.
- Community translations are loaded from `/etc/liquidde/i18n/` or `~/.config/liquidde/i18n/`.

### Translation Framework

#### Message Format
- Translation uses **Project Fluent** (`.ftl` files) — a modern localization system designed for natural-sounding translations.
- Fluent supports pluralization, gender, number formatting, and complex grammatical rules natively.
- Message files are stored at:
  - System: `/etc/liquidde/i18n/<locale>/messages.ftl`
  - User overrides: `~/.config/liquidde/i18n/<locale>/messages.ftl`
- Example message file:
  ```ftl
  # /etc/liquidde/i18n/de-DE/messages.ftl
  login-greeting-morning = Guten Morgen
  login-greeting-afternoon = Guten Tag
  login-greeting-evening = Guten Abend
  login-submit = Anmelden
  login-error-invalid = Falscher Benutzername oder Passwort
  dock-open-launcher = Starter öffnen
  settings-title = Einstellungen
  settings-display = Anzeige
  settings-keyboard = Tastatur
  settings-language = Sprache
  notification-connected = Verbunden mit { $server }
  session-idle-warning =
      { $minutes ->
          [one] Sitzung wird in { $minutes } Minute gesperrt
         *[other] Sitzung wird in { $minutes } Minuten gesperrt
      }
  ```

#### Fallback Chain
1. User's selected locale (e.g., `de-AT`).
2. Language base (e.g., `de-DE`).
3. English (`en-US`) as ultimate fallback.
4. Raw message key if no translation exists (development/debug only).

### Keyboard Layout Configuration

Full keyboard layout specification with variants and options:

```toml
# ~/.config/liquidde/keyboard-layout.toml

[keyboard]
layout = "us"                            # primary layout
variant = ""                             # layout variant (e.g., "dvorak", "colemak", "intl")
model = "pc105"                          # keyboard model
options = ["compose:ralt"]               # XKB options

# Additional layouts (switchable at runtime)
[[keyboard.additional]]
layout = "de"
variant = ""
label = "DE"                             # indicator label in status bar

[[keyboard.additional]]
layout = "ru"
variant = "phonetic"
label = "RU"

# Layout switching
[keyboard.switching]
method = "hotkey"                        # hotkey, indicator-click, both
hotkey = "super+space"                   # shortcut to cycle layouts
show_indicator = true                    # show current layout in status bar
indicator_format = "short"               # short (US), long (English (US)), flag
per_window = true                        # remember layout per window (vs. global)
```

#### Supported Keyboard Layouts (Complete List)

| Region | Layouts |
|--------|---------|
| **Americas** | us, us(intl), us(dvorak), us(colemak), us(workman), br, ca, ca(fr), latam |
| **Western Europe** | gb, ie, de, de(neo), fr, fr(bepo), es, pt, it, nl, be, be(iso-alternate), ch, ch(fr), at |
| **Northern Europe** | se, no, dk, fi, is |
| **Eastern Europe** | pl, cz, sk, hu, ro, bg, bg(phonetic), rs, rs(latin), hr, si, ee, lt, lv |
| **Cyrillic** | ru, ru(phonetic), ua, by, kz, mk |
| **Greek/Turkish** | gr, tr, tr(f), az |
| **Middle East** | ara, il, ir, pk |
| **South Asia** | in(deva), in(beng), in(taml), in(telu), in(knda), in(mlym), in(gujr), in(guru) |
| **East Asia** | jp, kr, cn, tw |
| **Southeast Asia** | th, th(pattachote), vn, my, id |
| **Other** | ge, am, et, dz, ma, tn, eg, ph, za |

### Regional Format Settings

Date, time, number, and currency formatting respect the user's locale:

```toml
# ~/.config/liquidde/session.toml

[locale]
language = "en-US"                       # UI language (message catalog)
region = "en-US"                         # regional formats (numbers, dates, currency)
# Override individual format categories:
date_format = "auto"                     # auto (from region), iso8601, us, european, japanese
time_format = "auto"                     # auto (from region), 24h, 12h
first_day_of_week = "auto"              # auto (from region), monday, sunday, saturday
number_format = "auto"                   # auto (from region), period-comma (1,234.56), comma-period (1.234,56), space-comma (1 234,56)
currency_format = "auto"                 # auto (from region), symbol-before ($1.00), symbol-after (1.00€)
measurement_system = "auto"              # auto (from region), metric, imperial
temperature_unit = "auto"                # auto (from region), celsius, fahrenheit
paper_size = "auto"                      # auto (from region), a4, letter
```

#### Date & Time Format Examples

| Locale | Date (long) | Date (short) | Time | First Day |
|--------|-------------|--------------|------|-----------|
| en-US | Saturday, February 8, 2026 | 2/8/2026 | 2:30 PM | Sunday |
| en-GB | Saturday, 8 February 2026 | 08/02/2026 | 14:30 | Monday |
| de-DE | Samstag, 8. Februar 2026 | 08.02.2026 | 14:30 | Monday |
| ja-JP | 2026年2月8日 土曜日 | 2026/02/08 | 14:30 | Sunday |
| ar-SA | السبت، ٨ فبراير ٢٠٢٦ | ٨/٢/٢٠٢٦ | ٢:٣٠ م | Saturday |
| zh-CN | 2026年2月8日 星期六 | 2026/2/8 | 14:30 | Monday |
| ko-KR | 2026년 2월 8일 토요일 | 2026. 2. 8. | 오후 2:30 | Sunday |
| pt-BR | sábado, 8 de fevereiro de 2026 | 08/02/2026 | 14:30 | Sunday |
| ru-RU | суббота, 8 февраля 2026 г. | 08.02.2026 | 14:30 | Monday |
| hi-IN | शनिवार, 8 फ़रवरी 2026 | 8/2/2026 | 2:30 अपराह्न | Sunday |

#### Number Format Examples

| Locale | Number | Currency | Percentage |
|--------|--------|----------|------------|
| en-US | 1,234,567.89 | $1,234.56 | 85.5% |
| de-DE | 1.234.567,89 | 1.234,56 € | 85,5 % |
| fr-FR | 1 234 567,89 | 1 234,56 € | 85,5 % |
| ja-JP | 1,234,567.89 | ¥1,235 | 85.5% |
| hi-IN | 12,34,567.89 | ₹1,234.56 | 85.5% |

### Text Directionality (BiDi)

- LiquidDE supports **right-to-left (RTL)** text layout for Arabic, Hebrew, Persian, and Urdu locales.
- When an RTL language is the primary locale:
  - Status bar layout mirrors (clock/tray on left, app menus on right).
  - Dock position defaults mirror (though user can override).
  - Settings panels, dialogs, and list layouts mirror horizontally.
  - Notification slide direction reverses (enters from left instead of right).
- **Mixed BiDi content** is handled within text fields using the Unicode Bidirectional Algorithm (UBA).
- CSS logical properties are used throughout the DE shell (e.g., `margin-inline-start` instead of `margin-left`) so layouts adapt automatically.

### Font Support

- **Font fallback chains** are locale-aware. When a glyph is not available in the primary font, the system falls back through locale-appropriate fonts:
  - CJK: Noto Sans CJK (SC/TC/JP/KR variants selected per locale).
  - Indic: Noto Sans Devanagari, Bengali, Tamil, etc.
  - Arabic: Noto Sans Arabic, Noto Naskh Arabic.
  - Thai: Noto Sans Thai.
- **Font rendering**: HarfBuzz is used for complex script shaping (Arabic ligatures, Indic conjuncts, Thai character composition).
- **System fonts shipped**: Inter (Latin), JetBrains Mono (monospace), Noto Sans (universal fallback), Noto Sans CJK, Noto Sans Arabic, Noto Sans Devanagari, Noto Sans Thai.

### Server-Side i18n Configuration

```toml
# /etc/liquidde/server.toml

[i18n]
default_locale = "en-US"                 # system default language
available_locales = ["en-US", "en-GB", "de-DE", "fr-FR", "ja-JP", "zh-CN", "ko-KR", "es-ES", "pt-BR", "ru-RU", "ar-SA", "hi-IN"]
fallback_locale = "en-US"
message_dir = "/etc/liquidde/i18n"       # system message catalogs
allow_user_translations = true           # allow user-provided .ftl overrides
keyboard_layout_dir = "/etc/liquidde/xkb" # custom XKB layout directory
```

### i18n in Login Screen

- The login screen language is determined by:
  1. Client-side language preference (if set).
  2. Server's `default_locale`.
- The login screen language selector (bottom-right utility area) allows changing the locale before authentication.
- Changing the login screen language reloads all translatable text (greeting, button labels, error messages, placeholders) immediately.
- Clock and date formatting on the login screen respect the selected locale.

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
  - `session.toml` — DE preferences (dock position, wallpaper, layout, locale, appearance).
  - `keybindings.toml` — custom keyboard shortcuts.
  - `keyboard-layout.toml` — preferred keyboard layout and variants.
  - `avatar.png` — user's profile avatar image (see User Profile & Avatar below).
- Defaults inherited from system config, user overrides take priority.
- Config changes apply live (no session restart needed for most settings).

### User Profile & Avatar

Each user has a profile that includes display metadata used on the login screen, lock screen, and within the DE shell.

#### Avatar Image

- **Storage location**: `~/.config/liquidde/avatar.png` (per-user, on the server).
- **Supported formats**: PNG (preferred), JPEG, WebP, SVG. All formats are internally converted and stored as PNG. SVG uploads are sanitized before rasterization (see SVG Upload Security below).
- **Dimensions**: source images are automatically cropped and resized. Final stored size: 256×256px (the server generates scaled versions as needed: 128×128 for dock/tray, 120×120 for login screen, 64×64 for small contexts, 32×32 for notifications).
- **Maximum upload size**: 2 MB (pre-crop/resize). Configurable per-server.
- **Circular crop**: the avatar is always displayed in a circular mask. The upload flow allows the user to position and resize a circular crop region on rectangular source images.
- **Fallback**: if no avatar is set, the system generates an initials-based avatar using the first letter of the username (or first+last initial if a display name is configured). The initials are rendered in the accent color on a frosted glass circle. This generated fallback is **visually indistinguishable** from users who have simply not uploaded an avatar — this is critical for user enumeration prevention on the login screen.
- **Anti-enumeration**: when the server responds to a username submission during login, it always returns an avatar response (real avatar or generated initials fallback) with identical timing and response format. See §15 Login Screen for details.

#### Avatar Management

Users can manage their avatar through:
1. **Settings app** → Profile section → Avatar editor.
   - Upload from file (PNG, JPEG, WebP, SVG).
   - Crop and position using a circular crop overlay.
   - Preview before applying.
   - Remove avatar (revert to initials fallback).
2. **CLI**: `liquidctl user avatar set <path>` / `liquidctl user avatar remove`.
3. **Management UI**: administrators can set/remove avatars for any user.

#### Avatar API (Server-Side)

```
POST   /api/v1/users/{username}/avatar     Upload avatar (multipart/form-data, field: "avatar")
GET    /api/v1/users/{username}/avatar      Get avatar image (returns PNG, or 204 No Content for initials fallback)
DELETE /api/v1/users/{username}/avatar      Remove avatar
```

- Upload endpoint accepts PNG, JPEG, WebP, or SVG. Server crops and resizes to 256×256. SVG uploads are sanitized and rasterized before storage.
- Server generates and caches scaled variants (128, 120, 64, 32px).
- Avatar cache invalidation: when avatar changes, all sessions for the user receive an avatar update notification. Lock screens update immediately.

#### SVG Upload Security

SVG files can contain executable content and external references that pose security risks. All SVG uploads undergo mandatory sanitization before rasterization:

- **Script removal**: all `<script>` elements, `on*` event handlers (onclick, onload, onerror, etc.), and `javascript:` URIs are stripped.
- **External reference removal**: `<use xlink:href="...">` with external URLs, `<image href="...">` with external URLs, CSS `url()` with external references, `<foreignObject>` elements.
- **Embedded content removal**: `<iframe>`, `<embed>`, `<object>`, `<applet>`, `<foreignObject>` elements.
- **Metadata stripping**: XML processing instructions, DTD declarations, and `<!ENTITY>` definitions (to prevent XML entity expansion attacks / "billion laughs").
- **Namespace restriction**: only SVG and XLink namespaces are permitted.
- **Size validation**: SVG `viewBox` and dimensions must produce a rasterizable image within reasonable bounds (max 4096×4096 logical units).
- After sanitization, the SVG is rasterized to PNG at the configured `stored_size` (default: 256×256) using the server's rendering pipeline.
- The original SVG is **never stored** — only the rasterized PNG is persisted.

#### User Display Name

- Optional display name for richer profile display.
- Stored in `~/.config/liquidde/session.toml`:
  ```toml
  [profile]
  display_name = "Alice Johnson"           # optional, shown on lock screen and login greeting
  avatar = "avatar.png"                    # relative to ~/.config/liquidde/
  initials_override = ""                   # override auto-generated initials (e.g., "AJ")
  ```
- If no display name is set, the Unix username is displayed.

#### Avatar Transfer

- **Login screen**: after username submission, the server sends the avatar (if it exists) or a generated initials SVG to the client. Transfer size: ≤64KB (120px avatar). The client caches avatars keyed on `server_address + username + avatar_hash`.
- **Lock screen**: avatar is already cached from the session start. If the avatar changes during the session, the lock screen updates on next display.
- **Client cache**: cached avatars persist across connections. Cache keyed on `(server_address, username, avatar_hash)`. Cache size configurable in client `[wallpaper_cache]` section.

#### Server Avatar Configuration

```toml
# /etc/liquidde/server.toml

[avatar]
enabled = true                           # allow user avatars
max_upload_size_bytes = 2097152          # 2 MB
stored_size = 256                        # stored avatar size (px, square)
allowed_formats = ["png", "jpeg", "webp", "svg"]
svg_sanitize = true                     # always sanitize SVG uploads (strongly recommended: true)
generate_initials_fallback = true        # generate initials avatar when none uploaded
default_avatar = ""                      # path to server-wide default avatar (blank = initials)
```

### Session Supervisor & Process Model

LiquidDE uses a **supervisor process model** for session isolation. The server daemon (`liquid-desktopd`) never runs user session code directly — it spawns a separate `liquid-session` child process for each authenticated user.

#### Process Hierarchy

```
liquid-desktopd (supervisor daemon)
├── liquid-session (user: alice, session: s-001)
│   ├── Worker threads (render, encode, transport, audio, input, plugin, ...)
│   └── Child processes (XWayland, user applications)
├── liquid-session (user: bob, session: s-002)
│   └── ...
└── Supervisor thread pool
    ├── Heartbeat monitor
    ├── Resource monitor (cgroups)
    └── Crash handler
```

#### Session Process Isolation

- Each `liquid-session` runs as the target user's UID/GID.
- cgroup v2 is used for resource containment per session:
  - CPU quota (shares or hard limit).
  - Memory limit (hard + soft, OOM events monitored).
  - I/O bandwidth limits.
  - Process count limits (pids.max).
- Crash of one session **never** affects other sessions or the supervisor daemon.
- The supervisor daemon runs as a privileged service (root or `liquidde` service user) and handles process lifecycle only — it never touches user data directly.

#### Heartbeat Monitoring

- Each `liquid-session` sends periodic heartbeat messages to the supervisor via a Unix domain socket IPC channel.
- Default: heartbeat every 5 seconds.
- If the supervisor misses `heartbeat_timeout_count` consecutive heartbeats (default: 3), the session is declared hung.
- Hung session handling:
  1. Send `SIGTERM` to session process (grace period: 10 seconds).
  2. If still alive, send `SIGKILL`.
  3. Capture crash context (same as crash path).
  4. Notify client.
  5. Attempt restart per restart policy.

#### Crash Capture

When a session process terminates unexpectedly (non-zero exit, signal):
1. Supervisor detects exit via `waitpid()` / `SIGCHLD`.
2. Exit code and signal number are recorded.
3. If `coredump_enabled = true` and a coredump exists, its path is recorded.
4. Last `crash_log_lines` (default: 100) lines from the session's log file are captured.
5. Session metadata is recorded: user, session ID, uptime, last known state, resource usage at time of crash.
6. A crash report is written to `crash_report_dir` (see §25).
7. Client is notified via `crash_info` message (see §25 BSOD).

#### Restart Policy

- **Immediate restart** on first crash (no delay).
- **Exponential backoff** on subsequent crashes: `restart_backoff_base_ms` × 2^(N-1), capped at 30 seconds.
- **Maximum restarts**: `max_restarts` (default: 5) within `restart_window_sec` (default: 600 seconds / 10 minutes).
- After exhausting restarts: session enters `failed` state. Client shows persistent crash screen. Admin can force restart via `liquidctl supervisor restart <session-id>`.
- Restart counter resets after `restart_window_sec` without a crash.
- On restart, the supervisor starts a fresh `liquid-session` process. If session state was persisted (compositor state, window list), the new process can resume from it — otherwise it starts a new session.

#### Resource Monitoring

- Supervisor periodically checks session resource usage via cgroup v2 interfaces:
  - `memory.current` / `memory.max` — detect approaching OOM before the kernel kills the process.
  - `memory.events` — detect OOM killer invocations.
  - `pids.current` — process/thread count.
  - `cpu.stat` — CPU usage tracking.
- When a session approaches resource limits (>90% memory, >95% PIDs), a warning is sent to the client.
- The supervisor itself has minimal resource footprint — it performs no rendering, encoding, or heavy computation.

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

A **full-featured application launcher** accessible via dock icon, keyboard shortcut (`Super` key by default), or configurable hot corner. The launcher is the primary interface for discovering, searching, and launching applications.

#### Visual Layout
- **Overlay panel**: a centered glass panel (default: 600px wide, up to 70% screen height) with heavy blur backdrop.
- **Search bar**: prominent text input at the top with a magnifying glass icon. Auto-focused on open.
- **Results area**: below the search bar, displays results as a scrollable list or grid.
- **Sections** (visible when search is empty):
  1. **Favorites / Pinned**: user-pinned apps displayed at the top in a horizontal row or small grid.
  2. **Recent**: recently launched apps sorted by recency (default) or frequency (configurable). Shows up to 8 entries.
  3. **All Apps by category**: categorized list of all available applications, collapsible per category.

#### Search
- **Fuzzy matching**: searches across application name, description, keywords, categories, and executable name.
- **Instant results**: results update as the user types (debounced at 50ms).
- **Ranking**: results ranked by: exact match > prefix match > substring match > fuzzy match. Frequency-weighted: frequently launched apps rank higher for ambiguous queries.
- **Calculator / Quick Answers**: if the query is a mathematical expression (e.g., `2+2`, `sqrt(144)`, `15% of 200`), the result is displayed inline at the top of the results. Supports basic arithmetic, percentages, powers, roots, trigonometry, and unit conversions (e.g., `10km in miles`, `72F in C`).
- **File search** (optional): when enabled, searches file names from an indexed database alongside apps. Results appear in a separate "Files" section.
- **Web search fallback** (optional): when no app results match, an option to "Search the web for '{query}'" appears at the bottom. Opens the configured default browser with the default search engine.
- **Custom commands**: if the query starts with a configurable prefix (default: `>`), it is treated as a shell command. A "Run: `{command}`" entry appears. Pressing Enter executes the command in a new terminal.

#### Categories
- System, Development, Internet, Office, Media, Graphics, Utilities, Games, Settings, Other.
- Categories are auto-assigned from `.desktop` file categories.
- Uncategorized apps appear under "Other."
- Category headers in list view show the category name and app count.
- Categories are collapsible — click the header to expand/collapse.

#### Application Metadata
- Apps are discovered from:
  1. `.desktop` files in standard XDG directories (`/usr/share/applications/`, `~/.local/share/applications/`).
  2. LiquidDE built-in apps (Liquid Terminal, File Manager, Settings, Task Monitor).
  3. Custom app entries defined in `~/.config/liquidde/apps.toml`.
- Metadata per app: `name`, `description`, `icon`, `exec`, `categories`, `keywords`, `terminal` (bool), `no_display` (bool).
- App list is refreshed on desktop file changes (inotify-based) and on launcher open.

#### Views
- **List view** (default): vertical list with icon (36px), app name, and description. One app per row.
- **Grid view**: icon grid with larger icons (64px) and app name below. Suitable for touch and visual browsing.
- Toggle between views with a view-switcher button in the launcher header or keyboard shortcut (`Ctrl+G`).
- View preference is persisted per user.

#### Keyboard Navigation
- **Type to search**: any keypress while the launcher is open starts filtering (search bar is always focused).
- **Arrow keys**: navigate through results (Up/Down in list view, Up/Down/Left/Right in grid view).
- **Enter**: launch the selected app (or execute the calculator result / custom command).
- **Escape**: close the launcher. If search has text, first Escape clears search; second Escape closes.
- **Tab**: cycle between sections (Favorites, Recent, All Apps, Search Results).
- **Ctrl+N / Ctrl+P**: alternative next/previous navigation (vim-style friendly).
- **Ctrl+1..9**: quick-launch the Nth result.

#### Quick Actions (Context Menu)
Right-clicking (or long-pressing on touch) an app item shows a context menu:
- **Launch**: open the application.
- **Pin to Favorites**: add to the favorites/pinned section.
- **Unpin from Favorites**: remove from favorites (shown only for pinned apps).
- **Pin to Dock**: add a persistent dock icon for this app.
- **Open File Location**: open the file manager at the app's `.desktop` file or executable location.
- **Run in Terminal**: launch the app inside a Liquid Terminal window.
- **App Info**: show details (version, .desktop file path, categories, executable).

#### Favorites / Pinned Apps
- Users can pin apps to a dedicated "Favorites" section at the top of the launcher.
- Favorites are stored in `~/.config/liquidde/session.toml` under `[launcher.favorites]`.
- Favorites are displayed as a horizontal row of icons (list view) or a top grid section (grid view).
- Drag-and-drop reordering within favorites.
- Maximum favorites: configurable (default: 20).

#### Animations
- **Open**: launcher fades in and scales from 95% to 100% over 200ms (`ease-out`).
- **Close**: reverse of open animation (100% to 95%, fade out) over 150ms.
- **Result transitions**: new results cross-fade in as the user types (100ms).
- **All animations respect** `prefers-reduced-motion` and the effect budget.

#### Plugins / Extension Points
- The launcher supports **result provider plugins** — external processes or scripts that can contribute search results:
  - Plugins register via a manifest file in `~/.config/liquidde/launcher-plugins/`.
  - Each plugin specifies: name, trigger prefix (optional), icon, and an executable that receives the query on stdin and returns results as JSON on stdout.
  - Plugin results appear in a dedicated section after app results.
  - Built-in plugins: Calculator, File Search, Web Search. These can be disabled individually.
- Plugin API:
  ```json
  // Input (stdin, one line):
  {"query": "user search text", "max_results": 10}
  // Output (stdout, one JSON object):
  {"results": [{"title": "Result", "description": "...", "icon": "path", "action": "open:https://..."}]}
  ```

#### Workspace Integration
- Optionally, the launcher can include a **workspace switcher strip** at the bottom showing numbered workspace thumbnails.
- Clicking a workspace switches to it and closes the launcher.
- Configurable: on, off (default: off).

#### Launcher Configuration
```toml
# ~/.config/liquidde/session.toml

[launcher]
shortcut = "Super"                        # activation shortcut
hot_corner = ""                           # none, top-left, top-right, bottom-left, bottom-right
default_view = "list"                     # list, grid
show_favorites = true
show_recent = true
recent_count = 8
recent_sort = "recency"                   # recency, frequency
show_categories = true
search_files = false                      # search file names alongside apps
search_web = false                        # offer web search fallback
web_search_engine = "https://duckduckgo.com/?q={query}"
custom_command_prefix = ">"
calculator_enabled = true
workspace_switcher = false
max_favorites = 20
plugin_dir = "~/.config/liquidde/launcher-plugins/"
animation_enabled = true

[launcher.favorites]
apps = ["liquid-terminal", "firefox", "code"]   # pinned app IDs in display order
```

#### Launcher CSS Classes
- `.liquid-launcher` — launcher overlay.
- `.liquid-launcher .search-bar` — search bar area.
- `.liquid-launcher .search-input` — search text input.
- `.liquid-launcher .results` — results area (list or grid).
- `.liquid-launcher .app-item` — application entry.
- `.liquid-launcher .app-item.selected` — highlighted entry.
- `.liquid-launcher .favorites-section` — favorites/pinned section.
- `.liquid-launcher .category-header` — category divider.
- `.liquid-launcher .quick-answer` — calculator/quick answer display.
- `.liquid-launcher .context-menu` — right-click context menu.
- Full CSS reference in [spec-design.md](spec-design.md).

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

### Seamless Window Mode

LiquidDE supports a **seamless window mode** where individual remote application windows are "detached" from the LiquidClient container and presented as **native OS windows** on the client's local desktop. This is analogous to Citrix Seamless Windows, RDP RemoteApp, or VMware Unity mode.

#### Server-Side Behavior
- When a client requests seamless mode (or when configured as default), the server:
  - Tracks each application window independently (geometry, z-order, state, icon, title).
  - Encodes and transmits frame data **per-window** rather than as a single desktop framebuffer.
  - Sends window lifecycle events (create, destroy, minimize, maximize, restore, move, resize, z-order change, title change, icon change, focus change).
  - Does **not** render the desktop shell (dock, status bar, wallpaper) — these are omitted or optionally presented as their own separate "shell windows."
  - Maintains a virtual desktop coordinate space for correct window positioning.

#### Per-Window Encoding
- Each window is treated as an independent encoding region.
- The server uses the same tile/video encoding pipeline, but scoped to each window's bounding rectangle.
- Windows with no changes consume zero bandwidth (damage tracking per window).
- Small utility windows (tooltips, menus, dropdowns) are grouped with their parent window to avoid per-window overhead.

#### Window Lifecycle Messages

| Message | Direction | Data |
|---------|-----------|------|
| `seamless_window_create` | Server → Client | window_id, app_id, title, icon, initial geometry, z_order, state |
| `seamless_window_destroy` | Server → Client | window_id |
| `seamless_window_geometry` | Server → Client | window_id, x, y, width, height |
| `seamless_window_state` | Server → Client | window_id, state (normal, minimized, maximized, fullscreen) |
| `seamless_window_title` | Server → Client | window_id, new_title |
| `seamless_window_icon` | Server → Client | window_id, icon_data |
| `seamless_window_zorder` | Server → Client | ordered list of window_ids |
| `seamless_window_focus` | Server → Client | window_id |
| `client_window_move` | Client → Server | window_id, new_x, new_y |
| `client_window_resize` | Client → Server | window_id, new_width, new_height |
| `client_window_state` | Client → Server | window_id, requested_state |
| `client_window_focus` | Client → Server | window_id |
| `client_window_close` | Client → Server | window_id |

#### Interaction with Window-Level Offload
- Seamless mode and window-level offload (see §9) are complementary.
- A terminal in seamless mode can be further offloaded: the client creates a native OS window and renders the terminal content locally from character grid data.
- This combination produces the most efficient result: zero server encoding, native window, pixel-perfect text.

#### Seamless Mode Limitations
- Additional per-window encoding overhead on the server (separate damage tracking and encode regions).
- Transient windows (menus, tooltips, dropdowns) may flicker or misposition if they extend beyond their parent window bounds. The server groups these with their parent.
- Audio remains session-wide (not per-window spatial audio).
- DE shell elements (dock, status bar) are not shown by default — optionally presented as their own native windows (`shell_as_window = true`).

#### Seamless Mode Configuration (Server-Side)
```toml
# Per-user session.toml
[seamless]
enabled = false                           # user preference for seamless mode
exclude_apps = ["liquidde-desktop"]       # apps that stay on the virtual desktop
shell_as_window = false                   # show dock/status bar as separate native windows
```

### Notifications
- "Stacking" to avoid animation storms.
- Do-not-disturb mode.
- Notification history panel.

---

## 14b) WASM Plugin & Extension System

### Overview

LiquidDE provides a comprehensive **WebAssembly (WASM)-based plugin system** that allows third-party and user-created extensions to augment the desktop environment. Plugins run in sandboxed WASM runtimes with strict memory and CPU resource limits, ensuring that no plugin can crash, hang, or compromise the host session.

The plugin system is designed for:
- **Performance**: Near-native execution speed via ahead-of-time compiled WASM. Minimal overhead per call. Memory-pooled allocations.
- **Safety**: Complete isolation — plugins cannot access host memory, other plugins, or the filesystem beyond what is explicitly granted via host functions.
- **Stability**: Versioned ABIs ensure plugins remain compatible across LiquidDE updates. Deprecation policy gives plugin authors time to migrate.
- **Extensibility**: Nine distinct extension points cover shell UI, input processing, notifications, file handling, theming, and more.

### Plugin Architecture

#### WASM Runtime

LiquidDE uses **wasmtime** as its WASM runtime:
- Rust-native, no FFI overhead.
- Ahead-of-time (AOT) compilation for near-native performance.
- **Fuel-based CPU metering**: each WASM instruction consumes fuel, providing precise CPU time quotas without relying on wall-clock timers.
- **Memory limiter**: per-instance hard memory caps enforced by the runtime.
- **Async host functions**: plugins can call host functions that perform async I/O without blocking other plugins.
- **Module caching**: compiled WASM modules are cached to avoid recompilation on restart.
- **WASI preview 2**: plugins get a minimal WASI environment (clock, random) but **no filesystem, network, or environment variable access** unless explicitly granted.

#### ABI Versioning

| ABI Version | Status | Introduced | Deprecated | Removed |
|-------------|--------|------------|------------|---------|
| v1 | Active | v0.1.0 | — | — |

- Plugins declare their target ABI version in `plugin.toml` (`abi_version = "v1"`).
- The host checks compatibility at load time. Incompatible plugins are rejected with a clear error message.
- **Forward compatibility**: new host functions can be added to an ABI version without breaking existing plugins (plugins that don't call the new functions continue to work).
- **Breaking changes** require a new ABI version. The host can support multiple ABI versions concurrently.
- **Deprecation policy**: deprecated ABI versions are supported for at least 2 major releases before removal.

### Extension Points

LiquidDE provides **nine extension points** that plugins can register for. A plugin may register for one or more extension points.

#### 1. Shell Extensions
Modify dock behavior, add status bar items, custom workspace behaviors.
- **Entry**: `on_shell_event(event: ShellEvent) -> ShellAction`
- **Events**: dock_item_click, workspace_switch, session_focus, status_bar_tick
- **Host functions available**: UI API, Session API, Theme API

#### 2. Panel Widgets
Small UI components rendered into status bar widget slots.
- **Entry**: `render_widget(width: u32, height: u32) -> WidgetRenderResult`
- **Trigger**: periodic (configurable interval, default 1s) or on event
- **Host functions available**: UI API, Session API, Timer API, Theme API
- **Output**: structured render commands (text, icon, progress bar, sparkline) — **not** raw pixels

#### 3. Notification Handlers
Intercept, filter, modify, or suppress notifications before they are displayed.
- **Entry**: `on_notification(notification: Notification) -> NotificationAction`
- **Actions**: pass_through, suppress, modify(fields), defer(duration)
- **Host functions available**: Notification API, Config API, Storage API

#### 4. File Type Handlers
Custom preview and thumbnail generation for the file manager.
- **Entry**: `generate_preview(file_info: FileInfo, max_size: Size) -> PreviewResult`
- **Input**: file metadata and a read-only byte slice of file content (capped at configurable size)
- **Output**: image data (PNG/RGBA), text preview, or "unsupported" signal
- **Host functions available**: Storage API (read-only), Logging API

#### 5. Input Preprocessors
Intercept and transform input events before they reach the compositor.
- **Entry**: `on_input(event: InputEvent) -> InputAction`
- **Actions**: pass_through, consume, replace(new_event), emit_multiple([events])
- **Use cases**: custom gesture recognition, key remapping, macro expansion, accessibility input transforms
- **Host functions available**: Input API, Config API, Timer API
- **Performance**: input preprocessors must return within 2ms (reduced fuel allocation)

#### 6. Theme Generators
Programmatic theme generation — wallpaper-adaptive color extraction, time-of-day theme shifting, dynamic accent colors.
- **Entry**: `generate_theme(context: ThemeContext) -> ThemeOverrides`
- **Trigger**: on wallpaper change, on time interval, on user event
- **Output**: set of CSS custom property overrides (`--liquid-accent`, `--liquid-bg-desktop`, etc.)
- **Host functions available**: Theme API, Timer API, Config API

#### 7. Launcher Result Providers
Provide custom search results in the app launcher. **Supersedes** the legacy stdin/stdout launcher plugin system described in §14 — legacy plugins continue to work via a compatibility shim that wraps them as WASM-equivalent providers.
- **Entry**: `on_query(query: string, max_results: u32) -> QueryResults`
- **Output**: array of `{title, description, icon, action, relevance_score}`
- **Actions**: `open_url`, `run_command`, `copy_text`, `insert_text`, `custom(data)`
- **Host functions available**: Config API, Storage API, Logging API
- **Performance**: queries must return within 100ms; partial results supported via streaming callback

#### 8. Session Lifecycle Hooks
Run logic on session lifecycle events.
- **Entry**: `on_session_event(event: SessionEvent)`
- **Events**: session_start, session_stop, session_lock, session_unlock, session_suspend, session_resume, theme_change, monitor_change
- **Host functions available**: Session API, Config API, Storage API, Logging API, Notification API

#### 9. Clipboard Transformers
Transform clipboard content during sync between server and client.
- **Entry**: `on_clipboard(content: ClipboardContent, direction: Direction) -> ClipboardAction`
- **Directions**: server_to_client, client_to_server
- **Actions**: pass_through, replace(new_content), block(reason)
- **Use cases**: format conversion, PII redaction, content sanitization, URL shortening
- **Host functions available**: Clipboard API, Config API, Logging API

### Plugin Manifest Format

Each plugin is a directory containing `plugin.toml` and `plugin.wasm`:

```toml
[plugin]
id = "com.example.weather-widget"
name = "Weather Widget"
description = "Shows current weather in the status bar"
version = "1.2.0"
author = "Example Corp"
license = "MIT"
abi_version = "v1"
entry_module = "plugin.wasm"

[plugin.requirements]
liquidde_min_version = "0.1.0"
capabilities = ["ui", "timers", "storage", "notifications"]

[plugin.resources]
max_memory_mb = 16                       # plugin requests up to 16 MB (< server max of 256 MB)
max_cpu_fuel = 10_000_000                # lower than default — this plugin is lightweight

[plugin.extension_points]
panel_widget = { slot = "status-bar-right", interval_ms = 60000 }
session_lifecycle = { events = ["session_start", "session_stop"] }

[plugin.config]
# Plugin-specific config keys with defaults
api_key = ""
city = "auto"
units = "metric"                         # metric, imperial
```

### Plugin Lifecycle

```
    ┌─────────┐
    │  LOAD   │  wasmtime compiles .wasm, validates manifest, checks ABI version
    └────┬────┘
         ▼
    ┌─────────┐
    │  INIT   │  host calls plugin's init(config) export with plugin config
    └────┬────┘
         ▼
    ┌──────────┐
    │ ACTIVATE │  plugin starts receiving events, rendering widgets, etc.
    └────┬─────┘
         │
    ┌────▼─────┐  ◄── on idle/lock: host calls suspend() — plugin serializes state
    │ SUSPEND  │
    └────┬─────┘
         │
    ┌────▼─────┐  ◄── on resume: host calls resume(state) — plugin restores
    │  RESUME  │
    └────┬─────┘
         │
    ┌────▼──────────┐
    │  DEACTIVATE   │  host calls deactivate() — plugin cleans up
    └────┬──────────┘
         ▼
    ┌─────────┐
    │ UNLOAD  │  WASM instance dropped, memory freed
    └─────────┘
```

- **Fault during any phase**: plugin is moved to `FAULTED` state. The session continues without the plugin. Watchdog may attempt restart depending on configuration.
- **Hot-reload**: triggers `DEACTIVATE → UNLOAD → LOAD (new binary) → INIT → ACTIVATE`. State is preserved via `suspend()`/`resume()` if supported.

### Plugin Registry & Discovery

- **System plugins**: `/etc/liquidde/plugins/<plugin-id>/plugin.toml` + `plugin.wasm`
- **User plugins**: `~/.config/liquidde/plugins/<plugin-id>/plugin.toml` + `plugin.wasm`
- **Discovery**: directories are scanned at session start. When `hot_reload = true`, inotify/kqueue watches detect changes.
- **Conflicts**: if system and user plugins share the same ID, the user plugin takes precedence (user can override system plugins).
- **Signature verification**: if `[plugins] signature_required = true`, each `plugin.wasm` must have a detached signature file `plugin.wasm.sig` (Ed25519).

### Host Functions (ABI v1)

Plugins call host functions to interact with the desktop environment. Functions are grouped by capability:

#### UI API (`capability: ui`)
| Function | Description |
|----------|-------------|
| `ui_show_toast(message, duration_ms)` | Show a temporary toast message |
| `ui_set_badge(target, count)` | Set a badge count on a dock item |
| `ui_request_attention(target)` | Request attention (bouncing dock icon) |
| `ui_get_active_window() -> WindowInfo` | Get info about the active window |
| `ui_get_monitor_info() -> [MonitorInfo]` | Get monitor layout and resolution |

#### Session API (`capability: session`)
| Function | Description |
|----------|-------------|
| `session_get_user() -> string` | Get current username |
| `session_get_uptime() -> u64` | Get session uptime in seconds |
| `session_get_state() -> SessionState` | Get session state (active, locked, etc.) |
| `session_get_locale() -> string` | Get session locale |

#### Notification API (`capability: notifications`)
| Function | Description |
|----------|-------------|
| `notification_send(title, body, icon, urgency)` | Send a notification |
| `notification_cancel(id)` | Cancel a pending notification |

#### Clipboard API (`capability: clipboard`)
| Function | Description |
|----------|-------------|
| `clipboard_read_text() -> string` | Read text from clipboard |
| `clipboard_write_text(text)` | Write text to clipboard |
| `clipboard_get_formats() -> [string]` | List available clipboard formats |

#### Config API (`capability: config`)
| Function | Description |
|----------|-------------|
| `config_get(key) -> string` | Read plugin config value |
| `config_set(key, value)` | Write plugin config value (persisted) |
| `config_get_all() -> Map<string, string>` | Read all plugin config values |

#### Storage API (`capability: storage`)
| Function | Description |
|----------|-------------|
| `storage_read(key) -> bytes` | Read from plugin-scoped persistent storage |
| `storage_write(key, value)` | Write to plugin-scoped persistent storage |
| `storage_delete(key)` | Delete a key from storage |
| `storage_list_keys() -> [string]` | List all keys in plugin storage |

Storage is plugin-scoped (isolated per plugin) and stored at `~/.config/liquidde/plugin-data/<plugin-id>/`.

#### Logging API (no capability required)
| Function | Description |
|----------|-------------|
| `log(level, message)` | Write to plugin log subsystem |

#### Timer API (`capability: timers`)
| Function | Description |
|----------|-------------|
| `timer_set(duration_ms, repeat) -> timer_id` | Schedule a timer callback |
| `timer_cancel(timer_id)` | Cancel a timer |

#### Theme API (`capability: theme`)
| Function | Description |
|----------|-------------|
| `theme_get_property(name) -> string` | Read a CSS custom property value |
| `theme_set_overrides(overrides: Map)` | Set CSS custom property overrides |
| `theme_get_type() -> string` | Get active theme type (dark/light) |

#### IPC API (`capability: ipc`)
| Function | Description |
|----------|-------------|
| `ipc_send(target_plugin_id, message)` | Send message to another plugin |
| `ipc_broadcast(channel, message)` | Broadcast message on a named channel |
| `ipc_subscribe(channel)` | Subscribe to messages on a named channel |

### Inter-Plugin Communication

- Plugins communicate via **typed message passing** through the IPC API — no shared memory between plugins.
- Messages are serialized (MessagePack) at the plugin boundary and deserialized by the recipient.
- **Directed messages**: `ipc_send(target_id, message)` — delivery guaranteed if target is active, dropped if target is faulted/unloaded.
- **Broadcast channels**: `ipc_broadcast(channel, message)` — all subscribers receive the message.
- **Built-in channels**: `theme_changed`, `session_event`, `locale_changed` (plugins can subscribe to system events via IPC).

### Hot-Reload

When a plugin's `.wasm` file changes on disk (detected via inotify/kqueue):

1. **Current instance**: `deactivate()` → `suspend()` → serialize state → `unload()`.
2. **New instance**: `load()` new binary → `init(config)` → `resume(saved_state)` → `activate()`.
3. **Rollback**: if the new instance fails to load or init, the old binary is re-loaded and the plugin is restored to its previous state. An error notification is shown to the user.
4. **No-state plugins**: if a plugin does not export `suspend()`/`resume()`, it is simply restarted fresh.

Hot-reload happens with **zero downtime** for other plugins and the session.

### Plugin Development SDK

The primary SDK is a Rust crate: `liquidde-plugin-sdk`.

```rust
use liquidde_plugin_sdk::prelude::*;

#[liquid_plugin]
struct WeatherWidget {
    api_key: String,
    city: String,
}

impl PanelWidget for WeatherWidget {
    fn render(&self, width: u32, height: u32) -> WidgetRenderResult {
        let temp = self.fetch_temperature();
        WidgetRenderResult::text(format!("{}°C", temp))
    }
}

impl SessionLifecycle for WeatherWidget {
    fn on_session_start(&mut self) {
        log_info!("Weather widget started");
    }
}
```

**Language support**: Plugins can be written in any language that compiles to WASM:
- **Rust** (primary, first-class SDK).
- **C/C++** (via wasi-sdk).
- **AssemblyScript** (TypeScript-like, compiles to WASM).
- **Go** (via TinyGo).
- **Zig** (native WASM target).

### Plugin Sandboxing & Resource Limits

#### Memory Limits
- Each plugin declares its maximum memory in `plugin.toml` (default: 32 MB, max: 256 MB).
- The wasmtime memory limiter enforces this at the WASM level — allocations beyond the limit cause a trap.
- **Total plugin memory budget**: `max_plugins_per_session × default_memory_limit_mb` gives a worst-case upper bound. In practice, most plugins use far less.

#### CPU Time Quotas
- Each host→plugin call is allocated fuel proportional to the expected execution time.
- Default: 50 million fuel units (~50ms of CPU time on typical hardware).
- Input preprocessors run with reduced fuel (2ms equivalent) for latency sensitivity.
- Launcher query handlers run with a 100ms fuel budget.
- **Fuel calibration**: the host benchmarks fuel-to-wall-clock ratio at startup and adjusts fuel allocations accordingly.

#### Fault Trapping
- All WASM traps are caught at the host boundary. The plugin is marked `FAULTED`.
- The extension point gracefully degrades:
  - Shell extension fault → shell reverts to default behavior.
  - Panel widget fault → widget slot shows a small error indicator (red dot).
  - Notification handler fault → notifications pass through unmodified.
  - Input preprocessor fault → input events pass through unmodified.
  - Clipboard transformer fault → clipboard content passes through unmodified.

#### Plugin Watchdog
- The Plugin Worker runs a watchdog timer that monitors all active plugins.
- **Fault tracking**: each plugin maintains a fault counter. Faults within the restart window increment the counter.
- **Auto-restart**: on fault, the plugin is automatically restarted (deactivate → unload → load → init → activate).
- **Backoff**: restart delays increase exponentially (1s, 2s, 4s, 8s...).
- **Permanent disable**: after `max_restarts` within `restart_window_sec`, the plugin is permanently disabled for the session. Admin can re-enable via `liquidctl plugins enable <id>`.

#### Graceful Degradation
When a plugin is disabled (manually or by watchdog), its extension points revert to default behavior. The session continues normally. A notification is shown to the user: "Plugin <name> has been disabled due to repeated errors."

### Plugin Configuration

#### Server-Side (`server.toml`)
See §19 `[plugins]` and `[plugins.resources]` configuration sections.

#### User-Side (`~/.config/liquidde/session.toml`)
```toml
[plugins]
enabled_plugins = []                     # empty = all installed; list IDs to restrict
disabled_plugins = ["com.example.broken-plugin"]  # explicitly disabled
```

#### Per-Plugin (`~/.config/liquidde/plugins/<id>/config.toml`)
```toml
# Plugin-specific config (schema defined by plugin.toml [plugin.config])
api_key = "user-api-key"
city = "London"
units = "metric"
```

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

### Login Screen

The login screen is the first visual experience a user has with LiquidDE. It is a full-screen, Liquid Glass themed interface that presents authentication options with elegance and clarity.

#### Visual Composition

The login screen is composed of distinct visual layers:

```
┌─────────────────────────────────────────────────────┐
│                                                     │
│   Wallpaper                                         │
│   ┌───────────────────────────────────────────────┐ │
│   │  Frosted Glass Layer (full-screen blur)       │ │
│   │                                               │ │
│   │           ┌──────────────┐                    │ │
│   │           │   Clock &    │                    │ │
│   │           │    Date      │                    │ │
│   │           └──────────────┘                    │ │
│   │                                               │ │
│   │           ┌──────────────┐                    │ │
│   │           │    Avatar    │                    │ │
│   │           │  (circular)  │                    │ │
│   │           └──────────────┘                    │ │
│   │           Username                            │ │
│   │           Greeting                            │ │
│   │                                               │ │
│   │           ┌──────────────────────┐            │ │
│   │           │  Password Input      │            │ │
│   │           └──────────────────────┘            │ │
│   │           [Auth method icons]                 │ │
│   │                                               │ │
│   │           ┌─────────────┐                     │ │
│   │           │  Sign In    │                     │ │
│   │           └─────────────┘                     │ │
│   │                                               │ │
│   │           [Session resume indicator]          │ │
│   │                                               │ │
│   │  ┌──────┐                      ┌──────────┐  │ │
│   │  │Server│                      │ Power /  │  │ │
│   │  │ Info │                      │ Network  │  │ │
│   │  └──────┘                      └──────────┘  │ │
│   └───────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────┘
```

#### Background Layer

- **Wallpaper**: the server's configured login wallpaper (separate from per-user desktop wallpaper).
- **Frosted glass overlay**: a full-screen layer of Liquid Glass blur applied over the wallpaper, creating a soft, diffused backdrop.
- **Blur intensity**: stronger than standard panel blur — `blur(40px)` default — to ensure the login content is the sole focus.
- **Ambient light**: a subtle radial gradient centered behind the avatar, giving a gentle glow that draws the eye inward.
- **Particle effect** (optional, disabled by default): slow-moving translucent circles that drift across the background, adding depth without distraction. Configurable, purely aesthetic.

#### Clock & Date

- Positioned at the **upper-center** of the screen, above the avatar.
- **Time**: large, light-weight display font. Default: 72px, `font-weight: 200` (thin). Renders the current server time.
- **Date**: below the clock, smaller secondary text. Format: "Saturday, February 8" (localized). `font-size: 16px`, `font-weight: 400`.
- Both respect the user's locale and 12h/24h preference.
- Subtle fade-in animation on screen activation.

#### User Avatar

- **Circular profile image** centered on-screen, framed by a glass ring.
- **Size**: 120×120px default, configurable up to 160px.
- **Glass ring**: a 3px translucent border with inner glow (`box-shadow: inset 0 0 12px rgba(255,255,255,0.15)`) that gives the avatar a "set into glass" appearance.
- **Fallback**: if no avatar is set, a frosted glass circle with the user's initials rendered in the accent color.
- **Avatar appears after username entry**: the avatar is initially hidden. Once the user enters a username and tabs/submits to the credential field, the server returns the avatar (if available) for that specific user. The avatar fades in with the entrance animation. To **prevent user enumeration**, the server always responds with a consistent delay whether or not the username exists — and returns either the real avatar or a generic initials fallback (indistinguishable from a user who simply has no avatar set).
- **Avatar entrance animation**: on reveal, the avatar fades in and gently scales from 0.9→1.0 (200ms, ease-out).

#### Username Input

- **Username field**: a glass-styled input field, horizontally centered, positioned above the credential input.
  - Width: 320px default (responsive, scales down on narrow viewports).
  - Height: 48px for comfortable touch interaction.
  - Same styling as the credential input: `var(--liquid-surface)` background, `backdrop-filter: blur(20px)`, pill-shaped border.
  - Placeholder text: "Username" with `var(--liquid-text-tertiary)` color.
  - Submit on Enter key moves focus to the credential input.
- **Pre-filled username flow**: when the client has a username from the connection profile, the username field is pre-filled and the login screen starts with focus on the credential input (password/PIN). The user can still edit the username field if needed.
- **Skip-to-credentials**: when the client connection profile provides both a username and `auto_fill = true`, the login screen can skip the username step entirely and show only the credential input — the username is displayed as read-only text above the avatar instead of an editable field.
- **No user list from server**: the server **never sends a list of available usernames** to the client. This prevents user enumeration attacks. The client always presents a blank username field (or pre-fills from its own saved profile).
- **Username submission**: when the user enters a username and presses Enter or Tab, the client sends the username to the server. The server responds with:
  - Available authentication methods for the login attempt (not per-user — the response is generic to avoid enumeration).
  - Avatar image (or a consistent generic fallback — see User Avatar above).
  - Whether a session is available for resume (or a generic "no session" response).
  - The server response timing and format are **identical** whether the username exists or not.

#### Greeting

- **Greeting**: a time-of-day greeting below the username. `font-size: 14px`, `color: var(--liquid-text-secondary)`.
  - 05:00–11:59 → "Good morning"
  - 12:00–16:59 → "Good afternoon"
  - 17:00–20:59 → "Good evening"
  - 21:00–04:59 → "Good night"
- Greeting is localized and can be disabled or replaced with a custom server message.
- The greeting is displayed from the start (it is not user-specific) — it uses the server's current time.

#### Credential Input Area

- **Password field**: a single glass-styled input field, horizontally centered.
  - Width: 320px default (responsive, scales down on narrow viewports).
  - Height: 48px for comfortable touch interaction.
  - Background: `var(--liquid-surface)` with `backdrop-filter: blur(20px)`.
  - Border: `1px solid var(--liquid-border)`, radius: `var(--liquid-radius-full)` (pill shape).
  - Placeholder text: "Password" with `var(--liquid-text-tertiary)` color.
  - **Focus state**: border transitions to accent color, subtle outer glow `box-shadow: 0 0 0 3px rgba(var(--liquid-accent-rgb), 0.25)`.
  - Eye icon (show/hide password toggle) inside the field at the right edge.
  - Submit on Enter key.
- **PIN mode**: when server requires PIN authentication, the input switches to a row of 4-8 individual digit boxes with glass styling. Digits auto-advance on input.
- **Smart card / security key prompt**: when these methods are active, the password field is replaced by an animated icon (pulsing card or key icon) with the text "Insert your smart card" or "Touch your security key". The icon pulses with a soft glow animation.
- **Biometric prompt**: when platform biometric is available, a fingerprint or face icon is shown below the password field as an alternative. Tapping it initiates the local biometric flow.

#### Authentication Method Indicators

- Below the credential input, a row of small icons indicates available authentication methods:
  - Password (key icon)
  - TOTP (phone icon)
  - FIDO2 (security key icon)
  - Smart card (card icon)
  - Biometric (fingerprint icon)
  - Certificate (shield icon)
- Only methods enabled by the server are shown.
- The currently active method is highlighted with the accent color; others are `var(--liquid-text-tertiary)`.
- Clicking an icon switches the credential input to that method.

#### Sign In Button

- **Primary action button**: glass-styled with accent color fill.
- Dimensions: pill-shaped, matching the input width (320px × 48px).
- Appears below the credential input.
- **Hover**: slightly brighter, subtle uplift shadow.
- **Active/pressed**: darkens slightly, shadow decreases.
- **Loading state**: button text replaced with a spinning glass ring (not a generic spinner — a translucent ring with a highlight arc that rotates).
- **Submit shortcut**: Enter key submits from the input field directly (button click is secondary).

#### Session Resume Indicator

- If the user has an existing session available for resume, a subtle indicator appears below the sign-in button:
  - Text: "Session available — sign in to resume" in `var(--liquid-text-secondary)`.
  - Optional: a small thumbnail of the last session state (blurred, low-res) shown inside a glass chip.
- If no previous session exists, this area is empty.

#### Error & Feedback States

- **Authentication failure**: the credential input shakes horizontally (3 quick oscillations, 300ms total) and the border briefly flashes `var(--liquid-danger)`. Error message appears below the input in `var(--liquid-danger)` text: "Incorrect username or password" (always generic — never reveals whether the username exists).
- **Account locked**: credential input is disabled. Message: "Account locked. Contact your administrator." with a lock icon.
- **MFA step**: after primary auth succeeds, the login card smoothly transforms — the password field cross-fades to the MFA input (TOTP code field, security key prompt, etc.) with a 200ms transition. The avatar and greeting remain, providing visual continuity.
- **Rate limiting**: after repeated failures, a countdown timer appears: "Try again in 45 seconds". The input field is disabled during the cooldown.
- **Network error**: status text below the input: "Cannot reach server — retrying..." with a subtle pulsing animation.

#### User Enumeration Prevention

- The server **never exposes a list of valid usernames** to unauthenticated clients. This is a deliberate security measure to prevent user enumeration attacks.
- All server responses during the login phase (avatar lookup, auth method query, session resume check) are **constant-time and indistinguishable** regardless of whether the submitted username exists.
- If a username does not exist, the server responds with:
  - A generic initials-based avatar (using the first letter of the submitted username).
  - The default set of authentication methods (same as a real user without custom overrides).
  - No session available for resume.
  - The same response delay as for a valid username.
- Authentication failure messages are generic: "Incorrect username or password" — never "User not found" or "Incorrect password" separately.

#### Server Information Strip

- Positioned at the **bottom-left** corner of the login screen.
- Displays: server hostname, server version, connection protocol indicator (QUIC/TLS icon).
- Styled in `var(--liquid-text-tertiary)`, `font-size: 12px` — present but unobtrusive.

#### Utility Controls (Bottom-Right)

- **Power menu** (if permitted by server policy): power icon that opens a small glass popover with options: restart session, shut down (if allowed).
- **Network indicator**: shows current connection quality (latency, protocol).
- **Accessibility toggle**: opens a quick-access panel with: high contrast toggle, large text toggle, on-screen keyboard toggle.
- **Language selector**: if multiple locales are configured, a language code chip (e.g., "EN") that opens a locale picker.
- All controls are glass-styled small icons that expand to popovers on click.

#### Animations

- **Screen activation**: wallpaper fades in (300ms), frosted overlay follows (200ms), then content elements cascade in top-to-bottom with 50ms stagger: clock → greeting → username input → credential input → button.
- **Username submitted**: after the user enters a username, the avatar fades in above the username field (200ms, ease-out with scale 0.9→1.0), and the credential input receives focus.
- **Auth success**: all elements gently fade out (200ms) while the desktop session fades in behind them. The transition feels like the login screen dissolves into the desktop.
- **Auth failure shake**: `transform: translateX` oscillation: 0 → -8px → 8px → -4px → 4px → 0 over 300ms.
- **All animations respect `prefers-reduced-motion`**: when enabled, transitions become instant cuts.

#### Accessibility

- **Full keyboard navigation**: Tab cycles through interactive elements (username input → credential input → sign-in button → utility controls). Focus ring clearly visible on all elements.
- **Screen reader support**: all elements have ARIA labels. Avatar announces "User: [name]". Auth method icons announce "Switch to [method] authentication".
- **High contrast mode**: glass effects are replaced with solid, high-contrast backgrounds. Input borders become thicker (2px). Colors follow WCAG AAA contrast ratios.
- **Large text mode**: all login screen text scales up proportionally. Input field height increases to 56px. Avatar size stays fixed.
- **On-screen keyboard**: can be activated from the accessibility controls for touch-only devices.

#### Login Screen Configuration

```toml
[login_screen]
# ─── Background ──────────────────────────────────────
wallpaper = "default"                       # default, custom, solid
custom_wallpaper = ""                       # path to custom login wallpaper
solid_color = "#1C1C2E"                     # used when wallpaper = "solid"
blur_intensity = 40                         # px, frosted glass blur over wallpaper
ambient_glow = true                         # radial glow behind avatar
particle_effect = false                     # floating translucent circles

# ─── Clock ───────────────────────────────────────────
show_clock = true
clock_format = "24h"                        # 24h, 12h
show_date = true
date_format = "long"                        # long ("Saturday, February 8"), short ("Feb 8")

# ─── User display ───────────────────────────────────
show_avatar = true
avatar_size = 120                           # px (64–160)
show_greeting = true
custom_greeting = ""                        # override time-of-day greeting
show_username_input = true                  # show username input field
username_input_placeholder = "Username"     # placeholder text for username field

# ─── Credential input ────────────────────────────────
default_auth_method = "password"            # password, pin, smartcard, fido2, certificate
show_auth_method_icons = true               # show alternative method icons
show_password_toggle = true                 # eye icon to reveal password
input_shape = "pill"                        # pill (rounded), rounded-rect
pin_length = 6                              # digits for PIN mode (4–8)

# ─── Session resume ──────────────────────────────────
show_resume_indicator = true
show_resume_thumbnail = false               # show blurred last-session preview

# ─── Server info ─────────────────────────────────────
show_server_info = true
show_connection_indicator = true

# ─── Utilities ───────────────────────────────────────
show_power_menu = false                     # requires explicit enable
show_accessibility_toggle = true
show_language_selector = "auto"             # auto (show if >1 locale), always, never

# ─── Branding ────────────────────────────────────────
custom_logo = ""                            # path to org logo (displayed above clock)
custom_banner_text = ""                     # legal/compliance banner at bottom
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

### WASM Plugin Security & Sandboxing

LiquidDE's WASM plugin system (§14b) introduces a controlled extension surface that must be secured against both malicious and buggy plugins. The following security properties are enforced:

#### Threat Model
- **Malicious plugin**: A plugin that attempts to exfiltrate data, escalate privileges, or disrupt the session.
- **Buggy plugin**: A plugin that has memory safety issues, infinite loops, or excessive resource consumption.
- **Supply chain attack**: A compromised plugin binary distributed through untrusted channels.

#### Memory Isolation
- Each WASM plugin runs in its own isolated linear memory space — it has **zero access** to host memory, other plugins' memory, or session state beyond what is explicitly passed through host function calls.
- Memory limit per plugin is enforced by wasmtime's memory limiter. Exceeding the limit triggers a WASM trap caught at the host boundary.
- Plugins cannot allocate memory beyond their declared maximum (`max_memory_mb` in plugin manifest, capped by `[plugins.resources] max_memory_limit_mb`).

#### CPU Quota Enforcement
- Each plugin call is metered using wasmtime's **fuel system**. A plugin call that exhausts its fuel allocation is terminated with a trap.
- A wall-clock timeout acts as a backstop for cases where fuel metering cannot catch (e.g., host function calls that block).
- Plugins that repeatedly exhaust CPU quotas are flagged and may be disabled by the watchdog.

#### Fault Boundary Guarantees
- WASM traps (out-of-bounds memory access, stack overflow, unreachable, division by zero) are **always** caught at the host boundary.
- A plugin trap **never** propagates to the session process — the plugin is marked as faulted, the session continues.
- Host function calls from plugins are validated: invalid arguments, out-of-range values, and excessive sizes are rejected with error codes, never causing host panics.

#### Plugin Signing & Verification
- Plugins can optionally be cryptographically signed (Ed25519).
- When `[plugins] signature_required = true`, unsigned or invalid-signature plugins are rejected at load time.
- Signature covers the WASM binary and the `plugin.toml` manifest.
- Key management: server administrator distributes the signing public key in server configuration.

#### Capability-Based Permissions
- Plugins declare required capabilities in their manifest (`[plugin.requirements] capabilities`).
- Host functions are gated by capability — a plugin that does not declare `clipboard` capability cannot call clipboard host functions.
- Available capabilities: `ui`, `session`, `notifications`, `clipboard`, `config`, `storage`, `timers`, `theme`, `ipc`, `input`.
- Administrator can further restrict capabilities per-plugin via server configuration.

#### Audit Logging
- Plugin load, unload, enable, disable, fault, and restart events are logged to the `plugin` log subsystem.
- Host function calls from plugins can optionally be logged at `debug` level for forensic analysis.
- Plugin-initiated actions that affect user-visible state (e.g., clipboard write, notification display) are logged at `info` level.

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

### Honeypot & Tarpit (Automatic)

LiquidDE can automatically detect **unambiguously malicious traffic** and respond with honeypot/tarpit tactics that waste attacker resources while gathering intelligence. Only patterns that have zero chance of being legitimate traffic trigger these mechanisms.

#### What Triggers Tarpit/Honeypot (Zero False-Positive Criteria)

These triggers are chosen because no legitimate client would ever produce them:

| Trigger | Why It's Safe | Response |
|---------|---------------|----------|
| **Invalid protocol magic bytes** | Legitimate clients always send the correct LiquidDE protocol header. Scanners (nmap, masscan, etc.) send HTTP, SSH, or random probes. | Tarpit: accept connection, respond very slowly with garbage data |
| **Known exploit signatures** | Pattern-matched against known RDP/VNC/SSH exploit payloads. No legitimate client sends CVE exploit code. | Honeypot: fake vulnerable service, log full payload |
| **Continued attempts after IP ban** | IP is already blocked by rate limiting. Legitimate users would stop or contact admin. Attackers use automation. | Tarpit: accept TCP, drip-feed data at 1 byte/sec |
| **Credential stuffing patterns** | Rapid sequential logins with different usernames from single IP. No human types that fast with different accounts. Threshold: >10 distinct usernames in 60 seconds. | Tarpit: simulate slow auth processing (5-30s delay), always reject |
| **Protocol downgrade attacks** | Attempts to force TLS 1.0/1.1 or null ciphers after server advertises minimum TLS 1.2+. | Tarpit: slow TLS handshake, then reject |
| **Port scan follow-up** | Connection arrives from an IP that probed 3+ closed ports within the last 60 seconds on this host. | Honeypot: fake open service, log all interaction |
| **Malformed packet floods** | >100 malformed packets per second from a single IP. No broken client produces sustained malformed traffic at this rate. | Tarpit: throttle to 1 response/sec, then blackhole |

#### What Does NOT Trigger Tarpit (Preserved Legitimate Behavior)

These are explicitly excluded to avoid false positives:

- **Wrong password** (user may have typo or changed password) — handled by normal rate limiting only.
- **Slow connections** (user may be on poor network) — never penalized for latency.
- **Expired/invalid certificates** (user may have stale config) — clear error, no tarpit.
- **Old client version** (user may not have updated) — version mismatch error, no tarpit.
- **Single failed auth then success** (common human pattern) — no penalty.
- **High bandwidth usage** (legitimate heavy session) — handled by QoS, not security.
- **Unusual connection times** (user may work odd hours) — no time-based suspicion.

#### Tarpit Behavior

When a tarpit is activated:
1. **TCP tarpit**: accept the connection but respond with artificially small TCP window sizes (1-10 bytes). Data drips out at 1 byte per second. Ties up attacker's connection slot and thread.
2. **TLS tarpit**: begin TLS handshake but send ServerHello parameters extremely slowly (one extension per second). May take 30-60 seconds for attacker to realize it's fake.
3. **Auth tarpit**: accept credentials, simulate "processing" with realistic timing jitter (5-30 seconds), then reject. Attacker cannot distinguish from a slow server.
4. **Bandwidth tarpit**: for ongoing connections, throttle to near-zero throughput with realistic TCP backpressure signals.
- Maximum concurrent tarpit connections: configurable (default: 100). Beyond this, new malicious connections are silently dropped (to prevent tarpit resource exhaustion).
- Tarpit connections are isolated in a dedicated thread pool (do not affect legitimate connection handling).

#### Honeypot Behavior

For confirmed malicious actors (post-ban, exploit attempts):
1. **Fake service emulation**: present a plausible-looking service that responds to protocol probes. Emulates just enough to keep scanners engaged.
2. **Intelligence gathering**: log all traffic from honeypot sessions — source IP, payloads, timing patterns, tool fingerprints — to a dedicated `honeypot.log`.
3. **Deception payloads**: for protocol probes, return plausible-looking but fake version strings and capability lists.
4. **Session recording**: full traffic capture of honeypot interactions for forensic analysis (stored separately, configurable retention).
- Honeypot is entirely passive — it never initiates outbound connections or retaliates.
- Honeypot data is never mixed with real audit logs (separate log stream).

#### Honeypot & Tarpit Configuration

```toml
[security.honeypot]
enabled = true
mode = "tarpit"                          # tarpit, honeypot, both, disabled

# Tarpit settings
tarpit_max_connections = 100             # max concurrent tarpit slots
tarpit_byte_rate = 1                     # bytes per second to drip
tarpit_tls_delay_ms = 1000              # ms between TLS handshake fragments
tarpit_auth_delay_sec = 15              # seconds to "process" fake auth
tarpit_thread_pool_size = 4             # dedicated threads for tarpit

# Honeypot settings
honeypot_log = "/var/log/liquidde/honeypot.log"
honeypot_capture_payloads = true         # capture full packet payloads
honeypot_max_capture_mb = 100           # max payload storage per day
honeypot_retention_days = 90            # how long to keep honeypot logs
honeypot_fake_version = ""              # empty = auto-generate plausible version

# Trigger thresholds (all require ZERO legitimate overlap)
trigger_on_invalid_protocol = true       # non-LiquidDE protocol magic
trigger_on_exploit_signatures = true     # known CVE exploit patterns
trigger_on_post_ban_attempts = true      # continued attempts after IP ban
trigger_on_credential_stuffing = true    # rapid multi-user auth attempts
credential_stuffing_threshold = 10       # distinct usernames per 60s
trigger_on_downgrade_attacks = true      # TLS downgrade attempts
trigger_on_port_scan_followup = true     # connection after port scanning
port_scan_probe_threshold = 3           # closed ports probed before trigger
trigger_on_malformed_floods = true       # sustained malformed packet floods
malformed_flood_threshold = 100          # packets per second

# Integration
notify_on_trigger = true                 # send alert to management API / webhook
webhook_url = ""                         # external notification endpoint
export_iocs = true                       # export indicators of compromise
ioc_export_format = "stix"              # stix, csv, json
```

### Workstation Lock & Timeout

LiquidDE provides extensive session lock and timeout controls for security and resource management. Lock policies can be configured globally, per-group, or per-user.

#### Lock Triggers

| Trigger | Description | Default |
|---------|-------------|---------|
| **Idle timeout** | No keyboard or mouse input for a configurable duration. | 15 minutes |
| **Disconnect lock** | Client disconnects (network drop, app close). | Immediate lock |
| **Schedule lock** | Time-of-day based lock (e.g., lock at 18:00 daily). | Disabled |
| **Manual lock** | User triggers lock via keyboard shortcut or DE menu. | Always available |
| **Admin lock** | Administrator locks session remotely via CLI or management UI. | Always available |
| **Policy lock** | External event triggers lock (e.g., PAM session event, LDAP group change). | Disabled |
| **Lid close** | If hardware lid sensor detected (e.g., laptop session). | Lock |
| **Screen blank** | Screen blanks after timeout (separate from lock). | 10 minutes |

#### Lock Actions

When a lock is triggered:
1. **Lock screen displayed**: glass-themed lock screen with user avatar, session info, and unlock prompt.
2. **Session preserved**: all applications continue running; no data loss.
3. **Screen content hidden**: frame buffer is replaced with lock screen; no leaking of session content.
4. **Input blocked**: all keyboard and mouse input rejected except unlock credentials.
5. **Active features paused**: clipboard sync paused, USB forwarding paused (configurable), audio playback muted (configurable).

#### Post-Lock Behavior (Escalation Chain)

After a session is locked, further timeouts can escalate the action:

```
Lock → (grace period) → Disconnect client → (background timeout) → Suspend session → (terminate timeout) → Terminate session
```

Each step is independently configurable:

| Stage | Description | Default |
|-------|-------------|---------|
| **Lock grace period** | Time after lock before client is forcibly disconnected. | Never (stay locked) |
| **Background timeout** | Time a locked+disconnected session runs in background before suspension. | 4 hours |
| **Suspend** | Session state serialized to disk, processes frozen (SIGSTOP + cgroup freeze). RAM reclaimed. | Disabled |
| **Terminate timeout** | Time a suspended (or background) session persists before termination. | 24 hours |
| **Terminate** | Session killed, processes terminated, temporary files cleaned. | Only on timeout |

#### Screen Blank (Pre-Lock)

- Screen blank is separate from lock — the screen goes dark but the session is not locked.
- Any input wakes the screen immediately (no authentication required).
- Useful for power saving and burn-in prevention without the friction of re-authenticating.
- Screen blank timeout is always shorter than or equal to lock timeout.

#### Lock Screen Customization

- **Lock screen wallpaper**: separate from desktop wallpaper (or same, configurable).
- **Lock screen message**: administrator-defined message displayed on lock screen (e.g., "Contact IT at x1234").
- **Lock screen clock**: display current time and date.
- **Session info**: show session duration, username, server name.
- **Auth method indicator**: show which authentication method is required to unlock (password, fingerprint, smart card).
- **Graceful unlock**: when unlocked, session is immediately visible without re-rendering (frame buffer swap).

#### Lock Configuration

```toml
[session.lock]
# ─── Idle lock ──────────────────────────────────────────────
idle_lock_enabled = true
idle_timeout_min = 15                    # minutes of no input before lock
idle_detection = "keyboard+mouse"        # keyboard+mouse, keyboard, mouse, any-input
ignore_audio_activity = true             # audio playback does not reset idle timer
ignore_clipboard_activity = true         # clipboard events do not reset idle timer

# ─── Screen blank ───────────────────────────────────────────
screen_blank_enabled = true
screen_blank_timeout_min = 10            # minutes before screen blanks (≤ idle_timeout)
screen_blank_on_lock = true              # also blank screen when locked

# ─── Disconnect behavior ────────────────────────────────────
disconnect_action = "lock"               # lock, keep-unlocked, terminate
disconnect_grace_sec = 0                 # seconds to wait before applying action (0 = immediate)

# ─── Post-lock escalation ──────────────────────────────────
lock_grace_min = 0                       # 0 = stay locked indefinitely; >0 = disconnect after N min
background_timeout_min = 240             # locked+disconnected session background time (0 = infinite)
suspend_enabled = false                  # serialize session to disk after background timeout
suspend_timeout_min = 1440               # time before suspended session is terminated (0 = infinite)
terminate_after_total_min = 0            # hard cap: terminate regardless of state (0 = disabled)

# ─── Schedule lock ──────────────────────────────────────────
schedule_lock_enabled = false
schedule_lock_cron = "0 18 * * *"        # cron expression for scheduled lock
schedule_unlock_cron = ""                # optional auto-unlock (empty = manual unlock only)
schedule_timezone = "UTC"

# ─── Lock screen appearance ─────────────────────────────────
lock_screen_wallpaper = "desktop"        # desktop (same as session), blur (blurred desktop), custom, solid
lock_screen_custom_wallpaper = ""        # path to custom lock screen image
lock_screen_message = ""                 # admin-defined message
lock_screen_show_clock = true
lock_screen_show_session_info = true
lock_screen_show_user_avatar = true

# ─── Feature behavior during lock ───────────────────────────
lock_pause_clipboard = true              # pause clipboard sync while locked
lock_pause_usb = false                   # pause USB forwarding while locked
lock_mute_audio = false                  # mute audio playback while locked
lock_pause_camera = true                 # pause camera forwarding while locked
```

#### Lock Policies (Per-Group / Per-User)

Lock settings can be overridden in `policies.toml`:

```toml
[default.lock]
idle_timeout_min = 15
disconnect_action = "lock"
background_timeout_min = 240

[group.contractors.lock]
idle_timeout_min = 5                     # stricter for contractors
background_timeout_min = 30              # shorter background time
terminate_after_total_min = 60           # hard terminate after 1 hour

[group.kiosk.lock]
idle_timeout_min = 2
disconnect_action = "terminate"          # terminate immediately on disconnect
schedule_lock_enabled = true
schedule_lock_cron = "0 22 * * *"        # lock nightly at 10 PM

[user.alice.lock]
idle_timeout_min = 30                    # alice gets a longer timeout
background_timeout_min = 480             # 8 hours background
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

# ─── Appearance ──────────────────────────────────────────────
[appearance]
default_theme = "liquid-glass"        # liquid-glass, night, sunset, midday, custom
theme_dir = "/etc/liquidde/themes"    # system theme directory
allow_user_themes = true              # allow users to override theme
wallpaper_dir = "/etc/liquidde/wallpapers"

# ─── Internationalization ────────────────────────────────────
[i18n]
default_locale = "en-US"
available_locales = ["en-US", "en-GB", "de-DE", "fr-FR", "ja-JP", "zh-CN", "ko-KR", "es-ES", "pt-BR", "ru-RU", "ar-SA", "hi-IN"]
fallback_locale = "en-US"
message_dir = "/etc/liquidde/i18n"
allow_user_translations = true
keyboard_layout_dir = "/etc/liquidde/xkb"

# ─── Avatar ──────────────────────────────────────────────────
[avatar]
enabled = true
max_upload_size_bytes = 2097152       # 2 MB
stored_size = 256                     # px, square
allowed_formats = ["png", "jpeg", "webp", "svg"]
svg_sanitize = true                    # strip scripts, external refs, embedded objects from SVG uploads
generate_initials_fallback = true

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
window_offload = "none"              # none, terminal, all-text-windows
terminal_offload_mode = "state"      # state (character grid), structured (text runs + layout)
terminal_scrollback_sync = 1000      # max scrollback lines synced to client initially
window_offload_apps = []             # app_ids eligible for window-level offload (empty = auto-detect)

# ─── Seamless Windows ────────────────────────────────────────
[seamless]
enabled = false                       # enable seamless/detached window mode
allow_client_request = true           # allow clients to request seamless mode
per_window_encoding = true            # encode each window independently
sync_z_order = true                   # sync window z-order to client
sync_taskbar_entries = true           # expose remote windows to client taskbar/dock
shell_as_window = false               # present dock/statusbar as separate native windows
excluded_app_ids = ["liquidde-desktop"]  # apps that cannot be detached

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

[security.honeypot]
enabled = true
mode = "tarpit"                          # tarpit, honeypot, both, disabled
tarpit_max_connections = 100
tarpit_auth_delay_sec = 15
trigger_on_invalid_protocol = true
trigger_on_exploit_signatures = true
trigger_on_post_ban_attempts = true
trigger_on_credential_stuffing = true
honeypot_log = "/var/log/liquidde/honeypot.log"

# ─── Session Lock ─────────────────────────────────────────
[session.lock]
idle_lock_enabled = true
idle_timeout_min = 15
screen_blank_enabled = true
screen_blank_timeout_min = 10
disconnect_action = "lock"
background_timeout_min = 240
suspend_enabled = false
terminate_after_total_min = 0            # 0 = disabled
schedule_lock_enabled = false
lock_screen_wallpaper = "blur"
lock_screen_message = ""
lock_pause_clipboard = true
lock_pause_camera = true

# ─── Plugins (WASM Extension System) ──────────────────────────
[plugins]
enabled = true                           # enable the WASM plugin system
plugin_dirs = ["/etc/liquidde/plugins", "~/.config/liquidde/plugins"]
signature_required = false               # require cryptographic signature on plugins
signature_public_key = ""                # path to public key for plugin verification
hot_reload = true                        # watch plugin directories for changes
max_plugins_per_session = 20             # maximum loaded plugins per session
abi_versions = ["v1"]                    # supported ABI versions

[plugins.resources]
default_memory_limit_mb = 32             # per-plugin WASM linear memory cap
default_cpu_fuel = 50_000_000            # fuel units per plugin call (~50ms CPU equivalent)
default_wall_timeout_ms = 250            # wall-clock timeout per call (backstop)
max_memory_limit_mb = 256                # absolute maximum any plugin can request
max_cpu_fuel = 500_000_000               # absolute maximum fuel any plugin can request
watchdog_interval_ms = 1000              # how often to check plugin health
max_restarts = 5                         # max auto-restarts before permanent disable
restart_backoff_base_ms = 1000           # exponential backoff base for restarts
restart_window_sec = 600                 # restart counter resets after this window

# ─── Session Supervisor ────────────────────────────────────────
[supervisor]
enabled = true                           # enable supervisor process model
heartbeat_interval_ms = 5000             # session→supervisor heartbeat frequency
heartbeat_timeout_count = 3              # missed heartbeats before declaring hang
max_restarts = 5                         # max session restarts within window
restart_window_sec = 600                 # restart counter reset window (10 min)
restart_backoff_base_ms = 1000           # exponential backoff base
coredump_enabled = true                  # capture coredumps on crash
coredump_max_size_mb = 512               # max coredump file size
crash_log_lines = 100                    # number of session log lines to capture

# ─── Crash Screen ──────────────────────────────────────────────
[crash_screen]
crash_report_dir = "/var/log/liquidde/crashes"
crash_report_retention_days = 30         # auto-delete old crash reports
crash_report_max_count = 1000            # max stored crash reports
telemetry_upload_enabled = false         # upload crash reports to endpoint
telemetry_upload_url = ""                # crash telemetry endpoint URL
include_coredump_in_report = false       # include coredump path in client-visible report

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
plugin = "info"
supervisor = "info"
crash = "warn"

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
plugins_enabled = true                   # allow WASM plugins for sessions in this policy
allowed_plugins = []                     # empty = all installed plugins allowed; list plugin IDs to restrict
plugin_install = "admin-only"            # admin-only, user, disabled

# Group overrides
[group.developers]
clipboard = "bidirectional"
file_transfer = true
max_sessions = 5
plugin_install = "user"                  # developers can install their own plugins

[group.guests]
clipboard = "server-to-client"
file_transfer = false
usb_redirection = false
max_resolution = "1920x1080"
max_fps = 30
plugins_enabled = false                  # guests cannot use plugins

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
   - Keyboard layout selection and switching configuration.
   - Language & regional format settings (locale, date/time format, number format).
   - Theme preset selector (Standard, Night, Sunset, Midday) with live preview.
   - Theme/CSS editor with live preview.
   - Dock configuration.
   - User profile editor (display name, avatar upload/crop/remove).

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
- WASM plugin system (runtime, ABI v1, 9 extension points).
- Plugin SDK and documentation.
- Session supervisor process model.
- BSOD crash screen (client-rendered).

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
- **Plugin isolation**: WASM plugin faults never crash the host session (see §14b).
- **Session isolation**: individual session process crashes never bring down the server daemon (see §13).

### Crash Categories

| Category | Severity | Detection | Automatic Response |
|----------|----------|-----------|-------------------|
| Worker thread hang | Low | Deadline exceeded (orchestrator health monitor) | Kill and restart worker thread |
| Worker thread panic | Low | Rust panic hook in worker | Catch at thread boundary, restart worker |
| WASM plugin fault | Low | Trap at WASM sandbox boundary | Disable plugin, notify user, session continues |
| Plugin resource exhaustion | Low | Fuel exhaustion / memory limit | Trap and terminate plugin call, session continues |
| Session process crash | Medium | Supervisor detects child exit (non-zero status) | Capture crash context, notify client, attempt restart |
| Session process OOM | Medium | cgroup OOM killer / supervisor watchdog | Capture context, notify client, restart with reduced limits |
| Session process hang | Medium | Heartbeat timeout (supervisor IPC) | SIGKILL after grace period, restart |
| Resource exhaustion (FDs, disk) | Medium | Periodic resource checks in supervisor | Log warning, attempt cleanup, restart session if unrecoverable |
| GPU driver fault | Medium | Render worker crash isolation | Fall back to CPU rendering, notify user |
| Server daemon crash | Critical | systemd watchdog / process exit | systemd restarts daemon, clients see "Server Unreachable" crash screen |
| Hardware fault | Critical | Kernel panic / hardware error | Out of scope — handled by OS |

### Session Supervisor Behavior

The `liquid-desktopd` daemon acts as a supervisor for `liquid-session` child processes (see §13 Session Supervisor & Process Model). When a session process crashes:

1. **Detection**: Supervisor receives `SIGCHLD` or heartbeat timeout.
2. **Classification**: Exit code, signal number, and coredump presence determine crash category.
3. **Context capture**:
   - Coredump (if enabled and available).
   - Last 100 log lines from session log.
   - Session metadata (user, uptime, last active window, resource usage).
   - Stack trace from coredump (symbolized if debug info available).
4. **Client notification**: Supervisor sends a `crash_info` message to the client via the transport connection (or via a dedicated supervisor-to-client notification channel if the session transport is lost).
5. **Restart policy**:
   - Immediate restart on first crash.
   - Exponential backoff on subsequent crashes: 1s, 2s, 4s, 8s, 16s, 30s max.
   - Maximum 5 restarts within a 10-minute window.
   - After exhaustion: session enters `failed` state, client shows persistent crash screen.
   - Admin can force restart via `liquidctl supervisor restart <session-id>`.
6. **Crash report**: Generated and stored in `/var/log/liquidde/crashes/crash-<session-id>-<timestamp>.json`.

### Crash Detection & Classification

- **Signal-based**: `SIGSEGV`, `SIGABRT`, `SIGBUS`, `SIGFPE`, `SIGILL` → session process crash.
- **Heartbeat timeout**: Session sends heartbeat to supervisor every 5 seconds. Miss 3 consecutive → supervisor assumes hang.
- **OOM killer**: cgroup v2 `memory.events` monitored for OOM events.
- **Rust panic handler**: Custom panic hook in `liquid-session` that logs panic info, flushes logs, and exits with a distinguishable exit code (exit code 101).
- **Resource exhaustion**: File descriptor count, /tmp disk usage, and thread count monitored periodically.

### BSOD-Like Crash Screen

When a fatal error prevents session continuation, the **client** renders a full-screen crash screen locally (not streamed as video frames — similar to the login screen rendering approach).

#### Crash Screen Data Format

The supervisor (or the session process's panic handler, if it can still communicate) sends a `crash_info` message to the client:

```json
{
  "type": "crash_info",
  "error_code": "SESSION_PROCESS_CRASH",
  "signal": 11,
  "description": "Session process terminated unexpectedly (SIGSEGV)",
  "stack_trace": [
    "liquid_session::compositor::render_frame+0x1a4",
    "liquid_session::compositor::compose_surfaces+0x892",
    "liquid_session::main_loop::tick+0x3f1"
  ],
  "session_id": "s-001",
  "user": "alice",
  "uptime_seconds": 8142,
  "timestamp": "2025-01-15T16:22:31.847Z",
  "server_version": "0.1.0",
  "crash_report_id": "crash-s001-20250115T162231",
  "recovery_options": ["restart_session", "download_report", "disconnect"],
  "restart_available": true
}
```

#### Crash Screen Visual Layout

```
┌─────────────────────────────────────────────────────────────┐
│                    [frosted dark-red glass]                  │
│                                                             │
│                       ⚠ (error icon)                        │
│                                                             │
│               SESSION_PROCESS_CRASH                         │
│                                                             │
│     Session process terminated unexpectedly (SIGSEGV)       │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  liquid_session::compositor::render_frame+0x1a4     │    │
│  │  liquid_session::compositor::compose_surfaces+0x892 │    │
│  │  liquid_session::main_loop::tick+0x3f1              │    │
│  └─────────────────────────────────────────────────────┘    │
│                                                             │
│     Session: s-001 · User: alice · Uptime: 2h 15m 42s      │
│     2025-01-15 16:22:31 UTC                                 │
│                                                             │
│   [ Restart Session ]   [ Download Report ]   [ Disconnect ]│
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

The crash screen is rendered using the Liquid Glass design language (see [spec-design.md](spec-design.md) §7.14 for full CSS specification). Three visual variants exist based on crash type:
- **Session crash** (`.type-session`) — red accent.
- **Connection fatal** (`.type-connection`) — amber accent.
- **Server unreachable** (`.type-server`) — dark red accent.

#### Recovery Options

| Action | Behavior |
|--------|----------|
| **Restart Session** | Client sends restart request to supervisor. Shows loading spinner. On success, crash screen dissolves into resumed session. On failure (restart limit exhausted), shows "session could not be restarted" message. |
| **Download Report** | Generates a crash report file (JSON) and offers it for download/save. Report includes: error code, stack trace, session metadata, system info, last 100 log lines. Sanitized — no screen content, no user files, no credentials. |
| **Disconnect** | Returns to the client connection dialog. Session remains in `failed` state on server until admin intervenes or TTL expires. |

### Crash Reporting & Telemetry

Crash reports are stored server-side in `/var/log/liquidde/crashes/`:

```json
{
  "crash_id": "crash-s001-20250115T162231",
  "timestamp": "2025-01-15T16:22:31.847Z",
  "error_code": "SESSION_PROCESS_CRASH",
  "signal": 11,
  "exit_code": null,
  "stack_trace": ["..."],
  "session_id": "s-001",
  "user": "alice",
  "uptime_seconds": 8142,
  "system_info": {
    "server_version": "0.1.0",
    "os": "Ubuntu 24.04",
    "kernel": "6.8.0-generic",
    "cpu": "AMD EPYC 7763",
    "memory_total_mb": 8192,
    "memory_used_mb": 6144
  },
  "last_log_lines": ["...last 100 lines from session log..."],
  "active_plugins": ["widget-clock", "clipboard-sanitizer"],
  "coredump_path": "/var/log/liquidde/crashes/core.s001.20250115T162231"
}
```

- **Retention**: Configurable, default 30 days.
- **Prometheus metrics**: `liquidde_crashes_total{type="session_crash"}`, `liquidde_crash_restarts_total`, `liquidde_session_uptime_at_crash_seconds` (histogram).
- **Optional telemetry upload**: Disabled by default. If enabled, crash reports (without coredumps or user data) are sent to a configurable endpoint.

### Crash Screen Rendering

The crash screen is rendered **client-side** using the same infrastructure as the login screen:

- **Normal path**: Client GPU renders the crash screen using the Liquid Glass CSS theme. Full glass effects, animations, and theme colors.
- **Emergency path**: If the client rendering engine itself is compromised, a software-rendered fallback activates:
  - Solid dark red background (`#1A0808`), no glass effects.
  - System monospace font, white text.
  - Minimal layout: error code, description, and "Press Enter to disconnect" prompt.
  - No animations, no blur, no network-dependent resources.

The crash screen is **never** streamed as encoded video frames from the server. The client has all the information it needs from the `crash_info` message to render it locally.

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
  - Plugin development guide.

- **Plugin SDK**:
  - `liquidde-plugin-sdk` — Rust crate for developing WASM plugins.
  - Sample plugins (status bar widget, clipboard transformer, theme generator).

- **Crash Reporting**:
  - Crash report format specification.
  - Crash collection and analysis tooling.

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
