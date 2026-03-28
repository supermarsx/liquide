/// Custom theme properties that a plugin can provide
#[derive(Debug, Clone)]
pub struct ThemeProperties {
    pub css_variables: Vec<(String, String)>,
    pub css_rules: Option<String>,
    pub icon_theme: Option<String>,
    pub cursor_theme: Option<String>,
    pub sound_theme: Option<String>,
}

pub trait ThemeExtensionProvider: Send + Sync {
    fn theme_name(&self) -> &str;
    fn properties(&self) -> ThemeProperties;
    fn on_theme_changed(&mut self, active_theme: &str);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_properties_empty() {
        let props = ThemeProperties {
            css_variables: vec![],
            css_rules: None,
            icon_theme: None,
            cursor_theme: None,
            sound_theme: None,
        };
        assert!(props.css_variables.is_empty());
        assert!(props.css_rules.is_none());
    }

    #[test]
    fn theme_properties_with_variables() {
        let props = ThemeProperties {
            css_variables: vec![
                ("--accent-color".into(), "#ff6600".into()),
                ("--border-radius".into(), "8px".into()),
            ],
            css_rules: Some("desktop-background { background: var(--accent-color); }".into()),
            icon_theme: Some("papirus".into()),
            cursor_theme: Some("breeze".into()),
            sound_theme: None,
        };
        assert_eq!(props.css_variables.len(), 2);
        assert_eq!(props.css_variables[0].0, "--accent-color");
        assert!(props.css_rules.is_some());
        assert_eq!(props.icon_theme.as_deref(), Some("papirus"));
        assert_eq!(props.cursor_theme.as_deref(), Some("breeze"));
        assert!(props.sound_theme.is_none());
    }
}
