use crate::actions::GestureBinding;

#[derive(Debug, Clone)]
pub struct GestureConfig {
    pub enabled: bool,
    pub swipe_threshold_px: f32,
    pub tap_timeout_ms: u64,
    pub long_press_ms: u64,
    pub edge_margin_px: f32,
    pub bindings: GestureBinding,
}

impl Default for GestureConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            swipe_threshold_px: 10.0,
            tap_timeout_ms: 300,
            long_press_ms: 500,
            edge_margin_px: 20.0,
            bindings: GestureBinding::default(),
        }
    }
}
