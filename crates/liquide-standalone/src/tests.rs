#[cfg(test)]
mod tests {
    use crate::config::StandaloneConfig;
    use crate::display::{DisplayOutput, OutputInfo};
    use crate::input::InputDeviceSummary;
    use crate::wayland::WaylandServerState;
    use crate::xwayland_bridge::XWaylandBridgeState;
    use liquide_drm::mode::{DrmMode, ModeFlags};

    #[test]
    fn test_standalone_config_default() {
        let config = StandaloneConfig::default();
        assert!(!config.dev_mode);
        assert!(config.vt_number.is_none());
        assert!(config.drm_device.is_none());
        assert_eq!(config.fps_cap, 0);
        assert_eq!(config.wayland_socket, "wayland-0");
        assert!(config.enable_xwayland);
        assert!(config.enable_wayland);
    }

    #[test]
    fn test_display_output() {
        let mut display = DisplayOutput::new();
        assert!(display.primary().is_none());
        display.add_output(OutputInfo {
            connector_id: 1,
            name: "HDMI-A-1".to_string(),
            mode: DrmMode {
                width: 1920, height: 1080, refresh_hz: 60,
                clock_khz: 148500, flags: ModeFlags::PREFERRED,
                name: "1920x1080@60".to_string(),
            },
            physical_width_mm: 520,
            physical_height_mm: 290,
            primary: true,
        });
        assert!(display.primary().is_some());
        assert_eq!(display.outputs().len(), 1);
    }

    #[test]
    fn test_input_summary_empty() {
        let summary = InputDeviceSummary::default();
        assert!(!summary.has_basic_input());
    }

    #[test]
    fn test_input_summary_from_devices() {
        use liquide_libinput::{DeviceInfo, DeviceClass, DeviceCapability};
        let devices = vec![
            DeviceInfo {
                path: "/dev/input/event0".to_string(),
                name: "Keyboard".to_string(),
                device_class: DeviceClass::Keyboard,
                capabilities: DeviceCapability::KEY,
                vendor_id: 0, product_id: 0, bus_type: 0,
            },
            DeviceInfo {
                path: "/dev/input/event1".to_string(),
                name: "Mouse".to_string(),
                device_class: DeviceClass::Mouse,
                capabilities: DeviceCapability::KEY.union(DeviceCapability::REL),
                vendor_id: 0, product_id: 0, bus_type: 0,
            },
        ];
        let summary = InputDeviceSummary::from_devices(devices);
        assert!(summary.has_basic_input());
        assert_eq!(summary.keyboard_count, 1);
        assert_eq!(summary.pointer_count, 1);
    }

    #[test]
    fn test_wayland_server_state() {
        let state = WaylandServerState::new();
        assert!(!state.is_accepting());
        assert_eq!(state.client_count(), 0);
    }

    #[test]
    fn test_xwayland_bridge_state() {
        let state = XWaylandBridgeState::default();
        assert!(!state.enabled);
        assert_eq!(state.window_count, 0);
    }
}
