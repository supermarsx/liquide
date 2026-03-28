use crate::cursor::{CursorShape, CursorImage};
use crate::theme::CursorTheme;

/// Create the built-in default cursor theme
pub fn create_builtin_theme() -> CursorTheme {
    let mut theme = CursorTheme::new("default");
    theme.display_name = "Default".to_string();
    theme.comment = "Built-in cursor theme".to_string();

    // Generate basic cursors procedurally
    theme.add_cursor(CursorShape::Default, generate_arrow(24));
    theme.add_cursor(CursorShape::Pointer, generate_hand(24));
    theme.add_cursor(CursorShape::Text, generate_ibeam(24));
    theme.add_cursor(CursorShape::Crosshair, generate_crosshair(24));
    theme.add_cursor(CursorShape::Move, generate_move(24));
    theme.add_cursor(CursorShape::ResizeNS, generate_resize_ns(24));
    theme.add_cursor(CursorShape::ResizeEW, generate_resize_ew(24));
    theme.add_cursor(CursorShape::NotAllowed, generate_not_allowed(24));
    theme.add_cursor(CursorShape::Wait, generate_wait(24));

    theme
}

fn generate_arrow(size: u32) -> CursorImage {
    let mut pixels = vec![0u8; (size * size * 4) as usize];
    // Simple arrow shape (triangle pointing upper-left)
    for y in 0..size {
        for x in 0..size {
            let in_arrow = x <= y && x < size / 2 && y < size - 2;
            let on_border = in_arrow && (x == 0 || x == y || y == size - 3);
            let offset = ((y * size + x) * 4) as usize;
            if on_border {
                pixels[offset] = 0; pixels[offset + 1] = 0; pixels[offset + 2] = 0; pixels[offset + 3] = 255;
            } else if in_arrow {
                pixels[offset] = 255; pixels[offset + 1] = 255; pixels[offset + 2] = 255; pixels[offset + 3] = 255;
            }
        }
    }
    CursorImage::new(size, size, 0, 0, pixels)
}

fn generate_hand(size: u32) -> CursorImage {
    // Simplified hand cursor — pointing finger shape
    let mut pixels = vec![0u8; (size * size * 4) as usize];
    let cx = size / 3;
    let cy = size / 4;
    // Draw a simple finger-pointing shape
    for y in 0..size {
        for x in 0..size {
            let dx = x as i32 - cx as i32;
            let dy = y as i32 - cy as i32;
            let in_finger = dx >= 0 && dx < 6 && dy >= 0 && dy < (size as i32 * 2 / 3);
            let in_palm = dy >= (size as i32 / 3) && dx >= -2 && dx < 10 && dy < (size as i32 - 2);
            let offset = ((y * size + x) * 4) as usize;
            if in_finger || in_palm {
                let on_edge = (in_finger && (dx == 0 || dx == 5)) || (in_palm && (dx == -2 || dx == 9));
                if on_edge {
                    pixels[offset] = 0; pixels[offset + 1] = 0; pixels[offset + 2] = 0; pixels[offset + 3] = 255;
                } else {
                    pixels[offset] = 255; pixels[offset + 1] = 255; pixels[offset + 2] = 255; pixels[offset + 3] = 255;
                }
            }
        }
    }
    CursorImage::new(size, size, cx, cy, pixels)
}

fn generate_ibeam(size: u32) -> CursorImage {
    let mut pixels = vec![0u8; (size * size * 4) as usize];
    let cx = size / 2;
    for y in 0..size {
        for x in 0..size {
            let on_stem = x == cx && y > 2 && y < size - 3;
            let on_serif = (y == 2 || y == size - 3) && (x >= cx - 3 && x <= cx + 3);
            let offset = ((y * size + x) * 4) as usize;
            if on_stem || on_serif {
                pixels[offset] = 0; pixels[offset + 1] = 0; pixels[offset + 2] = 0; pixels[offset + 3] = 255;
            }
        }
    }
    CursorImage::new(size, size, cx, size / 2, pixels)
}

fn generate_crosshair(size: u32) -> CursorImage {
    let mut pixels = vec![0u8; (size * size * 4) as usize];
    let c = size / 2;
    for y in 0..size {
        for x in 0..size {
            let on_h = y == c && (x < c - 2 || x > c + 2);
            let on_v = x == c && (y < c - 2 || y > c + 2);
            let offset = ((y * size + x) * 4) as usize;
            if on_h || on_v {
                pixels[offset] = 0; pixels[offset + 1] = 0; pixels[offset + 2] = 0; pixels[offset + 3] = 255;
            }
        }
    }
    CursorImage::new(size, size, c, c, pixels)
}

fn generate_move(size: u32) -> CursorImage {
    let mut pixels = vec![0u8; (size * size * 4) as usize];
    let c = size / 2;
    for y in 0..size {
        for x in 0..size {
            let on_h = y == c;
            let on_v = x == c;
            // Arrow tips
            let at_left = x < 4 && y >= c - (3 - x as i32).unsigned_abs() && y <= c + (3 - x as i32).unsigned_abs();
            let at_right = x > size - 5 && y >= c - (x as i32 - (size as i32 - 4)).unsigned_abs() && y <= c + (x as i32 - (size as i32 - 4)).unsigned_abs();
            let at_top = y < 4 && x >= c - (3 - y as i32).unsigned_abs() && x <= c + (3 - y as i32).unsigned_abs();
            let at_bottom = y > size - 5 && x >= c - (y as i32 - (size as i32 - 4)).unsigned_abs() && x <= c + (y as i32 - (size as i32 - 4)).unsigned_abs();

            let offset = ((y * size + x) * 4) as usize;
            if on_h || on_v || at_left || at_right || at_top || at_bottom {
                pixels[offset] = 0; pixels[offset + 1] = 0; pixels[offset + 2] = 0; pixels[offset + 3] = 255;
            }
        }
    }
    CursorImage::new(size, size, c, c, pixels)
}

fn generate_resize_ns(size: u32) -> CursorImage {
    let mut pixels = vec![0u8; (size * size * 4) as usize];
    let c = size / 2;
    for y in 0..size {
        for x in 0..size {
            let on_stem = x == c && y > 3 && y < size - 4;
            let at_top = y < 4 && x >= c - (3 - y as i32).unsigned_abs() && x <= c + (3 - y as i32).unsigned_abs();
            let at_bottom = y > size - 5 && x >= c - (y as i32 - (size as i32 - 4)).unsigned_abs() && x <= c + (y as i32 - (size as i32 - 4)).unsigned_abs();
            let offset = ((y * size + x) * 4) as usize;
            if on_stem || at_top || at_bottom {
                pixels[offset] = 0; pixels[offset + 1] = 0; pixels[offset + 2] = 0; pixels[offset + 3] = 255;
            }
        }
    }
    CursorImage::new(size, size, c, c, pixels)
}

fn generate_resize_ew(size: u32) -> CursorImage {
    let mut pixels = vec![0u8; (size * size * 4) as usize];
    let c = size / 2;
    for y in 0..size {
        for x in 0..size {
            let on_stem = y == c && x > 3 && x < size - 4;
            let at_left = x < 4 && y >= c - (3 - x as i32).unsigned_abs() && y <= c + (3 - x as i32).unsigned_abs();
            let at_right = x > size - 5 && y >= c - (x as i32 - (size as i32 - 4)).unsigned_abs() && y <= c + (x as i32 - (size as i32 - 4)).unsigned_abs();
            let offset = ((y * size + x) * 4) as usize;
            if on_stem || at_left || at_right {
                pixels[offset] = 0; pixels[offset + 1] = 0; pixels[offset + 2] = 0; pixels[offset + 3] = 255;
            }
        }
    }
    CursorImage::new(size, size, c, c, pixels)
}

fn generate_not_allowed(size: u32) -> CursorImage {
    let mut pixels = vec![0u8; (size * size * 4) as usize];
    let c = size as f32 / 2.0;
    let r = c - 2.0;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - c;
            let dy = y as f32 - c;
            let dist = (dx * dx + dy * dy).sqrt();
            let on_circle = (dist - r).abs() < 1.5;
            let on_slash = (dx + dy).abs() < 1.5 && dist < r;
            let offset = ((y * size + x) * 4) as usize;
            if on_circle || on_slash {
                pixels[offset] = 220; pixels[offset + 1] = 40; pixels[offset + 2] = 40; pixels[offset + 3] = 255;
            }
        }
    }
    CursorImage::new(size, size, size / 2, size / 2, pixels)
}

fn generate_wait(size: u32) -> CursorImage {
    // Hourglass shape
    let mut pixels = vec![0u8; (size * size * 4) as usize];
    let cx = size / 2;
    for y in 0..size {
        for x in 0..size {
            let normalized_y = y as f32 / size as f32;
            let width_at_y = if normalized_y < 0.5 {
                (0.5 - normalized_y) * (size as f32 * 0.8)
            } else {
                (normalized_y - 0.5) * (size as f32 * 0.8)
            };
            let half_w = (width_at_y / 2.0) as u32;
            let in_shape = x >= cx.saturating_sub(half_w) && x <= cx + half_w;
            let on_top_bottom = y == 0 || y == size - 1;
            let offset = ((y * size + x) * 4) as usize;
            if in_shape || (on_top_bottom && x >= cx - size / 4 && x <= cx + size / 4) {
                pixels[offset] = 0; pixels[offset + 1] = 0; pixels[offset + 2] = 0; pixels[offset + 3] = 255;
            }
        }
    }
    CursorImage::new(size, size, cx, size / 2, pixels)
}
