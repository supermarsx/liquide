//! Status notifier host — the tray manager that receives and displays items.
//!
//! A host is the visual tray bar that users interact with. It tracks registered
//! items, enforces ordering (by category, then registration time), limits the
//! maximum number of items, and emits events when items change.

use std::collections::HashMap;

use crate::item::{ItemCategory, ItemId, ItemStatus, StatusNotifierItem, ToolTip};
use crate::menu::TrayMenu;

/// Maximum number of items a host will accept by default (abuse prevention).
pub const DEFAULT_MAX_ITEMS: usize = 64;

/// Events emitted by the tray host when items are added, removed, or updated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayEvent {
    /// A new item was registered.
    ItemRegistered(ItemId),
    /// An item was unregistered.
    ItemRemoved(ItemId),
    /// An item's properties changed (icon, title, tooltip, etc.).
    ItemUpdated(ItemId),
    /// An item's status changed (Passive/Active/NeedsAttention).
    StatusChanged {
        id: ItemId,
        old: ItemStatus,
        new: ItemStatus,
    },
    /// An item's tooltip changed.
    ToolTipChanged(ItemId),
}

impl TrayEvent {
    /// Returns the item ID associated with this event.
    pub fn item_id(&self) -> &str {
        match self {
            Self::ItemRegistered(id)
            | Self::ItemRemoved(id)
            | Self::ItemUpdated(id)
            | Self::ToolTipChanged(id) => id,
            Self::StatusChanged { id, .. } => id,
        }
    }
}

/// The tray host: manages a collection of status notifier items, provides
/// ordered access, and enforces a maximum item limit.
pub struct TrayHost {
    /// Items keyed by their ID.
    items: HashMap<ItemId, StatusNotifierItem>,
    /// Insertion-order tracking for stable sort within the same category.
    insertion_order: Vec<ItemId>,
    /// Maximum allowed items (prevents abuse from rogue applications).
    max_items: usize,
    /// Accumulated events since last drain.
    events: Vec<TrayEvent>,
}

impl TrayHost {
    /// Create a new, empty tray host with the default item limit.
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
            insertion_order: Vec::new(),
            max_items: DEFAULT_MAX_ITEMS,
            events: Vec::new(),
        }
    }

    /// Create a tray host with a custom maximum item limit.
    pub fn with_max_items(max_items: usize) -> Self {
        Self {
            max_items,
            ..Self::new()
        }
    }

    /// Register a new status notifier item. Returns `true` on success,
    /// `false` if the maximum item limit has been reached or the ID is
    /// already registered.
    pub fn register_item(&mut self, item: StatusNotifierItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        if self.items.contains_key(&item.id) {
            return false;
        }
        let id = item.id.clone();
        self.items.insert(id.clone(), item);
        self.insertion_order.push(id.clone());
        self.events.push(TrayEvent::ItemRegistered(id));
        true
    }

    /// Unregister a status notifier item by ID. Returns the removed item, if
    /// it was registered.
    pub fn unregister_item(&mut self, id: &str) -> Option<StatusNotifierItem> {
        let removed = self.items.remove(id);
        if removed.is_some() {
            self.insertion_order.retain(|i| i != id);
            self.events.push(TrayEvent::ItemRemoved(id.to_string()));
        }
        removed
    }

    /// Look up an item by ID.
    pub fn get_item(&self, id: &str) -> Option<&StatusNotifierItem> {
        self.items.get(id)
    }

    /// Look up an item mutably by ID.
    pub fn get_item_mut(&mut self, id: &str) -> Option<&mut StatusNotifierItem> {
        self.items.get_mut(id)
    }

    /// Returns an ordered slice of all registered items. Ordering: by category
    /// sort key (Hardware first, ApplicationStatus last), then by registration
    /// time within the same category.
    pub fn items(&self) -> Vec<&StatusNotifierItem> {
        let mut sorted: Vec<&StatusNotifierItem> = self.items.values().collect();
        let order_map: HashMap<&str, usize> = self
            .insertion_order
            .iter()
            .enumerate()
            .map(|(i, id)| (id.as_str(), i))
            .collect();
        sorted.sort_by(|a, b| {
            a.category
                .sort_key()
                .cmp(&b.category.sort_key())
                .then_with(|| {
                    let oa = order_map.get(a.id.as_str()).copied().unwrap_or(usize::MAX);
                    let ob = order_map.get(b.id.as_str()).copied().unwrap_or(usize::MAX);
                    oa.cmp(&ob)
                })
        });
        sorted
    }

    /// Returns only items whose status is `Active` or `NeedsAttention`.
    pub fn visible_items(&self) -> Vec<&StatusNotifierItem> {
        self.items()
            .into_iter()
            .filter(|item| item.status.is_visible())
            .collect()
    }

    /// Returns items that currently need user attention.
    pub fn attention_items(&self) -> Vec<&StatusNotifierItem> {
        self.items()
            .into_iter()
            .filter(|item| item.needs_attention())
            .collect()
    }

    /// Total number of registered items.
    pub fn count(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` if there are no registered items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The current maximum item limit.
    pub fn max_items(&self) -> usize {
        self.max_items
    }

    /// Change the maximum item limit.
    pub fn set_max_items(&mut self, max: usize) {
        self.max_items = max;
    }

    /// Update the status of an item. Returns `true` if the item exists.
    /// Emits `StatusChanged` if the status actually changed.
    pub fn update_status(&mut self, id: &str, new_status: ItemStatus) -> bool {
        if let Some(item) = self.items.get_mut(id) {
            let old = item.status;
            if old != new_status {
                item.status = new_status;
                self.events.push(TrayEvent::StatusChanged {
                    id: id.to_string(),
                    old,
                    new: new_status,
                });
            }
            true
        } else {
            false
        }
    }

    /// Update the icon name of an item. Returns `true` if the item exists.
    pub fn update_icon(&mut self, id: &str, icon_name: impl Into<String>) -> bool {
        if let Some(item) = self.items.get_mut(id) {
            item.icon_name = icon_name.into();
            self.events.push(TrayEvent::ItemUpdated(id.to_string()));
            true
        } else {
            false
        }
    }

    /// Update the title of an item. Returns `true` if the item exists.
    pub fn update_title(&mut self, id: &str, title: impl Into<String>) -> bool {
        if let Some(item) = self.items.get_mut(id) {
            item.title = title.into();
            self.events.push(TrayEvent::ItemUpdated(id.to_string()));
            true
        } else {
            false
        }
    }

    /// Update the tooltip of an item. Returns `true` if the item exists.
    pub fn update_tooltip(&mut self, id: &str, tooltip: ToolTip) -> bool {
        if let Some(item) = self.items.get_mut(id) {
            item.tooltip = Some(tooltip);
            self.events.push(TrayEvent::ToolTipChanged(id.to_string()));
            true
        } else {
            false
        }
    }

    /// Update the menu of an item. Returns `true` if the item exists.
    pub fn update_menu(&mut self, id: &str, menu: TrayMenu) -> bool {
        if let Some(item) = self.items.get_mut(id) {
            item.menu = Some(menu);
            self.events.push(TrayEvent::ItemUpdated(id.to_string()));
            true
        } else {
            false
        }
    }

    /// Drain all accumulated events since the last call.
    pub fn drain_events(&mut self) -> Vec<TrayEvent> {
        std::mem::take(&mut self.events)
    }

    /// Peek at accumulated events without draining.
    pub fn pending_events(&self) -> &[TrayEvent] {
        &self.events
    }

    /// Filter items by category.
    pub fn items_by_category(&self, category: ItemCategory) -> Vec<&StatusNotifierItem> {
        self.items()
            .into_iter()
            .filter(|item| item.category == category)
            .collect()
    }

    /// Returns `true` if any item currently needs attention.
    pub fn has_attention(&self) -> bool {
        self.items
            .values()
            .any(|item| item.status == ItemStatus::NeedsAttention)
    }
}

impl Default for TrayHost {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TrayHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let visible = self.visible_items().len();
        write!(
            f,
            "TrayHost({} items, {} visible, max={})",
            self.count(),
            visible,
            self.max_items
        )
    }
}
