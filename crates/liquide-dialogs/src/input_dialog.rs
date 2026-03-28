use crate::{Dialog, DialogId, DialogResult};

/// Validator function type — returns Ok(()) on valid input, Err(message) on invalid
pub type ValidatorFn = Box<dyn Fn(&str) -> Result<(), String> + Send + Sync>;

/// Text input dialog state
pub struct InputDialog {
    pub id: DialogId,
    pub title: String,
    pub label: String,
    pub value: String,
    pub placeholder: String,
    pub password_mode: bool,
    pub validator: Option<ValidatorFn>,
    pub validation_error: Option<String>,
    pub max_length: Option<usize>,
}

impl std::fmt::Debug for InputDialog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InputDialog")
            .field("id", &self.id)
            .field("title", &self.title)
            .field("label", &self.label)
            .field("value", &if self.password_mode { "***".to_string() } else { self.value.clone() })
            .field("placeholder", &self.placeholder)
            .field("password_mode", &self.password_mode)
            .field("has_validator", &self.validator.is_some())
            .field("validation_error", &self.validation_error)
            .field("max_length", &self.max_length)
            .finish()
    }
}

impl InputDialog {
    pub fn new(
        id: DialogId,
        title: impl Into<String>,
        label: impl Into<String>,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            label: label.into(),
            value: String::new(),
            placeholder: String::new(),
            password_mode: false,
            validator: None,
            validation_error: None,
            max_length: None,
        }
    }

    /// Create a password input dialog
    pub fn password(
        id: DialogId,
        title: impl Into<String>,
        label: impl Into<String>,
    ) -> Self {
        Self {
            password_mode: true,
            ..Self::new(id, title, label)
        }
    }

    /// Set the initial value
    pub fn with_initial_value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self
    }

    /// Set the placeholder text
    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Set a validation function
    pub fn with_validator(mut self, validator: impl Fn(&str) -> Result<(), String> + Send + Sync + 'static) -> Self {
        self.validator = Some(Box::new(validator));
        self
    }

    /// Set maximum input length
    pub fn with_max_length(mut self, max: usize) -> Self {
        self.max_length = Some(max);
        self
    }

    /// Update the input value
    pub fn set_value(&mut self, value: impl Into<String>) {
        let mut value = value.into();
        if let Some(max) = self.max_length {
            value.truncate(max);
        }
        self.value = value;
        // Clear previous validation error on input change
        self.validation_error = None;
    }

    /// Validate the current value
    pub fn validate(&mut self) -> bool {
        if let Some(ref validator) = self.validator {
            match validator(&self.value) {
                Ok(()) => {
                    self.validation_error = None;
                    true
                }
                Err(msg) => {
                    self.validation_error = Some(msg);
                    false
                }
            }
        } else {
            self.validation_error = None;
            true
        }
    }

    /// Confirm the dialog — validates first, returns Ok if valid
    pub fn confirm(&mut self) -> DialogResult<String> {
        if self.validate() {
            DialogResult::Ok(self.value.clone())
        } else {
            DialogResult::Cancelled
        }
    }

    /// Check if there's a validation error
    pub fn has_error(&self) -> bool {
        self.validation_error.is_some()
    }
}

impl Dialog for InputDialog {
    type Output = String;
    fn id(&self) -> DialogId {
        self.id
    }
    fn title(&self) -> &str {
        &self.title
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_input() {
        let dlg = InputDialog::new(DialogId(1), "Rename", "New name:");
        assert_eq!(dlg.title, "Rename");
        assert_eq!(dlg.label, "New name:");
        assert!(dlg.value.is_empty());
        assert!(!dlg.password_mode);
    }

    #[test]
    fn test_password_mode() {
        let dlg = InputDialog::password(DialogId(1), "Login", "Password:");
        assert!(dlg.password_mode);
    }

    #[test]
    fn test_initial_value() {
        let dlg = InputDialog::new(DialogId(1), "T", "L")
            .with_initial_value("hello");
        assert_eq!(dlg.value, "hello");
    }

    #[test]
    fn test_placeholder() {
        let dlg = InputDialog::new(DialogId(1), "T", "L")
            .with_placeholder("type here...");
        assert_eq!(dlg.placeholder, "type here...");
    }

    #[test]
    fn test_set_value() {
        let mut dlg = InputDialog::new(DialogId(1), "T", "L");
        dlg.set_value("test");
        assert_eq!(dlg.value, "test");
    }

    #[test]
    fn test_max_length() {
        let mut dlg = InputDialog::new(DialogId(1), "T", "L")
            .with_max_length(5);
        dlg.set_value("hello world");
        assert_eq!(dlg.value, "hello");
    }

    #[test]
    fn test_validate_no_validator() {
        let mut dlg = InputDialog::new(DialogId(1), "T", "L");
        assert!(dlg.validate());
        assert!(!dlg.has_error());
    }

    #[test]
    fn test_validate_passes() {
        let mut dlg = InputDialog::new(DialogId(1), "T", "L")
            .with_validator(|s| {
                if s.is_empty() {
                    Err("Cannot be empty".into())
                } else {
                    Ok(())
                }
            });
        dlg.set_value("hello");
        assert!(dlg.validate());
        assert!(!dlg.has_error());
    }

    #[test]
    fn test_validate_fails() {
        let mut dlg = InputDialog::new(DialogId(1), "T", "L")
            .with_validator(|s| {
                if s.is_empty() {
                    Err("Cannot be empty".into())
                } else {
                    Ok(())
                }
            });
        assert!(!dlg.validate());
        assert!(dlg.has_error());
        assert_eq!(dlg.validation_error.as_deref(), Some("Cannot be empty"));
    }

    #[test]
    fn test_validate_error_clears_on_input() {
        let mut dlg = InputDialog::new(DialogId(1), "T", "L")
            .with_validator(|s| {
                if s.is_empty() {
                    Err("Empty".into())
                } else {
                    Ok(())
                }
            });
        dlg.validate(); // fails
        assert!(dlg.has_error());
        dlg.set_value("x"); // clears error
        assert!(!dlg.has_error());
    }

    #[test]
    fn test_confirm_valid() {
        let mut dlg = InputDialog::new(DialogId(1), "T", "L");
        dlg.set_value("result");
        match dlg.confirm() {
            DialogResult::Ok(val) => assert_eq!(val, "result"),
            _ => panic!("expected Ok"),
        }
    }

    #[test]
    fn test_confirm_invalid() {
        let mut dlg = InputDialog::new(DialogId(1), "T", "L")
            .with_validator(|s| {
                if s.len() < 3 {
                    Err("Too short".into())
                } else {
                    Ok(())
                }
            });
        dlg.set_value("ab");
        assert_eq!(dlg.confirm(), DialogResult::Cancelled);
    }

    #[test]
    fn test_debug_hides_password() {
        let dlg = InputDialog::password(DialogId(1), "Login", "Pass:")
            .with_initial_value("secret123");
        let debug = format!("{:?}", dlg);
        assert!(!debug.contains("secret123"));
        assert!(debug.contains("***"));
    }

    #[test]
    fn test_dialog_trait() {
        let dlg = InputDialog::new(DialogId(99), "Title", "Label");
        assert_eq!(dlg.id(), DialogId(99));
        assert_eq!(dlg.title(), "Title");
        assert!(dlg.is_modal());
    }
}
