//! DevTools state — manages the developer tools panel lifecycle and integration.

use liquide_devtools::{DevToolsPanel, FrameSnapshot, TemplateNode};
use liquide_shell::Shell;
use tracing::info;

use crate::telemetry::TelemetryHandle;

/// DevTools panel lifecycle and integration state.
pub(super) struct DevToolsState {
    pub(super) dev_mode: bool,
    pub(super) devtools: Option<DevToolsPanel>,
}

impl DevToolsState {
    pub(super) fn new() -> Self {
        Self {
            dev_mode: false,
            devtools: None,
        }
    }

    /// Enable or disable developer mode.
    pub(super) fn set_dev_mode(
        &mut self,
        enabled: bool,
        shell: &mut Shell,
        width: u32,
        height: u32,
    ) {
        self.dev_mode = enabled;
        if enabled && self.devtools.is_none() {
            let mut panel = DevToolsPanel::with_defaults();
            panel.set_screen_size(width as f32, height as f32);
            self.devtools = Some(panel);

            static DEVTOOLS_CSS: &str =
                include_str!("../../../../assets/themes/components/devtools.css");
            shell.add_stylesheet(DEVTOOLS_CSS);

            info!("devtools panel initialized (F12 to toggle)");
        } else if !enabled {
            shell.unmount_template("devtools-panel");
            self.devtools = None;
        }
    }

    /// Synchronise the devtools template into the shell DOM.
    ///
    /// Must be called **before** `shell.build_scene()` so the CSS pipeline
    /// can lay out and paint the devtools panel.
    pub(super) fn sync_template(&self, shell: &mut Shell) {
        if !self.dev_mode {
            return;
        }

        let visible = self.devtools.as_ref().map_or(false, |d| d.is_visible());

        if visible {
            let template = {
                let devtools = self.devtools.as_ref().unwrap();
                let doc = shell.document();
                match (shell.layout_tree(), shell.style_map()) {
                    (Some(layout), Some(styles)) => devtools.render_template(doc, layout, styles),
                    _ => TemplateNode::el("devtools-panel").id("devtools-panel"),
                }
            };
            shell.mount_template("devtools-panel", &template);
        } else {
            shell.unmount_template("devtools-panel");
        }
    }

    /// Overlay devtools scene nodes onto the scene graph.
    /// Called after `shell.build_scene()` to add the devtools panel.
    pub(super) fn overlay_scene(
        &mut self,
        scene: &mut liquide_compositor::scene::SceneNode,
        shell: &Shell,
        frame_count: u64,
        telemetry: &TelemetryHandle,
        width: u32,
        height: u32,
    ) {
        if !self.dev_mode {
            return;
        }

        let devtools = match self.devtools.as_mut() {
            Some(d) => d,
            None => return,
        };

        let doc = shell.document();
        devtools.refresh_inspector(doc);

        if let Ok(tel) = telemetry.read() {
            let fm = tel.frame_metrics();
            devtools.push_frame_snapshot(FrameSnapshot {
                frame_number: frame_count,
                fps: fm.current_fps,
                avg_frame_ms: fm.avg_frame_ms,
                css_rule_count: shell.css_rule_count(),
                css_variable_count: shell.css_variable_count(),
                stylesheet_count: shell.stylesheet_count(),
                viewport_w: width as f32,
                viewport_h: height as f32,
            });
        }

        if let (Some(layout), Some(styles)) = (shell.layout_tree(), shell.style_map()) {
            devtools.scene_debugger.snapshot(scene);
            for node in devtools.build_scene(doc, layout, styles) {
                scene.add_child(node);
            }
        }
    }

    /// Resize the devtools panel to match new screen dimensions.
    pub(super) fn on_resize(&mut self, width: u32, height: u32) {
        if let Some(ref mut devtools) = self.devtools {
            devtools.set_screen_size(width as f32, height as f32);
        }
    }
}
