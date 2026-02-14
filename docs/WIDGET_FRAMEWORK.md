# LiquiDE Widget Framework

## Core Trait

Every UI element implements the `Widget` trait:

```rust
pub trait Widget: Send {
    fn id(&self) -> WidgetId;
    fn layout(&mut self, constraints: Constraints) -> LayoutResult;
    fn paint(&self, painter: &mut Painter);
    fn event(&mut self, event: &Event) -> EventResult;
    fn lifecycle(&mut self) -> WidgetLifecycle;
    fn accessible(&self) -> Option<AccessibleNode> { None }
}
```

## Event System

Events flow top-down (tunneling) then bottom-up (bubbling):

```
Window receives raw input
    │
    ▼ Tunnel phase (Preview)
  Root → Parent → Child
    │
    ▼ Target phase
  Focused widget handles event
    │
    ▼ Bubble phase
  Child → Parent → Root (if not handled)
```

### Input Types

| Input   | Events |
|---------|--------|
| Pointer | Move, Enter, Leave, Down, Up, Click, DoubleClick |
| Touch   | Start, Move, End, Cancel |
| Pen     | Down, Move, Up, Pressure, Tilt, Barrel |
| Gesture | Pinch, Rotate, Swipe, LongPress |
| Keyboard| KeyDown, KeyUp, Char, IME composition |
| Focus   | FocusIn, FocusOut, FocusWithin |
| DnD     | DragEnter, DragOver, DragLeave, Drop |

## Focus Model

Tab-based focus navigation with directional (arrow key) focus for grid layouts:

- `FocusChain` — ordered list of focusable widget IDs
- `FocusDirection` — Next, Previous, Up, Down, Left, Right
- `FocusScope` — isolates focus within a subtree (dialogs, popovers)

## Widget Catalog

### Basic Controls

| Widget | Description |
|--------|-------------|
| `Button` | Push button with label, icon, variants (primary/secondary/ghost) |
| `Label` | Static text with alignment, ellipsis, wrapping |
| `Separator` | Horizontal or vertical divider |
| `Spinner` | Loading indicator (animated) |

### Input Controls

| Widget | Description |
|--------|-------------|
| `TextInput` | Single-line text field with placeholder, validation |
| `TextArea` | Multi-line text editor with scrolling |
| `Checkbox` | Boolean toggle with optional label |
| `Slider` | Continuous or stepped value slider |
| `Dropdown` | Combo box with popup selection list |

### Container Controls

| Widget | Description |
|--------|-------------|
| `ScrollView` | Scrollable viewport with inertial scrolling |
| `Splitter` | Resizable pane divider |
| `TabView` | Tabbed container with tab bar |
| `Toolbar` | Horizontal tool button strip |

### Data Controls (Virtualized)

All data controls implement viewport-based virtualization for huge datasets.
Only visible rows are instantiated — O(visible_rows) memory and paint cost.

| Widget | Description |
|--------|-------------|
| `ListView` | Virtualized scrollable list with selection |
| `TreeView` | Hierarchical tree with expand/collapse, indent |
| `TableView` | Multi-column table with sorting, resizing, selection |

### Navigation Controls

| Widget | Description |
|--------|-------------|
| `Menu` | Popup menu with items, separators, submenus |
| `MenuBar` | Application menu bar (File, Edit, View, etc.) |
| `ContextMenu` | Right-click popup menu |

### Feedback Controls

| Widget | Description |
|--------|-------------|
| `ProgressBar` | Determinate or indeterminate progress |
| `Tooltip` | Hover tooltip with delay and positioning |
| `StatusBar` | macOS-style status bar with app menu, indicators |

### Window Controls

| Widget | Description |
|--------|-------------|
| `Window` | Top-level application window |
| `TitleBar` | macOS/Qt-style traffic-light buttons |
| `WindowFrame` | Border, shadow, resize handles, glass effect |

## Virtualization Architecture

```
VirtualizedAdapter
    │
    ├── item_count() → usize
    ├── row_height(index) → f32  (estimated or exact)
    ├── create_widget(index) → Box<dyn Widget>
    └── update_widget(index, &mut dyn Widget)

VirtualizedViewport
    │
    ├── scroll_offset: f64
    ├── viewport_height: f64
    ├── first_visible: usize
    ├── last_visible: usize
    └── widget_pool: Vec<(usize, Box<dyn Widget>)>
```

The viewport maintains a recycling pool. When scrolling:
1. Remove widgets that scrolled out of view
2. Reuse recycled widgets for newly visible indices
3. Only create new widgets when the pool is empty

This ensures smooth scrolling even with 100,000+ items.

## Layout System

Constraint-based layout with flex and grid engines:

```rust
pub struct Constraints {
    pub min_width: f32,
    pub max_width: f32,
    pub min_height: f32,
    pub max_height: f32,
}

pub struct LayoutResult {
    pub width: f32,
    pub height: f32,
}
```

### Layout Modes

- **Flex** — horizontal/vertical box with spacing, alignment, grow/shrink
- **Grid** — CSS Grid-like rows/columns with spans
- **Stack** — absolute positioning with Z ordering
- **Flow** — wrapping inline layout (for text + inline widgets)
