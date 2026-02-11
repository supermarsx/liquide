//! Tests for paint primitives.

use crate::geometry::{Corner, Point, Rect};
use crate::paint::{
    Brush, Color, FontWeight, GradientStop, PaintCommand, PaintContext, StrokeStyle, TextStyle,
};

// ---------------------------------------------------------------------------
// Color construction
// ---------------------------------------------------------------------------

#[test]
fn test_color_from_rgb() {
    let c = Color::from_rgb(255, 128, 0);
    assert_eq!(c.r, 255);
    assert_eq!(c.g, 128);
    assert_eq!(c.b, 0);
    assert_eq!(c.a, 255);
}

#[test]
fn test_color_from_rgba() {
    let c = Color::from_rgba(10, 20, 30, 40);
    assert_eq!(c.r, 10);
    assert_eq!(c.g, 20);
    assert_eq!(c.b, 30);
    assert_eq!(c.a, 40);
}

#[test]
fn test_color_black() {
    let c = Color::black();
    assert_eq!(c.r, 0);
    assert_eq!(c.g, 0);
    assert_eq!(c.b, 0);
    assert_eq!(c.a, 255);
}

#[test]
fn test_color_white() {
    let c = Color::white();
    assert_eq!(c.r, 255);
    assert_eq!(c.g, 255);
    assert_eq!(c.b, 255);
    assert_eq!(c.a, 255);
}

#[test]
fn test_color_transparent() {
    let c = Color::transparent();
    assert_eq!(c.a, 0);
}

#[test]
fn test_color_with_alpha() {
    let c = Color::from_rgb(255, 0, 0).with_alpha(128);
    assert_eq!(c.r, 255);
    assert_eq!(c.g, 0);
    assert_eq!(c.b, 0);
    assert_eq!(c.a, 128);
}

// ---------------------------------------------------------------------------
// Color from_hex
// ---------------------------------------------------------------------------

#[test]
fn test_color_from_hex_6_digit() {
    let c = Color::from_hex("#FF0000").unwrap();
    assert_eq!(c.r, 255);
    assert_eq!(c.g, 0);
    assert_eq!(c.b, 0);
    assert_eq!(c.a, 255);
}

#[test]
fn test_color_from_hex_8_digit() {
    let c = Color::from_hex("#FF000080").unwrap();
    assert_eq!(c.r, 255);
    assert_eq!(c.g, 0);
    assert_eq!(c.b, 0);
    assert_eq!(c.a, 128);
}

#[test]
fn test_color_from_hex_3_digit() {
    let c = Color::from_hex("#F00").unwrap();
    assert_eq!(c.r, 255);
    assert_eq!(c.g, 0);
    assert_eq!(c.b, 0);
    assert_eq!(c.a, 255);
}

#[test]
fn test_color_from_hex_no_hash() {
    let c = Color::from_hex("00FF00").unwrap();
    assert_eq!(c.r, 0);
    assert_eq!(c.g, 255);
    assert_eq!(c.b, 0);
}

#[test]
fn test_color_from_hex_invalid() {
    assert!(Color::from_hex("#ZZZZZZ").is_none());
    assert!(Color::from_hex("#12345").is_none()); // wrong length
    assert!(Color::from_hex("").is_none());
}

#[test]
fn test_color_from_hex_case_insensitive() {
    let upper = Color::from_hex("#AB12CD").unwrap();
    let lower = Color::from_hex("#ab12cd").unwrap();
    assert_eq!(upper, lower);
}

// ---------------------------------------------------------------------------
// Brush variants
// ---------------------------------------------------------------------------

#[test]
fn test_brush_solid() {
    let brush = Brush::Solid(Color::white());
    match brush {
        Brush::Solid(c) => assert_eq!(c, Color::white()),
        _ => panic!("expected Solid"),
    }
}

#[test]
fn test_brush_linear_gradient() {
    let brush = Brush::LinearGradient {
        start: Point::new(0.0, 0.0),
        end: Point::new(100.0, 0.0),
        stops: vec![
            GradientStop {
                offset: 0.0,
                color: Color::black(),
            },
            GradientStop {
                offset: 1.0,
                color: Color::white(),
            },
        ],
    };
    match brush {
        Brush::LinearGradient { stops, .. } => assert_eq!(stops.len(), 2),
        _ => panic!("expected LinearGradient"),
    }
}

#[test]
fn test_brush_radial_gradient() {
    let brush = Brush::RadialGradient {
        center: Point::new(50.0, 50.0),
        radius: 50.0,
        stops: vec![GradientStop {
            offset: 0.0,
            color: Color::from_rgb(255, 0, 0),
        }],
    };
    match brush {
        Brush::RadialGradient { center, radius, .. } => {
            assert_eq!(center.x, 50.0);
            assert_eq!(radius, 50.0);
        }
        _ => panic!("expected RadialGradient"),
    }
}

// ---------------------------------------------------------------------------
// StrokeStyle
// ---------------------------------------------------------------------------

#[test]
fn test_stroke_style_new() {
    let s = StrokeStyle::new(2.0, Color::from_rgb(255, 0, 0));
    assert_eq!(s.width, 2.0);
    assert_eq!(s.color, Color::from_rgb(255, 0, 0));
    assert!(s.dash_pattern.is_none());
}

#[test]
fn test_stroke_style_default() {
    let s = StrokeStyle::default();
    assert_eq!(s.width, 1.0);
    assert_eq!(s.color, Color::black());
}

// ---------------------------------------------------------------------------
// TextStyle
// ---------------------------------------------------------------------------

#[test]
fn test_text_style_default() {
    let ts = TextStyle::default();
    assert_eq!(ts.font_family, "sans-serif");
    assert_eq!(ts.font_size, 14.0);
    assert_eq!(ts.font_weight, FontWeight::Regular);
    assert_eq!(ts.color, Color::black());
    assert!(ts.line_height.is_none());
    assert!(ts.letter_spacing.is_none());
}

#[test]
fn test_font_weight_default() {
    assert_eq!(FontWeight::default(), FontWeight::Regular);
}

// ---------------------------------------------------------------------------
// PaintContext
// ---------------------------------------------------------------------------

#[test]
fn test_paint_context_new() {
    let clip = Rect::new(0.0, 0.0, 800.0, 600.0);
    let ctx = PaintContext::new(clip);
    assert_eq!(ctx.clip_rect, clip);
    assert!(ctx.commands().is_empty());
}

#[test]
fn test_paint_context_fill_rect() {
    let mut ctx = PaintContext::new(Rect::new(0.0, 0.0, 100.0, 100.0));
    ctx.fill_rect(Rect::new(10.0, 10.0, 50.0, 50.0), Brush::Solid(Color::white()));
    assert_eq!(ctx.commands().len(), 1);
    match &ctx.commands()[0] {
        PaintCommand::FillRect { rect, .. } => {
            assert_eq!(rect.x, 10.0);
        }
        _ => panic!("expected FillRect"),
    }
}

#[test]
fn test_paint_context_stroke_rect() {
    let mut ctx = PaintContext::new(Rect::new(0.0, 0.0, 100.0, 100.0));
    ctx.stroke_rect(
        Rect::new(0.0, 0.0, 50.0, 50.0),
        StrokeStyle::new(1.0, Color::black()),
    );
    assert_eq!(ctx.commands().len(), 1);
    assert!(matches!(ctx.commands()[0], PaintCommand::StrokeRect { .. }));
}

#[test]
fn test_paint_context_fill_rounded_rect() {
    let mut ctx = PaintContext::new(Rect::new(0.0, 0.0, 100.0, 100.0));
    ctx.fill_rounded_rect(
        Rect::new(0.0, 0.0, 50.0, 50.0),
        Corner::all(5.0),
        Brush::Solid(Color::black()),
    );
    assert!(matches!(
        ctx.commands()[0],
        PaintCommand::FillRoundedRect { .. }
    ));
}

#[test]
fn test_paint_context_draw_text() {
    let mut ctx = PaintContext::new(Rect::new(0.0, 0.0, 100.0, 100.0));
    ctx.draw_text("Hello", 10.0, 20.0, TextStyle::default());
    match &ctx.commands()[0] {
        PaintCommand::DrawText { text, x, y, .. } => {
            assert_eq!(text, "Hello");
            assert_eq!(*x, 10.0);
            assert_eq!(*y, 20.0);
        }
        _ => panic!("expected DrawText"),
    }
}

#[test]
fn test_paint_context_draw_line() {
    let mut ctx = PaintContext::new(Rect::new(0.0, 0.0, 100.0, 100.0));
    ctx.draw_line(0.0, 0.0, 100.0, 100.0, StrokeStyle::default());
    assert!(matches!(ctx.commands()[0], PaintCommand::DrawLine { .. }));
}

#[test]
fn test_paint_context_fill_circle() {
    let mut ctx = PaintContext::new(Rect::new(0.0, 0.0, 100.0, 100.0));
    ctx.fill_circle(50.0, 50.0, 25.0, Brush::Solid(Color::from_rgb(0, 0, 255)));
    assert!(matches!(
        ctx.commands()[0],
        PaintCommand::FillCircle { .. }
    ));
}

#[test]
fn test_paint_context_stroke_circle() {
    let mut ctx = PaintContext::new(Rect::new(0.0, 0.0, 100.0, 100.0));
    ctx.stroke_circle(50.0, 50.0, 25.0, StrokeStyle::default());
    assert!(matches!(
        ctx.commands()[0],
        PaintCommand::StrokeCircle { .. }
    ));
}

#[test]
fn test_paint_context_push_pop_clip() {
    let original_clip = Rect::new(0.0, 0.0, 100.0, 100.0);
    let mut ctx = PaintContext::new(original_clip);

    let inner_clip = Rect::new(10.0, 10.0, 50.0, 50.0);
    ctx.push_clip(inner_clip);
    assert_eq!(ctx.clip_rect, inner_clip);

    ctx.pop_clip();
    assert_eq!(ctx.clip_rect, original_clip);
    assert_eq!(ctx.commands().len(), 2);
}

#[test]
fn test_paint_context_into_commands() {
    let mut ctx = PaintContext::new(Rect::new(0.0, 0.0, 100.0, 100.0));
    ctx.fill_rect(Rect::new(0.0, 0.0, 10.0, 10.0), Brush::Solid(Color::white()));
    ctx.fill_rect(Rect::new(10.0, 10.0, 10.0, 10.0), Brush::Solid(Color::black()));
    let commands = ctx.into_commands();
    assert_eq!(commands.len(), 2);
}

#[test]
fn test_paint_context_multiple_commands() {
    let mut ctx = PaintContext::new(Rect::new(0.0, 0.0, 800.0, 600.0));
    ctx.fill_rect(Rect::new(0.0, 0.0, 800.0, 600.0), Brush::Solid(Color::white()));
    ctx.stroke_rect(Rect::new(10.0, 10.0, 100.0, 100.0), StrokeStyle::default());
    ctx.draw_text("Hello World", 20.0, 30.0, TextStyle::default());
    ctx.draw_line(0.0, 0.0, 800.0, 600.0, StrokeStyle::new(2.0, Color::from_rgb(255, 0, 0)));
    assert_eq!(ctx.commands().len(), 4);
}
