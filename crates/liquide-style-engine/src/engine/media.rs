//! Media query, `@supports`, and `@container` condition evaluation.

use liquide_dom::{Document, NodeId};

use super::{ContainerCondition, StyleEngine};
use crate::computed::*;
use crate::style_map::StyleMap;

impl StyleEngine {
    /// Build the set of CSS properties we support for `@supports` runtime checks.
    pub(crate) fn build_supported_properties() -> std::collections::HashSet<&'static str> {
        [
            "display",
            "position",
            "box-sizing",
            "width",
            "height",
            "min-width",
            "max-width",
            "min-height",
            "max-height",
            "margin",
            "margin-top",
            "margin-right",
            "margin-bottom",
            "margin-left",
            "padding",
            "padding-top",
            "padding-right",
            "padding-bottom",
            "padding-left",
            "border",
            "border-width",
            "border-style",
            "border-color",
            "border-radius",
            "border-top",
            "border-right",
            "border-bottom",
            "border-left",
            "top",
            "right",
            "bottom",
            "left",
            "z-index",
            "float",
            "clear",
            "overflow",
            "overflow-x",
            "overflow-y",
            "visibility",
            "opacity",
            "color",
            "background",
            "background-color",
            "background-image",
            "background-size",
            "background-position",
            "background-repeat",
            "font-family",
            "font-size",
            "font-weight",
            "font-style",
            "line-height",
            "letter-spacing",
            "word-spacing",
            "text-align",
            "text-decoration",
            "text-transform",
            "text-overflow",
            "text-indent",
            "white-space",
            "word-break",
            "vertical-align",
            "flex",
            "flex-direction",
            "flex-wrap",
            "flex-grow",
            "flex-shrink",
            "flex-basis",
            "justify-content",
            "align-items",
            "align-self",
            "align-content",
            "order",
            "gap",
            "row-gap",
            "column-gap",
            "grid",
            "grid-template-columns",
            "grid-template-rows",
            "grid-column",
            "grid-row",
            "grid-area",
            "grid-auto-flow",
            "grid-auto-columns",
            "grid-auto-rows",
            "grid-template-areas",
            "transform",
            "transition",
            "animation",
            "box-shadow",
            "filter",
            "backdrop-filter",
            "clip-path",
            "cursor",
            "outline",
            "resize",
            "user-select",
            "pointer-events",
            "content",
            "counter-increment",
            "counter-reset",
            "quotes",
            "list-style",
            "list-style-type",
            "list-style-position",
            "table-layout",
            "border-collapse",
            "border-spacing",
            "columns",
            "column-count",
            "column-width",
            "column-gap",
            "column-rule",
            "column-span",
            "column-fill",
            "writing-mode",
            "direction",
            "unicode-bidi",
            "contain",
            "container-type",
            "container-name",
            "aspect-ratio",
            "object-fit",
            "object-position",
            "scroll-behavior",
            "scroll-snap-type",
            "scroll-snap-align",
            "scroll-padding",
            "scroll-margin",
            "overscroll-behavior",
            "accent-color",
            "caret-color",
            "appearance",
            "will-change",
            "isolation",
            "mix-blend-mode",
            "mask",
            "mask-image",
            "mask-size",
            "mask-position",
            "shape-outside",
            "shape-margin",
            "shape-image-threshold",
            // Logical properties
            "margin-inline",
            "margin-inline-start",
            "margin-inline-end",
            "margin-block",
            "margin-block-start",
            "margin-block-end",
            "padding-inline",
            "padding-inline-start",
            "padding-inline-end",
            "padding-block",
            "padding-block-start",
            "padding-block-end",
            "border-inline",
            "border-block",
            "inset",
            "inset-inline",
            "inset-block",
            // Modern CSS features
            "container",
            "subgrid",
        ]
        .into_iter()
        .collect()
    }

    /// Evaluate a `@supports` condition at runtime.
    pub fn evaluate_supports_condition(&self, condition: &str) -> bool {
        let condition = condition.trim();

        // Handle `not (…)`
        if let Some(inner) = condition.strip_prefix("not ") {
            return !self.evaluate_supports_condition(inner.trim());
        }

        // Handle bare parenthesized condition `(property: value)`
        if condition.starts_with('(') && condition.ends_with(')') {
            let inner = &condition[1..condition.len() - 1];
            if let Some((prop, _val)) = inner.split_once(':') {
                return self.supported_properties.contains(prop.trim());
            }
            // Could be a nested condition
            return self.evaluate_supports_condition(inner.trim());
        }

        // Handle `(…) and (…)`
        if condition.contains(") and (") {
            return condition.split(") and (").all(|part| {
                let p = part.trim().trim_start_matches('(').trim_end_matches(')');
                self.evaluate_supports_condition(&format!("({})", p))
            });
        }

        // Handle `(…) or (…)`
        if condition.contains(") or (") {
            return condition.split(") or (").any(|part| {
                let p = part.trim().trim_start_matches('(').trim_end_matches(')');
                self.evaluate_supports_condition(&format!("({})", p))
            });
        }

        // Default: assume supported
        true
    }

    /// Evaluate a `@container` condition by walking up the tree to find
    /// the nearest container ancestor and checking the condition against
    /// its computed dimensions.
    pub(crate) fn evaluate_container_condition(
        &self,
        condition: &ContainerCondition,
        node_id: NodeId,
        doc: &Document,
        map: &StyleMap,
    ) -> bool {
        // Walk ancestors to find a container
        let mut current = doc.parent(node_id);
        while let Some(ancestor_id) = current {
            if let Some(ancestor_style) = map.get(ancestor_id) {
                let ct = ancestor_style.container_type;
                if ct != ContainerType::Normal {
                    // Check container name if specified
                    if let Some(ref required_name) = condition.name {
                        if ancestor_style.container_name.as_deref() != Some(required_name.as_str())
                        {
                            current = doc.parent(ancestor_id);
                            continue;
                        }
                    }
                    // Evaluate the condition against this container's dimensions.
                    // Use real container dimensions if available from previous
                    // layout pass; fall back to viewport as a proxy.
                    let (cw, ch) = map
                        .container_size(ancestor_id)
                        .unwrap_or((self.viewport.width, self.viewport.height));
                    return self.evaluate_container_size_condition(&condition.condition, cw, ch);
                }
            }
            current = doc.parent(ancestor_id);
        }
        false // No matching container found
    }

    /// Parse and evaluate a container size condition like `(min-width: 600px)`.
    fn evaluate_container_size_condition(
        &self,
        condition: &str,
        container_w: f32,
        container_h: f32,
    ) -> bool {
        let condition = condition.trim();
        let inner = condition
            .strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
            .unwrap_or(condition);

        // Handle compound conditions
        if inner.contains(") and (") {
            return inner.split(") and (").all(|part| {
                self.evaluate_container_size_condition(
                    &format!("({})", part.trim_matches(|c| c == '(' || c == ')')),
                    container_w,
                    container_h,
                )
            });
        }
        if inner.contains(") or (") {
            return inner.split(") or (").any(|part| {
                self.evaluate_container_size_condition(
                    &format!("({})", part.trim_matches(|c| c == '(' || c == ')')),
                    container_w,
                    container_h,
                )
            });
        }

        if let Some((prop, value_str)) = inner.split_once(':') {
            let prop = prop.trim();
            let value_str = value_str.trim();
            let px_value = Self::parse_px_value(value_str).unwrap_or(0.0);
            match prop {
                "min-width" => container_w >= px_value,
                "max-width" => container_w <= px_value,
                "min-height" => container_h >= px_value,
                "max-height" => container_h <= px_value,
                "width" => (container_w - px_value).abs() < 1.0,
                "height" => (container_h - px_value).abs() < 1.0,
                _ => true,
            }
        } else {
            true
        }
    }

    /// Evaluate a serialized media condition string against the current viewport.
    ///
    /// Supports the most common media features:
    /// - `(prefers-color-scheme: dark|light)`
    /// - `(min-width: <px>)` / `(max-width: <px>)`
    /// - `(min-height: <px>)` / `(max-height: <px>)`
    /// - `all` / `screen` / `print`
    ///
    /// Returns `true` (include the rule) for unrecognised conditions.
    pub fn evaluate_media_condition(&self, condition: &str) -> bool {
        let condition = condition.trim();
        if condition.is_empty() || condition == "all" {
            return true;
        }
        // "print" rules never match a screen renderer
        if condition == "print" || condition == "not all" {
            return false;
        }

        // Handle "not <rest>"
        if let Some(rest) = condition.strip_prefix("not ") {
            return !self.evaluate_media_condition(rest.trim());
        }

        // Handle " and " compound
        if condition.contains(" and ") {
            return condition
                .split(" and ")
                .all(|part| self.evaluate_media_condition(part.trim()));
        }
        // Handle ", " (or-list in media)
        if condition.contains(", ") {
            return condition
                .split(", ")
                .any(|part| self.evaluate_media_condition(part.trim()));
        }

        // "screen" always matches
        if condition == "screen" {
            return true;
        }

        // Parenthesized feature query
        if condition.starts_with('(') && condition.ends_with(')') {
            let inner = &condition[1..condition.len() - 1];
            return self.evaluate_media_feature(inner);
        }

        // Unknown — default to include
        true
    }

    /// Evaluate a single media feature (the contents between parentheses).
    ///
    /// Supports CSS Media Queries Level 5 features. Features that require
    /// platform state not yet stored on `StyleEngine` use sensible desktop
    /// defaults (e.g. pointer=fine, hover=hover).
    fn evaluate_media_feature(&self, feature: &str) -> bool {
        let feature = feature.trim();

        // Boolean (no-value) media features: `(color)`, `(hover)`, etc.
        if !feature.contains(':') {
            return self.evaluate_boolean_media_feature(feature);
        }

        if let Some(colon_pos) = feature.find(':') {
            let name = feature[..colon_pos].trim();
            let value_str = feature[colon_pos + 1..].trim();

            match name {
                // ── Viewport dimension features ──────────────────────────
                "width" => {
                    if let Some(px) = Self::parse_px_value(value_str) {
                        return (self.viewport.width - px).abs() < 1.0;
                    }
                }
                "min-width" => {
                    if let Some(px) = Self::parse_px_value(value_str) {
                        return self.viewport.width >= px;
                    }
                }
                "max-width" => {
                    if let Some(px) = Self::parse_px_value(value_str) {
                        return self.viewport.width <= px;
                    }
                }
                "height" => {
                    if let Some(px) = Self::parse_px_value(value_str) {
                        return (self.viewport.height - px).abs() < 1.0;
                    }
                }
                "min-height" => {
                    if let Some(px) = Self::parse_px_value(value_str) {
                        return self.viewport.height >= px;
                    }
                }
                "max-height" => {
                    if let Some(px) = Self::parse_px_value(value_str) {
                        return self.viewport.height <= px;
                    }
                }
                // device-width / device-height — deprecated but still used
                "device-width" => {
                    if let Some(px) = Self::parse_px_value(value_str) {
                        return (self.viewport.width - px).abs() < 1.0;
                    }
                }
                "min-device-width" => {
                    if let Some(px) = Self::parse_px_value(value_str) {
                        return self.viewport.width >= px;
                    }
                }
                "max-device-width" => {
                    if let Some(px) = Self::parse_px_value(value_str) {
                        return self.viewport.width <= px;
                    }
                }
                "device-height" => {
                    if let Some(px) = Self::parse_px_value(value_str) {
                        return (self.viewport.height - px).abs() < 1.0;
                    }
                }
                "min-device-height" => {
                    if let Some(px) = Self::parse_px_value(value_str) {
                        return self.viewport.height >= px;
                    }
                }
                "max-device-height" => {
                    if let Some(px) = Self::parse_px_value(value_str) {
                        return self.viewport.height <= px;
                    }
                }

                // ── Aspect ratio ─────────────────────────────────────────
                "aspect-ratio" => {
                    return self.evaluate_aspect_ratio(value_str, |actual, query| {
                        (actual - query).abs() < 0.001
                    });
                }
                "min-aspect-ratio" => {
                    return self
                        .evaluate_aspect_ratio(value_str, |actual, query| actual >= query);
                }
                "max-aspect-ratio" => {
                    return self
                        .evaluate_aspect_ratio(value_str, |actual, query| actual <= query);
                }

                // ── Orientation ──────────────────────────────────────────
                "orientation" => {
                    let actual = if self.viewport.width >= self.viewport.height {
                        "landscape"
                    } else {
                        "portrait"
                    };
                    return value_str.eq_ignore_ascii_case(actual);
                }

                // ── Resolution ───────────────────────────────────────────
                "resolution" => {
                    if let Some(dpi) = Self::parse_resolution_value(value_str) {
                        return (96.0_f32 - dpi).abs() < 1.0;
                    }
                }
                "min-resolution" => {
                    if let Some(dpi) = Self::parse_resolution_value(value_str) {
                        return 96.0_f32 >= dpi;
                    }
                }
                "max-resolution" => {
                    if let Some(dpi) = Self::parse_resolution_value(value_str) {
                        return 96.0_f32 <= dpi;
                    }
                }

                // ── User preference features ─────────────────────────────
                "prefers-color-scheme" => {
                    return value_str.trim().eq_ignore_ascii_case(&self.preferred_color_scheme);
                }
                "prefers-reduced-motion" => {
                    return (value_str.trim() == "reduce") == self.prefers_reduced_motion;
                }
                "prefers-contrast" => {
                    // Default: no-preference (desktop default)
                    return value_str.trim().eq_ignore_ascii_case("no-preference");
                }
                "prefers-reduced-data" => {
                    // Default: no-preference
                    return value_str.trim().eq_ignore_ascii_case("no-preference");
                }
                "prefers-reduced-transparency" => {
                    // Default: no-preference
                    return value_str.trim().eq_ignore_ascii_case("no-preference");
                }
                "forced-colors" => {
                    // Default: none (no forced-colors mode)
                    return value_str.trim().eq_ignore_ascii_case("none");
                }

                // ── Pointer / hover interaction features ─────────────────
                "pointer" => {
                    // Desktop default: fine (mouse)
                    return value_str.trim().eq_ignore_ascii_case("fine");
                }
                "any-pointer" => {
                    // Desktop default: fine
                    return value_str.trim().eq_ignore_ascii_case("fine");
                }
                "hover" => {
                    // Desktop default: hover (mouse supports hover)
                    return value_str.trim().eq_ignore_ascii_case("hover");
                }
                "any-hover" => {
                    // Desktop default: hover
                    return value_str.trim().eq_ignore_ascii_case("hover");
                }

                // ── Display features ─────────────────────────────────────
                "display-mode" => {
                    // Desktop compositor default: standalone
                    return value_str.trim().eq_ignore_ascii_case("standalone");
                }
                "color-gamut" => {
                    // Desktop default: srgb. Also match if query is less capable.
                    let v = value_str.trim().to_ascii_lowercase();
                    return matches!(v.as_str(), "srgb");
                }
                "dynamic-range" => {
                    // Default: standard
                    return value_str.trim().eq_ignore_ascii_case("standard");
                }
                "video-dynamic-range" => {
                    // Default: standard
                    return value_str.trim().eq_ignore_ascii_case("standard");
                }

                // ── Color capability features ────────────────────────────
                "color" => {
                    // Bits per color channel, default 8
                    if let Ok(bits) = value_str.trim().parse::<u32>() {
                        return 8 >= bits;
                    }
                }
                "min-color" => {
                    if let Ok(bits) = value_str.trim().parse::<u32>() {
                        return 8 >= bits;
                    }
                }
                "max-color" => {
                    if let Ok(bits) = value_str.trim().parse::<u32>() {
                        return 8 <= bits;
                    }
                }
                "color-index" => {
                    // Default: 0 (not an indexed-color device)
                    if let Ok(n) = value_str.trim().parse::<u32>() {
                        return n == 0;
                    }
                }
                "min-color-index" => {
                    if let Ok(n) = value_str.trim().parse::<u32>() {
                        return 0_u32 >= n;
                    }
                }
                "max-color-index" => {
                    if let Ok(n) = value_str.trim().parse::<u32>() {
                        return 0_u32 <= n;
                    }
                }
                "monochrome" => {
                    // Default: 0 (not monochrome)
                    if let Ok(n) = value_str.trim().parse::<u32>() {
                        return n == 0;
                    }
                }
                "min-monochrome" => {
                    if let Ok(n) = value_str.trim().parse::<u32>() {
                        return 0_u32 >= n;
                    }
                }
                "max-monochrome" => {
                    if let Ok(n) = value_str.trim().parse::<u32>() {
                        return 0_u32 <= n;
                    }
                }

                // ── Grid ─────────────────────────────────────────────────
                "grid" => {
                    // Default: 0 (not a grid device like a TTY)
                    if let Ok(n) = value_str.trim().parse::<u32>() {
                        return n == 0;
                    }
                }

                // ── Update frequency ─────────────────────────────────────
                "update" => {
                    // Desktop display default: fast
                    return value_str.trim().eq_ignore_ascii_case("fast");
                }

                // ── Overflow ─────────────────────────────────────────────
                "overflow-block" => {
                    // Desktop default: scroll
                    return value_str.trim().eq_ignore_ascii_case("scroll");
                }
                "overflow-inline" => {
                    // Desktop default: scroll
                    return value_str.trim().eq_ignore_ascii_case("scroll");
                }

                // ── Scripting ────────────────────────────────────────────
                "scripting" => {
                    // Default: enabled (JS/WASM is available)
                    return value_str.trim().eq_ignore_ascii_case("enabled");
                }

                _ => {}
            }
        }
        // Unknown feature — include by default
        true
    }

    /// Evaluate a boolean media feature (no value, e.g. `(color)` or `(hover)`).
    fn evaluate_boolean_media_feature(&self, feature: &str) -> bool {
        match feature {
            "color" => true,            // color device: yes
            "color-index" => false,     // indexed-color: no
            "monochrome" => false,      // monochrome: no
            "grid" => false,            // grid device: no
            "hover" => true,            // hover capable: yes
            "any-hover" => true,        // any input hover: yes
            "pointer" => true,          // pointing device: yes
            "any-pointer" => true,      // any pointing device: yes
            _ => true,                  // unknown → include by default
        }
    }

    /// Parse an aspect ratio string like "16/9" or "4/3" and compare using the
    /// provided comparator.
    fn evaluate_aspect_ratio(&self, value_str: &str, cmp: impl Fn(f32, f32) -> bool) -> bool {
        if let Some((w_str, h_str)) = value_str.split_once('/') {
            if let (Ok(w), Ok(h)) = (
                w_str.trim().parse::<f32>(),
                h_str.trim().parse::<f32>(),
            ) {
                if h > 0.0 && self.viewport.height > 0.0 {
                    let actual = self.viewport.width / self.viewport.height;
                    let query = w / h;
                    return cmp(actual, query);
                }
            }
        }
        true // malformed → include by default
    }

    /// Parse a resolution value like "96dpi", "2dppx", or "300dpi".
    /// Returns the value normalised to dpi.
    fn parse_resolution_value(s: &str) -> Option<f32> {
        let s = s.trim();
        if let Some(num) = s.strip_suffix("dppx") {
            return num.trim().parse::<f32>().ok().map(|v| v * 96.0);
        }
        if let Some(num) = s.strip_suffix("dpcm") {
            return num.trim().parse::<f32>().ok().map(|v| v * 2.54);
        }
        if let Some(num) = s.strip_suffix("dpi") {
            return num.trim().parse::<f32>().ok();
        }
        // bare number treated as dpi
        s.parse::<f32>().ok()
    }

    /// Parse a pixel value like "768px" or "1024px".
    pub(crate) fn parse_px_value(s: &str) -> Option<f32> {
        let s = s.trim();
        let num_str = s.strip_suffix("px").unwrap_or(s);
        num_str.trim().parse::<f32>().ok()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::super::{StyleEngine, ViewportSize};

    fn engine() -> StyleEngine {
        StyleEngine::new(ViewportSize { width: 1920.0, height: 1080.0 }, 16.0)
    }

    fn engine_portrait() -> StyleEngine {
        StyleEngine::new(ViewportSize { width: 720.0, height: 1280.0 }, 16.0)
    }

    // ── Dimension features ───────────────────────────────────────────────

    #[test]
    fn width_exact() {
        let e = engine();
        assert!(e.evaluate_media_condition("(width: 1920px)"));
        assert!(!e.evaluate_media_condition("(width: 800px)"));
    }

    #[test]
    fn min_max_width() {
        let e = engine();
        assert!(e.evaluate_media_condition("(min-width: 1024px)"));
        assert!(!e.evaluate_media_condition("(min-width: 2560px)"));
        assert!(e.evaluate_media_condition("(max-width: 2560px)"));
        assert!(!e.evaluate_media_condition("(max-width: 800px)"));
    }

    #[test]
    fn height_exact() {
        let e = engine();
        assert!(e.evaluate_media_condition("(height: 1080px)"));
        assert!(!e.evaluate_media_condition("(height: 720px)"));
    }

    #[test]
    fn min_max_height() {
        let e = engine();
        assert!(e.evaluate_media_condition("(min-height: 768px)"));
        assert!(!e.evaluate_media_condition("(min-height: 1440px)"));
        assert!(e.evaluate_media_condition("(max-height: 1440px)"));
        assert!(!e.evaluate_media_condition("(max-height: 720px)"));
    }

    #[test]
    fn device_width_height() {
        let e = engine();
        assert!(e.evaluate_media_condition("(device-width: 1920px)"));
        assert!(e.evaluate_media_condition("(min-device-width: 1024px)"));
        assert!(e.evaluate_media_condition("(max-device-width: 2560px)"));
        assert!(e.evaluate_media_condition("(device-height: 1080px)"));
        assert!(e.evaluate_media_condition("(min-device-height: 768px)"));
        assert!(e.evaluate_media_condition("(max-device-height: 1440px)"));
    }

    // ── Aspect ratio ─────────────────────────────────────────────────────

    #[test]
    fn aspect_ratio() {
        let e = engine(); // 1920/1080 = 16/9
        assert!(e.evaluate_media_condition("(aspect-ratio: 16/9)"));
        assert!(!e.evaluate_media_condition("(aspect-ratio: 4/3)"));
    }

    #[test]
    fn min_max_aspect_ratio() {
        let e = engine(); // 16:9 ≈ 1.778
        assert!(e.evaluate_media_condition("(min-aspect-ratio: 1/1)"));
        assert!(!e.evaluate_media_condition("(min-aspect-ratio: 21/9)"));
        assert!(e.evaluate_media_condition("(max-aspect-ratio: 21/9)"));
        assert!(!e.evaluate_media_condition("(max-aspect-ratio: 1/1)"));
    }

    // ── Orientation ──────────────────────────────────────────────────────

    #[test]
    fn orientation_landscape() {
        let e = engine(); // 1920x1080 → landscape
        assert!(e.evaluate_media_condition("(orientation: landscape)"));
        assert!(!e.evaluate_media_condition("(orientation: portrait)"));
    }

    #[test]
    fn orientation_portrait() {
        let e = engine_portrait(); // 720x1280 → portrait
        assert!(e.evaluate_media_condition("(orientation: portrait)"));
        assert!(!e.evaluate_media_condition("(orientation: landscape)"));
    }

    // ── Resolution ───────────────────────────────────────────────────────

    #[test]
    fn resolution_dpi() {
        let e = engine();
        assert!(e.evaluate_media_condition("(resolution: 96dpi)"));
        assert!(!e.evaluate_media_condition("(resolution: 192dpi)"));
    }

    #[test]
    fn resolution_dppx() {
        let e = engine();
        assert!(e.evaluate_media_condition("(resolution: 1dppx)"));
        assert!(!e.evaluate_media_condition("(resolution: 2dppx)"));
    }

    #[test]
    fn min_max_resolution() {
        let e = engine();
        assert!(e.evaluate_media_condition("(min-resolution: 72dpi)"));
        assert!(!e.evaluate_media_condition("(min-resolution: 192dpi)"));
        assert!(e.evaluate_media_condition("(max-resolution: 192dpi)"));
        assert!(!e.evaluate_media_condition("(max-resolution: 72dpi)"));
    }

    // ── User preference features ─────────────────────────────────────────

    #[test]
    fn prefers_color_scheme() {
        let e = engine(); // default: light
        assert!(e.evaluate_media_condition("(prefers-color-scheme: light)"));
        assert!(!e.evaluate_media_condition("(prefers-color-scheme: dark)"));
    }

    #[test]
    fn prefers_reduced_motion() {
        let e = engine(); // default: false
        assert!(e.evaluate_media_condition("(prefers-reduced-motion: no-preference)"));
        assert!(!e.evaluate_media_condition("(prefers-reduced-motion: reduce)"));
    }

    #[test]
    fn prefers_contrast() {
        let e = engine();
        assert!(e.evaluate_media_condition("(prefers-contrast: no-preference)"));
        assert!(!e.evaluate_media_condition("(prefers-contrast: more)"));
    }

    #[test]
    fn prefers_reduced_data() {
        let e = engine();
        assert!(e.evaluate_media_condition("(prefers-reduced-data: no-preference)"));
        assert!(!e.evaluate_media_condition("(prefers-reduced-data: reduce)"));
    }

    #[test]
    fn prefers_reduced_transparency() {
        let e = engine();
        assert!(e.evaluate_media_condition("(prefers-reduced-transparency: no-preference)"));
        assert!(!e.evaluate_media_condition("(prefers-reduced-transparency: reduce)"));
    }

    #[test]
    fn forced_colors() {
        let e = engine();
        assert!(e.evaluate_media_condition("(forced-colors: none)"));
        assert!(!e.evaluate_media_condition("(forced-colors: active)"));
    }

    // ── Pointer / hover ──────────────────────────────────────────────────

    #[test]
    fn pointer_feature() {
        let e = engine();
        assert!(e.evaluate_media_condition("(pointer: fine)"));
        assert!(!e.evaluate_media_condition("(pointer: coarse)"));
        assert!(!e.evaluate_media_condition("(pointer: none)"));
    }

    #[test]
    fn any_pointer_feature() {
        let e = engine();
        assert!(e.evaluate_media_condition("(any-pointer: fine)"));
        assert!(!e.evaluate_media_condition("(any-pointer: coarse)"));
    }

    #[test]
    fn hover_feature() {
        let e = engine();
        assert!(e.evaluate_media_condition("(hover: hover)"));
        assert!(!e.evaluate_media_condition("(hover: none)"));
    }

    #[test]
    fn any_hover_feature() {
        let e = engine();
        assert!(e.evaluate_media_condition("(any-hover: hover)"));
        assert!(!e.evaluate_media_condition("(any-hover: none)"));
    }

    // ── Display features ─────────────────────────────────────────────────

    #[test]
    fn display_mode() {
        let e = engine();
        assert!(e.evaluate_media_condition("(display-mode: standalone)"));
        assert!(!e.evaluate_media_condition("(display-mode: fullscreen)"));
    }

    #[test]
    fn color_gamut() {
        let e = engine();
        assert!(e.evaluate_media_condition("(color-gamut: srgb)"));
        assert!(!e.evaluate_media_condition("(color-gamut: p3)"));
    }

    #[test]
    fn dynamic_range() {
        let e = engine();
        assert!(e.evaluate_media_condition("(dynamic-range: standard)"));
        assert!(!e.evaluate_media_condition("(dynamic-range: high)"));
    }

    #[test]
    fn video_dynamic_range() {
        let e = engine();
        assert!(e.evaluate_media_condition("(video-dynamic-range: standard)"));
        assert!(!e.evaluate_media_condition("(video-dynamic-range: high)"));
    }

    // ── Color capability features ────────────────────────────────────────

    #[test]
    fn color_bits() {
        let e = engine();
        // 8 bits per channel
        assert!(e.evaluate_media_condition("(color: 8)"));
        assert!(e.evaluate_media_condition("(min-color: 4)"));
        assert!(!e.evaluate_media_condition("(min-color: 10)"));
        assert!(e.evaluate_media_condition("(max-color: 10)"));
        assert!(!e.evaluate_media_condition("(max-color: 4)"));
    }

    #[test]
    fn color_index_feature() {
        let e = engine();
        assert!(e.evaluate_media_condition("(color-index: 0)"));
        assert!(!e.evaluate_media_condition("(color-index: 256)"));
    }

    #[test]
    fn monochrome_feature() {
        let e = engine();
        assert!(e.evaluate_media_condition("(monochrome: 0)"));
        assert!(!e.evaluate_media_condition("(monochrome: 1)"));
    }

    #[test]
    fn grid_feature() {
        let e = engine();
        assert!(e.evaluate_media_condition("(grid: 0)"));
        assert!(!e.evaluate_media_condition("(grid: 1)"));
    }

    // ── Update / overflow / scripting ────────────────────────────────────

    #[test]
    fn update_feature() {
        let e = engine();
        assert!(e.evaluate_media_condition("(update: fast)"));
        assert!(!e.evaluate_media_condition("(update: slow)"));
        assert!(!e.evaluate_media_condition("(update: none)"));
    }

    #[test]
    fn overflow_block_feature() {
        let e = engine();
        assert!(e.evaluate_media_condition("(overflow-block: scroll)"));
        assert!(!e.evaluate_media_condition("(overflow-block: none)"));
    }

    #[test]
    fn overflow_inline_feature() {
        let e = engine();
        assert!(e.evaluate_media_condition("(overflow-inline: scroll)"));
        assert!(!e.evaluate_media_condition("(overflow-inline: none)"));
    }

    #[test]
    fn scripting_feature() {
        let e = engine();
        assert!(e.evaluate_media_condition("(scripting: enabled)"));
        assert!(!e.evaluate_media_condition("(scripting: none)"));
    }

    // ── Boolean (no-value) features ──────────────────────────────────────

    #[test]
    fn boolean_color() {
        let e = engine();
        assert!(e.evaluate_media_condition("(color)"));
    }

    #[test]
    fn boolean_hover() {
        let e = engine();
        assert!(e.evaluate_media_condition("(hover)"));
    }

    #[test]
    fn boolean_pointer() {
        let e = engine();
        assert!(e.evaluate_media_condition("(pointer)"));
    }

    #[test]
    fn boolean_grid_false() {
        let e = engine();
        assert!(!e.evaluate_media_condition("(grid)"));
    }

    #[test]
    fn boolean_monochrome_false() {
        let e = engine();
        assert!(!e.evaluate_media_condition("(monochrome)"));
    }

    // ── Existing condition handling ──────────────────────────────────────

    #[test]
    fn media_all_screen_print() {
        let e = engine();
        assert!(e.evaluate_media_condition("all"));
        assert!(e.evaluate_media_condition("screen"));
        assert!(!e.evaluate_media_condition("print"));
        assert!(e.evaluate_media_condition(""));
    }

    #[test]
    fn media_not() {
        let e = engine();
        assert!(e.evaluate_media_condition("not print"));
        assert!(!e.evaluate_media_condition("not screen"));
    }

    #[test]
    fn media_compound_and() {
        let e = engine();
        assert!(e.evaluate_media_condition("(min-width: 1024px) and (max-width: 2560px)"));
        assert!(!e.evaluate_media_condition("(min-width: 1024px) and (max-width: 800px)"));
    }

    #[test]
    fn media_or_list() {
        let e = engine();
        assert!(e.evaluate_media_condition("print, screen"));
        assert!(e.evaluate_media_condition("print, (min-width: 1024px)"));
        assert!(!e.evaluate_media_condition("print, (min-width: 3840px)"));
    }

    // ── Resolution parsing ───────────────────────────────────────────────

    #[test]
    fn parse_resolution_units() {
        assert_eq!(StyleEngine::parse_resolution_value("96dpi"), Some(96.0));
        assert_eq!(StyleEngine::parse_resolution_value("1dppx"), Some(96.0));
        assert_eq!(StyleEngine::parse_resolution_value("2dppx"), Some(192.0));
        // dpcm: 1 inch = 2.54 cm, so 37.8dpcm ≈ 96dpi
        let dpcm = StyleEngine::parse_resolution_value("37.795dpcm").unwrap();
        assert!((dpcm - 96.0).abs() < 0.1);
    }

    #[test]
    fn prefers_reduced_motion_reduce() {
        let mut e = engine();
        e.prefers_reduced_motion = true;
        assert!(e.evaluate_media_condition("(prefers-reduced-motion: reduce)"));
        assert!(!e.evaluate_media_condition("(prefers-reduced-motion: no-preference)"));
    }

    #[test]
    fn prefers_color_scheme_dark() {
        let mut e = engine();
        e.set_preferred_color_scheme("dark");
        assert!(e.evaluate_media_condition("(prefers-color-scheme: dark)"));
        assert!(!e.evaluate_media_condition("(prefers-color-scheme: light)"));
    }

    #[test]
    fn new_with_color_scheme_dark() {
        let e = StyleEngine::new_with_color_scheme(
            ViewportSize { width: 1920.0, height: 1080.0 },
            16.0,
            "dark",
        );
        assert_eq!(e.preferred_color_scheme, "dark");
        assert!(e.evaluate_media_condition("(prefers-color-scheme: dark)"));
        assert!(!e.evaluate_media_condition("(prefers-color-scheme: light)"));
    }

    #[test]
    fn new_with_color_scheme_light() {
        let e = StyleEngine::new_with_color_scheme(
            ViewportSize { width: 1920.0, height: 1080.0 },
            16.0,
            "light",
        );
        assert_eq!(e.preferred_color_scheme, "light");
        assert!(e.evaluate_media_condition("(prefers-color-scheme: light)"));
    }

    #[test]
    fn new_with_color_scheme_normalizes() {
        let e = StyleEngine::new_with_color_scheme(
            ViewportSize { width: 1920.0, height: 1080.0 },
            16.0,
            "  DARK  ",
        );
        assert_eq!(e.preferred_color_scheme, "dark");
    }

    #[test]
    fn new_with_color_scheme_unknown_defaults_light() {
        let e = StyleEngine::new_with_color_scheme(
            ViewportSize { width: 1920.0, height: 1080.0 },
            16.0,
            "sepia",
        );
        assert_eq!(e.preferred_color_scheme, "light");
    }
}
