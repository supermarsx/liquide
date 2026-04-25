//! Built-in element rules for standard shell UI components.

use crate::computed::{AlignItems, Display, FlexDirection, JustifyContent, Position};

use super::element_rule::ElementRule;
use super::engine::RuleEngine;
use super::types::Severity;

impl RuleEngine {
    /// Create a rule engine with all builtin rules for shell elements.
    pub fn with_builtin_rules() -> Self {
        let mut engine = Self::new();

        // ── Desktop Background ──
        engine.add_rule(
            ElementRule::new("desktop-background")
                .description("Root background element covering the entire screen")
                .position_one_of(&[Position::Fixed, Position::Absolute], Severity::Critical)
                .width_defined(Severity::Error)
                .height_defined(Severity::Error)
                .background_visible(Severity::Warning),
        );

        // ── Status Bar ──
        engine.add_rule(
            ElementRule::new("statusbar")
                .description("Top status bar with system indicators")
                .display_one_of(&[Display::Flex], Severity::Critical)
                .position_one_of(&[Position::Fixed], Severity::Critical)
                .height_defined(Severity::Error)
                .width_defined(Severity::Error)
                .z_index_defined(Severity::Warning)
                .align_items_one_of(&[AlignItems::Center], Severity::Warning)
                .background_visible(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("statusbar-slot")
                .description("Status bar region (left/center/right)")
                .display_one_of(&[Display::Flex], Severity::Error)
                .align_items_one_of(&[AlignItems::Center], Severity::Warning)
                .parent_one_of(&["statusbar"], Severity::Error),
        );

        engine.add_rule(
            ElementRule::new("statusbar-logo")
                .description("Brand logo in status bar")
                .display_one_of(&[Display::Flex], Severity::Warning)
                .align_items_one_of(&[AlignItems::Center], Severity::Warning)
                .color_visible(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("statusbar-item")
                .description("Individual status bar item")
                .display_one_of(&[Display::Flex, Display::InlineFlex], Severity::Warning)
                .color_visible(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("status-indicator")
                .description("Status indicator (connection, etc.)")
                .color_visible(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("notification-indicator")
                .description("Notification count indicator")
                .color_visible(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("session-button")
                .description("Session/power button in status bar")
                .color_visible(Severity::Warning),
        );

        // ── Dock ──
        engine.add_rule(
            ElementRule::new("dock")
                .description("Application dock at bottom of screen")
                .display_one_of(&[Display::Flex], Severity::Critical)
                .position_one_of(&[Position::Fixed], Severity::Critical)
                .height_defined(Severity::Error)
                .width_defined(Severity::Error)
                .justify_content_one_of(&[JustifyContent::Center], Severity::Warning)
                .align_items_one_of(&[AlignItems::Center], Severity::Warning)
                .gap_positive(Severity::Warning)
                .background_visible(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("dock-item")
                .description("Individual dock icon slot")
                .display_one_of(&[Display::Flex], Severity::Error)
                .width_defined(Severity::Error)
                .height_defined(Severity::Error)
                .border_radius_positive(Severity::Warning)
                .align_items_one_of(&[AlignItems::Center], Severity::Warning)
                .justify_content_one_of(&[JustifyContent::Center], Severity::Warning)
                .parent_one_of(&["dock"], Severity::Error)
                .must_have_attribute("data-app-id", Severity::Warning),
        );

        // ── Context Menu ──
        engine.add_rule(
            ElementRule::new("context-menu")
                .description("Right-click context menu")
                .display_one_of(&[Display::Flex], Severity::Critical)
                .position_one_of(&[Position::Fixed], Severity::Critical)
                .flex_direction_one_of(&[FlexDirection::Column], Severity::Error)
                .z_index_defined(Severity::Error)
                .min_width_defined(Severity::Warning)
                .max_width_defined(Severity::Warning)
                .border_radius_positive(Severity::Warning)
                .background_visible(Severity::Error)
                .blur_radius_positive(Severity::Warning)
                .padding_defined(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("session-menu")
                .description("Session/power menu")
                .display_one_of(&[Display::Flex], Severity::Critical)
                .position_one_of(&[Position::Fixed], Severity::Critical)
                .flex_direction_one_of(&[FlexDirection::Column], Severity::Error)
                .z_index_defined(Severity::Error)
                .background_visible(Severity::Error)
                .blur_radius_positive(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("app-menu")
                .description("Application menu")
                .display_one_of(&[Display::Flex], Severity::Critical)
                .position_one_of(&[Position::Fixed], Severity::Critical)
                .flex_direction_one_of(&[FlexDirection::Column], Severity::Error)
                .z_index_defined(Severity::Error)
                .background_visible(Severity::Error),
        );

        engine.add_rule(
            ElementRule::new("menu-item")
                .description("Individual menu item")
                .display_one_of(&[Display::Flex], Severity::Error)
                .height_defined(Severity::Warning)
                .align_items_one_of(&[AlignItems::Center], Severity::Warning)
                .color_visible(Severity::Warning)
                .padding_defined(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("menu-separator")
                .description("Menu separator line")
                .height_defined(Severity::Error)
                .background_visible(Severity::Warning),
        );

        // ── Launcher ──
        engine.add_rule(
            ElementRule::new("launcher-overlay")
                .description("Launcher overlay backdrop")
                .display_one_of(&[Display::Flex], Severity::Critical)
                .position_one_of(&[Position::Fixed], Severity::Critical)
                .width_defined(Severity::Error)
                .height_defined(Severity::Error)
                .z_index_defined(Severity::Error)
                .align_items_one_of(&[AlignItems::Center], Severity::Warning)
                .justify_content_one_of(&[JustifyContent::Center], Severity::Warning)
                .background_visible(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("launcher")
                .description("Launcher search panel")
                .display_one_of(&[Display::Flex], Severity::Critical)
                .flex_direction_one_of(&[FlexDirection::Column], Severity::Error)
                .width_defined(Severity::Error)
                .border_radius_positive(Severity::Warning)
                .background_visible(Severity::Error)
                .blur_radius_positive(Severity::Warning)
                .padding_defined(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("launcher-search")
                .description("Launcher search input")
                .height_defined(Severity::Error)
                .padding_defined(Severity::Warning)
                .color_visible(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("launcher-results")
                .description("Launcher results container")
                .display_one_of(&[Display::Flex], Severity::Error)
                .flex_direction_one_of(&[FlexDirection::Column], Severity::Error),
        );

        engine.add_rule(
            ElementRule::new("launcher-item")
                .description("Individual launcher result item")
                .display_one_of(&[Display::Flex], Severity::Error)
                .height_defined(Severity::Warning)
                .align_items_one_of(&[AlignItems::Center], Severity::Warning)
                .color_visible(Severity::Warning),
        );

        // ── Notifications ──
        engine.add_rule(
            ElementRule::new("notification-area")
                .description("Notification container area")
                .display_one_of(&[Display::Flex], Severity::Critical)
                .position_one_of(&[Position::Fixed], Severity::Critical)
                .flex_direction_one_of(&[FlexDirection::Column], Severity::Error)
                .z_index_defined(Severity::Error)
                .gap_positive(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("notification")
                .description("Individual notification toast")
                .display_one_of(&[Display::Flex], Severity::Error)
                .flex_direction_one_of(&[FlexDirection::Column], Severity::Error)
                .width_defined(Severity::Warning)
                .border_radius_positive(Severity::Warning)
                .background_visible(Severity::Error)
                .blur_radius_positive(Severity::Warning)
                .padding_defined(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("notification-title")
                .description("Notification title text")
                .color_visible(Severity::Warning)
                .font_size_in_range(10.0, 20.0, Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("notification-body")
                .description("Notification body text")
                .color_visible(Severity::Warning),
        );

        // ── Windows ──
        engine.add_rule(
            ElementRule::new("window")
                .description("Application window frame")
                .position_one_of(&[Position::Absolute, Position::Fixed], Severity::Critical)
                .display_one_of(&[Display::Flex], Severity::Error)
                .flex_direction_one_of(&[FlexDirection::Column], Severity::Error)
                .border_radius_positive(Severity::Warning)
                .background_visible(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("window-titlebar")
                .description("Window title bar")
                .display_one_of(&[Display::Flex], Severity::Error)
                .height_defined(Severity::Error)
                .align_items_one_of(&[AlignItems::Center], Severity::Warning)
                .color_visible(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("window-title")
                .description("Window title text")
                .color_visible(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("window-content")
                .description("Window content area")
                .background_visible(Severity::Warning),
        );

        // ── Title bar buttons ──
        for button in &["close-button", "maximize-button", "minimize-button"] {
            engine.add_rule(
                ElementRule::new(*button)
                    .description("Window decoration button")
                    .display_one_of(&[Display::Flex], Severity::Error)
                    .width_defined(Severity::Error)
                    .height_defined(Severity::Error)
                    .border_radius_positive(Severity::Warning)
                    .align_items_one_of(&[AlignItems::Center], Severity::Warning)
                    .justify_content_one_of(&[JustifyContent::Center], Severity::Warning),
            );
        }

        // ── Loading ──
        engine.add_rule(
            ElementRule::new("loading-overlay")
                .description("Loading screen overlay")
                .display_one_of(&[Display::Flex], Severity::Critical)
                .position_one_of(&[Position::Fixed], Severity::Critical)
                .width_defined(Severity::Error)
                .height_defined(Severity::Error)
                .z_index_defined(Severity::Error)
                .align_items_one_of(&[AlignItems::Center], Severity::Warning)
                .justify_content_one_of(&[JustifyContent::Center], Severity::Warning)
                .background_visible(Severity::Warning),
        );

        engine.add_rule(
            ElementRule::new("loading-panel")
                .description("Loading panel content")
                .display_one_of(&[Display::Flex], Severity::Error)
                .flex_direction_one_of(&[FlexDirection::Column], Severity::Warning)
                .align_items_one_of(&[AlignItems::Center], Severity::Warning)
                .border_radius_positive(Severity::Warning)
                .background_visible(Severity::Warning)
                .color_visible(Severity::Warning),
        );

        engine
    }
}
