//! Tests for the liquide-accounts crate.

use crate::error::AccountError;
use crate::groups::Group;
use crate::login_history::{LoginEntry, LoginHistory, LoginMethod};
use crate::manager::UserManager;
use crate::password::{PasswordPolicy, PasswordStrength};
use crate::platform::PlatformBackend;
use crate::platform::stub::StubBackend;
use crate::types::{AccountType, UserAccount};

// ── AccountType tests ──────────────────────────────────────────────

#[test]
fn account_type_display() {
    assert_eq!(AccountType::Standard.to_string(), "Standard");
    assert_eq!(AccountType::Administrator.to_string(), "Administrator");
}

#[test]
fn account_type_default_is_standard() {
    assert_eq!(AccountType::default(), AccountType::Standard);
}

#[test]
fn account_type_equality() {
    assert_eq!(AccountType::Standard, AccountType::Standard);
    assert_eq!(AccountType::Administrator, AccountType::Administrator);
    assert_ne!(AccountType::Standard, AccountType::Administrator);
}

// ── UserAccount tests ──────────────────────────────────────────────

#[test]
fn user_account_is_admin() {
    let user = UserAccount {
        uid: 1000,
        username: "alice".into(),
        display_name: "Alice".into(),
        home_dir: "/home/alice".into(),
        shell: "/bin/bash".into(),
        account_type: AccountType::Administrator,
        avatar: None,
        is_logged_in: false,
        is_locked: false,
        password_last_changed: None,
        auto_login: false,
    };
    assert!(user.is_admin());
}

#[test]
fn user_account_not_admin() {
    let user = UserAccount {
        uid: 1001,
        username: "bob".into(),
        display_name: "Bob".into(),
        home_dir: "/home/bob".into(),
        shell: "/bin/bash".into(),
        account_type: AccountType::Standard,
        avatar: None,
        is_logged_in: false,
        is_locked: false,
        password_last_changed: None,
        auto_login: false,
    };
    assert!(!user.is_admin());
}

#[test]
fn user_account_display() {
    let user = UserAccount {
        uid: 1000,
        username: "alice".into(),
        display_name: "Alice".into(),
        home_dir: "/home/alice".into(),
        shell: "/bin/bash".into(),
        account_type: AccountType::Standard,
        avatar: None,
        is_logged_in: false,
        is_locked: false,
        password_last_changed: None,
        auto_login: false,
    };
    let s = format!("{user}");
    assert!(s.contains("alice"));
    assert!(s.contains("1000"));
    assert!(s.contains("Standard"));
}

// ── AccountError tests ─────────────────────────────────────────────

#[test]
fn account_error_display() {
    assert_eq!(AccountError::NotFound.to_string(), "account not found");
    assert_eq!(
        AccountError::PermissionDenied.to_string(),
        "permission denied"
    );
    assert_eq!(
        AccountError::AlreadyExists.to_string(),
        "account already exists"
    );
    assert_eq!(
        AccountError::WeakPassword("too short".into()).to_string(),
        "weak password: too short"
    );
    assert_eq!(
        AccountError::PlatformError("oops".into()).to_string(),
        "platform error: oops"
    );
    assert_eq!(
        AccountError::InvalidUsername("bad".into()).to_string(),
        "invalid username: bad"
    );
}

// ── PasswordPolicy tests ──────────────────────────────────────────

#[test]
fn default_policy_rejects_short_password() {
    let policy = PasswordPolicy::default();
    let result = policy.check("Ab1");
    assert!(result.is_err());
    let violations = result.unwrap_err();
    assert!(violations.iter().any(|v| v.contains("at least 8")));
}

#[test]
fn default_policy_rejects_no_uppercase() {
    let policy = PasswordPolicy::default();
    let result = policy.check("abcdefg1");
    assert!(result.is_err());
    let violations = result.unwrap_err();
    assert!(violations.iter().any(|v| v.contains("uppercase")));
}

#[test]
fn default_policy_rejects_no_lowercase() {
    let policy = PasswordPolicy::default();
    let result = policy.check("ABCDEFG1");
    assert!(result.is_err());
    let violations = result.unwrap_err();
    assert!(violations.iter().any(|v| v.contains("lowercase")));
}

#[test]
fn default_policy_rejects_no_digit() {
    let policy = PasswordPolicy::default();
    let result = policy.check("Abcdefgh");
    assert!(result.is_err());
    let violations = result.unwrap_err();
    assert!(violations.iter().any(|v| v.contains("digit")));
}

#[test]
fn default_policy_accepts_valid_password() {
    let policy = PasswordPolicy::default();
    assert!(policy.check("Abcdefg1").is_ok());
}

#[test]
fn policy_require_special() {
    let mut policy = PasswordPolicy::default();
    policy.require_special = true;
    let result = policy.check("Abcdefg1");
    assert!(result.is_err());
    let violations = result.unwrap_err();
    assert!(violations.iter().any(|v| v.contains("special")));

    assert!(policy.check("Abcdefg1!").is_ok());
}

#[test]
fn policy_custom_min_length() {
    let mut policy = PasswordPolicy::default();
    policy.min_length = 12;
    let result = policy.check("Abcdefg1");
    assert!(result.is_err());
    assert!(policy.check("Abcdefghijk1").is_ok());
}

#[test]
fn password_strength_empty_is_weak() {
    let policy = PasswordPolicy::default();
    assert_eq!(policy.strength(""), PasswordStrength::Weak);
}

#[test]
fn password_strength_short_is_weak() {
    let policy = PasswordPolicy::default();
    assert_eq!(policy.strength("abc"), PasswordStrength::Weak);
}

#[test]
fn password_strength_decent_is_fair_or_better() {
    let policy = PasswordPolicy::default();
    let s = policy.strength("Abcdef1!");
    assert!(s >= PasswordStrength::Fair);
}

#[test]
fn password_strength_long_diverse_is_strong_or_better() {
    let policy = PasswordPolicy::default();
    let s = policy.strength("C0mpl3x!P@ssw0rd#2024xyz");
    assert!(s >= PasswordStrength::Strong);
}

#[test]
fn password_strength_ordering() {
    assert!(PasswordStrength::Weak < PasswordStrength::Fair);
    assert!(PasswordStrength::Fair < PasswordStrength::Good);
    assert!(PasswordStrength::Good < PasswordStrength::Strong);
    assert!(PasswordStrength::Strong < PasswordStrength::VeryStrong);
}

#[test]
fn password_strength_display() {
    assert_eq!(PasswordStrength::Weak.to_string(), "Weak");
    assert_eq!(PasswordStrength::Fair.to_string(), "Fair");
    assert_eq!(PasswordStrength::Good.to_string(), "Good");
    assert_eq!(PasswordStrength::Strong.to_string(), "Strong");
    assert_eq!(PasswordStrength::VeryStrong.to_string(), "Very Strong");
}

// ── LoginHistory tests ────────────────────────────────────────────

#[test]
fn login_history_empty() {
    let h = LoginHistory::new();
    assert!(h.is_empty());
    assert_eq!(h.len(), 0);
    assert!(h.recent_logins(1000, 10).is_empty());
}

#[test]
fn login_history_record_and_retrieve() {
    let mut h = LoginHistory::new();
    h.record(LoginEntry {
        uid: 1000,
        timestamp: 100,
        success: true,
        method: LoginMethod::Password,
        ip: None,
    });
    h.record(LoginEntry {
        uid: 1000,
        timestamp: 200,
        success: false,
        method: LoginMethod::Password,
        ip: Some("10.0.0.1".into()),
    });
    h.record(LoginEntry {
        uid: 1001,
        timestamp: 150,
        success: true,
        method: LoginMethod::RemoteDesktop,
        ip: Some("10.0.0.2".into()),
    });

    assert_eq!(h.len(), 3);

    let user_1000 = h.recent_logins(1000, 10);
    assert_eq!(user_1000.len(), 2);
    // Should be newest first.
    assert_eq!(user_1000[0].timestamp, 200);
    assert_eq!(user_1000[1].timestamp, 100);

    let user_1001 = h.recent_logins(1001, 10);
    assert_eq!(user_1001.len(), 1);
}

#[test]
fn login_history_count_limit() {
    let mut h = LoginHistory::new();
    for i in 0..10 {
        h.record(LoginEntry {
            uid: 1000,
            timestamp: i,
            success: true,
            method: LoginMethod::AutoLogin,
            ip: None,
        });
    }
    let recent = h.recent_logins(1000, 3);
    assert_eq!(recent.len(), 3);
}

#[test]
fn login_history_failed_attempts_since() {
    let mut h = LoginHistory::new();
    h.record(LoginEntry {
        uid: 1000,
        timestamp: 100,
        success: false,
        method: LoginMethod::Password,
        ip: None,
    });
    h.record(LoginEntry {
        uid: 1000,
        timestamp: 200,
        success: true,
        method: LoginMethod::Password,
        ip: None,
    });
    h.record(LoginEntry {
        uid: 1000,
        timestamp: 300,
        success: false,
        method: LoginMethod::Password,
        ip: None,
    });
    assert_eq!(h.failed_attempts_since(1000, 150), 1);
    assert_eq!(h.failed_attempts_since(1000, 0), 2);
    assert_eq!(h.failed_attempts_since(1000, 500), 0);
}

#[test]
fn login_method_display() {
    assert_eq!(LoginMethod::Password.to_string(), "Password");
    assert_eq!(LoginMethod::Fingerprint.to_string(), "Fingerprint");
    assert_eq!(LoginMethod::SmartCard.to_string(), "Smart Card");
    assert_eq!(LoginMethod::AutoLogin.to_string(), "Auto Login");
    assert_eq!(LoginMethod::RemoteDesktop.to_string(), "Remote Desktop");
}

// ── Group tests ───────────────────────────────────────────────────

#[test]
fn group_contains() {
    let g = Group {
        gid: 100,
        name: "users".into(),
        members: vec![1000, 1001, 1002],
    };
    assert!(g.contains(1000));
    assert!(g.contains(1002));
    assert!(!g.contains(9999));
}

#[test]
fn group_member_count() {
    let g = Group {
        gid: 100,
        name: "staff".into(),
        members: vec![1000, 1001],
    };
    assert_eq!(g.member_count(), 2);
}

#[test]
fn group_display() {
    let g = Group {
        gid: 27,
        name: "sudo".into(),
        members: vec![1000],
    };
    let s = format!("{g}");
    assert!(s.contains("sudo"));
    assert!(s.contains("27"));
    assert!(s.contains("1 members"));
}

// ── StubBackend tests ─────────────────────────────────────────────

#[test]
fn stub_current_user() {
    let backend = StubBackend::new();
    let user = backend.current_user().unwrap();
    assert_eq!(user.uid, 1000);
    assert_eq!(user.username, "user");
    assert!(user.is_logged_in);
}

#[test]
fn stub_list_users() {
    let backend = StubBackend::new();
    let users = backend.list_users().unwrap();
    assert_eq!(users.len(), 1);
}

#[test]
fn stub_create_and_delete_user() {
    let mut backend = StubBackend::new();
    let user = backend
        .create_user("alice", "Alice Smith", AccountType::Standard, "P@ssw0rd")
        .unwrap();
    assert_eq!(user.username, "alice");
    assert_eq!(user.display_name, "Alice Smith");
    assert_eq!(user.account_type, AccountType::Standard);
    assert!(!user.is_logged_in);

    let users = backend.list_users().unwrap();
    assert_eq!(users.len(), 2);

    // Delete.
    backend.delete_user(user.uid, false).unwrap();
    let users = backend.list_users().unwrap();
    assert_eq!(users.len(), 1);
}

#[test]
fn stub_create_duplicate_fails() {
    let mut backend = StubBackend::new();
    let _ = backend
        .create_user("alice", "Alice", AccountType::Standard, "Pass1234")
        .unwrap();
    let result = backend.create_user("alice", "Alice 2", AccountType::Standard, "Pass1234");
    assert!(matches!(result, Err(AccountError::AlreadyExists)));
}

#[test]
fn stub_invalid_username() {
    let mut backend = StubBackend::new();
    let result = backend.create_user("", "Empty", AccountType::Standard, "Pass1234");
    assert!(matches!(result, Err(AccountError::InvalidUsername(_))));

    let result = backend.create_user("bad user!", "Bad", AccountType::Standard, "Pass1234");
    assert!(matches!(result, Err(AccountError::InvalidUsername(_))));
}

#[test]
fn stub_lock_unlock() {
    let mut backend = StubBackend::new();
    backend.lock_account(1000).unwrap();
    let user = backend.current_user().unwrap();
    assert!(user.is_locked);

    backend.unlock_account(1000).unwrap();
    let user = backend.current_user().unwrap();
    assert!(!user.is_locked);
}

#[test]
fn stub_set_display_name() {
    let mut backend = StubBackend::new();
    backend.set_display_name(1000, "New Name").unwrap();
    let user = backend.current_user().unwrap();
    assert_eq!(user.display_name, "New Name");
}

#[test]
fn stub_set_avatar() {
    let mut backend = StubBackend::new();
    backend.set_avatar(1000, "/tmp/avatar.png").unwrap();
    let user = backend.current_user().unwrap();
    assert_eq!(user.avatar, Some("/tmp/avatar.png".to_string()));
}

#[test]
fn stub_set_account_type() {
    let mut backend = StubBackend::new();
    backend
        .set_account_type(1000, AccountType::Standard)
        .unwrap();
    let user = backend.current_user().unwrap();
    assert_eq!(user.account_type, AccountType::Standard);

    backend
        .set_account_type(1000, AccountType::Administrator)
        .unwrap();
    let user = backend.current_user().unwrap();
    assert_eq!(user.account_type, AccountType::Administrator);
}

#[test]
fn stub_auto_login() {
    let mut backend = StubBackend::new();
    backend.set_auto_login(1000, true).unwrap();
    let user = backend.current_user().unwrap();
    assert!(user.auto_login);

    backend.set_auto_login(1000, false).unwrap();
    let user = backend.current_user().unwrap();
    assert!(!user.auto_login);
}

#[test]
fn stub_groups() {
    let backend = StubBackend::new();
    let groups = backend.list_groups().unwrap();
    assert!(groups.len() >= 2); // users + sudo

    let user_groups = backend.user_groups(1000).unwrap();
    assert!(user_groups.iter().any(|g| g.name == "users"));
    assert!(user_groups.iter().any(|g| g.name == "sudo"));
}

#[test]
fn stub_add_remove_group() {
    let mut backend = StubBackend::new();
    // Create a second user.
    let alice = backend
        .create_user("alice", "Alice", AccountType::Standard, "Pass1234")
        .unwrap();

    // Alice should be in her own group and the "users" group (not in sudo).
    let groups = backend.user_groups(alice.uid).unwrap();
    assert!(!groups.iter().any(|g| g.name == "sudo"));

    // Add alice to sudo (gid=27).
    backend.add_to_group(alice.uid, 27).unwrap();
    let groups = backend.user_groups(alice.uid).unwrap();
    assert!(groups.iter().any(|g| g.name == "sudo"));

    // Remove alice from sudo.
    backend.remove_from_group(alice.uid, 27).unwrap();
    let groups = backend.user_groups(alice.uid).unwrap();
    assert!(!groups.iter().any(|g| g.name == "sudo"));
}

#[test]
fn stub_not_found() {
    let mut backend = StubBackend::new();
    assert!(matches!(
        backend.delete_user(9999, false),
        Err(AccountError::NotFound)
    ));
    assert!(matches!(
        backend.lock_account(9999),
        Err(AccountError::NotFound)
    ));
    assert!(matches!(
        backend.set_display_name(9999, "x"),
        Err(AccountError::NotFound)
    ));
}

// ── UserManager integration tests ─────────────────────────────────

#[test]
fn manager_create_user_enforces_policy() {
    let mut mgr = UserManager::new(Box::new(StubBackend::new()));

    // Password too weak (no uppercase, no digit).
    let result = mgr.create_user("alice", "Alice", AccountType::Standard, "abcdefgh");
    assert!(matches!(result, Err(AccountError::WeakPassword(_))));

    // Good password.
    let user = mgr
        .create_user("alice", "Alice", AccountType::Standard, "G00dPass")
        .unwrap();
    assert_eq!(user.username, "alice");
}

#[test]
fn manager_change_password_enforces_policy() {
    let mut mgr = UserManager::new(Box::new(StubBackend::new()));
    // Weak new password.
    let result = mgr.change_password(1000, "old", "weak");
    assert!(matches!(result, Err(AccountError::WeakPassword(_))));

    // Strong new password.
    assert!(mgr.change_password(1000, "old", "Str0ngPw").is_ok());
}

#[test]
fn manager_validate_username() {
    assert!(UserManager::validate_username("alice").is_ok());
    assert!(UserManager::validate_username("_test").is_ok());
    assert!(UserManager::validate_username("user-1.dev").is_ok());

    assert!(UserManager::validate_username("").is_err());
    assert!(UserManager::validate_username("A_UPPER").is_err());
    assert!(UserManager::validate_username("has space").is_err());
    assert!(UserManager::validate_username("abcdefghijklmnopqrstuvwxyz1234567890").is_err());
}

#[test]
fn manager_login_history_fallback() {
    let mut mgr = UserManager::new(Box::new(StubBackend::new()));
    // No platform logins, so should fall back to in-memory.
    mgr.login_history_mut().record(LoginEntry {
        uid: 1000,
        timestamp: 500,
        success: true,
        method: LoginMethod::Password,
        ip: None,
    });
    let logins = mgr.recent_logins(1000, 10);
    assert_eq!(logins.len(), 1);
    assert_eq!(logins[0].timestamp, 500);
}

#[test]
fn manager_custom_policy() {
    let mut mgr = UserManager::new(Box::new(StubBackend::new()));
    let mut policy = PasswordPolicy::default();
    policy.min_length = 4;
    policy.require_uppercase = false;
    policy.require_lowercase = false;
    policy.require_digit = false;
    mgr.set_password_policy(policy);

    // "test" should now pass.
    let user = mgr
        .create_user("bob", "Bob", AccountType::Standard, "test")
        .unwrap();
    assert_eq!(user.username, "bob");
}

#[test]
fn manager_delegates_lock_unlock() {
    let mut mgr = UserManager::new(Box::new(StubBackend::new()));
    mgr.lock_account(1000).unwrap();
    let user = mgr.current_user().unwrap();
    assert!(user.is_locked);

    mgr.unlock_account(1000).unwrap();
    let user = mgr.current_user().unwrap();
    assert!(!user.is_locked);
}
