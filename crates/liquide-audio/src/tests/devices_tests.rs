//! Tests for the audio device enumeration module.

use crate::devices::*;

// ── EnumDeviceId ──────────────────────────────────────────────────────

#[test]
fn enum_device_id_equality() {
    assert_eq!(EnumDeviceId(1), EnumDeviceId(1));
    assert_ne!(EnumDeviceId(1), EnumDeviceId(2));
}

#[test]
fn enum_device_id_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(EnumDeviceId(1));
    set.insert(EnumDeviceId(2));
    set.insert(EnumDeviceId(1));
    assert_eq!(set.len(), 2);
}

// ── EnumDeviceType ────────────────────────────────────────────────────

#[test]
fn device_type_all_variants() {
    let types = [
        EnumDeviceType::Output,
        EnumDeviceType::Input,
        EnumDeviceType::Duplex,
    ];
    assert_eq!(types.len(), 3);
}

#[test]
fn device_type_display() {
    assert_eq!(format!("{}", EnumDeviceType::Output), "Output");
    assert_eq!(format!("{}", EnumDeviceType::Input), "Input");
    assert_eq!(format!("{}", EnumDeviceType::Duplex), "Duplex");
}

// ── DeviceAudioFormat ─────────────────────────────────────────────────

#[test]
fn device_audio_format_new() {
    let fmt = DeviceAudioFormat::new(48000, 2, 16);
    assert_eq!(fmt.sample_rate, 48000);
    assert_eq!(fmt.channels, 2);
    assert_eq!(fmt.bit_depth, 16);
}

#[test]
fn device_audio_format_frame_size() {
    let stereo_16 = DeviceAudioFormat::new(48000, 2, 16);
    assert_eq!(stereo_16.frame_size(), 4); // 2 channels * 2 bytes

    let mono_32 = DeviceAudioFormat::new(44100, 1, 32);
    assert_eq!(mono_32.frame_size(), 4); // 1 channel * 4 bytes

    let surround_24 = DeviceAudioFormat::new(96000, 6, 24);
    assert_eq!(surround_24.frame_size(), 18); // 6 channels * 3 bytes
}

#[test]
fn device_audio_format_byte_rate() {
    let fmt = DeviceAudioFormat::new(48000, 2, 16);
    assert_eq!(fmt.byte_rate(), 48000 * 4); // 192000
}

#[test]
fn device_audio_format_display() {
    let fmt = DeviceAudioFormat::new(48000, 2, 16);
    let s = format!("{fmt}");
    assert!(s.contains("48000"));
    assert!(s.contains("2ch"));
    assert!(s.contains("16bit"));
}

// ── AudioDevice ───────────────────────────────────────────────────────

#[test]
fn audio_device_new() {
    let dev = AudioDevice::new(
        EnumDeviceId(1),
        "alsa_output.pci".into(),
        "Built-in Audio".into(),
        EnumDeviceType::Output,
    );
    assert_eq!(dev.id, EnumDeviceId(1));
    assert_eq!(dev.name, "alsa_output.pci");
    assert_eq!(dev.description, "Built-in Audio");
    assert_eq!(dev.device_type, EnumDeviceType::Output);
    assert!(dev.sample_rates.is_empty());
    assert!(dev.channel_counts.is_empty());
    assert!(!dev.is_default);
}

#[test]
fn audio_device_supported_formats() {
    let mut dev = AudioDevice::new(
        EnumDeviceId(1),
        "dev".into(),
        "Device".into(),
        EnumDeviceType::Output,
    );
    dev.sample_rates = vec![44100, 48000];
    dev.channel_counts = vec![2];

    let formats = dev.supported_formats();
    // 2 rates * 1 channel count * 3 bit depths = 6 formats
    assert_eq!(formats.len(), 6);
}

#[test]
fn audio_device_supports_format() {
    let mut dev = AudioDevice::new(
        EnumDeviceId(1),
        "dev".into(),
        "Device".into(),
        EnumDeviceType::Output,
    );
    dev.sample_rates = vec![48000];
    dev.channel_counts = vec![2];

    let good = DeviceAudioFormat::new(48000, 2, 16);
    assert!(dev.supports_format(&good));

    let bad_rate = DeviceAudioFormat::new(44100, 2, 16);
    assert!(!dev.supports_format(&bad_rate));

    let bad_ch = DeviceAudioFormat::new(48000, 6, 16);
    assert!(!dev.supports_format(&bad_ch));
}

// ── DeviceEvent ───────────────────────────────────────────────────────

#[test]
fn device_event_display() {
    let ev = DeviceEvent::Removed(EnumDeviceId(5));
    let s = format!("{ev}");
    assert!(s.contains("DeviceRemoved"));
}

#[test]
fn device_event_property_changed() {
    let ev = DeviceEvent::PropertyChanged {
        device_id: EnumDeviceId(1),
        property: "description".into(),
    };
    let s = format!("{ev}");
    assert!(s.contains("PropertyChanged"));
    assert!(s.contains("description"));
}

// ── AudioDeviceManager ───────────────────────────────────────────────

#[test]
fn device_manager_new() {
    let mgr = AudioDeviceManager::new();
    assert_eq!(mgr.device_count(), 0);
    assert!(mgr.default_output().is_none());
    assert!(mgr.default_input().is_none());
}

#[test]
fn device_manager_default() {
    let mgr = AudioDeviceManager::default();
    assert_eq!(mgr.device_count(), 0);
}

#[test]
fn device_manager_add_device() {
    let mut mgr = AudioDeviceManager::new();
    let dev = AudioDevice::new(
        EnumDeviceId(0),
        "sink".into(),
        "Speakers".into(),
        EnumDeviceType::Output,
    );
    let id = mgr.add_device(dev);
    assert_eq!(mgr.device_count(), 1);
    assert!(mgr.get_device(id).is_some());
}

#[test]
fn device_manager_auto_default_output() {
    let mut mgr = AudioDeviceManager::new();
    let dev = AudioDevice::new(
        EnumDeviceId(0),
        "sink".into(),
        "Speakers".into(),
        EnumDeviceType::Output,
    );
    let id = mgr.add_device(dev);
    assert_eq!(mgr.default_output(), Some(id));
}

#[test]
fn device_manager_auto_default_input() {
    let mut mgr = AudioDeviceManager::new();
    let dev = AudioDevice::new(
        EnumDeviceId(0),
        "source".into(),
        "Microphone".into(),
        EnumDeviceType::Input,
    );
    let id = mgr.add_device(dev);
    assert_eq!(mgr.default_input(), Some(id));
}

#[test]
fn device_manager_remove_device() {
    let mut mgr = AudioDeviceManager::new();
    let dev = AudioDevice::new(
        EnumDeviceId(0),
        "sink".into(),
        "Speakers".into(),
        EnumDeviceType::Output,
    );
    let id = mgr.add_device(dev);
    assert_eq!(mgr.device_count(), 1);

    let removed = mgr.remove_device(id);
    assert!(removed.is_some());
    assert_eq!(mgr.device_count(), 0);
    assert!(mgr.default_output().is_none());
}

#[test]
fn device_manager_remove_nonexistent() {
    let mut mgr = AudioDeviceManager::new();
    assert!(mgr.remove_device(EnumDeviceId(999)).is_none());
}

#[test]
fn device_manager_set_default_output() {
    let mut mgr = AudioDeviceManager::new();
    let dev1 = AudioDevice::new(
        EnumDeviceId(0),
        "sink1".into(),
        "Speaker 1".into(),
        EnumDeviceType::Output,
    );
    let dev2 = AudioDevice::new(
        EnumDeviceId(0),
        "sink2".into(),
        "Speaker 2".into(),
        EnumDeviceType::Output,
    );
    let id1 = mgr.add_device(dev1);
    let id2 = mgr.add_device(dev2);

    assert_eq!(mgr.default_output(), Some(id1)); // First is auto-default
    assert!(mgr.set_default_output(id2));
    assert_eq!(mgr.default_output(), Some(id2));
}

#[test]
fn device_manager_set_default_input() {
    let mut mgr = AudioDeviceManager::new();
    let dev1 = AudioDevice::new(
        EnumDeviceId(0),
        "src1".into(),
        "Mic 1".into(),
        EnumDeviceType::Input,
    );
    let dev2 = AudioDevice::new(
        EnumDeviceId(0),
        "src2".into(),
        "Mic 2".into(),
        EnumDeviceType::Input,
    );
    let id1 = mgr.add_device(dev1);
    let id2 = mgr.add_device(dev2);

    assert_eq!(mgr.default_input(), Some(id1));
    assert!(mgr.set_default_input(id2));
    assert_eq!(mgr.default_input(), Some(id2));
}

#[test]
fn device_manager_cannot_set_output_as_input() {
    let mut mgr = AudioDeviceManager::new();
    let dev = AudioDevice::new(
        EnumDeviceId(0),
        "sink".into(),
        "Speakers".into(),
        EnumDeviceType::Output,
    );
    let id = mgr.add_device(dev);
    assert!(!mgr.set_default_input(id));
}

#[test]
fn device_manager_cannot_set_input_as_output() {
    let mut mgr = AudioDeviceManager::new();
    let dev = AudioDevice::new(
        EnumDeviceId(0),
        "source".into(),
        "Mic".into(),
        EnumDeviceType::Input,
    );
    let id = mgr.add_device(dev);
    assert!(!mgr.set_default_output(id));
}

#[test]
fn device_manager_duplex_can_be_both() {
    let mut mgr = AudioDeviceManager::new();
    let dev = AudioDevice::new(
        EnumDeviceId(0),
        "usb".into(),
        "USB Interface".into(),
        EnumDeviceType::Duplex,
    );
    let id = mgr.add_device(dev);

    // Duplex auto-becomes default output.
    assert_eq!(mgr.default_output(), Some(id));

    // Can also be set as default input.
    assert!(mgr.set_default_input(id));
    assert_eq!(mgr.default_input(), Some(id));
}

#[test]
fn device_manager_devices_by_type() {
    let mut mgr = AudioDeviceManager::new();
    let out = AudioDevice::new(EnumDeviceId(0), "out".into(), "Out".into(), EnumDeviceType::Output);
    let inp = AudioDevice::new(EnumDeviceId(0), "in".into(), "In".into(), EnumDeviceType::Input);
    mgr.add_device(out);
    mgr.add_device(inp);

    assert_eq!(mgr.devices_by_type(EnumDeviceType::Output).len(), 1);
    assert_eq!(mgr.devices_by_type(EnumDeviceType::Input).len(), 1);
    assert_eq!(mgr.devices_by_type(EnumDeviceType::Duplex).len(), 0);
}

#[test]
fn device_manager_supported_formats() {
    let mut mgr = AudioDeviceManager::new();
    let mut dev = AudioDevice::new(
        EnumDeviceId(0),
        "sink".into(),
        "Speakers".into(),
        EnumDeviceType::Output,
    );
    dev.sample_rates = vec![48000];
    dev.channel_counts = vec![2];
    let id = mgr.add_device(dev);

    let formats = mgr.supported_formats(id);
    assert_eq!(formats.len(), 3); // 1 rate * 1 ch * 3 bit depths
}

#[test]
fn device_manager_supported_formats_nonexistent() {
    let mgr = AudioDeviceManager::new();
    let formats = mgr.supported_formats(EnumDeviceId(999));
    assert!(formats.is_empty());
}

#[test]
fn device_manager_drain_events() {
    let mut mgr = AudioDeviceManager::new();
    let dev = AudioDevice::new(
        EnumDeviceId(0),
        "sink".into(),
        "Speakers".into(),
        EnumDeviceType::Output,
    );
    let id = mgr.add_device(dev);

    let events = mgr.drain_events();
    assert_eq!(events.len(), 1); // Added event

    mgr.remove_device(id);
    let events2 = mgr.drain_events();
    assert_eq!(events2.len(), 1); // Removed event

    let events3 = mgr.drain_events();
    assert!(events3.is_empty());
}

#[test]
fn device_manager_display() {
    let mgr = AudioDeviceManager::new();
    let s = format!("{mgr}");
    assert!(s.contains("AudioDeviceManager"));
}
