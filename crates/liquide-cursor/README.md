# liquide-cursor

Comprehensive cursor management for the Liquide desktop environment.

## Features

- **27 Standard Cursor Shapes** - Complete set covering all desktop use cases
- **Custom Cursor Images** - Load RGBA images with hotspot positioning
- **Cursor Themes** - Support for cursor theme packages
- **State Tracking** - Position, visibility, and shape management
- **Software Rendering** - Built-in cursor renderer with alpha blending
- **Animated Cursors** - Multi-frame animated cursors with timing
- **Serialization** - Full serde support for protocol transmission

## Usage

### Basic Cursor State

```rust
use liquide_cursor::{CursorState, CursorShape, ResizeDirection};

// Create cursor at position
let mut cursor = CursorState::new(100.0, 200.0);

// Set shape
cursor.set_shape(CursorShape::Pointer);

// Set resize cursor
cursor.set_shape(CursorShape::Resize(ResizeDirection::NorthWest));

// Hide/show
cursor.hide();
cursor.show();
```

### Custom Cursor Images

```rust
use liquide_cursor::CursorState;

let mut cursor = CursorState::default();

// Load custom cursor image (32x32 RGBA8)
let image_data = vec![0u8; 32 * 32 * 4];
cursor.set_custom_image(
    1,              // cursor ID
    image_data,
    32,             // width
    32,             // height
    16,             // hotspot X
    16,             // hotspot Y
)?;
```

### Cursor Rendering

```rust
use liquide_cursor::{CursorState, SoftwareCursorRenderer, CursorRenderer, RenderTarget};

let renderer = SoftwareCursorRenderer::new();
let cursor = CursorState::new(100.0, 100.0);

// Render to RGBA8 framebuffer
let target = RenderTarget::Rgba8 {
    pixels: &mut framebuffer,
    width: 1920,
    height: 1080,
    stride: 1920 * 4,
};

renderer.render(&cursor, target)?;
```

### Animated Cursors

```rust
use liquide_cursor::{AnimatedCursor, AnimatedCursorBuilder, CursorShape};

// Build animated cursor with multiple frames
let animated = AnimatedCursorBuilder::new(1, CursorShape::Wait)
    .add_frame(frame1_data, 32, 32, 16, 16, 100)  // 100ms duration
    .add_frame(frame2_data, 32, 32, 16, 16, 100)
    .add_frame(frame3_data, 32, 32, 16, 16, 100)
    .build();

// Update animation (call each frame)
if animated.update(delta_ms) {
    // Frame changed, apply to cursor state
    animated.apply_to_state(&mut cursor_state);
}
```

### Cursor Themes

```rust
use liquide_cursor::CursorTheme;

// Load theme from directory
let mut theme = CursorTheme::load("/usr/share/cursors/Adwaita")?;

// Get cursor image for specific shape and size
if let Some(cursor_image) = theme.get_cursor(CursorShape::Pointer, 24) {
    // Use cursor image...
}

// Use default theme
let default_theme = liquide_cursor::default_theme();
```

## Cursor Shapes

### Standard Shapes
- `Arrow` - Default pointer
- `Pointer` - Hand/clickable items
- `Text` - I-beam for text editing
- `Move` - Four-way movement
- `Wait` - Busy/loading
- `Progress` - Background operation
- `Crosshair` - Precise selection
- `Help` - Context help available
- `NotAllowed` - Invalid action
- `Grab` / `Grabbing` - Pan/drag
- `ZoomIn` / `ZoomOut` - Magnification
- `ContextMenu` - Right-click available
- `Alias` - Shortcut/link
- `Copy` - Copy operation
- `NoDrop` - Invalid drop zone
- `Cell` - Spreadsheet cell selection
- `VerticalText` - Vertical text editing
- `AllScroll` - Omnidirectional scrolling
- `ColResize` / `RowResize` - Table resizing

### Resize Cursors
Use `CursorShape::Resize(direction)` with:
- `ResizeDirection::North` / `South` - Vertical resize
- `ResizeDirection::East` / `West` - Horizontal resize
- `ResizeDirection::NorthEast` / etc. - Diagonal resize

### Custom Cursors
`CursorShape::Custom { id }` - Custom image-based cursor

### Hidden
`CursorShape::Hidden` - Invisible cursor

## Architecture

The cursor system is designed to be:
- **Portable** - Works on any platform
- **Efficient** - Minimal overhead for cursor updates
- **Extensible** - Easy to add new cursor shapes
- **Protocol-aware** - Serializable for client-server communication

## Migration from Legacy Code

Old code using `ResizeNS`, `ResizeEW`, `ResizeNWSE`, `ResizeNESW`:

```rust
// Old
CursorShape::ResizeNS

// New
CursorShape::Resize(ResizeDirection::North)  // or South
```

Old code using `ExpandH`, `ExpandV`:

```rust
// Old
CursorShape::ExpandH
CursorShape::ExpandV

// New
CursorShape::ColResize
CursorShape::RowResize
```

## License

MIT
