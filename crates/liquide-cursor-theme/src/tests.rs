use crate::cursor::{AnimatedCursor, CursorImage, CursorShape};
use crate::theme::{CursorTheme, CursorThemeManager, parse_theme_file};

#[test]
fn from_css_round_trip() {
    let css_names = [
        "default",
        "auto",
        "pointer",
        "text",
        "crosshair",
        "move",
        "wait",
        "progress",
        "help",
        "not-allowed",
        "grab",
        "grabbing",
        "n-resize",
        "s-resize",
        "e-resize",
        "w-resize",
        "ne-resize",
        "nw-resize",
        "se-resize",
        "sw-resize",
        "ns-resize",
        "ew-resize",
        "nesw-resize",
        "nwse-resize",
        "row-resize",
        "col-resize",
        "zoom-in",
        "zoom-out",
        "copy",
        "alias",
        "context-menu",
        "cell",
        "vertical-text",
        "no-drop",
        "none",
    ];
    for name in css_names {
        assert!(
            CursorShape::from_css(name).is_some(),
            "from_css({name:?}) returned None"
        );
    }
}

#[test]
fn from_css_unknown_returns_none() {
    assert!(CursorShape::from_css("banana").is_none());
    assert!(CursorShape::from_css("").is_none());
}

#[test]
fn x11_name_never_empty() {
    for shape in CursorShape::all() {
        let name = shape.x11_name();
        assert!(!name.is_empty(), "x11_name for {shape:?} is empty");
    }
}

#[test]
fn win32_id_never_zero() {
    for shape in CursorShape::all() {
        let id = shape.win32_id();
        assert!(id > 0, "win32_id for {shape:?} is zero");
    }
}

#[test]
fn all_shapes_count() {
    assert_eq!(CursorShape::all().len(), 34);
}

#[test]
fn solid_square_pixel_count() {
    let img = CursorImage::solid_square(16, 255, 0, 0);
    assert_eq!(img.width, 16);
    assert_eq!(img.height, 16);
    assert_eq!(img.pixels.len(), 16 * 16 * 4);
    // Check first pixel is red
    assert_eq!(img.pixels[0], 255);
    assert_eq!(img.pixels[1], 0);
    assert_eq!(img.pixels[2], 0);
    assert_eq!(img.pixels[3], 255);
}

#[test]
fn solid_square_all_pixels_correct() {
    let img = CursorImage::solid_square(4, 10, 20, 30);
    for i in 0..(4 * 4) {
        let off = i * 4;
        assert_eq!(img.pixels[off], 10);
        assert_eq!(img.pixels[off + 1], 20);
        assert_eq!(img.pixels[off + 2], 30);
        assert_eq!(img.pixels[off + 3], 255);
    }
}

#[test]
fn animated_cursor_single_frame_no_advance() {
    let frame = CursorImage::solid_square(8, 0, 0, 0);
    let mut anim = AnimatedCursor::new(vec![frame], vec![100]);
    assert!(!anim.tick(200));
    assert_eq!(anim.current_frame, 0);
}

#[test]
fn animated_cursor_tick_advances() {
    let f1 = CursorImage::solid_square(8, 255, 0, 0);
    let f2 = CursorImage::solid_square(8, 0, 255, 0);
    let f3 = CursorImage::solid_square(8, 0, 0, 255);
    let mut anim = AnimatedCursor::new(vec![f1, f2, f3], vec![100, 100, 100]);

    assert_eq!(anim.current_frame, 0);
    assert!(!anim.tick(50)); // not enough time
    assert_eq!(anim.current_frame, 0);
    assert!(anim.tick(50)); // exactly 100ms total
    assert_eq!(anim.current_frame, 1);
}

#[test]
fn animated_cursor_wraps_around() {
    let f1 = CursorImage::solid_square(8, 255, 0, 0);
    let f2 = CursorImage::solid_square(8, 0, 255, 0);
    let mut anim = AnimatedCursor::new(vec![f1, f2], vec![50, 50]);

    anim.tick(50); // frame 0 -> 1
    assert_eq!(anim.current_frame, 1);
    anim.tick(50); // frame 1 -> 0 (wrap)
    assert_eq!(anim.current_frame, 0);
}

#[test]
fn animated_cursor_current_image() {
    let f1 = CursorImage::solid_square(8, 255, 0, 0);
    let f2 = CursorImage::solid_square(8, 0, 255, 0);
    let anim = AnimatedCursor::new(vec![f1, f2], vec![100, 100]);
    let img = anim.current_image().unwrap();
    assert_eq!(img.pixels[0], 255); // red frame
}

#[test]
fn animated_cursor_frame_count() {
    let f1 = CursorImage::solid_square(8, 0, 0, 0);
    let f2 = CursorImage::solid_square(8, 0, 0, 0);
    let f3 = CursorImage::solid_square(8, 0, 0, 0);
    let anim = AnimatedCursor::new(vec![f1, f2, f3], vec![100, 100, 100]);
    assert_eq!(anim.frame_count(), 3);
}

#[test]
fn theme_add_and_get_cursor() {
    let mut theme = CursorTheme::new("test");
    let img = CursorImage::solid_square(24, 255, 255, 255);
    theme.add_cursor(CursorShape::Default, img);

    assert!(theme.has_cursor(CursorShape::Default));
    assert!(!theme.has_cursor(CursorShape::Pointer));

    let cursor = theme.get_cursor(CursorShape::Default, 24).unwrap();
    assert_eq!(cursor.width, 24);
}

#[test]
fn theme_closest_size() {
    let mut theme = CursorTheme::new("test");

    let mut img16 = CursorImage::solid_square(16, 0, 0, 0);
    img16.nominal_size = 16;
    let mut img32 = CursorImage::solid_square(32, 0, 0, 0);
    img32.nominal_size = 32;
    let mut img48 = CursorImage::solid_square(48, 0, 0, 0);
    img48.nominal_size = 48;

    theme.add_cursor(CursorShape::Default, img16);
    theme.add_cursor(CursorShape::Default, img32);
    theme.add_cursor(CursorShape::Default, img48);

    // Request 24 — closest is 16 (distance 8) vs 32 (distance 8), either is acceptable
    let cursor = theme.get_cursor(CursorShape::Default, 24).unwrap();
    assert!(cursor.nominal_size == 16 || cursor.nominal_size == 32);

    // Request 30 — closest is 32
    let cursor = theme.get_cursor(CursorShape::Default, 30).unwrap();
    assert_eq!(cursor.nominal_size, 32);

    // Request 50 — closest is 48
    let cursor = theme.get_cursor(CursorShape::Default, 50).unwrap();
    assert_eq!(cursor.nominal_size, 48);
}

#[test]
fn theme_available_sizes() {
    let mut theme = CursorTheme::new("test");

    let mut img16 = CursorImage::solid_square(16, 0, 0, 0);
    img16.nominal_size = 16;
    let mut img32 = CursorImage::solid_square(32, 0, 0, 0);
    img32.nominal_size = 32;

    theme.add_cursor(CursorShape::Pointer, img16);
    theme.add_cursor(CursorShape::Pointer, img32);

    let sizes = theme.available_sizes(CursorShape::Pointer);
    assert_eq!(sizes.len(), 2);
    assert!(sizes.contains(&16));
    assert!(sizes.contains(&32));

    // No sizes for a shape that doesn't exist
    assert!(theme.available_sizes(CursorShape::Wait).is_empty());
}

#[test]
fn theme_shape_count() {
    let mut theme = CursorTheme::new("test");
    assert_eq!(theme.shape_count(), 0);

    theme.add_cursor(CursorShape::Default, CursorImage::solid_square(24, 0, 0, 0));
    assert_eq!(theme.shape_count(), 1);

    theme.add_cursor(CursorShape::Pointer, CursorImage::solid_square(24, 0, 0, 0));
    assert_eq!(theme.shape_count(), 2);

    // Adding another size to existing shape doesn't increase count
    theme.add_cursor(CursorShape::Default, CursorImage::solid_square(32, 0, 0, 0));
    assert_eq!(theme.shape_count(), 2);
}

#[test]
fn manager_builtin_theme_exists() {
    let mgr = CursorThemeManager::new();
    assert_eq!(mgr.active_theme(), "default");

    // Builtin theme should have at least the arrow cursor
    let cursor = mgr.get_cursor(CursorShape::Default);
    assert!(cursor.is_some());
}

#[test]
fn manager_set_active() {
    let mut mgr = CursorThemeManager::new();
    assert!(mgr.set_active("default"));
    assert!(!mgr.set_active("nonexistent-theme"));
    assert_eq!(mgr.active_theme(), "default");
}

#[test]
fn manager_get_cursor_fallback() {
    let mut mgr = CursorThemeManager::new();

    // Add a custom theme without a Pointer cursor
    let mut custom = CursorTheme::new("custom");
    custom.add_cursor(CursorShape::Text, CursorImage::solid_square(24, 0, 0, 0));
    mgr.themes.insert("custom".to_string(), custom);
    mgr.set_active("custom");

    // Text cursor should come from "custom"
    assert!(mgr.get_cursor(CursorShape::Text).is_some());

    // Default cursor should fall back to builtin "default" theme
    let fallback = mgr.get_cursor(CursorShape::Default);
    assert!(fallback.is_some());
}

#[test]
fn manager_list_themes() {
    let mgr = CursorThemeManager::new();
    let themes = mgr.list_themes();
    assert!(!themes.is_empty());
    assert!(themes.iter().any(|(name, _)| *name == "default"));
}

#[test]
fn manager_set_default_size() {
    let mut mgr = CursorThemeManager::new();
    mgr.set_default_size(48);
    // Should still find cursors (builtin theme has size 24, closest match)
    assert!(mgr.get_cursor(CursorShape::Default).is_some());
}

#[test]
fn parse_theme_file_extracts_metadata() {
    let content = "\
[Icon Theme]
Name=My Cursors
Comment=A cool cursor theme
Inherits=default
Size=32
";
    let mut theme = CursorTheme::new("test");
    parse_theme_file(content, &mut theme);

    assert_eq!(theme.display_name, "My Cursors");
    assert_eq!(theme.comment, "A cool cursor theme");
    assert_eq!(theme.inherits.as_deref(), Some("default"));
    assert_eq!(theme.default_size, 32);
}

#[test]
fn parse_theme_file_partial() {
    let content = "Name=Only Name\n";
    let mut theme = CursorTheme::new("test");
    parse_theme_file(content, &mut theme);

    assert_eq!(theme.display_name, "Only Name");
    assert_eq!(theme.comment, ""); // unchanged
    assert!(theme.inherits.is_none());
    assert_eq!(theme.default_size, 24); // default unchanged
}

#[test]
fn builtin_theme_has_standard_cursors() {
    let theme = crate::builtin::create_builtin_theme();
    assert!(theme.has_cursor(CursorShape::Default));
    assert!(theme.has_cursor(CursorShape::Pointer));
    assert!(theme.has_cursor(CursorShape::Text));
    assert!(theme.has_cursor(CursorShape::Crosshair));
    assert!(theme.has_cursor(CursorShape::Move));
    assert!(theme.has_cursor(CursorShape::ResizeNS));
    assert!(theme.has_cursor(CursorShape::ResizeEW));
    assert!(theme.has_cursor(CursorShape::NotAllowed));
    assert!(theme.has_cursor(CursorShape::Wait));
    assert_eq!(theme.shape_count(), 9);
}

#[test]
fn builtin_cursors_have_valid_pixels() {
    let theme = crate::builtin::create_builtin_theme();
    let cursor = theme.get_cursor(CursorShape::Default, 24).unwrap();
    assert_eq!(cursor.width, 24);
    assert_eq!(cursor.height, 24);
    assert_eq!(cursor.pixels.len(), 24 * 24 * 4);
    // Should have at least some non-transparent pixels
    let has_opaque = cursor.pixels.chunks(4).any(|px| px[3] > 0);
    assert!(has_opaque, "Arrow cursor has no visible pixels");
}

#[test]
fn cursor_image_new_sets_nominal_size() {
    let img = CursorImage::new(32, 32, 0, 0, vec![0u8; 32 * 32 * 4]);
    assert_eq!(img.nominal_size, 32);
}
