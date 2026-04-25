use liquide_plugin_abi::host_functions::{FN_GET_CONFIG, FN_LOG, FN_SEND_MESSAGE};
use liquide_plugin_abi::types::PluginResult;
use liquide_plugin_abi::{ABI_VERSION, ExtensionPoint, PluginManifest};

use crate::config::PluginHostConfig;
use crate::host::PluginHost;

fn sample_manifest(id: &str, points: Vec<ExtensionPoint>) -> PluginManifest {
    PluginManifest {
        id: id.into(),
        name: format!("Plugin {id}"),
        version: "1.0.0".into(),
        abi_version: ABI_VERSION,
        extension_points: points,
        requested_memory_bytes: 1024,
    }
}

#[test]
fn host_create_default() {
    let host = PluginHost::with_defaults();
    assert_eq!(host.active_plugin_count(), 0);
    assert_eq!(host.config().max_plugins, 64);
}

#[test]
fn host_load_and_dispatch() {
    let mut host = PluginHost::with_defaults();
    let m = sample_manifest("com.example.foo", vec![ExtensionPoint::InputFilter]);
    let id = host.load_plugin(m).unwrap();
    assert_eq!(host.active_plugin_count(), 1);

    let results = host.dispatch(ExtensionPoint::InputFilter);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].plugin_id, id);
    assert_eq!(results[0].result, PluginResult::Ok);
}

#[test]
fn host_dispatch_empty() {
    let host = PluginHost::with_defaults();
    let results = host.dispatch(ExtensionPoint::PreAuth);
    assert!(results.is_empty());
}

#[test]
fn host_unload_removes_dispatch() {
    let mut host = PluginHost::with_defaults();
    let m = sample_manifest("com.example.bar", vec![ExtensionPoint::PolicyHook]);
    let id = host.load_plugin(m).unwrap();
    host.unload_plugin(id).unwrap();

    let results = host.dispatch(ExtensionPoint::PolicyHook);
    assert!(results.is_empty());
    assert_eq!(host.active_plugin_count(), 0);
}

#[test]
fn host_suspend_skips_dispatch() {
    let mut host = PluginHost::with_defaults();
    let m = sample_manifest("com.example.sus", vec![ExtensionPoint::ShellWidget]);
    let id = host.load_plugin(m).unwrap();
    host.suspend_plugin(id).unwrap();

    let results = host.dispatch(ExtensionPoint::ShellWidget);
    assert!(results.is_empty());
}

#[test]
fn host_resume_restores_dispatch() {
    let mut host = PluginHost::with_defaults();
    let m = sample_manifest("com.example.res", vec![ExtensionPoint::ShellWidget]);
    let id = host.load_plugin(m).unwrap();
    host.suspend_plugin(id).unwrap();
    host.resume_plugin(id).unwrap();

    let results = host.dispatch(ExtensionPoint::ShellWidget);
    assert_eq!(results.len(), 1);
}

#[test]
fn host_find_by_manifest_id() {
    let mut host = PluginHost::with_defaults();
    let m = sample_manifest("com.example.find-me", vec![]);
    host.load_plugin(m).unwrap();

    assert!(host.find_by_manifest_id("com.example.find-me").is_some());
    assert!(host.find_by_manifest_id("com.example.not-here").is_none());
}

#[test]
fn host_allocate_and_free_resource() {
    let mut host = PluginHost::with_defaults();
    let m = sample_manifest("com.example.alloc", vec![]);
    let id = host.load_plugin(m).unwrap();

    let handle = host.allocate_resource(id, 4096).unwrap();
    assert!(host.resources().get(handle).is_some());

    host.free_resource(handle).unwrap();
    assert!(host.resources().get(handle).is_none());
}

#[test]
fn host_invoke_known_host_function() {
    let host = PluginHost::with_defaults();
    assert_eq!(host.invoke_host_function(FN_LOG), PluginResult::Ok);
    assert_eq!(host.invoke_host_function(FN_GET_CONFIG), PluginResult::Ok);
    assert_eq!(host.invoke_host_function(FN_SEND_MESSAGE), PluginResult::Ok);
}

#[test]
fn host_invoke_unknown_host_function() {
    let host = PluginHost::with_defaults();
    assert_eq!(host.invoke_host_function(9999), PluginResult::Error);
}

#[test]
fn host_host_functions_list() {
    let host = PluginHost::with_defaults();
    let funcs = host.host_functions();
    assert!(funcs.len() >= 5);
    assert_eq!(funcs[0].name, "log");
}

#[test]
fn host_display() {
    let host = PluginHost::with_defaults();
    let s = format!("{host}");
    assert!(s.contains("PluginHost"));
    assert!(s.contains("0 active plugins"));
}

#[test]
fn host_extension_point_not_allowed() {
    let config = PluginHostConfig {
        allowed_extension_points: Some(vec![ExtensionPoint::InputFilter]),
        ..PluginHostConfig::default()
    };
    let mut host = PluginHost::new(config);
    let m = sample_manifest("com.example.denied", vec![ExtensionPoint::PolicyHook]);
    let result = host.load_plugin(m);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("not allowed"));
}
