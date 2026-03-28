/// Credential provider framework.
///
/// Abstracts authentication methods (password, PIN, fingerprint, smart-card,
/// etc.) behind a common trait so the greeter can enumerate available providers
/// and present the appropriate input fields.  Modelled after GDM/SDDM
/// credential provider patterns.

use crate::auth::AuthResult;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Field types and descriptors
// ---------------------------------------------------------------------------

/// The kind of input a credential field expects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldType {
    /// Masked password input.
    Password,
    /// Plain text input.
    Text,
    /// Numeric PIN (4-8 digits).
    Pin,
    /// Fingerprint scan (no text input — the value is opaque).
    Fingerprint,
    /// Smart-card / PKCS#11 token.
    SmartCard,
    /// One-time code (TOTP, SMS, etc.).
    OneTimeCode,
    /// Choose from a fixed set of values.
    Selection(Vec<String>),
}

/// Describes a single input field exposed by a credential provider.
#[derive(Debug, Clone)]
pub struct FieldDescriptor {
    /// Machine-readable identifier (e.g. `"username"`, `"password"`).
    pub id: String,
    /// Human-readable label shown next to the input.
    pub label: String,
    /// Input type — determines the widget the greeter renders.
    pub field_type: FieldType,
    /// Whether the field must be filled before submission.
    pub is_required: bool,
    /// Optional pre-filled default value.
    pub default_value: Option<String>,
}

impl FieldDescriptor {
    /// Create a required field with no default.
    pub fn required(id: &str, label: &str, field_type: FieldType) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            field_type,
            is_required: true,
            default_value: None,
        }
    }

    /// Create an optional field with no default.
    pub fn optional(id: &str, label: &str, field_type: FieldType) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            field_type,
            is_required: false,
            default_value: None,
        }
    }

    /// Builder: set the default value.
    pub fn with_default(mut self, value: &str) -> Self {
        self.default_value = Some(value.to_string());
        self
    }
}

/// A filled-in credential field submitted by the user.
#[derive(Debug, Clone)]
pub struct CredentialField {
    /// Matches `FieldDescriptor::id`.
    pub descriptor_id: String,
    /// The value entered by the user.
    pub value: String,
}

impl CredentialField {
    pub fn new(descriptor_id: &str, value: &str) -> Self {
        Self {
            descriptor_id: descriptor_id.to_string(),
            value: value.to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// CredentialProvider trait
// ---------------------------------------------------------------------------

/// A pluggable authentication method.
///
/// Each provider exposes the fields it needs (via `field_descriptors`) and
/// performs authentication when the user submits those fields.
pub trait CredentialProvider: Send {
    /// Unique machine-readable identifier (e.g. `"password"`, `"pin"`).
    fn id(&self) -> &str;

    /// Human-readable name shown to the user (e.g. `"Password"`, `"PIN"`).
    fn name(&self) -> &str;

    /// Optional icon name or path (e.g. `"dialog-password"`, `"fingerprint"`).
    fn icon(&self) -> Option<&str> {
        None
    }

    /// The set of input fields this provider requires.
    fn field_descriptors(&self) -> Vec<FieldDescriptor>;

    /// Attempt authentication with the supplied field values.
    fn authenticate(&self, fields: &[CredentialField]) -> AuthResult;
}

// ---------------------------------------------------------------------------
// ProviderRegistry
// ---------------------------------------------------------------------------

/// Registry of available credential providers.
///
/// The greeter queries this to show provider-selection buttons and to
/// dispatch authentication attempts.
pub struct ProviderRegistry {
    providers: Vec<Box<dyn CredentialProvider>>,
}

impl ProviderRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Register a credential provider.
    pub fn register(&mut self, provider: Box<dyn CredentialProvider>) {
        self.providers.push(provider);
    }

    /// Remove a provider by its id. Returns `true` if found and removed.
    pub fn unregister(&mut self, id: &str) -> bool {
        let before = self.providers.len();
        self.providers.retain(|p| p.id() != id);
        self.providers.len() < before
    }

    /// List all registered providers.
    pub fn list(&self) -> Vec<&dyn CredentialProvider> {
        self.providers.iter().map(|p| p.as_ref()).collect()
    }

    /// Look up a provider by id.
    pub fn get(&self, id: &str) -> Option<&dyn CredentialProvider> {
        self.providers.iter().find(|p| p.id() == id).map(|p| p.as_ref())
    }

    /// Number of registered providers.
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}

// ---------------------------------------------------------------------------
// PasswordProvider — built-in username + password
// ---------------------------------------------------------------------------

/// Built-in password credential provider.
///
/// Stores username/password pairs (FNV-1a hashed, **not** cryptographically
/// secure — for testing and development only).  Real deployments should
/// delegate to `PamBackend`.
pub struct PasswordProvider {
    entries: HashMap<String, u64>,
}

impl PasswordProvider {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Register a user with a plaintext password (hashed on storage).
    pub fn add_user(&mut self, username: &str, password: &str) {
        self.entries.insert(username.to_string(), Self::hash(password));
    }

    fn hash(password: &str) -> u64 {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in password.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }

    fn find_field<'a>(fields: &'a [CredentialField], id: &str) -> Option<&'a str> {
        fields.iter().find(|f| f.descriptor_id == id).map(|f| f.value.as_str())
    }
}

impl CredentialProvider for PasswordProvider {
    fn id(&self) -> &str {
        "password"
    }

    fn name(&self) -> &str {
        "Password"
    }

    fn icon(&self) -> Option<&str> {
        Some("dialog-password")
    }

    fn field_descriptors(&self) -> Vec<FieldDescriptor> {
        vec![
            FieldDescriptor::required("username", "Username", FieldType::Text),
            FieldDescriptor::required("password", "Password", FieldType::Password),
        ]
    }

    fn authenticate(&self, fields: &[CredentialField]) -> AuthResult {
        let username = match Self::find_field(fields, "username") {
            Some(u) => u,
            None => return AuthResult::Failed("Username is required.".into()),
        };
        let password = match Self::find_field(fields, "password") {
            Some(p) => p,
            None => return AuthResult::Failed("Password is required.".into()),
        };
        match self.entries.get(username) {
            Some(&stored) if stored == Self::hash(password) => AuthResult::Success,
            Some(_) => AuthResult::Failed("Incorrect password.".into()),
            None => AuthResult::Failed("User not found.".into()),
        }
    }
}

// ---------------------------------------------------------------------------
// PinProvider — built-in username + 4-8 digit PIN
// ---------------------------------------------------------------------------

/// Built-in numeric PIN credential provider.
///
/// Accepts a 4-8 digit PIN.  Like `PasswordProvider`, this is intended for
/// testing — production systems should delegate to PAM.
pub struct PinProvider {
    entries: HashMap<String, String>,
}

impl PinProvider {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Register a user with a PIN.  The PIN must be 4-8 digits.
    /// Returns `false` if the PIN is invalid.
    pub fn add_user(&mut self, username: &str, pin: &str) -> bool {
        if !Self::validate_pin(pin) {
            return false;
        }
        self.entries.insert(username.to_string(), pin.to_string());
        true
    }

    /// Validate that a PIN is 4-8 ASCII digits.
    pub fn validate_pin(pin: &str) -> bool {
        let len = pin.len();
        (4..=8).contains(&len) && pin.chars().all(|c| c.is_ascii_digit())
    }

    fn find_field<'a>(fields: &'a [CredentialField], id: &str) -> Option<&'a str> {
        fields.iter().find(|f| f.descriptor_id == id).map(|f| f.value.as_str())
    }
}

impl CredentialProvider for PinProvider {
    fn id(&self) -> &str {
        "pin"
    }

    fn name(&self) -> &str {
        "PIN"
    }

    fn icon(&self) -> Option<&str> {
        Some("input-dialpad")
    }

    fn field_descriptors(&self) -> Vec<FieldDescriptor> {
        vec![
            FieldDescriptor::required("username", "Username", FieldType::Text),
            FieldDescriptor::required("pin", "PIN", FieldType::Pin),
        ]
    }

    fn authenticate(&self, fields: &[CredentialField]) -> AuthResult {
        let username = match Self::find_field(fields, "username") {
            Some(u) => u,
            None => return AuthResult::Failed("Username is required.".into()),
        };
        let pin = match Self::find_field(fields, "pin") {
            Some(p) => p,
            None => return AuthResult::Failed("PIN is required.".into()),
        };
        if !Self::validate_pin(pin) {
            return AuthResult::Failed("PIN must be 4-8 digits.".into());
        }
        match self.entries.get(username) {
            Some(stored) if stored == pin => AuthResult::Success,
            Some(_) => AuthResult::Failed("Incorrect PIN.".into()),
            None => AuthResult::Failed("User not found.".into()),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- FieldDescriptor tests --

    #[test]
    fn field_descriptor_required() {
        let fd = FieldDescriptor::required("user", "Username", FieldType::Text);
        assert_eq!(fd.id, "user");
        assert_eq!(fd.label, "Username");
        assert!(fd.is_required);
        assert!(fd.default_value.is_none());
    }

    #[test]
    fn field_descriptor_optional_with_default() {
        let fd = FieldDescriptor::optional("domain", "Domain", FieldType::Text)
            .with_default("WORKGROUP");
        assert!(!fd.is_required);
        assert_eq!(fd.default_value.as_deref(), Some("WORKGROUP"));
    }

    #[test]
    fn field_type_selection() {
        let ft = FieldType::Selection(vec!["a".into(), "b".into()]);
        if let FieldType::Selection(opts) = &ft {
            assert_eq!(opts.len(), 2);
        } else {
            panic!("expected Selection");
        }
    }

    #[test]
    fn credential_field_new() {
        let cf = CredentialField::new("password", "s3cret");
        assert_eq!(cf.descriptor_id, "password");
        assert_eq!(cf.value, "s3cret");
    }

    // -- ProviderRegistry tests --

    #[test]
    fn registry_initially_empty() {
        let reg = ProviderRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
        assert!(reg.list().is_empty());
    }

    #[test]
    fn registry_register_and_list() {
        let mut reg = ProviderRegistry::new();
        reg.register(Box::new(PasswordProvider::new()));
        reg.register(Box::new(PinProvider::new()));
        assert_eq!(reg.len(), 2);
        let ids: Vec<&str> = reg.list().iter().map(|p| p.id()).collect();
        assert!(ids.contains(&"password"));
        assert!(ids.contains(&"pin"));
    }

    #[test]
    fn registry_get_by_id() {
        let mut reg = ProviderRegistry::new();
        reg.register(Box::new(PasswordProvider::new()));
        assert!(reg.get("password").is_some());
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn registry_unregister() {
        let mut reg = ProviderRegistry::new();
        reg.register(Box::new(PasswordProvider::new()));
        reg.register(Box::new(PinProvider::new()));
        assert_eq!(reg.len(), 2);
        assert!(reg.unregister("pin"));
        assert_eq!(reg.len(), 1);
        assert!(reg.get("pin").is_none());
    }

    #[test]
    fn registry_unregister_nonexistent() {
        let mut reg = ProviderRegistry::new();
        assert!(!reg.unregister("nothing"));
    }

    // -- PasswordProvider tests --

    #[test]
    fn password_provider_metadata() {
        let prov = PasswordProvider::new();
        assert_eq!(prov.id(), "password");
        assert_eq!(prov.name(), "Password");
        assert_eq!(prov.icon(), Some("dialog-password"));
    }

    #[test]
    fn password_provider_fields() {
        let prov = PasswordProvider::new();
        let fields = prov.field_descriptors();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].id, "username");
        assert_eq!(fields[1].id, "password");
        assert!(fields[0].is_required);
        assert!(fields[1].is_required);
    }

    #[test]
    fn password_provider_success() {
        let mut prov = PasswordProvider::new();
        prov.add_user("alice", "secret");
        let fields = vec![
            CredentialField::new("username", "alice"),
            CredentialField::new("password", "secret"),
        ];
        assert_eq!(prov.authenticate(&fields), AuthResult::Success);
    }

    #[test]
    fn password_provider_wrong_password() {
        let mut prov = PasswordProvider::new();
        prov.add_user("alice", "secret");
        let fields = vec![
            CredentialField::new("username", "alice"),
            CredentialField::new("password", "wrong"),
        ];
        assert!(matches!(prov.authenticate(&fields), AuthResult::Failed(_)));
    }

    #[test]
    fn password_provider_user_not_found() {
        let prov = PasswordProvider::new();
        let fields = vec![
            CredentialField::new("username", "nobody"),
            CredentialField::new("password", "pass"),
        ];
        assert!(matches!(prov.authenticate(&fields), AuthResult::Failed(msg) if msg.contains("not found")));
    }

    #[test]
    fn password_provider_missing_username() {
        let prov = PasswordProvider::new();
        let fields = vec![CredentialField::new("password", "pass")];
        assert!(matches!(prov.authenticate(&fields), AuthResult::Failed(msg) if msg.contains("Username")));
    }

    #[test]
    fn password_provider_missing_password() {
        let mut prov = PasswordProvider::new();
        prov.add_user("alice", "secret");
        let fields = vec![CredentialField::new("username", "alice")];
        assert!(matches!(prov.authenticate(&fields), AuthResult::Failed(msg) if msg.contains("Password")));
    }

    // -- PinProvider tests --

    #[test]
    fn pin_provider_metadata() {
        let prov = PinProvider::new();
        assert_eq!(prov.id(), "pin");
        assert_eq!(prov.name(), "PIN");
        assert_eq!(prov.icon(), Some("input-dialpad"));
    }

    #[test]
    fn pin_provider_fields() {
        let prov = PinProvider::new();
        let fields = prov.field_descriptors();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].id, "username");
        assert_eq!(fields[1].id, "pin");
        assert_eq!(fields[1].field_type, FieldType::Pin);
    }

    #[test]
    fn pin_provider_success() {
        let mut prov = PinProvider::new();
        assert!(prov.add_user("bob", "1234"));
        let fields = vec![
            CredentialField::new("username", "bob"),
            CredentialField::new("pin", "1234"),
        ];
        assert_eq!(prov.authenticate(&fields), AuthResult::Success);
    }

    #[test]
    fn pin_provider_wrong_pin() {
        let mut prov = PinProvider::new();
        prov.add_user("bob", "1234");
        let fields = vec![
            CredentialField::new("username", "bob"),
            CredentialField::new("pin", "9999"),
        ];
        assert!(matches!(prov.authenticate(&fields), AuthResult::Failed(_)));
    }

    #[test]
    fn pin_provider_invalid_pin_format() {
        let mut prov = PinProvider::new();
        prov.add_user("bob", "1234");
        let fields = vec![
            CredentialField::new("username", "bob"),
            CredentialField::new("pin", "abc"),
        ];
        assert!(matches!(prov.authenticate(&fields), AuthResult::Failed(msg) if msg.contains("4-8 digits")));
    }

    #[test]
    fn pin_validate_lengths() {
        assert!(!PinProvider::validate_pin("123"));       // too short
        assert!(PinProvider::validate_pin("1234"));       // 4 ok
        assert!(PinProvider::validate_pin("12345678"));   // 8 ok
        assert!(!PinProvider::validate_pin("123456789")); // 9 too long
        assert!(!PinProvider::validate_pin("12ab"));      // non-digit
        assert!(!PinProvider::validate_pin(""));          // empty
    }

    #[test]
    fn pin_add_user_rejects_invalid() {
        let mut prov = PinProvider::new();
        assert!(!prov.add_user("bob", "ab"));
        // No entry was stored
        let fields = vec![
            CredentialField::new("username", "bob"),
            CredentialField::new("pin", "ab"),
        ];
        assert!(matches!(prov.authenticate(&fields), AuthResult::Failed(_)));
    }

    #[test]
    fn pin_provider_missing_username() {
        let prov = PinProvider::new();
        let fields = vec![CredentialField::new("pin", "1234")];
        assert!(matches!(prov.authenticate(&fields), AuthResult::Failed(msg) if msg.contains("Username")));
    }

    #[test]
    fn pin_provider_missing_pin() {
        let mut prov = PinProvider::new();
        prov.add_user("bob", "1234");
        let fields = vec![CredentialField::new("username", "bob")];
        assert!(matches!(prov.authenticate(&fields), AuthResult::Failed(msg) if msg.contains("PIN")));
    }

    #[test]
    fn pin_provider_user_not_found() {
        let prov = PinProvider::new();
        let fields = vec![
            CredentialField::new("username", "nobody"),
            CredentialField::new("pin", "1234"),
        ];
        assert!(matches!(prov.authenticate(&fields), AuthResult::Failed(msg) if msg.contains("not found")));
    }

    #[test]
    fn pin_provider_eight_digit_pin() {
        let mut prov = PinProvider::new();
        assert!(prov.add_user("carol", "98765432"));
        let fields = vec![
            CredentialField::new("username", "carol"),
            CredentialField::new("pin", "98765432"),
        ];
        assert_eq!(prov.authenticate(&fields), AuthResult::Success);
    }
}
