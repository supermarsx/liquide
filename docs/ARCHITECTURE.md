# LiquiDE Architecture

## Overview

LiquiDE is a modular desktop environment / remote desktop protocol stack written
in pure Rust. It comprises 60+ crates organized into layers:

```
┌─────────────────────────────────────────────────────────────┐
│  Apps (files, settings, terminal, text-editor, task-manager)│
├─────────────────────────────────────────────────────────────┤
│  Bridges (GTK ↔ LiquiDE, Qt ↔ LiquiDE)                    │
├─────────────────────────────────────────────────────────────┤
│  Shell (windows, dock, status bar, launcher, context menu)  │
├─────────────────────────────────────────────────────────────┤
│  UI Toolkit (widgets, layout, animation, theming)           │
│  ┌──────────┬──────────┬────────┬──────────┬───────────┐   │
│  │ ui-core  │ widgets  │ window │ tooltip  │ statusbar │   │
│  └──────────┴──────────┴────────┴──────────┴───────────┘   │
├─────────────────────────────────────────────────────────────┤
│  Text Engine (shaping, rasterization, bidi, editing)        │
├─────────────────────────────────────────────────────────────┤
│  Compositor (scene graph, damage tracking, effects budget)  │
├─────────────────────────────────────────────────────────────┤
│  Renderers (CPU, GPU) + Render Coordinator (threaded)       │
├─────────────────────────────────────────────────────────────┤
│  Fonts · Cursors · CSS Engine · Theme Engine                │
├─────────────────────────────────────────────────────────────┤
│  Platform (input, clipboard, DnD, IME, accessibility)       │
├─────────────────────────────────────────────────────────────┤
│  Transport (TCP/UDP/QUIC/WebSocket, encoding, protocol)     │
├─────────────────────────────────────────────────────────────┤
│  Infrastructure (auth, policy, gateway, manager, supervisor)│
└─────────────────────────────────────────────────────────────┘
```

## Threading Model

The rendering pipeline separates *window chrome* from *content* to prevent
application hangs from freezing the desktop:

```
Main Thread
├── Event dispatch, focus management, layout
│
├──> Chrome Thread (per window)
│    └── Title bar, decorations, resize handle rendering
│
├──> Content Thread (per window)
│    └── Widget paint, text layout, application rendering
│
├──> Dock Thread
│    └── Dock/taskbar
│
├──> Status Bar Thread
│    └── System indicators, clock, notifications
│
└──> Wallpaper Thread
     └── Background, blur cache
```

Each thread owns a private `FrameBuffer` slice. The coordinator merges
dirty regions into the final composite buffer and ships tiles to the encoder.

## Scene Graph

The compositor maintains a retained scene graph (not immediate mode). Each
frame, the shell mutates the tree (move a window, change opacity, etc.) and
the compositor diffs the new tree against the previous via CRC-32C tile
hashing to produce a `DamageSet`.

```
SceneNode (tree)         ──flatten──►  FlatNode[] (z-sorted)
  ├ Root                                  │
  ├ Background                            ├── BackdropBlur
  ├ Workspace[0]                          ├── Glass
  │  ├ Window(id=42)                      ├── Shadow
  │  │  ├ Decoration                      ├── WindowDecoration
  │  │  ├ Shadow                          ├── SurfaceContent
  │  │  └ Content(Surface)                ├── Text
  │  └ Window(id=43)                      └── Cursor
  ├ StatusBar                      
  ├ Dock                                DamageTracker
  └ Cursor                            ├── previous_hashes[]
                                      ├── current_hashes[]
                                      └── changed_tiles → DamageSet
```

## Rendering Pipeline

```
SceneNode tree
    │
    ▼
Flatten + Z-sort → FlatNode[]
    │
    ▼
DamageTracker → DamageSet (changed tiles only)
    │
    ▼
Renderer (CPU or GPU)
  ├── fill_rect, rounded_rect, circle
  ├── gradients (linear, radial, conic, mesh)
  ├── path rasterizer (Bézier, 4x AA)
  ├── blur (separable Gaussian, fast downsampled)
  ├── shadows (SDF box shadow, inner glow)
  ├── image blit (bilinear scaling)
  ├── text (glyph atlas, subpixel rendering)
  └── effects (backdrop blur, glass)
    │
    ▼
FrameBuffer (BGRA8)
    │
    ▼
Encoder (LZ4/Zstd, XOR delta, tile compression)
    │
    ▼
Transport (QUIC/TCP → client)
```

## Text Engine

The text engine provides a complete stack from raw Unicode text to rendered
glyphs:

```
Input text (UTF-8)
    │
    ▼
Script detection (UAX #24)
    │
    ▼
BiDi algorithm (UAX #9) → resolved levels → visual reordering
    │
    ▼
Itemization (script runs × font × direction)
    │
    ▼
Shaping (OpenType GSUB/GPOS via HarfBuzz or built-in)
    │
    ▼
Line breaking (UAX #14) + word wrap
    │
    ▼
Paragraph layout (glyph positioning, alignment, spacing)
    │
    ▼
Rasterization (FreeType on Linux, DirectWrite on Windows, CoreText on macOS)
    │
    ▼
Glyph atlas → subpixel compositing → frame buffer
```

### Selection & Editing

- **Caret model**: logical position (byte offset) + affinity (upstream/downstream)
- **Selection**: anchor + focus with visual highlighting
- **Hit testing**: pixel (x, y) → character index
- **Editing model**: plain text (gap buffer) with intent to extend to rich text
- **IME integration**: composition string, candidate window placement

## Widget Framework

Inspired by Qt and GTK, the widget framework uses a `Widget` trait with:

- `layout(constraints) → LayoutResult` — constraint-based sizing
- `paint(painter)` — deferred paint commands
- `event(event) → EventResult` — input routing
- `lifecycle()` — mount/unmount/update

### Available Widgets

| Category   | Widgets |
|-----------|---------|
| Basic      | Button, Label, Separator, Spinner |
| Input      | TextInput, TextArea, Checkbox, Slider, Dropdown |
| Container  | ScrollView, Splitter, TabView, Toolbar |
| Data       | ListView (virtualized), TreeView, TableView |
| Navigation | Menu, MenuBar, ContextMenu |
| Feedback   | ProgressBar, Tooltip, StatusBar |
| Window     | Window, TitleBar, WindowFrame |

### Virtualization

`ListView`, `TreeView`, and `TableView` implement viewport-based virtualization:
only visible rows are instantiated. A `VirtualizedAdapter` provides item count,
row height estimation, and on-demand widget creation. This allows 100k+ items
with O(visible) memory.

## Bridges

### GTK Bridge

Maps LiquiDE widgets ↔ GTK4 widgets so GTK apps can render inside LiquiDE
windows and LiquiDE apps can embed GTK content:

- `GtkWindowBridge` — top-level window ↔ `GtkWindow`
- `GtkWidgetAdapter` — maps LiquiDE events to GDK events
- `GtkClipboardBridge` — unified clipboard

### Qt Bridge

Maps LiquiDE widgets ↔ Qt6 QWidgets:

- `QtWindowBridge` — top-level window ↔ `QMainWindow`
- `QtWidgetAdapter` — maps LiquiDE events to `QEvent`
- `QtClipboardBridge` — unified clipboard

Both bridges enable seamless interop: a GTK/Qt app can run as a native
LiquiDE window, receiving input and rendering through the LiquiDE compositor.

## Accessibility

The `liquide-a11y` crate maintains an accessibility tree parallel to the
widget tree. Each widget produces `AccessibleNode` entries with:

- Role (Button, TextInput, List, TreeItem, etc.)
- State (focused, checked, expanded, etc.)
- Name, description, value
- Actions (click, toggle, expand)
- Relations (labelled-by, described-by)

Platform bridges expose the tree via AT-SPI (Linux) or UIA (Windows).

## Platform Integration

| Feature     | Crate | Status |
|------------|-------|--------|
| Clipboard   | `liquide-clipboard` | Format negotiation, chunked transfer |
| Drag & Drop | `liquide-dnd` | Drag source, drop target, data transfer |
| IME         | `liquide-ime` | Composition, candidates, platform backend |
| Input       | `liquide-input` | Keyboard, mouse, touch, pen, gestures |
| Cursor      | `liquide-cursor` | 27 shapes, themed generation, vector cursors |
| Fonts       | `liquide-fonts` | Discovery, Google Fonts, hot-reload, roles |
