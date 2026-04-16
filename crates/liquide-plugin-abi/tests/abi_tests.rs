//! Comprehensive tests for the liquide-plugin-abi crate.

use liquide_plugin_abi::host_functions::*;
use liquide_plugin_abi::plugin_manifest::{is_compatible, parse_manifest};
use liquide_plugin_abi::types::*;
use liquide_plugin_abi::{ABI_VERSION, ExtensionPoint, PluginManifest};

// =========================================================================
// ABI version constant
// =========================================================================

#[test]
fn abi_version_is_defined() {
    assert_eq!(ABI_VERSION, 1);
}

// =========================================================================
// PluginManifest
// =========================================================================

#[test]
fn manifest_serde_roundtrip_json() {
    let manifest = PluginManifest {
        id: "com.example.test-plugin".into(),
        name: "Test Plugin".into(),
        version: "1.0.0".into(),
        abi_version: ABI_VERSION,
        extension_points: vec![ExtensionPoint::InputFilter, ExtensionPoint::ClipboardTransform],
        requested_memory_bytes: 1024 * 1024,
    };
    let json = serde_json::to_vec(&manifest).unwrap();
    let decoded: PluginManifest = serde_json::from_slice(&json).unwrap();
    assert_eq!(decoded.id, manifest.id);
    assert_eq!(decoded.name, manifest.name);
    assert_eq!(decoded.version, manifest.version);
    assert_eq!(decoded.abi_version, manifest.abi_version);
    assert_eq!(decoded.extension_points, manifest.extension_points);
    assert_eq!(decoded.requested_memory_bytes, manifest.requested_memory_bytes);
}

#[test]
fn manifest_parse_valid_json() {
    let json = serde_json::json!({
        "id": "com.test.plugin",
        "name": "My Plugin",
        "version": "0.1.0",
        "abi_version": 1,
        "extension_points": ["PreAuth", "PostAuth"],
        "requested_memory_bytes": 65536
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let manifest = parse_manifest(&bytes).unwrap();
    assert_eq!(manifest.id, "com.test.plugin");
    assert_eq!(manifest.abi_version, 1);
    assert_eq!(manifest.extension_points.len(), 2);
    assert_eq!(manifest.extension_points[0], ExtensionPoint::PreAuth);
    assert_eq!(manifest.extension_points[1], ExtensionPoint::PostAuth);
}

#[test]
fn manifest_parse_invalid_json() {
    let bad_json = b"{ not valid json }}}";
    let result = parse_manifest(bad_json);
    assert!(result.is_err());
}

#[test]
fn manifest_parse_missing_fields() {
    let partial = serde_json::json!({ "id": "x" });
    let bytes = serde_json::to_vec(&partial).unwrap();
    let result = parse_manifest(&bytes);
    assert!(result.is_err());
}

#[test]
fn manifest_is_compatible_matching_version() {
    let manifest = PluginManifest {
        id: "com.test".into(),
        name: "Test".into(),
        version: "1.0.0".into(),
        abi_version: ABI_VERSION,
        extension_points: vec![],
        requested_memory_bytes: 0,
    };
    assert!(is_compatible(&manifest));
}

#[test]
fn manifest_is_compatible_wrong_version() {
    let manifest = PluginManifest {
        id: "com.test".into(),
        name: "Test".into(),
        version: "1.0.0".into(),
        abi_version: ABI_VERSION + 1,
        extension_points: vec![],
        requested_memory_bytes: 0,
    };
    assert!(!is_compatible(&manifest));
}

#[test]
fn manifest_is_compatible_zero_version() {
    let manifest = PluginManifest {
        id: "com.test".into(),
        name: "Test".into(),
        version: "1.0.0".into(),
        abi_version: 0,
        extension_points: vec![],
        requested_memory_bytes: 0,
    };
    assert!(!is_compatible(&manifest));
}

// =========================================================================
// ExtensionPoint
// =========================================================================

#[test]
fn extension_point_all_variants_serde() {
    let points = vec![
        ExtensionPoint::PreAuth,
        ExtensionPoint::PostAuth,
        ExtensionPoint::InputFilter,
        ExtensionPoint::ClipboardTransform,
        ExtensionPoint::ChannelHandler,
        ExtensionPoint::ShellWidget,
        ExtensionPoint::PolicyHook,
        ExtensionPoint::EncoderStage,
    ];
    let json = serde_json::to_string(&points).unwrap();
    let decoded: Vec<ExtensionPoint> = serde_json::from_str(&json).unwrap();
    assert_eq!(points, decoded);
}

#[test]
fn extension_point_debug() {
    let ep = ExtensionPoint::InputFilter;
    let debug = format!("{:?}", ep);
    assert_eq!(debug, "InputFilter");
}

#[test]
fn extension_point_equality() {
    assert_eq!(ExtensionPoint::PreAuth, ExtensionPoint::PreAuth);
    assert_ne!(ExtensionPoint::PreAuth, ExtensionPoint::PostAuth);
}

// =========================================================================
// Host functions
// =========================================================================

#[test]
fn host_function_constants() {
    assert_eq!(FN_LOG, 1);
    assert_eq!(FN_GET_CONFIG, 2);
    assert_eq!(FN_SEND_MESSAGE, 3);
    assert_eq!(FN_ALLOCATE_BUFFER, 4);
    assert_eq!(FN_FREE_BUFFER, 5);
}

#[test]
fn host_functions_registry_count() {
    assert_eq!(HOST_FUNCTIONS.len(), 5);
}

#[test]
fn host_functions_registry_indices_match() {
    for func in HOST_FUNCTIONS {
        match func.index {
            FN_LOG => {
                assert_eq!(func.name, "log");
                assert_eq!(func.param_count, 2);
            }
            FN_GET_CONFIG => {
                assert_eq!(func.name, "get_config");
                assert_eq!(func.param_count, 1);
            }
            FN_SEND_MESSAGE => {
                assert_eq!(func.name, "send_message");
                assert_eq!(func.param_count, 2);
            }
            FN_ALLOCATE_BUFFER => {
                assert_eq!(func.name, "allocate_buffer");
                assert_eq!(func.param_count, 1);
            }
            FN_FREE_BUFFER => {
                assert_eq!(func.name, "free_buffer");
                assert_eq!(func.param_count, 1);
            }
            _ => panic!("unexpected host function index: {}", func.index),
        }
    }
}

#[test]
fn host_functions_unique_indices() {
    let mut seen = std::collections::HashSet::new();
    for func in HOST_FUNCTIONS {
        assert!(seen.insert(func.index), "duplicate index: {}", func.index);
    }
}

// =========================================================================
// Types: ResourceHandle
// =========================================================================

#[test]
fn resource_handle_serde_roundtrip() {
    let handle = ResourceHandle(42);
    let json = serde_json::to_string(&handle).unwrap();
    let decoded: ResourceHandle = serde_json::from_str(&json).unwrap();
    assert_eq!(handle, decoded);
}

#[test]
fn resource_handle_equality_and_hash() {
    let h1 = ResourceHandle(1);
    let h2 = ResourceHandle(1);
    let h3 = ResourceHandle(2);
    assert_eq!(h1, h2);
    assert_ne!(h1, h3);

    let mut set = std::collections::HashSet::new();
    set.insert(h1);
    assert!(set.contains(&h2));
    assert!(!set.contains(&h3));
}

#[test]
fn resource_handle_clone_copy() {
    let h1 = ResourceHandle(99);
    let h2 = h1; // Copy
    let h3 = h1.clone();
    assert_eq!(h1, h2);
    assert_eq!(h1, h3);
}

#[test]
fn resource_handle_debug() {
    let h = ResourceHandle(123);
    let debug = format!("{:?}", h);
    assert!(debug.contains("123"));
}

#[test]
fn resource_handle_zero() {
    let h = ResourceHandle(0);
    assert_eq!(h.0, 0);
}

#[test]
fn resource_handle_max() {
    let h = ResourceHandle(u64::MAX);
    assert_eq!(h.0, u64::MAX);
    let json = serde_json::to_string(&h).unwrap();
    let decoded: ResourceHandle = serde_json::from_str(&json).unwrap();
    assert_eq!(h, decoded);
}

// =========================================================================
// Types: PluginResult
// =========================================================================

#[test]
fn plugin_result_values() {
    assert_eq!(PluginResult::Ok as i32, 0);
    assert_eq!(PluginResult::Error as i32, -1);
    assert_eq!(PluginResult::NotHandled as i32, -2);
    assert_eq!(PluginResult::PermissionDenied as i32, -3);
}

#[test]
fn plugin_result_serde_roundtrip() {
    for result in [
        PluginResult::Ok,
        PluginResult::Error,
        PluginResult::NotHandled,
        PluginResult::PermissionDenied,
    ] {
        let json = serde_json::to_string(&result).unwrap();
        let decoded: PluginResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, decoded);
    }
}

#[test]
fn plugin_result_debug() {
    assert_eq!(format!("{:?}", PluginResult::Ok), "Ok");
    assert_eq!(format!("{:?}", PluginResult::Error), "Error");
    assert_eq!(format!("{:?}", PluginResult::PermissionDenied), "PermissionDenied");
}

// =========================================================================
// Types: MetadataEntry
// =========================================================================

#[test]
fn metadata_entry_serde_roundtrip() {
    let entry = MetadataEntry {
        key: "author".into(),
        value: "Test Author".into(),
    };
    let json = serde_json::to_string(&entry).unwrap();
    let decoded: MetadataEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.key, entry.key);
    assert_eq!(decoded.value, entry.value);
}

#[test]
fn metadata_entry_unicode() {
    let entry = MetadataEntry {
        key: "description".into(),
        value: "日本語テスト 🎉".into(),
    };
    let json = serde_json::to_string(&entry).unwrap();
    let decoded: MetadataEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.value, "日本語テスト 🎉");
}

#[test]
fn metadata_entry_empty_strings() {
    let entry = MetadataEntry {
        key: "".into(),
        value: "".into(),
    };
    let json = serde_json::to_string(&entry).unwrap();
    let decoded: MetadataEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.key, "");
    assert_eq!(decoded.value, "");
}

#[test]
fn metadata_entry_clone_debug() {
    let entry = MetadataEntry {
        key: "k".into(),
        value: "v".into(),
    };
    let cloned = entry.clone();
    assert_eq!(cloned.key, entry.key);
    let debug = format!("{:?}", entry);
    assert!(debug.contains("MetadataEntry"));
}

// =========================================================================
// Type layout (repr stability)
// =========================================================================

#[test]
fn plugin_result_repr_i32() {
    // PluginResult is #[repr(i32)] so its size should be 4 bytes
    assert_eq!(std::mem::size_of::<PluginResult>(), 4);
    assert_eq!(std::mem::align_of::<PluginResult>(), 4);
}
