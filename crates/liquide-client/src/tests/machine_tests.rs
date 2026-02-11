use crate::machine::MachineManager;

#[test]
fn test_add_and_get_machine() {
    let mut mgr = MachineManager::new();
    let id = mgr.add_machine("Dev Box", "dev.example.com:3389");
    let entry = mgr.get_machine(&id).unwrap();
    assert_eq!(entry.name(), "Dev Box");
    assert_eq!(entry.address(), "dev.example.com:3389");
}

#[test]
fn test_remove_machine() {
    let mut mgr = MachineManager::new();
    let id = mgr.add_machine("Dev Box", "dev.example.com:3389");
    assert!(mgr.remove_machine(&id));
    assert!(mgr.get_machine(&id).is_none());
}

#[test]
fn test_update_status() {
    let mut mgr = MachineManager::new();
    let id = mgr.add_machine("Server", "srv:3389");
    mgr.update_status(&id, Some(true), true);
    let entry = mgr.get_machine(&id).unwrap();
    assert_eq!(entry.is_online(), Some(true));
    assert!(entry.has_active_session());
}

#[test]
fn test_recent_machines_sorted() {
    let mut mgr = MachineManager::new();
    let id1 = mgr.add_machine("Old", "old:3389");
    let id2 = mgr.add_machine("New", "new:3389");

    mgr.record_connection(&id1, 1000);
    mgr.record_connection(&id2, 2000);

    let recent = mgr.recent_machines();
    assert_eq!(recent[0].name(), "New");
    assert_eq!(recent[1].name(), "Old");
}

#[test]
fn test_create_and_delete_group() {
    let mut mgr = MachineManager::new();
    assert!(mgr.create_group("Dev"));
    assert!(!mgr.create_group("Dev")); // duplicate
    assert!(mgr.delete_group("Dev"));
    assert!(!mgr.delete_group("Dev")); // already gone
}

#[test]
fn test_move_to_group() {
    let mut mgr = MachineManager::new();
    let id = mgr.add_machine("Server", "srv:3389");
    mgr.move_to_group(&id, "Production");

    let machines = mgr.machines_in_group("Production");
    assert_eq!(machines.len(), 1);
    assert_eq!(machines[0].name(), "Server");

    let entry = mgr.get_machine(&id).unwrap();
    assert_eq!(entry.group(), Some("Production"));
}

#[test]
fn test_display_name_fallback() {
    use crate::machine::MachineEntry;
    let entry = MachineEntry::new("1".to_string(), String::new(), "addr:3389".to_string());
    assert_eq!(entry.display_name(), "addr:3389");

    let entry2 = MachineEntry::new("2".to_string(), "My PC".to_string(), "addr:3389".to_string());
    assert_eq!(entry2.display_name(), "My PC");
}

#[test]
fn test_all_machines() {
    let mut mgr = MachineManager::new();
    mgr.add_machine("A", "a:3389");
    mgr.add_machine("B", "b:3389");
    assert_eq!(mgr.all_machines().len(), 2);
}

#[test]
fn test_remove_machine_from_group_on_delete() {
    let mut mgr = MachineManager::new();
    let id = mgr.add_machine("Server", "srv:3389");
    mgr.move_to_group(&id, "Staging");
    mgr.remove_machine(&id);
    let machines = mgr.machines_in_group("Staging");
    assert!(machines.is_empty());
}
