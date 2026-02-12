//! CBOR-serializable message structs for all Liquide protocol channels.
//!
//! Each sub-module corresponds to a channel (or group of channels) and contains
//! the Rust struct definitions that map 1:1 to the CDDL schemas in the
//! protocol specification.

pub mod audio;
pub mod clipboard;
pub mod common;
pub mod control;
pub mod cursor;
pub mod emergency;
pub mod input;
pub mod tile;
pub mod video;
