/// A single scroll snap point.
#[derive(Debug, Clone, Copy)]
pub struct SnapPoint {
    /// Scroll offset for this snap point.
    pub offset: f32,
    /// Alignment of this snap point.
    pub alignment: SnapAlignment,
}

/// How a snap point aligns within the viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapAlignment {
    /// Snap point aligns to the start of the viewport.
    Start,
    /// Snap point aligns to the center of the viewport.
    Center,
    /// Snap point aligns to the end of the viewport.
    End,
}

/// Scroll snap behavior type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapType {
    /// Always snap to the nearest snap point after scrolling ends.
    Mandatory,
    /// Only snap if the scroll position is within the proximity threshold.
    Proximity,
}

/// Configuration for scroll snapping on one axis.
#[derive(Debug, Clone)]
pub struct SnapConfig {
    /// Snap behavior type.
    pub snap_type: SnapType,
    /// Snap points along this axis.
    pub points: Vec<SnapPoint>,
    /// For `Proximity` type: maximum distance from a snap point to trigger snapping.
    pub proximity_threshold: f32,
}

impl SnapConfig {
    /// Create a new snap configuration.
    pub fn new(snap_type: SnapType, proximity_threshold: f32) -> Self {
        Self {
            snap_type,
            points: Vec::new(),
            proximity_threshold,
        }
    }

    /// Add a snap point.
    pub fn add_point(&mut self, offset: f32, alignment: SnapAlignment) {
        self.points.push(SnapPoint { offset, alignment });
    }
}

/// Find the best snap target given a current scroll position and velocity.
///
/// - `current`: the current scroll offset.
/// - `velocity`: the scroll velocity (px/ms). Positive = scrolling down/right.
/// - `viewport_size`: the viewport dimension (width or height) for alignment calculations.
/// - `config`: the snap configuration.
///
/// Returns `Some(target_offset)` if a snap point was found, `None` otherwise.
pub fn find_snap_target(
    current: f32,
    velocity: f32,
    viewport_size: f32,
    config: &SnapConfig,
) -> Option<f32> {
    if config.points.is_empty() {
        return None;
    }

    // Compute the effective scroll position for each snap point, adjusting for alignment.
    let mut candidates: Vec<(f32, f32)> = config
        .points
        .iter()
        .map(|p| {
            let effective_offset = match p.alignment {
                SnapAlignment::Start => p.offset,
                SnapAlignment::Center => p.offset - viewport_size * 0.5,
                SnapAlignment::End => p.offset - viewport_size,
            };
            (effective_offset, (effective_offset - current).abs())
        })
        .collect();

    // Filter by velocity direction: don't snap backwards against scroll direction.
    // If velocity is meaningful (not near zero), only consider snap points
    // in the direction of travel.
    let velocity_threshold = 0.005; // px/ms
    if velocity.abs() > velocity_threshold {
        let directional: Vec<(f32, f32)> = candidates
            .iter()
            .filter(|(offset, _)| {
                if velocity > 0.0 {
                    *offset >= current - 1.0 // Allow tiny backwards tolerance
                } else {
                    *offset <= current + 1.0
                }
            })
            .copied()
            .collect();

        if !directional.is_empty() {
            candidates = directional;
        }
        // If all candidates are behind us, fall through to nearest.
    }

    // Sort by distance to current position.
    candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    let best = candidates.first()?;

    match config.snap_type {
        SnapType::Mandatory => Some(best.0),
        SnapType::Proximity => {
            if best.1 <= config.proximity_threshold {
                Some(best.0)
            } else {
                None
            }
        }
    }
}
