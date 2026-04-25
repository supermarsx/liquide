#![doc = "Shared utilities for the Liquide project."]
#![doc = ""]
#![doc = "Provides common error types, configuration file parsing, and structured"]
#![doc = "logging initialization used across all Liquide crates."]

pub mod config;
pub mod error;
pub mod geometry;
pub mod logging;
pub mod node_id_bases;
pub mod sync;

pub use error::{LiquideError, Result};
pub use geometry::{FRect, IRect, Rect, RectScalar};
