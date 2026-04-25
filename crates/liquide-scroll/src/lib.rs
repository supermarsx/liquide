//! Comprehensive scrolling system for the LiquiDE desktop environment.
//!
//! Provides smooth scrolling, touch/trackpad momentum, overscroll rubber-banding,
//! scroll snap points, scrollbar management with auto-hide, and a unified
//! [`ScrollManager`](manager::ScrollManager) that coordinates all scroll containers.

pub mod manager;
pub mod momentum;
pub mod overscroll;
pub mod scrollbar;
pub mod smooth;
pub mod snap;
pub mod state;
pub mod wheel;

#[cfg(test)]
mod tests;
