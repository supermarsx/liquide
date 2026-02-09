#![doc = "Shared utilities for the Liquide project."]
#![doc = ""]
#![doc = "Provides common error types, configuration file parsing, and structured"]
#![doc = "logging initialization used across all Liquide crates."]

pub mod config;
pub mod error;
pub mod logging;

pub use error::{LiquideError, Result};
