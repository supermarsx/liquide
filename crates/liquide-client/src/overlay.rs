//! Stream-statistics overlay (HUD) for the client viewport.

/// Live metrics displayed in the overlay.
#[derive(Debug, Clone)]
pub struct OverlayMetrics {
    pub fps_render: f64,
    pub fps_decode: f64,
    pub fps_present: f64,
    pub rtt_ms: f64,
    pub packet_loss_percent: f64,
    pub bandwidth_in_mbps: f64,
    pub bandwidth_out_mbps: f64,
    pub encoder_name: String,
    pub transport_name: String,
    pub encryption_name: String,
    pub resolution: String,
    pub cache_hit_rate: f64,
    pub tile_mode_active: bool,
    pub effect_budget_percent: f64,
}

impl Default for OverlayMetrics {
    fn default() -> Self {
        Self {
            fps_render: 0.0,
            fps_decode: 0.0,
            fps_present: 0.0,
            rtt_ms: 0.0,
            packet_loss_percent: 0.0,
            bandwidth_in_mbps: 0.0,
            bandwidth_out_mbps: 0.0,
            encoder_name: String::new(),
            transport_name: String::new(),
            encryption_name: String::new(),
            resolution: String::new(),
            cache_hit_rate: 0.0,
            tile_mode_active: false,
            effect_budget_percent: 0.0,
        }
    }
}

/// An on-screen overlay showing live stream statistics.
pub struct StreamOverlay {
    visible: bool,
    position: String,
    opacity: f32,
    graph_mode: bool,
    metrics: OverlayMetrics,
}

impl StreamOverlay {
    /// Create a hidden overlay with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            visible: false,
            position: "top-left".to_string(),
            opacity: 0.8,
            graph_mode: false,
            metrics: OverlayMetrics::default(),
        }
    }

    /// Toggle visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Show the overlay.
    pub fn show(&mut self) {
        self.visible = true;
    }

    /// Hide the overlay.
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Whether the overlay is currently visible.
    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Replace the displayed metrics.
    pub fn update_metrics(&mut self, metrics: OverlayMetrics) {
        self.metrics = metrics;
    }

    /// Current overlay metrics.
    #[must_use]
    pub fn metrics(&self) -> &OverlayMetrics {
        &self.metrics
    }

    /// Set the screen position (e.g. "top-left", "bottom-right").
    pub fn set_position(&mut self, position: String) {
        self.position = position;
    }

    /// Set opacity (0.0 transparent .. 1.0 opaque).
    pub fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity.clamp(0.0, 1.0);
    }

    /// Toggle between numeric and graph visualisation of metrics.
    pub fn toggle_graph_mode(&mut self) {
        self.graph_mode = !self.graph_mode;
    }
}

impl Default for StreamOverlay {
    fn default() -> Self {
        Self::new()
    }
}
