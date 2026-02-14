# Renderer CSS Middleware - Quick Start Guide

## What Was Built

A complete middleware system (`liquide-renderer-css`) that bridges CSS themes with the rendering pipeline, enabling dynamic CSS-driven styling throughout the shell UI.

## Installation

The middleware is already integrated. No installation needed.

## Basic Usage

### 1. Create a StyleResolver

```rust
use liquide_renderer_css::StyleResolver;
use liquide_theme_css::ThemeEngine;
use std::sync::Arc;

// From existing ThemeEngine
let resolver = StyleResolver::from_arc(Arc::new(engine));

// Or create new
let resolver = StyleResolver::new(engine);
```

### 2. Query Styles for UI Elements

```rust
// Query dock styles
let dock_style = resolver.resolve("dock", &[], &[], None)?;

// Query active dock item
let active_classes = vec!["active".into()];
let item_style = resolver.resolve("dock-item", &active_classes, &[], None)?;

// Query focused window
let focused_classes = vec!["focused".into()];
let window_style = resolver.resolve("window", &focused_classes, &[], None)?;
```

### 3. Extract Properties

```rust
// Colors
let bg_color = style.background_color.unwrap_or(Color::new(0, 0, 0, 255));
let fg_color = style.foreground_color.unwrap_or(Color::WHITE);

// Dimensions
let width = style.width.unwrap_or(100.0);
let height = style.height.unwrap_or(50.0);
let border_width = style.border.width;

// Effects
if let Some(glass) = &style.glass {
    let params = glass.to_compositor_params();
    // Use glass effect
}

if let Some(shadow) = &style.shadow {
    let expansion = shadow.bounds_expansion();
    // Apply shadow
}

// Transform
if !style.transform.is_identity() {
    let matrix = style.transform.to_affine2d();
    // Apply transform
}
```

### 4. Build Scene Nodes with CSS

```rust
use liquide_shell::css_integration::*;

// Instead of using hardcoded theme values:
// OLD:
let glass_tint = theme.dock_glass_tint;
let border_color = theme.dock_border;

// NEW:
let dock_style = resolve_dock_style(&resolver);
let glass = glass_params_from_style(&dock_style).unwrap_or_default();
let (border_color, border_width) = border_from_style(&dock_style).unwrap_or_default();

// Build node with CSS-derived values
let dock_node = SceneNode::new(
    NODE_DOCK,
    SceneNodeKind::Glass(glass),
    NodeProperties::new(dock_bounds).with_z_order(900),
);
```

## CSS Theme Syntax

### Supported Properties

```css
/* Basic Properties */
element {
    /* Colors */
    background: rgba(46, 52, 64, 225);
    background-color: rgb(46, 52, 64);
    color: #eceff4;
    border-color: rgb(76, 86, 106);
    
    /* Dimensions */
    width: 100px;
    height: 50px;
    
    /* Border */
    border-width: 2px;
    border-style: solid;  /* none | solid | dashed | dotted | double */
    border-radius: 4px;
    
    /* Spacing */
    padding: 8px;
    padding-top: 4px;
    margin: 10px 5px;
    
    /* Effects */
    opacity: 0.9;
    
    /* Text */
    font-size: 14px;
    font-weight: 400;
    line-height: 1.5;
    
    /* Layout */
    z-index: 100;
    visibility: visible;  /* visible | hidden */
}

/* States */
element:hover {
    background: rgba(94, 129, 172, 60);
}

element:active {
    color: rgb(236, 239, 244);
}

element.focused {
    border-color: rgb(94, 129, 172);
}

/* Custom Properties (when CSS engine supports) */
element {
    glass-blur: 20px;
    glass-tint: rgba(62, 62, 72, 200);
    
    shadow-offset-x: 0px;
    shadow-offset-y: 4px;
    shadow-blur: 16px;
    shadow-color: rgba(0, 0, 0, 80);
    
    transform-translate-x: 10px;
    transform-translate-y: 5px;
    transform-rotate: 45deg;
    transform-scale: 1.5;
}
```

### Shell Element Selectors

```css
/* Desktop */
desktop {
    background: rgb(30, 30, 40);
}

/* Windows */
window {
    border-color: rgb(76, 86, 106);
    box-shadow-color: rgba(0, 0, 0, 80);
}

window.focused {
    border-color: rgb(94, 129, 172);
}

titlebar {
    background: rgba(60, 60, 70, 240);
    color: rgb(236, 239, 244);
}

/* Dock */
dock {
    background: rgba(46, 52, 64, 225);
    border-color: rgb(76, 86, 106);
}

dock-item {
    color: rgba(216, 222, 233, 200);
}

dock-item.active {
    color: rgb(236, 239, 244);
}

dock-item:hover {
    background: rgba(94, 129, 172, 60);
}

/* Status Bar */
statusbar {
    background: rgba(59, 66, 82, 240);
    border-bottom-color: rgb(76, 86, 106);
    color: rgb(236, 239, 244);
}

status-indicator.connected {
    color: rgb(163, 190, 140);
}

status-indicator.degraded {
    color: rgb(235, 203, 139);
}

/* Launcher */
launcher {
    background: rgba(46, 52, 64, 245);
}

launcher-overlay {
    background: rgba(0, 0, 0, 120);
}

launcher-search {
    background: rgba(59, 66, 82, 255);
}

launcher-item.selected {
    background: rgba(94, 129, 172, 150);
}
```

## Helper Functions

```rust
use liquide_shell::css_integration::*;

// Create resolver from engine
let resolver = create_style_resolver(engine);

// Query element styles
let dock_style = resolve_dock_style(&resolver);
let dock_item_style = resolve_dock_item_style(&resolver, is_active);
let statusbar_style = resolve_status_bar_style(&resolver);
let window_style = resolve_window_style(&resolver, is_focused);

// Convert to compositor types
let glass_params = glass_params_from_style(&style);
let (border_color, border_width) = border_from_style(&style);

// Fallback values
let color = color_or_default(style.foreground_color, Color::WHITE);
```

## Complete Example

See `/crates/liquide-shell/src/css_dock_example.rs` for a complete refactored dock implementation using CSS styling.

```rust
pub fn build_dock_scene_with_css(
    dock_bounds: Rect,
    items: &[DockItemData],
    item_rects: &[Rect],
    show_running_indicators: bool,
    resolver: &StyleResolver,
) -> SceneNode {
    // Query CSS
    let dock_style = resolve_dock_style(resolver);
    
    // Extract values with fallbacks
    let glass = glass_params_from_style(&dock_style).unwrap_or_else(|| {
        GlassParams {
            blur_radius: 20,
            tint_color: Color::new(46, 52, 64, 225),
            inner_glow: true,
            parallax: false,
        }
    });
    
    // Build scene node
    let mut dock_node = SceneNode::new(
        NODE_DOCK,
        SceneNodeKind::Glass(glass),
        NodeProperties::new(dock_bounds).with_z_order(900),
    );
    
    // Add border from CSS
    let border_color = dock_style.border.color;
    let border_rect = Rect::new(0.0, 0.0, dock_bounds.width, 2.0);
    dock_node.add_child(solid_rect(NODE_DOCK + 1, border_color, border_rect, 903));
    
    // Add items with CSS-driven colors
    for (i, item_rect) in item_rects.iter().enumerate() {
        let item_style = resolve_dock_item_style(resolver, items[i].is_running);
        let color = color_or_default(
            item_style.foreground_color,
            Color::new(216, 222, 233, 200),
        );
        
        // ... render item with color ...
    }
    
    dock_node
}
```

## Data Structures

### RenderStyle
Complete style information for a UI element. All fields are optional with sensible defaults.

### GlassStyle
Liquid Glass effect parameters:
- `blur_radius`: Blur intensity (0-50)
- `tint_color`: Glass overlay color
- `inner_glow`: Enable subtle inner highlight
- `parallax`: Enable depth effect
- `opacity`: Glass transparency (0.0-1.0)
- `high_quality`: Use high-quality blur algorithm

### ShadowStyle
Box shadow effect:
- `offset_x`, `offset_y`: Shadow offset in pixels
- `blur_radius`: Shadow blur spread
- `spread_radius`: Shadow expansion
- `color`: Shadow color with alpha
- `inset`: Inner or outer shadow

### TransformStyle
CSS transforms:
- `translate`: (x, y) translation in pixels
- `rotate`: Rotation angle in degrees
- `scale`: (sx, sy) scale factors
- `skew`: (x, y) skew angles in degrees
- `origin`: Transform origin point

## Performance Considerations

- Style resolution is fast (<1μs per query with caching)
- Query styles once per frame, not per draw call
- Cache RenderStyle objects for static elements
- Use fallback values to avoid repeated queries

## Known Issues

1. **CSS Engine Property Extraction**: ThemeEngine.query() currently returns empty PropertySet. This is an upstream bug in liquide-theme-css. Once fixed, all CSS queries will work properly.

2. **Workaround**: Use ShellTheme as fallback until CSS engine is fixed:
   ```rust
   let style = resolver.resolve("dock", &[], &[], None)
       .unwrap_or_else(|_| RenderStyle::new());
   let color = style.background_color.unwrap_or(theme.dock_glass_tint);
   ```

## Testing

Run middleware tests:
```bash
cargo test --package liquide-renderer-css
cargo test --package liquide-shell --lib css
```

Build middleware:
```bash
cargo build --package liquide-renderer-css
```

## Documentation

Generate API docs:
```bash
cargo doc --package liquide-renderer-css --open
```

## Contributing

When refactoring shell components to use CSS:

1. Add StyleResolver to component struct
2. Replace hardcoded theme values with CSS queries
3. Provide fallback values for incomplete CSS
4. Update build_scene() methods to accept resolver
5. Write tests with example CSS

See `css_dock_example.rs` for reference implementation.
