# Remote-first Liquid Glass Desktop (CPU-only) — Full App Spec

## 0) Concept
A **remote-first, native desktop environment** designed specifically for Remote Desktop use-cases:
- **Runs well with *no GPU*** (headless servers, VMs, iGPU-less boxes, cloud instances).
- **Feels like a modern “liquid glass” OS** (depth, translucency, blur, vibrancy) while remaining bandwidth/CPU efficient.
- **Implements aggressive performance optimizations end-to-end** (render → capture → encode → transport → decode → present).
- Works as a **complete desktop session** (shell + compositor + core apps), not a “screenshare your existing GNOME/KDE” solution.

Working name: **LiquidDE** (server) + **LiquidClient** (clients).

---

## 1) Product goals
### Primary goals
1. **Remote experience that feels local**
   - Low input latency.
   - Smooth motion under real networks (Wi‑Fi, LTE, congested WAN).
   - High text clarity for terminals/IDEs.

2. **Zero-GPU server requirement**
   - Compositor and UI must run on pure CPU.
   - No dependency on OpenGL/Vulkan being functional.

3. **Dynamic resize and multi-monitor virtualization**
   - Client window resize adjusts the remote “virtual monitor(s)” without reconnect.
   - Support: single monitor, multi-monitor, fractional scaling.

4. **Clipboard that actually works**
   - Bi-directional text by default.
   - Optional image + file list + rich text.
   - Policy-controlled (disable, whitelist, size limits, audit).

5. **Secure-by-default**
   - TLS everywhere.
   - Modern auth options.
   - Reasonable hardening and audit logs.

### Non-goals (initially)
- Perfect pixel parity with local GNOME/KDE.
- Gaming / high-end 3D rendering (can be added later with optional GPU acceleration).
- Implement every RDP device redirection feature from day 1.

---

## 2) Target users & scenarios
### Personas
- **IT / homelab / MSP**: wants stable remote Linux sessions with predictable behavior.
- **Developers**: terminal/IDE-heavy usage, needs crisp text, low latency, reliable clipboard.
- **Headless servers**: wants a “real desktop session” without GPU.

### Scenarios
- Remote session into a VM on a Proxmox/ESXi host.
- Remote session into an Arch box running headless Wayland.
- Remote session into a machine that crashes when a GPU/driver path is touched.

---

## 3) System requirements
### Server
- Linux (first-class). Must run on:
  - Bare metal, VMs, containers.
  - No GPU (CPU-only rendering).
- CPU: x86_64 or aarch64.
- RAM: 1–2 GB for minimal session; 4 GB recommended.

### Client
- Windows, Linux, macOS (native clients).
- Optional web client (WebRTC) for “no install”.

---

## 4) Architecture overview
LiquidDE is built from cleanly separated layers:

1. **Session Manager**
   - Auth, user session lifecycle, per-user isolation.

2. **Remote-first Compositor + Shell**
   - Wayland compositor built for CPU rendering.
   - Shell UI (panel, launcher, notifications, overview).

3. **Frame Graph + Damage Tracker**
   - Tracks what changed and produces minimal updates.

4. **Encoder + Transport**
   - Multi-codec pipeline.
   - Adaptive bitrate + latency tuning.

5. **Client**
   - Receives stream/updates, decodes, presents.
   - Handles input, clipboard, resize events.

### High-level data flow
- Client input → server compositor
- Compositor produces frame updates → encoder (or tile encoder)
- Transport sends → client decoder → present

---

## 5) Core design decision: “Remote-native” rendering
### Why not screenshare GNOME/KDE?
Screenshare stacks are optimized for “share what’s on screen”, not:
- Dynamic monitor resize.
- Multiplexed sessions.
- Remote-centric text clarity.
- Predictable CPU-only behavior.

LiquidDE is its own DE and compositor.

---

## 6) Rendering stack (CPU-first)
### Compositor
- **Wayland compositor** implemented in Rust or C.
- Must support:
  - xdg-shell, layer-shell (panels), input-method basics.
  - fractional scaling.
  - headless backend (no DRM needed).

### Renderer (no GPU)
- **CPU raster renderer** (Skia raster or Pixman-like compositing).
- SIMD accelerated compositing:
  - AVX2/AVX512 where available.
  - NEON on ARM.
- Font stack:
  - FreeType + HarfBuzz.
  - Subpixel rendering *only when safe* (configurable; remote codecs can blur subpixel).

### “Liquid glass” without melting the CPU
Glass is expensive (blur, translucency), so the renderer uses:
1. **Blur caching**
   - Glass surfaces keep a cached blurred backdrop.
   - Recompute blur only when background behind that surface changes.

2. **Downsampled blur**
   - Blur computed at 1/4 or 1/8 resolution + upsample.
   - Optional multi-pass separable blur.

3. **Effect budgets**
   - Each frame has a CPU budget (ms).
   - If over budget: reduce blur radius, reduce shadow samples, skip non-essential highlights.

4. **Animation policy**
   - Default animations are **event-driven** (input/transition) rather than constant.
   - Frame rate caps for UI-only animation (e.g., 30 fps) while cursor/input stays responsive.

---

## 7) Remote display model
### Virtual monitors
- Each session owns one or more **virtual monitors**.
- Client can:
  - Add/remove monitors.
  - Resize monitors dynamically.
  - Set DPI scale per monitor.

### Resize behavior
- When client window resizes, client sends a **Display Update** message.
- Server updates the virtual monitor(s), re-layouts UI, and continues without reconnect.
- For clients lacking true dynamic-res support:
  - Fall back to **smart scaling** (server fixed resolution, client scales) with optional “fit to window”.

### Multi-monitor mapping
- Supports:
  - “Match local monitors” (1:1)
  - “Single large canvas” (panorama)
  - “Tabbed monitors” (fast switch)

---

## 8) Transport & codec strategy
LiquidDE supports two modes:

### Mode A — Video stream (best for general UI)
- Encode as:
  - H.264 (fast, broad compatibility)
  - H.265/HEVC (optional)
  - AV1 (optional)
- Key optimizations:
  - **Separate cursor channel** (don’t encode cursor into video).
  - **Damage-aware encoding**: only encode dirty regions when codec supports ROI / region refresh; otherwise adjust QP/bitrate and skip frames.
  - **Adaptive frame pacing**:
    - Idle: 1–5 fps (or “only-on-change”).
    - Interaction: ramp to 30–60 fps.
  - **GOP tuning** for low latency.

### Mode B — Tile / bitmap stream (best for crisp text)
- Screen is partitioned into tiles (e.g., 64×64 or 128×128).
- Per tile:
  - Hash + change detection.
  - Compress changed tiles (Zstd/LZ4 + optional PNG for text-heavy tiles).
- Great for:
  - terminals, code editors, dashboards.
- The system can **hybridize**: video for large moving regions, tiles for text regions.

### Network transport
- Prefer **UDP/QUIC** where possible (low latency + loss recovery).
- Fall back to TLS over TCP.

---

## 9) Major performance optimizations checklist
### Capture / render
- Damage tracking at surface + tile level.
- Occlusion culling: don’t composite fully covered surfaces.
- Partial present: update only changed tiles.
- Cursor out-of-band.
- Double/triple buffering with frame pacing.

### Encode
- Multi-threaded encode with CPU affinity.
- Adaptive bitrate (ABR) based on:
  - packet loss
  - RTT
  - decode queue depth
  - input activity
- Content-type heuristics:
  - Text / UI → tile mode bias
  - Video playback → video mode bias
- Encoder presets:
  - “Interactive” (latency)
  - “Balanced”
  - “Bandwidth saver”

### Transport
- Packetization optimized for MTU.
- Forward error correction (optional).
- Congestion control tuned for interactive traffic.

### Client present
- Frame queue with jitter buffer.
- Present-time prediction.
- Input-to-photon metrics and adaptive tuning.

---

## 10) Clipboard & data channels
### Clipboard types
- Text (UTF‑8) — default.
- Rich text (HTML/RTF) — optional.
- Images — optional (size limit).
- File list — optional (maps to file transfer channel).

### Clipboard policy engine
Configurable per session/user:
- Enable/disable direction:
  - client → server
  - server → client
- Max size per item.
- Allowed MIME types.
- Audit log of clipboard events (metadata only, not content by default).

### File transfer channel
- Optional, policy-controlled.
- Two modes:
  1) “Drag & drop into session” (client uploads to server)
  2) “Browse server files” (read-only or read-write)

---

## 11) Input system
- Keyboard:
  - Layout-aware scancodes.
  - IME friendly basics.
- Mouse:
  - relative and absolute mode.
  - high precision wheel.
- Touch (future): gestures mapped to shell actions.

Special focus: **no ‘sticky modifiers’ under latency**.

---

## 12) Session management & isolation
### Session manager responsibilities
- Auth handshake.
- Start/stop sessions.
- Allocate virtual monitors.
- Assign policies (clipboard/file transfer).

### Isolation model
- Each user session runs in one of:
  - systemd --user session
  - user namespace sandbox
  - optional container (bubblewrap)

### Persistence
- “Resume last session” optional.
- Idle timeout + disconnect behavior:
  - lock session
  - keep running
  - terminate

---

## 13) Security
### Transport security
- TLS 1.3.
- Server certificate management:
  - self-signed bootstrap
  - ACME/Let’s Encrypt (optional)
  - enterprise PKI import

### Authentication
- Options:
  - Local accounts
  - PAM
  - LDAP/AD via PAM
  - OIDC (optional)

### Authorization
- Per-user policies:
  - clipboard allowed?
  - file transfer allowed?
  - max sessions
  - allowed client platforms

### Audit logging
- Log:
  - login success/failure
  - session start/stop
  - policy changes
  - clipboard/file transfer events (metadata)

---

## 14) Observability & operations
### Metrics (Prometheus)
- FPS (server render, encode, client present)
- Latency (input-to-photon estimate)
- Bandwidth in/out
- Packet loss / RTT
- Dirty region ratio
- Encode time per frame

### Logs
- Structured logs (JSON option).
- Per-session log correlation ID.

### Admin tools
- `liquidctl status` (sessions, bandwidth, latency)
- `liquidctl sessions list/kill`
- `liquidctl policy set ...`

---

## 15) User experience (Liquid Glass design)
### Design principles
- **Depth**: layers and translucency convey hierarchy.
- **Clarity**: text remains sharp, backgrounds soften.
- **Motion discipline**: animations never spam frames.
- **Remote-first ergonomics**:
  - big hit targets
  - clear focus outlines
  - “connection quality” indicator

### Visual language
- Glass panels with:
  - subtle blur
  - frosted noise texture
  - specular highlights that respond to pointer position (but throttled)
- Shadows are soft and cached.
- Accent color configurable.

### Shell UI components
- Top bar / side dock (configurable).
- App launcher + search.
- Window switcher (Alt-Tab) optimized for remote.
- Notifications with “stacking” to avoid animation storms.

### Accessibility
- Reduce motion toggle.
- High contrast mode.
- Large cursor + separate cursor channel.

---

## 16) Built-in core apps (remote-friendly)
1. **Liquid Terminal**
   - GPU-free terminal renderer.
   - Text-only fast path.
   - Optional ligatures.

2. **File Manager**
   - Large preview generation done lazily.
   - Network mounts optional.

3. **Settings**
   - Performance profiles
   - Clipboard policy
   - Display & scaling

4. **Task Monitor**
   - Shows CPU, RAM, encode load, FPS, latency.

---

## 17) Compatibility strategy
### RDP interoperability (optional path)
- Provide an RDP endpoint for “connect with mstsc”.
- If full RDP is too heavy for MVP:
  - ship native client first
  - add RDP compatibility later

### Native protocol (recommended for MVP)
- Purpose-built protocol for:
  - dynamic monitors
  - hybrid tile+video
  - strict clipboard policy
  - QUIC transport

---

## 18) Configuration
### Files
- `/etc/liquidde/config.toml` (server)
- `~/.config/liquidde/session.toml` (user)

### Example: server config (concept)
- listen address/port
- tls cert/key
- auth provider
- default policies
- codec preferences
- performance profile defaults

---

## 19) Implementation plan
### Language choices
- **Server compositor + shell**: Rust (safety + performance) or C (wlroots-like ecosystem).
- **Encoder/transport**: Rust with native bindings.
- **Client**: Rust + native UI toolkit per platform.

### MVP scope (first usable)
- Headless server session
- One virtual monitor
- Native client
- Clipboard (text)
- Dynamic resize
- Video mode + cursor channel
- Basic shell: panel, launcher, terminal

### v1 scope
- Multi-monitor
- Hybrid tile/video
- Clipboard rich types
- File transfer
- Metrics dashboard
- Policy engine

### vNext
- Web client (WebRTC)
- OIDC
- GPU acceleration option
- RDP compatibility endpoint

---

## 20) Test plan
### Functional
- Connect/disconnect/reconnect behavior.
- Clipboard bidirectional.
- Resize storms (rapid dragging).
- Multi-monitor add/remove.

### Performance
- Measure:
  - input-to-photon
  - encode time
  - bandwidth usage
- Network emulation:
  - high RTT
  - packet loss
  - bandwidth caps

### Reliability
- Fuzz protocol decoding.
- Long-run session soak tests.

---

## 21) Failure modes & “mishap hardening”
This DE is explicitly designed to avoid common remote-desktop pain:
- **No hidden GPU dependency**: renderer must stay CPU-only end-to-end.
- **No “unknown subcommand” dead ends**: one CLI controls everything, versioned and self-documenting.
- **Clipboard is a first-class feature**: not an afterthought.
- **Dynamic resizing is built-in**: not a best-effort.

---

## 22) Deliverables
- Server:
  - `liquid-desktopd` (daemon)
  - `liquid-session` (user session runner)
  - `liquidctl` (admin CLI)

- Client:
  - `LiquidClient` for Windows/macOS/Linux
  - Optional web gateway

- Docs:
  - Admin guide
  - Security guide
  - Performance tuning guide
  - Troubleshooting playbook

---

## 23) Appendix — Performance profiles (defaults)
### Interactive (default)
- Target latency: lowest
- FPS: 60 when active, 2 when idle
- Codec: H.264 low-latency preset

### Balanced
- Target: good quality
- FPS: 45 active, 2 idle
- Hybrid tile/video enabled

### Bandwidth saver
- Target: minimum bandwidth
- FPS: 30 active, on-change idle
- Aggressive tile dedupe and compression

---

## 24) Open questions (answered with sensible defaults)
- Default protocol for MVP: **native QUIC protocol**.
- RDP compatibility: **post-MVP**.
- Shell toolkit: keep it **minimal and composited** (avoid heavyweight dependencies).

---

If you want, I can immediately follow this by turning it into:
- a repo scaffold (README + architecture docs + config schema)
- a wireframe UI spec (panel/launcher/windows/settings)
- a protocol spec (messages, fields, flow,

