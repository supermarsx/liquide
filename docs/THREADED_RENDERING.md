# LiquiDE Threaded Rendering

## Design Principles

1. **Isolation**: If an application hangs, the window chrome (title bar,
   decorations, resize handles) must remain responsive.
2. **Parallelism**: Multiple windows render concurrently on separate threads.
3. **Damage-driven**: Only changed regions are re-rendered and transferred.
4. **Budget-aware**: Effects degrade gracefully under time pressure.

## Thread Architecture

```
                    ┌─────────────┐
                    │ Main Thread  │
                    │  (Event Loop)│
                    └──────┬──────┘
                           │ Scene mutations
                    ┌──────▼──────┐
                    │  Coordinator │
                    │  (Scheduler) │
                    └──────┬──────┘
           ┌───────┬───────┼───────┬──────────┐
           ▼       ▼       ▼       ▼          ▼
       ┌───────┐ ┌──────┐ ┌──────┐ ┌───────┐ ┌─────────┐
       │Chrome │ │Content│ │ Dock │ │Status │ │Wallpaper│
       │Thread │ │Thread │ │Thread│ │Thread │ │Thread   │
       │(per W)│ │(per W)│ │      │ │       │ │         │
       └───┬───┘ └───┬───┘ └──┬───┘ └───┬───┘ └────┬────┘
           │         │        │         │           │
           └─────────┴────────┴─────────┴───────────┘
                              │
                    ┌─────────▼─────────┐
                    │   Frame Merger    │
                    │ (Damage Assembly) │
                    └─────────┬─────────┘
                              │
                    ┌─────────▼─────────┐
                    │    Encoder        │
                    │ (LZ4/Zstd tiles)  │
                    └─────────┬─────────┘
                              │
                    ┌─────────▼─────────┐
                    │    Transport      │
                    │ (QUIC/TCP → Client)│
                    └───────────────────┘
```

## Chrome vs Content Separation

Each window has TWO render threads:

### Chrome Thread
- Renders: title bar, minimize/maximize/close buttons, borders, resize handles,
  shadow, glass effect
- Updates when: window moves, resizes, focus changes, button hover
- Latency: Must respond within 16ms for smooth interaction
- Never blocked by application content

### Content Thread
- Renders: application widget tree, text, images, custom drawing
- Updates when: application state changes, user input causes repaints
- May block: on complex layouts, slow data fetching, heavy computation
- Isolation: if this hangs, chrome thread keeps running

### Communication

```rust
pub struct WindowRenderMessage {
    pub window_id: WindowId,
    pub kind: RenderMessageKind,
}

pub enum RenderMessageKind {
    /// Chrome needs repaint (focus change, button hover, resize)
    ChromeInvalidate { region: Rect },
    /// Content needs repaint (widget state change)
    ContentInvalidate { region: Rect },
    /// Window moved — chrome + content both need update
    WindowMoved { new_bounds: Rect },
    /// Window resized — re-layout + full repaint
    WindowResized { new_size: Size },
    /// Window closed — tear down both threads
    WindowClosed,
}
```

## Frame Budget

Target: 60 FPS → 16.67ms per frame

| Phase | Budget |
|-------|--------|
| Scene diff | 0.5ms |
| Damage compute | 0.5ms |
| Chrome render | 2ms |
| Content render | 8ms |
| Merge + encode | 3ms |
| Transport | 2ms |
| **Total** | **16ms** |

The `DegradationController` monitors actual timings and escalates through
degradation levels when budgets are exceeded.

## Synchronization

Threads communicate via lock-free channels (`crossbeam-channel`):

```
Event Loop ──(mpsc)──► Chrome Thread ──(oneshot)──► Frame Merger
Event Loop ──(mpsc)──► Content Thread ──(oneshot)──► Frame Merger
```

The Frame Merger holds double-buffered composites:
- **Front buffer**: currently being encoded/transmitted
- **Back buffer**: currently being rendered into

Swap happens atomically at vsync boundaries.
