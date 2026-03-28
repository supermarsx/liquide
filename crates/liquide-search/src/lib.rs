//! Desktop search framework for LiquiDE.
//!
//! The crate is organised around the [`SearchProvider`](provider::SearchProvider)
//! trait.  Concrete providers live in [`providers`], and the
//! [`SearchEngine`](engine::SearchEngine) merges results from all registered
//! providers.

pub mod provider;
pub mod engine;
pub mod providers;

pub use engine::{SearchEngine, SearchSession};
pub use provider::{
    SearchCategory, SearchProvider, SearchResult, SearchResultAction,
    fuzzy_score, clamp_score,
};
pub use providers::{
    AppEntry, AppSearchProvider,
    CalcError, CalculatorProvider, evaluate,
    FileEntry, FileIndex, FileSearchProvider,
    SettingEntry, SettingsSearchProvider,
};
