//! Built-in vector icons for the desktop shell.
//!
//! Each icon is defined as a small set of drawing primitives (filled rects,
//! circles, lines, and rounded rects) with coordinates normalized to a
//! `0.0..1.0` unit square.  The renderer scales these to the desired pixel
//! size at draw time.
//!
//! Icons that contain internal detail (e.g. a terminal prompt drawn on top
//! of a filled body) store the body as the first shape; the renderer may
//! choose to draw subsequent shapes in a contrasting colour or as cutouts.

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Identifies a built-in vector icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IconId {
    /// Folder icon (rectangle with tab).
    FileManager,
    /// Terminal / console (rectangle with ">\_" prompt).
    Terminal,
    /// Globe / compass for a web browser (circle with crosshairs).
    Browser,
    /// Gear icon for system settings (circle with notches).
    Settings,
    /// Text document (rectangle with lines).
    TextEditor,
    /// Calendar (grid with header).
    Calendar,
    /// Single music note.
    Music,
    /// Camera (rectangle with circle lens).
    Camera,
    /// Envelope.
    Mail,
    /// Calculator (rectangle with grid of buttons).
    Calculator,
    /// Clock face (circle with hands).
    Clock,
    /// Wi-Fi signal arcs.
    Wifi,
    /// Battery indicator.
    Battery,
    /// Speaker with sound waves.
    Volume,
    /// Magnifying glass.
    Search,
    /// Power button (circle with line).
    Power,
    /// Bell / notification.
    Notification,
    /// Trash can.
    Trash,
}

/// A drawing primitive within an icon.
///
/// All coordinates are normalized to a `0.0..1.0` unit square.
#[derive(Debug, Clone, Copy)]
pub enum IconShape {
    /// Axis-aligned filled rectangle.
    FilledRect { x: f32, y: f32, w: f32, h: f32 },
    /// Filled circle.
    FilledCircle { cx: f32, cy: f32, r: f32 },
    /// Straight line segment with the given stroke width.
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        width: f32,
    },
    /// Filled rectangle with uniform corner rounding.
    RoundedRect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
    },
}

/// A built-in icon definition made up of one or more shapes.
///
/// The first shape is typically the outermost body of the icon; subsequent
/// shapes provide interior detail that the renderer may draw in a
/// contrasting colour.
pub struct IconDef {
    /// The shapes that compose this icon, drawn back-to-front.
    pub shapes: &'static [IconShape],
}

// ---------------------------------------------------------------------------
// Lookup
// ---------------------------------------------------------------------------

/// Return the static definition for the given icon.
#[must_use]
pub fn get_icon(id: IconId) -> &'static IconDef {
    match id {
        IconId::FileManager => &ICON_FILE_MANAGER,
        IconId::Terminal => &ICON_TERMINAL,
        IconId::Browser => &ICON_BROWSER,
        IconId::Settings => &ICON_SETTINGS,
        IconId::TextEditor => &ICON_TEXT_EDITOR,
        IconId::Calendar => &ICON_CALENDAR,
        IconId::Music => &ICON_MUSIC,
        IconId::Camera => &ICON_CAMERA,
        IconId::Mail => &ICON_MAIL,
        IconId::Calculator => &ICON_CALCULATOR,
        IconId::Clock => &ICON_CLOCK,
        IconId::Wifi => &ICON_WIFI,
        IconId::Battery => &ICON_BATTERY,
        IconId::Volume => &ICON_VOLUME,
        IconId::Search => &ICON_SEARCH,
        IconId::Power => &ICON_POWER,
        IconId::Notification => &ICON_NOTIFICATION,
        IconId::Trash => &ICON_TRASH,
    }
}

// ---------------------------------------------------------------------------
// Icon shape data
// ---------------------------------------------------------------------------

/// FileManager -- folder with tab protruding from the top-left.
static ICON_FILE_MANAGER: IconDef = IconDef {
    shapes: &[
        // Tab
        IconShape::FilledRect {
            x: 0.05,
            y: 0.15,
            w: 0.35,
            h: 0.12,
        },
        // Body
        IconShape::RoundedRect {
            x: 0.05,
            y: 0.25,
            w: 0.90,
            h: 0.60,
            r: 0.05,
        },
    ],
};

/// Terminal -- rounded rectangle body with a ">\_" prompt inside.
static ICON_TERMINAL: IconDef = IconDef {
    shapes: &[
        // Body
        IconShape::RoundedRect {
            x: 0.05,
            y: 0.10,
            w: 0.90,
            h: 0.80,
            r: 0.08,
        },
        // ">" upper stroke
        IconShape::Line {
            x1: 0.20,
            y1: 0.38,
            x2: 0.40,
            y2: 0.50,
            width: 0.07,
        },
        // ">" lower stroke
        IconShape::Line {
            x1: 0.40,
            y1: 0.50,
            x2: 0.20,
            y2: 0.62,
            width: 0.07,
        },
        // "_" cursor
        IconShape::Line {
            x1: 0.48,
            y1: 0.64,
            x2: 0.68,
            y2: 0.64,
            width: 0.06,
        },
    ],
};

/// Browser -- filled globe with latitude / longitude grid lines.
static ICON_BROWSER: IconDef = IconDef {
    shapes: &[
        // Globe body
        IconShape::FilledCircle {
            cx: 0.50,
            cy: 0.50,
            r: 0.42,
        },
        // Equator
        IconShape::Line {
            x1: 0.08,
            y1: 0.50,
            x2: 0.92,
            y2: 0.50,
            width: 0.04,
        },
        // Prime meridian
        IconShape::Line {
            x1: 0.50,
            y1: 0.08,
            x2: 0.50,
            y2: 0.92,
            width: 0.04,
        },
        // Upper latitude
        IconShape::Line {
            x1: 0.15,
            y1: 0.32,
            x2: 0.85,
            y2: 0.32,
            width: 0.03,
        },
        // Lower latitude
        IconShape::Line {
            x1: 0.15,
            y1: 0.68,
            x2: 0.85,
            y2: 0.68,
            width: 0.03,
        },
    ],
};

/// Settings -- gear shape built from a centre circle, axis-aligned bars,
/// and diagonal spokes creating an 8-tooth star.
static ICON_SETTINGS: IconDef = IconDef {
    shapes: &[
        // Centre hub
        IconShape::FilledCircle {
            cx: 0.50,
            cy: 0.50,
            r: 0.22,
        },
        // Vertical teeth (N / S)
        IconShape::FilledRect {
            x: 0.42,
            y: 0.06,
            w: 0.16,
            h: 0.88,
        },
        // Horizontal teeth (E / W)
        IconShape::FilledRect {
            x: 0.06,
            y: 0.42,
            w: 0.88,
            h: 0.16,
        },
        // Diagonal NW-SE
        IconShape::Line {
            x1: 0.20,
            y1: 0.20,
            x2: 0.80,
            y2: 0.80,
            width: 0.13,
        },
        // Diagonal NE-SW
        IconShape::Line {
            x1: 0.80,
            y1: 0.20,
            x2: 0.20,
            y2: 0.80,
            width: 0.13,
        },
    ],
};

/// TextEditor -- document rectangle with four horizontal text lines.
static ICON_TEXT_EDITOR: IconDef = IconDef {
    shapes: &[
        // Page body
        IconShape::RoundedRect {
            x: 0.10,
            y: 0.05,
            w: 0.80,
            h: 0.90,
            r: 0.05,
        },
        // Text line 1
        IconShape::Line {
            x1: 0.22,
            y1: 0.26,
            x2: 0.78,
            y2: 0.26,
            width: 0.05,
        },
        // Text line 2
        IconShape::Line {
            x1: 0.22,
            y1: 0.42,
            x2: 0.78,
            y2: 0.42,
            width: 0.05,
        },
        // Text line 3 (shorter)
        IconShape::Line {
            x1: 0.22,
            y1: 0.58,
            x2: 0.62,
            y2: 0.58,
            width: 0.05,
        },
        // Text line 4
        IconShape::Line {
            x1: 0.22,
            y1: 0.74,
            x2: 0.70,
            y2: 0.74,
            width: 0.05,
        },
    ],
};

/// Calendar -- body with coloured header bar, two hanging rings,
/// and a 2x2 grid of date cells.
static ICON_CALENDAR: IconDef = IconDef {
    shapes: &[
        // Body
        IconShape::RoundedRect {
            x: 0.08,
            y: 0.12,
            w: 0.84,
            h: 0.82,
            r: 0.06,
        },
        // Header bar
        IconShape::FilledRect {
            x: 0.08,
            y: 0.12,
            w: 0.84,
            h: 0.20,
        },
        // Left ring
        IconShape::Line {
            x1: 0.30,
            y1: 0.04,
            x2: 0.30,
            y2: 0.20,
            width: 0.06,
        },
        // Right ring
        IconShape::Line {
            x1: 0.70,
            y1: 0.04,
            x2: 0.70,
            y2: 0.20,
            width: 0.06,
        },
        // Horizontal grid divider
        IconShape::Line {
            x1: 0.08,
            y1: 0.56,
            x2: 0.92,
            y2: 0.56,
            width: 0.03,
        },
        // Vertical grid divider
        IconShape::Line {
            x1: 0.50,
            y1: 0.34,
            x2: 0.50,
            y2: 0.92,
            width: 0.03,
        },
    ],
};

/// Music -- single quaver: filled note head, vertical stem, and flag.
static ICON_MUSIC: IconDef = IconDef {
    shapes: &[
        // Note head
        IconShape::FilledCircle {
            cx: 0.38,
            cy: 0.72,
            r: 0.14,
        },
        // Stem
        IconShape::Line {
            x1: 0.51,
            y1: 0.72,
            x2: 0.51,
            y2: 0.12,
            width: 0.06,
        },
        // Flag
        IconShape::Line {
            x1: 0.51,
            y1: 0.12,
            x2: 0.74,
            y2: 0.32,
            width: 0.07,
        },
    ],
};

/// Camera -- rounded body with a circle lens and a viewfinder bump.
static ICON_CAMERA: IconDef = IconDef {
    shapes: &[
        // Body
        IconShape::RoundedRect {
            x: 0.05,
            y: 0.28,
            w: 0.90,
            h: 0.55,
            r: 0.06,
        },
        // Lens
        IconShape::FilledCircle {
            cx: 0.50,
            cy: 0.55,
            r: 0.16,
        },
        // Viewfinder bump
        IconShape::FilledRect {
            x: 0.35,
            y: 0.15,
            w: 0.30,
            h: 0.15,
        },
    ],
};

/// Mail -- envelope body with a V-shaped flap.
static ICON_MAIL: IconDef = IconDef {
    shapes: &[
        // Envelope body
        IconShape::RoundedRect {
            x: 0.05,
            y: 0.20,
            w: 0.90,
            h: 0.60,
            r: 0.04,
        },
        // Flap left edge
        IconShape::Line {
            x1: 0.05,
            y1: 0.20,
            x2: 0.50,
            y2: 0.55,
            width: 0.06,
        },
        // Flap right edge
        IconShape::Line {
            x1: 0.95,
            y1: 0.20,
            x2: 0.50,
            y2: 0.55,
            width: 0.06,
        },
    ],
};

/// Calculator -- tall rounded body, display screen, and grid dividers
/// suggesting a button pad.
static ICON_CALCULATOR: IconDef = IconDef {
    shapes: &[
        // Body
        IconShape::RoundedRect {
            x: 0.15,
            y: 0.05,
            w: 0.70,
            h: 0.90,
            r: 0.06,
        },
        // Display screen
        IconShape::FilledRect {
            x: 0.22,
            y: 0.14,
            w: 0.56,
            h: 0.16,
        },
        // Horizontal button divider
        IconShape::Line {
            x1: 0.15,
            y1: 0.56,
            x2: 0.85,
            y2: 0.56,
            width: 0.03,
        },
        // Vertical button divider
        IconShape::Line {
            x1: 0.50,
            y1: 0.38,
            x2: 0.50,
            y2: 0.92,
            width: 0.03,
        },
    ],
};

/// Clock -- filled face with a centre dot, minute hand, and hour hand.
static ICON_CLOCK: IconDef = IconDef {
    shapes: &[
        // Face
        IconShape::FilledCircle {
            cx: 0.50,
            cy: 0.50,
            r: 0.44,
        },
        // Centre pivot
        IconShape::FilledCircle {
            cx: 0.50,
            cy: 0.50,
            r: 0.05,
        },
        // Minute hand (pointing to 12)
        IconShape::Line {
            x1: 0.50,
            y1: 0.50,
            x2: 0.50,
            y2: 0.16,
            width: 0.05,
        },
        // Hour hand (pointing to ~2)
        IconShape::Line {
            x1: 0.50,
            y1: 0.50,
            x2: 0.76,
            y2: 0.38,
            width: 0.06,
        },
    ],
};

/// Wifi -- base dot with two nested chevron arcs radiating upward.
static ICON_WIFI: IconDef = IconDef {
    shapes: &[
        // Base dot
        IconShape::FilledCircle {
            cx: 0.50,
            cy: 0.80,
            r: 0.07,
        },
        // Inner arc left
        IconShape::Line {
            x1: 0.34,
            y1: 0.56,
            x2: 0.50,
            y2: 0.68,
            width: 0.07,
        },
        // Inner arc right
        IconShape::Line {
            x1: 0.66,
            y1: 0.56,
            x2: 0.50,
            y2: 0.68,
            width: 0.07,
        },
        // Outer arc left
        IconShape::Line {
            x1: 0.14,
            y1: 0.28,
            x2: 0.38,
            y2: 0.50,
            width: 0.07,
        },
        // Outer arc right
        IconShape::Line {
            x1: 0.86,
            y1: 0.28,
            x2: 0.62,
            y2: 0.50,
            width: 0.07,
        },
    ],
};

/// Battery -- horizontal body with a terminal nub and charge level fill.
static ICON_BATTERY: IconDef = IconDef {
    shapes: &[
        // Body
        IconShape::RoundedRect {
            x: 0.06,
            y: 0.28,
            w: 0.76,
            h: 0.44,
            r: 0.06,
        },
        // Positive terminal nub
        IconShape::FilledRect {
            x: 0.82,
            y: 0.38,
            w: 0.12,
            h: 0.24,
        },
        // Charge level indicator
        IconShape::FilledRect {
            x: 0.14,
            y: 0.36,
            w: 0.40,
            h: 0.28,
        },
    ],
};

/// Volume -- speaker box with a wider cone section and two sound-wave bars.
static ICON_VOLUME: IconDef = IconDef {
    shapes: &[
        // Speaker box (narrow)
        IconShape::FilledRect {
            x: 0.08,
            y: 0.36,
            w: 0.14,
            h: 0.28,
        },
        // Cone (wider)
        IconShape::FilledRect {
            x: 0.22,
            y: 0.22,
            w: 0.18,
            h: 0.56,
        },
        // Sound wave 1 (near)
        IconShape::Line {
            x1: 0.56,
            y1: 0.28,
            x2: 0.56,
            y2: 0.72,
            width: 0.06,
        },
        // Sound wave 2 (far)
        IconShape::Line {
            x1: 0.72,
            y1: 0.16,
            x2: 0.72,
            y2: 0.84,
            width: 0.06,
        },
    ],
};

/// Search -- magnifying glass: filled lens circle with a diagonal handle.
static ICON_SEARCH: IconDef = IconDef {
    shapes: &[
        // Lens
        IconShape::FilledCircle {
            cx: 0.40,
            cy: 0.38,
            r: 0.26,
        },
        // Handle
        IconShape::Line {
            x1: 0.58,
            y1: 0.56,
            x2: 0.88,
            y2: 0.88,
            width: 0.12,
        },
    ],
};

/// Power -- circle body with a vertical line extending above the top edge,
/// evoking the IEC 5009 power symbol.
static ICON_POWER: IconDef = IconDef {
    shapes: &[
        // Circle body (shifted slightly down so the line protrudes above)
        IconShape::FilledCircle {
            cx: 0.50,
            cy: 0.56,
            r: 0.34,
        },
        // Vertical bar (extends from above the circle into its centre)
        IconShape::Line {
            x1: 0.50,
            y1: 0.06,
            x2: 0.50,
            y2: 0.50,
            width: 0.10,
        },
    ],
};

/// Notification -- bell shape: domed top, rectangular body, flat rim,
/// and a small clapper ball.
static ICON_NOTIFICATION: IconDef = IconDef {
    shapes: &[
        // Dome
        IconShape::FilledCircle {
            cx: 0.50,
            cy: 0.30,
            r: 0.22,
        },
        // Body
        IconShape::FilledRect {
            x: 0.22,
            y: 0.30,
            w: 0.56,
            h: 0.38,
        },
        // Rim
        IconShape::FilledRect {
            x: 0.15,
            y: 0.64,
            w: 0.70,
            h: 0.10,
        },
        // Clapper
        IconShape::FilledCircle {
            cx: 0.50,
            cy: 0.82,
            r: 0.07,
        },
    ],
};

/// Trash -- lid with handle, can body, and two vertical ribs.
static ICON_TRASH: IconDef = IconDef {
    shapes: &[
        // Lid
        IconShape::FilledRect {
            x: 0.15,
            y: 0.15,
            w: 0.70,
            h: 0.08,
        },
        // Lid handle
        IconShape::FilledRect {
            x: 0.38,
            y: 0.05,
            w: 0.24,
            h: 0.12,
        },
        // Can body
        IconShape::RoundedRect {
            x: 0.22,
            y: 0.25,
            w: 0.56,
            h: 0.65,
            r: 0.04,
        },
        // Left rib
        IconShape::Line {
            x1: 0.38,
            y1: 0.32,
            x2: 0.38,
            y2: 0.82,
            width: 0.03,
        },
        // Right rib
        IconShape::Line {
            x1: 0.62,
            y1: 0.32,
            x2: 0.62,
            y2: 0.82,
            width: 0.03,
        },
    ],
};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_ICONS: [IconId; 18] = [
        IconId::FileManager,
        IconId::Terminal,
        IconId::Browser,
        IconId::Settings,
        IconId::TextEditor,
        IconId::Calendar,
        IconId::Music,
        IconId::Camera,
        IconId::Mail,
        IconId::Calculator,
        IconId::Clock,
        IconId::Wifi,
        IconId::Battery,
        IconId::Volume,
        IconId::Search,
        IconId::Power,
        IconId::Notification,
        IconId::Trash,
    ];

    /// Every icon must have between 2 and 6 shapes.
    #[test]
    fn all_icons_have_valid_shape_count() {
        for id in ALL_ICONS {
            let def = get_icon(id);
            assert!(
                def.shapes.len() >= 2,
                "{id:?} has only {} shape(s)",
                def.shapes.len(),
            );
            assert!(
                def.shapes.len() <= 6,
                "{id:?} has {} shapes (max 6)",
                def.shapes.len(),
            );
        }
    }

    /// All coordinates must stay within the 0.0..=1.0 unit square.
    #[test]
    fn all_coordinates_in_unit_square() {
        for id in ALL_ICONS {
            let def = get_icon(id);
            for (i, shape) in def.shapes.iter().enumerate() {
                match *shape {
                    IconShape::FilledRect { x, y, w, h } => {
                        assert!(x >= 0.0 && y >= 0.0, "{id:?} shape {i}: negative origin");
                        assert!(
                            x + w <= 1.01 && y + h <= 1.01,
                            "{id:?} shape {i}: rect exceeds unit square"
                        );
                    }
                    IconShape::FilledCircle { cx, cy, r } => {
                        assert!(r > 0.0, "{id:?} shape {i}: non-positive radius");
                        assert!(
                            cx - r >= -0.01 && cy - r >= -0.01,
                            "{id:?} shape {i}: circle extends below 0.0"
                        );
                        assert!(
                            cx + r <= 1.01 && cy + r <= 1.01,
                            "{id:?} shape {i}: circle exceeds unit square"
                        );
                    }
                    IconShape::Line {
                        x1,
                        y1,
                        x2,
                        y2,
                        width,
                    } => {
                        assert!(width > 0.0, "{id:?} shape {i}: non-positive line width");
                        assert!(
                            x1 >= 0.0 && y1 >= 0.0 && x2 >= 0.0 && y2 >= 0.0,
                            "{id:?} shape {i}: negative line endpoint"
                        );
                        assert!(
                            x1 <= 1.0 && y1 <= 1.0 && x2 <= 1.0 && y2 <= 1.0,
                            "{id:?} shape {i}: line endpoint exceeds 1.0"
                        );
                    }
                    IconShape::RoundedRect { x, y, w, h, r } => {
                        assert!(x >= 0.0 && y >= 0.0, "{id:?} shape {i}: negative origin");
                        assert!(
                            x + w <= 1.01 && y + h <= 1.01,
                            "{id:?} shape {i}: rounded rect exceeds unit square"
                        );
                        assert!(r >= 0.0, "{id:?} shape {i}: negative corner radius");
                    }
                }
            }
        }
    }
}
