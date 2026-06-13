#![cfg(target_os = "windows")]

use liquide_platform::win32::{
    DxgiPresentCapabilities, DxgiPresentMode, refresh_rate_hz_from_devmode_frequency,
};

#[test]
fn win32_refresh_metadata_ignores_default_frequency_sentinels() {
    assert_eq!(refresh_rate_hz_from_devmode_frequency(0), None);
    assert_eq!(refresh_rate_hz_from_devmode_frequency(1), None);
    assert_eq!(refresh_rate_hz_from_devmode_frequency(75), Some(75));
    assert_eq!(refresh_rate_hz_from_devmode_frequency(240), Some(240));
}

#[test]
fn dxgi_immediate_mode_uses_tearing_only_when_capable() {
    let tearing = DxgiPresentCapabilities::dxgi_swap_chain(true);
    let no_tearing = DxgiPresentCapabilities::dxgi_swap_chain(false);

    let tearing_params = DxgiPresentMode::Immediate.present_parameters(tearing);
    let no_tearing_params = DxgiPresentMode::Immediate.present_parameters(no_tearing);

    assert_eq!(tearing_params.sync_interval, 0);
    assert_ne!(tearing_params.flags, 0);
    assert_eq!(no_tearing_params.sync_interval, 0);
    assert_eq!(no_tearing_params.flags, 0);
}

#[test]
fn dxgi_refresh_sync_uses_vsync_and_disables_tearing_flag() {
    let capabilities = DxgiPresentCapabilities::dxgi_swap_chain(true);
    let params = DxgiPresentMode::RefreshSync.present_parameters(capabilities);

    assert!(capabilities.supports(DxgiPresentMode::RefreshSync));
    assert_eq!(params.sync_interval, 1);
    assert_eq!(params.flags, 0);
}

#[test]
fn dxgi_refresh_sync_falls_back_to_immediate_when_unsupported() {
    let capabilities = DxgiPresentCapabilities::IMMEDIATE_ONLY;
    let params = DxgiPresentMode::RefreshSync.present_parameters(capabilities);

    assert!(!capabilities.supports(DxgiPresentMode::RefreshSync));
    assert_eq!(
        DxgiPresentMode::RefreshSync.resolve(capabilities),
        DxgiPresentMode::Immediate
    );
    assert_eq!(params.sync_interval, 0);
    assert_eq!(params.flags, 0);
}
