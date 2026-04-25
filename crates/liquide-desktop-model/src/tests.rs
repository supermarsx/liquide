//! Tests for the desktop model crate.

use crate::atom_table::AtomTable;
use crate::clipboard::{ClipboardData, formats};
use crate::desktop::{
    DESKTOP_DEFAULT, DESKTOP_DISCONNECT, DESKTOP_SCREENSAVER, DESKTOP_WINLOGON, Desktop,
};
use crate::error::DesktopError;
use crate::heap::DesktopHeap;
use crate::manager::DesktopManager;
use crate::security::{DesktopAccess, DesktopFlags, DesktopSecurity, WindowStationFlags};
use crate::station::{SYSTEM_CLASSES, WindowStation};
use crate::types::{DesktopId, WindowId, WindowStationId};

// =======================================================================
// AtomTable tests
// =======================================================================

#[test]
fn atom_add_and_find() {
    let mut table = AtomTable::new();
    let atom = table.add("hello");
    assert_eq!(table.find("hello"), Some(atom));
    assert_eq!(table.get_name(atom), Some("hello"));
}

#[test]
fn atom_refcount_increment() {
    let mut table = AtomTable::new();
    let a1 = table.add("test");
    let a2 = table.add("test");
    assert_eq!(a1, a2);
    assert_eq!(table.refcount(a1), Some(2));
}

#[test]
fn atom_delete_refcount() {
    let mut table = AtomTable::new();
    let atom = table.add("temp");
    table.add("temp"); // refcount = 2
    table.delete(atom);
    // Still exists with refcount 1.
    assert_eq!(table.find("temp"), Some(atom));
    assert_eq!(table.refcount(atom), Some(1));
    table.delete(atom);
    // Now gone.
    assert_eq!(table.find("temp"), None);
    assert_eq!(table.get_name(atom), None);
}

#[test]
fn atom_system_classes_immortal() {
    let mut table = AtomTable::with_system_classes(&["Button", "Edit"]);
    let btn = table.find("Button").expect("system class should exist");
    assert_eq!(table.refcount(btn), Some(u32::MAX));
    // Trying to delete a system atom does nothing.
    table.delete(btn);
    assert_eq!(table.find("Button"), Some(btn));
    assert_eq!(table.refcount(btn), Some(u32::MAX));
}

#[test]
fn atom_find_nonexistent() {
    let table = AtomTable::new();
    assert_eq!(table.find("nope"), None);
}

#[test]
fn atom_len_and_empty() {
    let mut table = AtomTable::new();
    assert!(table.is_empty());
    table.add("x");
    assert_eq!(table.len(), 1);
    assert!(!table.is_empty());
}

#[test]
fn atom_multiple_strings() {
    let mut table = AtomTable::new();
    let a = table.add("alpha");
    let b = table.add("beta");
    let c = table.add("gamma");
    assert_ne!(a, b);
    assert_ne!(b, c);
    assert_eq!(table.len(), 3);
    assert_eq!(table.get_name(a), Some("alpha"));
    assert_eq!(table.get_name(b), Some("beta"));
    assert_eq!(table.get_name(c), Some("gamma"));
}

// =======================================================================
// ClipboardData tests
// =======================================================================

#[test]
fn clipboard_open_close_cycle() {
    let mut cb = ClipboardData::new();
    let w = WindowId(1);
    cb.open(w).unwrap();
    assert_eq!(cb.opened_by(), Some(w));
    cb.close().unwrap();
    assert_eq!(cb.opened_by(), None);
    assert_eq!(cb.owner(), Some(w));
}

#[test]
fn clipboard_set_get_data() {
    let mut cb = ClipboardData::new();
    let w = WindowId(1);
    cb.open(w).unwrap();
    cb.set_data(formats::TEXT, b"hello".to_vec()).unwrap();
    assert_eq!(cb.get_data(formats::TEXT), Some(b"hello".as_slice()));
    assert!(cb.has_format(formats::TEXT));
    assert!(!cb.has_format(formats::BITMAP));
    cb.close().unwrap();
}

#[test]
fn clipboard_empty_clears_data() {
    let mut cb = ClipboardData::new();
    let w = WindowId(1);
    cb.open(w).unwrap();
    cb.set_data(formats::TEXT, b"data".to_vec()).unwrap();
    let seq_before = cb.sequence_number();
    cb.empty().unwrap();
    assert!(cb.get_data(formats::TEXT).is_none());
    assert!(cb.sequence_number() > seq_before);
    cb.close().unwrap();
}

#[test]
fn clipboard_already_open_error() {
    let mut cb = ClipboardData::new();
    cb.open(WindowId(1)).unwrap();
    let err = cb.open(WindowId(2)).unwrap_err();
    assert!(matches!(err, DesktopError::ClipboardAlreadyOpen { .. }));
}

#[test]
fn clipboard_same_window_reopen() {
    let mut cb = ClipboardData::new();
    let w = WindowId(1);
    cb.open(w).unwrap();
    // Opening again by the same window is a no-op.
    cb.open(w).unwrap();
    assert_eq!(cb.opened_by(), Some(w));
}

#[test]
fn clipboard_not_open_error() {
    let mut cb = ClipboardData::new();
    let err = cb.set_data(formats::TEXT, vec![]).unwrap_err();
    assert!(matches!(err, DesktopError::ClipboardNotOpen));
    let err = cb.close().unwrap_err();
    assert!(matches!(err, DesktopError::ClipboardNotOpen));
}

#[test]
fn clipboard_viewer_chain() {
    let mut cb = ClipboardData::new();
    cb.add_viewer(WindowId(10));
    cb.add_viewer(WindowId(20));
    cb.add_viewer(WindowId(10)); // duplicate ignored
    assert_eq!(cb.viewer_chain().len(), 2);
    cb.remove_viewer(WindowId(10));
    assert_eq!(cb.viewer_chain(), &[WindowId(20)]);
}

#[test]
fn clipboard_remove_window() {
    let mut cb = ClipboardData::new();
    let w = WindowId(5);
    cb.open(w).unwrap();
    cb.close().unwrap();
    cb.add_viewer(w);
    assert_eq!(cb.owner(), Some(w));
    cb.remove_window(w);
    assert_eq!(cb.owner(), None);
    assert!(cb.viewer_chain().is_empty());
}

#[test]
fn clipboard_sequence_number_increments() {
    let mut cb = ClipboardData::new();
    let w = WindowId(1);
    cb.open(w).unwrap();
    let s0 = cb.sequence_number();
    cb.set_data(formats::TEXT, b"a".to_vec()).unwrap();
    let s1 = cb.sequence_number();
    cb.set_data(formats::UNICODE_TEXT, b"b".to_vec()).unwrap();
    let s2 = cb.sequence_number();
    assert!(s1 > s0);
    assert!(s2 > s1);
    cb.close().unwrap();
}

#[test]
fn clipboard_available_formats() {
    let mut cb = ClipboardData::new();
    let w = WindowId(1);
    cb.open(w).unwrap();
    cb.set_data(formats::TEXT, b"t".to_vec()).unwrap();
    cb.set_data(formats::HTML, b"h".to_vec()).unwrap();
    let mut fmts = cb.available_formats();
    fmts.sort();
    assert_eq!(
        fmts,
        vec![formats::TEXT, formats::HTML]
            .into_iter()
            .min()
            .map(|_| {
                let mut v = vec![formats::TEXT, formats::HTML];
                v.sort();
                v
            })
            .unwrap()
    );
    cb.close().unwrap();
}

// =======================================================================
// DesktopHeap tests
// =======================================================================

#[test]
fn heap_allocate_and_deallocate() {
    let mut heap = DesktopHeap::new(DesktopId(1), 1024);
    assert_eq!(heap.budget(), 1024);
    assert_eq!(heap.used(), 0);
    heap.allocate(100).unwrap();
    assert_eq!(heap.used(), 100);
    assert_eq!(heap.available(), 924);
    heap.deallocate(50);
    assert_eq!(heap.used(), 50);
}

#[test]
fn heap_exhausted_error() {
    let mut heap = DesktopHeap::new(DesktopId(1), 100);
    heap.allocate(80).unwrap();
    let err = heap.allocate(30).unwrap_err();
    assert!(matches!(err, DesktopError::HeapExhausted { .. }));
    // The failed allocation should not have changed used.
    assert_eq!(heap.used(), 80);
}

#[test]
fn heap_peak_tracking() {
    let mut heap = DesktopHeap::new(DesktopId(1), 1000);
    heap.allocate(500).unwrap();
    heap.deallocate(300);
    heap.allocate(100).unwrap();
    // Peak should be 500 (from first allocation).
    assert_eq!(heap.peak(), 500);
    assert_eq!(heap.used(), 300);
}

#[test]
fn heap_alloc_count() {
    let mut heap = DesktopHeap::new(DesktopId(1), 1000);
    heap.allocate(10).unwrap();
    heap.allocate(20).unwrap();
    heap.allocate(30).unwrap();
    assert_eq!(heap.alloc_count(), 3);
}

#[test]
fn heap_utilization() {
    let mut heap = DesktopHeap::new(DesktopId(1), 1000);
    heap.allocate(500).unwrap();
    let util = heap.utilization();
    assert!((util - 0.5).abs() < f64::EPSILON);
}

#[test]
fn heap_deallocate_saturates_at_zero() {
    let mut heap = DesktopHeap::new(DesktopId(1), 100);
    heap.allocate(10).unwrap();
    heap.deallocate(100); // more than used
    assert_eq!(heap.used(), 0);
}

// =======================================================================
// DesktopSecurity tests
// =======================================================================

#[test]
fn security_home_desktop_full_access() {
    let mut sec = DesktopSecurity::new();
    let d = DesktopId(1);
    sec.assign_thread(100, d);
    assert!(sec.check_access(d, 100, DesktopAccess::ALL));
}

#[test]
fn security_foreign_desktop_denied() {
    let mut sec = DesktopSecurity::new();
    sec.assign_thread(100, DesktopId(1));
    assert!(!sec.check_access(DesktopId(2), 100, DesktopAccess::READ_OBJECTS));
}

#[test]
fn security_grant_extra_access() {
    let mut sec = DesktopSecurity::new();
    let d1 = DesktopId(1);
    let d2 = DesktopId(2);
    sec.assign_thread(100, d1);
    sec.grant_access(
        100,
        d2,
        DesktopAccess::READ_OBJECTS | DesktopAccess::ENUMERATE,
    );
    assert!(sec.check_access(d2, 100, DesktopAccess::READ_OBJECTS));
    assert!(sec.check_access(d2, 100, DesktopAccess::ENUMERATE));
    assert!(!sec.check_access(d2, 100, DesktopAccess::WRITE_OBJECTS));
}

#[test]
fn security_revoke_access() {
    let mut sec = DesktopSecurity::new();
    let d1 = DesktopId(1);
    let d2 = DesktopId(2);
    sec.assign_thread(100, d1);
    sec.grant_access(
        100,
        d2,
        DesktopAccess::READ_OBJECTS | DesktopAccess::WRITE_OBJECTS,
    );
    sec.revoke_access(100, d2, DesktopAccess::WRITE_OBJECTS);
    assert!(sec.check_access(d2, 100, DesktopAccess::READ_OBJECTS));
    assert!(!sec.check_access(d2, 100, DesktopAccess::WRITE_OBJECTS));
}

#[test]
fn security_unknown_thread_denied() {
    let sec = DesktopSecurity::new();
    assert!(!sec.check_access(DesktopId(1), 999, DesktopAccess::READ_OBJECTS));
}

#[test]
fn security_threads_on_desktop() {
    let mut sec = DesktopSecurity::new();
    let d = DesktopId(5);
    sec.assign_thread(1, d);
    sec.assign_thread(2, d);
    sec.assign_thread(3, DesktopId(6));
    let mut threads = sec.threads_on_desktop(d);
    threads.sort();
    assert_eq!(threads, vec![1, 2]);
}

// =======================================================================
// WindowStation tests
// =======================================================================

#[test]
fn station_interactive_flags() {
    let s = WindowStation::new(WindowStationId(1), "WinSta0".into(), 0);
    assert!(s.is_interactive());
    assert!(s.flags.contains(WindowStationFlags::CLIPBOARD_ACCESS));
}

#[test]
fn station_non_interactive_flags() {
    let s = WindowStation::new_non_interactive(WindowStationId(2), "Service-0x0-1234$".into(), 1);
    assert!(!s.is_interactive());
    assert!(!s.flags.contains(WindowStationFlags::CLIPBOARD_ACCESS));
}

#[test]
fn station_has_system_atoms() {
    let s = WindowStation::new(WindowStationId(1), "WinSta0".into(), 0);
    for &class in SYSTEM_CLASSES {
        assert!(
            s.atom_table.find(class).is_some(),
            "system class '{}' should be pre-registered",
            class
        );
    }
}

// =======================================================================
// Desktop tests
// =======================================================================

#[test]
fn desktop_window_management() {
    let mut d = Desktop::new_interactive(
        DesktopId(1),
        "Default".into(),
        WindowStationId(1),
        WindowId(100),
    );
    assert_eq!(d.window_count(), 1); // root
    d.add_window(WindowId(200));
    d.add_window(WindowId(300));
    assert_eq!(d.window_count(), 3);
    assert!(d.set_foreground(WindowId(200)));
    assert_eq!(d.foreground_window, Some(WindowId(200)));
    d.remove_window(WindowId(200));
    assert_eq!(d.foreground_window, None); // cleared
    assert_eq!(d.window_count(), 2);
}

#[test]
fn desktop_set_foreground_unknown_window() {
    let mut d = Desktop::new_interactive(
        DesktopId(1),
        "Default".into(),
        WindowStationId(1),
        WindowId(100),
    );
    assert!(!d.set_foreground(WindowId(999)));
    assert_eq!(d.foreground_window, None);
}

#[test]
fn desktop_flags() {
    let d = Desktop::new_interactive(
        DesktopId(1),
        "Default".into(),
        WindowStationId(1),
        WindowId(100),
    );
    assert!(d.allows_input());
    assert!(!d.is_active());
    assert!(!d.is_secure());
    assert!(!d.is_locked());
}

// =======================================================================
// DesktopManager tests
// =======================================================================

#[test]
fn manager_create_station_and_desktop() {
    let mut mgr = DesktopManager::new();
    let sid = mgr.create_station("WinSta0", 0).unwrap();
    let did = mgr.create_desktop(sid, "Default").unwrap();
    assert_eq!(mgr.station_count(), 1);
    assert_eq!(mgr.desktop_count(), 1);
    assert_eq!(mgr.active_station(), Some(sid));
    assert_eq!(mgr.active_desktop(), Some(did));
}

#[test]
fn manager_duplicate_station_name() {
    let mut mgr = DesktopManager::new();
    mgr.create_station("WinSta0", 0).unwrap();
    let err = mgr.create_station("WinSta0", 1).unwrap_err();
    assert!(matches!(err, DesktopError::StationNameExists(_)));
}

#[test]
fn manager_duplicate_desktop_name() {
    let mut mgr = DesktopManager::new();
    let sid = mgr.create_station("WinSta0", 0).unwrap();
    mgr.create_desktop(sid, "Default").unwrap();
    let err = mgr.create_desktop(sid, "Default").unwrap_err();
    assert!(matches!(err, DesktopError::DesktopNameExists { .. }));
}

#[test]
fn manager_switch_desktop() {
    let mut mgr = DesktopManager::new();
    let sid = mgr.create_station("WinSta0", 0).unwrap();
    let d1 = mgr.create_desktop(sid, "Default").unwrap();
    let d2 = mgr.create_desktop(sid, "Other").unwrap();
    assert_eq!(mgr.active_desktop(), Some(d1));
    mgr.switch_desktop(d2).unwrap();
    assert_eq!(mgr.active_desktop(), Some(d2));
    // d1 should no longer be active.
    assert!(!mgr.desktop(d1).unwrap().is_active());
    assert!(mgr.desktop(d2).unwrap().is_active());
}

#[test]
fn manager_switch_desktop_wrong_station() {
    let mut mgr = DesktopManager::new();
    let sid1 = mgr.create_station("WinSta0", 0).unwrap();
    let _d1 = mgr.create_desktop(sid1, "Default").unwrap();
    let sid2 = mgr
        .create_non_interactive_station("Service-0x0-1$", 1)
        .unwrap();
    let d2 = mgr.create_desktop(sid2, "Default").unwrap();
    let err = mgr.switch_desktop(d2).unwrap_err();
    assert!(matches!(err, DesktopError::StationMismatch { .. }));
}

#[test]
fn manager_input_lock_blocks_switch() {
    let mut mgr = DesktopManager::new();
    let sid = mgr.create_station("WinSta0", 0).unwrap();
    let d1 = mgr.create_desktop(sid, "Default").unwrap();
    let d2 = mgr.create_secure_desktop(sid, "Winlogon").unwrap();

    mgr.switch_desktop(d2).unwrap();
    mgr.lock_input(d2).unwrap();

    // Can still switch to the locked desktop itself.
    mgr.switch_desktop(d2).unwrap();

    // Cannot switch away.
    let err = mgr.switch_desktop(d1).unwrap_err();
    assert!(matches!(err, DesktopError::InputLocked(_)));

    // Unlock restores normal switching.
    mgr.unlock_input();
    mgr.switch_desktop(d1).unwrap();
    assert_eq!(mgr.active_desktop(), Some(d1));
}

#[test]
fn manager_lock_input_requires_active_secure_desktop() {
    let mut mgr = DesktopManager::new();
    let sid = mgr.create_station("WinSta0", 0).unwrap();
    let default = mgr.create_desktop(sid, "Default").unwrap();
    let winlogon = mgr.create_secure_desktop(sid, "Winlogon").unwrap();

    let err = mgr.lock_input(default).unwrap_err();
    assert!(matches!(
        err,
        DesktopError::InputLockRequiresActiveSecureDesktop {
            desktop,
            active_desktop: Some(active_desktop),
        } if desktop == default && active_desktop == default
    ));

    let err = mgr.lock_input(winlogon).unwrap_err();
    assert!(matches!(
        err,
        DesktopError::InputLockRequiresActiveSecureDesktop {
            desktop,
            active_desktop: Some(active_desktop),
        } if desktop == winlogon && active_desktop == default
    ));

    mgr.switch_desktop(winlogon).unwrap();
    mgr.lock_input(winlogon).unwrap();
    assert_eq!(mgr.input_locked_desktop(), Some(winlogon));
}

#[test]
fn manager_secure_desktop_pattern() {
    let mut mgr = DesktopManager::new();
    let sid = mgr.create_station("WinSta0", 0).unwrap();
    let [default, winlogon, _ss, _dc] = mgr.create_standard_desktops(sid).unwrap();

    // Start on Default.
    assert_eq!(mgr.active_desktop(), Some(default));

    // Switch to secure Winlogon desktop, lock input.
    mgr.switch_desktop(winlogon).unwrap();
    mgr.lock_input(winlogon).unwrap();

    // Verify it's secure and locked.
    let wl = mgr.desktop(winlogon).unwrap();
    assert!(wl.is_secure());
    assert!(wl.is_locked());

    // User authenticates — unlock and switch back.
    mgr.unlock_input();
    mgr.switch_desktop(default).unwrap();
    assert_eq!(mgr.active_desktop(), Some(default));
}

#[test]
fn manager_standard_desktops() {
    let mut mgr = DesktopManager::new();
    let sid = mgr.create_station("WinSta0", 0).unwrap();
    let [d, w, s, dc] = mgr.create_standard_desktops(sid).unwrap();
    assert_eq!(mgr.desktop(d).unwrap().name, DESKTOP_DEFAULT);
    assert_eq!(mgr.desktop(w).unwrap().name, DESKTOP_WINLOGON);
    assert_eq!(mgr.desktop(s).unwrap().name, DESKTOP_SCREENSAVER);
    assert_eq!(mgr.desktop(dc).unwrap().name, DESKTOP_DISCONNECT);
    // Winlogon should be secure.
    assert!(mgr.desktop(w).unwrap().is_secure());
}

#[test]
fn manager_close_desktop() {
    let mut mgr = DesktopManager::new();
    let sid = mgr.create_station("WinSta0", 0).unwrap();
    let d1 = mgr.create_desktop(sid, "A").unwrap();
    let d2 = mgr.create_desktop(sid, "B").unwrap();
    mgr.assign_thread(1, d1).unwrap();

    mgr.close_desktop(d1).unwrap();
    assert_eq!(mgr.desktop_count(), 1);
    // Thread should be unassigned.
    assert_eq!(mgr.desktop_for_thread(1), None);
    // Active desktop should move to d2.
    assert_eq!(mgr.active_desktop(), Some(d2));
}

#[test]
fn manager_close_station() {
    let mut mgr = DesktopManager::new();
    let sid = mgr.create_station("WinSta0", 0).unwrap();
    mgr.create_desktop(sid, "Default").unwrap();
    mgr.create_desktop(sid, "Other").unwrap();

    mgr.close_station(sid).unwrap();
    assert_eq!(mgr.station_count(), 0);
    assert_eq!(mgr.desktop_count(), 0);
    assert_eq!(mgr.active_station(), None);
    assert_eq!(mgr.active_desktop(), None);
}

#[test]
fn manager_thread_assignment() {
    let mut mgr = DesktopManager::new();
    let sid = mgr.create_station("WinSta0", 0).unwrap();
    let did = mgr.create_desktop(sid, "Default").unwrap();

    mgr.assign_thread(42, did).unwrap();
    assert_eq!(mgr.desktop_for_thread(42), Some(did));
    assert!(mgr.check_access(did, 42, DesktopAccess::ALL));

    let threads = mgr.threads_on_desktop(did);
    assert_eq!(threads, vec![42]);

    mgr.unassign_thread(42);
    assert_eq!(mgr.desktop_for_thread(42), None);
}

#[test]
fn manager_thread_cross_desktop_denied() {
    let mut mgr = DesktopManager::new();
    let sid = mgr.create_station("WinSta0", 0).unwrap();
    let d1 = mgr.create_desktop(sid, "A").unwrap();
    let d2 = mgr.create_desktop(sid, "B").unwrap();

    mgr.assign_thread(1, d1).unwrap();
    // Thread 1 should not have access to d2.
    assert!(!mgr.check_access(d2, 1, DesktopAccess::READ_OBJECTS));
}

#[test]
fn manager_thread_cross_desktop_granted() {
    let mut mgr = DesktopManager::new();
    let sid = mgr.create_station("WinSta0", 0).unwrap();
    let d1 = mgr.create_desktop(sid, "A").unwrap();
    let d2 = mgr.create_desktop(sid, "B").unwrap();

    mgr.assign_thread(1, d1).unwrap();
    mgr.security_mut()
        .grant_access(1, d2, DesktopAccess::READ_OBJECTS);
    assert!(mgr.check_access(d2, 1, DesktopAccess::READ_OBJECTS));
    assert!(!mgr.check_access(d2, 1, DesktopAccess::WRITE_OBJECTS));
}

#[test]
fn manager_enum_stations_and_desktops() {
    let mut mgr = DesktopManager::new();
    let s1 = mgr.create_station("WinSta0", 0).unwrap();
    let s2 = mgr
        .create_non_interactive_station("Service-0x0-1$", 1)
        .unwrap();
    mgr.create_desktop(s1, "Default").unwrap();
    mgr.create_desktop(s1, "Winlogon").unwrap();
    mgr.create_desktop(s2, "Default").unwrap();

    let mut stations = mgr.enum_stations();
    stations.sort_by_key(|s| s.0);
    assert_eq!(stations.len(), 2);

    let desktops_s1 = mgr.enum_desktops(s1).unwrap();
    assert_eq!(desktops_s1.len(), 2);

    let desktops_s2 = mgr.enum_desktops(s2).unwrap();
    assert_eq!(desktops_s2.len(), 1);
}

#[test]
fn manager_find_by_name() {
    let mut mgr = DesktopManager::new();
    let sid = mgr.create_station("WinSta0", 0).unwrap();
    let did = mgr.create_desktop(sid, "Default").unwrap();

    assert_eq!(mgr.find_station_by_name("WinSta0"), Some(sid));
    assert_eq!(mgr.find_station_by_name("nope"), None);
    assert_eq!(mgr.find_desktop_by_name(sid, "Default"), Some(did));
    assert_eq!(mgr.find_desktop_by_name(sid, "nope"), None);
}

#[test]
fn manager_close_desktop_with_input_lock() {
    let mut mgr = DesktopManager::new();
    let sid = mgr.create_station("WinSta0", 0).unwrap();
    let d1 = mgr.create_desktop(sid, "A").unwrap();
    let d2 = mgr.create_secure_desktop(sid, "B").unwrap();

    mgr.switch_desktop(d2).unwrap();
    mgr.lock_input(d2).unwrap();
    // Closing the locked desktop should clear the input lock.
    mgr.close_desktop(d2).unwrap();
    assert_eq!(mgr.input_locked_desktop(), None);
    // Should be able to switch freely now.
    assert_eq!(mgr.active_desktop(), Some(d1));
}

#[test]
fn manager_desktop_not_found_error() {
    let mut mgr = DesktopManager::new();
    let err = mgr.switch_desktop(DesktopId(999)).unwrap_err();
    assert!(matches!(err, DesktopError::DesktopNotFound(_)));
}

#[test]
fn manager_station_not_found_error() {
    let mut mgr = DesktopManager::new();
    let err = mgr.close_station(WindowStationId(999)).unwrap_err();
    assert!(matches!(err, DesktopError::StationNotFound(_)));
}

#[test]
fn manager_assign_thread_to_nonexistent_desktop() {
    let mut mgr = DesktopManager::new();
    let err = mgr.assign_thread(1, DesktopId(999)).unwrap_err();
    assert!(matches!(err, DesktopError::DesktopNotFound(_)));
}

#[test]
fn manager_create_desktop_on_nonexistent_station() {
    let mut mgr = DesktopManager::new();
    let err = mgr.create_desktop(WindowStationId(999), "Foo").unwrap_err();
    assert!(matches!(err, DesktopError::StationNotFound(_)));
}

#[test]
fn manager_lock_input_nonexistent_desktop() {
    let mut mgr = DesktopManager::new();
    let err = mgr.lock_input(DesktopId(999)).unwrap_err();
    assert!(matches!(err, DesktopError::DesktopNotFound(_)));
}

// =======================================================================
// WindowStationFlags / DesktopFlags bitflag tests
// =======================================================================

#[test]
fn station_flags_combine() {
    let flags = WindowStationFlags::VISIBLE | WindowStationFlags::CLIPBOARD_ACCESS;
    assert!(flags.contains(WindowStationFlags::VISIBLE));
    assert!(flags.contains(WindowStationFlags::CLIPBOARD_ACCESS));
    assert!(!flags.contains(WindowStationFlags::CREATE_DESKTOP));
}

#[test]
fn desktop_flags_combine() {
    let mut flags = DesktopFlags::ACTIVE | DesktopFlags::ALLOW_INPUT;
    flags |= DesktopFlags::SECURE;
    assert!(flags.contains(DesktopFlags::SECURE));
    flags.remove(DesktopFlags::ACTIVE);
    assert!(!flags.contains(DesktopFlags::ACTIVE));
}

#[test]
fn desktop_access_all_contains_everything() {
    let all = DesktopAccess::ALL;
    assert!(all.contains(DesktopAccess::READ_OBJECTS));
    assert!(all.contains(DesktopAccess::WRITE_OBJECTS));
    assert!(all.contains(DesktopAccess::CREATE_WINDOW));
    assert!(all.contains(DesktopAccess::SWITCH_DESKTOP));
    assert!(all.contains(DesktopAccess::ENUMERATE));
    assert!(all.contains(DesktopAccess::CREATE_DESKTOP));
    assert!(all.contains(DesktopAccess::HOOK));
}

// =======================================================================
// Error Display tests
// =======================================================================

#[test]
fn error_display() {
    let err = DesktopError::StationNotFound(WindowStationId(42));
    let msg = format!("{}", err);
    assert!(msg.contains("42"));

    let err = DesktopError::HeapExhausted {
        desktop: DesktopId(1),
        requested: 100,
        available: 50,
    };
    let msg = format!("{}", err);
    assert!(msg.contains("100"));
    assert!(msg.contains("50"));
}

// =======================================================================
// Integration: full session lifecycle
// =======================================================================

#[test]
fn full_session_lifecycle() {
    let mut mgr = DesktopManager::new();

    // Create interactive station for session 0.
    let winsta0 = mgr.create_station("WinSta0", 0).unwrap();

    // Create standard desktops.
    let [default, winlogon, screensaver, _disconnect] =
        mgr.create_standard_desktops(winsta0).unwrap();

    // Verify initial state.
    assert_eq!(mgr.active_desktop(), Some(default));
    assert_eq!(mgr.desktop_count(), 4);

    // Assign some threads.
    mgr.assign_thread(100, default).unwrap();
    mgr.assign_thread(101, default).unwrap();
    mgr.assign_thread(200, winlogon).unwrap();

    // Lock screen flow: switch to winlogon, lock input.
    mgr.switch_desktop(winlogon).unwrap();
    mgr.lock_input(winlogon).unwrap();

    // App threads cannot switch back.
    assert!(mgr.switch_desktop(default).is_err());

    // User authenticates — unlock.
    mgr.unlock_input();
    mgr.switch_desktop(default).unwrap();

    // Screensaver flow.
    mgr.switch_desktop(screensaver).unwrap();
    assert!(mgr.desktop(screensaver).unwrap().is_active());

    // Come back.
    mgr.switch_desktop(default).unwrap();

    // Add a service station.
    let svc = mgr
        .create_non_interactive_station("Service-0x0-1234$", 0)
        .unwrap();
    let svc_desktop = mgr.create_desktop(svc, "Default").unwrap();
    mgr.assign_thread(500, svc_desktop).unwrap();

    // Service thread can access its own desktop but not the interactive one.
    assert!(mgr.check_access(svc_desktop, 500, DesktopAccess::ALL));
    assert!(!mgr.check_access(default, 500, DesktopAccess::READ_OBJECTS));

    // Desktop heap tracking.
    let d = mgr.desktop_mut(default).unwrap();
    d.heap.allocate(4096).unwrap();
    assert_eq!(d.heap.used(), 4096);
    d.heap.deallocate(1024);
    assert_eq!(d.heap.used(), 3072);

    // Station clipboard.
    let station = mgr.station_mut(winsta0).unwrap();
    station.clipboard.open(WindowId(1)).unwrap();
    station
        .clipboard
        .set_data(formats::TEXT, b"Hello from clipboard".to_vec())
        .unwrap();
    station.clipboard.close().unwrap();
    assert_eq!(
        station.clipboard.get_data(formats::TEXT),
        Some(b"Hello from clipboard".as_slice())
    );

    // Station atom table.
    let atom = station.atom_table.add("MyAppClass");
    assert_eq!(station.atom_table.get_name(atom), Some("MyAppClass"));

    // Clean up.
    mgr.close_station(winsta0).unwrap();
    mgr.close_station(svc).unwrap();
    assert_eq!(mgr.station_count(), 0);
    assert_eq!(mgr.desktop_count(), 0);
}
