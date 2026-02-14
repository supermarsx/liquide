# LiquiDE Rendering Pipeline Specification

## Architecture Overview

A Chrome-like CSS rendering pipeline without the JS overhead, built in Rust.
Desktop shell elements (statusbar, dock, background, window decorations) are
fully defined as DOM elements styled with CSS. Applications run sandboxed but
the base desktop is native DOM.

```
                        ┌─────────────────────────────────────────────────────────────┐
                        │                    liquide-dom                              │
                        │  ElementNode tree · Attributes · Classes · ID              │
                        │  events · parent/child · shadow subtrees                    │
                        └─────────────────┬───────────────────────────────────────────┘
                                          │
                        ┌─────────────────▼───────────────────────────────────────────┐
                        │               liquide-style-engine                          │
                        │  Selector matching (type/class/id/pseudo + combinators)     │
                        │  Cascade + specificity + inheritance + variables             │
                        │  Media queries · computed values · style sharing             │
                        │  Uses: liquide-theme-css for parsing                        │
                        └─────────────────┬───────────────────────────────────────────┘
                                          │  ComputedStyle per element
                        ┌─────────────────▼───────────────────────────────────────────┐
                        │               liquide-layout                                │
                        │  CSS box model · Block flow · Inline flow                   │
                        │  Flexbox (full spec) · Grid (full spec)                     │
                        │  Intrinsic sizing · min/max constraints                     │
                        │  Text measurement hooks · Line breaking                     │
                        │  Position: static/relative/absolute/fixed/sticky            │
                        │  Output: LayoutBox tree with resolved geometry              │
                        └─────────────────┬───────────────────────────────────────────┘
                                          │  LayoutBox tree
                        ┌─────────────────▼───────────────────────────────────────────┐
                        │               liquide-paint                                 │
                        │  Display list generation from LayoutBox + ComputedStyle     │
                        │  Borders · Shadows · Gradients · Backgrounds                │
                        │  Text painting · Image painting · Clipping                  │
                        │  Transforms · Filters · Blend modes · Masks                 │
                        │  Stacking context resolution                                │
                        └─────────────────┬───────────────────────────────────────────┘
                                          │  DisplayList
                        ┌─────────────────▼───────────────────────────────────────────┐
                        │          liquide-compositor (existing, extended)             │
                        │  SceneNode tree from DisplayList                            │
                        │  Layer splitting · Damage tracking · Compositing            │
                        │  Animation tick · Transition interpolation                  │
                        └─────────────────┬───────────────────────────────────────────┘
                                          │  FlatNode list
                        ┌─────────────┬───▼───┬───────────────────────────────────────┐
                        │ renderer-cpu│ wgpu  │  Future backends                      │
                        └─────────────┴───────┴───────────────────────────────────────┘
```

## New Crates

### 1. `liquide-dom` — Document Object Model

The DOM is the source of truth for the UI tree. Every desktop element (dock,
statusbar, window frame, notification, tooltip, menu) is a DOM element.

```rust
// Core types
pub type NodeId = u64;

pub struct Document {
    nodes: SlotMap<NodeId, Node>,
    root: NodeId,
    id_index: HashMap<String, NodeId>,        // id → node fast lookup
    class_index: HashMap<String, Vec<NodeId>>, // class → nodes
    next_id: AtomicU64,
    dirty: DirtySet,
    observers: Vec<Box<dyn MutationObserver>>,
}

pub struct Node {
    pub id: NodeId,
    pub tag: Tag,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub attrs: AttributeMap,
    pub classes: ClassList,
    pub element_id: Option<String>,       // HTML 'id' attribute
    pub pseudo_states: PseudoStateFlags,  // :hover, :focus, :active, etc.
    pub data: NodeData,
    pub style_dirty: bool,
    pub layout_dirty: bool,
}

pub enum NodeData {
    Element,                       // Generic container
    Text(String),                  // Text content
    Image { src: String, alt: String },
    Surface { surface_id: u64, buffer: Option<SurfaceBuffer> }, // sandboxed app
    Custom(Box<dyn Any + Send>),   // Extension point
}

// Tag is interned — constant-time comparison
pub struct Tag(u32); // Index into global string interner

// Attribute map optimized for small counts (inline ≤8)
pub struct AttributeMap { ... }

// Class list with fast membership test
pub struct ClassList {
    classes: SmallVec<[InternedString; 4]>,
    hash: u64,  // Bloom filter for fast negative checks
}

// Pseudo-state bitflags
bitflags! {
    pub struct PseudoStateFlags: u32 {
        const HOVER     = 0x0001;
        const FOCUS     = 0x0002;
        const ACTIVE    = 0x0004;
        const VISITED   = 0x0008;
        const DISABLED  = 0x0010;
        const CHECKED   = 0x0020;
        const FIRST_CHILD = 0x0040;
        const LAST_CHILD  = 0x0080;
        const FOCUS_WITHIN = 0x0100;
        const FOCUS_VISIBLE = 0x0200;
        const PLACEHOLDER_SHOWN = 0x0400;
        const READ_ONLY = 0x0800;
    }
}

// Dirty tracking for incremental style/layout
pub struct DirtySet {
    style_dirty: BitSet,     // Nodes needing style recalc
    layout_dirty: BitSet,    // Nodes needing layout
    paint_dirty: BitSet,     // Nodes needing repaint
    subtree_dirty: BitSet,   // Nodes whose descendants are dirty
}

// Mutation observer for reactive updates
pub trait MutationObserver: Send {
    fn on_node_added(&mut self, parent: NodeId, child: NodeId);
    fn on_node_removed(&mut self, parent: NodeId, child: NodeId);
    fn on_attribute_changed(&mut self, node: NodeId, attr: &str, old: Option<&str>, new: Option<&str>);
    fn on_class_changed(&mut self, node: NodeId, classes: &ClassList);
    fn on_text_changed(&mut self, node: NodeId, text: &str);
}

// Document API
impl Document {
    pub fn new() -> Self;
    pub fn create_element(&mut self, tag: &str) -> NodeId;
    pub fn create_text(&mut self, text: &str) -> NodeId;
    pub fn append_child(&mut self, parent: NodeId, child: NodeId);
    pub fn insert_before(&mut self, parent: NodeId, child: NodeId, before: NodeId);
    pub fn remove_child(&mut self, parent: NodeId, child: NodeId);
    pub fn set_attribute(&mut self, node: NodeId, key: &str, value: &str);
    pub fn get_attribute(&self, node: NodeId, key: &str) -> Option<&str>;
    pub fn add_class(&mut self, node: NodeId, class: &str);
    pub fn remove_class(&mut self, node: NodeId, class: &str);
    pub fn set_id(&mut self, node: NodeId, id: &str);
    pub fn set_text_content(&mut self, node: NodeId, text: &str);
    pub fn set_pseudo_state(&mut self, node: NodeId, state: PseudoStateFlags, active: bool);
    pub fn query_selector(&self, selector: &str) -> Option<NodeId>;
    pub fn query_selector_all(&self, selector: &str) -> Vec<NodeId>;
    pub fn get_element_by_id(&self, id: &str) -> Option<NodeId>;
    pub fn ancestors(&self, node: NodeId) -> AncestorIter;
    pub fn descendants(&self, node: NodeId) -> DescendantIter;
    pub fn walk<V: Visitor>(&self, node: NodeId, visitor: &mut V);
}
```

### 2. `liquide-style-engine` — Style Computation

Takes a Document + StyleSheets, computes styles per element using cascade,
specificity, inheritance, variables, and media queries.

```rust
// Selector matching with full combinator support
pub enum Combinator {
    Descendant,       // A B
    Child,            // A > B
    NextSibling,      // A + B
    SubsequentSibling, // A ~ B
}

pub struct ComplexSelector {
    compounds: Vec<CompoundSelector>,
    combinators: Vec<Combinator>,
}

pub struct CompoundSelector {
    tag: Option<InternedString>,
    id: Option<InternedString>,
    classes: Vec<InternedString>,
    pseudo_classes: Vec<PseudoClass>,
    pseudo_element: Option<PseudoElement>,
    attributes: Vec<AttributeSelector>,
}

pub enum PseudoClass {
    Hover, Focus, Active, Visited, Disabled, Checked,
    FirstChild, LastChild, NthChild(AnB), NthLastChild(AnB),
    Not(Box<ComplexSelector>),
    FocusWithin, FocusVisible,
    PlaceholderShown, ReadOnly, ReadWrite,
    Root, Empty,
}

pub struct AnB { pub a: i32, pub b: i32 } // :nth-child(an+b)

// Specificity as (inline, id_count, class_count, type_count)
#[derive(Ord, PartialOrd, Eq, PartialEq)]
pub struct Specificity(u32, u32, u32);

// ComputedStyle — the fully resolved style of an element after cascade +
//                 inheritance + variable substitution
pub struct ComputedStyle {
    // Box model
    pub display: Display,
    pub position: Position,
    pub box_sizing: BoxSizing,
    pub width: Dimension,
    pub height: Dimension,
    pub min_width: Dimension,
    pub max_width: Dimension,
    pub min_height: Dimension,
    pub max_height: Dimension,
    pub margin: Sides<Dimension>,
    pub padding: Sides<Dimension>,
    pub border_width: Sides<f32>,
    pub border_style: Sides<BorderLineStyle>,
    pub border_color: Sides<Color>,
    pub border_radius: Corners<f32>,

    // Flexbox
    pub flex_direction: FlexDirection,
    pub flex_wrap: FlexWrap,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub align_self: AlignSelf,
    pub align_content: AlignContent,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: Dimension,
    pub gap: Size<Dimension>,
    pub order: i32,

    // Grid
    pub grid_template_columns: Vec<TrackSize>,
    pub grid_template_rows: Vec<TrackSize>,
    pub grid_column: GridPlacement,
    pub grid_row: GridPlacement,
    pub grid_auto_flow: GridAutoFlow,
    pub grid_auto_columns: TrackSize,
    pub grid_auto_rows: TrackSize,

    // Positioning
    pub top: Dimension,
    pub right: Dimension,
    pub bottom: Dimension,
    pub left: Dimension,
    pub z_index: Option<i32>,

    // Typography
    pub color: Color,
    pub font_family: Vec<String>,
    pub font_size: f32,
    pub font_weight: u16,
    pub font_style: FontStyle,
    pub line_height: LineHeight,
    pub letter_spacing: f32,
    pub word_spacing: f32,
    pub text_align: TextAlign,
    pub text_decoration: TextDecoration,
    pub text_transform: TextTransform,
    pub text_overflow: TextOverflow,
    pub text_shadow: Vec<TextShadowSpec>,
    pub white_space: WhiteSpace,
    pub word_break: WordBreak,
    pub text_indent: f32,

    // Visual
    pub background: BackgroundSpec,
    pub box_shadow: Vec<BoxShadowSpec>,
    pub opacity: f32,
    pub visibility: Visibility,
    pub overflow_x: Overflow,
    pub overflow_y: Overflow,
    pub cursor: Cursor,
    pub pointer_events: PointerEvents,

    // Effects
    pub transform: Vec<Transform>,
    pub transform_origin: TransformOrigin,
    pub filter: Vec<FilterFunction>,
    pub backdrop_filter: Vec<FilterFunction>,
    pub mix_blend_mode: BlendMode,
    pub isolation: Isolation,
    pub clip_path: Option<ClipPath>,
    pub mask: Option<MaskSpec>,
    pub outline: Option<OutlineSpec>,

    // Transitions & animations
    pub transition: Vec<TransitionDef>,
    pub animation: Vec<AnimationDef>,

    // Generated content
    pub content: Option<ContentValue>,
}

// Resolution dimensions
pub enum Dimension {
    Px(f32),
    Percent(f32),
    Em(f32),
    Rem(f32),
    Vw(f32),
    Vh(f32),
    Vmin(f32),
    Vmax(f32),
    Ch(f32),
    Auto,
    MinContent,
    MaxContent,
    FitContent(Box<Dimension>),
    Calc(CssCalc),
    None,   // for max-width: none
}

// Style engine
pub struct StyleEngine {
    sheets: Vec<PreparedSheet>,         // Compiled rule sets
    inherited_properties: PropertySet,  // Which properties inherit
    initial_values: ComputedStyle,      // CSS initial values
    viewport: ViewportSize,
    base_font_size: f32,                // For rem
    // Bloom filter for fast selector rejection
    ancestor_filter: BloomFilter,
}

pub struct PreparedSheet {
    rules: Vec<PreparedRule>,
}

pub struct PreparedRule {
    selector: ComplexSelector,
    specificity: Specificity,
    source_order: u32,
    declarations: Vec<Declaration>,
}

impl StyleEngine {
    pub fn new(viewport: ViewportSize, base_font_size: f32) -> Self;
    pub fn add_stylesheet(&mut self, css: &str);
    pub fn compute_style(&self, doc: &Document, node: NodeId) -> ComputedStyle;
    pub fn restyle_subtree(&self, doc: &Document, node: NodeId, styles: &mut StyleMap);
    pub fn invalidate(&mut self, doc: &Document, changes: &[Mutation]);
    pub fn set_viewport(&mut self, size: ViewportSize);
    pub fn resolve_variable(&self, name: &str, fallback: &str) -> PropertyValue;
}

// Style sharing — elements with identical selectors share computed styles
pub struct StyleMap {
    styles: HashMap<NodeId, Arc<ComputedStyle>>,
    sharing_cache: StyleSharingCache,
}
```

### 3. `liquide-layout` — CSS Box Layout Engine

Full CSS layout: block, inline, flex, grid, positioned.

```rust
// Layout input
pub struct LayoutInput<'a> {
    pub doc: &'a Document,
    pub styles: &'a StyleMap,
    pub text_measurer: &'a dyn TextMeasurer,
    pub image_measurer: &'a dyn ImageMeasurer,
    pub viewport: Size,
}

// Text measurement hook (implemented by text engine)
pub trait TextMeasurer {
    fn measure(&self, text: &str, style: &TextStyle, max_width: Option<f32>) -> TextMetrics;
    fn line_height(&self, style: &TextStyle) -> f32;
    fn baseline(&self, style: &TextStyle) -> f32;
}

pub struct TextMetrics {
    pub width: f32,
    pub height: f32,
    pub baseline: f32,
    pub line_count: u32,
    pub lines: Vec<LineMetrics>,
}

pub struct LineMetrics {
    pub width: f32,
    pub baseline: f32,
    pub ascent: f32,
    pub descent: f32,
}

pub trait ImageMeasurer {
    fn intrinsic_size(&self, src: &str) -> Option<Size>;
}

// Layout output — positioned box tree
pub struct LayoutTree {
    boxes: Vec<LayoutBox>,
    root: LayoutBoxId,
}

pub type LayoutBoxId = usize;

pub struct LayoutBox {
    pub id: LayoutBoxId,
    pub node: NodeId,                   // Back-reference to DOM node
    pub box_type: BoxType,
    pub content_rect: Rect,             // Content area
    pub padding_rect: Rect,             // Content + padding
    pub border_rect: Rect,              // Content + padding + border
    pub margin_rect: Rect,              // Content + padding + border + margin
    pub children: Vec<LayoutBoxId>,
    pub baseline: Option<f32>,          // First baseline for flex alignment
    pub scroll_size: Option<Size>,      // Scrollable content size
}

pub enum BoxType {
    Block,
    Inline,
    InlineBlock,
    Flex,
    FlexItem,
    Grid,
    GridItem,
    Text { line_boxes: Vec<LineBox> },
    Replaced,           // Images, surfaces
    Absolute,           // Absolutely positioned
    Fixed,              // Fixed positioned
}

pub struct LineBox {
    pub range: Range<usize>,    // Glyph range
    pub rect: Rect,
    pub baseline: f32,
}

// Layout engine
pub struct LayoutEngine {
    cache: LayoutCache,
}

impl LayoutEngine {
    pub fn new() -> Self;
    pub fn layout(&mut self, input: &LayoutInput) -> LayoutTree;
    pub fn relayout_subtree(&mut self, input: &LayoutInput, node: NodeId, tree: &mut LayoutTree);
    pub fn invalidate(&mut self, node: NodeId);
}

// Internal layout algorithms
mod block;     // Block formatting context
mod inline;    // Inline formatting context
mod flex;      // Flexbox (CSS Flexbox Level 1)
mod grid;      // Grid (CSS Grid Level 1)
mod positioned; // Absolute/fixed/sticky positioning
mod intrinsic; // Intrinsic sizing (min-content, max-content, fit-content)
mod margin_collapse; // Block margin collapsing
mod float;     // CSS floats (simplified)
```

### 4. `liquide-paint` — Display List Generator

Converts LayoutBox + ComputedStyle into a display list / paint commands.

```rust
pub struct DisplayList {
    items: Vec<DisplayItem>,
}

pub enum DisplayItem {
    // Backgrounds
    SolidColor { rect: Rect, color: Color, radius: Corners<f32> },
    Gradient { rect: Rect, gradient: GradientSpec, radius: Corners<f32> },
    Image { rect: Rect, image_id: u64, fit: ImageFit, radius: Corners<f32> },

    // Borders
    Border { rect: Rect, sides: Sides<BorderSide>, radius: Corners<f32> },
    Outline { rect: Rect, outline: OutlineSide },

    // Shadows
    BoxShadow { rect: Rect, shadow: BoxShadowSpec, radius: Corners<f32> },

    // Text
    Text {
        rect: Rect,
        glyphs: Vec<PositionedGlyph>,
        color: Color,
        decorations: Vec<TextDecorationPaint>,
        shadows: Vec<TextShadowSpec>,
    },

    // Effects
    PushClip { rect: Rect, radius: Corners<f32> },
    PopClip,
    PushOpacity { opacity: f32 },
    PopOpacity,
    PushTransform { transform: Affine2D },
    PopTransform,
    PushBlendMode { mode: BlendMode },
    PopBlendMode,
    PushFilter { filters: Vec<FilterFunction> },
    PopFilter,
    PushMask { mask: MaskSpec },
    PopMask,

    // Stacking context marker
    PushStackingContext { z_index: i32, isolation: Isolation },
    PopStackingContext,

    // External surface (sandboxed application)
    Surface { rect: Rect, surface_id: u64, buffer: Option<SurfaceBuffer> },
}

pub struct Painter {
    display_list: DisplayList,
    stacking_contexts: Vec<StackingContext>,
}

struct StackingContext {
    z_index: i32,
    items: Vec<(DisplayItem, usize)>,  // (item, order)
}

impl Painter {
    pub fn new() -> Self;
    pub fn paint(&mut self, layout: &LayoutTree, styles: &StyleMap, doc: &Document) -> DisplayList;
}

// Converts DisplayList → SceneNode tree for compositor
pub fn display_list_to_scene(list: &DisplayList) -> SceneNode;
```

### 5. `liquide-hit-test` — Hit Testing & Input Routing

Processes input events against the layout tree for CSS-aware event dispatch.

```rust
pub struct HitTestEngine {
    layout: Arc<LayoutTree>,
    styles: Arc<StyleMap>,
}

pub struct HitTestResult {
    pub node: NodeId,
    pub point_in_node: Point,  // Coordinates relative to the node
    pub ancestors: Vec<NodeId>, // Bubble path
}

impl HitTestEngine {
    pub fn hit_test(&self, point: Point) -> Option<HitTestResult>;
    pub fn hit_test_all(&self, point: Point) -> Vec<HitTestResult>; // All overlapping
    pub fn update_layout(&mut self, layout: Arc<LayoutTree>, styles: Arc<StyleMap>);
}

// Event dispatch
pub struct EventDispatcher {
    doc: Arc<RwLock<Document>>,
    hit_test: HitTestEngine,
    focus_manager: FocusManager,
    hover_chain: Vec<NodeId>,
}

impl EventDispatcher {
    pub fn dispatch_mouse(&mut self, event: MouseEvent) -> Vec<DomEvent>;
    pub fn dispatch_keyboard(&mut self, event: KeyEvent) -> Vec<DomEvent>;
    pub fn dispatch_scroll(&mut self, event: ScrollEvent) -> Vec<DomEvent>;
    pub fn update_hover(&mut self, pos: Point); // :hover state management
}

pub struct DomEvent {
    pub target: NodeId,
    pub kind: DomEventKind,
    pub propagation: Propagation,
}

pub enum DomEventKind {
    MouseDown { button: MouseButton, x: f32, y: f32 },
    MouseUp { button: MouseButton, x: f32, y: f32 },
    MouseMove { x: f32, y: f32 },
    Click { button: MouseButton, x: f32, y: f32 },
    DoubleClick { x: f32, y: f32 },
    MouseEnter,
    MouseLeave,
    KeyDown { key: KeyCode, modifiers: Modifiers },
    KeyUp { key: KeyCode, modifiers: Modifiers },
    Focus,
    Blur,
    Scroll { dx: f32, dy: f32 },
    Input { value: String },
    // IME events
    CompositionStart,
    CompositionUpdate { text: String, cursor: usize },
    CompositionEnd { text: String },
}

pub enum Propagation {
    Continue,
    StopPropagation,
    StopImmediate,
    PreventDefault,
}
```

## Integration: The Frame Pipeline

One frame looks like this:

```
1. Input events arrive
   └─► EventDispatcher.dispatch() → DOM mutations (:hover, :focus, attribute changes)

2. Animation/Transition tick
   └─► AnimationScheduler.tick(dt) → style property updates on DOM nodes

3. Style recalculation (incremental)
   └─► StyleEngine.restyle_subtree(dirty_nodes) → updated ComputedStyle per node

4. Layout (incremental)
   └─► LayoutEngine.relayout_subtree(dirty_nodes) → updated LayoutBox geometry

5. Paint
   └─► Painter.paint(layout, styles) → DisplayList

6. Scene build
   └─► display_list_to_scene(list) → SceneNode tree

7. Composite (existing)
   └─► Compositor.submit_scene(scene) → FlatNode list + damage rects

8. Render (existing)
   └─► renderer_cpu.render(flat_nodes) OR renderer_wgpu.render(flat_nodes)
```

## Desktop Shell as DOM

```rust
// Bootstrap the desktop
fn init_desktop(doc: &mut Document) {
    let root = doc.root();

    // Background
    let bg = doc.create_element("desktop-background");
    doc.append_child(root, bg);

    // Workspace container
    let ws = doc.create_element("workspace-container");
    doc.append_child(root, ws);

    // Window layer
    let windows = doc.create_element("window-layer");
    doc.append_child(ws, windows);

    // Statusbar
    let bar = doc.create_element("statusbar");
    doc.append_child(root, bar);
    let clock = doc.create_element("statusbar-clock");
    doc.set_text_content(clock, "12:00");
    doc.append_child(bar, clock);

    // Dock
    let dock = doc.create_element("dock");
    doc.add_class(dock, "glass");
    doc.append_child(root, dock);
}
```

Styled with CSS:
```css
desktop-background {
    width: 100vw; height: 100vh;
    background: var(--wallpaper-url) center/cover no-repeat;
}

statusbar {
    display: flex;
    position: fixed;
    top: 0; left: 0; right: 0;
    height: 28px;
    backdrop-filter: blur(20px);
    background: rgba(0,0,0,0.4);
    z-index: 1000;
}

dock {
    display: flex;
    position: fixed;
    bottom: 8px;
    left: 50%; transform: translateX(-50%);
    gap: 4px;
    padding: 4px 8px;
    backdrop-filter: blur(30px) saturate(180%);
    background: rgba(255,255,255,0.15);
    border-radius: 16px;
    border: 1px solid rgba(255,255,255,0.2);
    box-shadow: 0 8px 32px rgba(0,0,0,0.3);
}

dock-item {
    width: 48px; height: 48px;
    border-radius: 12px;
    transition: transform 0.15s ease;
}

dock-item:hover {
    transform: scale(1.2) translateY(-4px);
}
```

## Window Manager Integration

Windows are DOM elements within the window layer:

```css
window {
    position: absolute;
    display: flex;
    flex-direction: column;
    border-radius: 10px;
    overflow: hidden;
    box-shadow: 0 12px 40px rgba(0,0,0,0.3);
}

window-titlebar {
    display: flex;
    align-items: center;
    height: 32px;
    padding: 0 12px;
    backdrop-filter: blur(20px);
    user-select: none;
    cursor: default;
}

window-content {
    flex: 1;
    overflow: auto;
}
```

## Sandbox Boundary

Desktop chrome (dock, statusbar, decorations) runs in the same DOM.
Application content is a Surface node — the app renders into a buffer
that is composited via `NodeData::Surface { surface_id, buffer }`.
No DOM access is given to sandboxed applications; they communicate
via the protocol layer (liquide-protocol).

Plugin/extension interop is via WASM containers using the existing
`liquide-plugin-abi` (8 extension points). Plugins receive typed
events and return typed commands — no raw DOM manipulation.

## Supported CSS Subset

### Selectors
- Type: `window`, `dock`, `statusbar`
- Class: `.active`, `.glass`, `.dark`
- ID: `#main-dock`
- Pseudo-class: `:hover`, `:focus`, `:active`, `:disabled`, `:checked`,
  `:first-child`, `:last-child`, `:nth-child(an+b)`, `:not()`,
  `:focus-within`, `:focus-visible`, `:empty`, `:root`
- Pseudo-element: `::before`, `::after`, `::placeholder`, `::selection`
- Combinators: descendant (` `), child (`>`), next-sibling (`+`), subsequent-sibling (`~`)

### Box Model
- `display`: block, inline, inline-block, flex, inline-flex, grid, inline-grid, none, contents
- `position`: static, relative, absolute, fixed, sticky
- `width`/`height`/`min-*`/`max-*`: px, %, em, rem, vw, vh, vmin, vmax,
  auto, min-content, max-content, fit-content, calc()
- `margin`/`padding`: all sides, shorthand
- `box-sizing`: content-box, border-box
- `overflow`: visible, hidden, scroll, auto

### Flexbox (Complete)
- `flex-direction`, `flex-wrap`, `justify-content`, `align-items`,
  `align-self`, `align-content`, `flex-grow`, `flex-shrink`, `flex-basis`,
  `gap`, `order`

### Grid (Complete)
- `grid-template-columns`/`rows`, `grid-column`/`row`,
  `grid-auto-flow`, `grid-auto-columns`/`rows`, `grid-gap`
- Track sizing: px, %, fr, min-content, max-content, minmax(), repeat()

### Typography
- `font-family`, `font-size`, `font-weight`, `font-style`
- `line-height`, `letter-spacing`, `word-spacing`
- `text-align`, `text-indent`, `text-transform`, `text-overflow`
- `text-decoration` (line, style, color)
- `text-shadow`
- `white-space`, `word-break`, `overflow-wrap`

### Visual
- `color`, `background` (color, image, gradient, size, position, repeat)
- `border` (width, style, color, radius)
- `box-shadow` (multiple, inset)
- `outline`
- `opacity`
- `visibility`
- `cursor`, `pointer-events`, `user-select`

### Effects
- `transform`: translate, scale, rotate, skew, matrix
- `transform-origin`
- `filter`: blur, brightness, contrast, grayscale, sepia, invert,
  saturate, hue-rotate, drop-shadow, opacity
- `backdrop-filter`: same as filter
- `mix-blend-mode`
- `isolation`
- `clip-path`
- `mask`/`mask-image`

### Animations & Transitions
- `transition`: property, duration, timing-function, delay
- `animation`: name, duration, timing-function, delay, iteration-count,
  direction, fill-mode, play-state
- `@keyframes`
- Timing: linear, ease, ease-in, ease-out, ease-in-out,
  cubic-bezier(), steps()

### At-Rules
- `@media` (width, height, color-scheme, prefers-reduced-motion)
- `@keyframes`
- `@font-face`
- `@import`
- CSS custom properties (`--var: value`, `var(--var, fallback)`)
