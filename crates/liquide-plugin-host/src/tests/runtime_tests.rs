use liquide_plugin_abi::{ABI_VERSION, ExtensionPoint, PluginManifest};

use crate::config::PluginHostConfig;
use crate::plugin::PluginId;
use crate::runtime::PluginRuntime;

fn sample_manifest(id: &str) -> PluginManifest {
    PluginManifest {
        id: id.into(),
        name: format!("Plugin {id}"),
        version: "0.1.0".into(),
        abi_version: ABI_VERSION,
        extension_points: vec![ExtensionPoint::InputFilter],
        requested_memory_bytes: 1024,
    }
}

#[test]
fn runtime_create() {
    let rt = PluginRuntime::new(PluginHostConfig::default());
    assert_eq!(rt.plugin_count(), 0);
    assert_eq!(rt.active_plugin_count(), 0);
}

#[test]
fn runtime_load_plugin() {
    let mut rt = PluginRuntime::new(PluginHostConfig::default());
    let id = rt.load_plugin(sample_manifest("com.rt.load")).unwrap();
    assert_eq!(rt.active_plugin_count(), 1);
    let p = rt.plugin(id).unwrap();
    assert!(p.is_active());
}

#[test]
fn runtime_load_incompatible_abi() {
    let mut rt = PluginRuntime::new(PluginHostConfig::default());
    let mut m = sample_manifest("com.rt.abi");
    m.abi_version = ABI_VERSION + 99;
    let result = rt.load_plugin(m);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("incompatible ABI"));
}

#[test]
fn runtime_load_duplicate() {
    let mut rt = PluginRuntime::new(PluginHostConfig::default());
    rt.load_plugin(sample_manifest("com.rt.dup")).unwrap();
    let result = rt.load_plugin(sample_manifest("com.rt.dup"));
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("duplicate"));
}

#[test]
fn runtime_plugin_limit() {
    let config = PluginHostConfig::new(2, 1024 * 1024);
    let mut rt = PluginRuntime::new(config);
    rt.load_plugin(sample_manifest("com.rt.p1")).unwrap();
    rt.load_plugin(sample_manifest("com.rt.p2")).unwrap();
    let result = rt.load_plugin(sample_manifest("com.rt.p3"));
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("limit reached"));
}

#[test]
fn runtime_memory_cap() {
    let config = PluginHostConfig::new(64, 512); // 512 bytes max per plugin
    let mut rt = PluginRuntime::new(config);
    let mut m = sample_manifest("com.rt.mem");
    m.requested_memory_bytes = 1024; // exceeds 512
    let result = rt.load_plugin(m);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("exhausted"));
}

#[test]
fn runtime_unload() {
    let mut rt = PluginRuntime::new(PluginHostConfig::default());
    let id = rt.load_plugin(sample_manifest("com.rt.unload")).unwrap();
    rt.unload_plugin(id).unwrap();
    assert!(rt.plugin(id).unwrap().is_unloaded());
    assert_eq!(rt.active_plugin_count(), 0);
}

#[test]
fn runtime_unload_already_unloaded() {
    let mut rt = PluginRuntime::new(PluginHostConfig::default());
    let id = rt
        .load_plugin(sample_manifest("com.rt.dbl-unload"))
        .unwrap();
    rt.unload_plugin(id).unwrap();
    let result = rt.unload_plugin(id);
    assert!(result.is_err());
}

#[test]
fn runtime_suspend_and_resume() {
    let mut rt = PluginRuntime::new(PluginHostConfig::default());
    let id = rt.load_plugin(sample_manifest("com.rt.sus")).unwrap();
    rt.suspend_plugin(id).unwrap();
    assert!(rt.plugin(id).unwrap().is_suspended());
    rt.resume_plugin(id).unwrap();
    assert!(rt.plugin(id).unwrap().is_active());
}

#[test]
fn runtime_suspend_not_active() {
    let mut rt = PluginRuntime::new(PluginHostConfig::default());
    let id = rt.load_plugin(sample_manifest("com.rt.sus-bad")).unwrap();
    rt.suspend_plugin(id).unwrap();
    // Suspend again while already suspended
    let result = rt.suspend_plugin(id);
    assert!(result.is_err());
}

#[test]
fn runtime_resume_not_suspended() {
    let mut rt = PluginRuntime::new(PluginHostConfig::default());
    let id = rt.load_plugin(sample_manifest("com.rt.res-bad")).unwrap();
    // Resume while active (not suspended)
    let result = rt.resume_plugin(id);
    assert!(result.is_err());
}

#[test]
fn runtime_fail_plugin() {
    let mut rt = PluginRuntime::new(PluginHostConfig::default());
    let id = rt.load_plugin(sample_manifest("com.rt.fail")).unwrap();
    rt.fail_plugin(id, "test failure").unwrap();
    assert!(rt.plugin(id).unwrap().is_failed());
}

#[test]
fn runtime_allocate_resource() {
    let mut rt = PluginRuntime::new(PluginHostConfig::default());
    let id = rt.load_plugin(sample_manifest("com.rt.alloc")).unwrap();
    let handle = rt.allocate_resource(id, 256).unwrap();
    assert!(rt.resources().get(handle).is_some());
    assert_eq!(rt.resources().total_allocated(), 256);
}

#[test]
fn runtime_allocate_resource_not_active() {
    let mut rt = PluginRuntime::new(PluginHostConfig::default());
    let id = rt.load_plugin(sample_manifest("com.rt.alloc-sus")).unwrap();
    rt.suspend_plugin(id).unwrap();
    let result = rt.allocate_resource(id, 256);
    assert!(result.is_err());
}

#[test]
fn runtime_free_resource() {
    let mut rt = PluginRuntime::new(PluginHostConfig::default());
    let id = rt.load_plugin(sample_manifest("com.rt.free")).unwrap();
    let handle = rt.allocate_resource(id, 128).unwrap();
    rt.free_resource(handle).unwrap();
    assert_eq!(rt.resources().total_allocated(), 0);
}

#[test]
fn runtime_unload_frees_resources() {
    let mut rt = PluginRuntime::new(PluginHostConfig::default());
    let id = rt.load_plugin(sample_manifest("com.rt.auto-free")).unwrap();
    rt.allocate_resource(id, 100).unwrap();
    rt.allocate_resource(id, 200).unwrap();
    assert_eq!(rt.resources().total_allocated(), 300);
    rt.unload_plugin(id).unwrap();
    assert_eq!(rt.resources().total_allocated(), 0);
}

#[test]
fn runtime_find_by_manifest_id() {
    let mut rt = PluginRuntime::new(PluginHostConfig::default());
    rt.load_plugin(sample_manifest("com.rt.find")).unwrap();
    assert!(rt.find_by_manifest_id("com.rt.find").is_some());
    assert!(rt.find_by_manifest_id("com.rt.nope").is_none());
}

#[test]
fn runtime_plugin_not_found() {
    let mut rt = PluginRuntime::new(PluginHostConfig::default());
    assert!(rt.plugin(PluginId(999)).is_none());
    assert!(rt.unload_plugin(PluginId(999)).is_err());
    assert!(rt.suspend_plugin(PluginId(999)).is_err());
    assert!(rt.resume_plugin(PluginId(999)).is_err());
}

#[test]
fn runtime_load_from_json() {
    let mut rt = PluginRuntime::new(PluginHostConfig::default());
    let json = serde_json::to_vec(&sample_manifest("com.rt.json")).unwrap();
    let id = rt.load_plugin_from_json(&json).unwrap();
    assert!(rt.plugin(id).unwrap().is_active());
}

#[test]
fn runtime_load_from_invalid_json() {
    let mut rt = PluginRuntime::new(PluginHostConfig::default());
    let result = rt.load_plugin_from_json(b"not json");
    assert!(result.is_err());
}

#[test]
fn runtime_display() {
    let rt = PluginRuntime::new(PluginHostConfig::default());
    let s = format!("{rt}");
    assert!(s.contains("PluginRuntime"));
    assert!(s.contains("0 active plugins"));
}
