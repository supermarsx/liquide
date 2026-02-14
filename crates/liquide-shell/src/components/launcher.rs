//! Launcher component — renders the app launcher overlay via the template engine.

use liquide_dom::PseudoStateFlags;

use crate::desktop_dom::{element_ids, LauncherItemInfo};
use crate::template::{Component, TemplateNode};

/// Launcher component that renders the full launcher overlay.
///
/// Produces a DOM tree like:
/// ```text
/// <launcher-overlay id="launcher-overlay">
///   <launcher id="shell-launcher">
///     <launcher-search id="launcher-search" data-query="fire" />
///     <launcher-results>
///       <launcher-item data-key="firefox" class="selected" data-app-id="firefox" data-icon="firefox">
///         <launcher-item-icon data-icon="firefox" />
///         <launcher-item-label>Firefox</launcher-item-label>
///       </launcher-item>
///       …
///     </launcher-results>
///   </launcher>
/// </launcher-overlay>
/// ```
///
/// **Fixes over old sync_dom**:
/// - Search query is synced to `data-query` attribute on `<launcher-search>`
/// - Selected item gets `.selected` class (CSS uses this, not just `:hover`)
/// - Each item has icon + label sub-elements for proper CSS targeting
pub struct LauncherComponent<'a> {
    pub items: &'a [LauncherItemInfo],
    pub selected_index: usize,
    pub search_query: &'a str,
    pub visible: bool,
}

impl Component for LauncherComponent<'_> {
    fn render(&self) -> TemplateNode {
        if !self.visible {
            // Return an empty overlay placeholder (will be unmounted)
            return TemplateNode::el("launcher-overlay")
                .id(element_ids::LAUNCHER_OVERLAY);
        }

        TemplateNode::el("launcher-overlay")
            .id(element_ids::LAUNCHER_OVERLAY)
            .child(
                TemplateNode::el("launcher")
                    .id(element_ids::LAUNCHER)
                    // Search box
                    .child(
                        TemplateNode::el("launcher-search")
                            .id(element_ids::LAUNCHER_SEARCH)
                            .attr("data-query", self.search_query)
                            .child(TemplateNode::text(
                                if self.search_query.is_empty() {
                                    "Search…"
                                } else {
                                    self.search_query
                                },
                            )),
                    )
                    // Results list
                    .child(
                        TemplateNode::el("launcher-results")
                            .children(self.items.iter().enumerate().map(|(i, item)| {
                                TemplateNode::el("launcher-item")
                                    .key(&item.app_id)
                                    .class_if("selected", i == self.selected_index)
                                    .attr("data-app-id", &item.app_id)
                                    .attr("data-icon", &item.icon)
                                    .attr("data-index", &i.to_string())
                                    .pseudo_if(
                                        PseudoStateFlags::HOVER,
                                        i == self.selected_index,
                                    )
                                    // Icon sub-element
                                    .child(
                                        TemplateNode::el("launcher-item-icon")
                                            .attr("data-icon", &item.icon),
                                    )
                                    // Label sub-element
                                    .child(
                                        TemplateNode::el("launcher-item-label")
                                            .child(TemplateNode::text(&item.label)),
                                    )
                            })),
                    ),
            )
    }

    fn mount_point(&self) -> &str {
        element_ids::LAUNCHER_OVERLAY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_items() -> Vec<LauncherItemInfo> {
        vec![
            LauncherItemInfo {
                app_id: "files".into(),
                label: "Files".into(),
                icon: "folder".into(),
            },
            LauncherItemInfo {
                app_id: "terminal".into(),
                label: "Terminal".into(),
                icon: "terminal".into(),
            },
            LauncherItemInfo {
                app_id: "browser".into(),
                label: "Browser".into(),
                icon: "globe".into(),
            },
        ]
    }

    #[test]
    fn launcher_renders_full_structure() {
        let items = make_items();
        let comp = LauncherComponent {
            items: &items,
            selected_index: 0,
            search_query: "",
            visible: true,
        };
        let tree = comp.render();

        assert_eq!(tree.tag, "launcher-overlay");
        assert_eq!(tree.children.len(), 1); // launcher

        let launcher = &tree.children[0];
        assert_eq!(launcher.tag, "launcher");
        assert_eq!(launcher.children.len(), 2); // search + results

        let results = &launcher.children[1];
        assert_eq!(results.children.len(), 3);
    }

    #[test]
    fn launcher_selected_item_has_class() {
        let items = make_items();
        let comp = LauncherComponent {
            items: &items,
            selected_index: 1,
            search_query: "",
            visible: true,
        };
        let tree = comp.render();

        let results = &tree.children[0].children[1];
        assert!(!results.children[0].classes.contains(&"selected".to_string()));
        assert!(results.children[1].classes.contains(&"selected".to_string()));
        assert!(!results.children[2].classes.contains(&"selected".to_string()));
    }

    #[test]
    fn launcher_search_query_synced() {
        let items = make_items();
        let comp = LauncherComponent {
            items: &items,
            selected_index: 0,
            search_query: "firefox",
            visible: true,
        };
        let tree = comp.render();

        let search = &tree.children[0].children[0];
        assert!(search.attrs.iter().any(|(k, v)| k == "data-query" && v == "firefox"));
        assert_eq!(search.children[0].text.as_deref(), Some("firefox"));
    }

    #[test]
    fn launcher_item_has_icon_and_label() {
        let items = make_items();
        let comp = LauncherComponent {
            items: &items,
            selected_index: 0,
            search_query: "",
            visible: true,
        };
        let tree = comp.render();

        let first_item = &tree.children[0].children[1].children[0];
        assert_eq!(first_item.children.len(), 2);
        assert_eq!(first_item.children[0].tag, "launcher-item-icon");
        assert_eq!(first_item.children[1].tag, "launcher-item-label");
    }

    #[test]
    fn launcher_not_visible_empty() {
        let items = make_items();
        let comp = LauncherComponent {
            items: &items,
            selected_index: 0,
            search_query: "",
            visible: false,
        };
        let tree = comp.render();

        // Should be an empty overlay with no children
        assert_eq!(tree.tag, "launcher-overlay");
        assert_eq!(tree.children.len(), 0);
    }
}
