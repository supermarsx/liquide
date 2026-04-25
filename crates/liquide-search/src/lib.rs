//! Desktop search framework for LiquiDE.
//!
//! The crate is organised around the [`SearchProvider`](provider::SearchProvider)
//! trait.  Concrete providers live in [`providers`], and the
//! [`SearchEngine`](engine::SearchEngine) merges results from all registered
//! providers.

pub mod engine;
pub mod provider;
pub mod providers;

pub use engine::{SearchEngine, SearchSession};
pub use provider::{
    SearchCategory, SearchProvider, SearchResult, SearchResultAction, clamp_score, fuzzy_score,
};
pub use providers::{
    AppEntry, AppSearchProvider, CalcError, CalculatorProvider, FileEntry, FileIndex,
    FileSearchProvider, SettingEntry, SettingsSearchProvider, evaluate,
};
