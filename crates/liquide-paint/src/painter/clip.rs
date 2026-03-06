//! CSS clip-path parsing.

/// Parse a CSS `clip-path` string into a `ClipPath` shape.
pub(crate) fn parse_clip_path(value: &str, bounds: &liquide_layout::Rect) -> Option<crate::display_list::ClipPath> {
    use crate::display_list::ClipPath;
    let trimmed = value.trim();

    if trimmed.starts_with("circle(") {
        // circle(r at cx cy) or circle(r)
        let inner = trimmed.trim_start_matches("circle(").trim_end_matches(')');
        let parts: Vec<&str> = inner.split_whitespace().collect();
        let r = parse_length_or_percent(parts.first().copied().unwrap_or("50%"), bounds.width * 0.5);
        let (cx, cy) = if parts.len() >= 4 && parts[1] == "at" {
            (
                parse_length_or_percent(parts[2], bounds.width) + bounds.x,
                parse_length_or_percent(parts[3], bounds.height) + bounds.y,
            )
        } else {
            (bounds.x + bounds.width * 0.5, bounds.y + bounds.height * 0.5)
        };
        Some(ClipPath::Circle { cx, cy, r })
    } else if trimmed.starts_with("ellipse(") {
        let inner = trimmed.trim_start_matches("ellipse(").trim_end_matches(')');
        let parts: Vec<&str> = inner.split_whitespace().collect();
        let rx = parse_length_or_percent(parts.first().copied().unwrap_or("50%"), bounds.width * 0.5);
        let ry = parse_length_or_percent(parts.get(1).copied().unwrap_or("50%"), bounds.height * 0.5);
        let (cx, cy) = if parts.len() >= 5 && parts[2] == "at" {
            (
                parse_length_or_percent(parts[3], bounds.width) + bounds.x,
                parse_length_or_percent(parts[4], bounds.height) + bounds.y,
            )
        } else {
            (bounds.x + bounds.width * 0.5, bounds.y + bounds.height * 0.5)
        };
        Some(ClipPath::Ellipse { cx, cy, rx, ry })
    } else if trimmed.starts_with("inset(") {
        let inner = trimmed.trim_start_matches("inset(").trim_end_matches(')');
        let parts: Vec<&str> = inner.split_whitespace().collect();
        let top = parse_length_or_percent(parts.first().copied().unwrap_or("0"), bounds.height);
        let right = parse_length_or_percent(parts.get(1).copied().unwrap_or("0"), bounds.width);
        let bottom = parse_length_or_percent(parts.get(2).copied().unwrap_or("0"), bounds.height);
        let left = parse_length_or_percent(parts.get(3).copied().unwrap_or("0"), bounds.width);
        Some(ClipPath::Inset {
            top,
            right,
            bottom,
            left,
            radius: liquide_style_engine::dimension::Corners::all(0.0),
        })
    } else if trimmed.starts_with("polygon(") {
        let inner = trimmed.trim_start_matches("polygon(").trim_end_matches(')');
        let points: Vec<(f32, f32)> = inner
            .split(',')
            .filter_map(|pair| {
                let coords: Vec<&str> = pair.trim().split_whitespace().collect();
                if coords.len() == 2 {
                    Some((
                        parse_length_or_percent(coords[0], bounds.width) + bounds.x,
                        parse_length_or_percent(coords[1], bounds.height) + bounds.y,
                    ))
                } else {
                    None
                }
            })
            .collect();
        if points.len() >= 3 {
            Some(ClipPath::Polygon(points))
        } else {
            None
        }
    } else {
        None
    }
}

/// Parse a CSS length value (px) or percentage into a pixel value.
pub(crate) fn parse_length_or_percent(value: &str, reference: f32) -> f32 {
    let trimmed = value.trim();
    if let Some(pct) = trimmed.strip_suffix('%') {
        pct.trim().parse::<f32>().unwrap_or(0.0) / 100.0 * reference
    } else if let Some(px) = trimmed.strip_suffix("px") {
        px.trim().parse::<f32>().unwrap_or(0.0)
    } else {
        trimmed.parse::<f32>().unwrap_or(0.0)
    }
}
