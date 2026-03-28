//! Comprehensive scrolling system for the LiquiDE desktop environment.
//!
//! Provides smooth scrolling, touch/trackpad momentum, overscroll rubber-banding,
//! scroll snap points, scrollbar management with auto-hide, and a unified
//! [`ScrollManager`](manager::ScrollManager) that coordinates all scroll containers.

pub mod state;
pub mod smooth;
pub mod momentum;
pub mod overscroll;
pub mod snap;
pub mod scrollbar;
pub mod wheel;
pub mod manager;

#[cfg(test)]
mod tests;
