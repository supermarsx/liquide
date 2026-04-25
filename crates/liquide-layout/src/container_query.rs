//! Container query evaluation — records container sizes during layout
//! and feeds them back to the style engine for `@container` rule evaluation.
//!
//! CSS container queries allow elements to be styled based on the size of
//! their nearest container ancestor with `container-type: size | inline-size`.
//!
//! ## Lifecycle
//!
//! 1. **Style resolve (initial)**: `@container` rules are skipped if no
//!    container sizes are known yet.
//! 2. **Layout pass**: For each element with `container-type != normal`,
//!    record its computed size in `ContainerSizeMap`.
//! 3. **Re-style (conditional)**: If any container sizes changed, re-run
//!    style resolution for descendants of affected containers.
//! 4. **Re-layout**: Only if styles actually changed.

use std::collections::HashMap;

use liquide_dom::NodeId;
use liquide_style_engine::computed::ContainerType;

/// Recorded container size for a given node.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContainerSize {
    /// Inline-axis size (width in horizontal-tb).
    pub inline_size: f32,
    /// Block-axis size (height in horizontal-tb).
    pub block_size: f32,
    /// What type of container this is.
    pub container_type: ContainerType,
}

/// A map of container node IDs to their computed sizes.
#[derive(Debug, Clone, Default)]
pub struct ContainerSizeMap {
    sizes: HashMap<NodeId, ContainerSize>,
}

impl ContainerSizeMap {
    pub fn new() -> Self {
        Self {
            sizes: HashMap::new(),
        }
    }

    /// Record the computed size of a container.
    /// Returns true if the size changed (triggering a potential re-style).
    pub fn record(&mut self, node_id: NodeId, size: ContainerSize) -> bool {
        if let Some(existing) = self.sizes.get(&node_id) {
            if (existing.inline_size - size.inline_size).abs() < 0.01
                && (existing.block_size - size.block_size).abs() < 0.01
            {
                return false; // No change
            }
        }
        self.sizes.insert(node_id, size);
        true
    }

    /// Get the container size for a given node.
    pub fn get(&self, node_id: NodeId) -> Option<&ContainerSize> {
        self.sizes.get(&node_id)
    }

    /// Find the nearest container ancestor for a given node, querying the DOM.
    pub fn find_container(
        &self,
        node_id: NodeId,
        container_name: Option<&str>,
        doc: &liquide_dom::Document,
        styles: &liquide_style_engine::StyleMap,
    ) -> Option<ContainerSize> {
        let mut current = doc.parent(node_id);
        while let Some(ancestor_id) = current {
            if let Some(ancestor_style) = styles.get(ancestor_id) {
                if ancestor_style.is_container_query_host() {
                    // Check container-name match if specified
                    if let Some(name) = container_name {
                        if ancestor_style.container_name.as_deref() != Some(name) {
                            current = doc.parent(ancestor_id);
                            continue;
                        }
                    }
                    if let Some(size) = self.sizes.get(&ancestor_id) {
                        return Some(*size);
                    }
                }
            }
            current = doc.parent(ancestor_id);
        }
        None
    }

    /// Evaluate a container query condition.
    ///
    /// Supports:
    /// - `min-width: <px>`, `max-width: <px>`
    /// - `min-height: <px>`, `max-height: <px>`
    /// - `width > <px>`, `width < <px>`, `width = <px>`
    /// - `orientation: portrait | landscape`
    pub fn evaluate_condition(&self, container: &ContainerSize, condition: &str) -> bool {
        let condition = condition.trim();

        // orientation: portrait | landscape
        if condition.starts_with("orientation:") {
            let val = condition["orientation:".len()..].trim();
            return match val {
                "portrait" => container.block_size > container.inline_size,
                "landscape" => container.inline_size > container.block_size,
                _ => false,
            };
        }

        // min-width / max-width / min-height / max-height
        if let Some(val) = condition.strip_prefix("min-width:") {
            if let Some(px) = parse_px(val) {
                return container.inline_size >= px;
            }
        }
        if let Some(val) = condition.strip_prefix("max-width:") {
            if let Some(px) = parse_px(val) {
                return container.inline_size <= px;
            }
        }
        if let Some(val) = condition.strip_prefix("min-height:") {
            if let Some(px) = parse_px(val) {
                return container.block_size >= px;
            }
        }
        if let Some(val) = condition.strip_prefix("max-height:") {
            if let Some(px) = parse_px(val) {
                return container.block_size <= px;
            }
        }

        // width > / < / =
        if let Some(rest) = condition.strip_prefix("width") {
            let rest = rest.trim();
            if let Some(val) = rest.strip_prefix(">=") {
                if let Some(px) = parse_px(val) {
                    return container.inline_size >= px;
                }
            } else if let Some(val) = rest.strip_prefix("<=") {
                if let Some(px) = parse_px(val) {
                    return container.inline_size <= px;
                }
            } else if let Some(val) = rest.strip_prefix('>') {
                if let Some(px) = parse_px(val) {
                    return container.inline_size > px;
                }
            } else if let Some(val) = rest.strip_prefix('<') {
                if let Some(px) = parse_px(val) {
                    return container.inline_size < px;
                }
            } else if let Some(val) = rest.strip_prefix('=') {
                if let Some(px) = parse_px(val) {
                    return (container.inline_size - px).abs() < 0.5;
                }
            }
        }

        // height > / < / =
        if let Some(rest) = condition.strip_prefix("height") {
            let rest = rest.trim();
            if let Some(val) = rest.strip_prefix(">=") {
                if let Some(px) = parse_px(val) {
                    return container.block_size >= px;
                }
            } else if let Some(val) = rest.strip_prefix("<=") {
                if let Some(px) = parse_px(val) {
                    return container.block_size <= px;
                }
            } else if let Some(val) = rest.strip_prefix('>') {
                if let Some(px) = parse_px(val) {
                    return container.block_size > px;
                }
            } else if let Some(val) = rest.strip_prefix('<') {
                if let Some(px) = parse_px(val) {
                    return container.block_size < px;
                }
            } else if let Some(val) = rest.strip_prefix('=') {
                if let Some(px) = parse_px(val) {
                    return (container.block_size - px).abs() < 0.5;
                }
            }
        }

        false // Unknown condition
    }

    /// Clear all recorded sizes (for full re-layout).
    pub fn clear(&mut self) {
        self.sizes.clear();
    }

    /// Number of tracked containers.
    pub fn len(&self) -> usize {
        self.sizes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sizes.is_empty()
    }
}

/// Parse a CSS pixel value like "300px" or "300".
fn parse_px(s: &str) -> Option<f32> {
    let s = s.trim();
    let s = s.strip_suffix("px").unwrap_or(s);
    s.trim().parse::<f32>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_and_get() {
        let mut map = ContainerSizeMap::new();
        let changed = map.record(
            42,
            ContainerSize {
                inline_size: 300.0,
                block_size: 200.0,
                container_type: ContainerType::InlineSize,
            },
        );
        assert!(changed);
        assert_eq!(map.get(42).unwrap().inline_size, 300.0);
    }

    #[test]
    fn test_no_change() {
        let mut map = ContainerSizeMap::new();
        map.record(
            42,
            ContainerSize {
                inline_size: 300.0,
                block_size: 200.0,
                container_type: ContainerType::Size,
            },
        );
        let changed = map.record(
            42,
            ContainerSize {
                inline_size: 300.0,
                block_size: 200.0,
                container_type: ContainerType::Size,
            },
        );
        assert!(!changed);
    }

    #[test]
    fn test_evaluate_min_width() {
        let map = ContainerSizeMap::new();
        let size = ContainerSize {
            inline_size: 500.0,
            block_size: 300.0,
            container_type: ContainerType::InlineSize,
        };
        assert!(map.evaluate_condition(&size, "min-width: 400px"));
        assert!(!map.evaluate_condition(&size, "min-width: 600px"));
    }

    #[test]
    fn test_evaluate_orientation() {
        let map = ContainerSizeMap::new();
        let landscape = ContainerSize {
            inline_size: 500.0,
            block_size: 300.0,
            container_type: ContainerType::Size,
        };
        assert!(map.evaluate_condition(&landscape, "orientation: landscape"));
        assert!(!map.evaluate_condition(&landscape, "orientation: portrait"));
    }

    #[test]
    fn test_evaluate_comparison() {
        let map = ContainerSizeMap::new();
        let size = ContainerSize {
            inline_size: 400.0,
            block_size: 300.0,
            container_type: ContainerType::Size,
        };
        assert!(map.evaluate_condition(&size, "width > 300px"));
        assert!(!map.evaluate_condition(&size, "width > 500px"));
        assert!(map.evaluate_condition(&size, "height <= 300px"));
    }
}
