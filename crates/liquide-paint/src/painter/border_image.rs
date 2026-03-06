//! CSS border-image parsing utilities.

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
) -> (
    crate::display_list::BorderImageRepeat,
    crate::display_list::BorderImageRepeat,
) {
    use crate::display_list::BorderImageRepeat;
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
