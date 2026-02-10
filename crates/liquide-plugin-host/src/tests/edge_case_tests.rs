use liquide_plugin_abi::{ABI_VERSION, ExtensionPoint, PluginManifest};

use crate::config::PluginHostConfig;
use crate::host::PluginHost;
use crate::plugin::PluginId;
use crate::resources::ResourcePool;

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

// --- Config edge cases ---

#[test]
fn config_default_values() {
    let config = PluginHostConfig::default();
    assert_eq!(config.max_plugins, 64);
    assert_eq!(config.max_memory_per_plugin, 64 * 1024 * 1024);
    assert_eq!(config.default_timeout_ms, 5000);
    assert!(config.allowed_extension_points.is_none());
    assert_eq!(config.resource_pool_capacity, 512 * 1024 * 1024);
}

#[test]
fn config_new_custom() {
    let config = PluginHostConfig::new(10, 2048);
    assert_eq!(config.max_plugins, 10);
    assert_eq!(config.max_memory_per_plugin, 2048);
    // Defaults inherited.
    assert_eq!(config.default_timeout_ms, 5000);
}

#[test]
fn config_is_extension_point_allowed_all() {
    let config = PluginHostConfig::default();
    assert!(config.is_extension_point_allowed(ExtensionPoint::PreAuth));
    assert!(config.is_extension_point_allowed(ExtensionPoint::PostAuth));
    assert!(config.is_extension_point_allowed(ExtensionPoint::InputFilter));
    assert!(config.is_extension_point_allowed(ExtensionPoint::ClipboardTransform));
    assert!(config.is_extension_point_allowed(ExtensionPoint::ChannelHandler));
    assert!(config.is_extension_point_allowed(ExtensionPoint::ShellWidget));
    assert!(config.is_extension_point_allowed(ExtensionPoint::PolicyHook));
    assert!(config.is_extension_point_allowed(ExtensionPoint::EncoderStage));
}

#[test]
fn config_is_extension_point_allowed_restricted() {
    let config = PluginHostConfig {
        allowed_extension_points: Some(vec![ExtensionPoint::InputFilter]),
        ..PluginHostConfig::default()
    };
    assert!(config.is_extension_point_allowed(ExtensionPoint::InputFilter));
    assert!(!config.is_extension_point_allowed(ExtensionPoint::PreAuth));
}

#[test]
fn config_display() {
    let config = PluginHostConfig::default();
    let s = format!("{config}");
    assert!(s.contains("PluginHostConfig"));
    assert!(s.contains("max_plugins=64"));
}

// --- Host sequential IDs ---

#[test]
fn host_plugin_ids_are_sequential() {
    let mut host = PluginHost::with_defaults();
    let id1 = host.load_plugin(sample_manifest("com.edge.seq1", vec![])).unwrap();
    let id2 = host.load_plugin(sample_manifest("com.edge.seq2", vec![])).unwrap();
    let id3 = host.load_plugin(sample_manifest("com.edge.seq3", vec![])).unwrap();
    assert_eq!(id1, PluginId(1));
    assert_eq!(id2, PluginId(2));
    assert_eq!(id3, PluginId(3));
}

#[test]
fn host_unload_nonexistent() {
    let mut host = PluginHost::with_defaults();
    assert!(host.unload_plugin(PluginId(999)).is_err());
}

#[test]
fn host_suspend_nonexistent() {
    let mut host = PluginHost::with_defaults();
    assert!(host.suspend_plugin(PluginId(999)).is_err());
}

#[test]
fn host_resume_nonexistent() {
    let mut host = PluginHost::with_defaults();
    assert!(host.resume_plugin(PluginId(999)).is_err());
}

#[test]
fn host_allocate_for_nonexistent() {
    let mut host = PluginHost::with_defaults();
    assert!(host.allocate_resource(PluginId(999), 64).is_err());
}

// --- Reload after unload ---

#[test]
fn host_reload_after_unload() {
    let mut host = PluginHost::with_defaults();
    let id1 = host
        .load_plugin(sample_manifest("com.edge.reload", vec![ExtensionPoint::PreAuth]))
        .unwrap();
    host.unload_plugin(id1).unwrap();

    // Can reload same manifest ID because the old one is now unloaded.
    let id2 = host
        .load_plugin(sample_manifest("com.edge.reload", vec![ExtensionPoint::PreAuth]))
        .unwrap();
    assert_ne!(id1, id2);
    assert_eq!(host.active_plugin_count(), 1);

    let results = host.dispatch(ExtensionPoint::PreAuth);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].plugin_id, id2);
}

// --- Multi-extension-point plugin ---

#[test]
fn host_multi_extension_plugin() {
    let mut host = PluginHost::with_defaults();
    let points = vec![
        ExtensionPoint::InputFilter,
        ExtensionPoint::PolicyHook,
        ExtensionPoint::ShellWidget,
    ];
    let id = host
        .load_plugin(sample_manifest("com.edge.multi", points))
        .unwrap();

    assert_eq!(host.dispatch(ExtensionPoint::InputFilter).len(), 1);
    assert_eq!(host.dispatch(ExtensionPoint::PolicyHook).len(), 1);
    assert_eq!(host.dispatch(ExtensionPoint::ShellWidget).len(), 1);
    assert_eq!(host.dispatch(ExtensionPoint::PreAuth).len(), 0);

    host.unload_plugin(id).unwrap();
    assert_eq!(host.dispatch(ExtensionPoint::InputFilter).len(), 0);
}

// --- Resource pool edge cases ---

#[test]
fn pool_allocate_then_free_then_allocate() {
    let mut pool = ResourcePool::new(100);
    let h1 = pool.allocate(80, PluginId(1)).unwrap();
    pool.free(h1).unwrap();
    // Now 100 available again.
    let h2 = pool.allocate(100, PluginId(1)).unwrap();
    assert_ne!(h1, h2); // Handles don't recycle.
    assert_eq!(pool.total_allocated(), 100);
}

#[test]
fn pool_zero_capacity() {
    let mut pool = ResourcePool::new(0);
    assert_eq!(pool.available(), 0);
    // Zero-size allocation should succeed.
    let handle = pool.allocate(0, PluginId(1)).unwrap();
    assert!(pool.get(handle).is_some());
    // Non-zero allocation should fail.
    assert!(pool.allocate(1, PluginId(1)).is_err());
}

// --- Error display ---

#[test]
fn error_display_plugin_not_found() {
    let err = crate::PluginHostError::PluginNotFound { id: PluginId(42) };
    assert_eq!(format!("{err}"), "plugin not found: Plugin(42)");
}

#[test]
fn error_display_incompatible_abi() {
    let err = crate::PluginHostError::IncompatibleAbi {
        expected: 1,
        found: 99,
    };
    assert_eq!(
        format!("{err}"),
        "incompatible ABI version: expected 1, found 99"
    );
}

#[test]
fn error_display_resource_exhausted() {
    let err = crate::PluginHostError::ResourceExhausted {
        requested: 1024,
        available: 100,
    };
    let s = format!("{err}");
    assert!(s.contains("1024"));
    assert!(s.contains("100"));
}

#[test]
fn error_display_duplicate() {
    let err = crate::PluginHostError::DuplicatePlugin {
        manifest_id: "com.dup.test".into(),
    };
    assert_eq!(format!("{err}"), "duplicate plugin: com.dup.test");
}

#[test]
fn error_display_internal() {
    let err = crate::PluginHostError::Internal("something broke".into());
    assert_eq!(format!("{err}"), "internal error: something broke");
}
