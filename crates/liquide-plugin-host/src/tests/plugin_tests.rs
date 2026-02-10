use liquide_plugin_abi::{ABI_VERSION, ExtensionPoint, PluginManifest};

use crate::plugin::{LoadedPlugin, PluginId, PluginState};

fn sample_manifest() -> PluginManifest {
    PluginManifest {
        id: "com.example.test-plugin".into(),
        name: "Test Plugin".into(),
        version: "1.0.0".into(),
        abi_version: ABI_VERSION,
        extension_points: vec![ExtensionPoint::InputFilter],
        requested_memory_bytes: 1024,
    }
}

#[test]
fn plugin_id_display() {
    assert_eq!(format!("{}", PluginId(1)), "Plugin(1)");
    assert_eq!(format!("{}", PluginId(0)), "Plugin(0)");
    assert_eq!(format!("{}", PluginId(999)), "Plugin(999)");
}

#[test]
fn plugin_state_display() {
    assert_eq!(format!("{}", PluginState::Loading), "Loading");
    assert_eq!(format!("{}", PluginState::Active), "Active");
    assert_eq!(format!("{}", PluginState::Suspended), "Suspended");
    assert_eq!(format!("{}", PluginState::Unloaded), "Unloaded");
    assert_eq!(
        format!(
            "{}",
            PluginState::Failed {
                reason: "crash".into()
            }
        ),
        "Failed(crash)"
    );
}

#[test]
fn loaded_plugin_new_starts_loading() {
    let p = LoadedPlugin::new(PluginId(1), sample_manifest(), 0);
    assert_eq!(p.state, PluginState::Loading);
    assert_eq!(p.manifest_id(), "com.example.test-plugin");
    assert_eq!(p.name(), "Test Plugin");
    assert_eq!(p.version(), "1.0.0");
    assert!(!p.is_active());
    assert!(!p.is_suspended());
    assert!(!p.is_failed());
    assert!(!p.is_unloaded());
}

#[test]
fn loaded_plugin_activate() {
    let mut p = LoadedPlugin::new(PluginId(1), sample_manifest(), 0);
    p.activate();
    assert!(p.is_active());
    assert_eq!(p.state, PluginState::Active);
}

#[test]
fn loaded_plugin_suspend() {
    let mut p = LoadedPlugin::new(PluginId(1), sample_manifest(), 0);
    p.activate();
    p.suspend();
    assert!(p.is_suspended());
}

#[test]
fn loaded_plugin_fail() {
    let mut p = LoadedPlugin::new(PluginId(1), sample_manifest(), 0);
    p.activate();
    p.fail("something went wrong");
    assert!(p.is_failed());
    assert_eq!(
        p.state,
        PluginState::Failed {
            reason: "something went wrong".into()
        }
    );
}

#[test]
fn loaded_plugin_unload() {
    let mut p = LoadedPlugin::new(PluginId(1), sample_manifest(), 0);
    p.activate();
    p.unload();
    assert!(p.is_unloaded());
}

#[test]
fn loaded_plugin_extension_points() {
    let p = LoadedPlugin::new(PluginId(1), sample_manifest(), 0);
    assert_eq!(p.extension_points(), &[ExtensionPoint::InputFilter]);
}

#[test]
fn loaded_plugin_config_json_default_none() {
    let p = LoadedPlugin::new(PluginId(1), sample_manifest(), 0);
    assert!(p.config_json.is_none());
}

#[test]
fn loaded_plugin_display() {
    let mut p = LoadedPlugin::new(PluginId(42), sample_manifest(), 0);
    p.activate();
    let s = format!("{p}");
    assert!(s.contains("Plugin(42)"));
    assert!(s.contains("com.example.test-plugin"));
    assert!(s.contains("Test Plugin"));
    assert!(s.contains("1.0.0"));
    assert!(s.contains("Active"));
}
