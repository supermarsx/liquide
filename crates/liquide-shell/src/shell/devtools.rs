//! DevTools accessors, external template mounting, and stylesheet management.

use liquide_dom::Document;
use liquide_hit_test::HitTestEngine;
use liquide_layout::tree::LayoutTree;
use liquide_style_engine::StyleMap;

use crate::{Result, ShellError};

use super::Shell;

impl Shell {
    // ─── DevTools accessors ───────────────────────────────────

    /// Get a reference to the desktop DOM document (for devtools).
    pub fn document(&self) -> &Document {
        &self.desktop_dom.doc
    }

    /// Get the most recently computed layout tree (available after build_scene).
    pub fn layout_tree(&self) -> Option<&LayoutTree> {
        self.hit_test_engine.as_ref().map(|e| e.layout())
    }

    /// Get the most recently computed style map (available after build_scene).
    pub fn style_map(&self) -> Option<&StyleMap> {
        self.hit_test_engine.as_ref().map(|e| e.styles())
    }

    /// Get the hit-test engine (available after build_scene).
    pub fn hit_test_engine(&self) -> Option<&HitTestEngine> {
        self.hit_test_engine.as_ref()
    }

    /// Total number of CSS rules compiled across all loaded stylesheets.
    pub fn css_rule_count(&self) -> usize {
        self.css_pipeline.style_engine.rule_count()
    }

    /// Number of loaded stylesheets.
    pub fn stylesheet_count(&self) -> usize {
        self.css_pipeline.style_engine.sheet_count()
    }

    /// Number of CSS custom properties (variables) defined.
    pub fn css_variable_count(&self) -> usize {
        self.css_pipeline.style_engine.variable_count()
    }

    // ─── External template mounting (for devtools, extensions, etc.) ──

    /// Mount an external template into the desktop DOM.
    ///
    /// The template will be rendered by the CSS pipeline on the next
    /// `build_scene()` call. Uses keyed reconciliation so repeated calls
    /// efficiently patch the existing subtree.
    pub fn mount_template(
        &mut self,
        element_id: &str,
        template: &liquide_components::TemplateNode,
    ) {
        if let Err(err) = self.mount_template_for_app("com.liquide.shell", element_id, template) {
            tracing::warn!("mount_template denied for shell: {}", err);
        }
    }

    /// Mount an external template on behalf of an application.
    ///
    /// Enforces sandbox DOM write permissions for `app_id`.
    pub fn mount_template_for_app(
        &mut self,
        app_id: &str,
        element_id: &str,
        template: &liquide_components::TemplateNode,
    ) -> Result<()> {
        self.sandbox_manager
            .validate_dom_access(app_id, true)
            .map_err(ShellError::InvalidOperation)?;
        use crate::TemplateRenderer;
        let root = self.desktop_dom.doc.root();
        TemplateRenderer::apply_or_create(&mut self.desktop_dom.doc, root, element_id, template);
        Ok(())
    }

    /// Remove a previously mounted external template from the DOM.
    pub fn unmount_template(&mut self, element_id: &str) {
        if let Err(err) = self.unmount_template_for_app("com.liquide.shell", element_id) {
            tracing::warn!("unmount_template denied for shell: {}", err);
        }
    }

    /// Remove a previously mounted external template on behalf of an app.
    pub fn unmount_template_for_app(&mut self, app_id: &str, element_id: &str) -> Result<()> {
        self.sandbox_manager
            .validate_dom_access(app_id, true)
            .map_err(ShellError::InvalidOperation)?;
        use crate::TemplateRenderer;
        TemplateRenderer::unmount(&mut self.desktop_dom.doc, element_id);
        Ok(())
    }

    /// Dynamically load an additional stylesheet into the CSS pipeline.
    pub fn add_stylesheet(&mut self, css: &str) -> bool {
        self.css_pipeline.add_stylesheet(css);
        // The window decoration geometry is now anchored to the laid-out CSS
        // boxes (t103-p6); a new stylesheet can move the titlebar/buttons, so it
        // must invalidate the window-scene + full-scene caches — otherwise a
        // steady-state hit could serve a window subtree painted against the old
        // decoration layout.
        self.mark_window_scene_dirty();
        true
    }

    /// Get @font-face rules from all loaded stylesheets.
    pub fn font_faces(&self) -> &[liquide_style_engine::engine::PreparedFontFace] {
        self.css_pipeline.font_faces()
    }
}
