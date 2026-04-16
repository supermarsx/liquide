//! XWayland bridge for standalone compositor.

/// State of the XWayland bridge in the standalone compositor.
#[derive(Debug, Default)]
pub struct XWaylandBridgeState {
    /// Whether XWayland is enabled.
    pub enabled: bool,
    /// Number of X11 windows currently mapped.
    pub window_count: u32,
    /// X11 display string (e.g. ":1").
    pub display: String,
}
