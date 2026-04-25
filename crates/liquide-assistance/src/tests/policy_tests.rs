use crate::config::{AssistanceConfig, ModeConfig, PermissionsConfig, StealthConfig};
use crate::mode::AssistanceMode;
use crate::observer::{Observer, ObserverRole};
use crate::policy::AssistancePolicy;

fn make_policy() -> AssistancePolicy {
    AssistancePolicy::from_config(
        AssistanceConfig::default(),
        ModeConfig::default(),
        StealthConfig::default(),
        PermissionsConfig::default(),
    )
}

#[test]
fn test_default_modes_allowed() {
    let policy = make_policy();
    assert!(policy.is_mode_allowed(AssistanceMode::ViewOnly));
    assert!(policy.is_mode_allowed(AssistanceMode::Interactive));
    assert!(policy.is_mode_allowed(AssistanceMode::Exclusive));
    // Stealth is disabled by default in both ModeConfig and StealthConfig.
    assert!(!policy.is_mode_allowed(AssistanceMode::Stealth));
}

#[test]
fn test_stealth_requires_config() {
    let mut stealth_cfg = StealthConfig::default();
    stealth_cfg.enabled = true;
    let mut mode_cfg = ModeConfig::default();
    mode_cfg.stealth = true;
    let policy = AssistancePolicy::from_config(
        AssistanceConfig::default(),
        mode_cfg,
        stealth_cfg,
        PermissionsConfig::default(),
    );
    assert!(policy.is_mode_allowed(AssistanceMode::Stealth));
}

#[test]
fn test_stealth_allowed_for_security_admin() {
    let mut stealth_cfg = StealthConfig::default();
    stealth_cfg.enabled = true;
    let policy = AssistancePolicy::from_config(
        AssistanceConfig::default(),
        ModeConfig::default(),
        stealth_cfg,
        PermissionsConfig::default(),
    );
    let sec = Observer::new(
        "o1".into(),
        "Sec".into(),
        ObserverRole::SecurityAdmin,
        AssistanceMode::ViewOnly,
    );
    assert!(policy.is_stealth_allowed(&sec));
    let hd = Observer::new(
        "o2".into(),
        "HD".into(),
        ObserverRole::HelpDesk,
        AssistanceMode::ViewOnly,
    );
    assert!(!policy.is_stealth_allowed(&hd));
}

#[test]
fn test_max_observers() {
    let policy = make_policy();
    assert_eq!(policy.max_observers(AssistanceMode::ViewOnly), 5);
    assert_eq!(policy.max_observers(AssistanceMode::Interactive), 2);
    assert_eq!(policy.max_observers(AssistanceMode::Exclusive), 1);
    assert_eq!(policy.max_observers(AssistanceMode::Stealth), 3);
}

#[test]
fn test_can_request() {
    let policy = make_policy();
    let hd = Observer::new(
        "o1".into(),
        "HD".into(),
        ObserverRole::HelpDesk,
        AssistanceMode::ViewOnly,
    );
    assert!(policy.can_request(&hd));
    let admin = Observer::new(
        "o2".into(),
        "Admin".into(),
        ObserverRole::Admin,
        AssistanceMode::ViewOnly,
    );
    assert!(policy.can_request(&admin));
}

#[test]
fn test_can_invite() {
    let policy = make_policy();
    assert!(policy.can_invite());

    let mut perm = PermissionsConfig::default();
    perm.user_can_invite = false;
    let policy2 = AssistancePolicy::from_config(
        AssistanceConfig::default(),
        ModeConfig::default(),
        StealthConfig::default(),
        perm,
    );
    assert!(!policy2.can_invite());
}
