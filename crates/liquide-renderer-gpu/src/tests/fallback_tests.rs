use crate::fallback::*;

#[test]
fn fallback_starts_inactive() {
    let mgr = FallbackManager::new();
    assert!(!mgr.is_active());
    assert!(mgr.reason().is_none());
    assert!(mgr.since().is_none());
}

#[test]
fn activate_sets_reason() {
    let mut mgr = FallbackManager::new();
    mgr.activate(FallbackReason::DeviceLost);

    assert!(mgr.is_active());
    assert!(mgr.reason().is_some());
    assert!(mgr.since().is_some());

    match mgr.reason().unwrap() {
        FallbackReason::DeviceLost => {}
        other => panic!("expected DeviceLost, got {:?}", other),
    }
}

#[test]
fn deactivate_clears_state() {
    let mut mgr = FallbackManager::new();
    mgr.activate(FallbackReason::NoGpu);
    assert!(mgr.is_active());

    mgr.deactivate();
    assert!(!mgr.is_active());
    assert!(mgr.reason().is_none());
    assert!(mgr.since().is_none());
}

#[test]
fn deactivate_when_already_inactive_is_noop() {
    let mut mgr = FallbackManager::new();
    mgr.deactivate(); // should not panic
    assert!(!mgr.is_active());
}

#[test]
fn activate_overwrites_previous_reason() {
    let mut mgr = FallbackManager::new();
    mgr.activate(FallbackReason::NoGpu);
    mgr.activate(FallbackReason::OutOfVram);

    assert!(mgr.is_active());
    match mgr.reason().unwrap() {
        FallbackReason::OutOfVram => {}
        other => panic!("expected OutOfVram, got {:?}", other),
    }
}

#[test]
fn fallback_reason_display() {
    assert_eq!(FallbackReason::NoGpu.to_string(), "no GPU available");
    assert_eq!(FallbackReason::DeviceLost.to_string(), "Vulkan device lost");
    assert_eq!(FallbackReason::OutOfVram.to_string(), "VRAM budget exhausted");
    assert_eq!(
        FallbackReason::UnsupportedFormat.to_string(),
        "unsupported pixel format"
    );
    assert_eq!(
        FallbackReason::DriverError("test".to_string()).to_string(),
        "driver error: test"
    );
}

#[test]
fn default_fallback_manager_is_inactive() {
    let mgr = FallbackManager::default();
    assert!(!mgr.is_active());
}
