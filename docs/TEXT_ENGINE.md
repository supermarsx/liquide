# LiquiDE Text Engine

## Overview

The `liquide-text-engine` crate provides the complete text rendering stack:
from raw Unicode input to positioned, shaped, rasterized glyphs ready for
compositing.

## Pipeline

```
UTF-8 input
    │
    ├──► Script detection (UAX #24)
    │      Assigns each character a Unicode script (Latin, Arabic, Han, etc.)
    │
    ├──► BiDi analysis (UAX #9)
    │      Resolves embedding levels for mixed LTR/RTL text
    │      Produces visual reordering of logical runs
    │
    ├──► Itemization
    │      Splits text into runs of uniform (script × font × direction)
    │
    ├──► Shaping
    │      Applies OpenType features (ligatures, kerning, mark positioning)
    │      Maps characters → glyph IDs + advances + offsets
    │      HarfBuzz-compatible interface with built-in fallback
    │
    ├──► Line breaking (UAX #14)
    │      Identifies legal break opportunities
    │      Greedy or optimal (Knuth-Plass) paragraph filling
    │
    ├──► Paragraph layout
    │      Positions glyphs on lines, applies alignment (left/center/right/justify)
    │      Handles indent, line spacing, tab stops
    │
    ├──► Rasterization
    │      FreeType (Linux), DirectWrite (Windows), CoreText (macOS)
    │      Outputs alpha/subpixel bitmaps → glyph atlas
    │
    └──► Compositing
           Blits glyphs from atlas to frame buffer with correct blending
```

## Script Detection (UAX #24)

Each Unicode code point has an assigned script property. The engine groups
contiguous characters of the same script into *script runs*, with `Common`
and `Inherited` scripts resolved to the surrounding context.

## Bidirectional Algorithm (UAX #9)

The full Unicode Bidirectional Algorithm:
1. Resolve explicit embedding levels (LRE, RLE, LRO, RLO, PDF, LRI, RLI, FSI, PDI)
2. Resolve weak types (EN, ES, ET, AN, CS, NSM, BN)
3. Resolve neutral types (B, S, WS, ON)
4. Resolve implicit levels
5. Reorder for display (L1–L4 rules)

The result maps each character to a *resolved level* and produces a visual
ordering of runs.

## Shaping

The shaper converts a sequence of Unicode code points into positioned glyphs:

```rust
pub struct ShapedGlyph {
    pub glyph_id: u32,       // Font-specific glyph index
    pub cluster: u32,        // Source character cluster
    pub x_advance: i32,      // Horizontal advance (26.6 fixed point)
    pub y_advance: i32,      // Vertical advance
    pub x_offset: i32,       // Glyph offset from baseline
    pub y_offset: i32,
}
```

Features applied during shaping:
- **GSUB** (glyph substitution): ligatures (`fi`, `ffl`), contextual alternates
- **GPOS** (glyph positioning): kerning, mark-to-base, mark-to-mark
- **Script-specific**: Arabic joining, Devanagari conjuncts, CJK vertical forms

## Line Breaking (UAX #14)

Every character pair has a break property:
- `Mandatory` — must break (CR, LF, LS, PS)
- `Allowed` — may break (spaces, hyphens)
- `Prohibited` — must not break (within words)

The algorithm resolves complex cases: CJK ideographs, numeric sequences,
emoji clusters, URL boundaries.

## Paragraph Layout

A `Paragraph` is a sequence of `LayoutLine`s, each containing positioned
`GlyphRun`s:

```rust
pub struct Paragraph {
    pub lines: Vec<LayoutLine>,
    pub width: f32,
    pub height: f32,
    pub alignment: TextAlignment,
}

pub struct LayoutLine {
    pub runs: Vec<GlyphRun>,
    pub baseline_y: f32,
    pub ascent: f32,
    pub descent: f32,
    pub width: f32,
}

pub struct GlyphRun {
    pub glyphs: Vec<PositionedGlyph>,
    pub font_id: FontId,
    pub size: f32,
    pub color: Color,
    pub direction: Direction,
}
```

## Selection Model

The selection model tracks:
- **Anchor**: where selection started (byte offset + affinity)
- **Focus**: current cursor position
- **Visual ranges**: painted highlight rectangles per line

### Caret Movement

| Action | Behavior |
|--------|----------|
| Left/Right | Move by grapheme cluster, respecting BiDi |
| Ctrl+Left/Right | Move by word boundary |
| Home/End | Move to line start/end |
| Up/Down | Visual line movement, maintaining preferred column |

### Hit Testing

Maps a pixel coordinate `(x, y)` to a character index within the paragraph:
1. Binary search for the line by `y`
2. Linear scan of glyph advances for `x` within that line
3. Return byte offset + affinity (before/after cluster boundary)

## Text Editing

The `TextEditor` provides a complete editing model:
- Insert/delete text at caret
- Selection + replace
- Undo/redo stack (operation-based)
- Clipboard cut/copy/paste
- IME composition integration

### Rich Text (future)

The editing model is designed to extend to rich text with:
- `Span` attributes (bold, italic, color, font, size)
- Inline images
- Embedded widgets
- Hyperlinks

## Font Rasterization

Platform-specific backends with a unified trait:

```rust
pub trait FontRasterizer: Send + Sync {
    fn rasterize_glyph(
        &self,
        font: &FontData,
        glyph_id: u32,
        size: f32,
        hints: HintingMode,
    ) -> RasterizedGlyph;

    fn metrics(&self, font: &FontData, size: f32) -> FontMetrics;
}
```

| Platform | Backend | Features |
|----------|---------|----------|
| Linux | FreeType + Fontconfig | Auto-hinting, LCD filtering |
| Windows | DirectWrite | ClearType, variable font support |
| macOS | Core Text | Sub-pixel rendering |
| Fallback | Built-in SDF | Basic Latin glyphs via SDF |

## Font Fallback

When a font doesn't contain a glyph, the fallback chain is consulted:

1. Requested font family
2. Role-specific stack (UI, terminal, data, etc.)
3. System default for the script
4. Noto Sans (covers all Unicode scripts)
5. Last-resort `.notdef` glyph
