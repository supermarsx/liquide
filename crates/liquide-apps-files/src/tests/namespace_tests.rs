//! Tests for the namespace module.

use crate::namespace::*;

#[test]
fn test_node_type_display() {
    assert_eq!(NodeType::Folder.to_string(), "folder");
    assert_eq!(NodeType::VirtualFolder.to_string(), "virtual-folder");
    assert_eq!(NodeType::Trash.to_string(), "trash");
    assert_eq!(NodeType::Search.to_string(), "search");
}

#[test]
fn test_static_node_implements_trait() {
    let node = StaticNode {
        name: "Home".into(),
        icon: "folder-home".into(),
        uri: "file:///home/user".into(),
        node_type: NodeType::Folder,
        parent_uri: None,
        children: vec!["file:///home/user/Documents".into()],
    };
    assert_eq!(node.name(), "Home");
    assert_eq!(node.icon(), "folder-home");
    assert_eq!(node.uri(), "file:///home/user");
    assert_eq!(node.node_type(), NodeType::Folder);
    assert!(node.parent_uri().is_none());
    assert_eq!(node.children().len(), 1);
}

#[test]
fn test_namespace_root_has_builtins() {
    let ns = NamespaceRoot::new();
    let uris = ns.root_uris();
    assert!(uris.len() >= 11); // Home..Videos + Trash + Recent + Network + Devices
    assert!(uris.iter().any(|u| u == "trash:///"));
    assert!(uris.iter().any(|u| u == "recent:///"));
    assert!(uris.iter().any(|u| u == "network:///"));
    assert!(uris.iter().any(|u| u == "devices:///"));
}

#[test]
fn test_namespace_root_resolve_builtin() {
    let ns = NamespaceRoot::new();
    let trash = ns.resolve("trash:///").unwrap();
    assert_eq!(trash.name, "Trash");
    assert_eq!(trash.node_type, NodeType::Trash);
}

#[test]
fn test_namespace_root_resolve_unknown() {
    let ns = NamespaceRoot::new();
    assert!(ns.resolve("ftp:///somewhere").is_none());
}

#[test]
fn test_namespace_root_empty() {
    let ns = NamespaceRoot::empty();
    assert_eq!(ns.node_count(), 0);
    assert!(ns.root_uris().is_empty());
}

#[test]
fn test_namespace_root_insert_and_remove() {
    let mut ns = NamespaceRoot::empty();
    let node = StaticNode {
        name: "Custom".into(),
        icon: "folder".into(),
        uri: "custom:///test".into(),
        node_type: NodeType::VirtualFolder,
        parent_uri: None,
        children: Vec::new(),
    };
    ns.insert(node);
    assert_eq!(ns.node_count(), 1);
    assert!(ns.resolve("custom:///test").is_some());

    assert!(ns.remove("custom:///test"));
    assert_eq!(ns.node_count(), 0);
    assert!(!ns.remove("custom:///test")); // already gone
}

#[test]
fn test_namespace_root_children_of() {
    let mut ns = NamespaceRoot::empty();
    let parent = StaticNode {
        name: "Parent".into(),
        icon: "folder".into(),
        uri: "file:///parent".into(),
        node_type: NodeType::Folder,
        parent_uri: None,
        children: Vec::new(),
    };
    let child = StaticNode {
        name: "Child".into(),
        icon: "folder".into(),
        uri: "file:///parent/child".into(),
        node_type: NodeType::Folder,
        parent_uri: Some("file:///parent".into()),
        children: Vec::new(),
    };
    ns.insert(parent);
    ns.insert(child);
    let children = ns.children_of("file:///parent");
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].name, "Child");
}

#[test]
fn test_resolve_uri_file() {
    let node = resolve_uri("file:///home/user/Documents").unwrap();
    assert_eq!(node.name, "Documents");
    assert_eq!(node.node_type, NodeType::Folder);
}

#[test]
fn test_resolve_uri_file_root() {
    let node = resolve_uri("file:///").unwrap();
    assert_eq!(node.name, "/");
}

#[test]
fn test_resolve_uri_trash() {
    let node = resolve_uri("trash:///").unwrap();
    assert_eq!(node.name, "Trash");
    assert_eq!(node.node_type, NodeType::Trash);
}

#[test]
fn test_resolve_uri_recent() {
    let node = resolve_uri("recent:///").unwrap();
    assert_eq!(node.name, "Recent");
    assert_eq!(node.node_type, NodeType::Recent);
}

#[test]
fn test_resolve_uri_favorites() {
    let node = resolve_uri("favorites:///").unwrap();
    assert_eq!(node.name, "Favorites");
    assert_eq!(node.node_type, NodeType::Favorites);
}

#[test]
fn test_resolve_uri_search() {
    let node = resolve_uri("search:///hello").unwrap();
    assert_eq!(node.name, "Search: hello");
    assert_eq!(node.node_type, NodeType::Search);
}

#[test]
fn test_resolve_uri_unknown_scheme() {
    assert!(resolve_uri("ftp:///server").is_none());
}

#[test]
fn test_uri_scheme() {
    assert_eq!(uri_scheme("file:///home"), "file");
    assert_eq!(uri_scheme("trash:///"), "trash");
    assert_eq!(uri_scheme("smb://server/share"), "smb");
    assert_eq!(uri_scheme("noscheme"), "noscheme");
}

// Provider test
struct TestProvider;

impl NamespaceProvider for TestProvider {
    fn scheme(&self) -> &str {
        "test"
    }
    fn resolve(&self, uri: &str) -> Option<StaticNode> {
        if uri == "test:///item" {
            Some(StaticNode {
                name: "TestItem".into(),
                icon: "test".into(),
                uri: "test:///item".into(),
                node_type: NodeType::VirtualFolder,
                parent_uri: None,
                children: Vec::new(),
            })
        } else {
            None
        }
    }
    fn root_nodes(&self) -> Vec<StaticNode> {
        vec![StaticNode {
            name: "TestRoot".into(),
            icon: "test".into(),
            uri: "test:///".into(),
            node_type: NodeType::VirtualFolder,
            parent_uri: None,
            children: Vec::new(),
        }]
    }
}

#[test]
fn test_register_provider() {
    let mut ns = NamespaceRoot::empty();
    ns.register_provider(Box::new(TestProvider));
    // Root node from provider should be in the tree.
    assert!(ns.resolve("test:///").is_some());
    assert!(ns.root_uris().contains(&"test:///".to_string()));
}

#[test]
fn test_resolve_dynamic_from_provider() {
    let mut ns = NamespaceRoot::empty();
    ns.register_provider(Box::new(TestProvider));
    let node = ns.resolve_dynamic("test:///item").unwrap();
    assert_eq!(node.name, "TestItem");
}

#[test]
fn test_resolve_dynamic_unknown() {
    let ns = NamespaceRoot::empty();
    assert!(ns.resolve_dynamic("nope:///x").is_none());
}

#[test]
fn test_namespace_nodes_iterator() {
    let ns = NamespaceRoot::new();
    let count = ns.nodes().count();
    assert_eq!(count, ns.node_count());
    assert!(count >= 11);
}
