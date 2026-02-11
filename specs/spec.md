# LiquiDE Server & Desktop Environment — Full Specification

> **Language**: Rust
> **License**: MIT
> **Related specs**: [Client](spec-client.md) · [Web Client](spec-web-client.md) · [Mobile Client](spec-mobile.md) · [Gateway](spec-gateway.md) · [Management UI](spec-manager.md) · [liquidctl CLI](spec-liquidctl.md) · [Design Language](spec-design.md) · [Protocol](spec-protocol-formal.md) · [Rendering](spec-rendering-software.md) · [Performance](spec-performance.md) · [Observability](spec-observability.md) · [Build](spec-build.md) · [Threat Model](spec-threat-model.md) · [Normative](spec-normative.md) · [Night Theme](spec-theme-night.md) · [Sunset Theme](spec-theme-sunset.md) · [Midday Theme](spec-theme-midday.md)

---

## 0) Concept

A **remote-first, native desktop environment** built entirely in Rust, designed specifically for remote desktop use-cases:

- **Runs well with *no GPU*** (headless servers, VMs, iGPU-less boxes, cloud instances) and **offers a first-class GPU Server Mode** with Vulkan compositing, hardware encoding, VRAM budgeting, and GPU sharing support (vGPU, MIG, MPS) when GPUs are available.
- **Feels like a modern "liquid glass" OS** (depth, translucency, blur, vibrancy) while remaining bandwidth/CPU efficient.
- **Users customize the DE appearance with CSS** — every visual element is themeable through a well-documented CSS system.
- **Multi-threaded architecture** where the main thread is strictly an orchestrator — all heavy work (rendering, encoding, I/O, effects) runs on dedicated worker threads to prevent hangs.
- **Implements aggressive performance optimizations end-to-end** (render → capture → encode → transport → decode → present).
- Works as a **complete desktop session** (shell + compositor + core apps + dock), not a "screenshare your existing GNOME/KDE" solution.
- **Reimplements the full graphics pipeline** — no dependency on existing compositors or display servers.
- **Multi-platform server**: x86_64 and ARM64 (Linux), with ARM64 support for macOS-hosted VMs and ARM Linux boards.
- Supports **multiple transport strategies** including QUIC, UDP, TCP, TLS, switchable and hybridizable on the fly.

Working name: **LiquiDE** (server) + **LiquidClient** (client, see [spec-client.md](spec-client.md)).

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

LiquiDE is built from cleanly separated layers, all in Rust, with a strict multi-threaded design.

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

The main thread **never** performs blocking work. It runs an async event loop (tokio) that dispatches work items to worker tasks via channels. Workers are implemented as **structured async tasks** (tokio tasks), not OS threads — this enables cooperative cancellation via `CancellationToken`. If any worker task exceeds a deadline, the orchestrator cancels its token and spawns a replacement task. For CPU-bound workers (render, encode), work is dispatched to a dedicated `tokio::task::spawn_blocking` thread pool; the orchestrator monitors liveness via heartbeat channels and drops the work handle on timeout — the blocking thread completes its current unit of work and then exits when it finds its channel closed. This is a **crash-only worker** model: workers are never forcibly killed mid-operation; they are canceled at natural yield points and allowed to clean up. If a blocking worker truly hangs (no yield within 2× the deadline), the session process itself is considered hung and the supervisor terminates it via `SIGKILL` (see §13).

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

#### XWayland Support Policy

XWayland enables legacy X11 applications (e.g., older GTK2 apps, Wine/Proton, some Java apps, many Electron apps with `--ozone-platform=x11`) to run inside the Wayland compositor. LiquiDE takes a **pragmatic but bounded** approach to XWayland:

**Supported (Tier 1 — tested, bugs are P1):**

| Feature | Notes |
|---------|-------|
| Basic window management | Map, unmap, resize, move, focus, close |
| Clipboard (text, images) | Uses Wayland clipboard protocol with XWayland bridge (`wl_data_device` ↔ X11 selection) |
| Keyboard input (standard layouts) | Via `wl_seat` keyboard → X11 `XKeyEvent` |
| Mouse input (absolute, buttons, scroll) | Via `wl_seat` pointer → X11 `XMotionEvent`, `XButtonEvent` |
| Window decorations | Server-side decorations rendered by the compositor (X11 apps get CSD=off) |
| Drag-and-drop (within X11 apps) | XDND protocol handled internally by XWayland |
| Window type mapping | `_NET_WM_WINDOW_TYPE` → `xdg_toplevel` / `xdg_popup` mapping |

**Supported (Tier 2 — best-effort, bugs are P2):**

| Feature | Notes |
|---------|-------|
| Drag-and-drop (X11 ↔ Wayland) | XDND ↔ `wl_data_device` bridge. Known edge cases with complex MIME types. |
| Multi-monitor (X11 app spanning monitors) | X11 apps see a single logical screen. `_NET_WM_FULLSCREEN_MONITORS` not supported. |
| X11 selections (PRIMARY, SECONDARY) | PRIMARY selection (middle-click paste) forwarded. SECONDARY not supported. |
| Custom cursors (X11 `XDefineCursor`) | Converted to Wayland cursor. Performance may degrade with frequent changes. |
| `_NET_WM_STRUT` (panel reservation) | Mapped to `layer_shell` exclusive zones with best-effort positioning. |

**Not supported (by design):**

| Feature | Reason | Alternative |
|---------|--------|-------------|
| Direct X11 rendering (`MIT-SHM`, DRI2/DRI3) | Requires GPU and breaks remote model | Use Wayland-native rendering path (XWayland uses `wl_shm` for buffer submission) |
| X11 screen capture (`XGetImage`, `XShm`) | Security: applications should not capture other windows | Use portal-based screenshot API (`xdg-desktop-portal`) |
| `XTEST` extension | Security: synthetic input injection | Not available (applications that require it won't work) |
| `Xrandr` for display configuration | Confusing with remote display management | Compositor manages display layout; X11 apps see the result |
| Complex X11 visuals (depth 8, 16) | Only 32-bit (ARGB) output supported | Apps must handle truecolor |

**Known hard edges in XWayland under remote desktop:**

1. **Popup positioning**: X11 popups use absolute screen coordinates (`OverrideRedirect` windows). In a remote session, the "screen" is the remote virtual display, not the client's monitor. Popups that appear near screen edges may be clipped if the remote display is larger than the client viewport. Mitigation: the compositor applies `xdg_popup.constraint_adjustment` rules to XWayland popups, clamping them to visible area.
2. **Focus stealing**: X11 has no focus-stealing prevention by default. LiquiDE applies the same `xdg-activation-v1` rules to XWayland windows — focus requests from background X11 apps are denied and trigger an urgent flag on the taskbar instead.
3. **Clipboard MIME mismatch**: X11 uses `TARGETS` atom to advertise clipboard formats, while Wayland uses MIME strings. The XWayland bridge maps common formats (UTF8_STRING → `text/plain;charset=utf-8`, PIXMAP → `image/png`) but obscure X11-specific targets may not translate. Unknown targets are dropped with a debug log.

### Known Implementation Hard Edges

This section documents non-obvious implementation challenges that affect correctness and interoperability. Implementors must handle these explicitly.

#### Popup & Transient Window Positioning

Popups (menus, tooltips, dropdowns, combobox lists, autocomplete) are the most fragile category of windows in a remote desktop:

| Problem | Cause | Mitigation |
|---------|-------|------------|
| **Popup clipped at viewport edge** | Remote Wayland popup anchored near screen edge. Client viewport may be smaller or scrolled. | Compositor applies `xdg_positioner.constraint_adjustment` (flip, slide, resize). Client-side: smooth scroll to reveal popup if partially offscreen. |
| **Popup placed on wrong monitor** | Multi-monitor with different scales. Popup anchor resolved on server coordinates, but client monitor layout differs. | Server sends popup geometry in logical coordinates. Client maps to local monitor. If mapping fails, popup placed at cursor position as fallback. |
| **Fractional-scale coordinate rounding** | At 125% or 150% scale, popup anchor coordinates may round to off-by-one pixel positions, causing visual misalignment. | All anchor calculations use fixed-point arithmetic internally (24.8 format). Only the final render step rounds to device pixels. Gap fills use subpixel anti-aliasing. |
| **Popup z-order vs. native windows** | In seamless mode, remote popups must appear above local windows that are behind the parent remote window. | Popup windows created as child windows (`WS_POPUP` on Windows, `NSPanel` with `floatingPanel` on macOS) with `level` above the parent window. |

#### Drag-and-Drop Correctness

Inter-application DnD (especially cross-toolkit: GTK ↔ Qt ↔ X11 ↔ native client) has known sharp edges:

| Problem | Cause | Mitigation |
|---------|-------|------------|
| **MIME type mismatch** | GTK offers `text/uri-list`, Qt offers `application/x-qt-mime-type-name`. | Normalize to canonical MIME types on the wire. Server-side adapter maps toolkit-specific types to standard MIME: `text/uri-list` for file paths, `text/plain` for text, `image/png` for images. Unknown types passed through unchanged. |
| **DnD across XWayland boundary** | Drag from Wayland app to X11 app (or vice versa). XDND ↔ `wl_data_device` bridge must translate formats and coordinates. | XWayland bridge handles XDND ↔ Wayland DnD conversion. Known issue: drag cursors may flicker during the transition. |
| **DnD to local client in seamless mode** | User drops file from remote window onto local desktop. | Client converts `text/uri-list` (with remote `file://` URIs) into file transfer requests. Transfer happens asynchronously — user sees a progress indicator. |
| **DnD cancel/escape** | Drag started but user presses Escape or moves outside valid drop target. Race condition between input event and DnD state machine. | Both client and server implement a 100ms timeout on DnD state transitions. If a drag enters an ambiguous state, it is force-cancelled with `wl_data_source.cancelled` / `DnDCancel` on the wire. |

#### Clipboard MIME Correctness

| Problem | Cause | Mitigation |
|---------|-------|------------|
| **`text/html` with embedded images** | HTML clipboard data may reference `cid:` or `data:` URIs for inline images. Some apps embed base64 images; others expect external resources. | Server sanitizes HTML: convert `cid:` references to inline `data:` URIs. Strip external resource references (security). Warn if sanitized HTML exceeds max clipboard size. |
| **Incremental transfer stalls** | Large clipboard items (multi-MB images) transferred in chunks. Slow network causes UI freeze if clipboard paste blocks on completion. | Non-blocking clipboard: paste immediately inserts placeholder ("Loading clipboard...") and updates async when data arrives. Text paste is always immediate (buffered). Image paste shows progressive loading. |
| **MIME negotiation failure** | Client requests MIME type that server doesn't support. Or server offers format that no client app can handle. | Server always offers `text/plain;charset=utf-8` as a fallback for any text-containing clipboard item. For images, always include `image/png` alongside native formats. |
| **Platform-specific clipboard quirks** | macOS `NSPasteboardType` names differ from MIME strings. Windows uses `CF_*` clipboard format IDs. | Client-side adapter between platform clipboard API and MIME-based wire protocol. Maps: `NSPasteboardTypeString` ↔ `text/plain`, `CF_UNICODETEXT` ↔ `text/plain;charset=utf-8`, `CF_DIB` ↔ `image/bmp` (converted to `image/png` on wire). |

#### IME Composition Across the Network

| Problem | Cause | Mitigation |
|---------|-------|------------|
| **Composition state appears stale** | Network latency means the preedit string shown on the server is one RTT behind the user's typing. Fast typists see their composition "lag". | Client-side composition: the client renders preedit text locally (overlay on the session view) and only sends committed text to the server. The server's inline preedit is used as confirmation — if it diverges from client-side prediction, the client re-syncs. This is opt-in (default: server-side composition, which is simpler but laggier). |
| **IME candidate window positioning** | Candidate window is rendered by the server compositor. Position is based on cursor rectangle sent by the focused app. In a remote session, the candidate window must appear near the cursor ON THE CLIENT. | Server sends candidate window as a popup surface with anchor coordinates. Client places it at the corresponding local position (accounting for viewport zoom/scroll). If client-side composition is active, the client renders its own candidate window locally. |
| **CJK on non-CJK client platform** | Windows/macOS client connecting to a Linux server with IBus/Fcitx5. Client OS keyboard does not have CJK layout. | The remote IME runs on the server. Client forwards raw key events (physical scancodes). The server's IME interprets them according to the server-side layout. This works but the client's input language indicator may be misleading — the toolbar shows the server's active IME. |
| **Dead key conflicts** | Client OS intercepts dead keys (e.g., macOS Option+e for acute accent). These are consumed locally and never reach the server. | Client sends `key_dead` protocol event when a dead key is consumed locally, so the server knows the next character will be composed. Alternatively, user can enable "raw key passthrough" mode (keyboard lock) where all keys are forwarded to the server. |

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

### GPU Server Mode (First-Class Profile)

While LiquiDE's CPU-only path is the primary deployment target, **GPU Server Mode** elevates GPU-accelerated rendering and encoding to a first-class server profile — not just "optional acceleration" but a fully distinct operating mode with its own resource model, scheduling, quality targets, and administrative tools.

GPU Server Mode is designed for:
- **VDI at scale with GPUs** — cloud VMs with vGPU (NVIDIA GRID, Intel GVT-g, AMD MxGPU) or dedicated GPU pools.
- **Power users** — CAD, 3D modeling, visualization, video editing workloads where GPU acceleration is essential.
- **High-density deployments** — GPU sharing allows more sessions per physical GPU with better quality than CPU-only.
- **Low-latency scenarios** — GPU compositing + hardware encoding eliminates the CPU bottleneck for frame delivery.

#### GPU Detection & Capability Probing

On session start, the server probes GPU availability:

```
┌─────────────────────────────────────────┐
│  GPU Probe Sequence                      │
│                                          │
│  1. Enumerate Vulkan physical devices    │
│  2. Check device type, VRAM, driver ver  │
│  3. Probe hardware encoder support:      │
│     - VAAPI (Intel, AMD via Mesa)        │
│     - NVENC (NVIDIA)                     │
│     - AMF (AMD via AMVF)                 │
│  4. Check Vulkan compute capability      │
│  5. Detect GPU sharing technology:       │
│     - SR-IOV / vGPU                      │
│     - MPS (NVIDIA Multi-Process Service) │
│     - MIG (NVIDIA Multi-Instance GPU)    │
│     - Time-sliced sharing                │
│  6. Measure available VRAM               │
│  7. Run micro-benchmark (optional):      │
│     - Blur throughput (Vulkan compute)    │
│     - Encode throughput (HW encoder)     │
│  8. Select GPU profile                   │
└─────────────────────────────────────────┘
```

| Detection Result | Selected Profile | Behavior |
|-----------------|-----------------|----------|
| No GPU / no Vulkan | `cpu-only` | Full software rendering, software encoding |
| GPU present, no HW encoder | `gpu-composite` | Vulkan compositing + blur, software encoding |
| GPU present, HW encoder | `gpu-full` | Vulkan compositing + blur, hardware encoding |
| vGPU / SR-IOV instance | `gpu-shared` | GPU-full with VRAM budgeting, encoder sharing |
| Dedicated GPU per session | `gpu-dedicated` | GPU-full, no sharing constraints |

#### GPU Resource Model

##### Per-Session GPU Budget

Each session in GPU mode is assigned a GPU resource budget:

| Resource | Default Budget | Hard Limit | Enforcement |
|----------|---------------|-----------|-------------|
| VRAM | 256 MB | Configurable | Vulkan allocation tracking + fallback to system RAM overflow |
| Encoder slots | 1 concurrent encode stream | 4 | Hardware encoder session limit |
| Compute time | Fair-share scheduling | 50% of GPU per session (time-slice) | Vulkan timeline semaphores + driver scheduling |
| Video decode (if server-side) | 1 decode stream | 2 | Hardware decoder session limit |

##### GPU Sharing Technologies

| Technology | Vendor | Isolation | VRAM Partitioning | Session Density | Latency Overhead |
|-----------|--------|-----------|-------------------|-----------------|-----------------|
| **SR-IOV / vGPU** | NVIDIA (GRID), Intel (GVT-g), AMD (MxGPU) | Hardware | Fixed partition (e.g., 1 GB per vGPU) | 8–32 per GPU | <1ms |
| **MIG** (Multi-Instance GPU) | NVIDIA A100/A30/H100 | Hardware (compute + memory) | Fixed (e.g., 1/7 of GPU) | 2–7 per GPU | <1ms |
| **MPS** (Multi-Process Service) | NVIDIA | Process-level (shared address space) | Soft quotas | 16–48 per GPU | <0.5ms |
| **Time-sliced sharing** | All vendors (driver-level) | Context switch | No partitioning (shared VRAM) | 4–16 per GPU | 1–5ms (context switch) |
| **None (dedicated)** | All | Full GPU per session | All VRAM | 1 per GPU | 0 |

LiquiDE auto-detects the sharing technology and adjusts resource limits accordingly. In vGPU environments, the session sees a virtual GPU with its allocated resources. In MPS/time-sliced environments, LiquiDE enforces soft limits via Vulkan memory budget tracking and compute scheduling.

##### VRAM Budget Management

```
Session VRAM Budget (e.g., 256 MB)
├── Compositor surfaces (framebuffers, intermediate)    ~60 MB @ 1080p
├── Blur textures (downsampled, cached)                 ~20 MB
├── Shadow cache                                         ~10 MB
├── Tile buffer (double-buffered tile grid)             ~40 MB @ 1080p
├── Encoder reference frames (HW encoder managed)       ~80 MB @ 1080p
├── Font atlas                                           ~10 MB
├── Cursor cache                                         ~2 MB
└── Headroom / overflow                                  ~34 MB
```

When VRAM is exhausted:
1. Evict shadow cache (LRU).
2. Reduce blur quality (smaller intermediate textures).
3. Reduce tile buffer depth (single-buffered).
4. Fall back to system RAM for overflow allocations (Vulkan `HOST_VISIBLE` memory).
5. If still insufficient: drop to `gpu-composite` (disable hardware encoder, free encoder VRAM).
6. Last resort: drop to `cpu-only`.

#### GPU Compositing Pipeline

In GPU mode, the compositor runs entirely on the GPU:

```
Wayland surface commits
    │
    ▼
Upload surface buffers to GPU (DMA-BUF / VkBuffer import)
    │
    ▼
Vulkan compute shader pipeline:
    1. Scene graph traversal → per-surface draw commands
    2. Rounded rectangle clipping (per-surface scissor regions)
    3. Alpha compositing (Porter-Duff) via compute shader
    4. Backdrop blur via Kawase blur shader (downscale → blur → upscale → composite)
    5. Box shadow rendering (Gaussian convolution)
    6. Cursor overlay composite
    │
    ▼
Output framebuffer (Vulkan VkImage)
    │
    ├──► Hardware encoder input (zero-copy: VkImage → VAAPI/NVENC surface)
    │
    └──► Damage readback for tile channel (optional: VkImage → host for tile delta)
```

Key differences from CPU path:
- **Zero-copy compositing**: Surface buffers from Wayland clients using DMA-BUF are imported directly into Vulkan without CPU-side copies.
- **Shader-based effects**: Blur, shadows, rounded corners, and alpha compositing run as Vulkan compute shaders — 10–50x faster than CPU SIMD paths.
- **Encoder zero-copy**: The composited framebuffer is passed directly from Vulkan to the hardware encoder (VAAPI via DMA-BUF export, NVENC via CUDA interop) without readback to CPU memory.
- **End-to-end GPU**: In the ideal case (DMA-BUF input → Vulkan composite → HW encode → network), the frame **never touches CPU memory**.

#### Hardware Encoder Integration

| Encoder API | GPU Vendor | Codec Support | Session Concurrency | LiquiDE Integration |
|-------------|-----------|---------------|--------------------|--------------------|
| **VAAPI** | Intel (iGPU, dGPU), AMD (via Mesa) | H.264, H.265, AV1 (Intel Arc+) | 6–32 (GPU-dependent) | `libva` FFI. DMA-BUF input from Vulkan compositor. |
| **NVENC** | NVIDIA (Turing+) | H.264, H.265, AV1 (Ada+) | 8 (consumer) / 64 (professional) | NVENC SDK FFI. CUDA context shared with Vulkan via `VK_KHR_external_memory`. |
| **AMF** | AMD (RDNA+) | H.264, H.265, AV1 (RDNA3+) | 4–16 | AMF SDK FFI. Vulkan interop via `amf::AMFVulkanDevice`. |
| **V4L2 M2M** | ARM SoCs (RK3588, Jetson) | H.264, H.265 | 2–8 | V4L2 ioctl. DMA-BUF import from Vulkan. |

##### Encoder Selection Priority

In GPU Server Mode, the encoder selection changes:

| Priority | Condition | Selected Encoder |
|----------|-----------|-----------------|
| 1 | GPU HW encoder available for negotiated codec | Hardware encoder |
| 2 | GPU HW encoder unavailable for codec, but available for lower codec | Hardware encoder with codec fallback (e.g., AV1→H.265→H.264) |
| 3 | Multiple HW encoders (e.g., dual GPU) | Load-balance across encoders |
| 4 | HW encoder session limit reached | Queue or fall back to software |
| 5 | No HW encoder / GPU failure | Software encoder (SVT-AV1, OpenH264) |

##### Encoder Quality in GPU Mode

Hardware encoders generally trade quality for speed. LiquiDE compensates:

| Strategy | Description |
|----------|-------------|
| Higher bitrate | HW encoder at 1.5–2x the bitrate of software encoder for equivalent visual quality |
| Lookahead | Enable encoder lookahead (1–4 frames) for better rate control, at the cost of slight latency |
| B-frames | Enable B-frames for HW encoder when latency budget allows (>25ms additional) |
| Two-pass for recording | Recording encoder uses two-pass mode (first pass = rate analysis, second = encode) |
| Perceptual tuning | Enable perceptual encoding hints (SSIM/VMAF optimization mode) when supported |

#### GPU Server Mode SLOs

GPU mode has tighter performance targets than CPU-only:

| Metric | CPU-Only SLO | GPU Mode SLO | Notes |
|--------|-------------|-------------|-------|
| Input-to-photon (LAN) | p50 <16ms, p99 <25ms | p50 <10ms, p99 <18ms | Zero-copy pipeline eliminates CPU bottleneck |
| Frame composite time (1080p) | <8ms | <2ms | Vulkan compute vs CPU SIMD |
| Frame composite time (4K) | <25ms | <5ms | GPU scales with resolution |
| Encode time (H.264, 1080p) | <8ms (software) | <2ms (hardware) | HW encoder is 4–10x faster |
| Encode time (AV1, 1080p) | <16ms (software) | <4ms (hardware AV1) | Only AV1-capable hardware |
| Blur time (1080p, 7-pass Kawase) | <4ms (cached) | <0.5ms | GPU compute shader |
| Max concurrent 1080p sessions (1 GPU) | N/A | 16–32 (vGPU), 8 (MIG), 4–8 (time-slice) | Depends on GPU model and sharing |
| VRAM per session (1080p) | N/A | 200–300 MB | Includes encoder reference frames |

#### GPU Failure Handling

| Failure | Detection | Recovery |
|---------|-----------|----------|
| GPU process/context hang (TDR) | Vulkan `VK_ERROR_DEVICE_LOST` | Reset Vulkan device. Compositor re-creates all GPU resources. Sessions experience 1–2 frame drop. If repeated, fall back to CPU. |
| VRAM exhaustion | `VK_ERROR_OUT_OF_DEVICE_MEMORY` | Evict caches → reduce quality → overflow to system RAM → fall back to cpu-only |
| Hardware encoder error | Encoder API error code | Retry once. If persistent, mark encoder as failed, fall back to software encoder for this session. |
| Driver crash | Kernel log / `dmesg` notification | Log error. All sessions on this GPU fall back to CPU. Alert admin via metrics/notification. |
| GPU hardware failure | No Vulkan device available | Sessions start in cpu-only mode. Admin notified. |
| vGPU migration | Hypervisor live migration | Vulkan device lost → re-probe → re-initialize. Brief interruption (1–5s). |

#### GPU Mode Configuration

```toml
[gpu]
mode = "auto"                            # auto, gpu-full, gpu-composite, cpu-only
                                          # auto: use GPU if available, fall back to CPU

[gpu.compositor]
enabled = true                            # use Vulkan compute for compositing
prefer_compute = true                     # prefer compute shaders over graphics pipeline
max_vram_compositor_mb = 128              # VRAM budget for compositor (excluding encoder)

[gpu.encoder]
enabled = true                            # use hardware encoder when available
prefer_api = "auto"                       # auto, vaapi, nvenc, amf, v4l2
max_sessions = 0                          # 0 = use hardware limit
lookahead_frames = 2                      # encoder lookahead (0 = disabled)
b_frames = false                          # enable B-frames (increases latency by ~1 frame)
quality_preset = "balanced"               # speed, balanced, quality (maps to encoder preset)

[gpu.sharing]
technology = "auto"                       # auto, sriov, mig, mps, time-slice, dedicated
vram_budget_mb = 256                      # per-session VRAM budget
enforce_vram_limit = true                 # hard-enforce VRAM limit (reject allocations beyond)
gpu_time_quota_percent = 50               # max GPU compute time per session (time-slice mode)

[gpu.fallback]
on_gpu_error = "degrade"                  # degrade (fall back to CPU), terminate, retry
max_retries = 3                           # retry count before permanent fallback
retry_delay_seconds = 5
log_gpu_errors = true
alert_on_fallback = true                  # emit metric/alert when GPU → CPU fallback occurs

[gpu.monitoring]
vram_usage_warn_percent = 80              # warn when VRAM usage exceeds this
encoder_queue_warn_depth = 4              # warn when encoder queue exceeds this
report_gpu_metrics = true                 # expose GPU metrics via Prometheus
```

#### GPU Policy Keys

| Policy Key | Type | Resolution | Description |
|------------|------|-----------|-------------|
| `gpu.mode` | enum | `highest_precedence` | GPU mode for sessions in this scope |
| `gpu.max_vram_mb` | int | `min` | Per-session VRAM cap |
| `gpu.encoder_enabled` | bool | `deny_overrides` | Allow hardware encoder |
| `gpu.max_sessions_per_gpu` | int | `min` | Session density limit per GPU |
| `gpu.allow_dedicated` | bool | `deny_overrides` | Allow dedicated GPU per session |

#### GPU Metrics (Prometheus)

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `liquide_gpu_vram_used_bytes` | gauge | `device`, `session_id` | VRAM usage per session |
| `liquide_gpu_vram_total_bytes` | gauge | `device` | Total VRAM per device |
| `liquide_gpu_encoder_active_sessions` | gauge | `device`, `api` | Active HW encoder sessions |
| `liquide_gpu_encoder_queue_depth` | gauge | `device` | Encoder queue depth |
| `liquide_gpu_composite_time_seconds` | histogram | `device` | GPU composite time |
| `liquide_gpu_encode_time_seconds` | histogram | `device`, `codec` | HW encode time |
| `liquide_gpu_fallback_total` | counter | `device`, `reason` | GPU → CPU fallback events |
| `liquide_gpu_errors_total` | counter | `device`, `error_type` | GPU errors |
| `liquide_gpu_temperature_celsius` | gauge | `device` | GPU temperature (if exposed) |
| `liquide_gpu_utilization_percent` | gauge | `device` | GPU utilization (if exposed) |


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

#### 6. Icon & Asset Caching

LiquiDE supports **client-side caching of application icons, shell assets, and UI resources** to reduce repeated transmission of static assets across sessions.

**What is cached:**

| Asset Type | Description | Typical Size |
|-----------|-------------|-------------|
| Application icons | `.desktop` file icons (all sizes: 16–256px + SVG) | 5–50 KB each |
| Tray icons | StatusNotifierItem icons and overlays | 1–10 KB each |
| Cursor themes | Cursor images for all shapes in the active cursor theme | 50–200 KB total |
| Shell assets | Dock icons, status bar icons, launcher category icons | 10–100 KB total |
| Notification icons | App icons shown in notification toasts | 5–20 KB each |
| Theme assets | Glass textures, UI pattern images, theme-specific graphics | 50–500 KB total |
| User avatars | Session user and login screen avatars | 5–64 KB each |

**How it works:**

1. **Server-side manifest**: at session start (after `ServerHello`), the server sends an **asset manifest** listing all assets the session will reference. Each entry contains:
   - Asset ID (unique string, e.g., `icon:firefox:48`, `cursor:default:left_ptr`).
   - Content hash (SHA-256 truncated to 128 bits).
   - Size in bytes.
   - MIME type.
   - Category (icon, cursor, theme, avatar).

2. **Client cache check**: the client checks its local asset cache against the manifest. For each asset:
   - **Cache hit** (hash matches): asset is not transferred. Client uses cached version.
   - **Cache miss** (hash mismatch or absent): client requests the asset from the server.

3. **Lazy transfer**: cache misses are transferred on-demand with priority ordering:
   - Cursor theme assets: highest priority (needed immediately for cursor rendering).
   - Dock/shell icons: high priority (visible immediately).
   - Application icons: medium priority (transferred as apps appear).
   - Theme assets, notification icons: low priority (transferred in background).

4. **Transfer protocol**: assets are sent on the control channel as `AssetData` messages (see spec-protocol-formal.md). Small assets (< 4 KB) are inlined in the manifest. Larger assets are requested individually.

5. **Automatic by default**: asset caching is enabled automatically. No user configuration is needed. The client detects optimal behavior based on:
   - **Platform rendering support**: the client reports its icon rendering capabilities (SVG support, icon sizes supported, HiDPI scale factor) in the `ClientHello` capabilities block.
   - **OS conventions**: the server sends icons in formats matching the client OS:
     - **Linux client**: SVG preferred, PNG fallback. freedesktop icon theme sizes.
     - **Windows client**: ICO or PNG at Windows-standard sizes (16, 20, 24, 32, 40, 48, 64, 256). Taskbar icons for seamless windows.
     - **macOS client**: ICNS or PNG at macOS standard sizes (16, 32, 64, 128, 256, 512, 1024). Retina-scaled (@2x) variants.
     - **Browser client**: PNG or SVG at rendered sizes only.

6. **Cache persistence**: the asset cache persists across sessions and server reconnections. Cached assets are keyed by `(server_fingerprint, asset_id, content_hash)`.

7. **Cache invalidation**: when the server's asset manifest changes (new theme, icon update, avatar change), only changed assets are re-transferred. The content hash ensures stale assets are replaced.

**Configuration (server-side):**

```toml
[performance.asset_cache]
enabled = true                       # master switch
inline_threshold_bytes = 4096        # assets smaller than this are inlined in manifest
icon_format_preference = "auto"      # auto (client-dependent), svg, png
send_retina_variants = true          # send @2x icons for HiDPI clients
cursor_theme_preload = true          # preload entire cursor theme on connect
max_manifest_size_mb = 2             # max asset manifest size (limits total tracked assets)
```

**Client cache behavior** is configured in the client config (see spec-client.md).

#### 7. Partial Caches for Static Regions
- **Status bars**, dock backgrounds, and other rarely-changing regions maintain cached rasterizations.
- Cache hit: blit from cache (nearly free).
- Cache invalidation: only on content change (clock tick, notification badge, etc.).
- **Fast bounce from idle**: when the session goes idle, caches are preserved in memory. On wake (input event), the first frame is assembled from caches with near-zero render cost, then incremental updates resume.
- **Configurable**: each auto-caching behavior can be:
  - `enabled` (default) — system manages cache lifecycle.
  - `disabled` — always re-render (useful for debugging or specific use cases).
  - `level:<N>` — set cache aggressiveness (1 = minimal caching, 5 = aggressive caching).

#### 8. Animation Policy
- Default animations are **event-driven** (input/transition) rather than constant.
- Frame rate caps for UI-only animation (e.g., 30 fps) while cursor/input stays responsive.
- Idle state: 1–2 fps or pure "only-on-change" mode.
- All animation durations and curves configurable via CSS.

#### 9. Color Management

LiquiDE provides server-side color management for accurate rendering. The design goal is **"looks the same everywhere"** — a document or image viewed in a LiquiDE session should appear visually consistent regardless of which client device is used, modulo the client display's hardware limitations.

- **ICC Profile Handling**:
  - The compositor applies ICC color profiles when rendering.
  - Default profile: sRGB (the de facto standard for web and desktop content).
  - Custom ICC profiles can be loaded per virtual monitor via config: `display.icc_profile = "/path/to/profile.icc"`.
  - The server applies the profile during compositing — the output framebuffer is in the target color space before encoding.
  - Rendering intent: perceptual (default), configurable to relative colorimetric, absolute colorimetric, or saturation.

- **Client Display Color**:
  - The encoded video stream is in sRGB by default (or the configured server-side profile).
  - The **client** is responsible for applying its own display ICC profile (monitor calibration). This is outside LiquiDE's control — it depends on the client OS and display hardware.
  - The client config includes `color.profile_hint` to inform the server of the client's display characteristics (gamut, white point). The server can use this to optimize rendering, but it is informational only.

- **Color Pipeline Modes**:

  LiquiDE supports three color pipeline modes, negotiated during session startup between client capabilities and server configuration. The default is SDR-sRGB for backward compatibility; wide color gamut (WCG) and HDR are opt-in.

  | Pipeline Mode | Internal Precision | Compositing Gamut | Output Bit Depth | Transfer Function | Codec Requirements |
  |--------------|-------------------|-------------------|------------------|-------------------|--------------------|
  | **SDR-sRGB** (default) | 8-bit per channel | sRGB / BT.709 | 8 bpc | sRGB gamma | All codecs (baseline profiles) |
  | **WCG-SDR** | 16-bit or float32 | Display-P3 or Rec.2020 | 10 bpc | sRGB gamma | H.265 Main 10, AV1 10-bit, VP9 Profile 2 |
  | **HDR** | float32 | Rec.2020 | 10 or 16 bpc | PQ (ST 2084) or HLG | H.265 Main 10, AV1 10-bit, VP9 Profile 2 |

  SDR-sRGB is the lowest-cost path and the only mode guaranteed on all hardware. WCG-SDR and HDR require 10-bit codec profiles, which may require hardware encoder support for real-time performance. H.264 does not support 10-bit encoding in its standard profiles and is unavailable in WCG/HDR modes. See [spec-rendering-software.md](spec-rendering-software.md) §3.3 for the full compositing pipeline specification.

  The active pipeline mode is determined by intersecting the client's `color.supported_modes` (sent in `ClientHello`) with the server's `display.color.pipeline_mode` configuration. If no match exists, the server falls back to SDR-sRGB.

- **Deep Color Pixel Formats**:

  | Format | Bits Per Pixel | Bit Layout | Use Case |
  |--------|---------------|------------|----------|
  | `rgb888` | 24 | 8R + 8G + 8B | SDR tile mode (default) |
  | `rgba8888` | 32 | 8R + 8G + 8B + 8A | SDR tile mode with alpha |
  | `rgb565` | 16 | 5R + 6G + 5B | Low-bandwidth SDR tile mode |
  | `rgb101010` | 32 | 10R + 10G + 10B + 2 pad | WCG/HDR tile mode (10-bit) |
  | `rgba1010102` | 32 | 10R + 10G + 10B + 2A | WCG/HDR tile mode with 2-bit alpha |
  | `rgba16161616` | 64 | 16R + 16G + 16B + 16A | HDR mastering/production (16-bit) |

  The pixel format for tile-mode encoding is negotiated during the tile channel setup (`TileConfig.pixel_format`). The server selects the format based on the active pipeline mode and the client's `color.supported_pixel_formats` capability. SDR sessions always use `rgb888`/`rgba8888`. WCG and HDR sessions use `rgb101010` or `rgba1010102` by default.

- **HDR Metadata Passthrough**:

  When the HDR pipeline mode is active, the compositor attaches HDR metadata to the encoded video stream:

  | Metadata Standard | Transport | Description |
  |------------------|-----------|-------------|
  | **HDR10** (SMPTE ST 2086) | `FrameHeader.hdr_metadata.hdr10` (CBOR) | Static mastering display metadata: display primaries, white point, min/max luminance, MaxCLL, MaxFALL. Sent once at stream start and on change. |
  | **HDR10+** (SMPTE ST 2094-40) | `FrameHeader.hdr_metadata.hdr10plus` (raw bytes) | Dynamic tone mapping metadata, per-frame. Passed through as opaque SEI/OBU data from the encoder. |
  | **HLG** (ARIB STD-B67) | Transfer function signaling only | No per-frame metadata — HLG is scene-referred. Transfer function is signaled in `FrameHeader.color_space.transfer`. |

  See [spec-protocol-formal.md](spec-protocol-formal.md) §8.4 for the full `ColorSpaceInfo`, `HDRMetadata`, and `HDR10Static` CBOR schemas.

- **Codec Color Metadata (10-bit and HDR)**:

  | Codec | 10-bit Profile | HDR Support | Color Signaling |
  |-------|---------------|-------------|-----------------|
  | H.264 | Not available (8-bit only in Baseline/Main/High) | No | N/A — H.264 is excluded from WCG/HDR modes |
  | H.265 | **Main 10** profile | HDR10, HDR10+, HLG | VUI: `colour_primaries`, `transfer_characteristics`, `matrix_coefficients` + SEI for HDR10 static/dynamic metadata |
  | VP9 | **Profile 2** (10/12-bit) | HDR10 (via container metadata) | Color space signaling in frame header: `CS_BT_2020` + `bit_depth=10` |
  | AV1 | Native 10/12-bit | HDR10, HDR10+, HLG | `color_config` OBU: primaries=9 (BT.2020), transfer=16 (PQ) or 18 (HLG), matrix=9 (BT.2020-NCL) |
  | Tile (bitmap) | `rgb101010`, `rgba1010102`, `rgba16161616` | Supported via pixel format | Color space metadata attached to `TileConfig`; no embedded ICC |

- **Gamma / Brightness Controls**:
  - Virtual gamma adjustment: `display.gamma = 1.0` (range: 0.5–2.0). Applied as a post-compositing transfer function.
  - Virtual brightness adjustment: `display.brightness = 100` (range: 10–100, percentage). Applied as a linear scale on the output.
  - Night mode color temperature: handled separately (see spec-settings.md §7.1).
  - These adjustments affect the encoded video stream — the client receives already-adjusted frames.

- **Color Pipeline End-to-End**:

  The complete color pipeline from application rendering to client display (showing all three modes):

  ```
  Application (renders in app color space — sRGB, P3, or HDR)
      │
      ▼
  Compositor
      │ SDR-sRGB: linear sRGB compositing, 256-entry LUT
      │ WCG-SDR:  linear P3/Rec.2020, 1024-entry LUT or analytical
      │ HDR:      linear Rec.2020, float32, analytical PQ/HLG
      ▼
  Post-compositing gamma/brightness adjustment
      │ (in HDR mode: tone mapping applied here for SDR fallback clients)
      ▼
  Encoder
      │ SDR:     sRGB gamma, 8-bit, BT.709 primaries
      │ WCG-SDR: sRGB gamma, 10-bit, P3/BT.2020 primaries
      │ HDR:     PQ/HLG, 10/16-bit, BT.2020 primaries + HDR metadata
      ▼
  Transport (encoded video/tile stream with color_space + hdr_metadata)
      │
      ▼
  Client decoder → Client color management → Display
      │ SDR:     direct display (client applies own ICC)
      │ WCG-SDR: gamut compress if display < P3 (client-side)
      │ HDR:     HDR passthrough (PQ/HLG), or client tone-maps to SDR
      ▼
  User's eyes
  ```

  **What LiquiDE guarantees**: the encoded stream is always in a well-defined color space with correct transfer function and color primaries metadata in the video bitstream (or tile channel). The pipeline mode and color space are explicitly negotiated during handshake — both sides agree on the output format. Applications that are color-managed (e.g., GIMP with ICC support) will render correctly because the Wayland compositor provides `wp_color_management_v1` protocol support.

  **What LiquiDE does NOT guarantee**: the final appearance on the user's physical display. This depends on the client display's calibration, ICC profile, and the client OS's color management. A poorly calibrated monitor will show inaccurate colors — this is the same limitation as any display system.

- **Codec Color Metadata (SDR Baseline)**:

  | Codec | Color Metadata (SDR-sRGB) | Notes |
  |-------|--------------------------|-------|
  | H.264 | VUI: `colour_primaries=1` (BT.709), `transfer_characteristics=13` (sRGB), `matrix_coefficients=1` (BT.709) | Standard for SDR sRGB content |
  | H.265 | Same VUI parameters as H.264 | SDR same as H.264; for 10-bit/HDR see Codec Color Metadata table above |
  | VP9 | Color space signaling in frame header | `CS_UNKNOWN` maps to sRGB; Profile 2 for 10-bit (see above) |
  | AV1 | `color_config`: primaries=1, transfer=13, matrix=1 | SDR sRGB; for 10-bit/HDR see above |
  | Tile (raw bitmap) | Color space per `TileConfig`; `rgb888` assumed sRGB | For deep color formats (`rgb101010`, `rgba1010102`, `rgba16161616`) color space negotiated via pipeline mode |

- **Configuration**:

  ```toml
  [display.color]
  # Color pipeline mode: "sdr-srgb" (default), "wcg-sdr", "hdr"
  pipeline_mode = "sdr-srgb"
  # Server-side compositing color space (for WCG/HDR modes)
  compositing_space = "srgb"          # srgb, display-p3, rec2020
  # Compositing gamut for WCG-SDR mode (ignored in SDR-sRGB)
  compositing_gamut = "display-p3"    # display-p3, rec2020
  # Output bit depth (8 for SDR, 10 for WCG/HDR, 16 for HDR mastering)
  compositing_bit_depth = 8           # 8, 10, 16
  # Per-monitor ICC profile
  icc_profile = ""                    # path to ICC profile file (empty = sRGB)
  # Rendering intent for ICC profile application
  rendering_intent = "perceptual"     # perceptual, relative, absolute, saturation
  # Embed ICC metadata in encoded video VUI
  embed_color_metadata = true
  # HDR transfer function (only used when pipeline_mode = "hdr")
  hdr_transfer_function = "pq"       # pq (ST 2084), hlg (ARIB STD-B67)
  # HDR static metadata: Maximum Content Light Level (nits, 0 = auto)
  hdr_max_cll = 0
  # HDR static metadata: Maximum Frame-Average Light Level (nits, 0 = auto)
  hdr_max_fall = 0
  # Pass through HDR10+ dynamic metadata from applications
  hdr10plus_passthrough = true
  # Tone mapping operator for HDR → SDR fallback
  tone_map_operator = "reinhard"      # reinhard, bt2390, hable, aces
  ```

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

### Per-Monitor DPI & Scaling

LiquiDE supports per-monitor DPI scaling, where each virtual monitor in a session can have a different DPI scale factor. This maps to the client's physical display configuration.

#### DPI Mapping Model

The **client controls DPI**. When the client connects (or when the user changes display settings), it sends a `DisplayUpdate` message containing per-monitor DPI information:

| Field | Description |
|-------|-------------|
| `monitor_id` | Virtual monitor identifier |
| `width` / `height` | Resolution in pixels |
| `scale_factor` | DPI scale (1.0 = 96 DPI, 1.25 = 120 DPI, 1.5 = 144 DPI, 2.0 = 192 DPI, etc.) |
| `physical_width_mm` / `physical_height_mm` | Physical dimensions (if known) for DPI calculation |

The server sets the virtual monitor's `wl_output` scale and fractional scale via `wp_fractional_scale_v1`. Wayland applications see the correct DPI and render accordingly.

#### Per-OS Client DPI Discovery

| Platform | API | Scale Factor Source | Notes |
|----------|-----|-------------------|-------|
| **Windows** | `GetDpiForMonitor()`, `PROCESS_DPI_AWARENESS` | Per-monitor V2 DPI awareness (Windows 10 1703+) | Client must be DPI-aware (manifest). Returns integer DPI per monitor (96, 120, 144, 192, etc.) |
| **Windows (older)** | `GetDpiForWindow()` | System DPI or per-monitor V1 | Fallback for older Windows. Less accurate for mixed-DPI. |
| **macOS** | `NSScreen.backingScaleFactor` | Per-screen. Returns 1.0 (non-Retina) or 2.0 (Retina). | macOS does not expose arbitrary DPI — it's always 1x or 2x. The OS handles intermediate scaling internally. |
| **Linux (Wayland)** | `wl_output.scale` + `wp_fractional_scale_v1` | Per-output integer or fractional scale | Fractional scale (e.g., 1.25) requires `wp_fractional_scale_v1` support. |
| **Linux (X11)** | `Xrandr` + `Xft.dpi` | Global DPI (Xft.dpi) or per-monitor via randr | X11 DPI is historically inconsistent. Client uses randr output physical size + resolution to compute true DPI. |
| **Web** | `window.devicePixelRatio` | Per-window (not per-monitor) | Changes on window drag across monitors. Client listens for `resize` events and updates. |

#### Mixed-DPI Behavior

When the client has monitors at different DPI (e.g., laptop at 2x + external at 1x):

1. **"Match local monitors" mode**: each virtual monitor gets the DPI of the corresponding physical monitor. The server renders at native DPI for each. Applications spanning two monitors see a DPI change at the boundary (standard Wayland behavior via `wl_surface.enter`/`wl_surface.leave` on different outputs).

2. **"Tabbed monitors" mode**: each tab gets the DPI of the physical monitor currently displaying the client window. When the user drags the client window across monitors, the DPI changes and the server re-renders.

3. **"Single large canvas" mode**: uses the DPI of the primary monitor. Other monitors may appear slightly larger or smaller depending on DPI mismatch.

4. **"Multi-window" mode**: each window gets the DPI of the physical monitor it is on. Moving a window to a different monitor triggers a `DisplayUpdate`.

#### DPI Change Handling

When DPI changes mid-session (user drags window to different monitor, changes OS scaling):

1. Client sends `DisplayUpdate` with new `scale_factor`.
2. Server updates `wl_output` scale.
3. Wayland surfaces receive `wl_surface.enter` with new output → applications re-render at new DPI.
4. Server compositor invalidates all caches for affected surfaces.
5. Full frame is re-encoded and sent (due to resolution/scale change).
6. Client receives new frame at new resolution and renders.

**Latency impact**: DPI changes trigger a full-screen redraw. Applications that are slow to respond to DPI changes (particularly XWayland/X11 apps) may show brief scaling artifacts. The server applies bilinear scaling as a temporary measure until the application re-renders.

#### Failure Modes

| Failure | Behavior | Mitigation |
|---------|----------|------------|
| Client reports wrong DPI | Text too large or too small in session | User can override scale in session settings. Admin can force scale via policy. |
| Client doesn't report DPI | Server defaults to 96 DPI (scale 1.0) | Client-side diagnostic warns if DPI detection fails. |
| Application ignores DPI change | Application renders at old DPI, compositor scales | Compositor applies scaled compositing. XWayland apps get server-side scaling. |
| Rapid DPI changes (window dragged between monitors) | Multiple `DisplayUpdate` messages in quick succession | Server debounces DPI changes (200ms). Only the final DPI value is applied. |
| Fractional scale on non-fractional-aware app | Sub-pixel rendering artifacts | Server rounds to nearest integer scale for non-fractional apps. Note in session settings. |

#### Testing Matrix

| Test Case | Platforms | Expected Result |
|-----------|----------|----------------|
| Single monitor, 100% (1x) | Windows, macOS, Linux | Text and UI at standard size |
| Single monitor, 150% (1.5x) | Windows, Linux Wayland | Fractional scale applied, no blur |
| Single monitor, 200% (2x) | Windows, macOS (Retina), Linux | HiDPI rendering, sharp text |
| Dual monitor, same DPI | All | Both monitors render identically |
| Dual monitor, mixed DPI (1x + 2x) | Windows, macOS | Per-monitor DPI correct, apps re-render on move |
| Dual monitor, mixed fractional (1.0 + 1.25) | Windows, Linux | Fractional scale applied per monitor |
| DPI change during session (OS setting change) | Windows, Linux | Session updates within 1 second |
| Window drag across monitors (different DPI) | Windows, macOS | DPI transitions within 500ms |
| Web client pixel ratio change | Chrome, Firefox | Canvas re-renders at new DPI |

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
- Screen partitioned into tiles (configurable: 32×32 to 256×256, default: 64×64).
- Per tile:
  - Hash-based change detection (CRC-32C of raw pixel data).
  - **Adaptive delta encoding**: when a tile changes, the encoder chooses the most efficient strategy:

| Strategy | Condition | Method |
|----------|-----------|--------|
| **Skip** | Tile hash unchanged from previous frame | No data sent (zero bandwidth) |
| **XOR delta** | < 50% of pixels changed (measured by XOR population count) | Compute XOR of current tile against previous tile, compress the XOR bitmap. Client applies XOR to reconstruct. |
| **Full tile** | ≥ 50% of pixels changed, or no previous tile cached (first frame) | Compress entire tile bitmap and send. |
| **Copy** | Tile is identical to another tile in the same frame (e.g., solid background) | Reference to the already-sent tile by index (2-byte overhead). |
| **Solid fill** | All pixels in the tile are the same color | Single RGBA value (4 bytes). |

- **Delta decision heuristic**: after XOR, count non-zero bytes. If non-zero ratio < `tile.delta_threshold` (default: `0.50`), send XOR delta. Otherwise, send full tile. The threshold auto-tunes based on observed compression ratios — if the XOR delta compresses larger than the full tile, the threshold is lowered.
- **Previous tile buffer**: the server maintains a per-tile ring buffer (depth 1 by default, configurable to 2 for lossy-recovery scenarios). The client maintains a matching buffer. Both sides stay in sync because the server explicitly flags each tile as `full`, `delta`, `copy`, `solid`, or `skip`.
- **Tile compression pipeline**: `raw pixels → [XOR delta] → codec compress → transport`. The codec compress step uses the selected tile codec (Zstd, LZ4, PNG, QOI, WebP, raw).
- **Scrolling optimization**: when the compositor detects a scroll operation (surface commits with `wl_surface.offset`), the tile grid is shifted and only the newly exposed strip of tiles is sent. The server transmits a `TileScroll` message with the scroll vector, and the client shifts its tile buffer accordingly — avoiding full-screen re-encoding for scroll events.
- Best for: terminals, code editors, dashboards, documents, remote admin tools.

**Tile encoding configuration:**

```toml
[performance.tile]
tile_size = 64                      # tile dimension in pixels (32, 64, 128, 256)
codec = "zstd"                      # zstd, lz4, png, qoi, webp, raw
delta_enabled = true                # enable XOR delta encoding
delta_threshold = 0.50              # pixel change ratio below which XOR delta is used
solid_detect = true                 # detect and optimize solid-color tiles
copy_detect = true                  # detect duplicate tiles within the same frame
scroll_detect = true                # detect scroll operations and send scroll vectors
color_depth = "rgb888"              # rgb888 (24-bit), rgba8888 (32-bit), rgb565 (16-bit, lossy), rgb101010 (30-bit), rgba1010102 (32-bit deep), rgba16161616 (64-bit HDR)
```

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
| **AES-128-GCM** | Local/LAN deployments | Lower overhead alternative (see below) |
| **AES-256-GCM** | High-security deployments | Maximum encryption strength |
| **ChaCha20-Poly1305** | ARM64 / no AES-NI | Faster on platforms without AES hardware |
| **None (plaintext)** | Localhost only | Must be explicitly enabled, policy-guarded |

- Encryption is **per-transport-stream**, allowing different encryption for control vs. media channels.
- Encryption scheme negotiated at connection or set by policy.

#### Low-Latency Encryption Mode (AES-128-GCM for LAN)

The AES-128-GCM "low-latency" option is **not** a weaker security mode. It is the same AEAD cipher used within TLS 1.3, applied with all standard security guarantees. The difference is operational: on LAN deployments where both client and server are on a trusted private network, AES-128-GCM offers lower per-packet CPU cost than AES-256-GCM (particularly on hardware with AES-NI) while still providing 128-bit security — more than sufficient for any current threat model.

**What low-latency mode IS:**

| Property | Guarantee |
|----------|-----------|
| AEAD | Yes — AES-128-GCM provides authenticated encryption with associated data. Every packet is integrity-protected and encrypted. Tampering is detected. |
| Key exchange | Standard TLS 1.3 handshake (ECDHE). Session keys are ephemeral, derived via HKDF. No pre-shared secrets unless explicitly configured (see PSK mode below). |
| Replay protection | Yes — TLS 1.3 record layer sequence numbers prevent replay. For DTLS/QUIC, the transport's anti-replay window applies. |
| Forward secrecy | Yes — ephemeral ECDHE key exchange. Compromising the server's long-term key does not decrypt past sessions. |

**What low-latency mode is NOT:**

- It is NOT "encryption disabled."
- It is NOT a pre-shared key mode (unless explicitly configured — see below).
- It does NOT skip the TLS handshake or certificate verification.
- It does NOT reduce the MAC tag size or weaken integrity protection.

**Pre-Shared Key (PSK) Mode**

For deployments where TLS handshake latency is unacceptable (e.g., session resume on ultra-low-latency LAN), LiquiDE supports TLS 1.3 PSK-based resumption:

| Property | PSK Mode Behavior |
|----------|------------------|
| Initial connection | Standard TLS 1.3 handshake with certificate verification. Server issues a PSK identity (session ticket). |
| Subsequent connections | Client presents PSK identity. Server validates. Handshake completes in 0-RTT or 1-RTT. |
| 0-RTT data | Supported but **disabled by default** (`transport.enable_0rtt = false`). 0-RTT data is replayable by design. Only safe for idempotent operations (e.g., `ClientHello`). |
| PSK lifetime | Default 24 hours. Configurable via `transport.psk_lifetime_sec`. |
| Forward secrecy | Yes for 1-RTT PSK (ECDHE + PSK). Reduced for 0-RTT data (inherent TLS 1.3 0-RTT limitation). |

> **Anti-footgun**: LiquiDE NEVER offers a "disable encryption" option outside of localhost-only connections. The `transport.encryption = "none"` setting is guarded by: (1) server policy `transport.allow_plaintext = false` (default), (2) connection source IP MUST be `127.0.0.1` or `::1`, (3) an audit event is emitted on every plaintext connection. Any attempt to use plaintext over a non-loopback interface is rejected with a protocol error.

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

**Target Controller Selection:**

| Transport | Default Controller | Fallback | Rationale |
|-----------|-------------------|----------|-----------|
| QUIC | BBR v2 | Cubic | BBR v2 avoids bufferbloat, optimizes for interactive traffic. Cubic as fallback for fairness with legacy TCP flows. |
| TCP (TLS) | Cubic (OS default) | New Reno | TCP uses kernel congestion control. Cubic is standard on modern Linux. |
| UDP (raw DTLS) | Custom pacer (BBR-inspired) | Fixed-rate with backoff | Custom implementation: RTT-based pacing, loss-driven reduction, no slow-start (assumes interactive). |

**BBR v2 Tuning for Remote Desktop:**

| Parameter | Value | Standard BBR v2 | Rationale |
|-----------|-------|-----------------|-----------|
| `min_cwnd` | 2 × MSS | 4 × MSS | Smaller minimum window — remote desktop sends small control packets during idle. Standard BBR holds too much in-flight. |
| `pacing_gain` (idle) | 1.0 | 1.25 | No probing during idle — remote desktop has zero traffic when screen is static. Probing would waste bandwidth. |
| `pacing_gain` (active) | 1.25 | 1.25 | Standard probing during active use. |
| `max_bw_filter_window` | 2 seconds | 10 RTTs | Shorter bandwidth memory — remote desktop traffic is bursty (type → idle → type). Long memory holds stale high estimates. |
| `drain_to_target` | True | True | Standard BBR v2 drain. |
| `loss_threshold` | 1% | 2% | More aggressive loss response — a single lost video keyframe causes visible corruption. Err on the side of caution. |

**Per-Channel Loss Recovery:**

| Channel | Loss Detection | Recovery Action | Max Recovery Time |
|---------|---------------|----------------|-------------------|
| Video (0x10) | Sequence gap in `FrameHeader.seq` | `KeyFrameRequest` → server IDR within 1 frame period | 16–33ms |
| Tiles (0x12) | TCP/QUIC reliable retransmit | Automatic retransmit (transport-level) | 1 RTT |
| Audio (0x20/21) | Sequence gap in audio frames | Opus PLC / FEC decode / repeat-with-fade | 0ms (concealment is instant) |
| Input (0x50) | TCP/QUIC reliable retransmit | Automatic retransmit | 1 RTT |
| Cursor (0x11) | No recovery (latest-wins) | Next update replaces lost one | 1 frame period |
| Control (0x00) | TCP/QUIC reliable retransmit | Automatic retransmit | 1 RTT |

**Queuing Delay Cap:**

The sender enforces a maximum queuing delay of **≤10ms** at the send buffer. If a frame has been queued for >10ms without being paced out, the transport layer signals the encoder to reduce output (backpressure). This prevents the "bufferbloat within the application" anti-pattern where the transport send queue absorbs latency that should be visible to the encoder.

```toml
[transport.congestion]
controller = "bbr2"                    # "bbr2", "cubic", "fixed" (for testing)
max_queuing_delay_ms = 10              # max time a frame sits in send buffer
loss_threshold_percent = 1.0           # trigger quality reduction above this loss rate
min_cwnd_mss = 2                       # minimum congestion window in MSS units
bandwidth_probe_interval_sec = 2       # BBR bandwidth filter window
```

#### Adaptive Bitrate Control Loop

The transport layer runs an **Adaptive Bitrate (ABR) control loop** at 100ms intervals that adjusts encoding and transmission parameters based on real-time network and system conditions.

**Control Loop Inputs:**

| Input | Source | Update Frequency |
|-------|--------|-----------------|
| Smoothed RTT (sRTT) | Ping/Pong + ACK timing | Per-ACK |
| Packet loss rate | QUIC loss detection / TCP retransmit counters | Per-ACK |
| Congestion window occupancy | `bytes_in_flight / cwnd` | Per-ACK |
| Server CPU utilization | Compositor + encoder thread CPU time | Per-frame |
| Client decode latency | `FrameAck.decode_time_us` / `TileBatchAck.decode_time_us` | Per-frame |
| Send queue depth | Bytes pending in transport send buffer | Per-tick (100ms) |
| Jitter (audio) | EWMA inter-packet jitter from audio channel | Per-audio-frame |

**Control Loop Outputs:**

| Output | Effect | Range |
|--------|--------|-------|
| Per-channel byte budget | Bytes each channel may send per tick | Computed from estimated bandwidth × priority weights |
| Video FPS cap | Maximum frames per second for video encoder | 1–60 fps |
| Quality index | Encoder quantizer / quality preset | 0 (best) – 51 (worst, H.264 CRF scale) |
| Keyframe interval | Seconds between periodic IDR frames | 2–10s (shorter under loss) |
| Tile compression level | Zstd level for tile data | 1 (fast) – 6 (high ratio) |
| Tile size | Adaptive tile grid size | 32×32, 64×64, 128×128, 256×256 |

**100ms Tick Pseudocode:**

```
every 100ms:
  bw_estimate = bandwidth_estimator.current()
  loss = loss_detector.rate()
  rtt = rtt_estimator.smoothed()
  cpu = cpu_monitor.session_utilization()
  queue = send_buffer.bytes_pending()
  client_decode = last_frame_ack.decode_time_us

  # Step 1: Compute available budget
  budget = bw_estimate * 0.1s  # bytes per tick

  # Step 2: Allocate to priority levels (P0-P6)
  allocate_priority_budgets(budget)

  # Step 3: Adjust encoder parameters
  if loss > 3% OR queue > 0.8 * send_buffer_size:
    reduce_quality(step=1)
    if fps > 30: reduce_fps(target=30)
  elif loss > 1%:
    reduce_quality(step=1)
  elif cpu > 0.9 * cpu_limit:
    reduce_fps(target=max(15, current_fps - 15))
  elif client_decode > frame_budget_ms * 0.8:
    reduce_quality(step=1)  # client struggling to decode
  elif loss < 0.1% AND queue < 0.3 * send_buffer_size AND cpu < 0.5:
    increase_quality(step=1)
    if fps < target_fps: increase_fps(step=15)

  # Step 4: Adjust keyframe interval
  if loss > 2%: keyframe_interval = max(2s, keyframe_interval - 1s)
  else: keyframe_interval = min(10s, keyframe_interval + 0.5s)
```

**Network-Level Degradation Order:**

Distinct from the compositor's visual degradation ladder (spec-rendering-software.md §7), these are transport-level responses to network degradation:

| Step | Trigger | Action | User Impact |
|------|---------|--------|------------|
| N0 | Nominal | Full quality, full FPS | None |
| N1 | Loss > 1% OR queue > 50% | Reduce background refresh rate, increase tile delta threshold | Slower background updates |
| N2 | Loss > 2% OR queue > 70% | Increase video quantizer by 5, reduce keyframe interval to 3s | Slightly softer image |
| N3 | Loss > 3% OR queue > 80% | Disable shell animations, force tile-only for non-active windows | Static non-focused windows |
| N4 | Loss > 5% OR queue > 90% | Clamp FPS to 15, maximum quantizer, audio bitrate to minimum | Choppy, low quality but functional |
| N5 | Loss > 10% OR queue = 100% | Video paused, tile key-updates only for cursor region + active input area, audio continues | Minimal — maintaining input responsiveness |

#### Channel Priority & Pacing

LiquiDE multiplexes several data streams over a shared transport. Under congestion, not all streams are equal. The transport layer enforces a **strict priority hierarchy** and **pacing rules** that are normative:

**Priority levels (highest first):**

| Priority | Channel(s) | Scheduling Rule |
|----------|-----------|-----------------|
| **P0 — Emergency** | Emergency (0x01) | Always sent immediately. Never queued. Pre-empts all other channels. |
| **P1 — Input** | Input (0x50) | Sent within 1ms of arrival. Never delayed by video/tile backlog. Input events are small (<100 bytes) and MUST NOT wait behind large video frames in the send queue. |
| **P2 — Cursor** | Cursor (0x11) | Sent as datagrams (unreliable). Independent of video frame pacing. Cursor updates bypass the send queue entirely on QUIC/UDP. |
| **P3 — Audio** | Audio playback (0x20), Audio capture (0x21) | Paced at the audio frame rate (typically 50 fps = 20ms frames for Opus). Audio MUST NOT starve — if bandwidth is insufficient for both audio and video, audio wins. Underrun budget: max 1 audio frame drop per 10 seconds. |
| **P4 — Control** | Control (0x00) | Reliable, ordered. Moderate priority. Protocol signaling, resize, codec switch. |
| **P5 — Video/Tile** | Video (0x10), Tile (0x12) | Consumes remaining bandwidth after P0–P4 are satisfied. Subject to adaptive quality reduction under congestion. |
| **P6 — Bulk** | Clipboard (0x30), File Transfer (0x31), USB (0x40), Camera (0x60), Plugin IPC (0xF0) | Best-effort. Throttled aggressively under congestion. File transfers paused entirely if bandwidth < 2 Mbps. |

**Pacing algorithm:**

```
Every send_interval (1ms default):
  1. Drain P0 (emergency) — unlimited, immediate.
  2. Drain P1 (input) — send all queued input events.
  3. Drain P2 (cursor) — send latest cursor position (coalesce if multiple queued).
  4. Drain P3 (audio) — send next audio frame if due (20ms cadence).
     If audio frame was delayed by >5ms, log metric (liquide_audio_schedule_delay_seconds).
  5. Calculate remaining_budget = estimated_bandwidth * send_interval - bytes_sent_p0_p3.
  6. Drain P4 (control) — send up to min(remaining_budget * 0.1, queued_control_bytes).
  7. Drain P5 (video/tile) — send up to remaining_budget * 0.85 worth of video/tile data.
     If send queue > 80% full: signal encoder to reduce quality/FPS (backpressure).
  8. Drain P6 (bulk) — send up to remaining_budget * 0.05.
     If bandwidth < 2 Mbps: pause all bulk transfers.
```

#### Per-Channel Queue Architecture

Each logical channel has a **dedicated bounded queue** with type-specific parameters rather than sharing a single FIFO. Queue isolation ensures that a stalled bulk transfer cannot block audio or input processing.

| Channel | Queue Type | Capacity | Coalescing | Overflow Policy |
|---------|-----------|----------|------------|-----------------|
| Emergency (0x01) | Lock-free SPSC | 16 messages | None | Drop oldest (should never overflow) |
| Input (0x50) | Lock-free SPSC | 256 events | Mouse-move: latest-wins | Latest-wins for mouse moves; keyboard and IME events are NEVER dropped |
| Cursor (0x11) | Single-slot | 1 position | Always latest | Overwrite — unreliable, most recent position is the only one that matters |
| Audio (0x20/0x21) | Ring buffer | 200ms (10 frames @ 20ms Opus) | None | Drop oldest frame; increment `liquide_audio_underrun_total` |
| Control (0x00) | Bounded MPSC | 64 messages | None | Block sender until space available (reliable, ordered) |
| Video (0x10) | Double buffer | 2 frames | Skip older frame | Drop older frame; request new keyframe from encoder |
| Tile (0x12) | Batch queue | 4 batches | Merge overlapping tile updates | Drop oldest batch; force keyframe for tiles covered by dropped batch |
| Bulk (0x30–0xF0) | Bounded MPSC | 1 MB per sub-channel | None | Block sender; resume when drained below low-water mark |

**Queue implementation notes:**
- SPSC (single-producer, single-consumer) queues are lock-free ring buffers — no mutex contention on the hot path.
- The cursor single-slot is an `AtomicU64` (packed x/y) — zero allocation, zero contention.
- Audio ring buffer is pre-allocated at session start (200ms × sample_rate × channels × 2 bytes = ~38 KB for stereo 48 kHz).
- Video double buffer allows the encoder to write the next frame while the transport sends the current one.

**Metrics:**
- `liquide_channel_queue_depth{channel="..."}` — current queue occupancy (gauge).
- `liquide_channel_queue_drops_total{channel="..."}` — dropped messages due to overflow (counter).

**Starvation guarantees:**

| Stream | Guarantee |
|--------|-----------|
| Audio | Never starved. If total bandwidth < audio bitrate (typically ~32 kbps), audio still sends, video suspends entirely. |
| Input | Never queued behind video. Worst-case input delay from transport layer: 1ms (one pacing interval). |
| Cursor | Datagram delivery — bypasses TCP/QUIC stream head-of-line blocking. Worst case: cursor update lost (re-sent on next mouse move). |
| Video | Adapts to available bandwidth. Under severe congestion: FPS reduced, resolution reduced, quality reduced. Video is the "shock absorber" of the system. |

#### Buffering Targets & Interactive Latency Budget

The end-to-end latency from user input to visible pixel change is the sum of each pipeline stage. This budget breakdown defines the maximum allowable contribution of each stage:

| Stage | Budget (LAN) | Budget (WAN, 50ms RTT) | Notes |
|-------|-------------|----------------------|-------|
| Client input capture | ≤1ms | ≤1ms | OS event loop to transport send |
| Transport (client → server) | ≤1ms | RTT/2 | Network transit |
| Server input injection | ≤1ms | ≤1ms | Transport receive to Wayland event queue |
| Application processing | ≤2ms | ≤2ms | App reacts and commits surface |
| Compositor render | ≤5ms | ≤5ms | Scene graph update + tile/frame composite |
| Encode | ≤5ms | ≤5ms | Video or tile encode |
| Transport (server → client) | ≤1ms | RTT/2 | Network transit |
| Client decode | ≤3ms | ≤3ms | Video/tile decode |
| Client present | ≤1ms | ≤1ms | Buffer swap / display |
| **Total (excluding RTT)** | **≤20ms** | **≤24ms** | |
| **Total (including RTT)** | **≤21ms** | **≤74ms** | Must fit SLO from spec-performance.md §2.1 |

**Per-Channel Jitter Buffer Targets:**

| Channel | Jitter Buffer | Rationale |
|---------|--------------|-----------|
| Video (0x10) | 0ms (no buffer) | Interactive — present immediately on decode. Dropped frames preferred over buffering. |
| Audio (0x20) | 20–200ms (adaptive) | Adaptive jitter buffer absorbs network jitter. See §7e adaptive jitter algorithm. |
| Cursor (0x11) | 0ms (no buffer) | Latest-wins — present immediately. Stale positions are worse than jitter. |
| Tiles (0x12) | 0ms (no buffer) | Reliable transport handles ordering. Present on decode. |
| Input (0x50) | 0ms (fire-and-forget) | Reliable transport, no client-side buffering needed. |

**Constraint:** the sum of buffering delays + network RTT must remain within the input-to-photon SLO defined in [spec-performance.md](spec-performance.md) §2.1. For LAN (1ms RTT): p50 < 16ms, p99 < 25ms. For WAN (50ms RTT): p50 < 70ms, p99 < 120ms.

**Observability counters:**

| Metric | Type | Description |
|--------|------|-------------|
| `liquide_transport_pacing_interval_seconds` | histogram | Actual pacing interval (should be ~1ms) |
| `liquide_transport_priority_bytes_total` | counter (label: `priority`) | Bytes sent per priority level |
| `liquide_transport_priority_starvation_total` | counter (label: `priority`) | Times a priority level was starved (budget exhausted by higher priority) |
| `liquide_transport_audio_schedule_delay_seconds` | histogram | Audio frame scheduling delay beyond target |
| `liquide_transport_input_queue_delay_seconds` | histogram | Time input events spend in send queue |
| `liquide_transport_video_backpressure_active` | gauge | 1 when video backpressure is active, 0 otherwise |

#### Network Condition Test Harness

The benchmark and CI pipeline includes a network condition test harness that injects realistic network impairments to validate protocol resilience and SLO compliance.

**Implementation:** `tc` (Linux traffic control) + `netem` wrapper invoked by `liquide-bench --network <profile>`. The harness configures ingress and egress shaping on the loopback interface or between test namespaces.

**Test Scenarios:**

| Scenario | Configuration | Success Criteria |
|----------|--------------|-----------------|
| Steady-state 1% loss | `netem loss 1%` | Input-to-photon p99 < WAN SLO. No visible corruption >500ms. |
| Steady-state 3% loss | `netem loss 3%` | Session remains usable. Degradation ≤ N3. Audio PLC masks most losses. |
| Steady-state 5% loss | `netem loss 5%` | Session functional at reduced quality. Degradation ≤ N4. |
| Burst loss (10% for 2s) | `netem loss 10% duration 2s` | Recovery to normal within 3s of burst end. Keyframe generated within 1 frame period. |
| Jitter sweep (0→100ms) | `netem delay 50ms 50ms distribution normal` | Audio jitter buffer adapts. No underruns after first 2s. |
| Bandwidth ramp down (100→2 Mbps) | `tc rate 100mbit; ramp to 2mbit over 10s` | ABR reduces quality smoothly. No send queue overflow. |
| 500ms network blackout | `netem loss 100% duration 500ms` | Reconnect overlay shown within 2s. Session recovers on first attempt. |
| 5% packet reorder | `netem reorder 5% gap 3` | No protocol errors. Reliable channels unaffected. Unreliable channels handle gracefully. |

Each scenario is run against the workload profiles defined in [spec-performance.md](spec-performance.md) §3.1. Results are compared against the corresponding SLO targets from [spec-performance.md](spec-performance.md) §2.1 and the network emulation profiles in §3.3.

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

### Corporate & Restrictive Network Environments

Enterprise networks impose constraints that consumer-oriented protocols rarely encounter. LiquiDE treats these as first-class deployment targets.

#### HTTP Proxy Traversal

Many corporate networks require all outbound traffic to pass through an HTTP CONNECT proxy. LiquiDE handles this at the transport layer:

| Proxy Type | Mechanism | LiquiDE Behavior |
|------------|-----------|-----------------|
| HTTP CONNECT (no auth) | `CONNECT host:port HTTP/1.1` | Client sends CONNECT to proxy, then upgrades to TLS. Works for TLS/TCP and WebSocket. |
| HTTP CONNECT (basic auth) | `Proxy-Authorization: Basic <creds>` | Client reads proxy credentials from config or OS credential store (Windows: WinHTTP, macOS: Keychain, Linux: `http_proxy` env). |
| HTTP CONNECT (NTLM/Negotiate) | SSPI (Windows) / GSSAPI (Linux/macOS) | Client performs NTLM/Kerberos authentication with proxy via platform APIs. |
| PAC file | JavaScript-based proxy selection | Client evaluates PAC file to determine correct proxy for the target host. PAC URL sourced from OS settings or config. |
| WPAD | Auto-discovery via DHCP/DNS | Client discovers PAC file via WPAD protocol when `proxy.mode = "auto"`. |
| SOCKS5 | SOCKS5 protocol | Supported. Username/password auth supported. |
| Transparent (intercepting) | No client action needed | Works if TLS inspection is not active. See below for TLS inspection. |

Proxy configuration:

```toml
# Client config (config.toml)
[proxy]
mode = "auto"                        # auto (OS settings/WPAD), manual, none
http_proxy = ""                      # manual: "http://proxy.corp.example:8080"
https_proxy = ""                     # manual: "http://proxy.corp.example:8080"
no_proxy = "localhost,127.0.0.1,*.internal.corp"
socks5_proxy = ""                    # "socks5://proxy.corp.example:1080"
pac_url = ""                         # manual PAC file URL
auth_method = "auto"                 # auto, basic, ntlm, negotiate, none
# Credentials: read from OS credential store by default.
# Can be overridden for headless/scripted deployments:
auth_username = ""
auth_password = ""                   # or use auth_password_env = "PROXY_PASSWORD"
```

When `mode = "auto"`:
- **Windows**: reads proxy settings from `WinHTTP` (system proxy) or Internet Options (per-user proxy).
- **macOS**: reads from `SystemConfiguration` framework (Network Preferences → Proxies).
- **Linux**: reads `http_proxy`, `https_proxy`, `no_proxy` environment variables, or NetworkManager proxy settings via D-Bus.

#### TLS Inspection (SSL Interception)

Corporate environments often deploy TLS-inspecting proxies (Zscaler, Palo Alto, Bluecoat, etc.) that terminate, inspect, and re-encrypt TLS traffic. This breaks certificate pinning and changes the TLS certificate chain.

LiquiDE's behavior:

| Scenario | Detection | Client Behavior |
|----------|-----------|----------------|
| No inspection | Server certificate chains to expected CA | Normal operation |
| TLS inspection (untrusted CA) | Handshake fails — unknown CA | Connection fails. Error message: "TLS certificate verification failed. Your network may be intercepting encrypted traffic. Contact your IT administrator." |
| TLS inspection (trusted system CA) | Handshake succeeds — chain includes system-trusted CA that is not the expected server CA | Connection succeeds. **Warning logged**: "TLS connection established via non-server CA. A TLS-inspecting proxy may be active." |
| TLS inspection (admin-approved) | CA in `tls.inspecting_proxy_cas` list | Connection succeeds. No warning. |

Configuration for environments with TLS inspection:

```toml
[tls]
# Additional CA certificates to trust (PEM format)
# Use this for enterprise internal CAs or TLS-inspecting proxy CAs
additional_ca_file = "/etc/liquide/enterprise-ca.pem"
# Specific proxy CAs that are known and approved (suppresses warnings)
inspecting_proxy_cas = ["/etc/liquide/zscaler-ca.pem"]
# Certificate pinning mode
pinning = "report-only"              # enforce, report-only, disabled
# enforce: reject connections where server cert doesn't match pinned key
# report-only: log warning but allow connection (recommended for enterprise)
# disabled: no pinning checks (for TLS-inspecting environments)
```

**QUIC and TLS inspection**: most TLS-inspecting proxies only support TCP-based protocols. QUIC (UDP-based) is typically blocked or passed through without inspection. When the client detects that QUIC connections fail but TLS/TCP succeeds, it permanently deprioritizes QUIC for that network profile.

#### ALPN Strategy

Application-Layer Protocol Negotiation (ALPN) is used during the TLS handshake to select the LiquiDE protocol:

| ALPN Token | Transport | Notes |
|------------|-----------|-------|
| `liquide/1` | TLS/TCP (native protocol) | Primary ALPN identifier |
| `h3` | QUIC (HTTP/3 framing) | Used when `transport = "quic"`, compatible with HTTP/3 proxies |
| `h2` | TLS/TCP with HTTP/2 framing | Fallback for proxies that only allow HTTP/2 traffic |
| `http/1.1` | WebSocket over HTTPS | Maximum compatibility — every proxy allows this |

**Fallback behavior**: if a proxy or middlebox strips or rejects the `liquide/1` ALPN token, the client renegotiates with `h2` framing. As a last resort, it falls back to WebSocket (`http/1.1` ALPN), which passes through virtually all corporate proxies and firewalls.

The server advertises all supported ALPN tokens. The client sends them in preference order. The first mutually-supported token wins.

#### Connectivity Preflight

Before establishing the full session, the client runs a **connectivity preflight** to diagnose the network environment:

```
Preflight Sequence (< 3 seconds total):
 1. DNS resolve (server hostname)                    → check DNS works
 2. TCP connect to server:port                       → check basic reachability
 3. TLS handshake (observe ALPN, cert chain)        → check TLS, detect inspection
 4. HTTP HEAD /health on gateway (if gateway URL)   → check gateway reachable
 5. QUIC probe (single packet, 1s timeout)          → check UDP/QUIC available
 6. STUN binding request (if web client)            → check NAT type
```

Results are displayed in the connection dialog as a compact status strip:

```
Network: ✓ DNS  ✓ TCP  ✓ TLS (proxy detected)  ✗ QUIC (blocked)
Transport: TLS/TCP via HTTP proxy (proxy.corp.example:8080)
```

If the preflight reveals problems, the client shows a diagnostic panel before attempting the full connection. The preflight results are also logged and included in connection failure reports.

Preflight can be disabled for fast connections: `connection.preflight = false`.

#### Force TCP-Only Mode

For severely restricted networks, administrators can force TCP-only operation:

```toml
[transport]
# Force TCP-only: disables QUIC, UDP, WebRTC TURN-UDP
# Useful for networks that block all UDP traffic
force_tcp = false

# When force_tcp = true, the transport priority becomes:
# 1. TLS/TCP (direct)
# 2. TLS/TCP via HTTP CONNECT proxy
# 3. WebSocket (wss://) via HTTPS proxy
# All UDP-based transports (QUIC, pure UDP, WebRTC TURN-UDP) are disabled.
# WebRTC TURN-TCP and TURNS-TLS remain available for web clients.
```

When `force_tcp = true`:
- QUIC is completely disabled.
- Pure UDP is completely disabled.
- For native clients: TLS/TCP or WebSocket only.
- For web clients: WebSocket signaling + WebRTC with TURN-TCP/TURNS-TLS only (no TURN-UDP).
- Network bandwidth estimation uses TCP-based probing only.
- Congestion control switches to TCP-appropriate algorithms.

#### TCP Transport Tuning

When using TCP as the transport (either via `force_tcp = true` or as a fallback), LiquiDE applies socket-level tuning to optimize for interactive remote desktop:

**Nagle Algorithm Control:**
- **`TCP_NODELAY` is enabled by default** on all interactive channels (P0–P5). The Nagle algorithm batches small writes into full MSS-sized segments, adding up to 200ms of latency on partial sends — unacceptable for input, cursor, and audio traffic that requires sub-millisecond send latency.
- **Optional Nagle for bulk (P6):** file transfers, clipboard sync, and other bulk operations MAY re-enable Nagle (`transport.tcp.nagle_bulk = true`) to improve throughput. When enabled, bulk channel writes are batched into full MSS segments before transmission, improving goodput on high-bandwidth transfers.

**Nagle as RTT-adaptive batching:** Nagle's algorithm flushes pending data when an ACK arrives from the peer — effectively batching small writes with a timer equal to one RTT. On a LAN (1ms RTT), Nagle adds ~1ms of batching delay; on a WAN (100ms RTT), it adds ~100ms. This makes Nagle an **RTT-adaptive batching mechanism** that automatically trades latency for throughput proportional to the link's round-trip time. For bulk P6 transfers, this is desirable: the batching delay is dominated by the network transit time anyway, and the throughput gain from full MSS segments is significant (up to 40× fewer packets for small-write workloads). For interactive channels (P0–P5), where the goal is sub-RTT delivery, Nagle is counterproductive and MUST remain disabled.

**TCP_CORK for Batch Assembly:**
- During tile batch assembly, the socket is **corked** (`TCP_CORK` / `TCP_NOPUSH`) at batch start and **uncorked** at batch end. This ensures that multiple small tile updates within a single batch are coalesced into fewer TCP segments without the latency penalty of Nagle's timer. On platforms where `TCP_CORK` is unavailable (Windows, macOS), the writer thread manually coalesces tile batch data into a single `writev()` / `WSASend()` call.

**Socket Buffer Sizing:**
- Send and receive buffers are auto-tuned to match the bandwidth-delay product of the connection. The transport measures RTT (from QUIC ACKs or TCP timestamp options) and estimated bandwidth, then sets `SO_SNDBUF` and `SO_RCVBUF` to `max(sndbuf_min, min(BDP × 2, sndbuf_max))`.
- Floor: 256 KB (sufficient for 20 Mbps at 100ms RTT).
- Ceiling: 4 MB (prevents excessive kernel memory usage).

**TCP Keepalive:**
- Application-level keepalives (§7a Ping/Pong) are the primary liveness mechanism.
- TCP-level keepalives serve as a backup for detecting half-open connections when the application layer is stalled.

```toml
[transport.tcp]
nodelay = true              # TCP_NODELAY on interactive channels P0-P5 (default: true)
nagle_bulk = false          # Re-enable Nagle for P6 bulk channel only (default: false)
cork_tile_batches = true    # TCP_CORK during tile batch assembly (default: true)
keepalive_idle_sec = 30     # Seconds before first TCP keepalive probe
keepalive_interval_sec = 10 # Seconds between keepalive probes
keepalive_count = 3         # Failed probes before connection considered dead
sndbuf_min = 262144         # 256 KB minimum send buffer
sndbuf_max = 4194304        # 4 MB maximum send buffer
```

#### Network Profile Auto-Detection

The client remembers network conditions per connected network (identified by SSID, gateway MAC, or static config) and auto-applies transport preferences:

```toml
# Stored in client data directory, auto-managed
[[network_profiles]]
identifier = "CorpWiFi-5G"          # SSID or user-assigned name
detected_proxy = true
detected_tls_inspection = true
quic_available = false
preferred_transport = "tls-tcp"
proxy_address = "proxy.corp.example:8080"
last_used = "2025-06-15T14:00:00Z"

[[network_profiles]]
identifier = "HomeWiFi"
detected_proxy = false
detected_tls_inspection = false
quic_available = true
preferred_transport = "quic"
last_used = "2025-06-14T20:00:00Z"
```

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

### Transport I/O Bridge Architecture

The transport I/O subsystem connects logical channel queues to the underlying wire transport (TCP sockets, QUIC streams, UDP datagrams, WebSocket frames) through a set of **bridge threads** that handle multiplexing, framing, and async cancellation.

#### Thread Model

| Thread | Count | Role |
|--------|-------|------|
| **Transport reader** | 1 per connection | Reads raw bytes from the transport, demultiplexes by `channel_id` from the frame header, pushes decoded frames into per-channel receive queues. Uses `tokio::select!` with cancel-safe branches — each read future is cancel-safe (no partial state lost on cancellation). |
| **Transport writer** | 1 per connection | Polls all per-channel send queues in priority order (P0→P6), serializes frames, writes to transport. Implements the pacing algorithm from §7d. Applies TCP_CORK during tile batch assembly (§8d). |
| **Channel workers** | 1 per active channel | Per-channel async tasks that consume from receive queues and produce into send queues. Each task holds a `CancellationToken` — on session teardown, all tasks are cancelled cooperatively. |

#### Writer Thread Scheduling Modes

The transport writer thread operates in one of three **scheduling modes**, switching dynamically based on channel queue state:

| Mode | Trigger | OS Scheduling | Behavior |
|------|---------|---------------|----------|
| **Idle** | All send queues empty for > 50ms | `SCHED_OTHER` (normal), `epoll_wait` / `io_uring_enter` with infinite timeout | Thread parks completely — zero CPU. Woken by queue push notification (eventfd / futex). |
| **Normal** | Any send queue non-empty, P0–P2 queues empty | `SCHED_OTHER`, 1ms pacing tick (timer_fd / `tokio::time::interval`) | Standard pacing algorithm (§7d). Drains queues P0→P6 per tick. |
| **Priority** | P0 (emergency) or P1 (input) queue has pending data | `SCHED_FIFO` or `SCHED_RR` (priority 40), busy-poll with `sched_yield()` between iterations | Sub-100µs drain latency for emergency and input. Writer does not sleep between iterations — it polls P0/P1 queues in a tight loop until drained, then demotes to Normal. |

**Mode transitions:**
- Idle → Normal: any channel worker pushes a frame into a send queue (eventfd notification).
- Normal → Priority: reader thread or channel worker pushes into P0 or P1 queue. Writer is interrupted via an atomic flag (checked at top of each pacing tick).
- Priority → Normal: P0 and P1 queues are empty after drain. Writer releases RT scheduling class.
- Normal → Idle: all queues remain empty for 50ms (configurable: `transport.idle_park_delay_ms`).
- **RT scheduling fallback**: if `SCHED_FIFO` is unavailable (container without `CAP_SYS_NICE`), Priority mode uses `SCHED_OTHER` with `nice -10` and busy-poll. Metric: `liquide_transport_writer_rt_unavailable` (counter, emitted once at session start).
- **CPU isolation**: on systems with ≥ 4 cores, the writer thread is pinned to a dedicated core via `sched_setaffinity` (configurable: `transport.writer_cpu_affinity`). This prevents scheduler jitter from other session threads affecting write latency.

```toml
[transport.writer]
idle_park_delay_ms = 50           # ms before parking writer thread
priority_mode_scheduling = "auto" # "auto" | "fifo" | "rr" | "nice" | "normal"
cpu_affinity = "auto"             # "auto" | specific CPU id | "none"
```

**Metrics:** `liquide_transport_writer_mode{mode="idle|normal|priority"}` (gauge, current mode). `liquide_transport_writer_mode_transitions_total{from="...",to="..."}` (counter).

#### Transport Proxy Layer

The proxy layer bridges transport-specific semantics to a uniform logical channel interface:

| Transport | Proxy Behavior |
|-----------|---------------|
| **TCP (TLS)** | All channels share one TLS connection. Proxy handles length-prefixed framing and head-of-line blocking mitigation via priority preemption (buffer splitting — high-priority frames interrupt mid-write of lower-priority bulk data). |
| **QUIC** | Each priority class maps to a separate QUIC stream. Unreliable channels (cursor, audio on lossy path) use QUIC datagrams (RFC 9221). No head-of-line blocking between channels. |
| **TCP+UDP** | Reliable channels (control, tile, clipboard, input) over TLS/TCP. Latency-sensitive channels (video, cursor, audio) over DTLS/UDP. Proxy manages both sockets and cross-correlates sequence numbers for reconnect consistency. |
| **WebSocket** | Binary frames over WSS. Same channel multiplexing as TCP mode. Proxy adds WebSocket framing overhead (~2–10 bytes per message). |

#### Cancel-Safe Async I/O

All transport read/write operations use `tokio::io::AsyncRead` / `AsyncWrite` with structured concurrency:

- Read operations MUST NOT hold partial parse state across `.await` points — either a complete frame is consumed or the read is retried from the same buffer position. This ensures that `tokio::select!` cancellation never loses data.
- Write operations are atomic at the frame level — a frame is either fully written to the transport buffer or not started.
- On session teardown, the shutdown sequence is: (1) cancel all channel worker tasks via `CancellationToken`, (2) drain remaining send queues with a 500ms deadline, (3) send `SessionEnd` on the control channel, (4) close transport sockets.

#### Virtual Channel Multiplexing

Plugin-defined virtual channels (channel IDs 0xF0+) route through the same bridge infrastructure. Plugin I/O is rate-limited (`plugin.max_channel_bandwidth = 1 Mbps` default) and sandboxed — a misbehaving plugin cannot monopolize the writer thread or starve interactive channels.

#### Send Buffer Pool & Allocation Priority

The writer thread allocates outgoing frame buffers from a **pre-allocated send buffer pool** rather than `malloc`-per-frame. This eliminates allocation jitter and enables strict memory caps on transport-layer buffering.

**Pool configuration:**

| Parameter | Default | Description |
|-----------|---------|-------------|
| `transport.sendbuf_pool_size` | 8 MB | Total pool size per connection |
| `transport.sendbuf_pool_slab_sizes` | `[128, 1024, 8192, 65536]` | Slab size classes (bytes). Frames pick the smallest slab that fits. |
| `transport.sendbuf_pool_reserved_control` | 256 KB | Reserved exclusively for P0–P4 (emergency, input, cursor, audio, control). Data channels (P5–P6) cannot allocate from this reserved region. |

**Allocation priority (normative):**
1. P0–P4 frames (emergency, input, cursor, audio, control) allocate from the **reserved region first**, then from the general pool. Allocation NEVER fails for P0–P4 — if the reserved region and general pool are both exhausted, the writer thread blocks P5–P6 senders and waits for pool reclamation (buffers recycled after transport ACK or send completion).
2. P5 frames (video/tile) allocate from the general pool. If the pool is exhausted, allocation blocks until buffers are recycled. The encoder thread's backpressure signal (§7d step 7) propagates — the encoder pauses frame production when the send buffer pool is >80% utilized.
3. P6 frames (bulk) allocate from the general pool with lowest priority. Under memory pressure (pool >90% utilized), P6 allocation is suspended entirely — bulk transfers pause.
4. **Reclamation**: send buffers are returned to the pool immediately after the OS acknowledges the send (TCP ACK for reliable, send-completion for QUIC/UDP). On QUIC, the pool integrates with the QUIC stack's flow control — buffers are held until the QUIC send window advances.

**Normative rule — control-flush-before-data:** when the general pool hits the 80% utilization threshold, the writer thread MUST drain all P0–P4 queues completely before allocating any buffer for P5–P6 frames. This guarantees that a burst of video/tile data cannot prevent control messages from being sent, even under extreme memory pressure.

**Metrics:**
- `liquide_sendbuf_pool_utilization` (gauge, 0.0–1.0) — current pool utilization.
- `liquide_sendbuf_pool_reserved_utilization` (gauge) — reserved region utilization.
- `liquide_sendbuf_pool_stalls_total{priority="p5|p6"}` (counter) — times allocation blocked waiting for reclamation.

```toml
[transport.sendbuf_pool]
size = 8388608                     # 8 MB total pool
slab_sizes = [128, 1024, 8192, 65536]
reserved_control = 262144          # 256 KB reserved for P0-P4
backpressure_threshold = 0.8       # signal encoder at 80%
suspend_bulk_threshold = 0.9       # suspend P6 at 90%
```

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
- **Tile delta optimization**:
  - XOR delta computation uses SIMD (AVX2/NEON) for popcount and comparison — < 0.1ms per 64×64 tile.
  - Solid-fill detection is fused with the hash pass (zero additional cost).
  - Copy detection uses a hash table of already-sent tile hashes within the current batch.
  - Scroll vector detection piggybacks on compositor damage events (`wl_surface.offset`).

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

#### Audio-Video Synchronization

Audio and video travel on separate channels with independent delivery characteristics (audio: jitter-buffered unreliable; video: lossy unreliable or reliable tiles). When network conditions or CPU pressure cause the video frame rate to drop, explicit A/V sync rules prevent lip-sync drift and maintain perceptual quality.

**Sync Model**

The session maintains a **presentation clock** (microseconds since session start) that both audio and video timestamps reference. The client uses this shared timeline to align audio playback with video frame presentation:

```
Audio pipeline:
  Server capture → Opus encode → transport → client jitter buffer → decode → present at timestamp_us

Video pipeline:
  Server capture → encode → transport → client decode → present at timestamp_us

Sync point: client presentation scheduler aligns audio and video using timestamp_us from each stream.
```

**Drift Tolerance & Correction**

| Drift Range | Classification | Client Behavior |
|------------|----------------|-----------------|
| ±15ms | In sync | No correction. Human perception threshold for lip-sync. |
| 15–40ms audio ahead | Minor drift (audio early) | Delay audio presentation by inserting silence padding (1 frame). Correction is imperceptible. |
| 15–40ms video ahead | Minor drift (video early) | Hold video frame for 1 extra frame period before presenting. Audio continues uninterrupted. |
| 40–120ms | Moderate drift | Gradual correction: adjust audio playout rate by ±2% (time-stretch via Opus decoder resampling). No sudden jumps. Emit `warn`-level metric. |
| 120–500ms | Severe drift | Hard resync: drop video frames to catch up (if video is behind) OR skip audio to catch up (if audio is behind, insert 10ms silence crossfade to mask the skip). Emit `error`-level metric. |
| >500ms | Desync | Treat as stream discontinuity. Client resets presentation clock to the latest video keyframe timestamp. Audio buffer is flushed and refilled. Brief audible glitch is acceptable. Emit `error` metric + trigger `KeyFrameRequest`. |

**Behavior During Video Frame Rate Drop**

When the server reduces video FPS (due to CPU pressure, bandwidth constraints, or idle screen):

| Video FPS | Audio Behavior | Rationale |
|-----------|---------------|-----------|
| 60 → 30 fps | Audio continues at full rate (48kHz). No change. | Audio is independent of video frame rate. 33ms frame intervals are well within sync tolerance. |
| 30 → 15 fps | Audio continues at full rate. Sync correction active if drift exceeds 15ms. | 66ms frame intervals may cause visible drift. Gradual time-stretch keeps sync. |
| 15 → 5 fps (idle) | Audio continues at full rate. Sync correction suspended — audio free-runs. | During idle, there is no meaningful visual content to sync against. |
| 5 → 1 fps (deep idle) | Audio continues if active. Sync disabled. | Audio and video are decoupled during deep idle. |
| 0 fps (screen frozen) | Audio continues. Client shows last frame. Sync is irrelevant. | Audio-only scenario (e.g., music playback on static screen). |
| FPS recovers (e.g., 5 → 60) | Resync within 100ms using gradual correction. No audio interruption. | On activity resume, smooth resync preferred over hard reset. |

**Priority Rules During Congestion**

Audio is priority P3 (above video at P5) in the pacing algorithm (see §7d priority table). During bandwidth contention:

1. **Audio is never dropped to make room for video.** Audio packets are small (~100 bytes per 20ms frame at Opus 64kbps) and are always transmitted.
2. **Video is the shock absorber.** Video FPS and quality are reduced first. Tile batches are throttled. Audio stream remains at full quality.
3. **If bandwidth is insufficient for even audio**, the connection is in a critically degraded state. The client shows a "Poor connection" indicator. Audio bitrate is reduced to minimum (Opus 6kbps mono). Video is paused entirely.

**Audio Jitter Buffer**

The client maintains a jitter buffer to absorb network timing variance:

| Setting | Default | Range | Description |
|---------|---------|-------|-------------|
| `audio.jitter_buffer_ms` | `60` | `20–200` | Target jitter buffer depth |
| `audio.jitter_buffer_adaptive` | `true` | — | Auto-adjust buffer depth based on measured jitter |
| `audio.max_jitter_buffer_ms` | `200` | `60–500` | Maximum buffer depth before dropping oldest packets |

When the jitter buffer overflows (more audio arriving than can be played, e.g., after a network stall recovery), the **oldest** packets are dropped (prefer latency over completeness — a brief audio gap is better than accumulating delay).

When the jitter buffer underflows (no audio arriving), the client plays silence for up to 200ms, then shows a "connection unstable" indicator.

**Configuration**

```toml
[audio.sync]
enabled = true                       # enable A/V sync correction
drift_tolerance_ms = 15              # below this, no correction applied
max_time_stretch_percent = 2         # max audio rate adjustment (±%)
hard_resync_threshold_ms = 120       # above this, hard resync (frame drop/skip)
desync_threshold_ms = 500            # above this, full clock reset
suspend_sync_below_fps = 10          # disable sync correction when video FPS is below this
```

#### Adaptive Jitter Buffer Algorithm

The client audio jitter buffer dynamically adjusts its depth based on observed network conditions:

- **Algorithm**: Exponentially Weighted Moving Average (EWMA) of inter-packet arrival jitter.
- **Formula**: `jitter_target = max(20ms, 2 × EWMA(jitter))`, where EWMA uses α = 0.125 (same as TCP RTT estimation).
- **Minimum buffer**: 20ms (1 Opus frame at 50fps). This floor prevents underruns during stable conditions.
- **Maximum buffer**: 200ms. Beyond this, audio latency is perceptible; the buffer caps and packets are dropped instead.
- **Adjustment rate**: buffer depth changes by at most 5ms per second (smooth transitions to avoid audible artifacts).
- **Metric**: `liquide_audio_jitter_target_ms` (gauge) — current jitter buffer target depth.
- **Metric**: `liquide_audio_jitter_buffer_underrun_total` (counter) — jitter buffer underrun events (audible glitch).

#### Forward Error Correction for Audio

When network packet loss exceeds a threshold, Forward Error Correction (FEC) is activated to mask single-frame losses:

| Loss Rate | FEC Strategy | Overhead | Audible Impact |
|-----------|-------------|----------|---------------|
| < 0.5% | None (PLC handles rare losses) | 0% | Imperceptible |
| 0.5–2% | Opus in-band FEC enabled | ~20% bitrate increase | Masked — FEC repairs single lost frames |
| 2–5% | Opus FEC + previous-frame repeat with 10ms fade | ~20% + minimal CPU | Occasional brief artifact on burst loss |
| > 5% | Opus FEC + aggressive PLC + bitrate reduction | ~30% + reduced quality | Noticeable degradation but continuous audio |

**Packet Loss Concealment (PLC) hierarchy:**
1. **Opus FEC decode**: if the next packet contains FEC data for the lost packet, decode from FEC (best quality).
2. **Opus PLC**: if no FEC available, Opus decoder generates a concealment frame from its internal state (good quality for single-frame loss).
3. **Repeat-last-frame with fade**: for multi-frame loss (>2 consecutive), repeat last decoded frame with exponential volume fade (−6dB per frame). After 5 consecutive losses (100ms), output silence.

#### Clock Drift Correction

Audio capture and playback clocks on different machines drift independently. Without correction, drift accumulates and causes buffer overflow/underflow over long sessions:

- **Detection**: the client measures the offset between its local audio clock and the server's presentation clock using `Ping/Pong` round-trip timestamps. Drift is computed as the rate of change of this offset over a 60-second sliding window.
- **Correction**: micro-resampling — the client's audio output resampler adjusts the sample rate by the drift amount. Maximum adjustment: ±50 ppm (±2.4 samples/sec at 48kHz). This is imperceptible.
- **Threshold**: correction activates only when measured drift exceeds ±10 ppm (below this, natural jitter buffer elasticity absorbs the drift).
- **Metric**: `liquide_audio_clock_drift_ppm` (gauge) — current measured clock drift in parts-per-million.

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

### Media Redirection (Collaboration-Grade RTC)

Remote desktop sessions frequently run collaboration apps (Teams, Zoom, Slack, Google Meet). Routing real-time audio/video through the compositor pipeline (capture → encode → transport → decode → re-encode by RTC app) adds unacceptable latency and double-compression artifacts. **Media redirection** provides a local breakout path.

#### Architecture

```
Without redirection (default):
  [RTC App] → [PipeWire sink] → [Opus encode] → [transport] → [client decode] → [speaker]
               ↑ [PipeWire source] ← [transport] ← [client mic encode]

With local breakout:
  [RTC App] → [PipeWire detects RTC sink] → signals client
  [Client] → creates local loopback: RTC media ↔ client hardware directly
  [Compositor] → RTC app window still rendered normally (screen share path unaffected)
```

#### Local Breakout Detection

PipeWire monitors connected clients. When a client matches the RTC allowlist, the server signals the client to establish a local media breakout:

| Detection Method | Trigger | Action |
|-----------------|---------|--------|
| Application name match | PipeWire `application.name` matches allowlist | Signal client for breakout |
| PulseAudio prop match | `media.role = "phone"` or `media.role = "communication"` | Signal client for breakout |
| Manual trigger | User activates "Optimize for calls" toggle | Force breakout mode |

#### RTC Application Allowlist

| Application | PipeWire Match | Breakout Support |
|-------------|---------------|-----------------|
| Microsoft Teams | `application.name = "msedge"` + `media.role = "communication"` | Audio + camera |
| Zoom | `application.name = "zoom"` | Audio + camera |
| Slack (Huddle) | `application.name = "slack"` + `media.role = "phone"` | Audio only |
| Google Meet | `application.name = "chrome"` + `media.role = "communication"` | Audio + camera |
| WebRTC (generic) | `media.role = "communication"` | Audio (camera optional) |
| Discord | `application.name = "discord"` | Audio + camera |

#### AEC / AGC / NS Responsibility

| Processing | Without Breakout | With Local Breakout |
|-----------|-----------------|-------------------|
| Acoustic Echo Cancellation (AEC) | Server-side (PipeWire plugin) — high latency, degraded quality | **Client-side** — low latency, direct hardware access. Best quality. |
| Automatic Gain Control (AGC) | Server-side | **Client-side** — direct mic access |
| Noise Suppression (NS) | Server-side (RNNoise plugin) | **Client-side** — RNNoise or native OS NS |
| Audio routing | Full round-trip through transport | Direct client hardware ↔ RTC app |

#### Configuration

```toml
[media.rtc]
enabled = true                              # enable RTC local breakout detection
allowlist = ["teams", "zoom", "slack", "meet", "webrtc", "discord"]  # app identifiers
auto_detect = true                          # auto-detect via PipeWire media.role
camera_breakout = true                      # include camera in breakout (not just audio)
fallback_to_remote = true                   # if breakout fails, fall back to standard audio path
```

**Policy notes:** when `media.rtc.enabled = false`, all audio/video flows through the standard compositor pipeline. This is appropriate for high-security environments where all media must be inspected. When enabled, the breakout only affects the media streams — the RTC app's *screen content* is still rendered by the compositor and transmitted via the normal video/tile pipeline.

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

#### USB Redirection Safety Guardrails

USB redirection is powerful but hazardous. An accidental "forward all devices" could expose security keys, authentication tokens, or local storage to the remote server. LiquiDE applies multiple layers of protection to prevent data loss, credential theft, and administrative overhead.

##### Default-Deny Posture

- USB redirection is **disabled by default** at both server and client.
- Even when enabled at the server, the client must also enable it (`usb.enabled = true` in client config).
- The client **never auto-forwards devices**. Each device must be explicitly selected by the user or pre-approved by policy.

##### Client-Side Device UI

When USB redirection is enabled, the client toolbar shows a USB icon. Clicking it opens the **USB Device Manager**:

```
┌─────────────────────────────────────────────┐
│  USB Devices                         [×]    │
│                                             │
│  Available (local):                         │
│  ┌─────────────────────────────────────────┐│
│  │ ⊘ YubiKey 5 NFC (FIDO2)        [Block] ││
│  │   Yubico · VID:1050 PID:0407            ││
│  │   ⚠ Security key — forwarding blocked   ││
│  ├─────────────────────────────────────────┤│
│  │ ○ SanDisk Ultra USB 3.0   [Forward ►]  ││
│  │   SanDisk · VID:0781 PID:5581           ││
│  │   Mass storage · 64 GB                  ││
│  ├─────────────────────────────────────────┤│
│  │ ● HP LaserJet Pro         [Disconnect]  ││
│  │   HP · VID:03F0 PID:2B4A               ││
│  │   Printer · Currently forwarded         ││
│  └─────────────────────────────────────────┘│
│                                             │
│  Forwarded (to remote):                     │
│  • HP LaserJet Pro (printer)                │
│                                             │
│  Policy: mass-storage, printer allowed      │
│  Security keys: auto-blocked by policy      │
└─────────────────────────────────────────────┘
```

Key UI behaviors:
- **Security keys (FIDO2/U2F) are highlighted with a warning** and blocked by default. Users must explicitly acknowledge a confirmation dialog to forward them. Admin policy can hard-block this.
- **YubiKey protection**: devices matching known security key VID/PIDs are classified as `security-key` class regardless of their USB device class descriptor. This prevents a YubiKey in HID mode from being accidentally forwarded as a generic HID device.
- **Confirmation dialog** for forwarding any device: "You are about to forward [Device Name] to the remote session. The remote server will have direct access to this device. Continue?"
- **Auto-disconnect on session end**: all forwarded devices are automatically disconnected when the session disconnects or terminates.

##### Admin Allowlist / Blocklist

Administrators control which device classes and specific devices can be forwarded:

| Rule Type | Scope | Example | Effect |
|-----------|-------|---------|--------|
| `allowed_device_classes` | Server-wide | `["mass-storage", "printer"]` | Only these USB classes can be forwarded |
| `blocked_device_classes` | Server-wide | `["hid", "wireless"]` | These classes are never forwarded |
| `allowed_vid_pid` | Server-wide | `["0781:5581", "03F0:*"]` | Only these VID:PID pairs can be forwarded (wildcard supported) |
| `blocked_vid_pid` | Server-wide | `["1050:*"]` | These VID:PID pairs are always blocked (overrides allow) |
| Policy key | Per-user/group | `usb.enabled = false` | Disable USB for specific users/groups |
| Policy key | Per-user/group | `usb.allowed_device_classes` | Restrict allowed classes per group |

**Resolution order** (most restrictive wins):
1. `blocked_vid_pid` — always wins (hard block).
2. `blocked_device_classes` — class-level block.
3. Policy `usb.allowed_device_classes` (intersection of server + group + user).
4. `allowed_vid_pid` — explicit VID/PID allowlist (if non-empty, only listed devices allowed).
5. `allowed_device_classes` — class-level allow.
6. Client-side user confirmation required for each device.

##### Known Security Key Identification

LiquiDE maintains a built-in list of known security key vendor/product IDs that trigger automatic blocking and warnings:

| Vendor | VID | Products | Classification |
|--------|-----|----------|---------------|
| Yubico | `1050` | All PIDs | `security-key` |
| SoloKeys | `1209` | `5070`, `5071` | `security-key` |
| Feitian | `096E` | FIDO-related PIDs | `security-key` |
| Google (Titan) | `18D1` | `5026`, `5028` | `security-key` |
| Nitrokey | `20A0` | `4287`, `42B1`, `42B2` | `security-key` |

This list is updated with software releases. Administrators can extend it via config:

```toml
[usb.security_key_overrides]
# Additional VID:PID pairs to classify as security keys
additional = ["1234:5678"]
# VID:PID pairs to remove from the security key list (false positives)
exceptions = []
```

##### Audit Events

| Event | Level | Fields |
|-------|-------|--------|
| `usb.device_forwarded` | `info` | `user`, `device_name`, `vid_pid`, `class`, `session_id` |
| `usb.device_disconnected` | `info` | `user`, `device_name`, `vid_pid`, `reason` |
| `usb.device_blocked` | `warn` | `user`, `device_name`, `vid_pid`, `class`, `block_reason` |
| `usb.security_key_forward_attempt` | `warn` | `user`, `device_name`, `vid_pid`, `allowed` |
| `usb.policy_violation` | `warn` | `user`, `device_name`, `vid_pid`, `policy_rule` |

#### USB Redirection Implementation Tiers

USB device redirection "looks easy on paper" but drags in platform-specific driver complexity. LiquiDE explicitly tiers its USB support to ship a reliable product at each tier before expanding scope.

| Tier | Name | Transport | Server-Side | Client Requirements | Ship Target |
|------|------|-----------|-------------|--------------------|----|
| **Tier 1** | Mass storage via file transfer | File transfer channel (`0x31`) | Virtual mount point (FUSE or bind mount) | File picker only — no raw block device access | v1.0 |
| **Tier 2** | Smart card via PC/SC APDU | Dedicated USB channel (`0x40`), APDU-level | `pcscd` virtual reader (PC/SC IPC) | PC/SC middleware on client (Windows: built-in, macOS: built-in, Linux: pcsclite) | v1.0 |
| **Tier 3** | Full USB/IP | Dedicated USB channel (`0x40`), device-level | Linux kernel VHCI (`usbip_host`) | Platform-specific USB/IP driver (Linux: kernel module, macOS: VirtualHere or similar, Windows: USB/IP driver) | v1.2+ |

**Tier 1: Mass Storage via File Transfer**

Instead of forwarding raw USB block devices (which exposes the server to filesystem exploits and requires kernel driver trust), Tier 1 treats mass storage as a **file transfer** operation:

- Client detects USB mass storage device insertion.
- Client lists files on the mounted device via native platform APIs.
- User selects files to upload → standard file transfer channel.
- Server mounts transferred files into a user-visible directory (e.g., `~/USB/<device-name>/`).
- **No raw block device access**, no `mount` on the server, no trusted filesystem parsing of untrusted media.
- Sufficient for 95% of use cases (user wants to copy files from a USB drive to the remote session).

**Tier 2: Smart Card via PC/SC APDU Forwarding**

Smart card redirection (PIV, CAC, FIDO2, PKI cards) uses APDU-level forwarding, not raw USB:

```
Client                                          Server
  │                                                │
  │  PC/SC middleware (native)                     │  pcscd (virtual reader)
  │  ├── SCardEstablishContext()                   │  ├── virtual PC/SC IPC socket
  │  ├── SCardConnect() to local reader            │  ├── presents virtual reader to apps
  │  │                                              │  │
  │  │  APDU forwarding (via USB channel 0x40):    │  │
  │  │  Client sends: SCardTransmit(APDU_cmd) ──► │  │  SCardTransmit(APDU_cmd) → virtual card
  │  │  Client receives: ◄── APDU_response         │  │  APDU_response ← virtual card
  │  │                                              │  │
  │  └── SCardDisconnect()                         │  └── reader removed
```

Benefits over full USB/IP for smart cards:
- **No kernel driver required** on client or server — PC/SC operates entirely in userspace.
- **Cross-platform**: Windows, macOS, and Linux all have native PC/SC stacks.
- **Secure**: only APDU commands cross the wire — no raw USB descriptors, no device enumeration attacks.
- **Low bandwidth**: APDU commands are tiny (typically <256 bytes per exchange).
- Smart card PIN is entered locally (client-side PIN pad support) when possible, never transmitted in cleartext.

**Tier 3: Full USB/IP (Raw Device)**

Full USB/IP device forwarding for devices that cannot be abstracted at a higher level (lab equipment, custom HID devices, hardware tokens without PC/SC support):

| Platform | Client Driver | Server Side | Status |
|----------|--------------|-------------|--------|
| Linux | Kernel `usbip` module (upstream) | Kernel VHCI driver | Supported, well-tested |
| Windows | USB/IP project driver (signed) or VirtualHere | Kernel VHCI | Requires third-party driver installation |
| macOS | VirtualHere or custom kext/dext | Kernel VHCI | Limited — Apple System Extensions required, notarization complexity |

> **Warning**: Tier 3 full USB/IP is inherently risky. A forwarded USB device has the same kernel attack surface as a locally-plugged device. LiquiDE mitigates this by running the VHCI import inside the session's cgroup/namespace jail, but kernel bugs in USB class drivers can still be exploited. Tier 3 SHOULD be used only when Tier 1/2 alternatives are insufficient.

**Tier Configuration**

```toml
[usb]
enabled = false
tier = "auto"                          # auto, 1, 2, 3
# auto: uses highest tier supported by both client and server
# 1: file transfer only (no raw USB)
# 2: file transfer + PC/SC smart card APDU
# 3: file transfer + PC/SC + full USB/IP

[usb.smartcard]
enabled = true                         # enable PC/SC APDU forwarding (Tier 2)
pin_entry = "client-side"              # client-side, server-side
apdu_timeout_ms = 5000                 # timeout for individual APDU exchanges
max_readers = 4                        # max concurrent smart card readers
```

#### USB Device Broker Isolation

USB/IP device forwarding in Tier 3 mode runs through a dedicated broker process (`liquid-usb-broker`) that is isolated from the session:

| Property | Value |
|----------|-------|
| Process | `liquid-usb-broker` (dedicated systemd service) |
| User | `DynamicUser=yes` (ephemeral UID, no home) |
| Namespace | PID + mount + IPC namespaces isolated from session |
| Network | `PrivateNetwork=yes` — broker communicates only via Unix socket |
| Filesystem | `ProtectHome=yes`, `ProtectSystem=strict`, `ReadOnlyPaths=/` |
| Syscall filter | seccomp-BPF allowlist: `read`, `write`, `ioctl`, `poll`, `mmap`, `close`, `futex`, USB-related ioctls |
| VHCI ownership | Broker owns `/dev/vhci_hcd` exclusively; session has no direct access |
| AppArmor | Profile `liquide-usb-broker` restricts file access to socket + VHCI device |

**Broker ↔ Session Protocol:**

1. Session sends `UsbAttachRequest` via Unix socket (includes VID/PID/class from client).
2. Broker validates against policy (device allowlists from server config).
3. If approved: broker attaches device via VHCI, creates udev-compatible device node, bind-mounts into session's mount namespace.
4. If denied: broker returns `UsbAttachDenied` with reason. Audit event emitted.
5. On session disconnect: broker detaches all devices, removes bind mounts.

**Platform DLP Parity:**

| Platform | DLP Mechanism | LiquiDE Equivalent |
|----------|--------------|-------------------|
| Windows (RDP) | Group Policy: "Removable Storage Access" | `usb.allowed_classes` + `usb.blocked_devices` |
| macOS (ARD) | MDM profiles: "Restrict USB" | `usb.enabled = false` or per-class/per-device policies |
| Citrix/VMware | USB device filtering by VID/PID/class | Tier-based filtering + device broker isolation |

Cross-references: [spec-threat-model.md](spec-threat-model.md) T-24 through T-28, [spec-system.md](spec-system.md) §6.5a USB Broker Service.

### Device Redirection Channel

LiquiDE provides a **unified device redirection channel** for forwarding peripheral devices from the client to the session. Rather than assigning a separate channel ID per device class, device traffic is multiplexed over a single logical channel using **typed sub-protocols** — similar in spirit to Microsoft's RDPDR (Remote Desktop Protocol Device Redirection) but with a modern CBOR-based wire format.

#### Channel Assignment

Device redirection shares the USB/IP channel (`0x40`) and uses a sub-protocol discriminator in the message header to route traffic to the correct device class handler.

#### Device Classes & Sub-Protocols

| Class ID | Device Class | Sub-Protocol | Transport | Notes |
|----------|-------------|-------------|-----------|-------|
| `0x01` | **Filesystem** | FUSE-over-wire (open/read/write/stat/readdir) | Reliable, ordered | Client directories mounted via FUSE in session namespace |
| `0x02` | **Printer** | IPP/PDF job submission | Reliable, ordered | Uses CUPS virtual printer on server (see §Remote Printing) |
| `0x03` | **Serial port** | Byte-stream relay (open/close/read/write/ioctl) | Reliable, ordered | Virtual serial device in session (`/dev/ttyVUSB0`) |
| `0x04` | **Smart card** | PC/SC APDU relay (see §USB Tier 2) | Reliable, ordered | Virtual PC/SC reader via `pcscd` IPC |
| `0x05` | **Raw USB** | USB/IP device-level (see §USB Tier 3) | Reliable, ordered | Kernel VHCI import. Policy-gated. |
| `0x06–0x0F` | Reserved | — | — | Future device classes |
| `0x10–0xFF` | Vendor | Vendor-defined | Per-vendor | Negotiated via `Capabilities` with `vendor.<id>.device_class` |

#### Message Structure

All device redirection messages share a common envelope:

```cddl
DeviceRedirectionMsg = {
    0: uint,                         ; class_id (device class, see table above)
    1: uint,                         ; device_instance (client-assigned, unique per class)
    2: uint,                         ; pdu_type (class-specific PDU type code)
    3: bstr,                         ; payload (class-specific CBOR or raw bytes)
    ? 4: uint,                       ; request_id (for request/response correlation)
}
```

#### Capability Announcement

Device classes are negotiated per-session via the `Capabilities` message:

```cddl
; Example: client announces filesystem + printer + smartcard
Capabilities = {
    action: "advertise",
    capabilities: {
        "device_redirect": true,
        "device_redirect.filesystem": { version: 1, max_open_files: 256 },
        "device_redirect.printer": { version: 1, ipp_version: "2.0" },
        "device_redirect.smartcard": { version: 1, max_readers: 4 },
    }
}
```

The server confirms only the classes that policy allows and the session supports. Unconfirmed classes are unavailable.

#### Filesystem Sub-Protocol (Class 0x01)

Client-local directories are exposed to the session via a **FUSE filesystem** that proxies I/O operations over the device channel:

| PDU Type | Name | Direction | Description |
|----------|------|-----------|-------------|
| `0x01` | `FsOpen` | S → C | Open file (path, flags, mode) |
| `0x02` | `FsOpenResult` | C → S | File handle or error |
| `0x03` | `FsRead` | S → C | Read (handle, offset, length) |
| `0x04` | `FsReadResult` | C → S | Data or error |
| `0x05` | `FsWrite` | S → C | Write (handle, offset, data) |
| `0x06` | `FsWriteResult` | C → S | Bytes written or error |
| `0x07` | `FsStat` | S → C | Stat (path) |
| `0x08` | `FsStatResult` | C → S | File attributes |
| `0x09` | `FsReadDir` | S → C | List directory entries |
| `0x0A` | `FsReadDirResult` | C → S | Entry list |
| `0x0B` | `FsClose` | S → C | Close file handle |
| `0x0C` | `FsNotify` | C → S | File change notification (inotify-style) |

**Security**: the client enforces access control — the server can only access paths that the user has explicitly shared. Path traversal beyond shared roots is rejected by the client. DLP policy on the server side can restrict read/write access, block specific file extensions, or limit transfer sizes.

#### Printer Sub-Protocol (Class 0x02)

Integrates with the Remote Printing architecture (§Remote Printing). The device channel carries:
- `PrinterAnnounce`: client advertises available local printers (name, capabilities, driver).
- `PrintJobSubmit`: server sends PDF print job to client.
- `PrintJobStatus`: client reports job status (spooling, printing, complete, error).
- `PrinterRemoved`: client printer disconnected.

This replaces the ad-hoc print-over-file-transfer path for native clients that support the device redirection channel. RDP clients continue using standard RDPDR printer sub-channel.

#### Serial Port Sub-Protocol (Class 0x03)

Byte-stream relay for serial devices (RS-232, USB-serial adapters):
- `SerialOpen`: open port (baud, parity, stop bits, flow control).
- `SerialData`: bidirectional byte stream chunks.
- `SerialIoctl`: control signals (DTR, RTS, break).
- `SerialClose`: close port.

The session sees a virtual serial device (`/dev/ttyVUSB<n>`) created by a userspace serial emulator that proxies to the device channel.

### Remote Printing

LiquiDE supports printing from remote sessions to client-local or network printers. The printing pipeline uses a **PDF-based architecture** — all print jobs are converted to PDF on the server and delivered to the client or a network print queue.

#### Printing Modes

| Mode | Description | Print Path | Use Case |
|------|-------------|-----------|----------|
| **Client redirect** (default) | Print jobs are sent to the client, which prints to a local printer | Server → PDF → Client → Local CUPS/Windows print queue | User's physical desk printer |
| **Network direct** | Session prints directly to a network printer (CUPS/IPP) | Server → CUPS → Network printer | Shared office printer on same LAN as server |
| **PDF download** | Print job is converted to PDF and offered as a file download to the client | Server → PDF → File transfer channel → Client saves file | Review before printing, printing not available |
| **Admin print queue** | Jobs are queued for administrator-managed printers (department printers, managed fleet) | Server → CUPS → Admin queue | Managed printing environment |

#### Architecture

```
Application prints (CUPS client API)
    │
    ▼
CUPS server (per-session, in-session namespace)
    │
    ├── cups-pdf backend → PDF file
    │       │
    │       ├── [Client redirect] → Print channel → Client → Local printer
    │       ├── [PDF download]   → File transfer channel → Client saves .pdf
    │       └── [DLP check]      → Policy engine evaluates job metadata
    │
    └── ipp backend → Network printer (direct mode)
```

Each session runs a lightweight CUPS instance (socket-activated, in the session's mount namespace) that intercepts print requests from applications. The CUPS instance is configured with virtual printers that correspond to the active printing mode.

#### Per-Session CUPS Socket Isolation

Applications find the CUPS server via the `CUPS_SERVER` environment variable or the default socket path `/run/cups/cups.sock`. LiquiDE ensures each session's applications talk to their own CUPS instance through the following mechanism:

**Namespace strategy:**

1. `liquid-session` creates a private mount namespace for the session process tree (`unshare(CLONE_NEWNS)`).
2. Within that namespace, a bind mount overlays the standard CUPS socket path:
   ```
   bind-mount: /run/liquide/sessions/<session-id>/cups.sock → /run/cups/cups.sock
   ```
3. The per-session CUPS scheduler (`cupsd`) listens on `/run/liquide/sessions/<session-id>/cups.sock`.
4. All child processes (applications, shell) inherit the mount namespace. When they open `/run/cups/cups.sock` (the default CUPS path), they transparently connect to the session-local CUPS instance.

**Environment injection (belt-and-suspenders):**

In addition to the bind mount, `liquid-session` sets:
```bash
CUPS_SERVER=/run/liquide/sessions/<session-id>/cups.sock
```
in the session environment. This covers applications that use `CUPS_SERVER` directly (e.g., some toolkits) and handles edge cases where mount namespace propagation interacts with Flatpak sandboxes.

**Flatpak applications:**

Flatpak apps with the `cups` socket permission (`--socket=cups`) get access to the session's CUPS socket via the portal or a bind mount into their sandbox at the standard path.

**CUPS instance lifecycle:**

| Event | Action |
|-------|--------|
| Session start | Socket path created. CUPS instance is socket-activated (not started until first print operation). |
| First print job | systemd socket activation starts `cupsd` for this session. |
| Session idle (no jobs for 5 min) | CUPS instance exits (socket activation will restart it on next job). |
| Session end | Socket removed. CUPS instance terminated. Spool directory cleaned (see §Print Spool Hardening). |

#### Client Printer Discovery

When a client connects, it advertises its available local printers:

| Field | Description |
|-------|-------------|
| `name` | Printer display name (e.g., "HP LaserJet Pro M404") |
| `driver` | PPD identifier or IPP Everywhere capability set |
| `capabilities` | Duplex, color, paper sizes, stapling |
| `default` | Whether this is the client's default printer |

The server creates a virtual CUPS printer for each client-advertised printer. Applications see these printers in the standard print dialog.

**Native client**: enumerates printers via platform API:
- **Windows**: Win32 `EnumPrinters` API.
- **macOS**: CUPS API (macOS uses CUPS natively).
- **Linux**: CUPS API (`cupsEnumDests` / `cupsGetDests2`).

**Web client**: printing is PDF-download-only. The web client cannot access local printers directly from the browser. The user saves the PDF and prints from their local system.

**RDP client**: printer redirection via the standard RDPDR printer sub-channel.

#### PDF Generation

All print jobs pass through a PDF conversion step:

- **Backend**: `cups-pdf` (or equivalent CUPS backend that renders to PDF).
- **PDF version**: PDF 1.7 (ISO 32000-1). Sufficient for all standard office printing needs.
- **PostScript input**: converted via Ghostscript or `pdftocairo`.
- **Raster input**: converted via `cups-filters` (cupsRasterToXyz → PDF).
- **Maximum job size**: configurable, default 100 MB per job.
- **Temporary storage**: PDF files are written to session-local tmpfs (RAM-backed), never to persistent disk. Cleared immediately after delivery or on session end.

#### Data Loss Prevention (DLP) Integration

Print jobs can be subject to policy-based DLP inspection:

```toml
[printing.dlp]
enabled = false
# When enabled, print jobs are inspected before delivery.
# inspection_mode determines what happens during inspection:
inspection_mode = "block-and-notify"   # block-and-notify, log-only, quarantine
# Inspection hook: external program or WASM plugin that receives PDF metadata
# and returns allow/deny decision.
inspection_hook = ""                    # path to script or plugin ID
# Metadata sent to hook: job title, username, printer name, page count, file size.
# The PDF content itself is NOT sent to the hook by default (performance).
# Set content_inspection = true to also provide PDF content (slower).
content_inspection = false
# Blocked job message shown to user:
block_message = "This print job was blocked by your organization's data protection policy."
```

DLP inspection metadata:

| Field | Description |
|-------|-------------|
| `job_id` | Unique print job ID |
| `user` | Session user |
| `printer` | Target printer name |
| `job_title` | Document title (from application) |
| `pages` | Page count |
| `file_size_bytes` | PDF size |
| `color` | Whether job uses color |
| `duplex` | Whether job uses duplex |
| `timestamp` | Job submission time |

#### Audit Events

All print jobs generate audit entries:

| Event | Level | Fields |
|-------|-------|--------|
| `print.job_submitted` | `info` | `user`, `printer`, `job_title`, `pages`, `size_bytes`, `mode` |
| `print.job_completed` | `info` | `user`, `printer`, `job_id`, `delivery_mode`, `duration_ms` |
| `print.job_blocked` | `warn` | `user`, `printer`, `job_title`, `reason`, `dlp_rule` |
| `print.job_failed` | `warn` | `user`, `printer`, `job_id`, `error` |
| `print.client_printer_added` | `info` | `user`, `printer_name`, `capabilities` |

#### Configuration

```toml
# Server config (server.toml)
[printing]
enabled = true
default_mode = "client-redirect"       # client-redirect, network-direct, pdf-download
max_job_size_mb = 100
max_concurrent_jobs = 10
pdf_temp_dir = ""                      # empty = session tmpfs (default, recommended)
retain_pdf_seconds = 300               # how long to keep PDF after delivery (for reprint)

# Client redirect settings
[printing.client_redirect]
enabled = true
auto_discover_client_printers = true

# Network direct settings
[printing.network_direct]
enabled = false
cups_server = "localhost:631"          # network CUPS server
allowed_printers = []                  # empty = all printers on CUPS server

# PDF download settings
[printing.pdf_download]
enabled = true                         # always available as fallback

# DLP (see above)
[printing.dlp]
enabled = false
```

#### Print Spool Hardening

The per-session CUPS instance is a potential DoS vector — a misbehaving application or user can submit arbitrarily many large print jobs, exhausting tmpfs, memory, or disk. LiquiDE applies hard limits to contain print spool abuse.

**Resource Limits**

| Resource | Default | Max | Enforcement |
|----------|---------|-----|-------------|
| Max concurrent jobs per session | 10 | 50 | CUPS rejects further jobs with `server-error-busy`. Application sees "printer busy." |
| Max job size (single job) | 100 MB | 500 MB | CUPS backend rejects oversize jobs before writing to spool. `print.job_blocked` audit event. |
| Max total spool size per session | 200 MB | 1 GB | Session-level tmpfs quota. When 90% full, new jobs are rejected. When 100%, oldest undelivered job is evicted with warning. |
| Max page count per job | 500 | 5000 | Estimated at PDF generation time. Oversize jobs blocked. |
| Max jobs per user per hour | 50 | 200 | Rate limiting at CUPS backend. Prevents scripted job flooding. |

**Spool Cleanup**

| Trigger | Action |
|---------|--------|
| Job delivered to client or network printer | PDF deleted from spool immediately (or after `retain_pdf_seconds` if reprint is configured). |
| Job delivery fails (client disconnect) | PDF retained for `retain_pdf_seconds` (default 300s), then deleted. Job re-queued on reconnect if within retention window. |
| Session disconnect (graceful) | All pending print jobs held for 5 minutes. If client reconnects (session resume), jobs resume delivery. After timeout, jobs are deleted. |
| Session termination (logout/crash) | All spool contents deleted immediately. No orphaned PDFs. |
| Session idle timeout | Spool cleaned as part of session teardown. |
| Periodic cleanup | Every 60 seconds, the spool watchdog scans for orphaned files (no corresponding CUPS job). Orphans older than 10 minutes are deleted. |

**Configuration**

```toml
[printing.spool]
max_concurrent_jobs = 10
max_job_size_mb = 100
max_spool_size_mb = 200
max_pages_per_job = 500
max_jobs_per_hour = 50
retain_pdf_seconds = 300                # keep delivered PDFs for reprint
spool_watchdog_interval_sec = 60        # orphan scan interval
spool_tmpfs = true                      # always use tmpfs (RAM), never persistent disk
```

> **Operational note**: The `spool_tmpfs = true` default is critical. Print spool data MUST NOT be written to persistent disk unless the administrator explicitly configures a disk-backed spool directory. Persistent spool introduces data-at-rest risk (print jobs may contain sensitive documents) and post-session cleanup complexity.

---

## 11) Clipboard & Data Channels

### Dedicated Transport Channels
LiquiDE uses **separate, dedicated transport channels** for different data types. Each channel operates independently with its own QoS, priority, and flow control:

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
| **Emergency** | Highest | Reliable | Crash info, supervisor heartbeat, log streaming (see [spec-protocol-formal.md](spec-protocol-formal.md) §9) |

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

#### MIME Type Mapping Rules

Clipboard data is exchanged with explicit MIME types. The conversion rules are:

| Source Format | Wire MIME Type | Notes |
|--------------|---------------|-------|
| Plain text | `text/plain;charset=utf-8` | Always available |
| HTML rich text | `text/html` | Sanitized: scripts removed, styles inlined |
| RTF | `text/rtf` | Passed through unchanged |
| PNG image | `image/png` | Preferred image format |
| JPEG image | `image/jpeg` | Lossy — only if source is JPEG |
| BMP image | `image/bmp` | Converted to PNG on wire if `convert_bmp = true` |
| SVG image | `image/svg+xml` | Sanitized (same rules as avatar SVG, see §13) |
| URI list | `text/uri-list` | One URI per line, `\r\n` separated |
| File list | `application/x-liquide-file-list` | Maps to file transfer channel (see below) |
| Custom | `application/octet-stream` | Opaque binary — passed through if policy allows |

When a clipboard offer contains multiple formats, the receiver requests in preference order: `text/html` > `text/plain` for text; `image/png` > `image/jpeg` for images. Applications that offer multiple representations should include all of them.

#### Size Limits & Chunking

- Maximum clipboard item size: `max_size_bytes` (default: 10 MB, configurable).
- Items exceeding 64 KB are transferred using **chunked transfer**:
  1. Sender sends `ClipboardOffer` with `size_hint` (total bytes if known, 0 if unknown).
  2. Receiver sends `ClipboardRequest` for the desired MIME type.
  3. Sender transmits `ClipboardData` messages with sequential chunk data (max 32 KB per chunk).
  4. Sender transmits `ClipboardDataEnd` with total size and SHA-256 hash.
  5. Receiver verifies hash and size match.
- **Progress reporting**: for transfers > 256 KB, `ClipboardProgress` messages are sent every 64 KB with `bytes_sent` / `total_bytes`. The client displays a progress indicator.
- **Cancellation**: either side can send `ClipboardCancel` at any time during a chunked transfer. The partial data is discarded.
- **Timeout**: if no chunk is received for 30 seconds, the transfer is cancelled automatically.

#### Filename Sanitization (File List Clipboard)

When clipboard contains file URIs, filenames are sanitized before use:
- Path traversal characters (`..`, leading `/`) are stripped.
- Null bytes and control characters (0x00–0x1F) are removed.
- Filenames exceeding 255 bytes are truncated (preserving extension).
- If the destination filename already exists: `filename (1).ext`, `filename (2).ext`, etc.
- Reserved names on target OS are prefixed (e.g., `CON` → `_CON` on Windows-hosted sessions, though LiquiDE is Linux-native this matters for file transfers to Windows clients).

#### Clipboard History

- LiquiDE maintains a clipboard history ring buffer per session.
- History size: `clipboard_history_size` (default: 25 items, max: 100).
- History stores: MIME type, size, timestamp, source application (if detectable via D-Bus), content hash (SHA-256).
- **Content storage**: text items are stored verbatim up to 1 MB. Image items store a thumbnail (128×128 PNG). Larger items store only metadata (MIME type, size, hash).
- History is accessible via:
  - `Super+V` keyboard shortcut (opens clipboard history overlay).
  - Clipboard history WASM plugin extension point (see §14b).
  - `org.liquide.Clipboard.GetHistory()` D-Bus method.
- History is cleared on session lock if `privacy.clipboard_clear_on_lock = true`.
- History does not survive session restart (in-memory only).

#### Audit Metadata

Every clipboard operation generates an audit event (if audit logging is enabled):

```json
{
  "event": "clipboard_transfer",
  "timestamp": "2025-01-15T16:22:31.123Z",
  "session_id": "s-042",
  "user": "alice",
  "direction": "server_to_client",
  "mime_type": "text/plain;charset=utf-8",
  "size_bytes": 1234,
  "content_hash": "sha256:a1b2c3d4...",
  "source_app": "org.gnome.Terminal",
  "policy_result": "allowed",
  "transfer_duration_ms": 12
}
```

Content is **never** stored in audit logs. Only metadata (MIME type, size, hash) is recorded. This enables compliance auditing without exposing sensitive clipboard content.

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
- Filename conflict resolution: auto-rename with numeric suffix (see clipboard filename sanitization above).
- Transfer progress: reported to client UI for display in drag-and-drop overlay.
- Resumable transfers: if a file transfer is interrupted (network drop), it can be resumed from the last acknowledged byte offset on reconnection.

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

### Input Method Editor (IME) & Text Input

LiquiDE implements full Wayland text-input and input-method protocols for complex script input (CJK, Indic, Arabic, compose sequences, dead keys). The session compositor acts as the text-input hub between Wayland client applications and the active input method engine.

#### Wayland Protocol Support

| Protocol | Version | Role | Description |
|----------|---------|------|-------------|
| `zwp_text_input_v3` | v3 (stable) | Client → Compositor | Applications report text input state (cursor rect, surrounding text, content type). Compositor forwards events to the active input method. |
| `zwp_input_method_v2` | v2 (stable) | Compositor → IME | Input method engine receives keystroke events, produces preedit (composition) strings and commit strings. |
| `zwp_input_method_keyboard_grab_v2` | v2 | IME → Compositor | IME grabs physical keyboard events for filtering/interception before they reach the client application. |
| `zwp_input_popup_surface_v2` | v2 | IME → Compositor | IME creates popup surfaces (candidate window, status indicator) positioned relative to the text cursor. |
| `zwp_virtual_keyboard_v1` | v1 | OSK → Compositor | On-screen keyboard synthesizes virtual key events. |

#### Architecture

```
┌────────────────────────────────────┐
│  Wayland Client Application        │
│  (terminal, text editor, browser)  │
│                                    │
│  zwp_text_input_v3                 │
│  - enable/disable                  │
│  - set surrounding text            │
│  - set content type                │
│  - set cursor rectangle            │
└──────────────┬─────────────────────┘
               │
               ▼
┌────────────────────────────────────┐
│  LiquiDE Compositor (text-input    │
│  manager / seat-level hub)         │
│                                    │
│  Routes text-input state to the    │
│  active input method per seat.     │
│  Manages focus transitions.        │
└──────────┬──────────┬──────────────┘
           │          │
           ▼          ▼
┌─────────────────┐  ┌────────────────────────┐
│  Built-in IME   │  │  External IME Process   │
│  Engine         │  │  (IBus/Fcitx bridge)    │
│                 │  │                          │
│  zwp_input_     │  │  zwp_input_method_v2    │
│  method_v2      │  │  zwp_input_popup_       │
│                 │  │  surface_v2             │
└─────────────────┘  └────────────────────────┘
```

#### Built-in IME Engine

LiquiDE includes a lightweight built-in input method engine that handles the most common input scenarios without requiring external software:

| Feature | Support |
|---------|---------|
| **Dead keys** | Full. Compose sequences via XKB dead key tables. `dead_acute` + `e` → `é`, `dead_diaeresis` + `u` → `ü`, etc. |
| **Compose key** (Multi_key) | Full. XKB Compose file support (`~/.XCompose` or system Compose). `Compose` + `o` + `c` → `©`. |
| **CJK — Pinyin (Chinese Simplified)** | Built-in. Dictionary-based Pinyin → Hanzi conversion. Candidate window with 9 candidates per page. |
| **CJK — Bopomofo (Chinese Traditional)** | Built-in. Zhuyin to Hanzi conversion. |
| **CJK — Romaji/Kana (Japanese)** | Built-in. Romaji → Hiragana → Kanji conversion with candidate selection. |
| **CJK — Hangul (Korean)** | Built-in. Jamo composition (2-Set Dubeolsik, 3-Set Sebeolsik). |
| **Arabic / Hebrew / RTL** | Handled by XKB layout + BiDi algorithm (no IME needed for character input; direction handled by Pango/HarfBuzz). |
| **Indic scripts** | XKB Inscript/phonetic layouts. Complex shaping handled by HarfBuzz at the rendering layer. |

#### External IME Bridge (IBus / Fcitx5)

For users requiring advanced input methods, language-specific dictionaries, or specialized engines (e.g., Mozc for Japanese, libchewing for Traditional Chinese, ibus-rime for Chinese), LiquiDE supports bridging to external IME frameworks via a D-Bus adapter:

| Framework | Bridge Mechanism | Status |
|-----------|-----------------|--------|
| **IBus** | D-Bus: `org.freedesktop.IBus` → `zwp_input_method_v2` adapter | Supported |
| **Fcitx5** | D-Bus: `org.fcitx.Fcitx5` → `zwp_input_method_v2` adapter | Supported |
| **Direct `zwp_input_method_v2` clients** | Native protocol — no bridge needed | Supported |

When an external IME is configured, the built-in engine is deactivated for the configured input types. The external IME process runs inside the session's cgroup and namespace.

```toml
[input.ime]
engine = "builtin"                     # "builtin", "ibus", "fcitx5", "external"
# For ibus/fcitx5:
# engine = "ibus"
# ibus_daemon = true                   # auto-start ibus-daemon in session
# default_method = "pinyin"            # default input method name
```

#### Preedit (Composition) Rendering

When the user is composing text (e.g., typing Pinyin before committing Hanzi), the compositor must render the preedit string inline in the application's text field:

1. **Application-side preedit** (preferred): The application receives `preedit_string` events via `zwp_text_input_v3` and renders the preedit inline using its own text rendering. This is the standard Wayland approach and works well with text editors and terminals.
2. **Compositor-side preedit** (fallback): If the application does not implement `zwp_text_input_v3` (e.g., legacy X11 applications under XWayland), the compositor renders a floating preedit overlay near the cursor position.

Preedit attributes supported:

| Attribute | Description |
|-----------|-------------|
| `underline` | Single underline (default for active composition) |
| `highlight` | Background highlight for the currently converting segment |
| `cursor` | Cursor position within the preedit string |

#### Candidate Window

The IME's candidate selection window is rendered as a popup surface via `zwp_input_popup_surface_v2`:

- Positioned relative to the text cursor (cursor rectangle reported by `zwp_text_input_v3`).
- Styled with the Liquid Glass design language (glass panel, blur backdrop, themed text).
- CSS class: `.liquid-ime-popup`.
- Follows the cursor across screen edges (repositions if clipped).
- Supports mouse and keyboard selection of candidates.
- Page navigation (PageUp/PageDown or arrow keys) for long candidate lists.
- Numbers 1-9 as candidate selection shortcuts.

#### Remote IME Forwarding

When a remote client (native or web) connects, IME events follow this path:

```
Client keyboard event
    │
    ▼
Client sends KeyDown/KeyUp on input channel (§5.8)
    │
    ▼
Server input processing
    │
    ▼
IME intercepts via zwp_input_method_keyboard_grab_v2
    │
    ▼
IME produces preedit + commit events
    │
    ▼
Application receives text via zwp_text_input_v3
    │
    ▼
Application renders updated content
    │
    ▼
Compositor captures damage → encode → transport → client display
```

For the **native client**: key events are forwarded as raw scancodes + keysyms. The IME runs server-side. Preedit and candidates are rendered server-side and streamed as part of the session video/tile output. The client has no awareness of IME state.

For the **web client**: key events during composition are handled differently — see [spec-web-client.md](spec-web-client.md) §7.4. The web client's browser-side IME handles composition locally and sends committed text to the server.

#### Right-to-Left (RTL) Text Support

| Feature | Implementation |
|---------|---------------|
| **BiDi algorithm** | Unicode BiDi Algorithm (UAX #9) applied by HarfBuzz/Pango at the text shaping layer. |
| **Paragraph direction** | Determined by the application (explicit `dir="rtl"` or first-strong-character heuristic). |
| **Cursor movement** | Visual cursor movement (left arrow moves left on screen) is the default. Logical movement available via setting. |
| **Text selection** | Selection follows visual order by default. |
| **Mixed LTR/RTL** | Correctly shaped and ordered within a single text run. |
| **Shell UI direction** | DE shell follows locale direction. Arabic/Hebrew locale → full RTL shell layout (dock on right, status bar text RTL, notification panel on left). |
| **CSS `direction` property** | Honored by the compositor's CSS layout engine for built-in UI elements. |

Configuration:
```toml
[input.bidi]
default_direction = "auto"             # "auto", "ltr", "rtl"
cursor_movement = "visual"             # "visual" or "logical"
shell_direction = "auto"               # "auto" (follows locale), "ltr", "rtl"
```

#### Keyboard Layout Switching

Users can switch between multiple keyboard layouts at runtime:

| Method | Trigger |
|--------|---------|
| Keyboard shortcut | `Super+Space` (default, configurable) |
| Status bar indicator | Click the layout indicator in the status bar |
| Per-window layout | Optional: each window remembers its last-used layout |
| Auto-switch | Optional: switch layout based on text field language hint (`content_type` from `zwp_text_input_v3`) |

Layout switching is instantaneous (XKB keymap switch) and does not require re-negotiation with the client.

```toml
[input.keyboard]
layouts = ["us", "de", "jp"]           # configured layouts
switch_shortcut = "Super+Space"
per_window_layout = false
auto_switch = false
layout_indicator = true                # show in status bar
```

#### Local IME Forwarding Mode (P0)

Users often want to use their **local OS IME** because it is better integrated with their language setup, has custom dictionaries, and provides a familiar experience. LiquiDE supports an explicit local IME forwarding mode where the client performs IME composition locally and sends only committed text to the server.

**Behavior:**

| Step | Actor | Action |
|------|-------|--------|
| 1 | Client | User types into the client window. Client's local OS IME intercepts keystrokes and opens its composition UI. |
| 2 | Client | IME composition happens entirely on the client (preedit rendering, candidate window, conversion). |
| 3 | Client | User commits text (e.g., presses Enter in the IME). |
| 4 | Client | Client sends `TextCommit { text: "committed string", cursor_position: N }` to the server. |
| 5 | Server | Server receives committed text and injects it into the focused application via `zwp_text_input_v3` commit. The server does **not** run a competing IME pipeline for this input. |

**Preedit forwarding (optional):** The client MAY forward preedit state to the server for rendering consistency:

| Message | Description |
|---------|-------------|
| `TextPreedit { text, cursor, attributes }` | Current preedit string (displayed in-line at cursor in remote app). Sent on every composition update. |
| `TextCommit { text, cursor_position }` | Final committed text. Server treats this as authoritative. |
| `TextPreeditClear` | Composition cancelled. Server removes preedit display. |

**Session/policy control:**

```toml
[input.ime]
mode = "server"                        # "server" (default), "client", "auto"
# "server"  — all IME runs server-side (native client forwards raw keys)
# "client"  — client performs composition locally, sends committed text
# "auto"    — client decides based on local IME availability and user preference
```

**When `mode = "client"`:**
- The server disables its IME pipeline for that session's text input.
- Raw key events are NOT forwarded during active composition — only committed text and optional preedit updates.
- Keys not consumed by the IME (shortcuts, navigation) are forwarded normally.

**When `mode = "auto"`:**
- The client negotiates mode during session setup based on whether a local IME is active.
- If the client detects an active IME (e.g., IBus, Fcitx5, macOS Input Sources, Windows IME), it uses client mode.
- If no IME is detected (e.g., US-English-only layout), it falls back to server mode (raw key forwarding).

This mode can be toggled per-session and per-policy. It is opt-in and solves a significant portion of "non-English remote desktop pain" — particularly for CJK languages where the local IME is deeply integrated with the user's OS.

#### Shortcut Conflict Resolution Policy

Remote desktop sessions involve four layers of shortcut handlers that can conflict:

| Layer | Examples | Priority (default) |
|-------|---------|-------------------|
| **Local OS** | Ctrl+Alt+Del (Windows/Linux), Cmd+Space (macOS Spotlight), Alt+Tab (local window manager) | Highest — intercepted before the client |
| **Client application** | Client menu shortcuts, client preferences shortcuts | Second |
| **Remote shell** | Super (launcher), Super+L (lock), Alt+Tab (remote window switch) | Third |
| **Remote application** | Ctrl+S (save), Ctrl+Z (undo), app-specific shortcuts | Lowest — receives whatever is not consumed above |

**Resolution rules:**

1. **Default mode ("smart passthrough")**: The client forwards most shortcuts to the remote session when the session has focus, but the local OS retains certain reserved shortcuts that cannot be intercepted (e.g., Ctrl+Alt+Del on Windows, Cmd+Option+Esc on macOS).

2. **Full passthrough mode ("keyboard lock")**: Enabled via client setting or `Ctrl+Alt+G` toggle. ALL keystrokes (including local Alt+Tab, Super key, etc.) are forwarded to the remote session. The client intercepts nothing except the unlock shortcut itself.

3. **Reserved shortcut list**: The following shortcuts are NEVER forwarded to the remote session (unless full passthrough mode is active):

   | Shortcut | Platform | Reason |
   |----------|----------|--------|
   | Ctrl+Alt+Del | Windows/Linux | OS security attention sequence |
   | Cmd+Option+Esc | macOS | Force Quit |
   | Ctrl+Alt+G | All | Toggle full passthrough mode (client escape hatch) |

4. **Collision handling table:**

   | Local OS Shortcut | Remote Session Wants | Resolution |
   |-------------------|---------------------|------------|
   | Alt+Tab (local WM) | Alt+Tab (remote WM) | Default: local wins. Full passthrough: remote wins. |
   | Super key (local launcher) | Super key (remote launcher) | Default: local wins. Full passthrough: remote wins. |
   | macOS Cmd+C | Ctrl+C (remote app copy) | Client maps Cmd → Ctrl for remote session (configurable). |

5. **Per-shortcut override**: Users can configure which shortcuts pass through and which are intercepted locally:

```toml
[input.shortcuts]
passthrough_mode = "smart"             # "smart", "full", "minimal"
passthrough_toggle = "Ctrl+Alt+G"
# Override specific shortcuts:
overrides = [
    { key = "Alt+Tab", action = "remote" },      # always send to remote
    { key = "Super", action = "local" },          # always keep local
    { key = "Super+L", action = "remote" },       # send lock to remote
]
```

This policy is documented so users understand why certain shortcuts "don't work" in a remote session and how to change the behavior.

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
LiquiDE provides comprehensive internationalization support. All user-facing text in the DE shell, login screen, settings, and built-in applications is translatable. The i18n system covers UI translations, keyboard layouts, date/time formatting, number formatting, and text directionality.

### Supported Languages

LiquiDE ships with translations for 40+ languages. The translation framework supports any additional language through community-contributed message catalogs.

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
- Community translations are loaded from `/etc/liquide/i18n/` or `~/.config/liquide/i18n/`.

### Translation Framework

#### Message Format
- Translation uses **Project Fluent** (`.ftl` files) — a modern localization system designed for natural-sounding translations.
- Fluent supports pluralization, gender, number formatting, and complex grammatical rules natively.
- Message files are stored at:
  - System: `/etc/liquide/i18n/<locale>/messages.ftl`
  - User overrides: `~/.config/liquide/i18n/<locale>/messages.ftl`
- Example message file:
  ```ftl
  # /etc/liquide/i18n/de-DE/messages.ftl
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
# ~/.config/liquide/keyboard-layout.toml

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
# ~/.config/liquide/session.toml

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

- LiquiDE supports **right-to-left (RTL)** text layout for Arabic, Hebrew, Persian, and Urdu locales.
- When an RTL language is the primary locale:
  - Status bar layout mirrors (clock/tray on left, app menus on right).
  - Dock position defaults mirror (though user can override).
  - Settings panels, dialogs, and list layouts mirror horizontally.
  - Notification slide direction reverses (enters from left instead of right).
- **Mixed BiDi content** is handled within text fields using the Unicode Bidirectional Algorithm (UBA).
- CSS logical properties are used throughout the DE shell (e.g., `margin-inline-start` instead of `margin-left`) so layouts adapt automatically.
- **Compositor-level mirroring**: when the primary locale is RTL, the compositor mirrors all window chrome (close/min/max buttons swap sides), dock layout, and status bar automatically. Application content is never mirrored — only shell-level UI managed by the compositor.

### UI Translation Pipeline

All user-facing strings in the shell UI (dock, status bar, settings, dialogs, notifications) are externalized using **Project Fluent** (`.ftl` files), the same system described above. The translation workflow:

| Step | Description |
|------|-------------|
| **Source strings** | Maintained in `i18n/en-US/liquide.ftl` as the canonical source. |
| **Community translations** | Submitted via standard pull request workflow. Each locale has its own `.ftl` file: `i18n/<locale>/liquide.ftl`. |
| **Fallback chain** | User locale → language without country code (e.g., `fr-FR` → `fr`) → `fallback_locale` (default: `en-US`). Missing keys fall back silently — no blank strings. |
| **Pluralization** | Handled natively by Fluent's plural category selectors (`one`, `two`, `few`, `many`, `other`) per CLDR rules. |
| **RTL-aware formatting** | Fluent messages use Unicode directional isolates (FSI/PDI) for embedded LTR text within RTL strings (e.g., filenames, URLs). |
| **Build-time validation** | CI validates all `.ftl` files for syntax errors and missing placeholders. A Fluent linter rejects translations that drop required variables. |
| **Runtime reload** | Translation files can be updated without session restart via `liquidctl i18n reload`. New strings take effect on the next UI render cycle. |

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
# /etc/liquide/server.toml

[i18n]
default_locale = "en-US"                 # system default language
available_locales = ["en-US", "en-GB", "de-DE", "fr-FR", "ja-JP", "zh-CN", "ko-KR", "es-ES", "pt-BR", "ru-RU", "ar-SA", "hi-IN"]
fallback_locale = "en-US"
message_dir = "/etc/liquide/i18n"       # system message catalogs
allow_user_translations = true           # allow user-provided .ftl overrides
keyboard_layout_dir = "/etc/liquide/xkb" # custom XKB layout directory
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
- Each user has their own DE configuration directory: `~/.config/liquide/`.
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

- **Storage location**: `~/.config/liquide/avatar.png` (per-user, on the server).
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
- Stored in `~/.config/liquide/session.toml`:
  ```toml
  [profile]
  display_name = "Alice Johnson"           # optional, shown on lock screen and login greeting
  avatar = "avatar.png"                    # relative to ~/.config/liquide/
  initials_override = ""                   # override auto-generated initials (e.g., "AJ")
  ```
- If no display name is set, the Unix username is displayed.

#### Avatar Transfer

- **Login screen**: after username submission, the server sends the avatar (if it exists) or a generated initials SVG to the client. Transfer size: ≤64KB (120px avatar). The client caches avatars keyed on `server_address + username + avatar_hash`.
- **Lock screen**: avatar is already cached from the session start. If the avatar changes during the session, the lock screen updates on next display.
- **Client cache**: cached avatars persist across connections. Cache keyed on `(server_address, username, avatar_hash)`. Cache size configurable in client `[wallpaper_cache]` section.

#### Server Avatar Configuration

```toml
# /etc/liquide/server.toml

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

LiquiDE uses a **supervisor process model** for session isolation. The server daemon (`liquid-desktopd`) never runs user session code directly — it spawns a separate `liquid-session` child process for each authenticated user.

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
- The supervisor daemon runs as a privileged service (root or `liquide` service user) and handles process lifecycle only — it never touches user data directly.

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

#### Admission Control & Capacity Planning

CPU-only video encoding does not scale as intuitively as GPU-accelerated encoding. Multi-user servers hit CPU walls when too many sessions demand high-quality video encoding simultaneously. The supervisor enforces **admission control** to prevent oversubscription:

**Per-session resource budgets:**

| Resource | Default Budget | Hard Limit | Enforcement |
|----------|---------------|------------|-------------|
| CPU cores (cgroup `cpu.max`) | 2 cores (200000/100000 us) | 6 cores | cgroup v2 CPU controller. Session is throttled, not killed. |
| Encoder threads | 2 | 4 | Session-level config. Encoder thread pool capped. |
| Memory (cgroup `memory.max`) | 512 MB | 1 GB | cgroup v2 memory controller. OOM events monitored. |
| PIDs (cgroup `pids.max`) | 256 | 1024 | cgroup v2 PIDs controller. |
| I/O bandwidth | 10 MB/s | 50 MB/s | cgroup v2 IO controller. |
| Network bandwidth (outbound) | 20 Mbps | 50 Mbps | Transport-level send rate cap. |

**Admission control rules:**

The supervisor tracks total host CPU and memory capacity. Before spawning a new session, it checks:

```
admission_check():
  available_cpu = host_cores - reserved_cores - sum(active_session_cpu_budgets)
  available_memory = host_memory - reserved_memory - sum(active_session_memory_budgets)

  if available_cpu < new_session_cpu_budget:
    reject with "server at capacity" (or queue if waiting is allowed)
  if available_memory < new_session_memory_budget:
    reject with "server at capacity"

  # Encoder-specific check:
  active_video_sessions = count(sessions where video encoding is active)
  if active_video_sessions >= max_concurrent_video_sessions:
    allow session but force tile-only mode (no video encoding)
```

**Configuration:**

```toml
[admission]
enabled = true
reserved_cpu_cores = 2                   # reserved for supervisor + OS
reserved_memory_mb = 1024                # reserved for supervisor + OS
max_sessions = 0                         # 0 = auto-calculate from resources
max_concurrent_video_sessions = 0        # 0 = auto-calculate (host_cores / 2)
queue_enabled = false                    # queue sessions when at capacity
queue_timeout_sec = 30                   # timeout for queued sessions
deny_4k_below_cores = 8                 # deny 4K resolution if host has < N cores
deny_60fps_below_cores = 4              # force 30fps cap if host has < N cores
```

#### Capacity Planning Formulas

The admission control rules above enforce runtime limits. These formulas help administrators **size hardware before deployment**:

**Per-session resource cost model:**

```
CPU_session   = CPU_compose + CPU_encode + CPU_io + CPU_audio
BW_session    = (fps × avg_tile_bytes × damage_ratio) + audio_bps + control_overhead
RAM_session   = (framebuf × 2) + glyph_atlas + shadow_cache + compress_scratch + channel_queues
```

**Reference costs per workload profile (measured on 8-core x86_64 AVX2 reference system):**

| Workload Profile | CPU (p50 / p95 cores) | RAM (p50 / p95 MB) | Bandwidth (p50 / p95 Mbps) |
|-----------------|----------------------|--------------------|-----------------------------|
| **idle** | 0.01 / 0.02 | 95 / 100 | 0 / 0.01 |
| **text-editing** | 0.25 / 0.40 | 170 / 190 | 0.3 / 1.0 |
| **web-browsing** | 0.90 / 1.40 | 230 / 270 | 3.5 / 8.0 |
| **document** | 0.30 / 0.50 | 180 / 200 | 0.5 / 2.0 |
| **desktop-workflow** | 0.80 / 1.20 | 250 / 300 | 3.0 / 6.0 |
| **video-playback** | 1.50 / 2.00 | 200 / 240 | 8.0 / 15.0 |

**Sizing formula:**

```
max_sessions = floor((host_cores - reserved_cores) / p95_cpu_per_session)
             = min(max_sessions_cpu, floor((host_ram_mb - reserved_ram_mb) / p95_ram_per_session))
             = min(above, floor(host_bandwidth_mbps / p95_bw_per_session))
```

**Quick-reference sizing (office workload: 70% text-editing/document, 30% web-browsing):**

| Host Configuration | Max Sessions (p95) | Notes |
|-------------------|--------------------|----|
| 8 cores / 32 GB | 6 | Budget server. Mixed workloads. |
| 16 cores / 64 GB | 14 | Typical 1U server. |
| 32 cores / 128 GB | 28 | Standard rack server. |
| 64 cores / 256 GB | 55 | Dense deployment. |

All figures include **20% CPU headroom** for reconnect storms (multiple clients reconnecting simultaneously after a network event) and **30% memory headroom** for transient allocation spikes. GPU-accelerated deployments can support approximately 2x the session count for video-heavy workloads by offloading encoding.

See [spec-performance.md §7a](spec-performance.md) for a detailed capacity planning reference with benchmark-derived data.

**Auto-downgrade under host pressure:**

When the host is under sustained CPU pressure (>85% total CPU for >10 seconds), the supervisor applies progressive downgrades to existing sessions:

| Host CPU Usage | Action | Recovery |
|---------------|--------|----------|
| 85–90% | Reduce max FPS for all sessions by 20% (e.g., 60→48, 30→24) | Restore when <80% for 30s |
| 90–95% | Force all sessions to tile-only mode (disable video encoding). Send `CodecSwitch` to clients. | Restore video when <85% for 60s |
| 95–100% | Reduce tile quality (increase compression, skip cosmetic tiles). Disable blur/shadows globally. | Restore when <90% for 60s |
| Sustained >95% for 60s | Notify admin. Consider suspending least-recently-active sessions. | Admin intervention |

**Encoder mode selection policy (CPU-only):**

The default encoding strategy prioritizes tile mode for most UI and reserves video encoding for specific scenarios:

| Content Type | Encoding Mode | Rationale |
|-------------|--------------|-----------|
| Static UI (text, forms, menus) | Tile (Zstd lossless) | Zero visual loss. Low CPU. High skip ratio (most tiles unchanged). |
| Text editing / terminal | Tile (Zstd lossless) | Lossless text is non-negotiable for readability. |
| Smooth scrolling | Tile with `TileScroll` optimization | Scroll vector + newly exposed tiles. Very efficient. |
| Embedded video / media player | Video (H.264/AV1) | Only when bandwidth allows. Dynamically scoped to the video region only. |
| Fast full-screen animation | Video (H.264) | Full-screen damage triggers video mode. Tile mode would overwhelm bandwidth. |
| Window drag/resize | Video (H.264, low quality) | Transient — switches back to tile mode when motion stops. |
| Idle / cursor blink only | Neither (skip) | Zero CPU, zero bandwidth. |

This means a typical office workload (text editor + browser + terminal) uses almost no video encoding CPU — tile mode handles it efficiently. Video encoding activates only for the 5–10% of screen time that involves motion.

### Session Lifecycle State Model

Each session follows a well-defined state machine. Transitions are logged as audit events and exposed via `liquidctl session status`.

```
                         ┌──────────┐
                         │ Created  │  (process spawned, initializing)
                         └────┬─────┘
                              │ init complete
                              ▼
                    ┌──────────────────┐
                    │ Authenticating   │  (waiting for user auth)
                    └────┬─────────┬──┘
                         │         │ auth timeout / max attempts
                    auth │         ▼
                  success│    ┌──────────┐
                         │    │Terminated│
                         │    └──────────┘
                         ▼
                    ┌──────────┐
              ┌────►│ Running  │◄───────────────────────────┐
              │     └──┬──┬──┬─┘                            │
              │        │  │  │                               │
              │  idle  │  │  │ user lock /                   │ unlock
              │ timeout│  │  │ policy lock                   │
              │        │  │  ▼                               │
              │        │  │ ┌──────────┐                    │
              │        │  │ │  Locked  │────────────────────┘
              │        │  │ └──────────┘
              │        │  │
              │        │  │ client disconnects
              │        │  ▼
              │        │ ┌──────────────┐
              │        │ │ Disconnected │ (session alive, no client)
              │        │ └──────┬──┬────┘
              │        │        │  │
              │        │ client │  │ disconnect timeout
              │        │ reconnect │ (session.disconnect_timeout_sec)
              │        │        │  ▼
              │        │        │ ┌──────────┐
              │        │        │ │Terminated│
              │        │        │ └──────────┘
              │        │        ▼
              │        │    Running (resumed)
              │        │
              │        │ explicit suspend / admin suspend
              │        ▼
              │   ┌───────────┐
              │   │ Suspended │ (session paused, state preserved, minimal resources)
              │   └─────┬─────┘
              │         │ resume (client reconnect or admin resume)
              │         ▼
              │     Running (resumed)
              │
              │ crash
              ▼
         ┌──────────┐
         │ Crashed  │ (crash detected, restart pending)
         └────┬─────┘
              │ restart policy
              ├──────────────► Running (restarted)
              │
              │ max restarts exceeded
              ▼
         ┌──────────┐
         │  Failed  │ (requires admin intervention)
         └──────────┘
```

**State definitions:**

| State | Description | CPU Usage | Client View |
|-------|-------------|-----------|-------------|
| **Created** | Process spawned, loading config, initializing compositor | Moderate (startup) | Connection accepted, waiting |
| **Authenticating** | Waiting for client to complete auth flow | Minimal | Login screen |
| **Running** | Active desktop session, all workers operational | Normal | Full desktop |
| **Locked** | Session locked (user-initiated, idle timeout, or policy). Screen locked, input blocked. Session process continues running. | Low (no rendering except lock screen) | Lock screen |
| **Disconnected** | Client disconnected but session is alive. Applications continue running. No rendering or encoding. | Minimal (apps run, no rendering) | N/A (no client) |
| **Suspended** | Session explicitly suspended. Worker threads paused. Applications frozen (SIGSTOP). Minimal memory footprint. | Near zero | N/A |
| **Crashed** | Session process exited abnormally. Supervisor handling restart. | Zero | Crash screen |
| **Failed** | Restart attempts exhausted. Session cannot recover without admin action. | Zero | Persistent crash screen |
| **Terminated** | Session ended. Process exited. All resources released. | Zero | Disconnected / login screen |

#### Session Lifecycle Invariants

The following invariants define what persists, what resets, and what is allowed in each state transition. These MUST hold across all session operations.

**What persists across reconnect (Disconnected → Running):**

| State | Persists? | Notes |
|-------|-----------|-------|
| Running applications | Yes | All processes continue during Disconnected state |
| Window positions and z-order | Yes | Restored exactly; z-order reconciled with client |
| Clipboard contents | Yes | Clipboard buffer retained on server |
| Unsaved application data | Yes | Applications were never interrupted |
| Audio playback state | Yes | Audio resumes on reconnect (buffered during disconnect) |
| IME composition state | No | Active preedit is committed or discarded on disconnect |
| Cursor position | Yes | Server tracks last known position |
| Session environment variables | Yes | Process environment unchanged |
| USB device redirections | No | Devices are re-enumerated on reconnect; client re-offers |
| Camera/microphone streams | No | Re-established by client after reconnect |

**What resets on reconnect:**

| Item | Reset Behavior |
|------|---------------|
| Transport negotiation | Re-negotiated (client may connect from different network) |
| Encoding parameters | Re-negotiated (client may have different capabilities) |
| Authentication token | New token issued (old token invalidated on disconnect timeout) |
| Client capabilities | Re-queried (client may be a different platform/version) |
| Display resolution | Re-negotiated if client reports different resolution |
| Full frame (IDR) | Sent immediately on reconnect (no delta from pre-disconnect state) |

**What is allowed while locked:**

| Action | Allowed? | Notes |
|--------|----------|-------|
| Lock screen rendering | Yes | Lock screen is rendered (optionally client-side) |
| Auth input on lock screen | Yes | Password/biometric input accepted |
| Application execution (background) | Yes | Apps continue running but do not receive input |
| Clipboard access | Configurable | `lock_pause_clipboard` (default: `true`) |
| USB device forwarding | Configurable | `lock_pause_usb` (default: `false`) |
| Audio playback | Configurable | `lock_mute_audio` (default: `false`) |
| Camera forwarding | Configurable | `lock_pause_camera` (default: `true`) |
| Admin session inspection | Yes | Admin can view session metadata, force disconnect |
| Policy changes | Yes | Policy changes are applied immediately; may force unlock or terminate |

**Policy changes mid-session:**

| Policy Change | Effect |
|--------------|--------|
| `sessions.idle_timeout` decreased | If session has been idle longer than new timeout, lock/disconnect immediately |
| `clipboard.enabled` changed to `false` | Clipboard sync paused immediately; pending transfers cancelled |
| `sessions.max_resolution` decreased | Resolution constraint applied on next resize or reconnect |
| `auth.mfa_required` enabled | Enforced on next authentication (reconnect or unlock) |
| `encoding.allowed_encoders` changed | Encoder re-negotiated on next keyframe; active encoder continues until next IDR |
| Session terminated by admin | Session transitions to Terminated immediately; client receives disconnect reason |

### Roaming & Multi-Client Sessions

A session can be accessed by multiple clients or moved between clients (roaming).

#### Connection Behavior Modes

When a new client connects to a session that already has an active connection:

| Mode | Behavior | Use Case |
|------|----------|----------|
| `steal` (default) | New client takes over the session. Previous client is disconnected with a "session taken by another client" message. | Moving between workstations |
| `mirror` | Both clients see the same session simultaneously. Input from both clients is merged (last-writer-wins for key events, both mouse cursors shown). | Pair programming, support |
| `deny` | New client is rejected with "session already in use" error. Only one client at a time. | Security-sensitive environments |
| `view-only` | New client can observe the session but cannot send input. First client retains full control. | Training, monitoring |

Configuration:

```toml
[session]
multi_client_mode = "steal"          # steal, mirror, deny, view-only
# Mirror mode settings
mirror_show_remote_cursor = true     # show a ghost cursor for the other client
mirror_max_clients = 4               # max simultaneous mirror clients
```

#### Session Selection at Login

When a user authenticates and has existing sessions:

```
┌─────────────────────────────────────────────────┐
│              Session Selection                   │
│                                                  │
│   You have existing sessions:                    │
│                                                  │
│   ┌─────────────────────────────────────────┐   │
│   │ ● Session s-001 (Running)               │   │
│   │   Started: 2h ago · 3 windows           │   │
│   │   [Resume]  [Terminate]                 │   │
│   ├─────────────────────────────────────────┤   │
│   │ ○ Session s-003 (Disconnected)          │   │
│   │   Disconnected: 45m ago · 1 window      │   │
│   │   [Resume]  [Terminate]                 │   │
│   └─────────────────────────────────────────┘   │
│                                                  │
│   [Start New Session]                            │
│                                                  │
└─────────────────────────────────────────────────┘
```

- If `session.auto_resume = true` (default) and only one session exists, the client automatically resumes it (no selection screen).
- If multiple sessions exist, the selection screen is shown.
- If policy `session.max_per_user` would be exceeded, the "Start New Session" option is disabled.

#### Session Migration vs. Reconnect-to-Restart

"Roaming" in LiquiDE means: **the same live session state, accessed from a new transport endpoint**. It does NOT mean "same user profile, new session."

| Capability | v1.0 | v2.0+ (Roadmap) |
|-----------|------|-----------------|
| **Reconnect-to-same-process** | Yes | Yes |
| Mechanism | Resume token (see §14a Resume Protocol). Client connects from new IP/transport, presents token, session transitions from Disconnected → Running. | Same |
| Latency | Sub-second (token validation + first IDR frame) | Same |
| State preserved | Everything (session process was alive the entire time) | Everything |
| **Live migration (server-to-server)** | No | Planned |
| Mechanism | N/A | CRIU checkpoint on source, transfer to destination with shared storage, restore. Requires D2 durability tier (see Session Durability Contract). |
| Prerequisite | N/A | Shared filesystem (NFS/Ceph) for session state, CRIU Wayland support, gateway-level migration orchestration |

**v1.0 policy**: roaming is always reconnect-to-same-process on the same server. If the server is unreachable, the client cannot resume — it must wait for the server to recover or start a new session elsewhere. The gateway does not migrate sessions between servers; it only routes new connections.

#### Seamless Windows Mode Survivability

When the client is operating in seamless windows mode (remote windows rendered as native OS windows) and a network disconnect occurs:

| Phase | Client Behavior | Server Behavior |
|-------|----------------|-----------------|
| **Disconnect detected** | Each native OS window freezes on its last rendered frame. A translucent "Reconnecting..." overlay appears on each window (50% opacity, centered text). | Session enters Disconnected state. Applications continue running. Compositor continues rendering (output is buffered, not sent). |
| **During reconnect attempts** | Overlay updates with attempt count and countdown timer. Windows remain interactive at the OS level (can be moved, minimized, but content is frozen). User can still interact with local OS UI. | No change. Session is alive but has no connected client. |
| **Reconnect succeeds** | Overlay fades out (200ms). Each window reconciles with server state: geometry (position, size) is updated to match server (server wins if geometry changed during disconnect — e.g., another client in mirror mode resized a window). Content updates via tile/video stream resume. | Session transitions to Running. First frame is IDR (keyframe). Damage for the entire screen is flagged (full refresh). |
| **Z-order reconciliation** | Best-effort: server sends the current Z-order, client reorders native windows. If the OS window manager has changed Z-order locally (user brought a local app to front), the local order takes precedence for non-LiquiDE windows. | Window tree is authoritative. Client-side Z-order is advisory. |
| **Reconnect fails (timeout)** | All seamless windows show "Session disconnected" overlay. User can click "Reconnect" (retry) or "Close" (terminate session). Closing a seamless window does NOT close the remote application — it only closes the local proxy window. | Session remains in Disconnected state until `disconnect_timeout_sec` expires, then terminates. |

#### Session Resume Protocol

Session resume allows a client to reconnect to a running or disconnected session without full re-authentication. This handles: laptop lid close/open, Wi-Fi roaming, network changes (VPN connect/disconnect), switching between office and home, and gateway failover.

##### Resume Token

On successful authentication, the server issues a **resume token** alongside the session:

| Token Field | Description |
|-------------|-------------|
| `token_id` | Opaque 256-bit random identifier |
| `session_id` | Bound session ID |
| `user_id` | Bound user identity |
| `issued_at` | Issuance timestamp (UTC) |
| `expires_at` | Expiration timestamp (UTC). Default: 7 days (configurable). |
| `client_fingerprint` | Hash of client properties (OS, machine-id, display config). Used for binding — not for security. |
| `max_uses` | Maximum number of resume attempts with this token. Default: unlimited. |
| `scope` | `"same-server"` or `"any-gateway"` — whether the token is valid only on the issuing server or routable via gateway. |

The token is stored:
- **Native client**: OS credential store (Windows: DPAPI/Credential Manager, macOS: Keychain, Linux: libsecret/kwallet).
- **Web client**: `sessionStorage` (tab-scoped, cleared on close) or `localStorage` (with "remember me" — encrypted via Web Crypto API + user passphrase).

##### Resume State Machine

```
Client                               Server (Supervisor)
  │                                       │
  │  Session disconnects (network drop,   │
  │  lid close, VPN change, etc.)         │
  │                                       │  Session enters Disconnected state
  │                                       │  (apps continue running)
  │                                       │
  │  ... time passes (network changes) ...│
  │                                       │
  │  ── TCP/QUIC connect ──────────────►  │  (may be different source IP,
  │     (may traverse different gateway)  │   different NAT, different port)
  │                                       │
  │  ── ResumeRequest ─────────────────►  │  Contains: resume_token, session_id,
  │     {token, session_id,               │  client capabilities, new display config
  │      client_caps, display}            │
  │                                       │
  │                                       │  Server validates:
  │                                       │   1. Token exists and not expired
  │                                       │   2. Token.session_id matches request
  │                                       │   3. Token.user_id matches session owner
  │                                       │   4. Session is in Disconnected or Running state
  │                                       │   5. Policy allows resume
  │                                       │
  │  ◄── ResumeAccepted ──────────────── │  Contains: new_resume_token (rotation),
  │      {new_token, session_state}       │  session state summary
  │                                       │
  │  ── Transport negotiation ─────────►  │  (new transport, may differ from before)
  │  ◄── First frame ─────────────────── │  (session rendering resumes)
  │                                       │
  │  Resume complete.                     │  Session returns to Running state.
  │  Old resume token invalidated.        │  New token issued.
```

**Token rotation**: every successful resume issues a **new** resume token and invalidates the old one. This limits the window of token theft.

**Resume failure modes**:

| Failure | Server Response | Client Action |
|---------|----------------|---------------|
| Token expired | `ResumeRejected { reason: "token_expired" }` | Fall back to full authentication |
| Token revoked / invalid | `ResumeRejected { reason: "token_invalid" }` | Fall back to full authentication |
| Session terminated | `ResumeRejected { reason: "session_terminated" }` | Show "Session ended" message, offer new session |
| Session in Failed state | `ResumeRejected { reason: "session_failed" }` | Show crash screen |
| Policy denies resume | `ResumeRejected { reason: "policy_denied" }` | Fall back to full authentication |
| Different user | `ResumeRejected { reason: "user_mismatch" }` | Fall back to full authentication |
| Concurrent connection (deny mode) | `ResumeRejected { reason: "session_in_use" }` | Show "session in use" message |

##### NAT / IP / Gateway Changes

Resume is **IP-agnostic**. The resume token is validated by session ID and user identity, not by source IP. This means:

- Client can resume from a completely different IP address (e.g., switching from office Ethernet to home Wi-Fi).
- Client can resume through a different gateway (if token scope is `"any-gateway"` and the gateways share a token validation backend).
- Client can resume after VPN connect/disconnect changes the source IP.
- Client can resume after mobile network handoff (4G → Wi-Fi).

The server logs the IP change for audit purposes but does not reject the resume based on IP mismatch.

##### Gateway-Routed Resume

When connecting through a gateway, the resume flow adds a routing step:

```
Client → Gateway: ResumeRequest { token, session_id }
Gateway: looks up session_id → backend server mapping
Gateway → Backend: forwards ResumeRequest
Backend: validates token, accepts/rejects
Backend → Gateway: ResumeAccepted/ResumeRejected
Gateway → Client: forwards response
```

For `scope: "any-gateway"` tokens, the gateway validates the token locally (shared token store across gateways, e.g., Redis or database) before routing to the backend. This prevents unnecessary routing attempts to backends that no longer host the session.

##### Resume Configuration

```toml
[session.resume]
enabled = true
token_lifetime_hours = 168            # 7 days
token_rotation = true                 # issue new token on each resume
token_scope = "same-server"           # same-server, any-gateway
max_disconnected_minutes = 60         # session terminated after this long disconnected
# (0 = never auto-terminate disconnected sessions)
require_mfa_on_resume = false         # true = require MFA on every resume
require_mfa_after_hours = 24          # require MFA if last auth was > N hours ago
```

### Crash Loop Containment

When a session repeatedly crashes, the system employs escalating containment:

| Restart # | Backoff | Action |
|-----------|---------|--------|
| 1 | 0ms (immediate) | Restart normally |
| 2 | 1s | Restart normally |
| 3 | 2s | Restart with plugins disabled (`--safe-plugins`) |
| 4 | 4s | Restart in safe mode (`--safe-mode`) — minimal compositor, no shell plugins, no user CSS, no animations |
| 5 | 8s | Restart in safe mode |
| 6+ | — | Enter `Failed` state. Client shows persistent crash screen with admin contact info. |

**Safe mode (`--safe-mode`)** disables:
- All WASM plugins
- User CSS overrides (uses default theme)
- All shell animations
- Wallpaper (solid color)
- Rendering profile forced to `minimal`
- Non-essential shell features (launcher plugins, notification handlers)

Safe mode retains:
- Core compositor functionality
- Dock, status bar, terminal
- File manager
- Authentication and session management
- Policy enforcement

**Plugin quarantine**: if crashes consistently occur after a specific plugin loads (detected by correlating crash timestamps with plugin load events), that plugin is automatically quarantined (disabled) and a warning is logged. The quarantine persists across session restarts until the admin explicitly re-enables the plugin.

### Session Configuration

```toml
# /etc/liquide/server.toml

[session]
auto_resume = true                   # auto-resume single existing session
disconnect_timeout_sec = 3600        # keep session alive for 1h after disconnect
idle_lock_sec = 300                  # lock session after 5min idle
idle_suspend_sec = 0                 # 0 = never auto-suspend
max_duration_sec = 86400             # max session duration (24h)
max_per_user = 3                     # max concurrent sessions per user

[session.multi_client]
mode = "steal"                       # steal, mirror, deny, view-only
mirror_max_clients = 4
mirror_show_remote_cursor = true

[supervisor]
heartbeat_interval_sec = 5
heartbeat_timeout_count = 3          # miss 3 heartbeats = hung
max_restarts = 5
restart_window_sec = 600
restart_backoff_base_ms = 1000
crash_report_dir = "/var/log/liquide/crashes"
coredump_enabled = true
crash_log_lines = 100
safe_mode_after_restart = 3          # enter safe mode after 3rd restart
plugin_quarantine_enabled = true
```

### Session Durability Contract

LiquiDE makes an explicit, versioned commitment about what happens when things go wrong. This contract defines what users and administrators can expect regarding session state preservation.

#### Durability Tiers

| Tier | Name | Ship Target | Description |
|------|------|-------------|-------------|
| **D1** | Crash-only restart | v1.0 | Session process restarts from scratch. Applications are relaunched but lose in-flight state. User profile, saved files, and configuration are preserved (they live on disk, not in process memory). |
| **D2** | Stateful migration | v2.0+ | Live session migration between servers. Requires shared storage (NFS/Ceph) for session state and CRIU for process checkpoint. |
| **D3** | Application checkpoint/restore | v2.0+ | Individual application state is checkpointed to disk and restored after crash or migration. Depends on CRIU integration and application cooperation. |

**v1.0 ships Tier D1 only.** This is an explicit product decision: crash-only restart is well-understood, testable, and avoids the complexity of stateful migration. Tiers D2 and D3 are roadmap items gated on CRIU maturity and shared-storage prerequisites.

#### State Preservation Matrix (Tier D1 — Crash-Only Restart)

| State Category | Preserved Across Restart? | Mechanism | Notes |
|---------------|--------------------------|-----------|-------|
| User files (`~/`) | Yes | Filesystem (survives process death) | No data loss for saved work |
| Configuration (server.toml, policies) | Yes | Filesystem | Reloaded on restart |
| Compositor window list | Best-effort | `session-state.json` written on clean shutdown, crash-recovery snapshot every 60s | Window positions and Z-order restored approximately; application must re-render content |
| Application PIDs / process tree | No | Lost on process death | Applications are relaunched by the compositor (if `auto_relaunch = true` in session config); unsaved in-app state is lost |
| Clipboard content | No | In-memory only | Clipboard is empty after restart |
| Audio pipeline state | No | Re-initialized on restart | Brief silence during restart; audio resumes automatically |
| USB device attachments | No | Devices detached on session death | User must re-forward devices; auto-forward rules can restore previously-approved devices |
| Resume token | Yes | Stored in supervisor (survives session crash) | Client can resume to the restarted session without re-authentication |
| Plugin state | Best-effort | Plugins with `persist_state = true` write to `~/.local/share/liquide/plugins/<id>/state.json` | Plugin decides what to persist; framework provides the storage API |

#### User-Visible Behavior on Crash

1. Client shows crash screen (see §25) within 500ms of crash detection.
2. Supervisor restarts session per restart policy (immediate on first crash).
3. Client auto-resumes via resume token (no re-authentication required).
4. Desktop shell relaunches. Previously-open applications are relaunched if `session.auto_relaunch = true`.
5. User sees their desktop restored with windows approximately in their previous positions.
6. **Unsaved work in applications is lost** — this is stated explicitly so users and administrators have correct expectations.

#### Roadmap Gate for D2/D3

Stateful migration (D2) and application checkpoint/restore (D3) will be considered when:
- CRIU (Checkpoint/Restore in Userspace) supports Wayland compositors reliably.
- A shared storage backend (NFS, Ceph, or similar) is validated for session state.
- The performance overhead of periodic checkpointing is measured and fits within the CPU budget (see [spec-performance.md §2.3b](spec-performance.md)).

Until then, LiquiDE's durability story is: **disconnect does not destroy work (session stays alive for `disconnect_timeout_sec`), but crash-and-restart does lose unsaved in-app state.**

### State & Storage Contract

All persistent and ephemeral state across LiquiDE components follows a unified storage model with defined ownership, atomic write guarantees, and migration semantics.

#### Directory Layout

```
/etc/liquide/                           # System configuration (root-owned, 0755)
├── server.toml                         # Main server configuration
├── policies/                           # Policy files (§15)
│   ├── server.toml                     # Server-wide policy
│   ├── groups/                         # Per-group policies
│   │   └── <group>.toml
│   └── users/                          # Per-user policies
│       └── <user>.toml
├── certs/                              # TLS certificates and keys
│   ├── server.crt                      # Server certificate (0644)
│   ├── server.key                      # Server private key (0600, liquide:liquide)
│   └── client-ca.pem                   # Client CA for mTLS (0644)
├── plugins/                            # Plugin management
│   ├── trusted-keys/                   # Ed25519 public keys for plugin signing
│   └── config/                         # Per-plugin configuration overrides
├── codecs/                             # Codec module configuration
└── gateway-psk                         # Gateway pre-shared key (0600)

/var/lib/liquide/                       # Persistent runtime state (liquide:liquide, 0750)
├── db/                                 # Embedded database
│   └── state.sqlite3                   # SQLite database (WAL mode)
├── codecs/                             # Downloaded codec binaries (OpenH264)
├── plugins/                            # Installed plugin .wasm files
├── rollback/                           # Rollback binaries for previous version
└── assets/                             # Cached processed assets (generated icons, etc.)

/var/log/liquide/                       # Logs (liquide:liquide, 0750)
├── server.log                          # Main server log
├── auth.log                            # Authentication events (fail2ban reads this)
├── audit.log                           # Audit events
├── session-<id>.log                    # Per-session logs
└── crashes/                            # Crash reports
    ├── crash-<session>-<timestamp>.json
    └── core.<session>.<timestamp>      # Coredumps (0600)

/run/liquide/                           # Ephemeral runtime (tmpfs, liquide:liquide, 0750)
├── supervisor.sock                     # Supervisor IPC socket
├── session-<id>.sock                   # Per-session IPC socket
├── session-<id>.pid                    # Per-session PID file
└── metrics/                            # Shared memory for hot metrics

~/.config/liquide/                      # Per-user DE configuration (user-owned)
├── theme.css                           # User theme overrides
├── session.toml                        # Session preferences
├── keybindings.toml                    # Keyboard shortcuts
├── keyboard-layout.toml                # Layout preferences
└── avatar.png                          # User avatar

~/.local/state/liquide/                 # Per-user state (user-owned)
├── session-state.json                  # Last session state (window positions, workspace)
├── clipboard-history.json              # Clipboard history ring buffer
└── recent-files.json                   # Recent files list

~/.local/share/liquide/                 # Per-user data (user-owned)
├── plugins/                            # User-installed plugins
├── themes/                             # User-installed themes
└── fonts/                              # User-installed fonts
```

#### Embedded Database

LiquiDE uses **SQLite 3** (WAL mode) as its single embedded database:

| Property | Specification |
|----------|--------------|
| **Location** | `/var/lib/liquide/db/state.sqlite3` |
| **Engine** | SQLite 3.40+ with WAL (Write-Ahead Logging) mode |
| **Access** | Single writer (supervisor daemon), multiple readers (session processes via shared-memory) |
| **Contents** | Session metadata, user attributes (cached from LDAP/OIDC), plugin registry, schema version, crash history, policy cache |
| **Size** | Typically < 50 MB for 1000 users. `VACUUM` runs weekly (configurable). |
| **Backup** | `liquidctl db backup` creates a consistent snapshot. Automatic pre-migration backup. |
| **No user data** | The database stores metadata only — never user files, screen content, clipboard, or session buffers. |

Why SQLite over alternatives:

| Considered | Rejected Because |
|-----------|-----------------|
| RocksDB | Overkill — LiquiDE's database is metadata-only, not a high-throughput KV store |
| PostgreSQL/MySQL | External dependency — LiquiDE should be self-contained |
| JSON files | No concurrent access safety, no transactions, no schema migration |
| Sled | Unmaintained, less mature than SQLite |

#### Schema Migration

Database migrations run automatically on daemon startup:

| Property | Specification |
|----------|--------------|
| **Migration format** | Embedded SQL files in the binary (`migrations/V001__initial.sql`, `V002__add_plugin_registry.sql`, ...). |
| **Versioning** | `schema_version` table tracks current version. Each migration has a monotonic version number and SHA-256 hash. |
| **Execution** | Forward-only by default. Each migration runs in a single SQLite transaction. On failure, the transaction rolls back and the daemon refuses to start. |
| **Down-migrations** | Paired `down_V002__remove_plugin_registry.sql` files exist for each migration. Run via `liquidctl db migrate --down --to V001`. Down-migrations are for rollback only, not for production use. |
| **Idempotency** | Checked by version number. A migration that has already run (version exists in `schema_version`) is skipped. |
| **Pre-migration backup** | Before any migration, the daemon creates `state.sqlite3.pre-migration-VN.bak`. Kept for 7 days. |
| **Large migrations** | Migrations that touch > 10,000 rows use batched updates (1000 per transaction) to avoid long locks. |
| **Dry-run** | `liquidctl db migrate --dry-run` shows pending migrations without applying them. |
| **CI validation** | CI runs `migrate up` then `migrate down` for every PR that touches migration files. |

```bash
# Check current schema version
liquidctl db status

# Run pending migrations (automatic on daemon start)
liquidctl db migrate

# Rollback to a specific version
liquidctl db migrate --down --to V005

# Create a consistent backup
liquidctl db backup /tmp/liquide-db-backup.sqlite3

# Show migration history
liquidctl db history
```

#### Atomic Write Rules

All file writes in LiquiDE follow these safety rules:

| Rule | Description |
|------|-------------|
| **Write-rename** | Configuration files and state files are written to a temporary file in the same directory, then atomically renamed (`rename(2)`) to the target path. This prevents partial reads. |
| **SQLite WAL** | Database writes use WAL mode for crash safety. A crash mid-write cannot corrupt the database. |
| **Fsync for durability** | After rename, `fsync` is called on the target file and its parent directory. |
| **Lock files** | Multi-process writes to the same file use advisory locks (`flock`). Only the supervisor writes to `/var/lib/liquide/db/`; session processes read. |
| **No partial config** | Configuration is loaded atomically on daemon start and on `SIGHUP` reload. A parse failure in the new config leaves the old config active. |

#### Ownership & Quotas

| Path | Owner | Permissions | Quota |
|------|-------|-------------|-------|
| `/etc/liquide/` | `root:root` | `0755` (dirs), `0644` (files), `0600` (keys) | N/A |
| `/var/lib/liquide/` | `liquide:liquide` | `0750` | Configurable, default 1 GB |
| `/var/log/liquide/` | `liquide:liquide` | `0750` | Log rotation: 50 MB × 5 files per log |
| `/run/liquide/` | `liquide:liquide` | `0750` | tmpfs, ephemeral |
| `~/.config/liquide/` | user | `0700` | Per-user quota: configurable, default 50 MB |
| `~/.local/state/liquide/` | user | `0700` | Per-user quota: configurable, default 100 MB |
| `~/.local/share/liquide/` | user | `0700` | Per-user quota: configurable, default 500 MB |

#### Cache Invalidation

| Cache | Location | Invalidation Trigger | Recovery |
|-------|----------|---------------------|----------|
| Glyph atlas | Session process memory | Font config change, DPI change | Rebuild from FreeType |
| Shadow cache | Session process memory | Window geometry change | Recompute shadow (LRU eviction) |
| Blur cache | Session process memory | Background content change | Recompute blur |
| Tile hash cache | Session process memory | Session restart | Full key frame rebuild |
| Asset cache (client) | Client-side storage | `AssetManifest` hash mismatch | Re-download from server |
| Plugin WASM cache | `/var/lib/liquide/plugins/` | Plugin update, `liquidctl plugins reload` | Recompile from source .wasm |
| OIDC JWKS cache | Supervisor memory | TTL expiry (default 1h) | Re-fetch from IdP |
| LDAP directory cache | SQLite | Sync interval expiry | Re-sync from LDAP |

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
  2. LiquiDE built-in apps (Liquid Terminal, File Manager, Settings, Task Monitor).
  3. Custom app entries defined in `~/.config/liquide/apps.toml`.
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
- Favorites are stored in `~/.config/liquide/session.toml` under `[launcher.favorites]`.
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
  - Plugins register via a manifest file in `~/.config/liquide/launcher-plugins/`.
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
# ~/.config/liquide/session.toml

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
plugin_dir = "~/.config/liquide/launcher-plugins/"
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

### Shell Keyboard Shortcuts

LiquiDE provides familiar keyboard shortcuts inspired by Windows, GNOME, and macOS conventions. All shortcuts are reconfigurable via `keybindings.toml` or Settings → Keyboard → Shortcuts. Shortcuts marked **(custom)** are LiquiDE-specific additions to the familiar set.

#### System & Session

| Shortcut | Action |
|----------|--------|
| `Super` (tap) | Open/close app launcher |
| `Super+L` | Lock session |
| `Super+D` | Show desktop (minimize/restore all windows) |
| `Ctrl+Alt+Del` | Open session menu (Lock, Log Out, Shut Down, Task Manager) |
| `Ctrl+Shift+Esc` | Open system monitor / task manager |
| `Super+I` | Open Settings |
| `Super+E` | Open file manager |
| `Super+T` | Open terminal **(custom)** |
| `Super+.` | Open emoji picker **(custom)** |
| `Super+;` | Open emoji picker (alias) **(custom)** |
| `Super+V` | Open clipboard history |
| `Super+A` | Open notification center / action center **(custom)** |
| `Super+N` | Open notification center (alias) **(custom)** |
| `Super+K` | Open quick settings flyout (Wi-Fi, Bluetooth, audio, brightness) **(custom)** |

#### Window Management

| Shortcut | Action |
|----------|--------|
| `Alt+Tab` | Switch windows (forward) |
| `Alt+Shift+Tab` | Switch windows (backward) |
| `Super+Tab` | Task overview / exposé (all windows with thumbnails) |
| `Alt+F4` | Close focused window |
| `Super+Up` | Maximize focused window |
| `Super+Down` | Restore / minimize focused window (toggle) |
| `Super+Left` | Tile window to left half |
| `Super+Right` | Tile window to right half |
| `Super+Shift+Left` | Move window to left monitor (or prev workspace if single monitor) |
| `Super+Shift+Right` | Move window to right monitor (or next workspace if single monitor) |
| `Super+Shift+Up` | Tile window to top half **(custom)** |
| `Super+Shift+Down` | Tile window to bottom half **(custom)** |
| `Super+Enter` | Toggle focused window fullscreen |
| `Super+M` | Minimize focused window **(custom)** |
| `Super+Home` | Minimize all except focused window **(custom)** |
| `Alt+Space` | Open window title bar menu (move, resize, minimize, close) |
| `Super+Shift+Arrow` | Swap tiled window position |

#### Workspaces & Virtual Desktops

| Shortcut | Action |
|----------|--------|
| `Super+Ctrl+Left` | Switch to previous workspace |
| `Super+Ctrl+Right` | Switch to next workspace |
| `Super+Ctrl+Up` | Workspace overview **(custom)** |
| `Super+Ctrl+D` | Add new workspace **(custom)** |
| `Super+Ctrl+F4` | Close current workspace (moves windows to adjacent) **(custom)** |
| `Super+Ctrl+Shift+Left` | Move focused window to previous workspace |
| `Super+Ctrl+Shift+Right` | Move focused window to next workspace |
| `Super+Ctrl+[1-9]` | Switch to workspace N |
| `Super+Ctrl+Shift+[1-9]` | Move focused window to workspace N |

#### Dock & Taskbar

| Shortcut | Action |
|----------|--------|
| `Super+[1-9]` | Launch or switch to Nth dock app |
| `Super+Shift+[1-9]` | Open new instance of Nth dock app |
| `Super+Alt+[1-9]` | Open Nth dock app's jump list / actions menu **(custom)** |
| `Super+0` | Launch or switch to 10th dock app |

#### Screenshot & Screen Recording

| Shortcut | Action |
|----------|--------|
| `Print Screen` | Screenshot full desktop (save to file) |
| `Alt+Print Screen` | Screenshot active window (save to file) |
| `Super+Shift+S` | Screenshot region select (interactive snipping tool) |
| `Super+Shift+R` | Toggle screen recording **(custom)** |
| `Super+Print Screen` | Screenshot full desktop (copy to clipboard) **(custom)** |

#### Accessibility

| Shortcut | Action |
|----------|--------|
| `Super+Alt+S` | Toggle screen reader |
| `Super+Alt+M` | Toggle magnifier |
| `Super+=` / `Super+-` | Zoom in / out |
| `Super+Alt+0` | Reset zoom to 1× |
| `Super+Shift+F` | Toggle focus/distraction-free mode |
| `Ctrl` (press and release) | Cursor locator (if enabled) |

#### Text Editing (Global)

| Shortcut | Action |
|----------|--------|
| `Ctrl+C` / `Ctrl+X` / `Ctrl+V` | Copy / Cut / Paste |
| `Ctrl+Z` / `Ctrl+Shift+Z` | Undo / Redo |
| `Ctrl+A` | Select all |
| `Ctrl+F` | Find (in-app) |

#### Shortcut Configuration

```toml
# ~/.config/liquide/keybindings.toml

[system]
lock = "Super+L"
show_desktop = "Super+D"
session_menu = "Ctrl+Alt+Delete"
task_manager = "Ctrl+Shift+Escape"
settings = "Super+I"
file_manager = "Super+E"
terminal = "Super+T"
emoji_picker = "Super+period"
clipboard_history = "Super+V"
notification_center = "Super+A"
quick_settings = "Super+K"
launcher = "Super"

[window]
close = "Alt+F4"
maximize = "Super+Up"
restore_minimize = "Super+Down"
tile_left = "Super+Left"
tile_right = "Super+Right"
tile_top = "Super+Shift+Up"
tile_bottom = "Super+Shift+Down"
fullscreen = "Super+Return"
minimize = "Super+M"
minimize_others = "Super+Home"
move_to_monitor_left = "Super+Shift+Left"
move_to_monitor_right = "Super+Shift+Right"
switch_forward = "Alt+Tab"
switch_backward = "Alt+Shift+Tab"
overview = "Super+Tab"
title_bar_menu = "Alt+space"

[workspace]
prev = "Super+Ctrl+Left"
next = "Super+Ctrl+Right"
overview = "Super+Ctrl+Up"
add = "Super+Ctrl+D"
close = "Super+Ctrl+F4"
move_window_prev = "Super+Ctrl+Shift+Left"
move_window_next = "Super+Ctrl+Shift+Right"
# workspace_1..9 = "Super+Ctrl+1" .. "Super+Ctrl+9"

[screenshot]
full = "Print"
window = "Alt+Print"
region = "Super+Shift+S"
screen_record = "Super+Shift+R"
full_to_clipboard = "Super+Print"

[accessibility]
screen_reader = "Super+Alt+S"
magnifier = "Super+Alt+M"
zoom_in = "Super+equal"
zoom_out = "Super+minus"
zoom_reset = "Super+Alt+0"
focus_mode = "Super+Shift+F"
```

All shortcuts can be disabled by setting the value to `""`. Conflicts between user shortcuts and system shortcuts are flagged in the Settings UI. System shortcuts take precedence unless explicitly overridden.

### Window Management
- Tiling and floating hybrid (configurable default).
- Snap to edges/corners.
- Alt+Tab window switcher optimized for remote (shows live thumbnails if bandwidth allows, else icons).
- Window animations governed by CSS transitions and effect budget.

### Extensive Window Tiling
LiquiDE includes a full-featured tiling window manager that coexists with the floating mode:

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

LiquiDE supports a **seamless window mode** where individual remote application windows are "detached" from the LiquidClient container and presented as **native OS windows** on the client's local desktop. This is analogous to Citrix Seamless Windows, RDP RemoteApp, or VMware Unity mode.

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

#### Per-OS Taskbar / Dock Integration

In seamless mode, each remote application window integrates into the client OS's native taskbar or dock as if it were a local application. The LiquiDE client creates real native windows and registers them with the OS window manager.

| Platform | Taskbar Integration | Implementation |
|----------|-------------------|----------------|
| **Windows** | Each seamless window appears as a separate taskbar button with live thumbnail preview (`DwmSetWindowAttribute`). App grouping uses `AppUserModelID` mapped from the remote `app_id`. Jump lists populated from server-reported recent files. | Win32 `HWND` per window. `ITaskbarList4` for progress bars, overlay icons, thumbnail toolbars. |
| **macOS** | Each seamless app appears in the Dock with its icon. Clicking the Dock icon raises all windows for that app. App menu bar switches context per focused seamless window. Mission Control / Exposé shows seamless windows alongside local windows. | `NSWindow` per window. `NSApplication` delegate coordinates Dock icon and app activation. `NSRunningApplication` entries via Launch Services. |
| **Linux (Wayland)** | Each seamless window is an `xdg_toplevel` on the local compositor. Appears in GNOME Activities / KDE taskbar / panel normally. `app_id` mapped for correct grouping. Desktop entry (`.desktop`) files generated or referenced for proper theming. | `wl_surface` + `xdg_toplevel` per window. `xdg-activation-v1` for focus stealing prevention. |
| **Linux (X11)** | Each seamless window is a top-level X11 window with correct `WM_CLASS` and `_NET_WM_PID`. Taskbar grouping follows EWMH conventions. | `XCreateWindow` per window. EWMH/ICCCM properties set. NET_WM_WINDOW_TYPE set per window type. |

#### Taskbar Features in Seamless Mode

| Feature | Windows | macOS | Linux |
|---------|---------|-------|-------|
| App icon in taskbar/dock | Yes | Yes | Yes |
| Live thumbnail preview | Yes (DWM) | No (not a macOS concept) | Compositor-dependent |
| Progress bar overlay | Yes (`ITaskbarList3::SetProgressValue`) | Yes (Dock badge bounce / NSTouchBar) | No standard |
| Window grouping by app | Yes (`AppUserModelID`) | Yes (Dock groups by app) | Yes (`app_id`/`WM_CLASS`) |
| Badge / notification count | Yes (overlay icon) | Yes (Dock badge number) | Yes (Unity Launcher API / desktop entry) |
| Jump list / recent files | Yes (`ICustomDestinationList`) | No (use app Dock menu instead) | No standard |
| Alt+Tab / Task Switcher | Yes (native) | Yes (Cmd+Tab) | Yes (native) |
| Snap / tile with local windows | Yes (Windows Snap) | Yes (macOS tiling, Stage Manager) | Yes (compositor tiling) |
| Pin to taskbar | Yes | Yes (Keep in Dock) | Yes (add to favorites) |

#### System Tray Integration

Remote applications that create system tray icons (via `StatusNotifierItem` / `org.freedesktop.StatusNotifierItem` on the server) are forwarded to the client's native system tray:

```
Server: Remote app creates StatusNotifierItem
    │
    ▼
liquid-session: Intercepts D-Bus StatusNotifierItem registration
    │
    ▼
Protocol: seamless_tray_icon_create {
    item_id, app_id, icon_data, tooltip,
    menu_model (list of {label, action, icon, submenu, separator})
}
    │
    ▼
Client: Creates native system tray icon
    - Windows: Shell_NotifyIcon (NOTIFYICONDATA)
    - macOS: NSStatusItem
    - Linux: StatusNotifierItem (forwarded) or XEmbed fallback
```

| Message | Direction | Fields |
|---------|-----------|--------|
| `seamless_tray_icon_create` | Server → Client | `item_id`, `app_id`, `icon_data`, `tooltip`, `menu_model` |
| `seamless_tray_icon_update` | Server → Client | `item_id`, changed fields (icon, tooltip, menu) |
| `seamless_tray_icon_destroy` | Server → Client | `item_id` |
| `seamless_tray_icon_activate` | Client → Server | `item_id`, `action` (left-click, right-click, scroll) |
| `seamless_tray_menu_action` | Client → Server | `item_id`, `action_id` |

#### Notification Forwarding

Remote desktop notifications (via `org.freedesktop.Notifications` on the server) are forwarded to the client's native notification system:

| Platform | Native API | Features Forwarded |
|----------|-----------|-------------------|
| **Windows** | Toast notifications (`ToastNotificationManager`) | Title, body, icon, actions (buttons), urgency, sound hint |
| **macOS** | `UNUserNotificationCenter` | Title, subtitle, body, icon, actions, sound |
| **Linux** | `org.freedesktop.Notifications` D-Bus (passthrough) | Title, body, icon, actions, urgency, expire timeout |

Notification flow:
1. Remote application sends a D-Bus notification via `org.freedesktop.Notifications.Notify`.
2. `liquid-session` intercepts the notification (the session *is* the notification daemon when in seamless mode).
3. Notification is serialized and sent to the client via `seamless_notification` message.
4. Client renders a native OS notification.
5. If the user clicks an action button on the notification, the client sends `seamless_notification_action` back to the server.
6. Server dispatches the action to the original requesting application via D-Bus.

```
seamless_notification {
    notification_id, app_id, summary, body, icon_data,
    urgency (low/normal/critical), expire_timeout_ms,
    actions: [{action_id, label}],
    hints: {category, sound_name, desktop_entry}
}

seamless_notification_action {
    notification_id, action_id
}

seamless_notification_closed {
    notification_id, reason (expired/dismissed/action/revoked)
}
```

#### Drag-and-Drop Between Local and Remote Windows

LiquiDE supports drag-and-drop operations between seamless remote windows and local windows:

| Direction | Supported Types | Implementation |
|-----------|----------------|----------------|
| Local → Remote | Files, text, URIs | Client starts local DnD operation. On drop onto seamless window, client sends `seamless_dnd_drop` with data. Server injects matching Wayland `wl_data_offer`. |
| Remote → Local | Text, URIs, images | Server detects drag starting on a seamless window. Sends `seamless_dnd_start` to client. Client creates local `DoDragDrop` / `NSPasteboard` / `wl_data_source`. On drop to local window, data is committed locally. |
| Remote → Remote | All types | Normal Wayland DnD (handled server-side, seamless encoding follows the drag visual) |
| File transfers | Files | Drag of file from local → remote triggers file upload via file transfer channel (§14.1). File appears in session's upload directory. |

| Message | Direction | Fields |
|---------|-----------|--------|
| `seamless_dnd_start` | Server → Client | `source_window_id`, `offered_mime_types`, `icon_data` |
| `seamless_dnd_motion` | Server → Client | `x`, `y` (in virtual desktop coordinates) |
| `seamless_dnd_drop` | Client → Server | `target_window_id`, `mime_type`, `data` |
| `seamless_dnd_finished` | Server → Client | `accepted` |
| `seamless_dnd_cancel` | Either → Either | (drag cancelled by user or timeout) |

#### Multi-Monitor in Seamless Mode

Seamless windows can span multiple client monitors:

- The client reports its monitor layout (positions, sizes, DPI) to the server.
- The server's virtual desktop coordinate space is mapped to the client's physical monitor layout.
- Windows can be moved between client monitors by the user — the server receives `client_window_move` and updates the virtual coordinate mapping.
- DPI scaling per monitor: each seamless window is scaled according to the monitor it is primarily on (>50% area).
- When a window straddles two monitors with different DPI, the client uses the higher DPI and scales the lower-DPI portion.

#### Window Type Mapping

Server Wayland window types are mapped to native client window types:

| Server (Wayland) | Windows | macOS | Linux (Wayland) |
|-----------------|---------|-------|-----------------|
| `xdg_toplevel` (normal) | `WS_OVERLAPPEDWINDOW` | `NSWindow` (titled, resizable) | `xdg_toplevel` |
| `xdg_toplevel` (dialog) | `WS_OVERLAPPED \| WS_DLGFRAME` | `NSPanel` (floating) | `xdg_toplevel` (parent set) |
| `xdg_popup` (menu) | `WS_POPUP` (borderless) | `NSMenu` or borderless `NSWindow` | `xdg_popup` |
| `xdg_popup` (tooltip) | `WS_POPUP` + `TTS_BALLOON` | `NSPopover` or borderless window | Tooltip surface |
| `zwlr_layer_surface` (overlay) | `WS_EX_TOPMOST` + `WS_EX_TOOLWINDOW` | `NSPanel` (floating, nonactivating) | Layer shell (if compositor supports) |

#### Seamless Mode Limitations
- Additional per-window encoding overhead on the server (separate damage tracking and encode regions).
- Transient windows (menus, tooltips, dropdowns) may flicker or misposition if they extend beyond their parent window bounds. The server groups these with their parent.
- Audio remains session-wide (not per-window spatial audio).
- DE shell elements (dock, status bar) are not shown by default — optionally presented as their own native windows (`shell_as_window = true`).
- Drag-and-drop of files is limited to the file transfer mechanism (no instant local file path injection).
- Client-local accessibility tools may not be able to inspect remote window content (see §23a for accessibility models).
- Alt+Tab ordering between local and remote windows is managed by the client OS — z-order synchronization may have slight latency.

#### Seamless Mode Configuration (Server-Side)
```toml
# Per-user session.toml
[seamless]
enabled = false                           # user preference for seamless mode
default_mode = "desktop"                  # "desktop" (normal) or "seamless" (remote apps as native windows)
exclude_apps = ["liquide-desktop"]       # apps that stay on the virtual desktop
shell_as_window = false                   # show dock/status bar as separate native windows
forward_notifications = true              # forward remote notifications to client native notifications
forward_tray_icons = true                 # forward remote tray icons to client system tray
dnd_enabled = true                        # drag-and-drop between local and remote windows
dnd_max_payload_mb = 50                   # max DnD payload size

[seamless.taskbar]
show_app_icons = true                     # app icons in client taskbar
show_progress = true                      # progress bar overlays
group_by_app = true                       # group windows by app_id
generate_desktop_entries = true           # create .desktop files for seamless apps (Linux)
jump_list_recent_files = 10               # number of recent files in jump lists (Windows)

[seamless.notifications]
forward = true
urgency_threshold = "low"                 # forward notifications at this urgency or above
sound = "native"                          # "native" (client sound), "remote" (server sound), "none"
max_concurrent = 5                        # max visible notifications before stacking
```

### Notifications
- "Stacking" to avoid animation storms.
- Do-not-disturb mode.
- Notification history panel.

---

## 14b) WASM Plugin & Extension System

### Overview

LiquiDE provides a comprehensive **WebAssembly (WASM)-based plugin system** that allows third-party and user-created extensions to augment the desktop environment. Plugins run in sandboxed WASM runtimes with strict memory and CPU resource limits, ensuring that no plugin can crash, hang, or compromise the host session.

The plugin system is designed for:
- **Performance**: Near-native execution speed via ahead-of-time compiled WASM. Minimal overhead per call. Memory-pooled allocations.
- **Safety**: Complete isolation — plugins cannot access host memory, other plugins, or the filesystem beyond what is explicitly granted via host functions.
- **Stability**: Versioned ABIs ensure plugins remain compatible across LiquiDE updates. Deprecation policy gives plugin authors time to migrate.
- **Extensibility**: Nine distinct extension points cover shell UI, input processing, notifications, file handling, theming, and more.

### Plugin Architecture

#### WASM Runtime

LiquiDE uses **wasmtime** as its WASM runtime:
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

LiquiDE provides **nine extension points** that plugins can register for. A plugin may register for one or more extension points.

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
liquide_min_version = "0.1.0"
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

- **System plugins**: `/etc/liquide/plugins/<plugin-id>/plugin.toml` + `plugin.wasm`
- **User plugins**: `~/.config/liquide/plugins/<plugin-id>/plugin.toml` + `plugin.wasm`
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

Storage is plugin-scoped (isolated per plugin) and stored at `~/.config/liquide/plugin-data/<plugin-id>/`.

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

The primary SDK is a Rust crate: `liquide-plugin-sdk`.

```rust
use liquide_plugin_sdk::prelude::*;

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

#### User-Side (`~/.config/liquide/session.toml`)
```toml
[plugins]
enabled_plugins = []                     # empty = all installed; list IDs to restrict
disabled_plugins = ["com.example.broken-plugin"]  # explicitly disabled
```

#### Per-Plugin (`~/.config/liquide/plugins/<id>/config.toml`)
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
  relying_party_id = "liquide.example.com"
  attestation = "none"                 # none, indirect, direct

  [auth.smartcard]
  pkcs11_module = "/usr/lib/opensc-pkcs11.so"
  ca_certificates = ["/etc/liquide/smartcard-ca.pem"]
  require_pin = true
  ```

#### Smart Card & Credential Forwarding

Smart cards (PIV, CAC, OpenPGP) are used in enterprise and government environments for authentication, digital signatures, and encryption. LiquiDE supports two smart card usage models:

##### Model 1: Client-Side Authentication (Login Only)

The smart card is used **on the client** to authenticate the user. The server never sees the smart card directly.

```
Client                                Server
  │                                     │
  │  Smart card inserted locally        │
  │  Client reads certificate via       │
  │  PKCS#11 / platform API             │
  │                                     │
  │  ── LoginResponse (certificate) ──► │
  │                                     │  Server verifies certificate chain
  │  ◄── LoginChallenge (nonce) ──────  │  against trusted CA list
  │                                     │
  │  Client signs nonce with            │
  │  smart card private key             │
  │  (PIN entry on client)              │
  │                                     │
  │  ── LoginChallengeResponse ──────►  │
  │                                     │  Server verifies signature
  │  ◄── LoginSuccess ────────────────  │
```

This model is the **default and recommended** approach. The private key never leaves the smart card, and the smart card never leaves the client machine. The PIN is entered locally on the client.

Platform APIs for client-side smart card access:

| Platform | API | Notes |
|----------|-----|-------|
| **Windows** | Windows Smart Card API (winscard.dll), CNG (NCryptSignHash) | Integrated with Windows Hello and Windows credential providers |
| **macOS** | CryptoTokenKit framework | Automatic token discovery, Keychain integration |
| **Linux** | PKCS#11 via `opensc-pkcs11.so` or `p11-kit` | Supports PC/SC-lite daemon |
| **Web** | WebAuthn (FIDO2 with smart card transport) | Limited to FIDO2-compliant cards; no raw PKCS#11 |

##### Model 2: Smart Card Forwarding (In-Session Access)

Some applications running **inside the remote session** need direct smart card access — email signing (S/MIME), document signing, VPN clients, or internal web applications that require client certificates.

Smart card forwarding tunnels the PC/SC (Personal Computer/Smart Card) API calls over the LiquiDE connection:

```
Remote Application (in-session)
    │
    ▼
Virtual PC/SC reader (pcscd socket, session-local)
    │
    ▼
LiquiDE Smart Card Channel (dedicated transport channel)
    │
    ▼ (encrypted, over session transport)
    │
LiquiDE Client (smart card proxy)
    │
    ▼
Physical PC/SC reader (client's pcscd or platform API)
    │
    ▼
Physical Smart Card
```

| Component | Description |
|-----------|-------------|
| Virtual reader | A `pcscd` instance in the session namespace with a virtual reader backed by LiquiDE |
| Transport channel | Dedicated reliable channel for PC/SC APDU (Application Protocol Data Unit) forwarding |
| Client proxy | Translates between LiquiDE protocol messages and local PC/SC API calls |
| Latency | PC/SC commands are request-response. Expect RTT-dependent latency per APDU exchange. |

**Key protection guarantees**:
- **Private keys never traverse the network**. Only APDU commands (cryptographic operation requests) are forwarded. The smart card performs all cryptographic operations locally on the client.
- **PIN entry**: configurable — `local` (entered on client, never sent to server, forwarded via PC/SC PIN verification), `remote` (entered in session UI, sent via secure channel to client PC/SC — less secure, but needed for some workflows). Default: `local`.
- **Card insertion/removal events** are forwarded in real-time.
- **Multiple readers**: up to 4 simultaneous readers forwarded per session.

Configuration:

```toml
[smartcard]
# Master switch: enable smart card forwarding
forwarding_enabled = false              # disabled by default

# Pin entry mode
pin_entry = "local"                     # local (client-side), remote (session-side)

# Maximum concurrent forwarded readers
max_readers = 4

# Allowed card ATR (Answer To Reset) patterns — empty = all cards allowed
# Use this to restrict forwarding to specific card types (e.g., only PIV cards)
allowed_atr_patterns = []

# Block specific card operations (APDU filtering)
# These INS (instruction) bytes are blocked. Default: none.
# Example: block GENERATE_KEY_PAIR (0x46) to prevent key generation over remote
blocked_apdu_ins = []

# Audit logging for smart card operations
audit_log = true

# Policy: which users/groups can use smart card forwarding
# (Uses standard policy engine — see §15 policy keys)
```

Policy keys for smart card forwarding:

| Key | Type | Resolution | Default | Description |
|-----|------|-----------|---------|-------------|
| `smartcard.forwarding_enabled` | bool | deny_overrides | `false` | Allow smart card forwarding |
| `smartcard.pin_entry` | enum | highest_precedence | `"local"` | PIN entry location |
| `smartcard.allowed_atr_patterns` | list | intersection | `[]` (all) | Allowed card ATR patterns |

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
  client_ca_file = "/etc/liquide/client-ca.pem"
  crl_file = "/etc/liquide/crl.pem"
  ocsp_enabled = false
  ocsp_responder_url = ""
  username_field = "CN"                # CN, SAN:email, SAN:upn
  ```

#### Enterprise Identity Architecture

LiquiDE supports a layered identity integration model. The architecture is designed so that the auth stack can be composed from independent modules — each deployment chooses the combination that fits its identity infrastructure.

##### Identity Provider Hierarchy

```
┌───────────────────────────────────────────────────────────────┐
│                    LiquiDE Auth Subsystem                       │
│                                                               │
│  ┌─────────┐  ┌─────────┐  ┌──────────┐  ┌───────────────┐  │
│  │  Local   │  │   PAM   │  │  OIDC /  │  │    SAML 2.0   │  │
│  │ Accounts │  │ (system)│  │  OAuth2  │  │  (optional)   │  │
│  └────┬─────┘  └────┬────┘  └────┬─────┘  └──────┬────────┘  │
│       │             │            │               │            │
│       └─────────────┴────────┬───┴───────────────┘            │
│                              │                                 │
│                     ┌────────▼────────┐                        │
│                     │  Identity       │                        │
│                     │  Resolution     │                        │
│                     │  (username →    │                        │
│                     │   uid, groups,  │                        │
│                     │   attributes)   │                        │
│                     └────────┬────────┘                        │
│                              │                                 │
│  ┌───────────────────────────▼───────────────────────────┐    │
│  │              MFA Layer (always after primary auth)      │    │
│  │  TOTP · FIDO2/WebAuthn · Smart Card · Biometric        │    │
│  └───────────────────────────┬───────────────────────────┘    │
│                              │                                 │
│  ┌───────────────────────────▼───────────────────────────┐    │
│  │         Session Binding (token + claims)                │    │
│  │  session_token bound to: user, session_id, IP (opt),   │    │
│  │  device_id (opt), MFA assertion, expiry                 │    │
│  └────────────────────────────────────────────────────────┘    │
└───────────────────────────────────────────────────────────────┘
```

##### OIDC / OAuth 2.0 (Recommended for Enterprise SSO)

OIDC is the **recommended** enterprise identity integration. LiquiDE acts as an OIDC Relying Party (RP).

| Property | Specification |
|----------|--------------|
| **Supported flows** | Authorization Code with PKCE (for interactive login), Device Authorization Grant (for headless), Client Credentials (for service accounts / gateway-to-server) |
| **Discovery** | OpenID Connect Discovery 1.0 (`.well-known/openid-configuration`). Auto-configures endpoints from issuer URL. |
| **Token validation** | ID token validated locally (JWT signature verification using JWKS endpoint). Access token used for UserInfo endpoint calls. |
| **Claims mapping** | Configurable mapping from OIDC claims to LiquiDE identity: `sub` → uid, `preferred_username` → username, `groups` → group membership, `email` → contact |
| **Group sync** | Groups from the `groups` claim (array of strings) are synced to LiquiDE's group-based policy system at login. |
| **Token refresh** | Refresh tokens stored server-side (in-memory). Session remains valid as long as the refresh token is valid. Token refresh happens transparently. |
| **Logout** | OIDC Back-Channel Logout 1.0 (server receives logout token → terminates session). Front-Channel Logout as fallback. |
| **Session binding** | The OIDC `sub` + `iss` pair is the canonical user identity. Session tokens reference this pair. |
| **Tested providers** | Microsoft Entra ID (Azure AD), Okta, Auth0, Keycloak, Google Workspace, AWS IAM Identity Center, Authentik, Zitadel |

Configuration:

```toml
[auth.oidc]
enabled = false
issuer = "https://auth.example.com/realms/company"
client_id = "liquide-server"
client_secret_file = "/etc/liquide/oidc-client-secret"  # or use client_secret env var
scopes = ["openid", "profile", "groups", "email"]
# Claims mapping (OIDC claim → LiquiDE field)
username_claim = "preferred_username"  # or "sub", "email", "upn"
groups_claim = "groups"                # claim containing group membership array
uid_claim = "sub"                      # stable unique identifier
display_name_claim = "name"
email_claim = "email"
# Advanced
audience = ""                          # expected 'aud' claim (default: client_id)
token_endpoint_auth_method = "client_secret_post"  # client_secret_post, client_secret_basic, private_key_jwt
allowed_issuers = []                   # restrict to specific issuers (empty = issuer config only)
jwks_cache_ttl_sec = 3600             # JWKS cache duration
back_channel_logout_enabled = true
back_channel_logout_path = "/auth/oidc/logout"
```

##### SAML 2.0 (Optional, for Legacy Enterprise)

SAML is supported for organizations that have not migrated to OIDC.

| Property | Specification |
|----------|--------------|
| **Role** | Service Provider (SP) |
| **Bindings** | HTTP-POST (assertions), HTTP-Redirect (requests) |
| **Assertion** | Signed (required), optionally encrypted |
| **NameID** | `urn:oasis:names:tc:SAML:2.0:nameid-format:persistent` (preferred), `emailAddress`, `unspecified` |
| **Attribute mapping** | Configurable: SAML attributes → LiquiDE identity fields |
| **Single Logout** | SAML SLO (HTTP-Redirect binding) |
| **Metadata** | SP metadata served at `/auth/saml/metadata`. IdP metadata imported from URL or file. |

Configuration:

```toml
[auth.saml]
enabled = false
idp_metadata_url = "https://idp.example.com/metadata"  # or idp_metadata_file
sp_entity_id = "https://liquide.example.com/auth/saml"
sp_acs_url = "https://liquide.example.com/auth/saml/acs"
sp_slo_url = "https://liquide.example.com/auth/saml/slo"
sp_private_key_file = "/etc/liquide/saml-sp.key"
sp_certificate_file = "/etc/liquide/saml-sp.crt"
username_attribute = "uid"
groups_attribute = "memberOf"
sign_requests = true
want_assertions_signed = true
want_assertions_encrypted = false
```

##### LDAP / Active Directory Sync

LDAP is used for **directory synchronization** (user and group information), not as a primary authentication protocol. Authentication goes through PAM (which itself may use LDAP) or OIDC.

| Property | Specification |
|----------|--------------|
| **Purpose** | User enumeration (for admin UI), group membership sync, attribute lookup |
| **Protocol** | LDAP v3 over TLS (LDAPS) or StartTLS. Plaintext LDAP is rejected. |
| **Bind** | Service account bind DN + password. Supports SASL (GSSAPI for Kerberos environments). |
| **Sync schedule** | Periodic (default: every 15 minutes) + on-demand via `liquidctl directory sync` |
| **User search** | Configurable base DN, filter, attribute mappings |
| **Group search** | Nested group resolution (AD `memberOf:1.2.840.113556.1.4.1941:=`) supported |
| **SCIM** | SCIM 2.0 endpoint (`/scim/v2/`) for push-based directory updates from identity providers that support SCIM (Okta, Azure AD, OneLogin). SCIM provisioning creates/updates/deactivates user records. |

Configuration:

```toml
[directory]
enabled = false
type = "ldap"                          # ldap, active_directory, scim

[directory.ldap]
url = "ldaps://ldap.example.com:636"
bind_dn = "cn=liquide-svc,ou=services,dc=example,dc=com"
bind_password_file = "/etc/liquide/ldap-password"
user_base_dn = "ou=users,dc=example,dc=com"
user_filter = "(&(objectClass=posixAccount)(uid={username}))"
user_attributes = { username = "uid", display_name = "cn", email = "mail", uid_number = "uidNumber" }
group_base_dn = "ou=groups,dc=example,dc=com"
group_filter = "(objectClass=posixGroup)"
group_member_attribute = "memberUid"
sync_interval_sec = 900               # 15 minutes
tls_ca_file = ""                       # custom CA for LDAPS (default: system CA)
connection_pool_size = 5
timeout_sec = 10

[directory.scim]
enabled = false
listen = "127.0.0.1:9405/scim/v2"
auth_token = ""                        # bearer token for SCIM requests
auto_create_users = false              # create local user on SCIM provision
auto_deactivate = true                 # deactivate user on SCIM deprovision
```

##### Device Posture Assessment (Optional)

For high-security deployments, the server can evaluate client device posture before granting full access.

| Check | Source | Action on Failure |
|-------|--------|-------------------|
| Client version minimum | ClientHello `client_version` | Warn or deny (configurable) |
| OS version minimum | ClientHello `client_platform` | Warn or deny |
| Encryption at rest | Client-reported attestation | Reduce clipboard/file transfer permissions |
| MDM enrollment | Client-reported attestation or external MDM API | Deny connectivity or restrict features |
| Certificate compliance | mTLS certificate attributes | Deny if cert doesn't meet policy |

Device posture is evaluated after authentication and affects the effective policy for the session. Posture checks are **optional** and disabled by default.

```toml
[auth.device_posture]
enabled = false
min_client_version = ""                # e.g., "0.3.0" — reject older clients
min_client_version_action = "warn"     # warn, deny
require_encryption_attestation = false
require_mdm_enrollment = false
mdm_api_url = ""                       # external MDM check endpoint
mdm_api_token_file = ""
posture_refresh_interval_sec = 3600    # re-check posture periodically
```

##### Auth Method Priority & Fallback

When multiple auth methods are configured, the server evaluates them in priority order:

1. **Certificate-based (mTLS)** — if the client presents a valid client certificate, it is used as the primary identity. MFA may still be required.
2. **OIDC** — redirect to IdP for authentication.
3. **SAML** — redirect to IdP for authentication.
4. **PAM** — system-level authentication (password, Kerberos, LDAP via PAM modules).
5. **Local accounts** — server-local user database.

The first method that the client supports and that succeeds is used. Admins can restrict available methods per group or globally.

### Login Screen

The login screen is the first visual experience a user has with LiquiDE. It is a full-screen, Liquid Glass themed interface that presents authentication options with elegance and clarity.

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

#### Policy Hierarchy & Resolution

Policies are evaluated from four sources, in ascending precedence order:

```
┌───────────────────────────────────────────────────────────────────┐
│ 4. Session Override  (highest — runtime overrides via liquidctl)  │
├───────────────────────────────────────────────────────────────────┤
│ 3. User Policy       (/etc/liquide/policies/users/<user>.toml)   │
├───────────────────────────────────────────────────────────────────┤
│ 2. Group Policy      (/etc/liquide/policies/groups/<group>.toml) │
├───────────────────────────────────────────────────────────────────┤
│ 1. Server Policy     (/etc/liquide/policies/server.toml)         │
│    (lowest — applies to all sessions unless overridden)          │
└───────────────────────────────────────────────────────────────────┘
```

**Resolution rules:**

1. **Last-writer-wins by precedence**: for each policy key, the highest-precedence source that defines the key wins. If a user policy sets `clipboard.enabled = false`, it overrides the server policy regardless of the server's value.

2. **Group membership**: a user may belong to multiple groups. When multiple groups define the same key, the **most restrictive value wins** (deny-overrides-allow). Groups are evaluated in alphabetical order only for tie-breaking of non-boolean keys.

3. **Deny always wins**: for boolean permission keys (`*.enabled`, `*.allowed`), `false` (deny) at any level overrides `true` (allow) at lower levels. A server policy of `clipboard.enabled = true` can be overridden to `false` by a group or user, but a server `false` **cannot** be overridden to `true` by a user (deny is sticky downward). This is the **deny-overrides-allow** principle.

4. **Numeric keys — most restrictive**: for numeric limits (`max_sessions`, `max_fps`, `max_resolution_width`, `clipboard.max_size`), the **lowest value** among all sources that define the key wins. A server allowing `max_fps = 60` and a group setting `max_fps = 30` results in an effective value of 30.

5. **List keys — intersection**: for list keys (`allowed_transports`, `allowed_codecs`, `allowed_mime_types`), the effective value is the **intersection** of all sources. A server allowing `["quic", "tcp", "udp"]` and a group allowing `["quic", "tcp"]` results in `["quic", "tcp"]`.

6. **String keys — highest precedence**: for non-list, non-boolean, non-numeric keys (e.g., `rendering.profile`, `transport.default`), the highest-precedence source wins.

7. **Session overrides**: admin can set per-session overrides via `liquidctl session policy <session_id> set <key> <value>`. These have the highest precedence but are ephemeral (lost on session restart). Used for debugging or temporary adjustments.

**Policy file format:**

```toml
# /etc/liquide/policies/server.toml
[clipboard]
enabled = true
direction = "both"                   # both, server_to_client, client_to_server
max_size = 52428800                  # 50 MB
allowed_mime_types = ["text/*", "image/png", "image/jpeg"]

[audio]
enabled = true
direction = "both"                   # both, playback, capture

[usb]
enabled = false
allowed_classes = []
allowed_devices = []
blocked_devices = []

[file_transfer]
enabled = true
max_size = 1073741824               # 1 GB
direction = "both"

[session]
max_per_user = 3
max_duration_sec = 86400            # 24 hours
max_idle_sec = 3600                 # 1 hour
token_lifetime_sec = 86400

[rendering]
max_resolution_width = 7680         # 8K
max_resolution_height = 4320
max_fps = 60
profile = "balanced"

[transport]
allowed = ["quic", "tcp", "udp", "websocket"]

[plugins]
enabled = true
require_signatures = false
allowed_plugins = []                # empty = all allowed
blocked_plugins = []
```

```toml
# /etc/liquide/policies/groups/developers.toml
[clipboard]
direction = "both"
max_size = 104857600                # 100 MB — developers get larger clipboard

[usb]
enabled = true
allowed_classes = ["hid", "hub"]    # developers can use USB keyboards/mice

[session]
max_per_user = 5                    # developers get more sessions
```

```toml
# /etc/liquide/policies/users/alice.toml
[rendering]
profile = "quality"                 # Alice has a powerful workstation
max_fps = 120
```

**Worked example — Alice (member of `developers` group):**

| Key | Server | Group: `developers` | User: `alice` | Effective | Rule |
|-----|--------|---------------------|---------------|-----------|------|
| `clipboard.enabled` | `true` | (not set) | (not set) | `true` | Server default |
| `clipboard.max_size` | 50 MB | 100 MB | (not set) | 50 MB | Numeric: min(50M, 100M) = 50M |
| `usb.enabled` | `false` | `true` | (not set) | `false` | Deny-overrides-allow: server `false` wins |
| `session.max_per_user` | 3 | 5 | (not set) | 3 | Numeric: min(3, 5) = 3 |
| `rendering.profile` | `"balanced"` | (not set) | `"quality"` | `"quality"` | String: highest precedence (user) wins |
| `rendering.max_fps` | 60 | (not set) | 120 | 60 | Numeric: min(60, 120) = 60 |

> **Note on `usb.enabled`**: the server sets `false` (deny). Even though the developers group sets `true`, the deny-overrides-allow rule means the effective value is `false`. To grant USB to developers, the server policy must set `usb.enabled = true` (or not set it, defaulting to `false`) and the group policy grants `true`. If the admin wants to allow USB only for specific groups, they should set the server policy to `true` and use group policies to restrict — or set server `false` and use session overrides.

**Effective policy introspection:**

```bash
# View effective policy for a user (before session starts)
liquidctl policy effective --user alice
# Output: merged policy with source annotations
# clipboard.enabled = true           [source: server]
# clipboard.max_size = 52428800      [source: server (min of server:52428800, group:developers:104857600)]
# usb.enabled = false                [source: server (deny-overrides-allow)]
# rendering.profile = "quality"      [source: user:alice]
# rendering.max_fps = 60             [source: server (min of server:60, user:alice:120)]

# View effective policy for a running session
liquidctl session policy s-001

# View which source defined each key
liquidctl policy effective --user alice --show-sources

# Test what-if scenarios
liquidctl policy effective --user alice --add-group contractors
```

#### Formal Policy Schema

Every policy key is formally defined in a schema that governs validation, resolution, and audit behavior. The policy engine rejects keys not present in the schema and values that fail type/range validation.

**Policy Key Schema Definition:**

| Field | Type | Description |
|-------|------|-------------|
| `key` | dotted path | Qualified key name (e.g., `clipboard.enabled`) |
| `type` | enum | `bool`, `uint`, `string`, `string_list`, `enum` |
| `resolution_rule` | enum | How conflicts resolve: `deny_overrides` (bool), `min` (uint), `intersection` (list), `highest_precedence` (string/enum) |
| `default` | any | Default value when no source defines the key |
| `range` | optional | For `uint`: `[min, max]`. For `enum`: `[allowed values]`. For `string_list`: `[allowed items]`. |
| `audited` | bool | Whether changes to this key emit an audit event |
| `locked` | bool | If `true`, only server policy can set this key (user/group overrides ignored) |
| `description` | text | Human-readable purpose |

**Complete Policy Key Catalog:**

| Key | Type | Resolution | Default | Range | Audited | Description |
|-----|------|-----------|---------|-------|---------|-------------|
| `clipboard.enabled` | bool | deny_overrides | `true` | — | Yes | Enable clipboard sync |
| `clipboard.direction` | enum | highest_precedence | `"both"` | `both`, `s2c`, `c2s` | Yes | Allowed clipboard direction |
| `clipboard.max_size` | uint | min | `52428800` | [0, 1073741824] | No | Max clipboard size (bytes) |
| `clipboard.allowed_mime_types` | string_list | intersection | `["text/*", "image/png", "image/jpeg"]` | — | No | Allowed MIME types |
| `audio.enabled` | bool | deny_overrides | `true` | — | Yes | Enable audio |
| `audio.direction` | enum | highest_precedence | `"both"` | `both`, `playback`, `capture` | Yes | Audio direction |
| `usb.enabled` | bool | deny_overrides | `false` | — | Yes | Enable USB redirect |
| `usb.allowed_classes` | string_list | intersection | `[]` | — | Yes | Allowed USB device classes |
| `usb.allowed_devices` | string_list | intersection | `[]` | — | Yes | Allowed USB VID:PID pairs |
| `usb.blocked_devices` | string_list | intersection | `[]` | — | Yes | Blocked USB VID:PID pairs |
| `file_transfer.enabled` | bool | deny_overrides | `true` | — | Yes | Enable file transfer |
| `file_transfer.max_size` | uint | min | `1073741824` | [0, 10737418240] | No | Max file size (bytes) |
| `file_transfer.direction` | enum | highest_precedence | `"both"` | `both`, `upload`, `download` | Yes | Transfer direction |
| `camera.enabled` | bool | deny_overrides | `false` | — | Yes | Enable camera passthrough |
| `session.max_per_user` | uint | min | `3` | [1, 100] | No | Max concurrent sessions |
| `session.max_duration_sec` | uint | min | `86400` | [300, 604800] | No | Max session duration |
| `session.max_idle_sec` | uint | min | `3600` | [60, 86400] | No | Max idle time before disconnect |
| `session.token_lifetime_sec` | uint | min | `86400` | [300, 604800] | No | Session token lifetime |
| `rendering.max_resolution_width` | uint | min | `7680` | [640, 15360] | No | Max horizontal resolution |
| `rendering.max_resolution_height` | uint | min | `4320` | [480, 8640] | No | Max vertical resolution |
| `rendering.max_fps` | uint | min | `60` | [1, 240] | No | Max frame rate |
| `rendering.profile` | enum | highest_precedence | `"balanced"` | `minimal`, `performance`, `balanced`, `quality` | No | Rendering quality profile |
| `transport.allowed` | string_list | intersection | `["quic", "tcp", "udp", "websocket"]` | — | No | Allowed transports |
| `plugins.enabled` | bool | deny_overrides | `true` | — | Yes | Enable WASM plugins |
| `plugins.require_signatures` | bool | deny_overrides | `false` | — | Yes | Require signed plugins |
| `plugins.allowed_plugins` | string_list | intersection | `[]` (empty = all) | — | Yes | Allowed plugin IDs |
| `plugins.blocked_plugins` | string_list | intersection | `[]` | — | Yes | Blocked plugin IDs |
| `plugins.install` | enum | highest_precedence | `"admin-only"` | `admin-only`, `user`, `disabled` | Yes | Plugin install permission |

New policy keys MAY be added in minor versions. Unknown keys in policy files MUST be logged as warnings and ignored (forward compatibility). The policy schema is the authoritative definition — the prose descriptions in this section are informative.

**Policy schema file location:** `crates/liquide-policy/schema/policy_schema.toml`

#### Evaluation Semantics

The policy engine evaluates the effective policy for a session at two points:

1. **Session creation** — the full effective policy is computed and frozen for the session lifetime (except for session overrides).
2. **Runtime policy check** — individual policy keys are checked during operations (e.g., clipboard paste checks `clipboard.enabled` and `clipboard.direction`).

**Evaluation algorithm (pseudocode):**

```
function evaluate_effective_policy(user, groups) -> EffectivePolicy:
    effective = {}

    for key in POLICY_SCHEMA:
        sources = []

        # Collect values from all sources
        if server_policy.has(key):
            sources.push({value: server_policy[key], precedence: 1, source: "server"})

        for group in sort(groups):  # alphabetical for tie-breaking
            if group_policy[group].has(key):
                sources.push({value: group_policy[group][key], precedence: 2, source: "group:" + group})

        if user_policy[user].has(key):
            sources.push({value: user_policy[user][key], precedence: 3, source: "user:" + user})

        if session_override.has(key):
            sources.push({value: session_override[key], precedence: 4, source: "session_override"})

        # Resolve based on schema rule
        match POLICY_SCHEMA[key].resolution_rule:
            deny_overrides:
                # Any source setting false → false. All true → true.
                if any(s.value == false for s in sources):
                    effective[key] = {value: false, source: first_deny_source}
                elif sources:
                    effective[key] = {value: true, source: highest_precedence_source}
                else:
                    effective[key] = {value: POLICY_SCHEMA[key].default, source: "default"}

            min:
                # Lowest numeric value wins
                if sources:
                    winner = min(sources, key=s.value)
                    effective[key] = {value: winner.value, source: winner.source}
                else:
                    effective[key] = {value: POLICY_SCHEMA[key].default, source: "default"}

            intersection:
                # Intersection of all lists
                if sources:
                    result = sources[0].value
                    for s in sources[1:]:
                        if s.value:  # non-empty list restricts
                            result = intersect(result, s.value)
                    effective[key] = {value: result, source: "intersection"}
                else:
                    effective[key] = {value: POLICY_SCHEMA[key].default, source: "default"}

            highest_precedence:
                # Highest precedence source wins
                if sources:
                    winner = max(sources, key=s.precedence)
                    effective[key] = {value: winner.value, source: winner.source}
                else:
                    effective[key] = {value: POLICY_SCHEMA[key].default, source: "default"}

        # Validate
        validate_range(key, effective[key].value)

    return effective
```

**Key invariants:**

| Invariant | Description |
|-----------|-------------|
| **Deny is irrevocable** | A `deny_overrides` key set to `false` at **any** level cannot be overridden to `true` by a higher-precedence source. Exception: session overrides (admin `liquidctl` command) can override deny — this is intentional for break-glass scenarios and is audit-logged. |
| **Empty list means "all allowed"** | For `string_list` keys, an empty list (`[]`) at any source means "no restriction from this source". A non-empty list restricts. The intersection of an empty list with any list is the non-empty list. |
| **Locked keys** | If the schema marks a key as `locked: true`, only the server policy source is considered. Group, user, and session override sources are ignored. |
| **Schema validation** | Values that fail range/type validation are rejected at policy load time (the policy file fails to load). Runtime policy checks never encounter invalid values. |
| **Deterministic** | For the same set of policy files and user/group membership, the evaluation always produces the same effective policy. No randomness, no time-dependence. |

#### Policy Change Notification

When a policy file is modified on disk (detected via `inotify`/`kqueue`):

1. Policy engine reloads the changed file.
2. For each active session affected by the change, the new effective policy is computed.
3. Changed keys are compared against the previous effective policy.
4. For each changed key:
   - If the key is `audited`: emit an audit event (`policy_change`, old value, new value, source).
   - If the key affects an active operation (e.g., `clipboard.enabled` toggled to `false` during an active clipboard transfer): the operation is canceled gracefully.
5. The session receives a `PolicyUpdate` control message (see spec-protocol-formal.md §5.1) notifying it of the changed keys.
6. The client receives a `ConfigUpdate` if the change affects client-visible behavior.

Policy changes take effect **within 5 seconds** of the file modification. No session restart required.

#### Audit Emission Rules

Policy-related audit events follow a strict emission contract:

| Event | Trigger | Required Fields | Log Level |
|-------|---------|----------------|-----------|
| `policy.load` | Policy file loaded/reloaded | `file_path`, `key_count`, `errors` | `info` |
| `policy.load_error` | Policy file fails validation | `file_path`, `error`, `line` | `error` |
| `policy.change` | Effective policy key changed for session | `session_id`, `user`, `key`, `old_value`, `new_value`, `source` | `info` |
| `policy.deny` | Operation blocked by policy | `session_id`, `user`, `key`, `attempted_action`, `source` | `warn` |
| `policy.override` | Session override applied (via `liquidctl`) | `session_id`, `admin_user`, `key`, `value` | `warn` |
| `policy.evaluation` | Effective policy computed for new session | `session_id`, `user`, `groups`, `key_count` | `debug` |

All audit events are structured JSON (see spec-observability.md §4) and are always emitted regardless of log level configuration — the audit subsystem has its own output path:

```toml
[audit]
enabled = true
output = "/var/log/liquide/audit.log"    # dedicated audit log file
format = "json"                           # json or cef (Common Event Format)
retention_days = 90
max_size_mb = 500
sign_entries = false                      # optional: cryptographic signing of audit entries
```
- Log:
  - Login success/failure with source IP.
  - Session start/stop with duration.
  - Policy changes.
  - Clipboard/file transfer events (metadata).
  - USB device attach/detach.
  - Administrative actions.

### Intrusion Prevention (fail2ban Integration)
- LiquiDE integrates with **fail2ban** for automated intrusion prevention.
- Server emits structured authentication events to a log file or syslog that fail2ban can monitor.
- **Built-in fail2ban jails** ship with the server:
  - `liquide-auth` — ban IPs after repeated authentication failures.
  - `liquide-brute` — ban IPs attempting rapid connection attempts.
  - `liquide-proto` — ban IPs sending malformed protocol messages.
- Jail configuration (shipped as `/etc/fail2ban/jail.d/liquide.conf`):
  ```ini
  [liquide-auth]
  enabled = true
  filter = liquide-auth
  logpath = /var/log/liquide/auth.log
  maxretry = 5
  findtime = 600
  bantime = 3600
  action = iptables-multiport[name=liquide, port="3389,3390"]

  [liquide-brute]
  enabled = true
  filter = liquide-brute
  logpath = /var/log/liquide/auth.log
  maxretry = 20
  findtime = 60
  bantime = 86400

  [liquide-proto]
  enabled = true
  filter = liquide-proto
  logpath = /var/log/liquide/server.log
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
  fail2ban_log = "/var/log/liquide/auth.log"
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

LiquiDE's WASM plugin system (§14b) introduces a controlled extension surface that must be secured against both malicious and buggy plugins. The following security properties are enforced:

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
- LiquiDE supports hiding its identity from network scanners and unauthorized probes.
- **Protocol obfuscation**:
  - The initial handshake can be disguised to not reveal the service type.
  - Connection attempts without a valid protocol version header receive no response (silent drop).
  - Configurable banner/identification:
    - `default` — identifies as LiquiDE (standard).
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
  silent_drop_unknown = true           # silently drop non-LiquiDE connections
  port_knocking_enabled = false
  port_knocking_sequence = [7331, 8442, 9553]
  port_knocking_timeout_sec = 10
  timing_randomization = true
  fingerprint_reduction = true
  ```

### Honeypot & Tarpit (Automatic)

LiquiDE can automatically detect **unambiguously malicious traffic** and respond with honeypot/tarpit tactics that waste attacker resources while gathering intelligence. Only patterns that have zero chance of being legitimate traffic trigger these mechanisms.

#### What Triggers Tarpit/Honeypot (Zero False-Positive Criteria)

These triggers are chosen because no legitimate client would ever produce them:

| Trigger | Why It's Safe | Response |
|---------|---------------|----------|
| **Invalid protocol magic bytes** | Legitimate clients always send the correct LiquiDE protocol header. Scanners (nmap, masscan, etc.) send HTTP, SSH, or random probes. | Tarpit: accept connection, respond very slowly with garbage data |
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
honeypot_log = "/var/log/liquide/honeypot.log"
honeypot_capture_payloads = true         # capture full packet payloads
honeypot_max_capture_mb = 100           # max payload storage per day
honeypot_retention_days = 90            # how long to keep honeypot logs
honeypot_fake_version = ""              # empty = auto-generate plausible version

# Trigger thresholds (all require ZERO legitimate overlap)
trigger_on_invalid_protocol = true       # non-LiquiDE protocol magic
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

LiquiDE provides extensive session lock and timeout controls for security and resource management. Lock policies can be configured globally, per-group, or per-user.

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
  - Tile delta ratio (% of tiles sent as XOR delta vs. full).
  - Tile skip ratio (% of tiles unchanged and skipped).
  - Cache hit rates (blur, wallpaper, partial regions).
  - Effect budget utilization.
- Accessible via:
  - `liquidctl stats` CLI command.
  - Client-side overlay (see [spec-client.md](spec-client.md)).
  - Prometheus metrics endpoint (for monitoring infrastructure).
  - Management UI (see [spec-manager.md](spec-manager.md)).

---

## 16a) Session Recording & Replay

Session recording captures the entire visual, audio, and metadata content of a remote desktop session into a structured file for later playback, compliance auditing, incident investigation, helpdesk support, and training.

### Recording Architecture

```
liquid-session
├── Compositor → Frame buffer (post-composite, pre-encode)
│                    │
│                    ▼
│              ┌──────────────────┐
│              │  Recording Tap   │ ◄── zero-copy: reads from same DMA buffer
│              │  (optional,      │     as the encoder. No re-composite.
│              │   per-session)   │
│              └────────┬─────────┘
│                       │
│                       ▼
│              ┌──────────────────┐
│              │  Recording       │ AV1 low-bitrate encode (dedicated encoder
│              │  Encoder         │ instance, not shared with live stream).
│              └────────┬─────────┘     Alternatively: same codec as live
│                       │               stream with separate quality settings.
│                       ▼
│              ┌──────────────────┐
│              │  Recording       │
│              │  Muxer (.lqr)   │ ◄── writes to disk or streams to
│              └────────┬─────────┘     recording storage backend
│                       │
│                       ▼
│              ┌──────────────────┐
│              │  Storage Backend │ local disk / NFS / S3 / SFTP
│              └──────────────────┘
│
├── Input Worker ──► Recording Tap (input events interleaved)
├── Audio Worker ──► Recording Tap (audio frames interleaved)
├── Clipboard ──► Recording Tap (clipboard events, optionally redacted)
└── Session Events ──► Recording Tap (window open/close, app launch, login/logout)
```

The recording tap is a **passive observer** — it reads from the compositor's output buffer after compositing but independently of the live encoding pipeline. This means:
- **No performance impact on the live session** beyond the recording encoder's CPU cost (isolated to its own thread / cgroup budget).
- **Recording can use a different codec or quality** than the live stream (e.g., AV1 for better compression at archival bitrate).
- **Recording can be started/stopped at any time** without disrupting the session.

### Recording Format (`.lqr`)

LiquiDE recordings use a custom container format (`.lqr` — LiquiDE Recording) that stores multiplexed video, audio, metadata, and event streams with full seeking support.

#### Container Structure

```
┌─────────────────────────────────────────────┐
│ LQR File Header                              │
│   magic: "LQR\x01"                          │
│   version: u16                               │
│   created_at: u64 (Unix timestamp µs)        │
│   duration_us: u64                           │
│   session_id: String                         │
│   user: String (or "[redacted]")             │
│   server: String                             │
│   resolution: (u32, u32)                     │
│   recording_policy: String                   │
│   encryption: EncryptionInfo                 │
├─────────────────────────────────────────────┤
│ Stream Directory                             │
│   stream[0]: video  (codec, dimensions, fps) │
│   stream[1]: audio  (codec, sample_rate, ch) │
│   stream[2]: input  (event log)              │
│   stream[3]: clipboard (event log)           │
│   stream[4]: session_events (event log)      │
│   stream[5]: annotations (text markers)      │
├─────────────────────────────────────────────┤
│ Seek Index                                   │
│   Keyframe positions at 10-second intervals  │
│   Event stream byte offsets per keyframe     │
├─────────────────────────────────────────────┤
│ Interleaved Packets                          │
│   [stream_id: u8] [timestamp_us: u64]        │
│   [payload_len: u32] [payload: bytes]        │
│   ...                                        │
├─────────────────────────────────────────────┤
│ Trailer                                      │
│   Final seek index copy (for append-mode)    │
│   SHA-256 integrity hash of all packets      │
│   Digital signature (optional, Ed25519)      │
└─────────────────────────────────────────────┘
```

#### Stream Details

| Stream | Codec / Format | Default Settings | Notes |
|--------|---------------|-----------------|-------|
| Video | AV1 (SVT-AV1, CRF 35) | 1080p, 10 fps capture (adaptive: higher during activity) | Keyframe every 10s. Resolution follows session. |
| Audio | Opus, 48 kHz mono | 32 kbps | Both playback and capture (if enabled) mixed into single stream. |
| Input | CBOR event log | All keyboard/mouse/touch events | Timestamps relative to recording start. |
| Clipboard | CBOR event log | Clipboard offers + content (text only; images optionally redacted) | Redaction policy configurable. |
| Session Events | CBOR event log | Window create/destroy, app launch/exit, resize, focus changes | Non-PII metadata. |
| Annotations | CBOR list of `{timestamp, author, text}` | Admin or automated annotations (e.g., "DLP violation detected") | Added during recording or post-hoc. |

#### Adaptive Frame Rate

The recording encoder uses adaptive frame capture to minimize storage:

| Session Activity | Capture FPS | Notes |
|-----------------|------------|-------|
| Active input (typing, mouse movement) | 10–15 fps | Sufficient for compliance review |
| Window transitions / animations | Up to 30 fps | Captures transient UI states |
| Idle (no input, no damage) | 0.5 fps (1 frame every 2s) | Proves screen state, minimal storage |
| Full-screen video playback | 5 fps | Reduces redundancy (user is watching, not acting) |

Typical storage: **50–200 MB per hour** at 1080p with adaptive capture.

### Recording Modes

| Mode | Trigger | User Notification | Privacy | Use Case |
|------|---------|------------------|---------|----------|
| **Policy-enforced** | Always-on per policy | Mandatory persistent indicator in status bar + session start banner | Highest compliance burden | Financial services, healthcare, government |
| **Admin-initiated** | Admin starts recording via `liquidctl` or Manager UI | Mandatory indicator appears when recording starts | Admin audit trail | Incident investigation, suspected misuse |
| **User-initiated** | User starts recording from session menu | Self-recording, no indicator needed | User owns recording | Self-training, demo creation, bug reports |
| **Support-initiated** | Helpdesk triggers recording during remote assistance | Mandatory indicator + consent dialog | Combined admin+user consent | Support session documentation |

### Privacy & Consent

#### Notification Requirements

| Recording Mode | Status Bar Indicator | Session Start Banner | Consent Dialog | Audio Announcement |
|---------------|---------------------|--------------------|-----------------|--------------------|
| Policy-enforced | Red dot + "Recording" text, always visible, cannot be hidden | "This session is being recorded per organizational policy" | None (consent implicit in employment/usage agreement) | Optional (configurable) |
| Admin-initiated | Red dot + "Recording" text, appears when started | None | Optional (configurable: `require_consent = true`) | None |
| User-initiated | Green dot + "Recording" (self-indicator only) | None | None | None |
| Support-initiated | Red dot + "Recording" + "Support recording active" | None | Required consent dialog with accept/decline | None |

#### Recording Indicator Protocol

The server sends a control message to the client when recording state changes:

```
RecordingStateChanged {
    recording: bool,
    mode: "policy" | "admin" | "user" | "support",
    initiated_by: String (user ID, or "policy"),
    started_at: u64 (timestamp),
    indicator_dismissable: bool (false for policy/admin/support),
}
```

The client MUST render the recording indicator. The indicator cannot be removed by the user for non-user-initiated recordings. The client spec (spec-client.md) defines the visual rendering of the indicator.

#### Data Redaction

Administrators can configure what data is included or redacted in recordings:

| Data Type | Redaction Option | Default | Notes |
|-----------|-----------------|---------|-------|
| Screen video | Cannot redact (purpose of recording) | Included | — |
| Audio playback | `redact_audio_playback` | Included | Muted in recording if redacted |
| Audio capture (mic) | `redact_audio_capture` | **Redacted** | Microphone audio excluded by default |
| Keyboard input log | `redact_keyboard` | Included (events only, not keystrokes displayed on screen) | For compliance: which keys were pressed |
| Mouse input log | `redact_mouse` | Included | Click positions and movement |
| Clipboard content | `redact_clipboard_content` | **Redacted** (offers logged, content omitted) | Content may contain passwords/PII |
| Clipboard offers | `redact_clipboard_offers` | Included | MIME types only, no content |
| Window titles | `redact_window_titles` | Included | May reveal document names |
| Application names | `redact_app_names` | Included | Which apps were used |

### Recording Storage

#### Storage Backends

| Backend | Configuration | Use Case | Notes |
|---------|--------------|----------|-------|
| Local filesystem | `path = "/var/lib/liquide/recordings/"` | Single-server | Simplest. Use with log rotation. |
| NFS / shared filesystem | `path = "/mnt/recordings/"` | Multi-server with shared storage | Standard NFS mount. |
| S3-compatible object store | `s3_bucket`, `s3_prefix`, `s3_region`, `s3_endpoint` | Cloud / large-scale | Streaming upload. Multipart. |
| SFTP | `sftp_host`, `sftp_path`, `sftp_user`, `sftp_key` | Secure transfer to archive | Uploads after recording ends. |

#### Retention Policy

```toml
[recording.retention]
max_age_days = 90                    # auto-delete recordings older than this
max_storage_gb = 500                 # total storage cap (oldest deleted first)
archive_after_days = 30              # move to cold storage after 30 days
archive_backend = "s3"               # cold storage backend
legal_hold_tag = "hold"              # recordings tagged "hold" are never auto-deleted
```

#### Encryption at Rest

Recordings are encrypted at rest using AES-256-GCM:

| Component | Key Source |
|-----------|-----------|
| Per-recording data encryption key (DEK) | Random 256-bit key generated at recording start |
| Key encryption key (KEK) | Derived from server master key or HSM |
| Key wrapping | DEK encrypted with KEK, stored in `.lqr` header |
| Integrity | SHA-256 over all packets + Ed25519 signature (optional) |

The server master key is configured in `[recording.encryption]`. HSM/KMS integration is supported for enterprise deployments.

### Playback

#### Playback Methods

| Method | Implementation | Features |
|--------|---------------|----------|
| `liquidctl recording play <file>` | CLI player, opens in LiquiDE client | Full playback with seek, speed control, event overlay |
| Manager UI built-in player | Web-based player in `liquid-manager` | Streaming playback, no download needed, annotations |
| Export to MP4 | `liquidctl recording export <file> --format mp4` | Standard video for sharing outside LiquiDE ecosystem |
| Third-party player (exported) | Standard MP4/MKV | Video only, no event overlay |

#### Playback Features

- **Seek**: Jump to any timestamp (keyframe-based, <500ms seek time).
- **Speed control**: 0.25x – 8x playback speed.
- **Event overlay**: Toggle display of keyboard inputs, mouse clicks (visual ripple), clipboard events, window events as translucent overlay.
- **Timeline markers**: Annotations shown as markers on the timeline scrubber.
- **Search**: Text search through keyboard input log and window title events.
- **Screenshot extraction**: Export individual frames as PNG.
- **Audit trail**: Playback access is logged (who viewed which recording, when).

### Recording Configuration

```toml
[recording]
enabled = false                      # master switch
mode = "policy"                      # policy, admin-only, user-only, disabled
storage_backend = "local"            # local, s3, sftp
storage_path = "/var/lib/liquide/recordings/"

[recording.video]
codec = "av1"                        # av1, h264, vp9
crf = 35                             # quality (lower = better, more storage)
max_fps = 15                         # max capture frame rate
adaptive_fps = true                  # reduce FPS during idle
resolution = "session"               # "session" (match session) or "720p", "1080p"

[recording.audio]
include_playback = true              # record session audio output
include_capture = false              # record microphone input (default: off)
codec = "opus"
bitrate_kbps = 32

[recording.redaction]
redact_audio_capture = true
redact_clipboard_content = true
redact_keyboard = false
redact_mouse = false
redact_window_titles = false

[recording.retention]
max_age_days = 90
max_storage_gb = 500
archive_after_days = 30
archive_backend = "local"

[recording.encryption]
enabled = true
method = "aes-256-gcm"
key_source = "config"                # config, hsm, kms
# master_key = "..."                 # only if key_source = "config"

[recording.notification]
show_indicator = true                # cannot be false for policy/admin modes
show_session_banner = true
audio_announcement = false
consent_required = false             # for admin-initiated recordings
```

### Recording Policy Keys

Recording integrates with the formal policy engine (§15):

| Policy Key | Type | Resolution | Description |
|------------|------|-----------|-------------|
| `recording.enabled` | bool | `deny_overrides` | Whether recording is active for this scope |
| `recording.mode` | enum | `highest_precedence` | Recording mode: `policy`, `admin-only`, `user-only` |
| `recording.include_audio` | bool | `deny_overrides` | Include audio in recording |
| `recording.include_clipboard` | bool | `deny_overrides` | Include clipboard content |
| `recording.max_retention_days` | int | `min` | Maximum retention (lower wins for compliance) |
| `recording.allow_user_download` | bool | `deny_overrides` | Allow users to download their own recordings |
| `recording.require_consent` | bool | `deny_overrides` (deny = require) | Require consent dialog for admin recordings |

### Recording Audit Events

| Event | Level | Fields |
|-------|-------|--------|
| `recording.started` | `info` | `session_id`, `mode`, `initiated_by`, `recording_id` |
| `recording.stopped` | `info` | `session_id`, `recording_id`, `duration_seconds`, `file_size_bytes` |
| `recording.consent_granted` | `info` | `session_id`, `user`, `recording_id` |
| `recording.consent_denied` | `warn` | `session_id`, `user`, `recording_id` (recording not started) |
| `recording.playback` | `info` | `recording_id`, `viewer_user`, `access_method` |
| `recording.exported` | `info` | `recording_id`, `exported_by`, `format` |
| `recording.deleted` | `info` | `recording_id`, `deleted_by`, `reason` (retention/manual) |
| `recording.retention_purge` | `info` | `count`, `total_bytes_freed`, `oldest_age_days` |

---

## 16b) Remote Assistance & Shadow Sessions

Remote assistance allows administrators, helpdesk agents, or invited users to observe and optionally interact with another user's live session, with explicit consent flows and full audit trails.

### Terminology

| Term | Definition |
|------|-----------|
| **Owner** | The user who owns the session being observed |
| **Observer** | The assisting party (admin, helpdesk, invited peer) |
| **Shadow session** | An observer's view-only or interactive connection to the owner's session |
| **Consent** | Explicit owner approval for an observer to join |
| **Escalation** | Upgrading an observer from view-only to interactive control |

### Assistance Modes

| Mode | Observer Capabilities | Owner Visibility | Consent Required | Use Case |
|------|----------------------|-----------------|-----------------|----------|
| **View-only** | See screen + audio. No input. | Sees observer cursor (ghost). Indicator in status bar. | Yes (default) | Support diagnosis, training observation |
| **Interactive** | Full input (keyboard, mouse). Owner input also active (shared control). | Sees two cursors (owner = primary, observer = ghost). Indicator. | Yes (explicit escalation consent) | Hands-on support, pair programming |
| **Exclusive control** | Full input. Owner input temporarily blocked (can reclaim). | Full-screen notification "Remote control active". | Yes (separate consent) | Critical troubleshooting requiring uninterrupted control |
| **Stealth observation** | See screen. No indicator shown to owner. | None (owner unaware). | **Admin policy only** — no user consent. Audit-logged. | Compliance monitoring, insider threat investigation |

Stealth observation is a sensitive capability. It is:
- **Disabled by default** in the server configuration.
- Requires explicit `assistance.stealth_enabled = true` in server config.
- Requires the observer to have the `stealth_observe` permission (typically restricted to security/compliance roles).
- **Every second** of stealth observation is audit-logged with the observer identity.
- Subject to legal requirements in many jurisdictions — admin is warned at enable time.

### Assistance Flow

#### Standard Request Flow (Observer-Initiated)

```
Observer (Admin/Helpdesk)                    Server                     Owner (User)
        │                                       │                          │
        │  ── AssistanceRequest ──────────────►  │                          │
        │     {target_session, mode: "view",     │                          │
        │      reason: "Ticket #1234"}           │                          │
        │                                       │  ── ConsentPrompt ────►  │
        │                                       │     {observer_name,       │
        │                                       │      observer_role,       │
        │                                       │      mode: "view",        │
        │                                       │      reason, timeout: 60s}│
        │                                       │                          │
        │                                       │  ◄── ConsentResponse ──  │
        │                                       │     {accepted: true}      │
        │                                       │                          │
        │  ◄── AssistanceGranted ──────────────  │                          │
        │     {shadow_session_id, token}         │                          │
        │                                       │                          │
        │  ═══ Shadow connection established ═══════════════════════════   │
        │  (Observer receives live frame stream) │  (Indicator appears)    │
```

#### Owner-Initiated Request (Invite)

```
Owner (User)                            Server                     Observer (Invited)
     │                                       │                          │
     │  ── AssistanceInvite ──────────────►  │                          │
     │     {invite_code_length: 6,           │                          │
     │      mode: "interactive",             │                          │
     │      expires_in: 300s}                │                          │
     │                                       │                          │
     │  ◄── InviteCreated ─────────────────  │                          │
     │     {code: "A3X-9K2",                │                          │
     │      url: "https://remote/assist/..."}│                          │
     │                                       │                          │
     │  (Owner shares code via phone/chat)   │                          │
     │                                       │                          │
     │                                       │  ◄── JoinWithCode ─────  │
     │                                       │     {code: "A3X-9K2"}    │
     │                                       │                          │
     │  (No consent dialog — owner initiated)│                          │
     │                                       │                          │
     │  ═══ Shadow connection established ═══════════════════════════   │
```

#### Escalation Flow (View → Interactive)

```
Observer                                Server                     Owner
     │                                       │                          │
     │  ── EscalationRequest ─────────────►  │                          │
     │     {mode: "interactive"}              │                          │
     │                                       │  ── EscalationPrompt ──► │
     │                                       │     {"Observer requests    │
     │                                       │      keyboard+mouse       │
     │                                       │      control. Allow?"}    │
     │                                       │                          │
     │                                       │  ◄── EscalationResponse  │
     │                                       │     {accepted: true}      │
     │                                       │                          │
     │  ◄── EscalationGranted ─────────────  │                          │
     │     (Observer can now send input)      │  (Indicator updates)    │
```

### Observer Capabilities & Limits

| Capability | View-Only | Interactive | Exclusive | Stealth |
|-----------|-----------|------------|-----------|---------|
| See screen | Yes | Yes | Yes | Yes |
| Hear audio | Yes (configurable) | Yes | Yes | Yes (configurable) |
| Move mouse | Ghost cursor only (visual indicator, no server-side effect) | Yes | Yes | No cursor |
| Keyboard input | No | Yes | Yes | No |
| Clipboard access | No | Read-only | Read/write | No |
| File transfer | No | No (separate permission) | No (separate permission) | No |
| Request control escalation | Yes (→ interactive) | Yes (→ exclusive) | N/A | No |
| Observer cursor visible to owner | Yes (ghost) | Yes (ghost, different color) | Yes (replaces owner cursor) | No |
| Status bar indicator | Yes | Yes | Yes (+ full-screen banner) | No |
| Max concurrent observers | 5 (configurable) | 2 | 1 | 3 |
| Recording integration | Observer join/leave logged | Input attributed to observer | Input attributed | Stealth audit log |

### Observer Cursor

When multiple users interact with the same session, each user's cursor is distinguished:

| Cursor | Appearance | Label |
|--------|-----------|-------|
| Owner | Normal system cursor | None |
| Observer (view-only) | Translucent ghost cursor (50% opacity), accent-colored ring | Observer display name |
| Observer (interactive) | Solid cursor with colored ring (different color per observer) | Observer display name |
| Observer (exclusive) | Normal cursor (owner cursor hidden) | "Remote Control" label |

Cursor rendering follows the design language in spec-design.md. Ghost cursors use `mix-blend-mode: screen` and are rendered as a CSS-styled overlay.

### Chat Channel

During a remote assistance session, a **text chat channel** is available between the owner and observers:

- Messages are sent over a dedicated reliable data channel (part of the assistance session, not the main session channels).
- Chat is displayed as a small floating panel (glass-themed, anchored to bottom-right, resizable).
- Chat history is persisted for the duration of the assistance session.
- Chat messages are logged in the audit trail.
- Optionally included in session recordings.

### Assistance Protocol Messages

| Message | Direction | Fields |
|---------|-----------|--------|
| `AssistanceRequest` | Observer → Server | `target_session_id`, `mode`, `reason`, `observer_credentials` |
| `ConsentPrompt` | Server → Owner | `observer_name`, `observer_role`, `mode`, `reason`, `timeout_seconds` |
| `ConsentResponse` | Owner → Server | `accepted`, `restrictions` (e.g., "view-only, no audio") |
| `AssistanceGranted` | Server → Observer | `shadow_session_id`, `token`, `capabilities` |
| `AssistanceDenied` | Server → Observer | `reason` ("declined", "timeout", "policy") |
| `AssistanceInvite` | Owner → Server | `mode`, `expires_seconds`, `max_uses` |
| `InviteCreated` | Server → Owner | `code`, `url`, `expires_at` |
| `JoinWithCode` | Observer → Server | `code`, `observer_identity` |
| `EscalationRequest` | Observer → Server | `target_mode` |
| `EscalationPrompt` | Server → Owner | `observer_name`, `target_mode` |
| `EscalationResponse` | Owner → Server | `accepted` |
| `EscalationGranted` | Server → Observer | `new_capabilities` |
| `AssistanceEnd` | Any → Server | `reason` ("observer_left", "owner_revoked", "timeout", "admin_terminated") |
| `ChatMessage` | Any ↔ Any | `sender`, `text`, `timestamp` |
| `AnnotationAdd` | Observer → Server | `text`, `timestamp` (annotation on recording timeline) |
| `OwnerReclaimControl` | Owner → Server | (immediately revokes exclusive control) |

### Assistance Consent UI

The consent dialog rendered on the owner's session:

```
┌──────────────────────────────────────────────────────┐
│                    Remote Assistance                  │
│                                                      │
│    ┌────┐                                            │
│    │ 👤 │  Alex (Helpdesk Agent)                    │
│    └────┘  requests to VIEW your session.            │
│                                                      │
│    Reason: "Investigating ticket #1234 — reported    │
│    printing issue"                                   │
│                                                      │
│    They will be able to:                             │
│    ✓ See your screen                                 │
│    ✓ See your audio output                           │
│    ✗ Control your keyboard or mouse                  │
│    ✗ Access your clipboard                           │
│                                                      │
│    ┌──────────┐  ┌──────────┐  ┌─────────────────┐  │
│    │  Allow   │  │  Deny    │  │  Allow (this     │  │
│    │          │  │          │  │  time, 30 min)  │  │
│    └──────────┘  └──────────┘  └─────────────────┘  │
│                                                      │
│    This request expires in 45 seconds                │
└──────────────────────────────────────────────────────┘
```

If the owner does not respond within the timeout (default: 60s), the request is automatically denied.

### Assistance Configuration

```toml
[assistance]
enabled = true
max_concurrent_observers = 5         # per session
invitation_expiry_seconds = 300      # invite code expiry
consent_timeout_seconds = 60         # auto-deny after timeout

[assistance.modes]
view_only = true                     # allow view-only shadow
interactive = true                   # allow interactive (shared control)
exclusive = true                     # allow exclusive control
stealth = false                      # stealth observation (DISABLED by default)

[assistance.stealth]
# Only effective if stealth = true above
enabled = false
required_role = "security_admin"     # role required to initiate stealth
audit_interval_seconds = 1           # audit log entry frequency
max_duration_minutes = 60            # hard time limit per stealth session
legal_notice = "Stealth observation may be subject to legal requirements in your jurisdiction."

[assistance.permissions]
helpdesk_can_request = true          # helpdesk role can initiate requests
admin_can_force = false              # admin can bypass consent (NOT recommended)
user_can_invite = true               # users can create invite codes

[assistance.recording]
auto_record = true                   # auto-start recording when assistance begins
include_chat = true                  # include chat messages in recording
```

### Assistance Policy Keys

| Policy Key | Type | Resolution | Description |
|------------|------|-----------|-------------|
| `assistance.enabled` | bool | `deny_overrides` | Allow remote assistance |
| `assistance.allow_interactive` | bool | `deny_overrides` | Allow interactive mode |
| `assistance.allow_exclusive` | bool | `deny_overrides` | Allow exclusive control |
| `assistance.stealth_enabled` | bool | `deny_overrides` | Allow stealth observation |
| `assistance.auto_record` | bool | `deny_overrides` (deny = always record) | Auto-record assistance sessions |
| `assistance.user_can_invite` | bool | `deny_overrides` | Allow users to generate invite codes |
| `assistance.max_observers` | int | `min` | Max simultaneous observers |

### Assistance Audit Events

| Event | Level | Fields |
|-------|-------|--------|
| `assistance.requested` | `info` | `observer`, `target_session`, `mode`, `reason` |
| `assistance.consent_granted` | `info` | `owner`, `observer`, `mode`, `restrictions` |
| `assistance.consent_denied` | `info` | `owner`, `observer`, `mode` |
| `assistance.consent_timeout` | `info` | `owner`, `observer` |
| `assistance.started` | `info` | `shadow_session_id`, `observer`, `target_session`, `mode` |
| `assistance.escalated` | `warn` | `shadow_session_id`, `observer`, `from_mode`, `to_mode` |
| `assistance.owner_reclaimed` | `info` | `shadow_session_id`, `observer` |
| `assistance.ended` | `info` | `shadow_session_id`, `reason`, `duration_seconds` |
| `assistance.stealth_started` | `warn` | `observer`, `target_session`, `justification` |
| `assistance.stealth_active` | `info` | `observer`, `target_session` (every `audit_interval_seconds`) |
| `assistance.stealth_ended` | `warn` | `observer`, `target_session`, `duration_seconds` |
| `assistance.chat_message` | `debug` | `shadow_session_id`, `sender`, `text_length` |
| `assistance.invite_created` | `info` | `owner`, `code_hash`, `mode`, `expires_at` |
| `assistance.invite_used` | `info` | `code_hash`, `observer`, `owner` |

---

## 17) Observability & Operations

### Metrics (Prometheus)
- FPS (server render, encode, client present).
- Latency (input-to-photon estimate).
- Bandwidth in/out (total, per channel).
- Packet loss / RTT.
- Dirty region ratio.
- Tile delta ratio (XOR delta vs. full tile).
- Tile skip ratio (unchanged tiles).
- Tile scroll events per second.
- Encode time per frame.
- Active sessions count.
- CPU and memory usage per session.
- Cache hit rates.

### Logs
- Structured logs (JSON option).
- Per-session log correlation ID.
- Configurable log levels per subsystem.

### Extensive Logging System
LiquiDE has a comprehensive, per-component logging system designed for production debugging, auditing, and monitoring:

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
base_dir = "/var/log/liquide"
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

**Enterprise Observability & Operations Cross-References**

For pre-built operational tooling mapped to the SLOs in [spec-performance.md](spec-performance.md):

- **Golden dashboards** (8 Grafana panels: session health, input-to-photon, frame rate, transport health, audio health, degradation, CPU/memory, encode efficiency) — see [spec-observability.md §5a](spec-observability.md).
- **SLO-mapped alert rules** (7 Prometheus alerting rules with runbook links) — see [spec-observability.md §5a](spec-observability.md).
- **Certificate lifecycle automation** (ACME integration, renewal, zero-downtime rotation) — see [spec-manager.md §6b](spec-manager.md).
- **Policy versioning, staged rollout & canary** (version tracking, 3-stage rollout, SLO-based auto-rollback) — see [spec-manager.md §5](spec-manager.md).
- **Backup & disaster recovery** (RPO/RTO targets, automated backup, restore procedures) — see [spec-manager.md §6a](spec-manager.md).

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
  nla = true                           # Network Level Authentication (CredSSP/TLS)
  tls_cert = ""                        # uses server TLS cert if empty
  max_sessions = 0                     # 0 = same as native limit
  security_layer = "tls"               # tls (recommended), nla, rdp (legacy, insecure)
  ```
- When enabled, provides an RDP endpoint for standard RDP clients (mstsc, FreeRDP, etc.).
- Useful for environments where installing LiquidClient is not possible or as a migration path from existing RDP deployments.

### Supported RDP Features

| RDP Feature | Status | Notes |
|-------------|--------|-------|
| Display (bitmap updates) | Supported | RFX and NSCodec for better quality where client supports it |
| Keyboard input | Supported | Scancode and Unicode modes |
| Mouse input | Supported | Absolute and relative positioning |
| Clipboard (text) | Supported | UTF-8 text, bidirectional |
| Clipboard (images) | Supported | PNG/BMP, size-limited by policy |
| Clipboard (files) | Supported | File list transfer via CLIPRDR channel |
| Audio playback | Supported | RDPSND channel, PCM and AAC |
| Audio capture | Supported | AUDIN channel |
| Drive redirection | Supported | RDPDR channel, read/write to client drives |
| Printer redirection | Supported | RDPDR printer sub-channel (see §10a Printing) |
| Smart card redirection | Supported | RDPDR smart card sub-channel |
| USB redirection | Not supported | Use LiquidClient for USB/IP |
| RemoteFX (RFX codec) | Supported | Better quality than raw bitmap |
| H.264/AVC (RDP 10) | Not supported | Would require RDP-specific AVC framing |
| Multi-monitor | Supported | Up to 4 monitors, per RDP spec limits |
| Dynamic resolution | Supported | DISP channel (Display Update Virtual Channel) |
| NLA (Network Level Auth) | Supported | CredSSP with NTLM or Kerberos |
| TLS transport | Supported | TLS 1.2+ (server-configured) |
| RemoteApp / seamless | Not supported | Out of scope — LiquiDE uses full desktop sessions |
| Gateway (RD Gateway) | Not supported | Use `liquid-gateway` instead |
| Multitouch | Not supported | No RDP multitouch channel implementation |
| Graphics pipeline (EGFX) | Partial | RFX progressive codec only, no H.264 sub-mode |

### Feature Gap: Native vs RDP

Features that are **available in LiquidClient but not via RDP**:

| Feature | Reason | Workaround |
|---------|--------|------------|
| Hybrid tile+video encoding | RDP protocol does not support LiquiDE's tile channel | RFX codec provides acceptable quality |
| Transport switching | RDP uses TCP only | N/A |
| QUIC / UDP transport | RDP standard is TCP-only | N/A |
| Client-side font rendering | Requires LiquiDE protocol extensions | Server-rendered fonts (standard behavior) |
| WASM plugin inter-op | Requires LiquiDE protocol | N/A |
| Cursor prediction | Requires LiquiDE cursor channel semantics | Standard RDP cursor (slightly higher latency) |
| Tile XOR delta | LiquiDE-specific optimization | RFX progressive provides similar benefit |
| Session roaming tokens | LiquiDE session resume protocol | RDP reconnection (less seamless) |
| WebTransport | Not applicable to RDP | N/A |
| Screen Wake Lock | Client-side, LiquidClient feature | N/A |

### Enterprise RDP Expectations: Safe Defaults

For organizations evaluating LiquiDE as a replacement for existing RDP solutions, the RDP compatibility layer provides a migration bridge. These defaults are chosen for maximum compatibility with common RDP clients:

| Setting | Default | Rationale |
|---------|---------|-----------|
| Security layer | TLS + NLA | Matches modern Windows RDP Server defaults |
| Clipboard | Text only, 4 MB limit | Prevents accidental large data exfiltration |
| Drive redirection | Read-only | Prevents writes to client machine until admin enables |
| Audio | Playback only | Microphone capture requires explicit enable |
| Max color depth | 32-bit | Full color |
| Max resolution | 8192×8192 | Covers all practical monitor sizes |
| Idle timeout | 30 minutes | Matches common enterprise policy |
| Encryption | TLS 1.2+, AES-256 | No legacy RC4 or DES |

### Hard "Won't Do" List

The following RDP features will **not** be implemented. They are either legacy, security risks, or replaced by better LiquiDE-native alternatives:

| RDP Feature | Reason for Exclusion |
|-------------|---------------------|
| RDP Security Layer (legacy RC4 encryption) | Insecure. TLS or NLA required. |
| CredSSP with NTLMv1 | Insecure. NTLMv2 minimum, Kerberos preferred. |
| RemoteApp / seamless windows | Out of scope. LiquiDE provides full desktop sessions. Use LiquidClient if seamless windows are needed. |
| RD Gateway (HTTPS tunneling) | Replaced by `liquid-gateway` with better performance and security. |
| RD Web Access | Replaced by LiquiDE web client (see [spec-web-client.md](spec-web-client.md)). |
| RDP Licensing Server integration | LiquiDE uses its own licensing model (MIT). No CAL required. |
| RDP Virtual Channels (custom) | Only standard channels (CLIPRDR, RDPSND, AUDIN, RDPDR, DISP) implemented. Custom virtual channel plugins are not supported via RDP. |
| Bitmap caching (RDP persistent cache) | LiquiDE uses its own tile caching. RDP bitmap cache is not implemented. Clients fall back to non-cached mode. |
| Network Characteristics Detection (AUTODETECT) | LiquiDE performs its own bandwidth estimation. RDP autodetect channel is acknowledged but not actively used. |

---

## 19) Server Configuration

### Configuration Files
- `/etc/liquide/server.toml` — server-wide configuration.
- `/etc/liquide/policies.toml` — policy definitions.
- `~/.config/liquide/session.toml` — per-user session preferences.
- `~/.config/liquide/theme.css` — per-user CSS theme.

### Server Configuration Structure (`server.toml`)

```toml
# ─── General ────────────────────────────────────────────────
[general]
hostname = "liquid-server-01"
log_level = "info"                    # trace, debug, info, warn, error
log_format = "json"                   # json, text
data_dir = "/var/lib/liquide"

# ─── Appearance ──────────────────────────────────────────────
[appearance]
default_theme = "liquid-glass"        # liquid-glass, night, sunset, midday, custom
theme_dir = "/etc/liquide/themes"    # system theme directory
allow_user_themes = true              # allow users to override theme
wallpaper_dir = "/etc/liquide/wallpapers"

# ─── Internationalization ────────────────────────────────────
[i18n]
default_locale = "en-US"
available_locales = ["en-US", "en-GB", "de-DE", "fr-FR", "ja-JP", "zh-CN", "ko-KR", "es-ES", "pt-BR", "ru-RU", "ar-SA", "hi-IN"]
fallback_locale = "en-US"
message_dir = "/etc/liquide/i18n"
allow_user_translations = true
keyboard_layout_dir = "/etc/liquide/xkb"

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
cert = "/etc/liquide/cert.pem"
key = "/etc/liquide/key.pem"
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
clipboard_history_size = 25              # max items in clipboard history ring buffer (0 = disabled)
convert_bmp_to_png = true                # convert BMP clipboard images to PNG on wire

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
excluded_app_ids = ["liquide-desktop"]  # apps that cannot be detached

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
fail2ban_log = "/var/log/liquide/auth.log"
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
honeypot_log = "/var/log/liquide/honeypot.log"

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
plugin_dirs = ["/etc/liquide/plugins", "~/.config/liquide/plugins"]
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
crash_report_dir = "/var/log/liquide/crashes"
crash_report_retention_days = 30         # auto-delete old crash reports
crash_report_max_count = 1000            # max stored crash reports
telemetry_upload_enabled = false         # upload crash reports to endpoint
telemetry_upload_url = ""                # crash telemetry endpoint URL
include_coredump_in_report = false       # include coredump path in client-visible report

# ─── Logging ───────────────────────────────────────────────
[logging]
base_dir = "/var/log/liquide"
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
LiquiDE uses a CSS-like styling language for all visual elements. Users and administrators can customize the appearance of the entire desktop environment through CSS files.

### CSS Scope
- **System theme**: `/etc/liquide/theme.css` — base theme, ships with Liquid Glass defaults.
- **User theme**: `~/.config/liquide/theme.css` — user overrides, merged on top of system theme.

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

5. **Software Center**
   - Browse, install, update, and remove Flatpak applications from Flathub and other configured remotes.
   - Permission management for sandboxed apps.
   - Integrated with LiquiDE update system.
   - See [spec-addons.md §15](spec-addons.md) for full specification.

> **Third-party applications**: LiquiDE uses **Flatpak** as the primary mechanism for users to install third-party applications. Flathub is configured out of the box. Flatpak apps run inside standard sandboxes and interact with LiquiDE through `xdg-desktop-portal-liquide`. Full Flatpak support — including policy controls, auto-updates, runtime management, and CLI integration — is specified in [spec-interop.md §6.3](spec-interop.md), [spec-system.md §14](spec-system.md), [spec-updates.md §9](spec-updates.md), and [spec-liquidctl.md §3.15](spec-liquidctl.md).

---

## 22) Implementation Plan

### Language
- **Everything in Rust**: server, compositor, shell, renderer, encoder bindings, transport, CLI tools.
- C FFI bindings for: FreeType, HarfBuzz, Fontconfig, codec libraries (see Codec Legal & Packaging Policy below).
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

| Feature | Crate(s) | Status |
|---------|----------|--------|
| Headless server session (single virtual monitor) | `liquide-session`, `liquide-compositor` | **Implemented** |
| Native client (Linux, macOS, Windows) | `liquide-client`, `liquide-client-renderer` | **Implemented** |
| Clipboard (text, bidirectional) | `liquide-clipboard` | **Implemented** |
| Dynamic resize | `liquide-compositor` | **Implemented** |
| Video mode (H.264) + cursor channel | `liquide-encoder`, `liquide-encoder-hw` | **Implemented** |
| QUIC transport | `liquide-transport` | **Implemented** |
| Basic shell: dock, launcher, terminal | `liquide-shell` | **Implemented** |
| CSS theming (basic) | `liquide-css` | **Implemented** |
| CPU-only rendering | `liquide-renderer-cpu` | **Implemented** |

### v1 Scope

| Feature | Crate(s) | Status |
|---------|----------|--------|
| Multi-monitor with on-demand virtual screens | `liquide-compositor` | **Implemented** |
| Hybrid tile/video encoding | `liquide-encoder` | **Implemented** |
| All 10+ encoders | `liquide-encoder`, `liquide-encoder-hw` | **Implemented** |
| Bidirectional audio | `liquide-audio` | **Implemented** |
| Full policy engine | `liquide-policy` | **Implemented** |
| Stream analysis | — | Planned |
| Server configuration tool | `liquide-ctl` | **Implemented** |
| Full CSS theming | `liquide-css` | **Implemented** |
| Multiple transport strategies | `liquide-transport` | **Implemented** |
| GPU acceleration (optional) | `liquide-encoder-hw` | **Implemented** |
| Gateway support | `liquide-gateway` | **Implemented** |

### vNext

| Feature | Crate(s) | Status |
|---------|----------|--------|
| Web client (WebRTC) — see [spec-web-client.md](spec-web-client.md) | — | Planned |
| Mobile clients (iOS, Android) — see [spec-mobile.md](spec-mobile.md) | `liquide-mobile-core` | Stub |
| Camera passthrough | — | Planned |
| USB redirection | `liquide-usb` | **Implemented** |
| RDP compatibility layer | — | Planned |
| OIDC authentication | `liquide-auth` | **Implemented** |
| Management UI | `liquide-manager` | Stub |
| Client rendering offload (full) | `liquide-client-renderer` | **Implemented** |
| WASM plugin system (runtime, ABI v1, 9 extension points) | `liquide-plugin-host`, `liquide-plugin-abi` | **Implemented** |
| Plugin SDK and documentation | — | Planned |
| Session supervisor process model | `liquide-supervisor` | **Implemented** |
| BSOD crash screen (client-rendered) | `liquide-client` | **Implemented** |
| Session recording & replay (compliance) | `liquide-recording` | **Implemented** |
| Remote assistance / shadow sessions | `liquide-assistance` | **Implemented** |
| Seamless app streaming (full per-OS taskbar, tray, notifications, DnD) | `liquide-interop` | **Implemented** |
| GPU Server Mode (first-class profile with VRAM budgeting, encoder integration) | `liquide-encoder-hw` | **Implemented** |

### Codec Legal & Packaging Policy

Video codecs carry licensing and patent obligations that affect how LiquiDE is packaged and distributed.

#### Codec Classification

| Codec | License of Reference Impl | Patent Status | LiquiDE Default | Distribution Strategy |
|-------|--------------------------|---------------|-----------------|----------------------|
| **H.264 / AVC** | OpenH264 (BSD-2-Clause, Cisco covers MPEG-LA royalties) | Patented (MPEG-LA pool). Cisco's OpenH264 binary distribution includes royalty coverage. | **Default encoder** | Ship OpenH264 binary module (auto-downloaded like Firefox). NOT x264 (GPL). |
| **H.265 / HEVC** | kvazaar (LGPL-2.1, dynamically linked) | Patented (MPEG-LA + HEVC Advance + Velos Media — fragmented pools) | Optional | **Do not ship by default.** Provide as a "bring-your-own" codec module. User installs system FFmpeg or vendor-provided encoder. |
| **AV1** | SVT-AV1 (BSD-2-Clause + Patent Grant), dav1d (BSD-2-Clause) | Royalty-free (AOMedia patent grant) | Supported | Ship SVT-AV1 (encode) + dav1d (decode). Safe to distribute. |
| **VP8** | libvpx (BSD-3-Clause) | Royalty-free (Google patent pledge) | Supported | Ship libvpx. Safe to distribute. |
| **VP9** | libvpx (BSD-3-Clause) | Royalty-free (Google patent pledge) | Supported | Ship libvpx. Safe to distribute. |
| **MJPEG** | libjpeg-turbo (BSD-3-Clause + IJG) | No active patent barriers | Supported | Ship libjpeg-turbo. Safe to distribute. |

#### Distribution Rules

| Rule | Description |
|------|-------------|
| **Never ship GPL codec libraries** | x264 (GPL-2.0) and x265 (GPL-2.0) MUST NOT be linked into LiquiDE binaries. LiquiDE is MIT-licensed and cannot statically or dynamically link GPL libraries. |
| **LGPL is acceptable via dynamic linking only** | Libraries under LGPL (e.g., kvazaar, FFmpeg's LGPL build) MAY be used via dynamic linking as system dependencies. They MUST NOT be statically linked. |
| **OpenH264 model** | H.264 encoding uses Cisco's OpenH264, which includes MPEG-LA patent royalty coverage when distributed as Cisco's binary. LiquiDE downloads OpenH264 at install time or first use (similar to Firefox's model). The download URL and SHA-256 hash are pinned in configuration. |
| **Bring-your-own codec module** | For patented codecs where LiquiDE cannot distribute a licensed binary (HEVC), users install the encoder separately. LiquiDE discovers codecs at runtime via a plugin interface (`/usr/lib/liquide/codecs/` or `codec_search_paths` config). |
| **Hardware encoders are unaffected** | VAAPI, NVENC, AMF, and V4L2 M2M interfaces call into the GPU driver's encoder, which is licensed by the hardware vendor. LiquiDE only uses the OS-level API. No additional patent licensing applies to LiquiDE. |
| **Flatpak client** | The Flatpak client bundles only royalty-free codecs (AV1, VP8/9, MJPEG) and OpenH264. Additional codecs are available via the `org.freedesktop.Platform.ffmpeg-full` extension. |

#### Codec Module Interface

External codec modules are loaded at runtime:

```toml
[codecs]
# Paths searched for codec modules (shared libraries)
search_paths = ["/usr/lib/liquide/codecs", "/usr/local/lib/liquide/codecs"]

# OpenH264 auto-download configuration
[codecs.openh264]
enabled = true
download_url = "https://github.com/cisco/openh264/releases/download/v2.4.1/libopenh264-2.4.1-linux64.7.so.bz2"
sha256 = ""                          # pinned hash — verified before loading
auto_download = true                 # download on first use if not present
cache_path = "/var/lib/liquide/codecs/"

# Bring-your-own HEVC encoder
[codecs.hevc]
enabled = false                      # disabled by default — requires user action
library = ""                         # path to shared library (e.g., "/usr/lib/x86_64-linux-gnu/libkvazaar.so")
```

#### Patent Compliance Notices

- LiquiDE does not practice H.264, H.265, or any other patented codec technology directly. Encoding is performed by external libraries (OpenH264, hardware encoders, or user-supplied modules).
- Users deploying LiquiDE in jurisdictions that enforce software patents are responsible for ensuring they have appropriate codec licenses for their use case.
- The SVT-AV1 and VP8/VP9 codecs are covered by royalty-free patent grants from the Alliance for Open Media and Google, respectively.

---

## 23) Compatibility & Interop

### Wayland Protocol Support

LiquiDE's compositor implements the following Wayland protocols. Each protocol is assigned a **support tier** that determines testing cadence, regression priority, and compatibility commitments.

#### Tier Definitions

| Tier | Testing | Regression Priority | Commitment |
|------|---------|--------------------|-----------|
| **Tier 1** | CI (every PR) + manual QA each release | P0 — release-blocking | Protocol fully implemented per spec, tested with reference clients, regressions are release blockers |
| **Tier 2** | Nightly smoke tests + weekly integration | P2 — fix within the next minor release | Protocol implemented, tested with representative apps, regressions are high-priority but not release-blocking |
| **Tier 3** | Per-release smoke test only | P3 — best-effort, may defer | Protocol implemented but not regularly tested, may break between releases, community bug reports accepted |

#### Protocol Support Matrix

| Protocol | Version | Tier | Direction | Description | Notes |
|----------|---------|------|-----------|-------------|-------|
| `wl_compositor` | 6 | 1 | Core | Surface creation and subcomposition | Mandatory. Subsurface support included. |
| `wl_shm` | 1 | 1 | Core | Shared memory buffer allocation | Mandatory. All SHM formats supported. |
| `wl_seat` | 9 | 1 | Core | Input device management (keyboard, pointer, touch) | Mandatory. Pointer constraints see below. |
| `wl_output` | 4 | 1 | Core | Monitor geometry, scale, modes | Virtual monitor integration. |
| `wl_data_device_manager` | 3 | 1 | Core | Clipboard and drag-and-drop | Clipboard integration with remote clipboard channel. |
| `xdg_wm_base` (xdg_shell) | 6 | 1 | Shell | Toplevel windows, popups, positioning | xdg_toplevel, xdg_popup with position constraints. |
| `zwlr_layer_shell_v1` | 4 | 1 | Shell | Panels, overlays, lock screens, docks | Used by LiquiDE's own shell, third-party panels. |
| `xdg_decoration_unstable_v1` | 1 | 1 | Shell | Server-side window decoration negotiation | CSD/SSD negotiation; default is SSD. |
| `wp_fractional_scale_v1` | 1 | 1 | Scaling | Sub-integer DPI scaling | Applied per-surface. Maps to client DPI. |
| `wp_viewporter` | 1 | 1 | Scaling | Surface viewport/crop/scale | Used for video surfaces, fractional scaling. |
| `zwp_text_input_v3` | 1 | 1 | Input | IME text input protocol | Full CJK/IME support. See §12. |
| `zwp_input_method_v2` | 1 | 1 | Input | Input method engine interface | Server-side IME engine. See §12. |
| `zwp_input_method_keyboard_grab_v2` | 1 | 1 | Input | IME keyboard grab | IME composition key interception. |
| `zwp_input_popup_surface_v2` | 1 | 1 | Input | IME candidate window positioning | Positioned relative to cursor in text field. |
| `zwp_virtual_keyboard_v1` | 1 | 2 | Input | On-screen keyboard | For touch/tablet clients. |
| `wp_primary_selection_unstable_v1` | 1 | 1 | Clipboard | Middle-click paste selection buffer | X11 primary selection compatibility. |
| `wp_content_type_v1` | 1 | 2 | Media | Surface content type hint | `none`, `photo`, `video`, `game` — informs encoder mode selection. |
| `wp_presentation_time` | 1 | 2 | Media | Frame presentation timestamps | Used for video sync, latency measurement. |
| `linux_dmabuf_v1` | 4 | 2 | Media | DMA-BUF buffer sharing | GPU mode only. Zero-copy import from GPU-rendered surfaces. |
| `ext_session_lock_v1` | 1 | 1 | Security | Session lock protocol | Secure lock screen. Prevents bypass. |
| `wp_security_context_v1` | 1 | 2 | Security | Sandboxed client restrictions | Flatpak/sandbox security boundary. |
| `xdg_activation_v1` | 1 | 2 | Shell | Cross-surface focus/activation tokens | Prevents focus stealing; token-gated activation. |
| `zwlr_foreign_toplevel_management_v1` | 3 | 2 | Shell | Task manager / window list | Used by dock and Alt-Tab for window metadata. |
| `wp_pointer_constraints_unstable_v1` | 1 | 2 | Input | Pointer lock and confinement | Used by games/3D apps. Latency constrained by network RTT. |
| `zwp_relative_pointer_v1` | 1 | 2 | Input | Relative pointer motion | Raw deltas for games/CAD. Discretized to 1px minimum. |
| `zwp_pointer_gestures_v1` | 3 | 3 | Input | Swipe, pinch, hold gestures | Touchpad gestures. Client forwarding required. |
| `wp_drm_lease_v1` | 1 | 3 | Media | DRM output lease | GPU mode only. VR headset passthrough use case. |
| `wp_color_management_v1` | 1 | 2 | Color | Surface color space and ICC profiles | Used by color-managed applications (GIMP, Firefox). Depends on WCG/HDR pipeline mode for full functionality. |
| `kde_server_decoration` | 1 | 3 | Compat | KDE server decoration protocol | Legacy compatibility for older KDE applications. |
| `org_kde_kwin_server_decoration_manager` | 1 | 3 | Compat | KDE decoration manager | Alternative decoration negotiation for KDE apps. |
| XWayland | Xwayland 24.1+ | 2 | Compat | X11 application compatibility | XWayland version tracks latest stable. Clipboard bridge with 2s timeout. HiDPI via Xft.dpi and randr. |

### RDP Compatibility
- See §18.

---

## 23a) Accessibility Conformance

LiquiDE is a remote desktop environment where accessibility operates at two layers: the **server-side desktop** (Wayland compositor, shell, applications) and the **client-side UI** (connection dialog, login screen, toolbar, crash screen, settings). Both layers must be independently accessible.

### Conformance Target

| Standard | Level | Scope |
|----------|-------|-------|
| **WCAG 2.1** | AA | Client-side UI: connection dialog, login, toolbar, settings, crash screen |
| **Section 508** | Applicable requirements | Entire product (for US government deployments) |
| **EN 301 549** | Applicable clauses | Entire product (for EU public sector deployments) |

"Accessibility done" for a release means: all automated accessibility tests pass, and manual screen reader testing has been completed on all Tier 1 platforms.

### Server-Side Accessibility (Remote Session)

Applications running inside the LiquiDE session use the standard Linux accessibility stack:

```
Application (GTK/Qt/etc.)
    │
    ▼ (AT-SPI2 D-Bus interface)
AT-SPI2 Registry (per-session, runs inside session)
    │
    ▼
LiquiDE AT-SPI2 Bridge
    │
    ├── Client-side screen reader passthrough
    │   (accessibility tree forwarded to client over dedicated channel)
    │
    └── Server-side audio output
        (screen reader speech rendered server-side via speech-dispatcher → audio channel)
```

#### Screen Reader Support

| Mode | Description | Trade-offs |
|------|-------------|-----------|
| **Server-side speech** (default) | Screen reader (Orca) runs inside the session. Speech synthesized server-side (speech-dispatcher + eSpeak-NG/Piper). Audio output sent to client via audio channel. | Works with all clients. Screen reader has full AT-SPI2 access. Latency dependent on audio pipeline. |
| **Client-side passthrough** | AT-SPI2 accessibility tree serialized and forwarded to client over a dedicated accessibility channel. Client's native screen reader (NVDA, JAWS, VoiceOver, Orca) reads the tree. | Lower latency. User's preferred screen reader voice. Requires client screen reader support in LiquidClient. |
| **Hybrid** | Server-side Orca for basic navigation. Client-side passthrough for detailed reading. | Best of both, most complex. |

Server-side speech is the default because it works with all client types (including RDP and web clients). Client-side passthrough requires explicit protocol support in LiquidClient.

#### AT-SPI2 Bridge Protocol

When client-side passthrough is enabled, the accessibility tree is serialized and forwarded:

| Message | Direction | Content |
|---------|-----------|---------|
| `A11yTreeSnapshot` | Server → Client | Full accessibility tree (on session start and major changes) |
| `A11yTreeDelta` | Server → Client | Incremental changes (node added, removed, property changed) |
| `A11yAction` | Client → Server | User action request (click, focus, expand, etc.) |
| `A11yTextQuery` | Client → Server | Text content request for a specific node |
| `A11yTextResponse` | Server → Client | Text content response |

Tree serialization format: CBOR encoding of AT-SPI2 accessible object tree (role, name, description, states, value, text, relations).

#### Server-Side Configuration

```toml
[accessibility]
enabled = true                         # enable accessibility infrastructure
screen_reader = "orca"                 # orca (default), none
speech_dispatcher = true               # enable speech-dispatcher for server-side TTS
a11y_channel = true                    # enable AT-SPI2 bridge for client passthrough
high_contrast_available = true
large_text_available = true
reduce_motion_available = true
on_screen_keyboard = true              # enable on-screen keyboard via zwp_virtual_keyboard_v1
```

### Client-Side Accessibility

Client-side UI elements (rendered by LiquidClient, not streamed from server) follow platform accessibility conventions:

| Platform | Accessibility API | Screen Reader Tested |
|----------|------------------|---------------------|
| **Windows** | UI Automation (UIA) | NVDA (primary), JAWS (secondary), Narrator (secondary) |
| **macOS** | NSAccessibility protocol | VoiceOver |
| **Linux** | AT-SPI2 | Orca |
| **Web** | WAI-ARIA | NVDA + Chrome, VoiceOver + Safari, Orca + Firefox |

#### Per-Element Requirements

| UI Element | Keyboard | Screen Reader | High Contrast | Large Text |
|------------|----------|--------------|--------------|------------|
| Connection dialog | Full Tab/Enter navigation | All fields labeled, server list announced | Solid backgrounds, visible borders | Text scales with OS setting |
| Login screen | Tab order: username → password → sign in → utilities | Avatar announced, auth method announced, errors announced as live regions | Glass replaced with solid panel | Input height increases |
| Session toolbar | Arrow key navigation between controls | Each control announced with name and state | Icons have visible labels | Toolbar height increases |
| Settings panel | Tab + arrow navigation, value adjustment via arrow keys | Section headings, control labels, current values announced | High-contrast toggle visible | Category text scales |
| Crash screen | Tab between action buttons, Enter to activate | Error type, description, and available actions announced | Emergency mode has high contrast by default | Error text uses larger size |
| USB device dialog | Tab between devices, Enter to forward/block | Device name, type, status announced | Device status indicators have text alternatives | Device list text scales |
| Print dialog | Tab between options | Printer names, settings announced | Standard dialog styling | Text scales |

#### Automated Testing

Accessibility conformance is verified by automated tests in CI:

| Test Tool | Scope | CI Integration |
|-----------|-------|---------------|
| `axe-core` (via `playwright-axe`) | Web client: WCAG 2.1 AA automated checks | PR pipeline (Tier 1 browsers) |
| Platform UI Automation tests | Native client: verify UIA tree on Windows, NSAccessibility on macOS | PR pipeline (Windows, macOS) |
| AT-SPI2 tests | Linux client: verify accessible tree | PR pipeline (Linux) |
| `Accessibility Insights` (manual) | Native client: guided manual testing | Release candidate QA |
| Screen reader testing (manual) | NVDA, VoiceOver, Orca | Release candidate QA |

**CI gate**: automated accessibility tests are blocking. A PR that introduces accessibility regressions (new WCAG violations detected by axe-core, missing UIA properties, etc.) is blocked.

#### Release Gate

A release is blocked if:
1. Any automated accessibility test fails.
2. Manual screen reader testing on Windows (NVDA) + macOS (VoiceOver) has not been completed.
3. Known P1 accessibility bugs are open.

A release may proceed if:
1. Known P2 accessibility bugs are documented in release notes.
2. Platform-specific issues on Tier 2 platforms (Linux Orca, JAWS) are documented.

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

### IME & Text Input
- Verify `zwp_text_input_v3` protocol: enable/disable, surrounding text, content type, cursor rectangle.
- Verify `zwp_input_method_v2` protocol: preedit, commit, keyboard grab, popup surface.
- Verify built-in Pinyin input: type "nihao" → candidate window shows "你好" → select → commit.
- Verify built-in Romaji/Kana input: type "nihon" → hiragana "にほん" → Kanji candidates → commit "日本".
- Verify built-in Hangul input: Jamo composition produces correct syllable blocks.
- Verify dead keys: `dead_acute` + `e` → `é` in text field.
- Verify Compose key: `Multi_key` + `o` + `c` → `©`.
- Verify external IBus bridge: ibus-daemon starts, methods list populated, input works end-to-end.
- Verify external Fcitx5 bridge: fcitx5 starts, methods work.
- Verify preedit rendering: underline, highlight, cursor position visible in application.
- Verify candidate window positioning follows cursor across screen edges.
- Verify XWayland compositor-side preedit fallback for legacy X11 applications.
- Verify RTL shell layout: Arabic locale produces mirrored dock/status bar layout.
- Verify BiDi mixed text: LTR + RTL in same paragraph renders correctly.
- Verify keyboard layout switching via `Super+Space` cycles through configured layouts.
- Verify per-window layout memory (when enabled): switching windows restores each window's layout.
- Verify IME over remote connection: preedit and candidates render correctly in streamed video/tiles.

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
- Worker task cancellation and replacement tests.
- Transport failover tests.

### Wayland Protocol Conformance

Tests verifying that the LiquiDE compositor correctly implements all supported Wayland protocols. Test tiers align with the protocol support matrix (§23).

**Every PR (Tier 1 protocols):**
- xdg_shell lifecycle: create toplevel → map → configure → ack → commit → close → destroy. Verify no leaks.
- xdg_shell popups: create popup with position constraints → reposition → dismiss (click outside) → destroy.
- xdg_shell window states: maximize, fullscreen, minimize, tiled → verify configure events with correct states.
- layer_shell: create surfaces on all four anchors (top, bottom, left, right) and `overlay` layer → verify stacking order.
- layer_shell exclusion zones: dock claims 48px bottom → verify toplevel window workarea excludes dock height.
- wl_seat keyboard focus: map two toplevels → click second → verify keyboard enter/leave events fire correctly.
- wp_fractional_scale: set scale to 1.25 → verify `preferred_scale` event → surface commits at correct buffer size.
- wp_viewporter: set viewport crop → verify compositor renders cropped region.
- Text input (zwp_text_input_v3): enable → enter → commit text → disable. Verify preedit and commit events.
- ext_session_lock_v1: lock → verify all outputs show lock surface → verify input rejected on non-lock surfaces → unlock.
- Clipboard (wl_data_device): set selection → request data → verify MIME types and content match.
- Primary selection: set selection → middle-click paste → verify content.
- weston-test-suite core subset: run `weston-test-suite` against LiquiDE compositor, core protocol tests only.
- Fuzz corpus replay: replay stored corpus of valid Wayland wire messages, verify no crashes.

**Nightly (Tier 2 protocols + extended Tier 1):**
- Full weston-test-suite: all protocol tests, including edge cases.
- xdg_activation_v1: app A generates token → sends to app B → app B requests activation → verify focus change.
- wp_content_type_v1: set content type `video` → verify compositor switches to video-mode encoding for that surface.
- wp_presentation_time: commit surface → verify presentation feedback timestamp is within 1 frame period of actual display.
- linux_dmabuf_v1 (GPU mode only): create dma-buf → import as surface → verify rendering. Skip if no GPU.
- wp_security_context_v1: create context → spawn sandboxed client → verify restricted protocol access.
- zwlr_foreign_toplevel_management_v1: list toplevels → verify metadata matches mapped windows → activate toplevel → verify focus.
- wp_pointer_constraints_unstable_v1: lock pointer → generate motion → verify confined to region → unlock.
- zwp_relative_pointer_v1: enable relative motion → move mouse → verify raw deltas reported.
- wp_color_management_v1: attach sRGB profile to surface → verify composited output matches (pixelwise, with tolerance).
- Protocol parser fuzz: 10,000 randomized Wayland wire messages → verify no crashes, no undefined behavior, appropriate protocol errors.
- xdg_decoration: negotiate SSD → verify server draws decorations. Negotiate CSD → verify server omits decorations.

**Per-release (Tier 3 + full integration):**
- zwp_pointer_gestures_v1: simulate pinch/swipe → verify gesture events forwarded.
- wp_drm_lease_v1: request lease → verify negotiation (GPU mode only).
- KDE server decoration: verify older KDE apps negotiate decorations correctly.
- Full manual QA: interact with 10+ real applications for 30 minutes, verify no visual/behavioral anomalies.

### Application Smoke Matrix

Automated application tests that verify LiquiDE's Wayland compatibility with real-world applications. Each application is launched, scripted interactions are performed, and results are checked for crashes, rendering artifacts, and correct behavior.

| Application | Toolkit | Tier | CI Cadence | Test Coverage |
|-------------|---------|------|------------|---------------|
| GNOME Text Editor | GTK4 | 1 | Every PR | Launch, type text, undo/redo, save dialog, close |
| Nautilus (Files) | GTK4 | 1 | Every PR | Launch, navigate directories, right-click menu, rename file |
| GNOME Terminal | VTE/GTK4 | 1 | Every PR | Launch, run command, scroll, copy/paste, tab create/close |
| Firefox | Gecko | 1 | Nightly | Launch, load page, scroll, text input, video playback, clipboard, print dialog |
| Chromium | Blink/Ozone | 1 | Nightly | Launch, load page, scroll, text input, WebGL, clipboard |
| VS Code | Electron | 1 | Nightly | Launch, open file, type, search, terminal panel, extensions sidebar |
| LibreOffice Writer | VCL/GTK3 | 1 | Nightly | Launch, type, format text, insert image, print preview |
| Dolphin (KDE Files) | Qt6 | 2 | Weekly | Launch, navigate, context menu, drag-and-drop |
| Kate (KDE Editor) | Qt6 | 2 | Weekly | Launch, open file, syntax highlight, search |
| mpv | WL_SHM/DMA-BUF | 2 | Weekly | Launch, play video (SHM + dmabuf paths), fullscreen toggle, seek |
| VLC | Qt5 | 2 | Weekly | Launch, play video, playlist, audio output |
| GIMP | GTK3 | 2 | Weekly | Launch, open image, draw, filters, color picker |
| Blender | Custom/GHOST | 2 | Weekly | Launch, viewport navigation, render preview |
| Flatpak app (sandboxed) | GTK4 | 2 | Weekly | Launch Flatpak app through security_context portal, verify sandboxed protocol access |
| Steam | SDL2 | 3 | Per-release | Launch, navigate store, launch a game (windowed) |
| Wine (winewayland.drv) | Wine/Wayland | 3 | Per-release | Launch Wine app, verify window management and input |
| Java/Swing (JetBrains) | AWT/Wayland | 3 | Per-release | Launch IntelliJ/JetBrains IDE, open project, type, navigate |

#### Application Compatibility Stance (User-Facing)

For users and administrators evaluating LiquiDE, this is the compatibility commitment:

| Tier | Guarantee | What It Means |
|------|-----------|---------------|
| **Tier 1** (Guaranteed) | Release-blocking. Regressions in Tier 1 apps block a release. | GTK4 apps (GNOME Text Editor, Nautilus, Terminal), Firefox, Chromium, VS Code, LibreOffice Writer. These applications are tested on every PR or nightly and MUST work correctly. |
| **Tier 2** (Supported) | High-priority fixes. Regressions are fixed within the next minor release. | Qt6/Qt5 apps (Dolphin, Kate, VLC), media players (mpv), image editors (GIMP), 3D tools (Blender), Flatpak sandboxed apps. These are tested weekly. |
| **Tier 3** (Best-effort) | Community-reported bugs accepted. Fixes may be deferred. | Steam, Wine/Winewayland, Java/Swing (JetBrains). These are smoke-tested per release. Breaking changes are acknowledged but may not block releases. |

**XWayland compatibility:**

- XWayland is supported as a **compatibility layer** for legacy X11 applications.
- XWayland apps run in Tier 2 or Tier 3 depending on the application.
- Known limitations: XWayland apps may have subtle input handling differences, clipboard format mismatches, and DPI scaling inconsistencies compared to native Wayland apps.
- XWayland is NOT a long-term strategy — LiquiDE prioritizes native Wayland support and encourages upstreams to migrate.

**What is NOT supported:**

- Direct GPU rendering / OpenGL passthrough for gaming (non-goal; see §2 Non-Goals).
- Windows-native applications (without Wine).
- Applications that require X11-specific extensions not implemented in XWayland.

### Security
- TLS configuration validation.
- Policy enforcement tests.
- Authentication brute-force protection.
- Audit log completeness.

### Policy Engine
- Verify `deny_overrides` resolution: server `false` + user `true` → effective `false`.
- Verify `deny_overrides` session override: admin override of deny → effective `true` + audit event emitted.
- Verify `min` resolution: server `60` + group `30` + user `120` → effective `30`.
- Verify `intersection` resolution: server `["quic", "tcp", "udp"]` + group `["quic", "tcp"]` → effective `["quic", "tcp"]`.
- Verify `highest_precedence` resolution: server `"balanced"` + user `"quality"` → effective `"quality"`.
- Verify empty list semantics: server `[]` (all) + group `["hid"]` → effective `["hid"]`.
- Verify multi-group conflict: user in groups A (`clipboard.enabled = true`) and B (`clipboard.enabled = false`) → effective `false` (deny wins).
- Verify locked keys: `locked: true` key set by user policy → user value ignored, server value used.
- Verify schema validation: policy file with out-of-range value → rejected at load, error logged.
- Verify unknown key in policy file → warning logged, key ignored.
- Verify policy hot-reload: modify policy file → new effective policy within 5 seconds.
- Verify `PolicyUpdate` message sent to session on policy change.
- Verify `policy.deny` audit event when clipboard blocked by policy.
- Verify `policy.change` audit event when key value changes due to file modification.
- Verify `liquidctl policy effective --user <user>` shows correct merged policy with source annotations.
- Verify `liquidctl policy effective --user <user> --add-group <group>` what-if shows correct hypothetical policy.
- Verify determinism: same inputs → identical effective policy across 1000 evaluations.

### Corporate Network & Transport
- Verify connection through HTTP CONNECT proxy (no auth, basic auth, NTLM).
- Verify PAC file evaluation selects correct proxy.
- Verify WPAD auto-discovery finds proxy in DHCP/DNS environment.
- Verify SOCKS5 proxy traversal with username/password auth.
- Verify TLS inspection detection: log warning when non-server CA in chain.
- Verify `tls.inspecting_proxy_cas` suppresses warning for approved CAs.
- Verify `tls.pinning = "enforce"` rejects connections through inspecting proxy without approved CA.
- Verify ALPN fallback: `liquide/1` → `h2` → `http/1.1` (WebSocket) when middlebox strips ALPN.
- Verify connectivity preflight completes in < 3 seconds on LAN.
- Verify connectivity preflight detects blocked QUIC and reports it.
- Verify `force_tcp = true` disables all UDP transports.
- Verify network profile auto-detection remembers proxy settings per SSID.

### RDP Compatibility
- Verify RDP connection from mstsc (Windows built-in) with NLA.
- Verify RDP connection from FreeRDP.
- Verify RDP clipboard (text, bidirectional).
- Verify RDP audio playback (RDPSND).
- Verify RDP drive redirection (read-only default).
- Verify RDP printer redirection via RDPDR.
- Verify RDP multi-monitor (up to 4).
- Verify RDP dynamic resolution (DISP channel).
- Verify legacy RC4 security layer is rejected.
- Verify NTLMv1 auth is rejected.

### Printing
- Verify client-redirect print: application prints → PDF → client → local printer.
- Verify PDF-download mode: application prints → PDF offered as download.
- Verify network-direct mode: application prints → CUPS → network printer.
- Verify client printer discovery (Windows, macOS, Linux).
- Verify web client receives PDF download (no local printer access).
- Verify DLP block: print job matching DLP rule is blocked, user sees block message.
- Verify DLP log-only: print job matching DLP rule is logged but allowed.
- Verify print audit events (`print.job_submitted`, `print.job_completed`, `print.job_blocked`).
- Verify max job size enforcement (job exceeding limit is rejected).
- Verify PDF temp files are cleared after delivery and on session end.

### Smart Card
- Verify client-side smart card authentication (certificate + nonce signing).
- Verify smart card forwarding: application in session accesses forwarded card via PC/SC.
- Verify PIN entry mode `local`: PIN entered on client, never sent to server.
- Verify PIN entry mode `remote`: PIN entered in session UI, forwarded via secure channel.
- Verify card insert/remove events forwarded in real-time.
- Verify `allowed_atr_patterns` restricts forwarding to specified card types.
- Verify `blocked_apdu_ins` blocks specific APDU instructions.
- Verify smart card audit events logged.

### Session Resume
- Verify session resume with valid token after network disconnect.
- Verify resume from different IP address (Wi-Fi → Ethernet).
- Verify resume through different gateway (token scope `any-gateway`).
- Verify token rotation: successful resume issues new token, invalidates old.
- Verify expired token falls back to full authentication.
- Verify revoked/invalid token falls back to full authentication.
- Verify `require_mfa_on_resume = true` requires MFA on every resume.
- Verify `require_mfa_after_hours` triggers MFA after configured time.
- Verify `max_disconnected_minutes` terminates session after timeout.
- Verify gateway-routed resume: gateway routes ResumeRequest to correct backend.

### Multi-Monitor DPI
- Verify single monitor at 100%, 150%, 200% DPI on Windows, macOS, Linux.
- Verify dual monitor with mixed DPI (1x + 2x) in "match local monitors" mode.
- Verify DPI change mid-session (window dragged between monitors).
- Verify DPI change debouncing (200ms).
- Verify server-side bilinear scaling for XWayland apps during DPI transition.
- Verify web client `devicePixelRatio` change detection.
- Verify fractional scale (1.25x) on Linux Wayland with `wp_fractional_scale_v1`.

### USB Safety
- Verify USB redirection is disabled by default (server and client).
- Verify client never auto-forwards devices without user confirmation.
- Verify security key (YubiKey, Titan) is auto-blocked with warning.
- Verify `blocked_vid_pid` overrides allow rules.
- Verify `allowed_device_classes` restricts forwarding to listed classes.
- Verify confirmation dialog shown before forwarding any device.
- Verify auto-disconnect of forwarded devices on session end.
- Verify USB audit events (`usb.device_forwarded`, `usb.device_blocked`, `usb.security_key_forward_attempt`).

### Accessibility
- Verify server-side Orca screen reader produces speech output via audio channel.
- Verify AT-SPI2 bridge serializes accessibility tree to client (passthrough mode).
- Verify client-side UI elements have correct UIA properties (Windows).
- Verify client-side UI elements have correct NSAccessibility properties (macOS).
- Verify crash screen is keyboard-navigable (Tab between buttons, Enter to activate).
- Verify high-contrast mode replaces glass effects with solid backgrounds.
- Verify `prefers-reduced-motion` disables animations.
- Verify web client passes axe-core WCAG 2.1 AA automated checks.

### Crash Handling
- Verify support bundle generation via `liquidctl support-bundle`.
- Verify PII scrubbing: username replaced with `<user>`, home paths scrubbed, passwords redacted.
- Verify coredump is NOT included in bundle by default.
- Verify coredump IS included when `include_coredump_in_bundle = true`.
- Verify "Share with Admin" sends bundle to configured HTTPS endpoint.
- Verify "Share with Admin" launches email client when endpoint is `mailto:`.
- Verify bundle manifest lists all included files and scrubbing applied.
- Verify custom scrubbing patterns (`additional_patterns`) are applied.
- Verify bundle size < 10 MB without coredump.

### Session Recording & Replay
- Verify recording starts automatically when `recording.enabled = true` and mode is `policy`.
- Verify recording indicator (red dot) appears in status bar for policy/admin recordings and cannot be hidden.
- Verify recording indicator does NOT appear for stealth observation (server does not send indicator).
- Verify `.lqr` file contains all streams: video, audio, input, clipboard, session events, annotations.
- Verify seek index allows seeking to any 10-second interval within 500ms.
- Verify adaptive frame rate: 10–15 fps during typing, 0.5 fps during idle, up to 30 fps during transitions.
- Verify recording storage: local filesystem, S3, SFTP backends all write successfully.
- Verify encryption at rest: `.lqr` file is AES-256-GCM encrypted with DEK wrapped by KEK.
- Verify Ed25519 digital signature on `.lqr` file validates correctly.
- Verify data redaction: clipboard content excluded when `redact_clipboard_content = true`.
- Verify data redaction: audio capture excluded by default.
- Verify playback: `liquidctl recording play` opens client with seek, speed control, event overlay.
- Verify export: `liquidctl recording export --format mp4` produces valid MP4.
- Verify retention: recordings older than `max_age_days` are automatically purged.
- Verify legal hold: recordings tagged with `legal_hold_tag` are never auto-deleted.
- Verify consent dialog: admin-initiated recording with `require_consent = true` shows consent dialog.
- Verify consent denied: recording does not start when user declines.
- Verify recording audit events: `recording.started`, `recording.stopped`, `recording.playback` emitted.
- Verify playback audit: accessing a recording logs viewer identity and timestamp.
- Verify recording does not degrade live session performance (< 5% CPU overhead).
- Verify policy key `recording.enabled` with `deny_overrides` resolution.

### Remote Assistance & Shadow Sessions
- Verify observer-initiated request: `AssistanceRequest` triggers `ConsentPrompt` on owner session.
- Verify consent dialog: shows observer name, role, mode, reason, and timeout countdown.
- Verify consent timeout: request auto-denied after `consent_timeout_seconds` (default 60s).
- Verify consent granted: observer receives live frame stream within 2 seconds.
- Verify view-only mode: observer can see screen but input events are rejected by server.
- Verify interactive mode: observer keyboard and mouse events are accepted and merged with owner input.
- Verify exclusive control: owner input is blocked, observer has full control. Owner can reclaim via `OwnerReclaimControl`.
- Verify ghost cursor: observer cursor appears as translucent overlay with name label on owner screen.
- Verify escalation: view-only → interactive requires separate consent prompt and acceptance.
- Verify invite code: owner generates code, observer uses `JoinWithCode`, connection established without consent dialog.
- Verify invite expiry: code expires after `invitation_expiry_seconds`.
- Verify max observers: connection rejected when `max_concurrent_observers` exceeded.
- Verify chat channel: text messages flow bidirectionally between owner and observer(s).
- Verify stealth observation: disabled by default. When enabled, no indicator shown to owner. Audit log entry every second.
- Verify stealth requires role: observer without `stealth_observe` permission is rejected.
- Verify assistance auto-records when `assistance.recording.auto_record = true`.
- Verify all audit events: `assistance.requested`, `assistance.consent_granted`, `assistance.started`, `assistance.escalated`, `assistance.ended`.
- Verify stealth audit events: `assistance.stealth_started` (warn), `assistance.stealth_active` (info, per-second), `assistance.stealth_ended`.
- Verify recording includes observer input attribution (which events came from observer vs owner).

### Seamless App Streaming (Extended)
- Verify per-OS taskbar integration: Windows (taskbar button with thumbnail), macOS (Dock icon with window activation), Linux (xdg_toplevel with `app_id`).
- Verify window grouping by app: multiple windows from same `app_id` grouped in taskbar.
- Verify system tray forwarding: remote `StatusNotifierItem` creates native tray icon on client.
- Verify tray icon context menu: `seamless_tray_menu_action` triggers correct action on server.
- Verify notification forwarding: remote D-Bus notification appears as native OS notification on client.
- Verify notification action: clicking action button on client notification sends `seamless_notification_action` to server.
- Verify drag-and-drop: local file dropped on seamless window triggers file upload.
- Verify drag-and-drop: remote text dragged from seamless window can be dropped into local app.
- Verify window type mapping: dialog windows, popups, tooltips mapped to correct native types per platform.
- Verify multi-monitor seamless: windows can be moved between client monitors with correct DPI scaling.
- Verify seamless + external keyboard: keyboard shortcut Ctrl+C/V works across local and remote context.

### GPU Server Mode
- Verify GPU detection: Vulkan physical device enumeration succeeds on machines with GPU.
- Verify auto-profile selection: `gpu-full` selected when HW encoder is available.
- Verify `cpu-only` fallback: session starts successfully when no GPU present.
- Verify GPU compositing: Vulkan compute shader pipeline composites surfaces correctly (pixel-compare with CPU path).
- Verify zero-copy: DMA-BUF import from Wayland client → Vulkan → encoder without CPU readback.
- Verify VAAPI encoding: H.264 hardware encode produces valid bitstream.
- Verify NVENC encoding: H.264/H.265 hardware encode via NVENC SDK.
- Verify encoder fallback: software encoder used when HW encoder session limit reached.
- Verify VRAM budget: session respects `vram_budget_mb` (reject allocations beyond).
- Verify VRAM exhaustion recovery: caches evicted, quality reduced, eventually falls back to CPU.
- Verify GPU error recovery: `VK_ERROR_DEVICE_LOST` triggers device reset and GPU resource re-creation.
- Verify GPU metrics: `liquide_gpu_vram_used_bytes`, `liquide_gpu_encode_time_seconds`, `liquide_gpu_fallback_total` emitted.
- Verify GPU SLOs: input-to-photon p50 <10ms on LAN with GPU mode.
- Verify GPU sharing: multiple sessions on same GPU (MPS or time-slice) operate concurrently without corruption.
- Verify policy key `gpu.mode` controls GPU profile selection.

---

## 25) Failure Modes & Mishap Hardening

This DE is explicitly designed to avoid common remote-desktop pain:

- **No hidden GPU dependency**: renderer stays CPU-only unless GPU explicitly enabled and available.
- **No hanging**: main thread is an orchestrator only; stuck worker tasks are canceled via `CancellationToken` and replaced. Blocking workers that fail to yield are escalated to the supervisor for process-level `SIGKILL`.
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
| Worker task hang | Low | Deadline exceeded (orchestrator heartbeat channel) | Cancel the task's `CancellationToken`, drop work handle, spawn replacement task. If blocking worker does not yield within 2× deadline, escalate to supervisor (session process hang). |
| Worker task panic | Low | Rust panic hook in `spawn_blocking` / `tokio::spawn` catch_unwind | Catch at task boundary via `JoinHandle` error, log panic, spawn replacement task |
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
6. **Crash report**: Generated and stored in `/var/log/liquide/crashes/crash-<session-id>-<timestamp>.json`.

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

Crash reports are stored server-side in `/var/log/liquide/crashes/`:

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
  "coredump_path": "/var/log/liquide/crashes/core.s001.20250115T162231"
}
```

- **Retention**: Configurable, default 30 days.
- **Prometheus metrics**: `liquide_crashes_total{type="session_crash"}`, `liquide_crash_restarts_total`, `liquide_session_uptime_at_crash_seconds` (histogram).
- **Optional telemetry upload**: Disabled by default. If enabled, crash reports (without coredumps or user data) are sent to a configurable endpoint.

### Support Bundle

A **support bundle** is a self-contained diagnostic archive that can be generated by users, administrators, or automatically on crash. It is designed for sharing with IT support or the LiquiDE development team. Support bundles undergo mandatory PII scrubbing before export.

#### Bundle Contents

| Category | Files Included | PII Risk | Scrubbing Applied |
|----------|---------------|----------|-------------------|
| **Crash reports** | All crash JSON files for the session | Low (usernames, session IDs) | Username hashed, session ID preserved (needed for correlation) |
| **Session logs** | Last 1000 log lines from the session | Medium (may contain filenames, app output) | File paths scrubbed (`/home/alice/...` → `/home/<user>/...`), IP addresses hashed |
| **Supervisor logs** | Last 500 log lines from supervisor | Low | Same scrubbing as session logs |
| **System info** | OS version, kernel, CPU, memory, GPU, disk | None | Included as-is |
| **Configuration** | `server.toml` (redacted), `session.toml` (redacted) | High (may contain passwords, tokens) | Passwords/tokens replaced with `<REDACTED>`. Paths preserved. |
| **Metrics snapshot** | Current Prometheus metric values | None | Included as-is |
| **Plugin list** | Active plugins with versions | None | Included as-is |
| **Transport stats** | RTT, bandwidth, packet loss, transport type | Low (server IPs) | Server IPs preserved (needed for diagnosis), client IPs hashed |
| **Coredump** | Core dump file (if available and configured) | High (may contain memory with user data) | **NOT included by default**. Admin must explicitly enable with `crash.include_coredump = true`. |
| **Screenshots** | **Never included** | — | Not collected |
| **Clipboard contents** | **Never included** | — | Not collected |
| **User files** | **Never included** | — | Not collected |

#### PII Scrubbing Rules

All text content in the support bundle passes through a scrubbing pipeline:

| PII Type | Detection | Replacement |
|----------|-----------|-------------|
| Usernames | Known session user + adjacent path patterns | `<user>` |
| Home directory paths | `/home/<username>/`, `C:\Users\<username>\` | `/home/<user>/`, `C:\Users\<user>\` |
| Email addresses | Regex `[\w.+-]+@[\w-]+\.[\w.-]+` | `<email>` |
| IPv4 addresses (client) | Regex + known client IP from session metadata | SHA-256 truncated to 8 hex chars |
| IPv6 addresses (client) | Regex + known client IP | Same SHA-256 truncation |
| Server IPs | Preserved (needed for network diagnosis) | Not scrubbed |
| Bearer tokens | Regex `Bearer [A-Za-z0-9._-]+` | `Bearer <REDACTED>` |
| Passwords in configs | Keys matching `password`, `secret`, `token`, `credential`, `key` in TOML | Value replaced with `<REDACTED>` |
| OIDC tokens | JWT patterns `eyJ...` | `<REDACTED_JWT>` |
| Smart card PINs | Never logged (not present in logs) | N/A |

Admin can configure additional scrubbing patterns:

```toml
[crash.scrubbing]
# Additional regex patterns to scrub from support bundles
additional_patterns = [
  { pattern = "ACME-\\d{8}", replacement = "<ACME_ID>" },
  { pattern = "SSN:\\d{3}-\\d{2}-\\d{4}", replacement = "SSN:<REDACTED>" },
]
# Scrub all file paths (not just home directories)
scrub_all_paths = false
# Scrub hostnames (replace internal hostnames with hashes)
scrub_hostnames = false
```

#### Bundle Generation

Support bundles can be generated via:

| Method | Initiator | Trigger |
|--------|-----------|---------|
| `liquidctl crash report --session <id> --bundle` | Admin (CLI) | On-demand |
| Crash screen "Download Report" button | User (client UI) | On crash |
| `liquidctl support-bundle` | Admin (CLI) | On-demand (no crash needed) |
| Manager UI "Export Support Bundle" | Admin (web UI) | On-demand |
| Automatic (on crash, if configured) | System | `crash.auto_bundle = true` |

Bundle format: `.tar.gz` archive with a manifest (`manifest.json`) listing contents and scrubbing applied.

Bundle size target: < 10 MB without coredump, < 500 MB with coredump.

#### Share-with-Admin Flow

The crash screen includes a **"Share with Admin"** button (in addition to "Download Report"):

```
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│   [ Restart Session ]   [ Share with Admin ]   [ Download ] │
│                         ▲                                   │
│                         │                                   │
│                    Sends bundle to                          │
│                    admin-configured                         │
│                    endpoint                                 │
└─────────────────────────────────────────────────────────────┘
```

When "Share with Admin" is clicked:
1. Client generates the support bundle (with PII scrubbing).
2. Client shows a summary of what will be included: "This report contains: crash details, system info, last 1000 log lines, configuration (passwords redacted), plugin list. It does NOT contain: screenshots, clipboard data, files, or passwords."
3. User confirms.
4. Bundle is uploaded to the configured admin endpoint.

Admin endpoint configuration:

```toml
[crash.share]
# Enable "Share with Admin" button on crash screen
enabled = true
# Endpoint for support bundle upload
# Can be: file path (local/network share), HTTPS URL, email address
endpoint = ""
# Examples:
# endpoint = "https://support.corp.example/crashes/upload"
# endpoint = "/mnt/shared/liquide-crashes/"
# endpoint = "mailto:it-support@corp.example"

# For HTTPS endpoints:
auth_method = "none"                  # none, bearer, basic
auth_token = ""
# Maximum bundle size for upload (bytes)
max_upload_size_mb = 50
# User-visible message shown after successful share
success_message = "Report submitted to IT support. Reference: {crash_id}"
```

When `endpoint` is an email address, the client launches the system email client with the bundle as an attachment and a pre-filled subject line: "LiquiDE Crash Report: {crash_id}".

#### Crash Configuration Summary

```toml
[crash]
# Storage
crash_dir = "/var/log/liquide/crashes"
retention_days = 30
max_crash_reports = 1000              # per server, oldest deleted first

# Coredumps
coredump_enabled = true               # capture coredumps on session crash
coredump_max_size_mb = 500
include_coredump_in_bundle = false    # never include by default — explicit admin opt-in

# Auto-bundle
auto_bundle = false                   # automatically generate support bundle on crash
auto_bundle_dir = "/var/log/liquide/bundles"

# Telemetry upload (developer telemetry, not admin share)
telemetry_upload = false
telemetry_endpoint = ""
telemetry_include_logs = false
telemetry_include_stack_trace = true

# Share with admin (see above)
[crash.share]
enabled = true
endpoint = ""

# PII scrubbing (see above)
[crash.scrubbing]
additional_patterns = []
scrub_all_paths = false
scrub_hostnames = false
```

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
  - Web client (see [spec-web-client.md](spec-web-client.md)).
  - Mobile clients — iOS and Android (see [spec-mobile.md](spec-mobile.md)).

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
  - GPU server mode deployment guide.
  - Mobile client deployment guide (MDM configuration, App Store / Play Store distribution).

- **Plugin SDK**:
  - `liquide-plugin-sdk` — Rust crate for developing WASM plugins.
  - Sample plugins (status bar widget, clipboard transformer, theme generator).

- **Crash Reporting**:
  - Crash report format specification.
  - Crash collection and analysis tooling.

- **Session Recording**:
  - `.lqr` recording format specification.
  - `liquidctl recording` CLI tooling (play, export, list, manage).
  - Recording storage backend integrations (local, S3, SFTP).
  - Manager UI recording browser and player.

- **Remote Assistance**:
  - Shadow session protocol implementation.
  - Consent flow UI components (client-rendered).
  - Chat channel implementation.
  - Invite code system.

- **Mobile Shared Library**:
  - `libmobileclient` — shared Rust core library for iOS (.xcframework) and Android (.so).
  - UniFFI binding generation for Swift and Kotlin.

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

## 27a) Authoritative Configuration Schema

All LiquiDE configuration defaults currently live across multiple spec documents (`server.toml` in spec.md §19, policy keys in §20, client config in spec-client.md, etc.). To prevent implementation drift, a single **machine-readable schema** serves as the authoritative source of truth for all configuration keys.

### Schema Format

The schema is defined in TOML Schema format (`config-schema.toml`) — a structured document that describes every configuration key:

```toml
[[keys]]
path = "general.hostname"
type = "string"
default = ""
description = "Server hostname for TLS certificate and client display"
required = false
env_var = "LIQUIDE_HOSTNAME"
cli_flag = "--hostname"
introduced = "1.0.0"

[[keys]]
path = "performance.active_fps"
type = "integer"
default = 60
min = 1
max = 240
description = "Target frames per second during active user interaction"
required = false
env_var = "LIQUIDE_ACTIVE_FPS"
cli_flag = "--active-fps"
introduced = "1.0.0"

[[keys]]
path = "encoding.allowed_encoders"
type = "list[string]"
default = ["h264", "h265"]
allowed_values = ["h264", "h265", "av1", "vp9"]
description = "Encoders available for session negotiation"
required = false
policy_key = "encoding.allowed_encoders"
policy_merge = "intersection"
introduced = "1.0.0"
```

### Generated Artifacts

The schema generates (via `tools/gen-config.sh`):

| Artifact | Purpose | Generated File |
|----------|---------|---------------|
| **Documentation tables** | spec.md §19 config tables, CLI help text | `docs/config-reference.md` |
| **Config validation** | Runtime config validation in `liquid-desktopd` | `crates/config/src/schema_generated.rs` |
| **CLI autocompletion** | Shell completions for `liquidctl config set/get` | `completions/{bash,zsh,fish}/liquidctl` |
| **Management UI forms** | Auto-generated settings forms in `liquid-manager` | `crates/manager-ui/src/config_forms_generated.ts` |
| **Default config file** | Commented `server.toml.example` with all defaults | `packaging/server.toml.example` |
| **JSON Schema** | For external tooling integration | `schema/config.schema.json` |

### Validation Rules

- On startup, `liquid-desktopd` validates the loaded config against the compiled schema.
- Unknown keys produce a `warn`-level log (not an error — forward compatibility).
- Out-of-range values produce an `error`-level log and the daemon refuses to start.
- Type mismatches produce an `error`-level log and the daemon refuses to start.

---

## 27b) Local Desktop Environment Mode

The `--local-session` flag (see [spec-system.md](spec-system.md) §13.3) enables LiquiDE to run as a **local desktop environment** — not just a remote desktop server. Local mode is supported as a **daily-driver desktop option** but is secondary to the primary remote use case.

### Mode Definition

| Aspect | Remote Mode (default) | Local Mode (`--local-session`) |
|--------|----------------------|-------------------------------|
| Transport | QUIC/TCP over network | Compositor renders directly to local DRM/KMS output |
| Encoding | H.264/H.265 video encoding | No encoding — direct scanout or software blit |
| Latency | Network-dependent (5–100ms) | Native (~1ms input-to-photon) |
| Multi-user | Multiple concurrent sessions | Single user session |
| GPU | Optional (encoding acceleration) | Used for direct rendering if available |
| Authentication | PAM/LDAP/OIDC over network | PAM via display manager (GDM, SDDM, greetd) |

### Local Mode Requirements

For daily-driver use, local mode additionally requires:

| Requirement | Description |
|-------------|-------------|
| **Display manager integration** | Session entry (`/usr/share/wayland-sessions/liquide.desktop`) works with GDM, SDDM, greetd, and Ly. Correct `DesktopNames=LiquiDE` for XDG detection. |
| **Power management** | Integration with UPower for battery status in the status bar. Idle detection triggers DPMS blanking. Lid close/open events handled (suspend/wake). Inhibit idle on video playback via `org.freedesktop.ScreenSaver` D-Bus. |
| **Network management** | NetworkManager integration via D-Bus for Wi-Fi, VPN, and wired connections. Status bar shows network status. |
| **Bluetooth** | BlueZ integration for Bluetooth device management (audio, input devices). |
| **Volume/brightness hardware keys** | XKB keysym mapping for `XF86AudioRaiseVolume`, `XF86AudioLowerVolume`, `XF86MonBrightnessUp`, etc. |
| **Removable media** | UDisks2 integration for USB drive mount/unmount notifications and file manager integration. |
| **Print management** | CUPS integration for printer discovery and management. |
| **Notifications** | D-Bus notification daemon (already part of LiquiDE) serves local apps directly without transport encoding. |

### What Local Mode Does NOT Change

- Shell UI, compositor, and Wayland protocol support are identical between local and remote mode.
- Configuration files, policies, and themes are shared.
- Flatpak, application launching, and XDG standards work identically.
- `liquidctl` works locally via Unix socket instead of network connection.

### Configuration

```toml
# /etc/liquide/local.toml (optional, local-mode-specific overrides)
[local]
power_management = true               # enable UPower integration
network_management = true             # enable NetworkManager integration
bluetooth = true                      # enable BlueZ integration
removable_media = true                # enable UDisks2 integration
dpms_standby_sec = 300                # DPMS standby after 5 minutes idle
dpms_suspend_sec = 600                # DPMS suspend after 10 minutes idle
lid_close_action = "suspend"          # "suspend", "lock", "nothing"
```

---

## 27c) Enterprise Credential & SSO Integration Summary

LiquiDE provides credential integration points across the full authentication lifecycle. This section consolidates the enterprise identity story.

### Authentication Methods

| Method | Spec Reference | Use Case |
|--------|---------------|----------|
| **PAM** | spec-system.md §5 | Local and LDAP authentication. Default for Linux deployments. |
| **OIDC / OAuth 2.0** | spec.md §16 (auth) | Enterprise SSO via Azure AD, Okta, Keycloak, Google Workspace. Recommended for enterprise. |
| **LDAP/AD** | spec.md §16 (auth) | Direct LDAP bind authentication. For environments without OIDC. |
| **Client certificate** | spec.md `[auth.certificate]` | Mutual TLS authentication. For zero-trust / high-security environments. |
| **FIDO2 / WebAuthn** | spec.md `[auth.fido2]` | Hardware key authentication (YubiKey, etc.). |
| **Smart card (PKCS#11)** | spec.md `[auth.smartcard]` | CAC/PIV smart card authentication. Government / defense deployments. |

### Kerberos / GSSAPI

- Kerberos ticket acquisition happens during PAM authentication (`pam_setcred` with `PAM_ESTABLISH_CRED`).
- The session inherits the Kerberos ticket cache (`KRB5CCNAME`).
- Applications in the remote session can use Kerberos tickets transparently (SSO to internal services, NFS mounts, etc.).
- HTTP proxy traversal supports GSSAPI/Negotiate authentication (see spec-client.md transport §7).

### Smart Card Forwarding

| Feature | Description |
|---------|-------------|
| PKCS#11 module | Configurable path to `.so`/`.dylib` for smart card access. |
| PIN entry location | `"local"` (client-side PIN pad) or `"remote"` (server-side PIN dialog). Default: `"local"` (most secure). |
| ATR pattern filtering | Restrict which card types are forwarded via `smartcard.allowed_atr_patterns`. |
| USB passthrough | Smart card readers forwarded via USB redirection (if card-level forwarding is insufficient). |

### Browser SSO in Remote Sessions

Web applications running in remote browsers (Firefox, Chromium) benefit from:

- **OIDC session cookies**: If OIDC auth was used for the LiquiDE session, the browser can pick up the same IdP session (cookie sharing with IdP depends on IdP SSO session lifetime).
- **Kerberos**: If the browser is configured with Negotiate auth (`network.negotiate-auth.trusted-uris` in Firefox), it uses the session's Kerberos tickets for SSO to internal web apps.
- **Certificate auth**: If client certificates are forwarded (via PKCS#11 or smart card), the browser can use them for mutual TLS to internal sites.

---

## 27d) Minimum Shell Feature Checklist

This checklist consolidates all required shell features. Each item MUST be implemented and tested before the shell is considered feature-complete. Features are specified in detail in their respective sections — this serves as an implementation tracking list.

### Window Management

| Feature | Spec Reference | Status |
|---------|---------------|--------|
| Alt+Tab window switcher (forward/backward) | §12 Keyboard Shortcuts | Required |
| Alt+Tab shows live thumbnails when bandwidth allows, else icons | §12 | Required |
| Super+Tab task overview / expose (all windows with thumbnails) | §12 | Required |
| Window snapping to half/quarter screen via Super+Arrow | §12 | Required |
| Edge/corner snap zones with semi-transparent preview | §12 Tiling | Required |
| Tiling layouts: split-h, split-v, quadrant, 3-col, spiral, stacking | §12 Tiling | Required |
| Super+Shift+Arrow swap tiled window positions | §12 | Required |
| Floating/tiling/hybrid mode toggle | §12 `[tiling]` | Required |
| Window minimize, maximize, close via title bar buttons | Server-side decorations | Required |
| Window resize from edges and corners | Compositor | Required |

### Workspace Management

| Feature | Spec Reference | Status |
|---------|---------------|--------|
| Super+Ctrl+Up workspace overview | §12 | Required |
| Super+Ctrl+Left/Right switch workspace | §12 | Required |
| Drag window to workspace edge to move to adjacent workspace | §12 | Required |
| Workspace indicator in status bar | Shell UI | Required |

### Multi-Monitor

| Feature | Spec Reference | Status |
|---------|---------------|--------|
| Super+Shift+Left/Right move window to adjacent monitor | §12 | Required |
| Per-monitor workspace (each monitor has independent workspace stack) | §12 | Required |
| Focus follows monitor (active monitor determined by cursor or last input) | Display management | Required |
| Monitor hotplug: add/remove monitors without session restart | §12 | Required |

### Launcher

| Feature | Spec Reference | Status |
|---------|---------------|--------|
| Super key opens launcher | §12 | Required |
| Fuzzy search across app name, description, keywords, categories | §12 Launcher | Required |
| Type-to-search (any keypress starts filtering) | §12 Launcher | Required |
| List view and grid view (Ctrl+G toggle) | §12 Launcher | Required |
| Plugin result providers (custom search results) | §12 Launcher | Required |

### Notifications

| Feature | Spec Reference | Status |
|---------|---------------|--------|
| Notification popups with urgency levels | spec-interop.md §3 | Required |
| Notification history (SQLite-backed, 7-day retention) | spec-interop.md §3 | Required |
| Reconnect sync (missed notifications sent on reconnect) | spec-interop.md §3 | Required |
| Do-not-disturb mode | Shell UI | Required |
| Critical notification override (DND bypass) | spec-interop.md §3 | Required |

### Screenshot & Recording

| Feature | Spec Reference | Status |
|---------|---------------|--------|
| Print → screenshot full desktop (save to file) | §12 Shortcuts | Required |
| Alt+Print → screenshot active window | §12 Shortcuts | Required |
| Super+Shift+S → region select screenshot | §12 Shortcuts | Required |
| Super+Print → screenshot to clipboard | §12 Shortcuts | Required |
| Super+Shift+R → toggle screen recording | §12 Shortcuts | Required |

### System Tray & Status Bar

| Feature | Spec Reference | Status |
|---------|---------------|--------|
| Clock / date display | Shell UI | Required |
| Network status indicator | Shell UI | Required |
| Audio volume indicator + quick slider | Shell UI | Required |
| Keyboard layout indicator (click to switch) | §12 Keyboard | Required |
| Battery indicator (local mode only) | §27b Local Mode | Required (local mode) |
| Notification count badge | Shell UI | Required |

### Dock

| Feature | Spec Reference | Status |
|---------|---------------|--------|
| Pinned application shortcuts | Shell UI | Required |
| Running application indicators | Shell UI | Required |
| Drag to reorder | Shell UI | Required |
| Right-click context menu (new window, quit, pin/unpin) | Shell UI | Required |
| Auto-hide option | Shell UI `[dock]` | Required |
| Position: bottom (default), left, right, top | Shell UI `[dock]` | Required |

### Global Search vs. Launcher Scope

The launcher's search scope is explicitly defined:

| Search Source | Included by Default | Scope |
|--------------|-------------------|-------|
| Installed applications (`.desktop` files) | Yes | Name, description, keywords, categories, executable |
| Recent files (via GTK recent manager / Zeitgeist) | Yes | File name only |
| Calculator (inline math evaluation) | Yes | Arithmetic expressions |
| Plugin results | Yes (if plugins installed) | Plugin-defined |
| File system search | No (opt-in) | Full-text file name search |
| Web search | No (opt-in) | Redirect to default browser |

Global search (Super then type) is **launcher-scoped by default**. A separate file manager search (Ctrl+F in file manager) provides file content search. These are intentionally separate to keep the launcher fast.

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
- Default color pipeline: **SDR-sRGB, 8-bit per channel**. Wide color gamut (WCG-SDR) and HDR modes are opt-in via `display.color.pipeline_mode` configuration. This ensures zero performance and compatibility impact for deployments that do not require deep color.

---

## 29) Implementation Status

> This section tracks the current implementation state of the LiquiDE codebase. It is updated as crates are implemented.

### Workspace Statistics

| Metric | Value |
|--------|-------|
| Total crates | 40 |
| Fully implemented | 36 (90%) |
| Stub / scaffolded | 4 (10%) |
| Total `.rs` source files | 620 |
| Total lines of Rust | ~82,000 |
| Crates with test suites | 27 |

### Crate Implementation Matrix

Each crate is categorized by implementation status:

- **Implemented** — has full module structure, types, logic, and (usually) tests.
- **Stub** — crate exists in the workspace but contains only placeholder code.

#### Core Infrastructure

| Crate | Modules | Source Files | Tests | Binary | Description |
|-------|---------|-------------|-------|--------|-------------|
| `liquide-common` | 3 | 4 | — | — | Shared config, error, logging utilities |
| `liquide-protocol` | 5 | 6 | — | — | Wire protocol: channels, codecs, frames, messages, versioning |
| `liquide-crypto` | 3 | 4 | — | — | TLS, certificates, token management |
| `liquide-transport` | 6 | 7 | — | — | QUIC, TCP, UDP, WebSocket transports |
| `liquide-auth` | 5 | 6 | — | — | PAM, LDAP, OIDC, MFA authentication providers |
| `liquide-policy` | 4 | 5 | — | — | Policy engine: rules, evaluation, hierarchy |

#### Server-Side

| Crate | Modules | Source Files | Tests | Binary | Description |
|-------|---------|-------------|-------|--------|-------------|
| `liquide-session` | 10 | 13 | Yes | `liquid-session` | Per-user session lifecycle, state machine, crash recovery, resume, sandboxing |
| `liquide-gateway` | 14 | 22 | Yes | `liquid-gateway` | Connection gateway: routing, relay, rate limiting, health, cluster |
| `liquide-supervisor` | 14 | 15 | Yes | `liquid-desktopd` | Session supervisor: admission control, heartbeat, crash handling, restart policy, resource monitoring, auto-downgrade |
| `liquide-manager` | 11 | 26 | Yes | `liquid-manager` | Management backend: dashboard, server/session/user/gateway/policy subsystems, audit, metrics, REST API (23 endpoints) |
| `liquide-compositor` | 8 | 17 | Yes | — | Wayland-style compositor: damage tracking, scene graph, effects |
| `liquide-shell` | 10 | 23 | Yes | — | Desktop shell: windows, workspaces, focus, layout, dock |
| `liquide-ctl` | 29 cmds | 35 | — | `liquidctl` | Admin CLI tool |

#### Rendering & Encoding

| Crate | Modules | Source Files | Tests | Binary | Description |
|-------|---------|-------------|-------|--------|-------------|
| `liquide-renderer-cpu` | 8 | 20 | Yes | — | CPU software renderer: rasterizer, blending, blur, glyphs |
| `liquide-renderer-gpu` | 13 | 21 | Yes | — | GPU-accelerated renderer: Vulkan device probing, profile selection, VRAM budget, compute pipeline, blur, compositing, DMA-BUF, fallback |
| `liquide-encoder` | 9 | 20 | Yes | — | Tile encoder: delta, hashing, bandwidth, compression strategies |
| `liquide-encoder-hw` | 22 | 32 | Yes | — | HW encoding abstraction: VAAPI, NVENC, AMF, V4L2 |
| `liquide-css` | 4 | 5 | — | — | CSS theming: parser, properties, values, theme engine |

#### Client-Side

| Crate | Modules | Source Files | Tests | Binary | Description |
|-------|---------|-------------|-------|--------|-------------|
| `liquide-client` | 15 | 25 | Yes | `liquidclient` | Native desktop client: connection, display, input, cursor, decoder, overlay, crash screen, color, credentials |
| `liquide-client-renderer` | 6 | 15 | Yes | — | Client-side rendering: frame decode, surface presentation, cursor |

#### Input / Output

| Crate | Modules | Source Files | Tests | Binary | Description |
|-------|---------|-------------|-------|--------|-------------|
| `liquide-input` | 6 | 15 | Yes | — | Input system: keyboard, mouse, touch, event routing |
| `liquide-clipboard` | 5 | 13 | Yes | — | Clipboard: formats, offers, transfer, storage |
| `liquide-audio` | 7 | 17 | Yes | — | Bidirectional audio: buffers, codecs, devices, sessions |
| `liquide-usb` | 10 | 22 | Yes | — | USB redirection: device forwarding, smart cards, file transfer, bandwidth |

#### Features & Extensions

| Crate | Modules | Source Files | Tests | Binary | Description |
|-------|---------|-------------|-------|--------|-------------|
| `liquide-plugin-abi` | 3 | 4 | — | — | Plugin ABI: host functions, manifest, types |
| `liquide-plugin-host` | 6 | 14 | Yes | — | WASM plugin runtime: sandbox, resources, dispatcher |
| `liquide-recording` | 7 | 17 | Yes | — | Session recording: format, muxer, retention, storage |
| `liquide-interop` | 6 | 15 | Yes | — | Desktop interop: XDG, MIME, icons, notifications, tray |
| `liquide-a11y` | 6 | 15 | Yes | — | Accessibility: tree, focus, screen reader, navigation |
| `liquide-assistance` | 17 | 30 | Yes | — | Remote assistance: shadowing, consent, chat, invite, stealth, policy |
| `liquide-ui` | 11 | 20 | Yes | — | UI toolkit: geometry, widget tree, layout engines (box/stack/grid), focus chain, animation, paint context, panels, theming |

#### Built-in Applications

| Crate | Modules | Source Files | Tests | Binary | Description |
|-------|---------|-------------|-------|--------|-------------|
| `liquide-apps-terminal` | 10 | 17 | Yes | `liquid-terminal` | Terminal emulator: VT parser, character grid, PTY, scrollback, search, shell integration, URL detection, tabs |
| `liquide-apps-files` | 10 | 17 | Yes | `liquid-files` | File manager: directory listing, natural sort, sidebar bookmarks, preview, clipboard, search, file operations queue, navigation history |
| `liquide-apps-settings` | 9 | 16 | Yes | `liquid-settings` | Settings app: 8 categories, typed entries with validation, change tracking with undo/redo, policy constraints, notifications |
| `liquide-apps-text-editor` | 10 | 17 | Yes | `liquid-text-editor` | Text editor: line buffer, cursor/selection/multi-cursor, syntax highlighting (5 languages), auto-indent, search/replace, undo/redo, diagnostics, multi-document |
| `liquide-apps-software-center` | 9 | 16 | Yes | `liquid-software-center` | Software center: package catalog with search, repository management, install queue with progress, update manager, reviews/ratings, screenshot gallery |

#### Stub Crates (Not Yet Implemented)

| Crate | Category | Description |
|-------|----------|-------------|
| `liquide-manager-frontend` | Management | Management UI frontend |
| `liquide-mobile-core` | Client | Shared mobile client library (iOS/Android) |
| `liquide-bench` | Testing | Benchmark suite |
| `liquide-conformance` | Testing | Conformance test suite |

### Scope Completion Summary

| Milestone | Total Features | Implemented | Percentage |
|-----------|---------------|-------------|------------|
| **MVP** | 9 | 9 | **100%** |
| **v1** | 11 | 11 | **100%** |
| **vNext** | 16 | 16 | **100%** |
| **Overall** | 36 | 36 | **100%** |

### Features Implemented Beyond Original Spec

The following capabilities were implemented but not originally listed in sections 22/26:

| Feature | Crate | Spec Section Coverage |
|---------|-------|-----------------------|
| Accessibility (screen reader, focus tracking, navigation) | `liquide-a11y` | §12 Input System (partial) |
| Hardware encoding abstraction (VAAPI/NVENC/AMF/V4L2) | `liquide-encoder-hw` | §8 Transport & Codec Strategy |
| Client-side renderer with GPU decode paths | `liquide-client-renderer` | §7 Remote Display Model |
| Desktop interop (XDG, MIME, icons, notifications, tray) | `liquide-interop` | §14 Desktop Environment |
| Audio bidirectional streaming with codec negotiation | `liquide-audio` | §10 Bidirectional Audio & Media |
| Session recording with `.lqr` format | `liquide-recording` | §16 Stream Analysis |
| Stealth monitoring mode for assistance | `liquide-assistance` | §15 Security |
| USB smart card reader redirection | `liquide-usb` | vNext scope |
| HDR/WCG color pipeline in client | `liquide-client` | §7 Remote Display Model |
| Admission control with resource reservation | `liquide-supervisor` | §13 Session Management |
| Auto-downgrade levels (ReduceFps → TileOnly → ReduceQuality → Suspend) | `liquide-supervisor` | §25 Failure Modes |
| GPU profile auto-selection (CpuOnly through GpuDedicated) | `liquide-renderer-gpu` | §6 Rendering Stack |
| VRAM budget management with allocation tracking | `liquide-renderer-gpu` | §6 Rendering Stack |
| Render target pooling and DMA-BUF import | `liquide-renderer-gpu` | §6 Rendering Stack |
| Widget tree with z-order hit testing | `liquide-ui` | §14 Desktop Environment |
| Box/stack/grid layout engines | `liquide-ui` | §14 Desktop Environment |
| Animation manager with easing curves | `liquide-ui` | §14 Desktop Environment |
| Focus chain with directional navigation | `liquide-ui` | §14 Desktop Environment |
| Policy versioning with diff and rollback | `liquide-manager` | §18 Management |
| Dashboard builder with alert severity levels | `liquide-manager` | §18 Management |
| Admin lockout after failed authentication attempts | `liquide-manager` | §15 Security |
| Metrics collector with configurable retention | `liquide-manager` | §16 Stream Analysis |
| VT100/xterm sequence parser with SGR, OSC, CSI dispatch | `liquide-apps-terminal` | §14 Desktop Environment |
| Shell integration via OSC 7/133 (CWD, prompt, command tracking) | `liquide-apps-terminal` | §14 Desktop Environment |
| URL and file path detection in terminal output | `liquide-apps-terminal` | §14 Desktop Environment |
| Natural sort for file listings (file1 < file2 < file10) | `liquide-apps-files` | §14 Desktop Environment |
| File operation queue with progress tracking | `liquide-apps-files` | §14 Desktop Environment |
| Navigation history with back/forward | `liquide-apps-files` | §14 Desktop Environment |
| Typed setting entries with validation and slider/choice/toggle kinds | `liquide-apps-settings` | §14 Desktop Environment |
| Policy engine for locked/hidden/read-only setting constraints | `liquide-apps-settings` | §14 Desktop Environment |
| Undo/redo change tracker with pending/applied stacks | `liquide-apps-settings` | §14 Desktop Environment |
| Line-oriented text buffer with insert/delete/range operations | `liquide-apps-text-editor` | §14 Desktop Environment |
| Syntax highlighting tokenizer for Rust, Python, JS, C, TOML | `liquide-apps-text-editor` | §14 Desktop Environment |
| Multi-cursor editing with selection support | `liquide-apps-text-editor` | §14 Desktop Environment |
| Package catalog with scored search ranking | `liquide-apps-software-center` | §14 Desktop Environment |
| Install queue with download/install progress tracking | `liquide-apps-software-center` | §14 Desktop Environment |
| Repository manager with official/community/flatpak defaults | `liquide-apps-software-center` | §14 Desktop Environment |

