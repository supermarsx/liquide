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
        if !self.dev_mode {
            return;
        }

        // Determine visibility first with a shared borrow.
        let visible = self.devtools.as_ref().map_or(false, |d| d.is_visible());

        if visible {
            // Build the template from (previous frame's) data.
            // We clone just the TemplateNode out so all shared borrows are dropped
            // before the mutable mount call.
            let template = {
                let devtools = self.devtools.as_ref().unwrap();
                let doc = self.shell.document();
                match (self.shell.layout_tree(), self.shell.style_map()) {
                    (Some(layout), Some(styles)) => devtools.render_template(doc, layout, styles),
                    _ => {
                        // First frame — minimal stub so the pipeline has something.
                        liquide_devtools::TemplateNode::el("devtools-panel").id("devtools-panel")
                    }
                }
            };
            self.shell.mount_template("devtools-panel", &template);
        } else {
            self.shell.unmount_template("devtools-panel");
        }
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
    }
}
