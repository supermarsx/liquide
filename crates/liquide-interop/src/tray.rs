use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{InteropError, Result};

/// Status of a tray item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrayItemStatus {
    Active,
    Passive,
    NeedsAttention,
}

/// A menu item within a tray item's context menu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrayMenuItem {
    pub id: String,
    pub label: String,
    pub enabled: bool,
    pub separator: bool,
}

impl TrayMenuItem {
    #[must_use]
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            enabled: true,
            separator: false,
        }
    }

    #[must_use]
    pub fn separator() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            enabled: false,
            separator: true,
        }
    }
}

/// A system tray item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrayItem {
    pub id: String,
    pub title: String,
    pub icon: Option<String>,
    pub tooltip: Option<String>,
    pub menu: Vec<TrayMenuItem>,
    pub status: TrayItemStatus,
}

impl TrayItem {
    #[must_use]
    pub fn new(id: &str, title: &str) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            icon: None,
            tooltip: None,
            menu: Vec::new(),
            status: TrayItemStatus::Active,
        }
    }
}

impl fmt::Display for TrayItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TrayItem(id={}, title={}, status={:?})",
            self.id, self.title, self.status
        )
    }
}

/// System tray — manages a set of tray items.
#[derive(Debug, Clone)]
pub struct SystemTray {
    items: Vec<TrayItem>,
}

impl SystemTray {
    #[must_use]
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Add an item to the tray.
    pub fn add_item(&mut self, item: TrayItem) {
        self.items.push(item);
    }

    /// Remove an item by ID.
    pub fn remove_item(&mut self, id: &str) -> Result<()> {
        let before = self.items.len();
        self.items.retain(|i| i.id != id);
        if self.items.len() == before {
            return Err(InteropError::NotFound {
                kind: "TrayItem".to_string(),
                name: id.to_string(),
            });
        }
        Ok(())
    }

    /// Update an item by ID using a callback.
    pub fn update_item<F>(&mut self, id: &str, f: F) -> Result<()>
    where
        F: FnOnce(&mut TrayItem),
    {
        let item = self.items.iter_mut().find(|i| i.id == id).ok_or_else(|| {
            InteropError::NotFound {
                kind: "TrayItem".to_string(),
                name: id.to_string(),
            }
        })?;
        f(item);
        Ok(())
    }

    /// Get all items.
    #[must_use]
    pub fn items(&self) -> &[TrayItem] {
        &self.items
    }

    /// Find an item by ID.
    #[must_use]
    pub fn find(&self, id: &str) -> Option<&TrayItem> {
        self.items.iter().find(|i| i.id == id)
    }

    /// Number of tray items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the tray is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl Default for SystemTray {
    fn default() -> Self {
        Self::new()
    }
}
