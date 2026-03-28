//! Workspace switch animations.
//!
//! Provides transform computations for transitioning between workspace views.
//! Each transition style produces per-frame transforms for the outgoing and
//! incoming workspace surfaces, parameterized by a progress value (0.0 = showing
//! old workspace, 1.0 = showing new workspace).
//!
//! # Transition Styles
//!
//! - **Slide**: Horizontal or vertical slide (GNOME/KDE style).
//! - **Fade**: Cross-fade between workspaces.
//! - **Cube**: 3D cube rotation (Compiz style).
//! - **Stack**: Card stack push/pop effect (iOS style).

/// A 2D transform representation used for workspace transitions.
///
/// All transforms are relative to the workspace surface's origin. The
/// compositor applies these via the layer transform mechanism.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform2D {
    /// Horizontal translation in pixels.
    pub translate_x: f64,
    /// Vertical translation in pixels.
    pub translate_y: f64,
    /// Horizontal scale factor (1.0 = identity).
    pub scale_x: f64,
    /// Vertical scale factor (1.0 = identity).
    pub scale_y: f64,
    /// Opacity (0.0 = invisible, 1.0 = fully opaque).
    pub opacity: f64,
}

impl Transform2D {
    /// Identity transform (no translation, scale=1, full opacity).
    pub fn identity() -> Self {
        Self {
            translate_x: 0.0,
            translate_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            opacity: 1.0,
        }
    }

    /// Create a translated transform.
    pub fn translate(x: f64, y: f64) -> Self {
        Self {
            translate_x: x,
            translate_y: y,
            scale_x: 1.0,
            scale_y: 1.0,
            opacity: 1.0,
        }
    }

    /// Convert to a 2D affine matrix [a, b, c, d, tx, ty].
    pub fn to_affine(&self) -> [f32; 6] {
        [
            self.scale_x as f32,
            0.0,
            0.0,
            self.scale_y as f32,
            self.translate_x as f32,
            self.translate_y as f32,
        ]
    }
}

impl Default for Transform2D {
    fn default() -> Self {
        Self::identity()
    }
}

/// A 3D transform representation extending Transform2D with rotation.
///
/// Used for the cube transition where workspaces are faces of a rotating cube.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform3D {
    /// Horizontal translation in pixels.
    pub translate_x: f64,
    /// Vertical translation in pixels.
    pub translate_y: f64,
    /// Horizontal scale factor.
    pub scale_x: f64,
    /// Vertical scale factor.
    pub scale_y: f64,
    /// Opacity.
    pub opacity: f64,
    /// Y-axis rotation in radians. Positive = clockwise when viewed from above.
    pub rotate_y: f64,
    /// Perspective distance (pixels). Smaller = more dramatic perspective.
    /// Typical: 800-1200.
    pub perspective: f64,
}

impl Transform3D {
    /// Identity 3D transform.
    pub fn identity() -> Self {
        Self {
            translate_x: 0.0,
            translate_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            opacity: 1.0,
            rotate_y: 0.0,
            perspective: 1000.0,
        }
    }

    /// Flatten to a 2D approximation by projecting the Y rotation.
    ///
    /// This is a simplified perspective projection suitable for compositing
    /// when a full 3D pipeline is not available. The rotation is approximated
    /// as a horizontal scale and translation.
    pub fn to_2d_approx(&self, face_width: f64) -> Transform2D {
        let cos_r = self.rotate_y.cos();
        let sin_r = self.rotate_y.sin();

        // Perspective foreshortening factor.
        let depth = face_width * 0.5 * sin_r;
        let persp = if self.perspective > 0.0 {
            self.perspective / (self.perspective + depth)
        } else {
            1.0
        };

        Transform2D {
            translate_x: self.translate_x + face_width * 0.5 * (1.0 - cos_r * persp),
            translate_y: self.translate_y,
            scale_x: self.scale_x * cos_r.abs() * persp,
            scale_y: self.scale_y * persp,
            opacity: self.opacity,
        }
    }
}

impl Default for Transform3D {
    fn default() -> Self {
        Self::identity()
    }
}

/// The visual style used for workspace transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionStyle {
    /// Horizontal or vertical slide. Workspaces move side by side.
    Slide,
    /// Cross-fade between workspaces.
    Fade,
    /// 3D cube rotation (workspaces are faces of a cube).
    Cube,
    /// Card stack effect. Incoming workspace slides over outgoing.
    Stack,
}

/// Direction of the workspace transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionDirection {
    /// Moving to the next workspace (left/up).
    Forward,
    /// Moving to the previous workspace (right/down).
    Backward,
}

/// Workspace transition state and transform computation.
///
/// Holds the transition style, direction, and viewport dimensions. Call the
/// transform methods with a progress value (0.0-1.0) to get per-frame
/// transforms for the outgoing and incoming workspace surfaces.
pub struct WorkspaceTransition {
    /// Visual style.
    pub style: TransitionStyle,
    /// Transition direction.
    pub direction: TransitionDirection,
    /// Viewport width in pixels.
    pub viewport_width: f64,
    /// Viewport height in pixels.
    pub viewport_height: f64,
}

impl WorkspaceTransition {
    /// Create a new workspace transition.
    pub fn new(
        style: TransitionStyle,
        direction: TransitionDirection,
        viewport_width: f64,
        viewport_height: f64,
    ) -> Self {
        Self {
            style,
            direction,
            viewport_width,
            viewport_height,
        }
    }

    /// Compute transforms for both workspaces at the given progress.
    ///
    /// Returns `(outgoing, incoming)` transforms. At progress=0, outgoing is
    /// at identity and incoming is off-screen. At progress=1, outgoing is
    /// off-screen and incoming is at identity.
    pub fn compute(&self, progress: f64) -> (Transform2D, Transform2D) {
        let p = progress.clamp(0.0, 1.0);
        match self.style {
            TransitionStyle::Slide => self.compute_slide(p),
            TransitionStyle::Fade => self.compute_fade(p),
            TransitionStyle::Stack => self.compute_stack(p),
            TransitionStyle::Cube => {
                let (out_3d, in_3d) = self.compute_cube(p);
                (
                    out_3d.to_2d_approx(self.viewport_width),
                    in_3d.to_2d_approx(self.viewport_width),
                )
            }
        }
    }

    /// Slide transition: workspaces translate horizontally.
    fn compute_slide(&self, progress: f64) -> (Transform2D, Transform2D) {
        let (out, inc) = slide_transform(progress, self.direction, self.viewport_width);
        (out, inc)
    }

    /// Fade transition: cross-fade between workspaces.
    fn compute_fade(&self, progress: f64) -> (Transform2D, Transform2D) {
        let (out_alpha, in_alpha) = fade_transform(progress);
        let mut out = Transform2D::identity();
        out.opacity = out_alpha as f64;
        let mut inc = Transform2D::identity();
        inc.opacity = in_alpha as f64;
        (out, inc)
    }

    /// Stack transition: incoming slides over outgoing.
    fn compute_stack(&self, progress: f64) -> (Transform2D, Transform2D) {
        stack_transform(progress, self.direction, self.viewport_width)
    }

    /// Cube transition in 3D.
    fn compute_cube(&self, progress: f64) -> (Transform3D, Transform3D) {
        cube_transform(progress, self.direction, self.viewport_width)
    }

    /// Get the raw 3D transforms for the cube transition.
    ///
    /// Returns `None` if the style is not `Cube`.
    pub fn cube_transforms(&self, progress: f64) -> Option<(Transform3D, Transform3D)> {
        if self.style != TransitionStyle::Cube {
            return None;
        }
        Some(self.compute_cube(progress.clamp(0.0, 1.0)))
    }
}

/// Compute slide transforms for the given progress and direction.
///
/// `viewport_size` is the width (horizontal) or height (vertical) of the
/// viewport.
pub fn slide_transform(
    progress: f64,
    direction: TransitionDirection,
    viewport_size: f64,
) -> (Transform2D, Transform2D) {
    let sign = match direction {
        TransitionDirection::Forward => -1.0,
        TransitionDirection::Backward => 1.0,
    };

    let outgoing = Transform2D {
        translate_x: sign * progress * viewport_size,
        translate_y: 0.0,
        scale_x: 1.0,
        scale_y: 1.0,
        opacity: 1.0,
    };

    let incoming = Transform2D {
        translate_x: -sign * (1.0 - progress) * viewport_size,
        translate_y: 0.0,
        scale_x: 1.0,
        scale_y: 1.0,
        opacity: 1.0,
    };

    (outgoing, incoming)
}

/// Compute fade transforms for the given progress.
///
/// Returns `(outgoing_opacity, incoming_opacity)` as f32 values.
pub fn fade_transform(progress: f64) -> (f32, f32) {
    let p = progress.clamp(0.0, 1.0);
    let outgoing = (1.0 - p) as f32;
    let incoming = p as f32;
    (outgoing, incoming)
}

/// Compute 3D cube rotation transforms.
///
/// Each workspace is a face of a virtual cube. The outgoing face rotates away
/// while the incoming face rotates into view. `face_size` is the viewport
/// width.
pub fn cube_transform(
    progress: f64,
    direction: TransitionDirection,
    face_size: f64,
) -> (Transform3D, Transform3D) {
    let half_pi = std::f64::consts::FRAC_PI_2;
    let p = progress.clamp(0.0, 1.0);

    let sign = match direction {
        TransitionDirection::Forward => 1.0,
        TransitionDirection::Backward => -1.0,
    };

    let outgoing = Transform3D {
        translate_x: 0.0,
        translate_y: 0.0,
        scale_x: 1.0,
        scale_y: 1.0,
        opacity: 1.0 - p * 0.3, // slight fade for depth
        rotate_y: sign * p * half_pi,
        perspective: 1000.0,
    };

    let incoming = Transform3D {
        translate_x: 0.0,
        translate_y: 0.0,
        scale_x: 1.0,
        scale_y: 1.0,
        opacity: 0.7 + p * 0.3,
        rotate_y: -sign * (1.0 - p) * half_pi,
        perspective: 1000.0,
    };

    let _ = face_size; // used in to_2d_approx, not directly here
    (outgoing, incoming)
}

/// Compute stack transforms (card stack push/pop).
///
/// The incoming workspace slides in from the edge while the outgoing workspace
/// scales down slightly and dims, creating a layered card effect.
pub fn stack_transform(
    progress: f64,
    direction: TransitionDirection,
    viewport_width: f64,
) -> (Transform2D, Transform2D) {
    let p = progress.clamp(0.0, 1.0);

    let sign = match direction {
        TransitionDirection::Forward => 1.0,
        TransitionDirection::Backward => -1.0,
    };

    // Outgoing: scales down and dims (goes behind).
    let out_scale = 1.0 - 0.1 * p; // scale from 1.0 to 0.9
    let out_translate_x = -sign * p * viewport_width * 0.15; // shift slightly away
    let outgoing = Transform2D {
        translate_x: out_translate_x,
        translate_y: 0.0,
        scale_x: out_scale,
        scale_y: out_scale,
        opacity: 1.0 - 0.3 * p, // dim from 1.0 to 0.7
    };

    // Incoming: slides in from edge.
    let incoming = Transform2D {
        translate_x: sign * (1.0 - p) * viewport_width,
        translate_y: 0.0,
        scale_x: 1.0,
        scale_y: 1.0,
        opacity: 1.0,
    };

    (outgoing, incoming)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 0.001;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < EPSILON
    }

    // --- Transform2D tests ---

    #[test]
    fn transform2d_identity() {
        let t = Transform2D::identity();
        assert!(approx(t.translate_x, 0.0));
        assert!(approx(t.translate_y, 0.0));
        assert!(approx(t.scale_x, 1.0));
        assert!(approx(t.scale_y, 1.0));
        assert!(approx(t.opacity, 1.0));
    }

    #[test]
    fn transform2d_to_affine() {
        let t = Transform2D {
            translate_x: 10.0,
            translate_y: 20.0,
            scale_x: 2.0,
            scale_y: 3.0,
            opacity: 0.5,
        };
        let m = t.to_affine();
        assert!((m[0] - 2.0).abs() < 0.001);
        assert!((m[3] - 3.0).abs() < 0.001);
        assert!((m[4] - 10.0).abs() < 0.001);
        assert!((m[5] - 20.0).abs() < 0.001);
    }

    #[test]
    fn transform3d_identity() {
        let t = Transform3D::identity();
        assert!(approx(t.rotate_y, 0.0));
        assert!(approx(t.perspective, 1000.0));
        assert!(approx(t.opacity, 1.0));
    }

    #[test]
    fn transform3d_to_2d_no_rotation() {
        let t = Transform3D::identity();
        let t2 = t.to_2d_approx(1920.0);
        assert!(approx(t2.translate_x, 0.0));
        assert!(approx(t2.scale_x, 1.0));
        assert!(approx(t2.opacity, 1.0));
    }

    // --- Slide transition tests ---

    #[test]
    fn slide_at_start() {
        let (out, inc) = slide_transform(0.0, TransitionDirection::Forward, 1920.0);
        assert!(approx(out.translate_x, 0.0));
        assert!(approx(inc.translate_x, 1920.0));
    }

    #[test]
    fn slide_at_end() {
        let (out, inc) = slide_transform(1.0, TransitionDirection::Forward, 1920.0);
        assert!(approx(out.translate_x, -1920.0));
        assert!(approx(inc.translate_x, 0.0));
    }

    #[test]
    fn slide_at_midpoint() {
        let (out, inc) = slide_transform(0.5, TransitionDirection::Forward, 1920.0);
        assert!(approx(out.translate_x, -960.0));
        assert!(approx(inc.translate_x, 960.0));
    }

    #[test]
    fn slide_backward_reverses() {
        let (out_fwd, _) = slide_transform(0.5, TransitionDirection::Forward, 1920.0);
        let (out_bwd, _) = slide_transform(0.5, TransitionDirection::Backward, 1920.0);
        assert!(approx(out_fwd.translate_x, -out_bwd.translate_x));
    }

    // --- Fade transition tests ---

    #[test]
    fn fade_at_start() {
        let (out, inc) = fade_transform(0.0);
        assert!((out - 1.0).abs() < 0.001);
        assert!((inc - 0.0).abs() < 0.001);
    }

    #[test]
    fn fade_at_end() {
        let (out, inc) = fade_transform(1.0);
        assert!((out - 0.0).abs() < 0.001);
        assert!((inc - 1.0).abs() < 0.001);
    }

    #[test]
    fn fade_at_midpoint() {
        let (out, inc) = fade_transform(0.5);
        assert!((out - 0.5).abs() < 0.001);
        assert!((inc - 0.5).abs() < 0.001);
    }

    #[test]
    fn fade_sum_is_one() {
        for i in 0..=10 {
            let p = i as f64 / 10.0;
            let (out, inc) = fade_transform(p);
            let sum = out + inc;
            assert!((sum - 1.0).abs() < 0.01, "fade sum at p={p}: {sum}");
        }
    }

    // --- Cube transition tests ---

    #[test]
    fn cube_at_start() {
        let (out, inc) = cube_transform(0.0, TransitionDirection::Forward, 1920.0);
        assert!(approx(out.rotate_y, 0.0));
        assert!(approx(out.opacity, 1.0));
        assert!((inc.rotate_y.abs() - std::f64::consts::FRAC_PI_2).abs() < EPSILON);
    }

    #[test]
    fn cube_at_end() {
        let (out, inc) = cube_transform(1.0, TransitionDirection::Forward, 1920.0);
        assert!((out.rotate_y.abs() - std::f64::consts::FRAC_PI_2).abs() < EPSILON);
        assert!(approx(inc.rotate_y, 0.0));
    }

    #[test]
    fn cube_at_midpoint() {
        let (out, inc) = cube_transform(0.5, TransitionDirection::Forward, 1920.0);
        let half_half_pi = std::f64::consts::FRAC_PI_2 * 0.5;
        assert!((out.rotate_y.abs() - half_half_pi).abs() < EPSILON);
        assert!((inc.rotate_y.abs() - half_half_pi).abs() < EPSILON);
    }

    #[test]
    fn cube_backward_reverses_rotation() {
        let (out_fwd, _) = cube_transform(0.5, TransitionDirection::Forward, 1920.0);
        let (out_bwd, _) = cube_transform(0.5, TransitionDirection::Backward, 1920.0);
        assert!(approx(out_fwd.rotate_y, -out_bwd.rotate_y));
    }

    // --- Stack transition tests ---

    #[test]
    fn stack_at_start() {
        let (out, inc) = stack_transform(0.0, TransitionDirection::Forward, 1920.0);
        assert!(approx(out.scale_x, 1.0));
        assert!(approx(out.opacity, 1.0));
        assert!(approx(inc.translate_x, 1920.0));
    }

    #[test]
    fn stack_at_end() {
        let (out, inc) = stack_transform(1.0, TransitionDirection::Forward, 1920.0);
        assert!(approx(out.scale_x, 0.9));
        assert!(approx(out.opacity, 0.7));
        assert!(approx(inc.translate_x, 0.0));
    }

    #[test]
    fn stack_outgoing_scales_down() {
        let (out, _) = stack_transform(0.5, TransitionDirection::Forward, 1920.0);
        assert!(out.scale_x < 1.0, "outgoing should scale down: {}", out.scale_x);
        assert!(out.scale_x > 0.9, "should not scale too much: {}", out.scale_x);
    }

    // --- WorkspaceTransition tests ---

    #[test]
    fn workspace_transition_slide() {
        let wt = WorkspaceTransition::new(
            TransitionStyle::Slide,
            TransitionDirection::Forward,
            1920.0, 1080.0,
        );
        let (out, inc) = wt.compute(0.0);
        assert!(approx(out.translate_x, 0.0));
        assert!(approx(inc.translate_x, 1920.0));
    }

    #[test]
    fn workspace_transition_fade() {
        let wt = WorkspaceTransition::new(
            TransitionStyle::Fade,
            TransitionDirection::Forward,
            1920.0, 1080.0,
        );
        let (out, inc) = wt.compute(0.5);
        assert!(approx(out.opacity, 0.5));
        assert!(approx(inc.opacity, 0.5));
    }

    #[test]
    fn workspace_transition_cube() {
        let wt = WorkspaceTransition::new(
            TransitionStyle::Cube,
            TransitionDirection::Forward,
            1920.0, 1080.0,
        );
        let (out, inc) = wt.compute(0.0);
        // At start, outgoing should be roughly at identity.
        assert!(approx(out.scale_x, 1.0));
    }

    #[test]
    fn workspace_transition_stack() {
        let wt = WorkspaceTransition::new(
            TransitionStyle::Stack,
            TransitionDirection::Forward,
            1920.0, 1080.0,
        );
        let (out, inc) = wt.compute(1.0);
        assert!(approx(out.scale_x, 0.9));
        assert!(approx(inc.translate_x, 0.0));
    }

    #[test]
    fn cube_transforms_returns_3d() {
        let wt = WorkspaceTransition::new(
            TransitionStyle::Cube,
            TransitionDirection::Forward,
            1920.0, 1080.0,
        );
        let result = wt.cube_transforms(0.5);
        assert!(result.is_some());
    }

    #[test]
    fn cube_transforms_returns_none_for_slide() {
        let wt = WorkspaceTransition::new(
            TransitionStyle::Slide,
            TransitionDirection::Forward,
            1920.0, 1080.0,
        );
        assert!(wt.cube_transforms(0.5).is_none());
    }

    #[test]
    fn compute_clamps_progress() {
        let wt = WorkspaceTransition::new(
            TransitionStyle::Slide,
            TransitionDirection::Forward,
            1920.0, 1080.0,
        );
        let (out_neg, _) = wt.compute(-0.5);
        let (out_zero, _) = wt.compute(0.0);
        // WorkspaceTransition::compute clamps progress to [0, 1].
        assert!(approx(out_neg.translate_x, out_zero.translate_x));

        let (_, inc_over) = wt.compute(1.5);
        let (_, inc_one) = wt.compute(1.0);
        assert!(approx(inc_over.translate_x, inc_one.translate_x));
    }
}
