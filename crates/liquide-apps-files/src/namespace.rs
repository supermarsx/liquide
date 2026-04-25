//! Shell namespace abstraction for virtual folders.
//!
//! Provides a unified tree of locations that includes physical directories
//! (Home, Documents, etc.), virtual folders (Trash, Recent, Favorites),
//! network locations, and mounted devices.  Modelled after the freedesktop
//! places concept and GNOME Files/Nautilus namespace model.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Node types
// ---------------------------------------------------------------------------

/// The kind of node in the namespace tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeType {
    Folder,
    File,
    VirtualFolder,
    Drive,
    Network,
    Trash,
    Recent,
    Favorites,
    Search,
}

impl std::fmt::Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Folder => write!(f, "folder"),
            Self::File => write!(f, "file"),
            Self::VirtualFolder => write!(f, "virtual-folder"),
            Self::Drive => write!(f, "drive"),
            Self::Network => write!(f, "network"),
            Self::Trash => write!(f, "trash"),
            Self::Recent => write!(f, "recent"),
            Self::Favorites => write!(f, "favorites"),
            Self::Search => write!(f, "search"),
        }
    }
}

// ---------------------------------------------------------------------------
// NamespaceNode trait
// ---------------------------------------------------------------------------

/// A single node in the namespace tree.
///
/// Implementors may represent a physical directory, a virtual folder (e.g.
/// Trash, Recent), a mounted drive, or a network share.
pub trait NamespaceNode {
    /// Display name shown in the sidebar / breadcrumb.
    fn name(&self) -> &str;

    /// Icon name (freedesktop icon-naming-spec).
    fn icon(&self) -> &str;

    /// URI that uniquely identifies this node (e.g. `file:///home/user`,
    /// `trash:///`, `recent:///`).
    fn uri(&self) -> &str;

    /// The type of this node.
    fn node_type(&self) -> NodeType;

    /// URI of the parent node, or `None` for root-level nodes.
    fn parent_uri(&self) -> Option<&str>;

    /// Ordered list of child URIs.  Empty for leaf nodes or nodes whose
    /// children are determined lazily (files on disk).
    fn children(&self) -> Vec<String>;
}

// ---------------------------------------------------------------------------
// StaticNode — simple concrete implementation
// ---------------------------------------------------------------------------

/// A concrete, in-memory namespace node.
#[derive(Debug, Clone)]
pub struct StaticNode {
    pub name: String,
    pub icon: String,
    pub uri: String,
    pub node_type: NodeType,
    pub parent_uri: Option<String>,
    pub children: Vec<String>,
}

impl NamespaceNode for StaticNode {
    fn name(&self) -> &str {
        &self.name
    }
    fn icon(&self) -> &str {
        &self.icon
    }
    fn uri(&self) -> &str {
        &self.uri
    }
    fn node_type(&self) -> NodeType {
        self.node_type
    }
    fn parent_uri(&self) -> Option<&str> {
        self.parent_uri.as_deref()
    }
    fn children(&self) -> Vec<String> {
        self.children.clone()
    }
}

// ---------------------------------------------------------------------------
// NamespaceProvider trait
// ---------------------------------------------------------------------------

/// Extension point: third-party code can register additional namespace
/// providers (e.g. cloud storage, MTP devices, GVFS mounts).
pub trait NamespaceProvider {
    /// Unique scheme handled by this provider (e.g. `"smb"`, `"mtp"`).
    fn scheme(&self) -> &str;

    /// Resolve a URI within this provider's scheme.
    fn resolve(&self, uri: &str) -> Option<StaticNode>;

    /// List the top-level nodes this provider contributes to the root.
    fn root_nodes(&self) -> Vec<StaticNode>;
}

// ---------------------------------------------------------------------------
// NamespaceRoot — the top-level tree
// ---------------------------------------------------------------------------

/// Top-level namespace tree that aggregates built-in roots and registered
/// providers.
pub struct NamespaceRoot {
    /// All known nodes keyed by URI.
    nodes: HashMap<String, StaticNode>,
    /// Registered providers keyed by scheme.
    providers: Vec<Box<dyn NamespaceProvider>>,
    /// Ordered list of root-level URIs.
    root_uris: Vec<String>,
}

impl NamespaceRoot {
    /// Create a namespace root pre-populated with the standard built-in
    /// locations.
    #[must_use]
    pub fn new() -> Self {
        let mut ns = Self {
            nodes: HashMap::new(),
            providers: Vec::new(),
            root_uris: Vec::new(),
        };
        ns.populate_builtins();
        ns
    }

    /// Create an empty namespace with no built-in roots.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            nodes: HashMap::new(),
            providers: Vec::new(),
            root_uris: Vec::new(),
        }
    }

    /// Register an external namespace provider.
    pub fn register_provider(&mut self, provider: Box<dyn NamespaceProvider>) {
        for node in provider.root_nodes() {
            let uri = node.uri.clone();
            self.nodes.insert(uri.clone(), node);
            if !self.root_uris.contains(&uri) {
                self.root_uris.push(uri);
            }
        }
        self.providers.push(provider);
    }

    /// Resolve a URI to a namespace node.
    ///
    /// Checks in-memory nodes first, then falls back to registered providers
    /// whose scheme matches the URI prefix.
    #[must_use]
    pub fn resolve(&self, uri: &str) -> Option<&StaticNode> {
        if let Some(node) = self.nodes.get(uri) {
            return Some(node);
        }
        None
    }

    /// Resolve a URI, checking providers if the static map has no match.
    pub fn resolve_dynamic(&self, uri: &str) -> Option<StaticNode> {
        if let Some(node) = self.nodes.get(uri) {
            return Some(node.clone());
        }
        let scheme = uri_scheme(uri);
        for provider in &self.providers {
            if provider.scheme() == scheme {
                if let Some(node) = provider.resolve(uri) {
                    return Some(node);
                }
            }
        }
        None
    }

    /// Insert or update a node.
    pub fn insert(&mut self, node: StaticNode) {
        let uri = node.uri.clone();
        let is_root = node.parent_uri.is_none();
        self.nodes.insert(uri.clone(), node);
        if is_root && !self.root_uris.contains(&uri) {
            self.root_uris.push(uri);
        }
    }

    /// Remove a node by URI.
    pub fn remove(&mut self, uri: &str) -> bool {
        self.root_uris.retain(|u| u != uri);
        self.nodes.remove(uri).is_some()
    }

    /// List all root-level URIs.
    #[must_use]
    pub fn root_uris(&self) -> &[String] {
        &self.root_uris
    }

    /// Number of nodes in the tree.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Iterate all nodes.
    pub fn nodes(&self) -> impl Iterator<Item = &StaticNode> {
        self.nodes.values()
    }

    /// Get children of a node.
    #[must_use]
    pub fn children_of(&self, uri: &str) -> Vec<&StaticNode> {
        self.nodes
            .values()
            .filter(|n| n.parent_uri.as_deref() == Some(uri))
            .collect()
    }

    // -----------------------------------------------------------------------
    // Built-in population
    // -----------------------------------------------------------------------

    fn populate_builtins(&mut self) {
        let home = home_dir();

        let builtins: Vec<(&str, &str, String, NodeType)> = vec![
            (
                "Home",
                "folder-home",
                format!("file://{home}"),
                NodeType::Folder,
            ),
            (
                "Desktop",
                "folder-desktop",
                format!("file://{home}/Desktop"),
                NodeType::Folder,
            ),
            (
                "Documents",
                "folder-documents",
                format!("file://{home}/Documents"),
                NodeType::Folder,
            ),
            (
                "Downloads",
                "folder-download",
                format!("file://{home}/Downloads"),
                NodeType::Folder,
            ),
            (
                "Music",
                "folder-music",
                format!("file://{home}/Music"),
                NodeType::Folder,
            ),
            (
                "Pictures",
                "folder-pictures",
                format!("file://{home}/Pictures"),
                NodeType::Folder,
            ),
            (
                "Videos",
                "folder-videos",
                format!("file://{home}/Videos"),
                NodeType::Folder,
            ),
            (
                "Trash",
                "user-trash",
                "trash:///".to_string(),
                NodeType::Trash,
            ),
            (
                "Recent",
                "document-open-recent",
                "recent:///".to_string(),
                NodeType::Recent,
            ),
            (
                "Network",
                "network-workgroup",
                "network:///".to_string(),
                NodeType::Network,
            ),
            (
                "Devices",
                "drive-harddisk",
                "devices:///".to_string(),
                NodeType::Drive,
            ),
        ];

        for (name, icon, uri, ntype) in builtins {
            let node = StaticNode {
                name: name.to_string(),
                icon: icon.to_string(),
                uri: uri.clone(),
                node_type: ntype,
                parent_uri: None,
                children: Vec::new(),
            };
            self.root_uris.push(uri.clone());
            self.nodes.insert(uri, node);
        }
    }
}

impl Default for NamespaceRoot {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Free-standing URI resolution
// ---------------------------------------------------------------------------

/// Resolve a URI string to a `StaticNode` using the built-in scheme handlers.
///
/// Recognised schemes: `file://`, `trash://`, `recent://`, `favorites://`,
/// `search://`.
#[must_use]
pub fn resolve_uri(uri: &str) -> Option<StaticNode> {
    let scheme = uri_scheme(uri);
    match scheme {
        "file" => {
            let path = uri.strip_prefix("file://").unwrap_or(uri);
            let name = path.rsplit('/').next().unwrap_or(path);
            let display = if name.is_empty() { "/" } else { name };
            Some(StaticNode {
                name: display.to_string(),
                icon: "folder".to_string(),
                uri: uri.to_string(),
                node_type: NodeType::Folder,
                parent_uri: None,
                children: Vec::new(),
            })
        }
        "trash" => Some(StaticNode {
            name: "Trash".to_string(),
            icon: "user-trash".to_string(),
            uri: "trash:///".to_string(),
            node_type: NodeType::Trash,
            parent_uri: None,
            children: Vec::new(),
        }),
        "recent" => Some(StaticNode {
            name: "Recent".to_string(),
            icon: "document-open-recent".to_string(),
            uri: "recent:///".to_string(),
            node_type: NodeType::Recent,
            parent_uri: None,
            children: Vec::new(),
        }),
        "favorites" => Some(StaticNode {
            name: "Favorites".to_string(),
            icon: "starred".to_string(),
            uri: "favorites:///".to_string(),
            node_type: NodeType::Favorites,
            parent_uri: None,
            children: Vec::new(),
        }),
        "search" => {
            let query = uri.strip_prefix("search:///").unwrap_or("");
            Some(StaticNode {
                name: format!("Search: {query}"),
                icon: "system-search".to_string(),
                uri: uri.to_string(),
                node_type: NodeType::Search,
                parent_uri: None,
                children: Vec::new(),
            })
        }
        _ => None,
    }
}

/// Extract the scheme portion of a URI (everything before `://`).
#[must_use]
pub fn uri_scheme(uri: &str) -> &str {
    uri.split("://").next().unwrap_or("")
}

/// Platform-independent home directory helper.
fn home_dir() -> String {
    if let Ok(home) = std::env::var("HOME") {
        return home;
    }
    #[cfg(target_os = "windows")]
    if let Ok(profile) = std::env::var("USERPROFILE") {
        return profile.replace('\\', "/");
    }
    "/home/user".to_string()
}
