# liquide-theme-css

CSS parser and theme engine for Liquide desktop compositor.

## Overview

Complete CSS-based theme system allowing desktop themes to be defined using standard CSS syntax with support for selectors, cascade rules, variables, and hot-reloading.

## Features

- ✅ Full CSS parser with properties and selectors
- ✅ Element, class, ID, and pseudo-class selectors
- ✅ CSS cascade and specificity rules
- ✅ CSS variables (custom properties)
- ✅ Color manipulation (lighten, darken, mix)
- ✅ Gradients (linear and radial)
- ✅ Box shadows with multiple layers
- ✅ Border styles
- ✅ Hot-reloading with file watching
- ✅ Length units (px, pt, em, rem, %)

## Quick Start

```rust
use liquide_theme_css::{ThemeParser, ThemeEngine};

// Parse CSS theme
let css = r#"
    window {
        background: #2e3440;
        border: 1px solid #4c566a;
        border-radius: 8px;
    }
    
    window.focused {
        border-color: #5e81ac;
    }
"#;

let parser = ThemeParser::new();
let theme = parser.parse_str(css)?;

// Create engine and query styles
let engine = ThemeEngine::new(theme);
let styles = engine.query("window", &vec!["focused".to_string()], &[])?;

// Get property values
if let Some(bg) = styles.get("background") {
    let color = bg.as_color().unwrap();
    println!("Background: #{:02x}{:02x}{:02x}", color.r, color.g, color.b);
}
```

## CSS Theme Format

### Basic Styling

```css
/* Element selectors */
window {
    background: #2e3440;
    border: 1px solid #4c566a;
    border-radius: 8px;
    padding: 0px;
}

button {
    background: #4c566a;
    color: #eceff4;
    border-radius: 4px;
    padding: 8px;
}

titlebar {
    background: #3b4252;
    height: 32px;
    color: #eceff4;
}
```

### Class Selectors

```css
button.primary {
    background: #5e81ac;
}

button.danger {
    background: #bf616a;
}

window.frameless {
    border: none;
}
```

### ID Selectors

```css
#main-window {
    width: 1920px;
    height: 1080px;
}

#dock {
    height: 48px;
}
```

### Pseudo-classes

```css
button:hover {
    background: #5e81ac;
}

button:active {
    background: #4c566a;
}

window:focus {
    border-color: #88c0d0;
}
```

### Complex Selectors

```css
button.primary:hover {
    background: #81a1c1;
}

window.focused titlebar {
    background: #434c5e;
}
```

## Advanced Features

### CSS Variables

```css
:root {
    --primary-color: #5e81ac;
    --secondary-color: #81a1c1;
    --accent-color: #88c0d0;
    --bg-color: #2e3440;
    --text-color: #eceff4;
}

button {
    background: var(--primary-color);
    color: var(--text-color);
}

button:hover {
    background: var(--secondary-color);
}
```

### Gradients

```css
titlebar {
    background: linear-gradient(180deg, #3b4252 0%, #2e3440 100%);
}

button.glossy {
    background: linear-gradient(180deg, #5e81ac 0%, #4c566a 100%);
}

window.radial-bg {
    background: radial-gradient(circle, #434c5e 0%, #2e3440 100%);
}
```

### Box Shadows

```css
window {
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.3);
}

button {
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2),
                inset 0 1px 0 rgba(255, 255, 255, 0.1);
}

window.focused {
    box-shadow: 0 8px 24px rgba(94, 129, 172, 0.3);
}
```

### Complete Theme Example

```css
/* Nord Theme for Liquide Desktop */

:root {
    --nord0: #2e3440;
    --nord1: #3b4252;
    --nord2: #434c5e;
    --nord3: #4c566a;
    --nord4: #d8dee9;
    --nord5: #e5e9f0;
    --nord6: #eceff4;
    --nord7: #8fbcbb;
    --nord8: #88c0d0;
    --nord9: #81a1c1;
    --nord10: #5e81ac;
    --nord11: #bf616a;
    --nord12: #d08770;
    --nord13: #ebcb8b;
    --nord14: #a3be8c;
    --nord15: #b48ead;
}

/* Windows */
window {
    background: var(--nord0);
    border: 1px solid var(--nord3);
    border-radius: 8px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.3);
}

window.focused {
    border-color: var(--nord10);
    box-shadow: 0 6px 20px rgba(94, 129, 172, 0.4);
}

/* Titlebar */
titlebar {
    background: linear-gradient(180deg, var(--nord1) 0%, var(--nord0) 100%);
    height: 32px;
    color: var(--nord6);
    border-bottom: 1px solid var(--nord3);
}

titlebar.inactive {
    background: var(--nord1);
    color: var(--nord4);
}

/* Window buttons */
button.close {
    background: var(--nord11);
    border-radius: 50%;
}

button.close:hover {
    background: var(--nord12);
}

button.minimize {
    background: var(--nord13);
    border-radius: 50%;
}

button.minimize:hover {
    background: #d08770;
}

button.maximize {
    background: var(--nord14);
    border-radius: 50%;
}

button.maximize:hover {
    background: #a3be8c;
}

/* Dock */
dock {
    background: rgba(46, 52, 64, 0.95);
    height: 48px;
    border-top: 1px solid var(--nord3);
    box-shadow: 0 -2px 8px rgba(0, 0, 0, 0.2);
}

dock-item {
    background: transparent;
    border-radius: 8px;
    width: 48px;
    height: 48px;
}

dock-item:hover {
    background: var(--nord2);
}

dock-item.running {
    background: var(--nord10);
}

/* Status bar */
statusbar {
    background: var(--nord0);
    height: 24px;
    border-bottom: 1px solid var(--nord3);
    color: var(--nord4);
}

/* Context menus */
menu {
    background: var(--nord1);
    border: 1px solid var(--nord3);
    border-radius: 6px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
}

menu-item {
    padding: 8px 16px;
    color: var(--nord6);
}

menu-item:hover {
    background: var(--nord10);
}

menu-separator {
    background: var(--nord3);
    height: 1px;
}

/* Buttons */
button {
    background: var(--nord3);
    color: var(--nord6);
    border-radius: 4px;
    padding: 8px 16px;
    border: none;
}

button:hover {
    background: var(--nord10);
}

button:active {
    background: var(--nord9);
}

button.primary {
    background: var(--nord10);
}

button.primary:hover {
    background: var(--nord9);
}

/* Scrollbars */
scrollbar {
    width: 8px;
    background: var(--nord1);
}

scrollbar-thumb {
    background: var(--nord3);
    border-radius: 4px;
}

scrollbar-thumb:hover {
    background: var(--nord10);
}
```

## API Usage

### Parsing Themes

```rust
use liquide_theme_css::ThemeParser;

let parser = ThemeParser::new();

// From string
let theme = parser.parse_str(css_string)?;

// From file
let theme = parser.parse_file("themes/nord.css")?;
```

### Querying Styles

```rust
use liquide_theme_css::ThemeEngine;

let engine = ThemeEngine::new(theme);

// Query with classes
let styles = engine.query(
    "button",
    &vec!["primary".to_string()],
    &vec!["hover".to_string()],
)?;

// Query with ID
let styles = engine.query_with_id(
    "window",
    Some("main"),
    &vec!["focused".to_string()],
    &[],
)?;

// Get specific property
let bg = engine.get_property("button", &[], &[], "background")?;
```

### Working with Colors

```rust
use liquide_theme_css::prelude::Color;

// Parse colors
let color = Color::from_hex("#5e81ac")?;
let rgba = Color::new(94, 129, 172, 255);
let rgb = Color::rgb(94, 129, 172);

// Manipulate colors
let lighter = color.lighten(0.2);  // 20% lighter
let darker = color.darken(0.1);    // 10% darker
let mixed = color.mix(&Color::rgb(255, 255, 255), 0.5); // 50% mix

// Convert to hex
let hex = color.to_hex();  // "#5e81ac"
```

### Hot-Reloading

```rust
use liquide_theme_css::watcher::ThemeWatcher;
use std::sync::{Arc, Mutex};

let engine = Arc::new(Mutex::new(ThemeEngine::new(theme)));
let mut watcher = ThemeWatcher::new();

// Set update callback
let engine_clone = engine.clone();
watcher.on_update(move |new_theme| {
    let mut eng = engine_clone.lock().unwrap();
    eng.set_stylesheet(new_theme);
    println!("Theme reloaded!");
});

// Watch theme directory
watcher.watch("themes/")?;
watcher.start()?;

// Theme updates automatically when files change
```

## Property Values

### Supported Properties

| Property | Values | Example |
|----------|--------|---------|
| background | color, gradient | `#2e3440`, `linear-gradient(...)` |
| color | color | `#eceff4` |
| border | width style color | `1px solid #4c566a` |
| border-radius | length | `8px` |
| padding | length | `8px`, `1em` |
| margin | length | `16px` |
| width | length | `100px`, `50%` |
| height | length | `32px` |
| box-shadow | shadows | `0 4px 16px rgba(0,0,0,0.3)` |
| opacity | number | `0.95` |

### Length Units

- **px** - Pixels (absolute)
- **pt** - Points (1pt = 1.333px)
- **em** - Relative to element font size
- **rem** - Relative to root font size
- **%** - Percentage of parent

```rust
use liquide_theme_css::value::LengthUnit;

let px = LengthUnit::Px(16.0);
let em = LengthUnit::Em(1.5);
let percent = LengthUnit::Percent(50.0);

// Convert to pixels
let pixels = em.to_px(16.0); // 1.5em * 16px = 24px
```

## CSS Cascade

The engine implements proper CSS cascade rules:

1. **Specificity**: ID > Class > Element
2. **Source order**: Later rules override earlier ones
3. **Inheritance**: Some properties inherit from parent

```rust
// These rules are applied in specificity order
let css = r#"
    button { background: red; }           /* Specificity: (0,0,1) */
    button.primary { background: blue; }  /* Specificity: (0,1,1) */
    #main-btn { background: green; }      /* Specificity: (1,0,0) */
"#;

// Query for button#main-btn.primary
// Result: green (ID has highest specificity)
```

## Performance

- **Parse time**: ~5ms for 100-rule stylesheet
- **Query time**: ~1μs per style query (cached selectors)
- **Memory**: ~50KB for typical 100-rule stylesheet
- **Hot-reload**: ~10ms to reparse and apply

## Testing

```bash
# Run tests
cargo test -p liquide-theme-css

# Test parser
cargo test -p liquide-theme-css -- test_parse --nocapture

# Test cascade
cargo test -p liquide-theme-css -- test_cascade --nocapture
```

## Examples

See `examples/parse_theme.rs` for a complete example:

```bash
cargo run --example parse_theme
```

## Debugging

Enable logging to see theme parsing and query details:

```bash
RUST_LOG=liquide_theme_css=debug cargo run
```

## Migration from Hardcoded Styles

### Before

```rust
struct WindowStyle {
    background: Color,
    border_color: Color,
    border_radius: f32,
}

let style = WindowStyle {
    background: Color::rgb(46, 52, 64),
    border_color: Color::rgb(76, 86, 106),
    border_radius: 8.0,
};
```

### After

```rust
// theme.css
// window {
//     background: #2e3440;
//     border-color: #4c566a;
//     border-radius: 8px;
// }

let styles = engine.query("window", &[], &[])?;
let background = styles.get("background")?.as_color()?.clone();
let border_color = styles.get("border-color")?.as_color()?.clone();
let border_radius = styles.get("border-radius")?.as_length()?.to_px(16.0);
```

## Future Enhancements

- [ ] CSS animations and transitions
- [ ] Media queries for resolution/theme
- [ ] CSS flexbox for layout
- [ ] CSS grid support
- [ ] @import rules
- [ ] Nested selectors (SCSS-like)
- [ ] CSS custom functions
- [ ] Theme inheritance

## Dependencies

- `lightningcss` - Fast CSS parsing
- `cssparser` - CSS tokenization
- `csscolorparser` - Color parsing
- `notify` - File watching
- `serde` - Serialization

## License

MIT OR Apache-2.0

## See Also

- [ADVANCED_FEATURES.md](../../docs/ADVANCED_FEATURES.md) - Complete integration guide
- [liquide-cursor](../liquide-cursor) - Cursor management
- [liquide-cursor-vector](../liquide-cursor-vector) - Vector cursor rendering
