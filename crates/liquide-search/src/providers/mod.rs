//! Built-in search providers.

pub mod apps;
pub mod calculator;
pub mod files;
pub mod settings;

pub use apps::{AppEntry, AppSearchProvider};
pub use calculator::{CalcError, CalculatorProvider, evaluate};
pub use files::{FileEntry, FileIndex, FileSearchProvider};
pub use settings::{SettingEntry, SettingsSearchProvider};
