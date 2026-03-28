pub mod schema;
pub mod store;
pub mod panels;
pub mod policy;
pub mod lockdown;
pub mod dconf;
pub mod profile;

pub use schema::{Setting, SettingValue, SettingKey, SettingCategory};
pub use schema::{SchemaEntry, SchemaValueType, SettingsSchema, ValidationError};
pub use store::{SettingsStore, SettingsError};
pub use policy::{PolicyKey, PolicyValue, PolicySource, PolicyDatabase, PolicyEntry, PolicyError};
pub use lockdown::{Feature, LockdownProfile, LockdownManager};
pub use dconf::{DconfPath, DconfStore, DconfLock, DconfError};
pub use profile::{UserProfile, ProfileStore, ProfileError};

#[cfg(test)]
mod tests;
