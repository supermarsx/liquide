use crate::effects::Rect;

/// Which side of a rectangle an edge belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

/// Describes the source of a snap edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SnapEdge {
    /// Snap to a screen boundary.
    ScreenEdge(Side),
    /// Snap to another window's edge (window id + side).
    WindowEdge(u64, Side),
}

/// A candidate snap: which edge, the coordinate to snap to, and how far away.
#[derive(Debug, Clone)]
pub struct SnapResult {
    pub edge: SnapEdge,
    pub snap_pos: f32,
    pub distance: f32,
}

/// Configuration for edge snapping behaviour.
#[derive(Debug, Clone)]
pub struct SnapConfig {
    pub enabled: bool,
    /// Maximum distance (px) at which snapping activates.
    pub threshold: f32,
    /// Resistance factor — higher values make it harder to pull away from an edge.
    pub resistance_px: f32,
    /// Snap to the edges of the screen work-area.
    pub screen_edge_snap: bool,
    /// Snap to the edges of other windows.
    pub window_edge_snap: bool,
}

impl Default for SnapConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold: 12.0,
            resistance_px: 8.0,
            screen_edge_snap: true,
            window_edge_snap: true,
        }
    }
}

/// Stateless helper for edge snapping and magnetic alignment.
pub struct EdgeSnapper;

impl EdgeSnapper {
    /// Find all snap candidates for a window at `(x, y, w, h)` given a list of
    /// other windows and the screen work-area.  Returns only candidates within
    /// `config.threshold`.
    pub fn find_snap(
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        others: &[(u64, Rect)],
        screen: Rect,
        config: &SnapConfig,
    ) -> Vec<SnapResult> {
        if !config.enabled {
            return Vec::new();
        }

        let mut results = Vec::new();
        let threshold = config.threshold;

        // Screen-edge snapping
        if config.screen_edge_snap {
            let screen_right = screen.x + screen.width;
            let screen_bottom = screen.y + screen.height;

            // Left edge of window → left edge of screen
            let d = (x - screen.x).abs();
            if d < threshold {
                results.push(SnapResult {
                    edge: SnapEdge::ScreenEdge(Side::Left),
                    snap_pos: screen.x,
                    distance: d,
                });
            }
            // Right edge of window → right edge of screen
            let d = ((x + w) - screen_right).abs();
            if d < threshold {
                results.push(SnapResult {
                    edge: SnapEdge::ScreenEdge(Side::Right),
                    snap_pos: screen_right - w,
                    distance: d,
                });
            }
            // Top edge of window → top edge of screen
            let d = (y - screen.y).abs();
            if d < threshold {
                results.push(SnapResult {
                    edge: SnapEdge::ScreenEdge(Side::Top),
                    snap_pos: screen.y,
                    distance: d,
                });
            }
            // Bottom edge of window → bottom edge of screen
            let d = ((y + h) - screen_bottom).abs();
            if d < threshold {
                results.push(SnapResult {
                    edge: SnapEdge::ScreenEdge(Side::Bottom),
                    snap_pos: screen_bottom - h,
                    distance: d,
                });
            }
        }

        // Window-edge snapping
        if config.window_edge_snap {
            for &(id, ref other) in others {
                let or = other.x + other.width;
                let ob = other.y + other.height;

                // Our left ↔ other right
                let d = (x - or).abs();
                if d < threshold {
                    results.push(SnapResult {
                        edge: SnapEdge::WindowEdge(id, Side::Right),
                        snap_pos: or,
                        distance: d,
                    });
                }
                // Our right ↔ other left
                let d = ((x + w) - other.x).abs();
                if d < threshold {
                    results.push(SnapResult {
                        edge: SnapEdge::WindowEdge(id, Side::Left),
                        snap_pos: other.x - w,
                        distance: d,
                    });
                }
                // Our top ↔ other bottom
                let d = (y - ob).abs();
                if d < threshold {
                    results.push(SnapResult {
                        edge: SnapEdge::WindowEdge(id, Side::Bottom),
                        snap_pos: ob,
                        distance: d,
                    });
                }
                // Our bottom ↔ other top
                let d = ((y + h) - other.y).abs();
                if d < threshold {
                    results.push(SnapResult {
                        edge: SnapEdge::WindowEdge(id, Side::Top),
                        snap_pos: other.y - h,
                        distance: d,
                    });
                }

                // Magnetic alignment: our left ↔ other left
                let d = (x - other.x).abs();
                if d < threshold {
                    results.push(SnapResult {
                        edge: SnapEdge::WindowEdge(id, Side::Left),
                        snap_pos: other.x,
                        distance: d,
                    });
                }
                // Magnetic alignment: our right ↔ other right
                let d = ((x + w) - or).abs();
                if d < threshold {
                    results.push(SnapResult {
                        edge: SnapEdge::WindowEdge(id, Side::Right),
                        snap_pos: or - w,
                        distance: d,
                    });
                }
                // Magnetic alignment: our top ↔ other top
                let d = (y - other.y).abs();
                if d < threshold {
                    results.push(SnapResult {
                        edge: SnapEdge::WindowEdge(id, Side::Top),
                        snap_pos: other.y,
                        distance: d,
                    });
                }
                // Magnetic alignment: our bottom ↔ other bottom
                let d = ((y + h) - ob).abs();
                if d < threshold {
                    results.push(SnapResult {
                        edge: SnapEdge::WindowEdge(id, Side::Bottom),
                        snap_pos: ob - h,
                        distance: d,
                    });
                }
            }
        }

        results
    }

    /// Given a set of snap candidates, pick the closest horizontal and vertical
    /// snap and return the adjusted `(x, y)`.
    pub fn apply_snap(x: f32, y: f32, _w: f32, _h: f32, snaps: &[SnapResult]) -> (f32, f32) {
        let mut best_x: Option<&SnapResult> = None;
        let mut best_y: Option<&SnapResult> = None;

        for s in snaps {
            match s.edge {
                SnapEdge::ScreenEdge(Side::Left)
                | SnapEdge::ScreenEdge(Side::Right)
                | SnapEdge::WindowEdge(_, Side::Left)
                | SnapEdge::WindowEdge(_, Side::Right) => {
                    if best_x.is_none() || s.distance < best_x.unwrap().distance {
                        best_x = Some(s);
                    }
                }
                SnapEdge::ScreenEdge(Side::Top)
                | SnapEdge::ScreenEdge(Side::Bottom)
                | SnapEdge::WindowEdge(_, Side::Top)
                | SnapEdge::WindowEdge(_, Side::Bottom) => {
                    if best_y.is_none() || s.distance < best_y.unwrap().distance {
                        best_y = Some(s);
                    }
                }
            }
        }

        let out_x = best_x.map_or(x, |s| s.snap_pos);
        let out_y = best_y.map_or(y, |s| s.snap_pos);
        (out_x, out_y)
    }

    /// Compute a resistance-adjusted velocity.  When the window is within
    /// `resistance` pixels of an edge, movement is dampened proportionally.
    ///
    /// Returns the adjusted velocity (same sign, smaller magnitude when close).
    pub fn edge_resistance(velocity: f32, distance_to_edge: f32, resistance: f32) -> f32 {
        if resistance <= 0.0 || distance_to_edge >= resistance {
            return velocity;
        }
        if distance_to_edge <= 0.0 {
            return 0.0;
        }
        // Linear ramp: full speed at `resistance` px away, zero at the edge.
        let factor = distance_to_edge / resistance;
        velocity * factor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> Rect {
        Rect::new(0.0, 0.0, 1920.0, 1080.0)
    }

    #[test]
    fn snap_to_screen_left_edge() {
        let cfg = SnapConfig::default();
        let results = EdgeSnapper::find_snap(3.0, 200.0, 400.0, 300.0, &[], screen(), &cfg);
        assert!(
            results
                .iter()
                .any(|r| matches!(r.edge, SnapEdge::ScreenEdge(Side::Left)))
        );
    }

    #[test]
    fn snap_to_screen_right_edge() {
        let cfg = SnapConfig::default();
        // Window right edge at 1920-5 = 1915
        let results = EdgeSnapper::find_snap(1515.0, 200.0, 400.0, 300.0, &[], screen(), &cfg);
        assert!(
            results
                .iter()
                .any(|r| matches!(r.edge, SnapEdge::ScreenEdge(Side::Right)))
        );
    }

    #[test]
    fn snap_to_screen_top_edge() {
        let cfg = SnapConfig::default();
        let results = EdgeSnapper::find_snap(200.0, 5.0, 400.0, 300.0, &[], screen(), &cfg);
        assert!(
            results
                .iter()
                .any(|r| matches!(r.edge, SnapEdge::ScreenEdge(Side::Top)))
        );
    }

    #[test]
    fn snap_to_screen_bottom_edge() {
        let cfg = SnapConfig::default();
        // y + h = 775 + 300 = 1075, distance to 1080 = 5
        let results = EdgeSnapper::find_snap(200.0, 775.0, 400.0, 300.0, &[], screen(), &cfg);
        assert!(
            results
                .iter()
                .any(|r| matches!(r.edge, SnapEdge::ScreenEdge(Side::Bottom)))
        );
    }

    #[test]
    fn no_snap_when_far_from_edges() {
        let cfg = SnapConfig::default();
        let results = EdgeSnapper::find_snap(500.0, 400.0, 400.0, 300.0, &[], screen(), &cfg);
        assert!(results.is_empty());
    }

    #[test]
    fn snap_disabled_returns_empty() {
        let cfg = SnapConfig {
            enabled: false,
            ..Default::default()
        };
        let results = EdgeSnapper::find_snap(3.0, 3.0, 400.0, 300.0, &[], screen(), &cfg);
        assert!(results.is_empty());
    }

    #[test]
    fn snap_to_other_window_edge() {
        let cfg = SnapConfig::default();
        let others = vec![(42, Rect::new(500.0, 100.0, 400.0, 300.0))];
        // Our right edge at 497, other left at 500 → distance 3
        let results = EdgeSnapper::find_snap(97.0, 150.0, 400.0, 300.0, &others, screen(), &cfg);
        assert!(
            results
                .iter()
                .any(|r| matches!(r.edge, SnapEdge::WindowEdge(42, _)))
        );
    }

    #[test]
    fn magnetic_alignment_left_edges() {
        let cfg = SnapConfig::default();
        let others = vec![(10, Rect::new(200.0, 100.0, 400.0, 300.0))];
        // Our left at 205, other left at 200 → distance 5
        let results = EdgeSnapper::find_snap(205.0, 500.0, 300.0, 200.0, &others, screen(), &cfg);
        let aligned = results.iter().any(|r| {
            matches!(r.edge, SnapEdge::WindowEdge(10, Side::Left))
                && (r.snap_pos - 200.0).abs() < 1e-3
        });
        assert!(aligned);
    }

    #[test]
    fn apply_snap_picks_closest() {
        let snaps = vec![
            SnapResult {
                edge: SnapEdge::ScreenEdge(Side::Left),
                snap_pos: 0.0,
                distance: 5.0,
            },
            SnapResult {
                edge: SnapEdge::WindowEdge(1, Side::Right),
                snap_pos: 10.0,
                distance: 2.0,
            },
            SnapResult {
                edge: SnapEdge::ScreenEdge(Side::Top),
                snap_pos: 0.0,
                distance: 3.0,
            },
        ];
        let (sx, sy) = EdgeSnapper::apply_snap(12.0, 3.0, 400.0, 300.0, &snaps);
        assert!(
            (sx - 10.0).abs() < 1e-5,
            "should pick window edge (distance 2)"
        );
        assert!((sy - 0.0).abs() < 1e-5, "should pick screen top");
    }

    #[test]
    fn apply_snap_no_candidates_returns_original() {
        let (sx, sy) = EdgeSnapper::apply_snap(100.0, 200.0, 400.0, 300.0, &[]);
        assert!((sx - 100.0).abs() < 1e-5);
        assert!((sy - 200.0).abs() < 1e-5);
    }

    #[test]
    fn edge_resistance_full_speed_far_away() {
        let v = EdgeSnapper::edge_resistance(10.0, 20.0, 8.0);
        assert!((v - 10.0).abs() < 1e-5);
    }

    #[test]
    fn edge_resistance_zero_at_edge() {
        let v = EdgeSnapper::edge_resistance(10.0, 0.0, 8.0);
        assert!((v - 0.0).abs() < 1e-5);
    }

    #[test]
    fn edge_resistance_half_speed_halfway() {
        let v = EdgeSnapper::edge_resistance(10.0, 4.0, 8.0);
        assert!((v - 5.0).abs() < 1e-5);
    }

    #[test]
    fn edge_resistance_zero_resistance_passthrough() {
        let v = EdgeSnapper::edge_resistance(10.0, 2.0, 0.0);
        assert!((v - 10.0).abs() < 1e-5);
    }

    #[test]
    fn edge_resistance_negative_velocity() {
        let v = EdgeSnapper::edge_resistance(-8.0, 4.0, 8.0);
        assert!((v - (-4.0)).abs() < 1e-5);
    }

    #[test]
    fn snap_screen_only_ignores_windows() {
        let cfg = SnapConfig {
            window_edge_snap: false,
            ..Default::default()
        };
        let others = vec![(1, Rect::new(500.0, 100.0, 400.0, 300.0))];
        let results = EdgeSnapper::find_snap(497.0, 150.0, 400.0, 300.0, &others, screen(), &cfg);
        assert!(
            !results
                .iter()
                .any(|r| matches!(r.edge, SnapEdge::WindowEdge(_, _)))
        );
    }

    #[test]
    fn snap_windows_only_ignores_screen() {
        let cfg = SnapConfig {
            screen_edge_snap: false,
            ..Default::default()
        };
        let results = EdgeSnapper::find_snap(3.0, 3.0, 400.0, 300.0, &[], screen(), &cfg);
        assert!(results.is_empty());
    }
}
