//! CSS border-image parsing and 9-slice utilities.

use crate::display_list::BorderImageRepeat;

/// Parse a CSS border-image quad value (e.g. "10 20 30 40" or "10%" or "1").
/// Returns (top, right, bottom, left) as f32 values.
pub(crate) fn parse_border_image_quad(value: &str, fallback: f32) -> (f32, f32, f32, f32) {
    let parts: Vec<f32> = value
        .split_whitespace()
        .map(|p| {
            if let Some(pct) = p.strip_suffix('%') {
                pct.parse::<f32>().unwrap_or(fallback)
            } else {
                p.parse::<f32>().unwrap_or(fallback)
            }
        })
        .collect();
    match parts.len() {
        1 => (parts[0], parts[0], parts[0], parts[0]),
        2 => (parts[0], parts[1], parts[0], parts[1]),
        3 => (parts[0], parts[1], parts[2], parts[1]),
        4 => (parts[0], parts[1], parts[2], parts[3]),
        _ => (fallback, fallback, fallback, fallback),
    }
}

/// Parse CSS border-image-repeat value (e.g. "stretch", "round repeat").
/// Returns (repeat_x, repeat_y).
pub(crate) fn parse_border_image_repeat(
    value: &str,
) -> (BorderImageRepeat, BorderImageRepeat) {
    let parse_one = |s: &str| -> BorderImageRepeat {
        match s.trim() {
            "repeat" => BorderImageRepeat::Repeat,
            "round" => BorderImageRepeat::Round,
            "space" => BorderImageRepeat::Space,
            _ => BorderImageRepeat::Stretch,
        }
    };
    let parts: Vec<&str> = value.split_whitespace().collect();
    let x = parse_one(parts.first().copied().unwrap_or("stretch"));
    let y = parse_one(parts.get(1).copied().unwrap_or(parts.first().copied().unwrap_or("stretch")));
    (x, y)
}

// ─── 9-Slice Region Computation ─────────────────────────────

/// A rectangle defined by position and size (independent of layout crate).
#[derive(Debug, Clone, Copy)]
pub(crate) struct SliceRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl SliceRect {
    pub fn is_empty(&self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }
}

/// The nine regions produced by slicing a border image.
///
/// Corners are never stretched/tiled — they are drawn 1:1.
/// Edges are tiled according to `repeat_x` / `repeat_y`.
/// The center is optionally filled (CSS `border-image-slice: … fill`).
#[derive(Debug, Clone)]
pub(crate) struct NineSlice {
    // Corners: top-left, top-right, bottom-right, bottom-left
    pub corner_tl: SliceRect,
    pub corner_tr: SliceRect,
    pub corner_br: SliceRect,
    pub corner_bl: SliceRect,
    // Edges: top, right, bottom, left
    pub edge_top: SliceRect,
    pub edge_right: SliceRect,
    pub edge_bottom: SliceRect,
    pub edge_left: SliceRect,
    // Center
    pub center: SliceRect,
}

/// Compute the nine destination regions for border-image rendering.
///
/// # Parameters
/// - `element`: the bounding rectangle of the element (after outset).
/// - `widths`: (top, right, bottom, left) border-image widths in pixels.
pub(crate) fn compute_nine_slice_regions(
    element_x: f32,
    element_y: f32,
    element_w: f32,
    element_h: f32,
    widths: (f32, f32, f32, f32),
) -> NineSlice {
    let (wt, wr, wb, wl) = widths;

    // Clamp widths so they don't exceed element dimensions
    let scale_x = if wl + wr > element_w {
        element_w / (wl + wr)
    } else {
        1.0
    };
    let scale_y = if wt + wb > element_h {
        element_h / (wt + wb)
    } else {
        1.0
    };
    let wt = wt * scale_y;
    let wb = wb * scale_y;
    let wl = wl * scale_x;
    let wr = wr * scale_x;

    let inner_x = element_x + wl;
    let inner_y = element_y + wt;
    let inner_w = (element_w - wl - wr).max(0.0);
    let inner_h = (element_h - wt - wb).max(0.0);

    NineSlice {
        // Corners
        corner_tl: SliceRect { x: element_x, y: element_y, width: wl, height: wt },
        corner_tr: SliceRect { x: element_x + element_w - wr, y: element_y, width: wr, height: wt },
        corner_br: SliceRect { x: element_x + element_w - wr, y: element_y + element_h - wb, width: wr, height: wb },
        corner_bl: SliceRect { x: element_x, y: element_y + element_h - wb, width: wl, height: wb },
        // Edges
        edge_top: SliceRect { x: inner_x, y: element_y, width: inner_w, height: wt },
        edge_right: SliceRect { x: element_x + element_w - wr, y: inner_y, width: wr, height: inner_h },
        edge_bottom: SliceRect { x: inner_x, y: element_y + element_h - wb, width: inner_w, height: wb },
        edge_left: SliceRect { x: element_x, y: inner_y, width: wl, height: inner_h },
        // Center
        center: SliceRect { x: inner_x, y: inner_y, width: inner_w, height: inner_h },
    }
}

/// Compute the source-image slice regions (normalized coordinates 0..1).
///
/// Slice values are percentages of the source image dimensions.
/// Returns regions in the same `NineSlice` layout but with coordinates
/// normalized to [0, 1] for use as texture UV coordinates.
pub(crate) fn compute_source_slice_uvs(
    slice: (f32, f32, f32, f32),
) -> NineSlice {
    // slice values are in percentage (0..100), normalize to 0..1
    let st = (slice.0 / 100.0).clamp(0.0, 1.0);
    let sr = (slice.1 / 100.0).clamp(0.0, 1.0);
    let sb = (slice.2 / 100.0).clamp(0.0, 1.0);
    let sl = (slice.3 / 100.0).clamp(0.0, 1.0);

    let inner_u = sl;
    let inner_v = st;
    let inner_w = (1.0 - sl - sr).max(0.0);
    let inner_h = (1.0 - st - sb).max(0.0);

    NineSlice {
        corner_tl: SliceRect { x: 0.0, y: 0.0, width: sl, height: st },
        corner_tr: SliceRect { x: 1.0 - sr, y: 0.0, width: sr, height: st },
        corner_br: SliceRect { x: 1.0 - sr, y: 1.0 - sb, width: sr, height: sb },
        corner_bl: SliceRect { x: 0.0, y: 1.0 - sb, width: sl, height: sb },
        edge_top: SliceRect { x: inner_u, y: 0.0, width: inner_w, height: st },
        edge_right: SliceRect { x: 1.0 - sr, y: inner_v, width: sr, height: inner_h },
        edge_bottom: SliceRect { x: inner_u, y: 1.0 - sb, width: inner_w, height: sb },
        edge_left: SliceRect { x: 0.0, y: inner_v, width: sl, height: inner_h },
        center: SliceRect { x: inner_u, y: inner_v, width: inner_w, height: inner_h },
    }
}

/// Compute how many tiles fit along an edge, accounting for repeat mode.
///
/// Returns the tile size after adjustment and the number of tiles.
/// - `Stretch`: 1 tile stretched to fill `edge_len`.
/// - `Repeat`: tiles at natural size, possibly clipped at ends.
/// - `Round`: tiles resized so an integer number fills `edge_len`.
/// - `Space`: tiles at natural size with equal spacing between them.
pub(crate) fn compute_edge_tiling(
    edge_len: f32,
    natural_tile_size: f32,
    mode: BorderImageRepeat,
) -> (f32, usize) {
    if edge_len <= 0.0 || natural_tile_size <= 0.0 {
        return (0.0, 0);
    }
    match mode {
        BorderImageRepeat::Stretch => (edge_len, 1),
        BorderImageRepeat::Repeat => {
            let count = (edge_len / natural_tile_size).ceil().max(1.0) as usize;
            (natural_tile_size, count)
        }
        BorderImageRepeat::Round => {
            let count = (edge_len / natural_tile_size).round().max(1.0) as usize;
            let adjusted = edge_len / count as f32;
            (adjusted, count)
        }
        BorderImageRepeat::Space => {
            let count = (edge_len / natural_tile_size).floor().max(1.0) as usize;
            // Tiles keep natural size; spacing is distributed evenly.
            (natural_tile_size, count)
        }
    }
}

/// Compute the spacing between tiles for `Space` repeat mode.
/// Returns 0.0 for other modes.
pub(crate) fn compute_space_gap(
    edge_len: f32,
    tile_size: f32,
    tile_count: usize,
    mode: BorderImageRepeat,
) -> f32 {
    if mode != BorderImageRepeat::Space || tile_count <= 1 {
        return 0.0;
    }
    let total_tile = tile_size * tile_count as f32;
    let remaining = (edge_len - total_tile).max(0.0);
    remaining / (tile_count - 1) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_quad_single() {
        assert_eq!(parse_border_image_quad("10", 0.0), (10.0, 10.0, 10.0, 10.0));
    }

    #[test]
    fn parse_quad_two() {
        assert_eq!(parse_border_image_quad("10 20", 0.0), (10.0, 20.0, 10.0, 20.0));
    }

    #[test]
    fn parse_quad_three() {
        assert_eq!(parse_border_image_quad("10 20 30", 0.0), (10.0, 20.0, 30.0, 20.0));
    }

    #[test]
    fn parse_quad_four() {
        assert_eq!(parse_border_image_quad("10 20 30 40", 0.0), (10.0, 20.0, 30.0, 40.0));
    }

    #[test]
    fn parse_quad_percent() {
        assert_eq!(parse_border_image_quad("25%", 0.0), (25.0, 25.0, 25.0, 25.0));
    }

    #[test]
    fn parse_repeat_single() {
        let (x, y) = parse_border_image_repeat("round");
        assert_eq!(x, BorderImageRepeat::Round);
        assert_eq!(y, BorderImageRepeat::Round);
    }

    #[test]
    fn parse_repeat_two() {
        let (x, y) = parse_border_image_repeat("repeat space");
        assert_eq!(x, BorderImageRepeat::Repeat);
        assert_eq!(y, BorderImageRepeat::Space);
    }

    #[test]
    fn nine_slice_basic() {
        let ns = compute_nine_slice_regions(0.0, 0.0, 100.0, 100.0, (10.0, 10.0, 10.0, 10.0));
        assert!((ns.corner_tl.width - 10.0).abs() < 0.001);
        assert!((ns.center.width - 80.0).abs() < 0.001);
        assert!((ns.center.height - 80.0).abs() < 0.001);
        assert!((ns.edge_top.width - 80.0).abs() < 0.001);
        assert!((ns.edge_top.height - 10.0).abs() < 0.001);
    }

    #[test]
    fn nine_slice_clamped_widths() {
        // Widths exceed element size — should be scaled down
        let ns = compute_nine_slice_regions(0.0, 0.0, 20.0, 20.0, (15.0, 15.0, 15.0, 15.0));
        let total_x = ns.corner_tl.width + ns.center.width + ns.corner_tr.width;
        assert!((total_x - 20.0).abs() < 0.01);
    }

    #[test]
    fn source_slice_uvs() {
        let uvs = compute_source_slice_uvs((25.0, 25.0, 25.0, 25.0));
        assert!((uvs.corner_tl.width - 0.25).abs() < 0.001);
        assert!((uvs.center.x - 0.25).abs() < 0.001);
        assert!((uvs.center.width - 0.5).abs() < 0.001);
    }

    #[test]
    fn edge_tiling_stretch() {
        let (size, count) = compute_edge_tiling(80.0, 20.0, BorderImageRepeat::Stretch);
        assert_eq!(count, 1);
        assert!((size - 80.0).abs() < 0.001);
    }

    #[test]
    fn edge_tiling_repeat() {
        let (size, count) = compute_edge_tiling(80.0, 20.0, BorderImageRepeat::Repeat);
        assert_eq!(count, 4);
        assert!((size - 20.0).abs() < 0.001);
    }

    #[test]
    fn edge_tiling_round() {
        // 80 / 30 = 2.67 → rounds to 3 tiles, each ~26.67px
        let (size, count) = compute_edge_tiling(80.0, 30.0, BorderImageRepeat::Round);
        assert_eq!(count, 3);
        assert!((size - 80.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn edge_tiling_space() {
        let (size, count) = compute_edge_tiling(80.0, 20.0, BorderImageRepeat::Space);
        assert_eq!(count, 4);
        assert!((size - 20.0).abs() < 0.001);
        let gap = compute_space_gap(80.0, 20.0, 4, BorderImageRepeat::Space);
        assert!((gap - 0.0).abs() < 0.001); // 4*20=80, no leftover
    }

    #[test]
    fn space_gap_with_remainder() {
        // 100px edge, 30px tiles → 3 tiles (90px), 10px remaining, 2 gaps → 5px each
        let (size, count) = compute_edge_tiling(100.0, 30.0, BorderImageRepeat::Space);
        assert_eq!(count, 3);
        assert!((size - 30.0).abs() < 0.001);
        let gap = compute_space_gap(100.0, 30.0, 3, BorderImageRepeat::Space);
        assert!((gap - 5.0).abs() < 0.001);
    }
}
