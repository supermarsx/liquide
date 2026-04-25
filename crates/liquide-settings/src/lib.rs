pub mod dconf;
pub mod lockdown;
pub mod panels;
pub mod policy;
pub mod profile;
pub mod schema;
pub mod store;

pub use dconf::{DconfError, DconfLock, DconfPath, DconfStore};
pub use lockdown::{Feature, LockdownManager, LockdownProfile};
pub use policy::{PolicyDatabase, PolicyEntry, PolicyError, PolicyKey, PolicySource, PolicyValue};
pub use profile::{ProfileError, ProfileStore, UserProfile};
pub use schema::{SchemaEntry, SchemaValueType, SettingsSchema, ValidationError};
pub use schema::{Setting, SettingCategory, SettingKey, SettingValue};
pub use store::{SettingsError, SettingsStore};

#[cfg(test)]
mod tests;
