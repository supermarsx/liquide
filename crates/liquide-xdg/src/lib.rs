//! XDG Desktop Standards compliance for LiquiDE.
//!
//! Implements the freedesktop.org specifications:
//! - XDG Base Directory Specification
//! - MIME type detection and associations
//! - Desktop Entry (.desktop) file parsing
//! - Desktop Portals (abstract interface)
//! - XDG Autostart directories
//! - FreeDesktop Trash specification

pub mod autostart;
pub mod base_dirs;
pub mod desktop_entry;
pub mod mime;
pub mod portals;
pub mod trash;
