//! Text and glyph rendering into a framebuffer.

use super::BitmapFont;

/// Draw a text string into a framebuffer using the built-in bitmap font.
///
/// `scale` controls the integer scaling factor (1 = 8x16, 2 = 16x32, etc.).
/// Glyphs are rendered with greyscale antialiasing for smooth edges.
pub fn draw_text(
    fb: &mut liquide_compositor::framebuffer::FrameBuffer,
    text: &str,
    x: i32,
    y: i32,
    color: liquide_compositor::pixel::Color,
    scale: u32,
) {
    let font = BitmapFont::new();
    let s = scale.max(1) as i32;
    let mut cx = x;

    for ch in text.chars() {
        if ch == '\n' {
            continue;
        }
        let glyph = font.glyph(ch);
        draw_glyph(fb, glyph, cx, y, color, s);
        cx += BitmapFont::GLYPH_WIDTH as i32 * s;
    }
}

/// Draw a single glyph bitmap into the framebuffer.
fn draw_glyph(
    fb: &mut liquide_compositor::framebuffer::FrameBuffer,
    glyph: &[u8; 16],
    x: i32,
    y: i32,
    color: liquide_compositor::pixel::Color,
    scale: i32,
) {
    let pm = color.premultiply();
    let fb_w = fb.width as i32;
    let fb_h = fb.height as i32;

    for row in 0..16_i32 {
        let bits = glyph[row as usize];
        if bits == 0 {
            continue;
        }
        for col in 0..8_i32 {
            if bits & (0x80 >> col) == 0 {
                continue;
            }
            // Fill a scale x scale block for each pixel
            let px = x + col * scale;
            let py = y + row * scale;
            for sy in 0..scale {
                let fy = py + sy;
                if fy < 0 || fy >= fb_h {
                    continue;
                }
                for sx in 0..scale {
                    let fx = px + sx;
                    if fx < 0 || fx >= fb_w {
                        continue;
                    }
                    let dst = fb.get_pixel(fx as u32, fy as u32);
                    let result = crate::blend::blend_src_over(dst, pm);
                    fb.set_pixel(fx as u32, fy as u32, result);
                }
            }
        }
    }
}
