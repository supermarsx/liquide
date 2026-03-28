use crate::PluginId;

/// A context menu item provided by a plugin
#[derive(Debug, Clone)]
pub struct ContextMenuItem {
    pub id: String,
    pub label: String,
    pub icon: Option<String>,
    pub shortcut: Option<String>,
    pub enabled: bool,
    pub visible: bool,
    pub submenu: Option<Vec<ContextMenuItem>>,
    pub separator_before: bool,
    pub plugin_id: PluginId,
}

/// Context for a context menu request
#[derive(Debug, Clone)]
pub struct MenuContext {
    /// What was right-clicked
    pub target: MenuTarget,
    /// Selected file paths (for file manager context)
    pub selected_paths: Vec<std::path::PathBuf>,
    /// Window ID if targeting a window
    pub window_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuTarget {
    Desktop,
    File,
    Folder,
    Window,
    Statusbar,
    Dock,
    Custom(String),
}

/// Plugin implements this to provide context menu items
pub trait ContextMenuProvider: Send + Sync {
    /// Return items for the given context
    fn query_items(&self, context: &MenuContext) -> Vec<ContextMenuItem>;

    /// Execute a menu item action
    fn execute(&mut self, item_id: &str, context: &MenuContext) -> Result<(), String>;

    /// Supported menu targets
    fn supported_targets(&self) -> Vec<MenuTarget>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn menu_context_creation() {
        let ctx = MenuContext {
            target: MenuTarget::Desktop,
            selected_paths: vec![],
            window_id: None,
        };
        assert_eq!(ctx.target, MenuTarget::Desktop);
        assert!(ctx.selected_paths.is_empty());
        assert!(ctx.window_id.is_none());
    }

    #[test]
    fn menu_context_with_paths() {
        let ctx = MenuContext {
            target: MenuTarget::File,
            selected_paths: vec![
                PathBuf::from("/home/user/file.txt"),
                PathBuf::from("/home/user/image.png"),
            ],
            window_id: Some(42),
        };
        assert_eq!(ctx.target, MenuTarget::File);
        assert_eq!(ctx.selected_paths.len(), 2);
        assert_eq!(ctx.window_id, Some(42));
    }

    #[test]
    fn context_menu_item_defaults() {
        let item = ContextMenuItem {
            id: "action.paste".into(),
            label: "Paste".into(),
            icon: None,
            shortcut: Some("Ctrl+V".into()),
            enabled: true,
            visible: true,
            submenu: None,
            separator_before: false,
            plugin_id: PluginId("test".into()),
        };
        assert_eq!(item.id, "action.paste");
        assert_eq!(item.label, "Paste");
        assert!(item.icon.is_none());
        assert_eq!(item.shortcut, Some("Ctrl+V".into()));
        assert!(item.enabled);
        assert!(item.visible);
        assert!(item.submenu.is_none());
        assert!(!item.separator_before);
    }

    #[test]
    fn context_menu_item_with_submenu() {
        let child = ContextMenuItem {
            id: "sub.child".into(),
            label: "Child Item".into(),
            icon: None,
            shortcut: None,
            enabled: true,
            visible: true,
            submenu: None,
            separator_before: false,
            plugin_id: PluginId("test".into()),
        };
        let parent = ContextMenuItem {
            id: "sub.parent".into(),
            label: "Parent".into(),
            icon: Some("folder".into()),
            shortcut: None,
            enabled: true,
            visible: true,
            submenu: Some(vec![child]),
            separator_before: true,
            plugin_id: PluginId("test".into()),
        };
        assert!(parent.submenu.is_some());
        assert_eq!(parent.submenu.as_ref().unwrap().len(), 1);
        assert!(parent.separator_before);
    }

    #[test]
    fn menu_target_custom() {
        let target = MenuTarget::Custom("panel".into());
        assert_eq!(target, MenuTarget::Custom("panel".into()));
        assert_ne!(target, MenuTarget::Desktop);
    }

    #[test]
    fn menu_target_equality() {
        assert_eq!(MenuTarget::Desktop, MenuTarget::Desktop);
        assert_eq!(MenuTarget::File, MenuTarget::File);
        assert_ne!(MenuTarget::File, MenuTarget::Folder);
        assert_ne!(MenuTarget::Window, MenuTarget::Dock);
    }
}
