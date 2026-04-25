//! Output (monitor) protocol (wl_output equivalent).
//!
//! Describes physical and logical properties of display outputs,
//! including geometry, modes, subpixel layout, and transforms.

use crate::protocol::ObjectId;
use bitflags::bitflags;

// ---------------------------------------------------------------------------
// SubpixelOrder
// ---------------------------------------------------------------------------

/// Subpixel layout of the output panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubpixelOrder {
    Unknown,
    None,
    HorizontalRgb,
    HorizontalBgr,
    VerticalRgb,
    VerticalBgr,
}

impl Default for SubpixelOrder {
    fn default() -> Self {
        Self::Unknown
    }
}

// ---------------------------------------------------------------------------
// OutputTransform
// ---------------------------------------------------------------------------

/// Transform applied by the compositor to compensate for output orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputTransform {
    Normal,
    Rotate90,
    Rotate180,
    Rotate270,
    Flipped,
    FlippedRotate90,
    FlippedRotate180,
    FlippedRotate270,
}

impl Default for OutputTransform {
    fn default() -> Self {
        Self::Normal
    }
}

impl OutputTransform {
    /// Returns true if the transform swaps width and height
    /// (any 90/270 degree rotation).
    pub fn is_transposed(self) -> bool {
        matches!(
            self,
            Self::Rotate90 | Self::Rotate270 | Self::FlippedRotate90 | Self::FlippedRotate270
        )
    }

    /// Returns true if the transform includes a horizontal flip.
    pub fn is_flipped(self) -> bool {
        matches!(
            self,
            Self::Flipped | Self::FlippedRotate90 | Self::FlippedRotate180 | Self::FlippedRotate270
        )
    }
}

// ---------------------------------------------------------------------------
// OutputModeFlags
// ---------------------------------------------------------------------------

bitflags! {
    /// Flags for an output mode.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct OutputModeFlags: u32 {
        /// This is the current active mode.
        const CURRENT   = 1 << 0;
        /// This is the preferred (native) mode.
        const PREFERRED = 1 << 1;
    }
}

// ---------------------------------------------------------------------------
// OutputMode
// ---------------------------------------------------------------------------

/// A display mode (resolution + refresh rate).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputMode {
    /// Mode flags.
    pub flags: OutputModeFlags,
    /// Width in pixels.
    pub width: i32,
    /// Height in pixels.
    pub height: i32,
    /// Refresh rate in millihertz (e.g. 60000 = 60 Hz).
    pub refresh: i32,
}

impl OutputMode {
    pub fn new(width: i32, height: i32, refresh: i32, flags: OutputModeFlags) -> Self {
        Self {
            flags,
            width,
            height,
            refresh,
        }
    }

    /// Refresh rate in Hz as a float.
    pub fn refresh_hz(&self) -> f64 {
        self.refresh as f64 / 1000.0
    }

    /// Whether this is the current mode.
    pub fn is_current(&self) -> bool {
        self.flags.contains(OutputModeFlags::CURRENT)
    }

    /// Whether this is the preferred mode.
    pub fn is_preferred(&self) -> bool {
        self.flags.contains(OutputModeFlags::PREFERRED)
    }
}

// ---------------------------------------------------------------------------
// OutputGeometry
// ---------------------------------------------------------------------------

/// Physical and logical geometry of an output.
#[derive(Debug, Clone)]
pub struct OutputGeometry {
    /// X position in the global compositor space.
    pub x: i32,
    /// Y position in the global compositor space.
    pub y: i32,
    /// Physical width in millimeters.
    pub physical_width: i32,
    /// Physical height in millimeters.
    pub physical_height: i32,
    /// Subpixel layout.
    pub subpixel: SubpixelOrder,
    /// Manufacturer name.
    pub make: String,
    /// Model name.
    pub model: String,
    /// Output transform.
    pub transform: OutputTransform,
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

/// A display output (monitor).
///
/// Corresponds to the `wl_output` global in the Wayland protocol.
/// Reports geometry, supported modes, scale factor, and descriptive
/// information.
#[derive(Debug)]
pub struct Output {
    /// Protocol object ID.
    id: ObjectId,
    /// Geometry information.
    geometry: OutputGeometry,
    /// Available display modes.
    modes: Vec<OutputMode>,
    /// Integer scale factor for HiDPI.
    scale: i32,
    /// Logical name (e.g. "HDMI-A-1").
    name: String,
    /// Human-readable description.
    description: String,
}

impl Output {
    /// Create a new output.
    pub fn new(
        id: ObjectId,
        geometry: OutputGeometry,
        name: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id,
            geometry,
            modes: Vec::new(),
            scale: 1,
            name: name.into(),
            description: description.into(),
        }
    }

    pub fn id(&self) -> ObjectId {
        self.id
    }

    pub fn geometry(&self) -> &OutputGeometry {
        &self.geometry
    }

    pub fn modes(&self) -> &[OutputMode] {
        &self.modes
    }

    pub fn scale(&self) -> i32 {
        self.scale
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    /// Add a display mode.
    pub fn add_mode(&mut self, mode: OutputMode) {
        self.modes.push(mode);
    }

    /// Set the scale factor.
    pub fn set_scale(&mut self, scale: i32) {
        self.scale = scale;
    }

    /// Update the geometry.
    pub fn set_geometry(&mut self, geometry: OutputGeometry) {
        self.geometry = geometry;
    }

    /// Get the current mode (the one with the CURRENT flag).
    pub fn current_mode(&self) -> Option<&OutputMode> {
        self.modes.iter().find(|m| m.is_current())
    }

    /// Get the preferred mode (the one with the PREFERRED flag).
    pub fn preferred_mode(&self) -> Option<&OutputMode> {
        self.modes.iter().find(|m| m.is_preferred())
    }

    /// Logical size in compositor space, accounting for transform.
    ///
    /// Returns `None` if no current mode is set.
    pub fn logical_size(&self) -> Option<(i32, i32)> {
        let mode = self.current_mode()?;
        let (w, h) = if self.geometry.transform.is_transposed() {
            (mode.height, mode.width)
        } else {
            (mode.width, mode.height)
        };
        Some((w / self.scale, h / self.scale))
    }

    /// Physical DPI (dots per inch), computed from physical size and mode.
    ///
    /// Returns `None` if physical dimensions are zero or no current mode.
    pub fn dpi(&self) -> Option<(f64, f64)> {
        let mode = self.current_mode()?;
        if self.geometry.physical_width <= 0 || self.geometry.physical_height <= 0 {
            return None;
        }
        let dpi_x = mode.width as f64 / (self.geometry.physical_width as f64 / 25.4);
        let dpi_y = mode.height as f64 / (self.geometry.physical_height as f64 / 25.4);
        Some((dpi_x, dpi_y))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_geometry() -> OutputGeometry {
        OutputGeometry {
            x: 0,
            y: 0,
            physical_width: 530,
            physical_height: 300,
            subpixel: SubpixelOrder::HorizontalRgb,
            make: "ACME".into(),
            model: "UltraWide 34".into(),
            transform: OutputTransform::Normal,
        }
    }

    fn sample_output() -> Output {
        let mut output = Output::new(
            ObjectId(5),
            sample_geometry(),
            "HDMI-A-1",
            "ACME UltraWide 34",
        );
        output.add_mode(OutputMode::new(
            3440,
            1440,
            60000,
            OutputModeFlags::CURRENT | OutputModeFlags::PREFERRED,
        ));
        output.add_mode(OutputMode::new(2560, 1080, 75000, OutputModeFlags::empty()));
        output
    }

    #[test]
    fn output_basics() {
        let o = sample_output();
        assert_eq!(o.id(), ObjectId(5));
        assert_eq!(o.name(), "HDMI-A-1");
        assert_eq!(o.description(), "ACME UltraWide 34");
        assert_eq!(o.scale(), 1);
    }

    #[test]
    fn output_geometry_fields() {
        let o = sample_output();
        let g = o.geometry();
        assert_eq!(g.physical_width, 530);
        assert_eq!(g.make, "ACME");
        assert_eq!(g.subpixel, SubpixelOrder::HorizontalRgb);
        assert_eq!(g.transform, OutputTransform::Normal);
    }

    #[test]
    fn output_modes() {
        let o = sample_output();
        assert_eq!(o.modes().len(), 2);
    }

    #[test]
    fn output_current_mode() {
        let o = sample_output();
        let m = o.current_mode().unwrap();
        assert_eq!(m.width, 3440);
        assert_eq!(m.height, 1440);
        assert_eq!(m.refresh, 60000);
        assert!(m.is_current());
        assert!(m.is_preferred());
    }

    #[test]
    fn output_preferred_mode() {
        let o = sample_output();
        let m = o.preferred_mode().unwrap();
        assert_eq!(m.width, 3440);
    }

    #[test]
    fn output_refresh_hz() {
        let m = OutputMode::new(1920, 1080, 144000, OutputModeFlags::CURRENT);
        assert!((m.refresh_hz() - 144.0).abs() < 0.01);
    }

    #[test]
    fn output_logical_size_no_transform() {
        let o = sample_output();
        assert_eq!(o.logical_size(), Some((3440, 1440)));
    }

    #[test]
    fn output_logical_size_with_scale() {
        let mut o = sample_output();
        o.set_scale(2);
        assert_eq!(o.logical_size(), Some((1720, 720)));
    }

    #[test]
    fn output_logical_size_transposed() {
        let mut geo = sample_geometry();
        geo.transform = OutputTransform::Rotate90;
        let mut o = Output::new(ObjectId(5), geo, "DP-1", "Vertical");
        o.add_mode(OutputMode::new(1920, 1080, 60000, OutputModeFlags::CURRENT));
        // Transposed: width=1080, height=1920
        assert_eq!(o.logical_size(), Some((1080, 1920)));
    }

    #[test]
    fn output_dpi() {
        let o = sample_output();
        let (dpi_x, dpi_y) = o.dpi().unwrap();
        // 3440 px / (530mm / 25.4) = ~164.9 dpi
        assert!(dpi_x > 160.0 && dpi_x < 170.0);
        assert!(dpi_y > 120.0 && dpi_y < 130.0);
    }

    #[test]
    fn output_dpi_zero_physical() {
        let geo = OutputGeometry {
            x: 0,
            y: 0,
            physical_width: 0,
            physical_height: 0,
            subpixel: SubpixelOrder::Unknown,
            make: "Virtual".into(),
            model: "None".into(),
            transform: OutputTransform::Normal,
        };
        let mut o = Output::new(ObjectId(1), geo, "VIRTUAL-1", "Virtual output");
        o.add_mode(OutputMode::new(1920, 1080, 60000, OutputModeFlags::CURRENT));
        assert!(o.dpi().is_none());
    }

    #[test]
    fn output_no_current_mode() {
        let o = Output::new(ObjectId(1), sample_geometry(), "DP-1", "No modes");
        assert!(o.current_mode().is_none());
        assert!(o.logical_size().is_none());
    }

    #[test]
    fn output_transform_is_transposed() {
        assert!(!OutputTransform::Normal.is_transposed());
        assert!(OutputTransform::Rotate90.is_transposed());
        assert!(!OutputTransform::Rotate180.is_transposed());
        assert!(OutputTransform::Rotate270.is_transposed());
        assert!(!OutputTransform::Flipped.is_transposed());
        assert!(OutputTransform::FlippedRotate90.is_transposed());
        assert!(!OutputTransform::FlippedRotate180.is_transposed());
        assert!(OutputTransform::FlippedRotate270.is_transposed());
    }

    #[test]
    fn output_transform_is_flipped() {
        assert!(!OutputTransform::Normal.is_flipped());
        assert!(!OutputTransform::Rotate90.is_flipped());
        assert!(OutputTransform::Flipped.is_flipped());
        assert!(OutputTransform::FlippedRotate90.is_flipped());
        assert!(OutputTransform::FlippedRotate180.is_flipped());
        assert!(OutputTransform::FlippedRotate270.is_flipped());
    }

    #[test]
    fn subpixel_default() {
        assert_eq!(SubpixelOrder::default(), SubpixelOrder::Unknown);
    }

    #[test]
    fn output_set_geometry() {
        let mut o = sample_output();
        let mut new_geo = sample_geometry();
        new_geo.x = 1920;
        o.set_geometry(new_geo);
        assert_eq!(o.geometry().x, 1920);
    }

    #[test]
    fn mode_flags_empty() {
        let m = OutputMode::new(800, 600, 60000, OutputModeFlags::empty());
        assert!(!m.is_current());
        assert!(!m.is_preferred());
    }
}
