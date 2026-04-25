/// Widget position preference in statusbar
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetPosition {
    Left,
    Center,
    Right,
    SystemTray,
}

/// Widget content to render
#[derive(Debug, Clone)]
pub struct WidgetContent {
    pub icon: Option<String>,
    pub text: Option<String>,
    pub tooltip: Option<String>,
    pub badge: Option<String>,
    pub css_class: Option<String>,
    pub width_hint: Option<u32>,
}

/// Statusbar widget trait
pub trait StatusBarWidgetProvider: Send + Sync {
    fn widget_id(&self) -> &str;
    fn position(&self) -> WidgetPosition;
    fn priority(&self) -> i32 {
        0
    } // higher = more left/prominent
    fn content(&self) -> WidgetContent;
    fn on_click(&mut self) -> Option<WidgetAction>;
    fn on_scroll(&mut self, delta: f32) -> Option<WidgetAction>;
    fn tick(&mut self) -> bool; // return true if content changed
}

#[derive(Debug, Clone)]
pub enum WidgetAction {
    ShowPopup {
        html: String,
        width: u32,
        height: u32,
    },
    TogglePanel(String),
    RunCommand(String),
    OpenUrl(String),
    Custom(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widget_content_empty() {
        let content = WidgetContent {
            icon: None,
            text: None,
            tooltip: None,
            badge: None,
            css_class: None,
            width_hint: None,
        };
        assert!(content.icon.is_none());
        assert!(content.text.is_none());
        assert!(content.tooltip.is_none());
        assert!(content.badge.is_none());
        assert!(content.css_class.is_none());
        assert!(content.width_hint.is_none());
    }

    #[test]
    fn widget_content_full() {
        let content = WidgetContent {
            icon: Some("clock".into()),
            text: Some("12:34".into()),
            tooltip: Some("Current time".into()),
            badge: Some("3".into()),
            css_class: Some("clock-widget".into()),
            width_hint: Some(80),
        };
        assert_eq!(content.icon.as_deref(), Some("clock"));
        assert_eq!(content.text.as_deref(), Some("12:34"));
        assert_eq!(content.tooltip.as_deref(), Some("Current time"));
        assert_eq!(content.badge.as_deref(), Some("3"));
        assert_eq!(content.css_class.as_deref(), Some("clock-widget"));
        assert_eq!(content.width_hint, Some(80));
    }

    #[test]
    fn widget_position_equality() {
        assert_eq!(WidgetPosition::Left, WidgetPosition::Left);
        assert_ne!(WidgetPosition::Left, WidgetPosition::Right);
        assert_ne!(WidgetPosition::Center, WidgetPosition::SystemTray);
    }

    #[test]
    fn widget_action_variants() {
        let popup = WidgetAction::ShowPopup {
            html: "<div>hello</div>".into(),
            width: 200,
            height: 100,
        };
        assert!(matches!(popup, WidgetAction::ShowPopup { .. }));

        let toggle = WidgetAction::TogglePanel("settings".into());
        assert!(matches!(toggle, WidgetAction::TogglePanel(_)));

        let cmd = WidgetAction::RunCommand("notify-send hi".into());
        assert!(matches!(cmd, WidgetAction::RunCommand(_)));

        let url = WidgetAction::OpenUrl("https://example.com".into());
        assert!(matches!(url, WidgetAction::OpenUrl(_)));

        let custom = WidgetAction::Custom("special".into());
        assert!(matches!(custom, WidgetAction::Custom(_)));
    }
}
