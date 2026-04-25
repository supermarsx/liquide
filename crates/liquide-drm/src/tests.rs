#[cfg(test)]
mod tests {
    use crate::atomic::{AtomicFlags, AtomicRequest};
    use crate::connector::{
        ConnectorId, ConnectorInfo, ConnectorStatus, ConnectorType, SubpixelOrder,
        stable_connector_name,
    };
    use crate::crtc::CrtcId;
    use crate::error::DrmError;
    use crate::mode::{
        DrmMode, ModeFlags, RawDrmModeInfo, current_mode, from_raw_mode_info, launchable_mode,
    };
    use crate::pageflip::{
        DrmEvent, PageFlipEvent, PageFlipFlags, UnknownDrmEvent, VblankEvent, parse_drm_events,
    };

    const DRM_EVENT_VBLANK: u32 = 0x01;
    const DRM_EVENT_FLIP_COMPLETE: u32 = 0x02;

    #[test]
    fn test_drm_mode_preferred() {
        let mode = DrmMode {
            width: 1920,
            height: 1080,
            refresh_hz: 60,
            clock_khz: 148500,
            flags: ModeFlags::PREFERRED,
            name: "1920x1080@60".to_string(),
        };
        assert!(mode.is_preferred());
        assert_eq!(mode.width, 1920);
        assert_eq!(mode.height, 1080);
    }

    #[test]
    fn test_drm_mode_not_preferred() {
        let mode = DrmMode {
            width: 1280,
            height: 720,
            refresh_hz: 60,
            clock_khz: 74250,
            flags: ModeFlags::empty(),
            name: "1280x720@60".to_string(),
        };
        assert!(!mode.is_preferred());
    }

    #[test]
    fn test_preferred_mode_selection() {
        let modes = vec![
            DrmMode {
                width: 1280,
                height: 720,
                refresh_hz: 60,
                clock_khz: 74250,
                flags: ModeFlags::empty(),
                name: "720p".to_string(),
            },
            DrmMode {
                width: 1920,
                height: 1080,
                refresh_hz: 60,
                clock_khz: 148500,
                flags: ModeFlags::PREFERRED,
                name: "1080p".to_string(),
            },
        ];
        let preferred = crate::mode::preferred_mode(&modes);
        assert!(preferred.is_some());
        assert_eq!(preferred.unwrap().width, 1920);
    }

    #[test]
    fn test_current_mode_selection() {
        let modes = vec![
            DrmMode {
                width: 2560,
                height: 1440,
                refresh_hz: 144,
                clock_khz: 241500,
                flags: ModeFlags::CURRENT,
                name: "current".to_string(),
            },
            DrmMode {
                width: 2560,
                height: 1440,
                refresh_hz: 165,
                clock_khz: 300000,
                flags: ModeFlags::PREFERRED,
                name: "preferred".to_string(),
            },
        ];

        let current = current_mode(&modes).expect("current mode should be selected");
        assert_eq!(current.name, "current");
    }

    #[test]
    fn test_launchable_mode_selection_prefers_current_then_preferred_then_first_usable() {
        let current = DrmMode {
            width: 1920,
            height: 1080,
            refresh_hz: 60,
            clock_khz: 148500,
            flags: ModeFlags::CURRENT,
            name: "current".to_string(),
        };
        let preferred = DrmMode {
            width: 2560,
            height: 1440,
            refresh_hz: 144,
            clock_khz: 241500,
            flags: ModeFlags::PREFERRED,
            name: "preferred".to_string(),
        };

        assert_eq!(launchable_mode(&[current.clone(), preferred.clone()]), Some(&current));
        assert_eq!(launchable_mode(&[preferred.clone()]), Some(&preferred));

        let fallback = DrmMode {
            width: 1280,
            height: 720,
            refresh_hz: 0,
            clock_khz: 74250,
            flags: ModeFlags::empty(),
            name: "fallback".to_string(),
        };
        assert_eq!(launchable_mode(&[fallback.clone()]), Some(&fallback));

        let unusable = DrmMode {
            width: 0,
            height: 720,
            refresh_hz: 60,
            clock_khz: 74250,
            flags: ModeFlags::CURRENT,
            name: "bad".to_string(),
        };
        assert_eq!(launchable_mode(&[unusable]), None);
    }

    #[test]
    fn test_translate_raw_mode_sets_flags_refresh_and_name() {
        let mut raw = RawDrmModeInfo {
            clock: 148_500,
            hdisplay: 1920,
            htotal: 2200,
            vdisplay: 1080,
            vtotal: 1125,
            vrefresh: 60,
            flags: 1 << 4,
            mode_type: (1 << 1) | (1 << 3),
            ..Default::default()
        };
        write_mode_name(&mut raw.name, b"1920x1080");

        let mode = from_raw_mode_info(&raw).expect("raw mode should translate");
        assert_eq!(mode.width, 1920);
        assert_eq!(mode.height, 1080);
        assert_eq!(mode.refresh_hz, 60);
        assert!(mode.is_current());
        assert!(mode.is_preferred());
        assert!(mode.flags.contains(ModeFlags::INTERLACE));
        assert_eq!(mode.name, "1920x1080");
    }

    #[test]
    fn test_translate_raw_mode_computes_refresh_when_driver_omits_vrefresh() {
        let raw = RawDrmModeInfo {
            clock: 241_500,
            hdisplay: 2560,
            htotal: 2720,
            vdisplay: 1440,
            vtotal: 1481,
            ..Default::default()
        };

        let mode = from_raw_mode_info(&raw).expect("raw mode should translate");
        assert_eq!(mode.refresh_hz, 60);
        assert_eq!(mode.name, "2560x1440@60");
    }

    #[test]
    fn test_translate_raw_mode_rejects_unusable_geometry() {
        let raw = RawDrmModeInfo {
            hdisplay: 0,
            vdisplay: 1080,
            ..Default::default()
        };

        assert!(from_raw_mode_info(&raw).is_none());
    }

    #[test]
    fn test_connector_types() {
        assert_ne!(ConnectorType::HDMI, ConnectorType::DisplayPort);
        let id = ConnectorId(42);
        assert_eq!(id.0, 42);
    }

    #[test]
    fn test_crtc_ids() {
        let id = CrtcId(1);
        assert_eq!(id.0, 1);
    }

    #[test]
    fn test_atomic_request() {
        let mut req = AtomicRequest::new();
        req.add_property(1, 2, 3);
        assert_eq!(req.changes().len(), 1);
    }

    #[test]
    fn test_atomic_flags() {
        let flags = AtomicFlags::NONBLOCK | AtomicFlags::PAGE_FLIP_EVENT;
        assert!(flags.contains(AtomicFlags::NONBLOCK));
        assert!(!flags.contains(AtomicFlags::ALLOW_MODESET));
    }

    #[test]
    fn test_pageflip_flags() {
        let flags = PageFlipFlags::EVENT;
        assert!(flags.contains(PageFlipFlags::EVENT));
    }

    #[test]
    fn test_parse_pageflip_event_buffer() {
        let buffer = build_vblank_like_record(DRM_EVENT_FLIP_COMPLETE, 3, 42_500, 27, 9);
        let events = parse_drm_events(&buffer).unwrap();

        assert_eq!(
            events,
            vec![DrmEvent::PageFlip(PageFlipEvent {
                sequence: 27,
                timestamp_ns: 3_042_500_000,
                crtc_id: CrtcId(9),
            })]
        );
    }

    #[test]
    fn test_parse_vblank_event_buffer() {
        let buffer = build_vblank_like_record(DRM_EVENT_VBLANK, 11, 125, 91, 4);
        let events = parse_drm_events(&buffer).unwrap();

        assert_eq!(
            events,
            vec![DrmEvent::Vblank(VblankEvent {
                sequence: 91,
                timestamp_ns: 11_000_125_000,
                crtc_id: CrtcId(4),
            })]
        );
    }

    #[test]
    fn test_parse_unknown_event_passthrough() {
        let buffer = build_unknown_record(0x55, &[0xAA, 0xBB, 0xCC, 0xDD]);
        let events = parse_drm_events(&buffer).unwrap();

        assert_eq!(
            events,
            vec![DrmEvent::Unknown(UnknownDrmEvent {
                event_type: 0x55,
                raw_record: buffer,
            })]
        );
    }

    #[test]
    fn test_parse_invalid_event_buffers_fail() {
        let truncated_header = vec![0x01, 0x00, 0x00, 0x00];
        assert!(matches!(
            parse_drm_events(&truncated_header),
            Err(DrmError::EventBufferTruncated {
                offset: 0,
                expected: 8,
                actual: 4,
            })
        ));

        let mut truncated_record = build_vblank_like_record(DRM_EVENT_FLIP_COMPLETE, 1, 0, 2, 3);
        truncated_record.truncate(20);
        assert!(matches!(
            parse_drm_events(&truncated_record),
            Err(DrmError::EventBufferTruncated {
                offset: 0,
                expected: 32,
                actual: 20,
            })
        ));

        let malformed_record = build_header_only_record(DRM_EVENT_VBLANK, 4);
        assert!(matches!(
            parse_drm_events(&malformed_record),
            Err(DrmError::EventBufferMalformed { offset: 0, .. })
        ));
    }

    #[test]
    fn test_device_find_primary_non_linux() {
        // On non-Linux platforms, this should return NoDevice.
        #[cfg(not(target_os = "linux"))]
        {
            let result = crate::device::DrmDevice::find_primary();
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_connector_status_display() {
        let status = ConnectorStatus::Connected;
        assert_eq!(format!("{:?}", status), "Connected");
    }

    #[test]
    fn test_connector_name_shape_and_launchable_mode_helper() {
        assert_eq!(stable_connector_name(10, 1, 99), "DP-1");
        assert_eq!(stable_connector_name(11, 2, 99), "HDMI-A-2");
        assert_eq!(stable_connector_name(0, 0, 42), "Unknown-42");

        let connector = ConnectorInfo {
            id: ConnectorId(9),
            connector_type: ConnectorType::DisplayPort,
            connector_type_id: 1,
            name: "DP-1".to_string(),
            status: ConnectorStatus::Connected,
            modes: vec![
                DrmMode {
                    width: 1920,
                    height: 1080,
                    refresh_hz: 60,
                    clock_khz: 148500,
                    flags: ModeFlags::empty(),
                    name: "fallback".to_string(),
                },
                DrmMode {
                    width: 2560,
                    height: 1440,
                    refresh_hz: 144,
                    clock_khz: 241500,
                    flags: ModeFlags::CURRENT,
                    name: "current".to_string(),
                },
            ],
            physical_width_mm: 600,
            physical_height_mm: 340,
            subpixel_order: SubpixelOrder::HorizontalRgb,
            encoder_id: Some(7),
        };

        assert!(connector.is_connected());
        assert_eq!(connector.stable_name(), "DP-1");
        assert_eq!(connector.launchable_mode().map(|mode| mode.name.as_str()), Some("current"));
    }

    #[test]
    fn test_subpixel_order() {
        let order = SubpixelOrder::HorizontalRgb;
        assert_eq!(format!("{:?}", order), "HorizontalRgb");
    }

    fn write_mode_name(target: &mut [u8; 32], value: &[u8]) {
        let len = value.len().min(target.len().saturating_sub(1));
        target[..len].copy_from_slice(&value[..len]);
        target[len] = 0;
    }

    fn build_vblank_like_record(
        event_type: u32,
        seconds: u32,
        microseconds: u32,
        sequence: u32,
        crtc_id: u32,
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        push_u32_native(&mut bytes, event_type);
        push_u32_native(&mut bytes, 32);
        push_u64_native(&mut bytes, 0);
        push_u32_native(&mut bytes, seconds);
        push_u32_native(&mut bytes, microseconds);
        push_u32_native(&mut bytes, sequence);
        push_u32_native(&mut bytes, crtc_id);
        bytes
    }

    fn build_unknown_record(event_type: u32, payload: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        push_u32_native(&mut bytes, event_type);
        push_u32_native(&mut bytes, (8 + payload.len()) as u32);
        bytes.extend_from_slice(payload);
        bytes
    }

    fn build_header_only_record(event_type: u32, length: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        push_u32_native(&mut bytes, event_type);
        push_u32_native(&mut bytes, length);
        bytes
    }

    fn push_u32_native(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend_from_slice(&value.to_ne_bytes());
    }

    fn push_u64_native(bytes: &mut Vec<u8>, value: u64) {
        bytes.extend_from_slice(&value.to_ne_bytes());
    }
}
