use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::Shell;
use crate::shell::hooks::{HookManager, HookPriority, HookResult, ShellHookEvent};
use liquide_compositor::geometry::Rect;

// ── HookManager unit tests ─────────────────────────────────────────

#[test]
fn register_and_unregister() {
    let mut mgr = HookManager::new();
    assert_eq!(mgr.hook_count(), 0);

    let id1 = mgr.register(HookPriority::NORMAL, Box::new(|_| HookResult::Continue));
    let id2 = mgr.register(HookPriority::NORMAL, Box::new(|_| HookResult::Continue));
    assert_eq!(mgr.hook_count(), 2);

    assert!(mgr.unregister(id1));
    assert_eq!(mgr.hook_count(), 1);

    // Double unregister returns false.
    assert!(!mgr.unregister(id1));
    assert_eq!(mgr.hook_count(), 1);

    assert!(mgr.unregister(id2));
    assert_eq!(mgr.hook_count(), 0);
}

#[test]
fn priority_ordering() {
    let call_order = Arc::new(std::sync::Mutex::new(Vec::<i32>::new()));

    let mut mgr = HookManager::new();

    // Register in reverse order — LOW first, then SYSTEM, then ACCESSIBILITY.
    let co1 = Arc::clone(&call_order);
    mgr.register(
        HookPriority::LOW,
        Box::new(move |_| {
            co1.lock().unwrap().push(200);
            HookResult::Continue
        }),
    );
    let co2 = Arc::clone(&call_order);
    mgr.register(
        HookPriority::SYSTEM,
        Box::new(move |_| {
            co2.lock().unwrap().push(0);
            HookResult::Continue
        }),
    );
    let co3 = Arc::clone(&call_order);
    mgr.register(
        HookPriority::ACCESSIBILITY,
        Box::new(move |_| {
            co3.lock().unwrap().push(-100);
            HookResult::Continue
        }),
    );

    let event = ShellHookEvent::WindowCreated { window_id: 1 };
    mgr.dispatch(&event);

    let order = call_order.lock().unwrap();
    assert_eq!(
        *order,
        vec![-100, 0, 200],
        "Hooks must fire in priority order (low number first)"
    );
}

#[test]
fn dispatch_continue() {
    let counter = Arc::new(AtomicU32::new(0));
    let mut mgr = HookManager::new();

    let c1 = Arc::clone(&counter);
    mgr.register(
        HookPriority::NORMAL,
        Box::new(move |_| {
            c1.fetch_add(1, Ordering::SeqCst);
            HookResult::Continue
        }),
    );
    let c2 = Arc::clone(&counter);
    mgr.register(
        HookPriority::LOW,
        Box::new(move |_| {
            c2.fetch_add(1, Ordering::SeqCst);
            HookResult::Continue
        }),
    );

    let result = mgr.dispatch(&ShellHookEvent::WindowClosed { window_id: 42 });
    assert_eq!(result, HookResult::Continue);
    assert_eq!(counter.load(Ordering::SeqCst), 2, "Both hooks should fire");
}

#[test]
fn dispatch_handled_stops_propagation() {
    let counter = Arc::new(AtomicU32::new(0));
    let mut mgr = HookManager::new();

    let c1 = Arc::clone(&counter);
    mgr.register(
        HookPriority::SYSTEM,
        Box::new(move |_| {
            c1.fetch_add(1, Ordering::SeqCst);
            HookResult::Handled
        }),
    );
    let c2 = Arc::clone(&counter);
    mgr.register(
        HookPriority::NORMAL,
        Box::new(move |_| {
            c2.fetch_add(1, Ordering::SeqCst);
            HookResult::Continue
        }),
    );

    let result = mgr.dispatch(&ShellHookEvent::WindowActivated { window_id: 7 });
    assert_eq!(result, HookResult::Handled);
    assert_eq!(
        counter.load(Ordering::SeqCst),
        1,
        "Second hook should not fire after Handled"
    );
}

#[test]
fn dispatch_modified_propagates() {
    let counter = Arc::new(AtomicU32::new(0));
    let mut mgr = HookManager::new();

    let c1 = Arc::clone(&counter);
    mgr.register(
        HookPriority::SYSTEM,
        Box::new(move |_| {
            c1.fetch_add(1, Ordering::SeqCst);
            HookResult::Modified
        }),
    );
    let c2 = Arc::clone(&counter);
    mgr.register(
        HookPriority::NORMAL,
        Box::new(move |_| {
            c2.fetch_add(1, Ordering::SeqCst);
            HookResult::Continue
        }),
    );

    let result = mgr.dispatch(&ShellHookEvent::LauncherOpened);
    assert_eq!(result, HookResult::Modified, "Modified is sticky");
    assert_eq!(counter.load(Ordering::SeqCst), 2, "Both hooks should fire");
}

#[test]
fn enable_disable() {
    let counter = Arc::new(AtomicU32::new(0));
    let mut mgr = HookManager::new();

    let c = Arc::clone(&counter);
    let id = mgr.register(
        HookPriority::NORMAL,
        Box::new(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
            HookResult::Continue
        }),
    );
    assert_eq!(mgr.active_count(), 1);

    // Disable.
    mgr.set_active(id, false);
    assert_eq!(mgr.active_count(), 0);
    assert_eq!(mgr.hook_count(), 1, "Hook still registered, just inactive");

    mgr.dispatch(&ShellHookEvent::WindowClosed { window_id: 1 });
    assert_eq!(
        counter.load(Ordering::SeqCst),
        0,
        "Disabled hook should not fire"
    );

    // Re-enable.
    mgr.set_active(id, true);
    assert_eq!(mgr.active_count(), 1);

    mgr.dispatch(&ShellHookEvent::WindowClosed { window_id: 1 });
    assert_eq!(
        counter.load(Ordering::SeqCst),
        1,
        "Re-enabled hook should fire"
    );
}

#[test]
fn clear_all() {
    let mut mgr = HookManager::new();
    mgr.register(HookPriority::NORMAL, Box::new(|_| HookResult::Continue));
    mgr.register(HookPriority::LOW, Box::new(|_| HookResult::Continue));
    mgr.register(HookPriority::SYSTEM, Box::new(|_| HookResult::Continue));
    assert_eq!(mgr.hook_count(), 3);

    mgr.clear();
    assert_eq!(mgr.hook_count(), 0);
    assert_eq!(mgr.active_count(), 0);
}

#[test]
fn dispatch_with_no_hooks() {
    let mgr = HookManager::new();
    let result = mgr.dispatch(&ShellHookEvent::ThemeChanged {
        theme_name: "night".to_string(),
    });
    assert_eq!(result, HookResult::Continue);
}

#[test]
fn hook_receives_correct_event_data() {
    let received_id = Arc::new(AtomicU32::new(0));
    let mut mgr = HookManager::new();

    let rid = Arc::clone(&received_id);
    mgr.register(
        HookPriority::NORMAL,
        Box::new(move |event| {
            if let ShellHookEvent::WindowCreated { window_id } = event {
                rid.store(*window_id as u32, Ordering::SeqCst);
            }
            HookResult::Continue
        }),
    );

    mgr.dispatch(&ShellHookEvent::WindowCreated { window_id: 42 });
    assert_eq!(received_id.load(Ordering::SeqCst), 42);
}

#[test]
fn default_trait() {
    let mgr = HookManager::default();
    assert_eq!(mgr.hook_count(), 0);
}

// ── Integration tests with Shell ────────────────────────────────────

#[test]
fn shell_hook_fires_on_window_open_close() {
    let events = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));

    let mut shell = Shell::new(1920.0, 1080.0);

    let ev = Arc::clone(&events);
    shell.hook_manager_mut().register(
        HookPriority::NORMAL,
        Box::new(move |event| {
            match event {
                ShellHookEvent::WindowCreated { window_id } => {
                    ev.lock().unwrap().push(format!("created:{}", window_id));
                }
                ShellHookEvent::WindowClosed { window_id } => {
                    ev.lock().unwrap().push(format!("closed:{}", window_id));
                }
                _ => {}
            }
            HookResult::Continue
        }),
    );

    let wid = shell.open_window("Test", Rect::new(100.0, 100.0, 400.0, 300.0));
    let _ = shell.close_window(wid);

    let log = events.lock().unwrap();
    assert_eq!(log.len(), 2);
    assert!(
        log[0].starts_with("created:"),
        "Expected WindowCreated, got: {}",
        log[0]
    );
    assert!(
        log[1].starts_with("closed:"),
        "Expected WindowClosed, got: {}",
        log[1]
    );
}

#[test]
fn shell_hook_fires_on_focus_change() {
    let events = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));

    let mut shell = Shell::new(1920.0, 1080.0);
    let w1 = shell.open_window("Win1", Rect::new(100.0, 100.0, 400.0, 300.0));
    let w2 = shell.open_window("Win2", Rect::new(200.0, 200.0, 400.0, 300.0));

    // Register hook after window creation to avoid those events.
    let ev = Arc::clone(&events);
    shell.hook_manager_mut().register(
        HookPriority::NORMAL,
        Box::new(move |event| {
            match event {
                ShellHookEvent::WindowActivated { window_id } => {
                    ev.lock().unwrap().push(format!("activated:{}", window_id));
                }
                ShellHookEvent::WindowDeactivated { window_id } => {
                    ev.lock()
                        .unwrap()
                        .push(format!("deactivated:{}", window_id));
                }
                _ => {}
            }
            HookResult::Continue
        }),
    );

    let _ = shell.set_focus(w1);
    let _ = shell.set_focus(w2);

    let log = events.lock().unwrap();
    // set_focus(w1): activated w1 (no previous focus to deactivate)
    // set_focus(w2): deactivated w1, activated w2
    assert!(log.contains(&format!("activated:{}", w1.0)));
    assert!(log.contains(&format!("deactivated:{}", w1.0)));
    assert!(log.contains(&format!("activated:{}", w2.0)));
}

#[test]
fn shell_hook_fires_on_minimize_maximize_restore() {
    let events = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));

    let mut shell = Shell::new(1920.0, 1080.0);
    let wid = shell.open_window("Test", Rect::new(100.0, 100.0, 400.0, 300.0));

    let ev = Arc::clone(&events);
    shell.hook_manager_mut().register(
        HookPriority::NORMAL,
        Box::new(move |event| {
            match event {
                ShellHookEvent::WindowMinimized { window_id } => {
                    ev.lock().unwrap().push(format!("minimized:{}", window_id));
                }
                ShellHookEvent::WindowMaximized { window_id } => {
                    ev.lock().unwrap().push(format!("maximized:{}", window_id));
                }
                ShellHookEvent::WindowRestored { window_id } => {
                    ev.lock().unwrap().push(format!("restored:{}", window_id));
                }
                _ => {}
            }
            HookResult::Continue
        }),
    );

    let _ = shell.maximize(wid);
    let _ = shell.restore(wid);
    let _ = shell.minimize(wid);

    let log = events.lock().unwrap();
    assert_eq!(log[0], format!("maximized:{}", wid.0));
    assert_eq!(log[1], format!("restored:{}", wid.0));
    assert_eq!(log[2], format!("minimized:{}", wid.0));
}

#[test]
fn shell_hook_manager_accessor() {
    let shell = Shell::new(1920.0, 1080.0);
    assert_eq!(shell.hook_manager().hook_count(), 0);
}
