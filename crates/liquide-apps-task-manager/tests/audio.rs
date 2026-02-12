//! Tests for `audio` submodule types.

use liquide_apps_task_manager::audio::*;
use liquide_apps_task_manager::audio::device::*;
use liquide_apps_task_manager::audio::stream::*;
use liquide_apps_task_manager::audio::effects::*;
use liquide_apps_task_manager::audio::routing::*;
use liquide_apps_task_manager::audio::spatial::*;
use liquide_apps_task_manager::audio::midi::*;
use liquide_apps_task_manager::audio::stats::*;
use liquide_apps_task_manager::audio::diagnostics::*;

// ---------------------------------------------------------------------------
// AudioView
// ---------------------------------------------------------------------------

#[test]
fn audio_view_all_variants() {
    let variants = [
        AudioView::OutputDevices,
        AudioView::InputDevices,
        AudioView::Streams,
        AudioView::Routing,
        AudioView::Effects,
        AudioView::Spatial,
        AudioView::Midi,
        AudioView::Stats,
        AudioView::Diagnostics,
        AudioView::Overview,
    ];
    assert_eq!(variants.len(), 10);
}

#[test]
fn audio_view_display() {
    assert_eq!(AudioView::OutputDevices.to_string(), "Output Devices");
    assert_eq!(AudioView::Midi.to_string(), "MIDI");
    assert_eq!(AudioView::Overview.to_string(), "Overview");
}

// ---------------------------------------------------------------------------
// AudioDeviceStatus
// ---------------------------------------------------------------------------

#[test]
fn audio_device_status_all_variants() {
    let variants = [
        AudioDeviceStatus::Active,
        AudioDeviceStatus::Disabled,
        AudioDeviceStatus::NotPresent,
        AudioDeviceStatus::Unplugged,
        AudioDeviceStatus::Default,
        AudioDeviceStatus::Exclusive,
    ];
    assert_eq!(variants.len(), 6);
}

#[test]
fn audio_device_status_display() {
    assert_eq!(AudioDeviceStatus::Active.to_string(), "Active");
    assert_eq!(AudioDeviceStatus::NotPresent.to_string(), "Not Present");
    assert_eq!(AudioDeviceStatus::Exclusive.to_string(), "Exclusive");
}

#[test]
fn audio_device_status_serde_roundtrip() {
    let val = AudioDeviceStatus::Default;
    let json = serde_json::to_string(&val).unwrap();
    let back: AudioDeviceStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(back, val);
}

// ---------------------------------------------------------------------------
// OutputType
// ---------------------------------------------------------------------------

#[test]
fn output_type_all_variants() {
    let variants = [
        OutputType::Speakers,
        OutputType::Headphones,
        OutputType::Hdmi,
        OutputType::DisplayPort,
        OutputType::Bluetooth,
        OutputType::Usb,
        OutputType::Spdif,
        OutputType::Analog,
        OutputType::Virtual,
    ];
    assert_eq!(variants.len(), 9);
}

#[test]
fn output_type_display() {
    assert_eq!(OutputType::Speakers.to_string(), "Speakers");
    assert_eq!(OutputType::Hdmi.to_string(), "HDMI");
    assert_eq!(OutputType::Spdif.to_string(), "S/PDIF");
    assert_eq!(OutputType::Virtual.to_string(), "Virtual");
}

// ---------------------------------------------------------------------------
// InputType
// ---------------------------------------------------------------------------

#[test]
fn input_type_all_variants() {
    let variants = [
        InputType::Microphone,
        InputType::LineIn,
        InputType::Bluetooth,
        InputType::Usb,
        InputType::Hdmi,
        InputType::Spdif,
        InputType::Loopback,
        InputType::Virtual,
    ];
    assert_eq!(variants.len(), 8);
}

// ---------------------------------------------------------------------------
// ChannelConfig
// ---------------------------------------------------------------------------

#[test]
fn channel_config_all_variants() {
    let variants = [
        ChannelConfig::Mono,
        ChannelConfig::Stereo,
        ChannelConfig::Surround51,
        ChannelConfig::Surround71,
        ChannelConfig::Custom,
    ];
    assert_eq!(variants.len(), 5);
}

// ---------------------------------------------------------------------------
// AudioFormat
// ---------------------------------------------------------------------------

#[test]
fn audio_format_all_variants() {
    let variants = [
        AudioFormat::Pcm16,
        AudioFormat::Pcm24,
        AudioFormat::Pcm32,
        AudioFormat::Float32,
        AudioFormat::Float64,
        AudioFormat::Dsd,
        AudioFormat::Compressed,
    ];
    assert_eq!(variants.len(), 7);
}

// ---------------------------------------------------------------------------
// ExclusiveMode
// ---------------------------------------------------------------------------

#[test]
fn exclusive_mode_all_variants() {
    let variants = [
        ExclusiveMode::Shared,
        ExclusiveMode::Exclusive,
        ExclusiveMode::Passthrough,
    ];
    assert_eq!(variants.len(), 3);
}

// ---------------------------------------------------------------------------
// SpatialMode
// ---------------------------------------------------------------------------

#[test]
fn spatial_mode_all_variants() {
    let variants = [
        SpatialMode::Off,
        SpatialMode::Stereo,
        SpatialMode::Surround,
        SpatialMode::Binaural,
        SpatialMode::ObjectBased,
    ];
    assert_eq!(variants.len(), 5);
}

// ---------------------------------------------------------------------------
// MeterType
// ---------------------------------------------------------------------------

#[test]
fn meter_type_all_variants() {
    let variants = [
        MeterType::Peak,
        MeterType::Rms,
        MeterType::Vu,
        MeterType::Lufs,
        MeterType::TruePeak,
    ];
    assert_eq!(variants.len(), 5);
}

// ---------------------------------------------------------------------------
// StreamDirection, StreamFormat, StreamState
// ---------------------------------------------------------------------------

#[test]
fn stream_direction_all_variants() {
    let variants = [StreamDirection::Output, StreamDirection::Input];
    assert_eq!(variants.len(), 2);
}

#[test]
fn stream_format_all_variants() {
    let variants = [
        StreamFormat::Pcm,
        StreamFormat::Compressed,
        StreamFormat::Raw,
        StreamFormat::Passthrough,
    ];
    assert_eq!(variants.len(), 4);
}

#[test]
fn stream_state_all_variants() {
    let variants = [
        StreamState::Active,
        StreamState::Inactive,
        StreamState::Suspended,
        StreamState::Error,
    ];
    assert_eq!(variants.len(), 4);
}

#[test]
fn stream_state_display() {
    assert_eq!(StreamState::Active.to_string(), "Active");
    assert_eq!(StreamState::Suspended.to_string(), "Suspended");
}

// ---------------------------------------------------------------------------
// StreamAction
// ---------------------------------------------------------------------------

#[test]
fn stream_action_all_variants() {
    let variants = [
        StreamAction::Mute,
        StreamAction::Unmute,
        StreamAction::SetVolume,
        StreamAction::Redirect,
        StreamAction::AddEffect,
        StreamAction::RemoveEffect,
        StreamAction::Properties,
        StreamAction::Record,
        StreamAction::Monitor,
    ];
    assert_eq!(variants.len(), 9);
}

// ---------------------------------------------------------------------------
// AudioEffect
// ---------------------------------------------------------------------------

#[test]
fn audio_effect_all_variants() {
    let variants = [
        AudioEffect::Equalizer,
        AudioEffect::Compressor,
        AudioEffect::Limiter,
        AudioEffect::NoiseGate,
        AudioEffect::Reverb,
        AudioEffect::Echo,
        AudioEffect::BassBoost,
        AudioEffect::VirtualSurround,
        AudioEffect::LoudnessEqualization,
        AudioEffect::RoomCorrection,
    ];
    assert_eq!(variants.len(), 10);
}

#[test]
fn audio_effect_display() {
    assert_eq!(AudioEffect::Equalizer.to_string(), "Equalizer");
    assert_eq!(AudioEffect::NoiseGate.to_string(), "Noise Gate");
    assert_eq!(AudioEffect::RoomCorrection.to_string(), "Room Correction");
}

// ---------------------------------------------------------------------------
// EffectNode construction
// ---------------------------------------------------------------------------

#[test]
fn effect_node_construction() {
    let node = EffectNode {
        effect: AudioEffect::Equalizer,
        enabled: true,
        device_id: "dev-1".into(),
        order: 0,
        parameters: "{}".into(),
    };
    assert_eq!(node.effect, AudioEffect::Equalizer);
    assert!(node.enabled);
}

// ---------------------------------------------------------------------------
// DspLoad construction
// ---------------------------------------------------------------------------

#[test]
fn dsp_load_construction() {
    let load = DspLoad {
        device_id: "dev-1".into(),
        device_name: "Speakers".into(),
        cpu_percent: 5.0,
        latency_contribution_ms: 10.0,
        effect_count: 3,
    };
    assert_eq!(load.cpu_percent, 5.0);
    assert_eq!(load.effect_count, 3);
}

// ---------------------------------------------------------------------------
// AudioRoutingEntry
// ---------------------------------------------------------------------------

#[test]
fn audio_routing_entry_construction() {
    let entry = AudioRoutingEntry {
        source_device_id: "src-1".into(),
        source_name: "App Output".into(),
        target_device_id: "tgt-1".into(),
        target_name: "Speakers".into(),
        active: true,
        volume_percent: 80.0,
        muted: false,
    };
    assert!(entry.active);
    assert_eq!(entry.volume_percent, 80.0);
}

// ---------------------------------------------------------------------------
// SpatialEngine
// ---------------------------------------------------------------------------

#[test]
fn spatial_engine_all_variants() {
    let variants = [
        SpatialEngine::None,
        SpatialEngine::WindowsSonic,
        SpatialEngine::DolbyAtmos,
        SpatialEngine::DtsX,
        SpatialEngine::Custom,
    ];
    assert_eq!(variants.len(), 5);
}

// ---------------------------------------------------------------------------
// MidiDeviceType
// ---------------------------------------------------------------------------

#[test]
fn midi_device_type_all_variants() {
    let variants = [
        MidiDeviceType::Input,
        MidiDeviceType::Output,
        MidiDeviceType::Both,
    ];
    assert_eq!(variants.len(), 3);
}

// ---------------------------------------------------------------------------
// FftWindow & SpectrumMode
// ---------------------------------------------------------------------------

#[test]
fn fft_window_all_variants() {
    let variants = [
        FftWindow::Hann,
        FftWindow::Hamming,
        FftWindow::Blackman,
        FftWindow::FlatTop,
    ];
    assert_eq!(variants.len(), 4);
}

#[test]
fn spectrum_mode_all_variants() {
    let variants = [
        SpectrumMode::ThirdOctave,
        SpectrumMode::FullOctave,
        SpectrumMode::Linear,
        SpectrumMode::Log,
        SpectrumMode::Mel,
        SpectrumMode::Bark,
        SpectrumMode::Erb,
    ];
    assert_eq!(variants.len(), 7);
}

// ---------------------------------------------------------------------------
// AudioTest
// ---------------------------------------------------------------------------

#[test]
fn audio_test_all_variants() {
    let variants = [
        AudioTest::SineTone,
        AudioTest::WhiteNoise,
        AudioTest::PinkNoise,
        AudioTest::Sweep,
        AudioTest::ChannelIdentification,
        AudioTest::LatencyMeasurement,
        AudioTest::LoopbackTest,
        AudioTest::SpeakerPolarity,
        AudioTest::FrequencyResponse,
        AudioTest::ThresholdOfHearing,
        AudioTest::BluetoothCodecTest,
        AudioTest::BufferStressTest,
        AudioTest::SampleRateConversion,
        AudioTest::ExclusiveModeTest,
    ];
    assert_eq!(variants.len(), 14);
}

// ---------------------------------------------------------------------------
// AudioEventType
// ---------------------------------------------------------------------------

#[test]
fn audio_event_type_all_variants() {
    let variants = [
        AudioEventType::DeviceAdded,
        AudioEventType::DeviceRemoved,
        AudioEventType::DefaultChanged,
        AudioEventType::FormatChanged,
        AudioEventType::VolumeChanged,
        AudioEventType::StreamCreated,
        AudioEventType::StreamDestroyed,
        AudioEventType::GlitchDetected,
        AudioEventType::ExclusiveModeLost,
        AudioEventType::DriverError,
        AudioEventType::LatencySpike,
    ];
    assert_eq!(variants.len(), 11);
}
