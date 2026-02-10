use liquide_plugin_abi::ExtensionPoint;
use liquide_plugin_abi::types::PluginResult;

use crate::config::PluginHostConfig;
use crate::dispatcher::Dispatcher;
use crate::plugin::PluginId;

#[test]
fn dispatcher_create_empty() {
    let d = Dispatcher::new();
    assert_eq!(d.total_registrations(), 0);
    assert_eq!(d.active_extension_point_count(), 0);
}

#[test]
fn dispatcher_default_is_empty() {
    let d = Dispatcher::default();
    assert_eq!(d.total_registrations(), 0);
}

#[test]
fn dispatcher_register() {
    let mut d = Dispatcher::new();
    d.register(ExtensionPoint::InputFilter, PluginId(1), None)
        .unwrap();
    assert_eq!(d.total_registrations(), 1);
    assert_eq!(d.active_extension_point_count(), 1);
    assert!(d.has_handlers(ExtensionPoint::InputFilter));
    assert!(!d.has_handlers(ExtensionPoint::PreAuth));
}

#[test]
fn dispatcher_register_duplicate_ignored() {
    let mut d = Dispatcher::new();
    d.register(ExtensionPoint::InputFilter, PluginId(1), None)
        .unwrap();
    d.register(ExtensionPoint::InputFilter, PluginId(1), None)
        .unwrap();
    assert_eq!(d.total_registrations(), 1);
}

#[test]
fn dispatcher_register_multiple_plugins() {
    let mut d = Dispatcher::new();
    d.register(ExtensionPoint::InputFilter, PluginId(1), None)
        .unwrap();
    d.register(ExtensionPoint::InputFilter, PluginId(2), None)
        .unwrap();
    d.register(ExtensionPoint::PolicyHook, PluginId(1), None)
        .unwrap();
    assert_eq!(d.total_registrations(), 3);
    assert_eq!(d.active_extension_point_count(), 2);
    assert_eq!(d.plugins_for(ExtensionPoint::InputFilter).len(), 2);
}

#[test]
fn dispatcher_unregister() {
    let mut d = Dispatcher::new();
    d.register(ExtensionPoint::InputFilter, PluginId(1), None)
        .unwrap();
    let removed = d.unregister(ExtensionPoint::InputFilter, PluginId(1));
    assert!(removed);
    assert!(!d.has_handlers(ExtensionPoint::InputFilter));
}

#[test]
fn dispatcher_unregister_not_registered() {
    let mut d = Dispatcher::new();
    let removed = d.unregister(ExtensionPoint::InputFilter, PluginId(1));
    assert!(!removed);
}

#[test]
fn dispatcher_unregister_all() {
    let mut d = Dispatcher::new();
    d.register(ExtensionPoint::InputFilter, PluginId(1), None)
        .unwrap();
    d.register(ExtensionPoint::PolicyHook, PluginId(1), None)
        .unwrap();
    d.register(ExtensionPoint::ShellWidget, PluginId(1), None)
        .unwrap();
    d.register(ExtensionPoint::InputFilter, PluginId(2), None)
        .unwrap();

    let count = d.unregister_all(PluginId(1));
    assert_eq!(count, 3);
    assert_eq!(d.total_registrations(), 1);
    assert_eq!(d.plugins_for(ExtensionPoint::InputFilter), &[PluginId(2)]);
}

#[test]
fn dispatcher_dispatch() {
    let mut d = Dispatcher::new();
    d.register(ExtensionPoint::InputFilter, PluginId(10), None)
        .unwrap();
    d.register(ExtensionPoint::InputFilter, PluginId(20), None)
        .unwrap();

    let results = d.dispatch(ExtensionPoint::InputFilter);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].plugin_id, PluginId(10));
    assert_eq!(results[0].result, PluginResult::Ok);
    assert_eq!(results[1].plugin_id, PluginId(20));
}

#[test]
fn dispatcher_dispatch_empty() {
    let d = Dispatcher::new();
    let results = d.dispatch(ExtensionPoint::PreAuth);
    assert!(results.is_empty());
}

#[test]
fn dispatcher_plugins_for_empty() {
    let d = Dispatcher::new();
    assert!(d.plugins_for(ExtensionPoint::EncoderStage).is_empty());
}

#[test]
fn dispatcher_extension_point_not_allowed() {
    let config = PluginHostConfig {
        allowed_extension_points: Some(vec![ExtensionPoint::InputFilter]),
        ..PluginHostConfig::default()
    };
    let mut d = Dispatcher::new();
    // InputFilter is allowed.
    d.register(ExtensionPoint::InputFilter, PluginId(1), Some(&config))
        .unwrap();
    // PolicyHook is not allowed.
    let result = d.register(ExtensionPoint::PolicyHook, PluginId(1), Some(&config));
    assert!(result.is_err());
}

#[test]
fn dispatcher_display() {
    let mut d = Dispatcher::new();
    d.register(ExtensionPoint::InputFilter, PluginId(1), None)
        .unwrap();
    let s = format!("{d}");
    assert!(s.contains("Dispatcher"));
    assert!(s.contains("1 extension points"));
    assert!(s.contains("1 registrations"));
}

#[test]
fn dispatch_result_display() {
    let r = crate::dispatcher::DispatchResult {
        plugin_id: PluginId(5),
        result: PluginResult::Ok,
    };
    let s = format!("{r}");
    assert!(s.contains("Plugin(5)"));
    assert!(s.contains("Ok"));
}
