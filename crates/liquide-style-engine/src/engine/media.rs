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
    fn evaluate_media_feature(&self, feature: &str) -> bool {
        let feature = feature.trim();
        if let Some(colon_pos) = feature.find(':') {
            let name = feature[..colon_pos].trim();
            let value_str = feature[colon_pos + 1..].trim();

            match name {
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
                "prefers-color-scheme" => {
                    return value_str.trim().eq_ignore_ascii_case(&self.preferred_color_scheme);
                }
                "prefers-reduced-motion" => {
                    return (value_str == "reduce") == self.prefers_reduced_motion;
                }
                _ => {}
            }
        }
        // Unknown feature — include by default
        true
    }

    /// Parse a pixel value like "768px" or "1024px".
    pub(crate) fn parse_px_value(s: &str) -> Option<f32> {
        let s = s.trim();
        let num_str = s.strip_suffix("px").unwrap_or(s);
        num_str.trim().parse::<f32>().ok()
    }
}
