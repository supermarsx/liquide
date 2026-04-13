//! DevTools integration and telemetry reporting.

use tracing::info;

use crate::telemetry::TelemetryHandle;

use super::DesktopCompositor;

impl DesktopCompositor {
    /// Synchronise the devtools template into the shell DOM.
    ///
    /// Must be called **before** `shell.build_scene()` so the CSS pipeline
    /// can lay out and paint the devtools panel.  Uses the previous frame's
    /// layout / style data (one-frame-behind is expected for dev tools).
    pub(super) fn sync_devtools_template(&mut self) {
        self.dt.sync_template(&mut self.shell);
    }

    /// Get a clone of the telemetry handle for monitoring.
    pub fn telemetry(&self) -> TelemetryHandle {
        self.telemetry.clone()
    }

    /// Print comprehensive telemetry status report to log.
    pub fn print_telemetry_report(&self) {
        if let Ok(telemetry) = self.telemetry.read() {
            let report = telemetry.status_report();
            info!("\n{}", report);
        }
        // Append render pipeline metrics from liquide-render-coordinator.
        let rm = self.render_metrics.snapshot();
        if rm.tasks_submitted > 0 {
            info!(
                submitted = rm.tasks_submitted,
                completed = rm.tasks_completed,
                failed = rm.tasks_failed,
                avg_us = format!("{:.0}", rm.avg_render_time_us),
                p95_us = rm.p95_render_time_us,
                p99_us = rm.p99_render_time_us,
                throughput = format!("{:.1}", rm.tasks_per_second),
                "render pipeline metrics"
            );
        }
        // Feed render metrics into the telemetry viewer registry.
        use liquide_telemetry_viewer::metrics;
        self.viewer_metrics.set(metrics::FRAME_COUNT, self.frame_count);
        self.viewer_metrics.set(metrics::DROPPED_FRAMES, rm.tasks_failed);
    }
}
