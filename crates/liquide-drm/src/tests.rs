#[cfg(test)]
mod tests {
    use crate::atomic::{
        AtomicFlags, AtomicRequest, EncodedAtomicRequest, ObjectId, PropertyChange, PropertyId,
        atomic_args_from_encoded, encode_atomic_request,
    };
    use crate::connector::{
        ConnectorId, ConnectorInfo, ConnectorStatus, ConnectorType, SubpixelOrder,
        stable_connector_name,
    };
    use crate::crtc::{CrtcId, CrtcInfo, select_crtc_for_connector};
    use crate::encoder::{
        DRM_MODE_ENCODER_DAC, DRM_MODE_ENCODER_DPI, DRM_MODE_ENCODER_DPMST, DRM_MODE_ENCODER_DSI,
        DRM_MODE_ENCODER_LVDS, DRM_MODE_ENCODER_NONE, DRM_MODE_ENCODER_TMDS,
        DRM_MODE_ENCODER_TVDAC, DRM_MODE_ENCODER_VIRTUAL, EncoderId, EncoderInfo, EncoderType,
        encoder_type_from_raw,
    };
    use crate::error::DrmError;
    use crate::mode::{
        DrmMode, ModeFlags, RawDrmModeInfo, closest_refresh_mode, current_mode,
        from_raw_mode_info, highest_resolution_mode, launchable_mode, match_mode_by_dimensions,
    };
    use crate::pageflip::{
        DrmEvent, PageFlipEvent, PageFlipFlags, PresentRequest, UnknownDrmEvent, VblankEvent,
        page_flip_request_args, parse_drm_events,
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
        req.add_property(ObjectId(1), PropertyId(2), 3);
        assert_eq!(req.changes().len(), 1);
    }

    #[test]
    fn test_atomic_flags() {
        let flags = AtomicFlags::NONBLOCK | AtomicFlags::PAGE_FLIP_EVENT;
        assert!(flags.contains(AtomicFlags::NONBLOCK));
        assert!(!flags.contains(AtomicFlags::ALLOW_MODESET));
    }

    // ---- t25-e1: typed atomic ids + pure encoding helper regressions ----

    #[test]
    fn encode_atomic_empty_yields_empty_buffers() {
        let encoded = encode_atomic_request(&[]);
        assert!(encoded.objs.is_empty());
        assert!(encoded.count_props.is_empty());
        assert!(encoded.props.is_empty());
        assert!(encoded.prop_values.is_empty());
    }

    #[test]
    fn encode_atomic_single_change_groups_one_object() {
        let changes = vec![PropertyChange {
            object_id: ObjectId(7),
            property_id: PropertyId(13),
            value: 42,
        }];
        let encoded = encode_atomic_request(&changes);
        assert_eq!(encoded.objs, vec![7]);
        assert_eq!(encoded.count_props, vec![1]);
        assert_eq!(encoded.props, vec![13]);
        assert_eq!(encoded.prop_values, vec![42]);
    }

    #[test]
    fn encode_atomic_groups_by_first_appearance_with_interleaving() {
        let changes = vec![
            PropertyChange {
                object_id: ObjectId(1),
                property_id: PropertyId(10),
                value: 100,
            },
            PropertyChange {
                object_id: ObjectId(2),
                property_id: PropertyId(20),
                value: 200,
            },
            PropertyChange {
                object_id: ObjectId(1),
                property_id: PropertyId(11),
                value: 101,
            },
            PropertyChange {
                object_id: ObjectId(2),
                property_id: PropertyId(21),
                value: 201,
            },
            PropertyChange {
                object_id: ObjectId(3),
                property_id: PropertyId(30),
                value: 300,
            },
        ];
        let encoded = encode_atomic_request(&changes);
        assert_eq!(encoded.objs, vec![1, 2, 3]);
        assert_eq!(encoded.count_props, vec![2, 2, 1]);
        assert_eq!(encoded.props, vec![10, 11, 20, 21, 30]);
        assert_eq!(encoded.prop_values, vec![100, 101, 200, 201, 300]);
    }

    #[test]
    fn encode_atomic_preserves_duplicate_property_ordering() {
        let changes = vec![
            PropertyChange {
                object_id: ObjectId(5),
                property_id: PropertyId(9),
                value: 1,
            },
            PropertyChange {
                object_id: ObjectId(5),
                property_id: PropertyId(9),
                value: 2,
            },
            PropertyChange {
                object_id: ObjectId(5),
                property_id: PropertyId(9),
                value: 3,
            },
        ];
        let encoded = encode_atomic_request(&changes);
        assert_eq!(encoded.objs, vec![5]);
        assert_eq!(encoded.count_props, vec![3]);
        assert_eq!(encoded.props, vec![9, 9, 9]);
        assert_eq!(encoded.prop_values, vec![1, 2, 3]);
    }

    // ---- t28-e1: atomic ioctl arg materializer regressions -----------

    #[test]
    fn atomic_args_pointer_fidelity() {
        let changes = vec![
            PropertyChange {
                object_id: ObjectId(1),
                property_id: PropertyId(10),
                value: 100,
            },
            PropertyChange {
                object_id: ObjectId(2),
                property_id: PropertyId(20),
                value: 200,
            },
            PropertyChange {
                object_id: ObjectId(1),
                property_id: PropertyId(11),
                value: 101,
            },
        ];
        let encoded = encode_atomic_request(&changes);
        let (args, arrays) = atomic_args_from_encoded(&encoded, AtomicFlags::empty(), 0);
        assert_eq!(args.objs_ptr, arrays.objs.as_ptr() as u64);
        assert_eq!(args.count_props_ptr, arrays.count_props.as_ptr() as u64);
        assert_eq!(args.props_ptr, arrays.props.as_ptr() as u64);
        assert_eq!(args.prop_values_ptr, arrays.prop_values.as_ptr() as u64);
        assert_eq!(args.count_objs, arrays.objs.len() as u32);
        assert_eq!(arrays.objs, encoded.objs);
        assert_eq!(arrays.count_props, encoded.count_props);
        assert_eq!(arrays.props, encoded.props);
        assert_eq!(arrays.prop_values, encoded.prop_values);
    }

    #[test]
    fn atomic_args_flags_and_user_data() {
        let changes = vec![PropertyChange {
            object_id: ObjectId(7),
            property_id: PropertyId(13),
            value: 42,
        }];
        let encoded = encode_atomic_request(&changes);
        let flags = AtomicFlags::NONBLOCK | AtomicFlags::PAGE_FLIP_EVENT;
        let (args, _arrays) = atomic_args_from_encoded(&encoded, flags, 0xCAFEBABE);
        assert_eq!(
            args.flags,
            (AtomicFlags::NONBLOCK | AtomicFlags::PAGE_FLIP_EVENT).bits()
        );
        assert_eq!(args.user_data, 0xCAFEBABE);
        assert_eq!(args.reserved, 0);
    }

    #[test]
    fn atomic_args_empty_request_yields_null_safe_zero_count() {
        let encoded = EncodedAtomicRequest {
            objs: Vec::new(),
            count_props: Vec::new(),
            props: Vec::new(),
            prop_values: Vec::new(),
        };
        let (args, arrays) = atomic_args_from_encoded(&encoded, AtomicFlags::empty(), 0);
        assert_eq!(args.count_objs, 0);
        // Vec::as_ptr on empty Vec returns a non-null dangling pointer; just
        // assert the args pointer fields agree with the owned storage and
        // that the count is zero so the kernel never dereferences them.
        assert_eq!(args.objs_ptr, arrays.objs.as_ptr() as u64);
        assert_eq!(args.count_props_ptr, arrays.count_props.as_ptr() as u64);
        assert_eq!(args.props_ptr, arrays.props.as_ptr() as u64);
        assert_eq!(args.prop_values_ptr, arrays.prop_values.as_ptr() as u64);
        assert_eq!(args.flags, 0);
        assert_eq!(args.user_data, 0);
        assert_eq!(args.reserved, 0);
    }

    #[test]
    fn test_pageflip_flags() {
        let flags = PageFlipFlags::EVENT;
        assert!(flags.contains(PageFlipFlags::EVENT));
    }

    // ---- t24-e1: typed page-flip request surface regressions ----------

    #[test]
    fn page_flip_flags_bitor_combines_event_and_async() {
        let combined = PageFlipFlags::EVENT | PageFlipFlags::ASYNC;
        assert_eq!(combined.bits(), 0x03);
        assert!(combined.contains(PageFlipFlags::EVENT));
        assert!(combined.contains(PageFlipFlags::ASYNC));
        // Kernel uapi value pinning.
        assert_eq!(PageFlipFlags::EVENT.bits(), 0x01);
        assert_eq!(PageFlipFlags::ASYNC.bits(), 0x02);
    }

    #[test]
    fn page_flip_request_args_translates_typed_inputs() {
        let args = page_flip_request_args(
            CrtcId(7),
            FramebufferId(42),
            PageFlipFlags::EVENT | PageFlipFlags::ASYNC,
            0xDEAD_BEEF,
        );
        assert_eq!(args.crtc_id, 7);
        assert_eq!(args.fb_id, 42);
        assert_eq!(args.flags, 0x03);
        assert_eq!(args.reserved, 0);
        assert_eq!(args.user_data, 0xDEAD_BEEF);
    }

    #[test]
    fn page_flip_request_args_zero_flags_when_unset() {
        let args = page_flip_request_args(
            CrtcId(0),
            FramebufferId(0),
            PageFlipFlags::empty(),
            0,
        );
        assert_eq!(args.flags, 0);
        assert_eq!(args.crtc_id, 0);
        assert_eq!(args.fb_id, 0);
        assert_eq!(args.reserved, 0);
        assert_eq!(args.user_data, 0);
    }

    // ---- t39-e1: PresentRequest typed value object regressions --------

    #[test]
    fn present_request_new_packs_fields() {
        let req = PresentRequest::new(
            CrtcId(7),
            FramebufferId(42),
            PageFlipFlags::EVENT,
            0xDEAD_BEEF,
        );
        assert_eq!(req.crtc, CrtcId(7));
        assert_eq!(req.fb, FramebufferId(42));
        assert_eq!(req.flags, PageFlipFlags::EVENT);
        assert_eq!(req.user_data, 0xDEAD_BEEF);
    }

    #[test]
    fn present_request_with_flags_replaces_only_flags() {
        let original = PresentRequest::new(
            CrtcId(3),
            FramebufferId(11),
            PageFlipFlags::EVENT,
            0xAA,
        );
        let updated = original.with_flags(PageFlipFlags::EVENT | PageFlipFlags::ASYNC);
        assert_eq!(updated.flags, PageFlipFlags::EVENT | PageFlipFlags::ASYNC);
        assert_eq!(updated.crtc, original.crtc);
        assert_eq!(updated.fb, original.fb);
        assert_eq!(updated.user_data, original.user_data);
    }

    #[test]
    fn present_request_with_user_data_replaces_only_user_data() {
        let original = PresentRequest::new(
            CrtcId(3),
            FramebufferId(11),
            PageFlipFlags::EVENT,
            0xAA,
        );
        let updated = original.with_user_data(0xCAFE_F00D);
        assert_eq!(updated.user_data, 0xCAFE_F00D);
        assert_eq!(updated.crtc, original.crtc);
        assert_eq!(updated.fb, original.fb);
        assert_eq!(updated.flags, original.flags);
    }

    #[test]
    fn present_request_round_trips_through_translation_helper() {
        let req = PresentRequest::new(
            CrtcId(9),
            FramebufferId(123),
            PageFlipFlags::EVENT | PageFlipFlags::ASYNC,
            0x1234_5678_9ABC_DEF0,
        );
        let args = page_flip_request_args(req.crtc, req.fb, req.flags, req.user_data);
        assert_eq!(args.crtc_id, 9);
        assert_eq!(args.fb_id, 123);
        assert_eq!(args.flags, 0x03);
        assert_eq!(args.user_data, 0x1234_5678_9ABC_DEF0);
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

    #[test]
    fn encoder_type_translation_round_trips_known_kinds() {
        assert_eq!(encoder_type_from_raw(DRM_MODE_ENCODER_NONE), EncoderType::None);
        assert_eq!(encoder_type_from_raw(DRM_MODE_ENCODER_DAC), EncoderType::DAC);
        assert_eq!(encoder_type_from_raw(DRM_MODE_ENCODER_TMDS), EncoderType::TMDS);
        assert_eq!(encoder_type_from_raw(DRM_MODE_ENCODER_LVDS), EncoderType::LVDS);
        assert_eq!(encoder_type_from_raw(DRM_MODE_ENCODER_TVDAC), EncoderType::TVDAC);
        assert_eq!(
            encoder_type_from_raw(DRM_MODE_ENCODER_VIRTUAL),
            EncoderType::Virtual
        );
        assert_eq!(encoder_type_from_raw(DRM_MODE_ENCODER_DSI), EncoderType::DSI);
        assert_eq!(encoder_type_from_raw(DRM_MODE_ENCODER_DPMST), EncoderType::DPMST);
        assert_eq!(encoder_type_from_raw(DRM_MODE_ENCODER_DPI), EncoderType::DPI);
        assert_eq!(encoder_type_from_raw(0xDEAD_BEEF), EncoderType::Unknown(0xDEAD_BEEF));
    }

    #[test]
    fn select_crtc_for_connector_prefers_live_encoder_attachment() {
        let connector = make_test_connector(Some(7));
        let encoders = vec![
            EncoderInfo {
                id: EncoderId(7),
                encoder_type: EncoderType::TMDS,
                crtc_id: Some(CrtcId(20)),
                possible_crtcs: 0b111,
                possible_clones: 0,
            },
            EncoderInfo {
                id: EncoderId(8),
                encoder_type: EncoderType::DAC,
                crtc_id: None,
                possible_crtcs: 0b001,
                possible_clones: 0,
            },
        ];
        let crtcs = vec![
            crtc_with_id(10),
            crtc_with_id(20),
            crtc_with_id(30),
        ];

        assert_eq!(
            select_crtc_for_connector(&connector, &encoders, &crtcs),
            Some(CrtcId(20))
        );
    }

    #[test]
    fn select_crtc_for_connector_falls_back_to_possible_crtcs_mask() {
        let connector = make_test_connector(Some(7));
        // Live attachment points at a CRTC not in our enumeration.
        let encoders = vec![EncoderInfo {
            id: EncoderId(7),
            encoder_type: EncoderType::TMDS,
            crtc_id: Some(CrtcId(99)),
            // bits 0 and 2 → indices 0 and 2 in `crtcs`.
            possible_crtcs: 0b101,
            possible_clones: 0,
        }];
        let crtcs = vec![
            crtc_with_id(10),
            crtc_with_id(20),
            crtc_with_id(30),
        ];

        // Lowest set bit (index 0) wins → crtc id 10.
        assert_eq!(
            select_crtc_for_connector(&connector, &encoders, &crtcs),
            Some(CrtcId(10))
        );

        // With bit 0 cleared, the next set bit (index 2) wins.
        let encoders_higher = vec![EncoderInfo {
            possible_crtcs: 0b100,
            ..encoders[0].clone()
        }];
        assert_eq!(
            select_crtc_for_connector(&connector, &encoders_higher, &crtcs),
            Some(CrtcId(30))
        );
    }

    #[test]
    fn select_crtc_for_connector_returns_none_when_no_match() {
        let connector = make_test_connector(Some(7));
        let encoders = vec![EncoderInfo {
            id: EncoderId(7),
            encoder_type: EncoderType::TMDS,
            crtc_id: None,
            possible_crtcs: 0,
            possible_clones: 0,
        }];
        let crtcs = vec![crtc_with_id(10), crtc_with_id(20)];

        assert_eq!(select_crtc_for_connector(&connector, &encoders, &crtcs), None);

        // Empty crtc list also yields None even when an encoder declares possibilities.
        let plenty = vec![EncoderInfo {
            possible_crtcs: 0xFFFF_FFFF,
            ..encoders[0].clone()
        }];
        assert_eq!(select_crtc_for_connector(&connector, &plenty, &[]), None);

        // Connector with no live encoder attachment and no encoders also yields None.
        let detached = make_test_connector(None);
        assert_eq!(select_crtc_for_connector(&detached, &[], &crtcs), None);
    }

    #[test]
    fn crtc_info_mode_translation_uses_raw_mode_info() {
        // A CRTC populated by `DRM_IOCTL_MODE_GETCRTC` translates its
        // `drm_mode_modeinfo` payload through the same `from_raw_mode_info`
        // helper used for connector mode lists. This regression pins that
        // translation contract, so the kernel-side ioctl wrapper stays
        // host-testable as a pure data shape.
        let mut raw = RawDrmModeInfo {
            clock: 148_500,
            hdisplay: 1920,
            htotal: 2200,
            vdisplay: 1080,
            vtotal: 1125,
            vrefresh: 60,
            mode_type: 1 << 1, // DRM_MODE_TYPE_CLOCK_C → CURRENT
            ..Default::default()
        };
        write_mode_name(&mut raw.name, b"1920x1080");

        let translated = from_raw_mode_info(&raw).expect("raw mode should translate");
        let crtc = CrtcInfo {
            id: CrtcId(42),
            x: 0,
            y: 0,
            width: u32::from(raw.hdisplay),
            height: u32::from(raw.vdisplay),
            mode: Some(translated.clone()),
            connector_id: None,
        };

        let mode = crtc.mode.as_ref().expect("translated mode should be present");
        assert_eq!(mode.width, 1920);
        assert_eq!(mode.height, 1080);
        assert_eq!(mode.refresh_hz, 60);
        assert!(mode.is_current());
        assert_eq!(mode.name, "1920x1080");
        assert_eq!(crtc.width, 1920);
        assert_eq!(crtc.height, 1080);

        // mode_valid == 0 path: synthesise the same shape `enumerate_crtc`
        // produces when the kernel reports no current mode.
        let blank_crtc = CrtcInfo {
            id: CrtcId(43),
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            mode: None,
            connector_id: None,
        };
        assert!(blank_crtc.mode.is_none());
    }

    fn make_test_connector(encoder_id: Option<u32>) -> ConnectorInfo {
        ConnectorInfo {
            id: ConnectorId(11),
            connector_type: ConnectorType::HDMI,
            connector_type_id: 1,
            name: "HDMI-A-1".to_string(),
            status: ConnectorStatus::Connected,
            modes: Vec::new(),
            physical_width_mm: 0,
            physical_height_mm: 0,
            subpixel_order: SubpixelOrder::Unknown,
            encoder_id,
        }
    }

    fn crtc_with_id(id: u32) -> CrtcInfo {
        CrtcInfo {
            id: CrtcId(id),
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            mode: None,
            connector_id: None,
        }
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

    // ---- t23-e1: typed framebuffer shape regressions ------------------

    use crate::framebuffer::{
        DROP_RECORDER, DrmFramebuffer, DumbBuffer, Fourcc, FramebufferId, add_fb2_args,
        create_dumb_buffer_args,
    };

    /// XR24 / DRM_FORMAT_XRGB8888 fourcc (`'X','R','2','4'`).
    const FOURCC_XRGB8888: Fourcc = Fourcc::XRGB8888;

    fn fixture_dumb_buffer(handle: u32, pitch: u32, width: u32, height: u32) -> DumbBuffer {
        DumbBuffer {
            handle,
            pitch,
            size: u64::from(pitch) * u64::from(height),
            width,
            height,
            bpp: 32,
            device_fd: -1,
        }
    }

    fn clear_drop_recorder() {
        DROP_RECORDER.with(|r| r.borrow_mut().clear());
    }

    #[test]
    fn framebuffer_id_is_distinct_newtype() {
        let a = FramebufferId(7);
        let b = FramebufferId(7);
        let c = FramebufferId(8);
        let copy = a;
        assert_eq!(a, b);
        assert_eq!(a, copy);
        assert_ne!(a, c);
        assert_eq!(a.0, 7);
        // Hash/Eq trait surface — newtype must work as a HashMap key.
        let mut set = std::collections::HashSet::new();
        set.insert(a);
        assert!(set.contains(&b));
        assert!(!set.contains(&c));
    }

    #[test]
    fn create_dumb_args_translation() {
        let args = create_dumb_buffer_args(1920, 1080, 32);
        assert_eq!(args.width, 1920);
        assert_eq!(args.height, 1080);
        assert_eq!(args.bpp, 32);
        assert_eq!(args.flags, 0);
        assert_eq!(args.handle, 0);
        assert_eq!(args.pitch, 0);
        assert_eq!(args.size, 0);
    }

    #[test]
    fn add_fb2_args_translation() {
        let dumb = fixture_dumb_buffer(0xAB, 7680, 1920, 1080);
        let cmd = add_fb2_args(&dumb, FOURCC_XRGB8888, 0);

        assert_eq!(cmd.fb_id, 0);
        assert_eq!(cmd.width, 1920);
        assert_eq!(cmd.height, 1080);
        assert_eq!(cmd.pixel_format, FOURCC_XRGB8888.0);
        assert_eq!(cmd.flags, 0);

        assert_eq!(cmd.handles[0], 0xAB);
        assert_eq!(cmd.pitches[0], 7680);
        assert_eq!(cmd.offsets[0], 0);
        assert_eq!(cmd.modifier[0], 0);

        for slot in 1..4 {
            assert_eq!(cmd.handles[slot], 0, "handles[{}] must be zero", slot);
            assert_eq!(cmd.pitches[slot], 0, "pitches[{}] must be zero", slot);
            assert_eq!(cmd.offsets[slot], 0, "offsets[{}] must be zero", slot);
            assert_eq!(cmd.modifier[slot], 0, "modifier[{}] must be zero", slot);
        }

        // Drop the placeholder fixture cleanly.
        drop(dumb);
    }

    #[test]
    fn dumb_buffer_drop_is_noop_on_host() {
        clear_drop_recorder();
        {
            let _dumb = fixture_dumb_buffer(1, 4096, 1024, 768);
        }
        // Drop must not panic. On Windows host the Linux ioctl stub is not
        // even compiled in; we just record the drop tag.
        let order = DROP_RECORDER.with(|r| r.borrow().clone());
        assert_eq!(order, vec!["dumb"]);
    }

    #[test]
    fn framebuffer_drop_releases_dumb_first() {
        clear_drop_recorder();
        {
            let dumb = fixture_dumb_buffer(0x42, 7680, 1920, 1080);
            let _fb = DrmFramebuffer {
                id: FramebufferId(99),
                width: 1920,
                height: 1080,
                pixel_format: FOURCC_XRGB8888,
                dumb,
            };
        }
        // The outer `DrmFramebuffer::drop` body runs first (recording "fb"),
        // and only afterwards the inner `DumbBuffer` field is dropped
        // (recording "dumb"). This is the FB-released-before-dumb-destroy
        // ordering the production Linux Drop relies on.
        let order = DROP_RECORDER.with(|r| r.borrow().clone());
        assert_eq!(order, vec!["fb", "dumb"]);
    }

    // ---- t29-e1: Fourcc newtype regressions ---------------------------

    #[test]
    fn fourcc_xrgb8888_constant_value() {
        assert_eq!(Fourcc::XRGB8888.0, 0x34325258);
    }

    #[test]
    fn fourcc_named_constants_match_le_bytes() {
        assert_eq!(Fourcc::XRGB8888, Fourcc::from_bytes([b'X', b'R', b'2', b'4']));
        assert_eq!(Fourcc::ARGB8888, Fourcc::from_bytes([b'A', b'R', b'2', b'4']));
        assert_eq!(Fourcc::XBGR8888, Fourcc::from_bytes([b'X', b'B', b'2', b'4']));
        assert_eq!(Fourcc::ABGR8888, Fourcc::from_bytes([b'A', b'B', b'2', b'4']));
    }

    #[test]
    fn fourcc_distinct_named_constants() {
        let all = [
            Fourcc::XRGB8888,
            Fourcc::ARGB8888,
            Fourcc::XBGR8888,
            Fourcc::ABGR8888,
        ];
        for i in 0..all.len() {
            for j in (i + 1)..all.len() {
                assert_ne!(all[i], all[j], "fourcc {} and {} must differ", i, j);
            }
        }
    }

    #[test]
    fn add_fb2_args_translates_typed_fourcc() {
        let dumb = fixture_dumb_buffer(0xCD, 7680, 1920, 1080);
        let cmd = add_fb2_args(&dumb, Fourcc::XRGB8888, 0);
        assert_eq!(cmd.pixel_format, 0x34325258);
        drop(dumb);
    }

    // ---- t27-e1: typed plane enumeration shape regressions ------------

    use crate::device::DrmDevice;
    use crate::error::Result as DrmResult;
    use crate::plane::{PlaneId, PlaneInfo, PlaneType, enumerate_planes};

    #[test]
    fn plane_id_newtype_round_trips() {
        let a = PlaneId(7);
        let b = PlaneId(7);
        let c = PlaneId(8);
        let copy = a;
        assert_eq!(a, b);
        assert_eq!(a, copy);
        assert_ne!(a, c);
        assert_eq!(a.0, 7);
        // Hash/Eq trait surface — newtype must work as a HashSet key.
        let mut set = std::collections::HashSet::new();
        set.insert(a);
        assert!(set.contains(&b));
        assert!(!set.contains(&c));
    }

    #[test]
    fn plane_type_variants_distinct() {
        let variants = [
            PlaneType::Primary,
            PlaneType::Cursor,
            PlaneType::Overlay,
            PlaneType::Unknown,
        ];
        for (i, a) in variants.iter().enumerate() {
            for (j, b) in variants.iter().enumerate() {
                if i == j {
                    assert_eq!(a, b);
                } else {
                    assert_ne!(a, b, "{:?} must differ from {:?}", a, b);
                }
            }
        }
        // from_caps mapping mirrors kernel DRM_PLANE_TYPE_* values.
        assert_eq!(PlaneType::from_caps(0), PlaneType::Overlay);
        assert_eq!(PlaneType::from_caps(1), PlaneType::Primary);
        assert_eq!(PlaneType::from_caps(2), PlaneType::Cursor);
        assert_eq!(PlaneType::from_caps(99), PlaneType::Unknown);
    }

    #[test]
    fn enumerate_planes_returns_empty_on_non_linux_or_no_device() {
        // Compile-time wiring: confirm `enumerate_planes` carries the
        // expected typed signature regardless of platform.
        let _: fn(&DrmDevice) -> DrmResult<Vec<PlaneInfo>> = enumerate_planes;

        #[cfg(not(target_os = "linux"))]
        {
            // On non-Linux, `DrmDevice::open` returns NoDevice — which is the
            // outer invariant guarding the trivial Ok(Vec::new()) stub.
            assert!(matches!(DrmDevice::open("/dev/null"), Err(DrmError::NoDevice)));
        }
        #[cfg(target_os = "linux")]
        {
            // On Linux, opening a path that cannot exist must surface as an
            // error before reaching enumerate_planes.
            assert!(DrmDevice::open("/this/path/does/not/exist/liquide").is_err());
        }
    }

    // ---- t37-e1: DrmResources snapshot lookup regressions -------------

    fn synthetic_connector(id: u32) -> ConnectorInfo {
        ConnectorInfo {
            id: ConnectorId(id),
            connector_type: ConnectorType::HDMI,
            connector_type_id: 1,
            name: format!("HDMI-{}", id),
            status: ConnectorStatus::Connected,
            modes: Vec::new(),
            physical_width_mm: 0,
            physical_height_mm: 0,
            subpixel_order: SubpixelOrder::Unknown,
            encoder_id: None,
        }
    }

    fn synthetic_encoder(id: u32) -> EncoderInfo {
        EncoderInfo {
            id: EncoderId(id),
            encoder_type: EncoderType::TMDS,
            crtc_id: None,
            possible_crtcs: 0,
            possible_clones: 0,
        }
    }

    fn synthetic_crtc(id: u32) -> CrtcInfo {
        CrtcInfo {
            id: CrtcId(id),
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            mode: None,
            connector_id: None,
        }
    }

    fn synthetic_plane(id: u32) -> PlaneInfo {
        PlaneInfo {
            id: PlaneId(id),
            possible_crtcs: 0,
            formats: Vec::new(),
        }
    }

    fn synthetic_resources() -> crate::resources::DrmResources {
        crate::resources::DrmResources {
            connectors: vec![synthetic_connector(11), synthetic_connector(22)],
            encoders: vec![synthetic_encoder(33), synthetic_encoder(44)],
            crtcs: vec![synthetic_crtc(55), synthetic_crtc(66)],
            planes: vec![synthetic_plane(77), synthetic_plane(88)],
        }
    }

    #[test]
    fn drm_resources_lookup_connector_by_id() {
        let r = synthetic_resources();
        assert_eq!(r.connector(ConnectorId(11)).map(|c| c.id), Some(ConnectorId(11)));
        assert_eq!(r.connector(ConnectorId(22)).map(|c| c.id), Some(ConnectorId(22)));
        assert!(r.connector(ConnectorId(999)).is_none());
    }

    #[test]
    fn drm_resources_lookup_encoder_by_id() {
        let r = synthetic_resources();
        assert_eq!(r.encoder(EncoderId(33)).map(|e| e.id), Some(EncoderId(33)));
        assert_eq!(r.encoder(EncoderId(44)).map(|e| e.id), Some(EncoderId(44)));
        assert!(r.encoder(EncoderId(999)).is_none());
    }

    #[test]
    fn drm_resources_lookup_crtc_by_id() {
        let r = synthetic_resources();
        assert_eq!(r.crtc(CrtcId(55)).map(|c| c.id), Some(CrtcId(55)));
        assert_eq!(r.crtc(CrtcId(66)).map(|c| c.id), Some(CrtcId(66)));
        assert!(r.crtc(CrtcId(999)).is_none());
    }

    #[test]
    fn drm_resources_lookup_plane_by_id() {
        let r = synthetic_resources();
        assert_eq!(r.plane(PlaneId(77)).map(|p| p.id), Some(PlaneId(77)));
        assert_eq!(r.plane(PlaneId(88)).map(|p| p.id), Some(PlaneId(88)));
        assert!(r.plane(PlaneId(999)).is_none());
    }

    fn make_mode(w: u32, h: u32, hz: u32) -> DrmMode {
        DrmMode {
            width: w,
            height: h,
            refresh_hz: hz,
            clock_khz: 0,
            flags: ModeFlags::empty(),
            name: format!("{w}x{h}@{hz}"),
        }
    }

    #[test]
    fn match_mode_by_dimensions_finds_exact_match() {
        let modes = vec![
            make_mode(1920, 1080, 60),
            make_mode(3840, 2160, 60),
            make_mode(1280, 720, 60),
        ];
        let m = match_mode_by_dimensions(&modes, 3840, 2160).expect("match");
        assert_eq!((m.width, m.height), (3840, 2160));
    }

    #[test]
    fn match_mode_by_dimensions_returns_none_when_absent() {
        let modes = vec![
            make_mode(1920, 1080, 60),
            make_mode(3840, 2160, 60),
            make_mode(1280, 720, 60),
        ];
        assert!(match_mode_by_dimensions(&modes, 800, 600).is_none());
    }

    #[test]
    fn match_mode_by_dimensions_skips_unusable_zero_sized() {
        let modes = vec![make_mode(0, 0, 60)];
        assert!(!modes[0].is_usable());
        assert!(match_mode_by_dimensions(&modes, 0, 0).is_none());
    }

    #[test]
    fn highest_resolution_mode_picks_largest_area() {
        let modes = vec![
            make_mode(1920, 1080, 60),
            make_mode(3840, 2160, 60),
            make_mode(1280, 720, 60),
        ];
        let m = highest_resolution_mode(&modes).expect("some");
        assert_eq!((m.width, m.height), (3840, 2160));
    }

    #[test]
    fn highest_resolution_mode_breaks_ties_by_refresh() {
        let modes = vec![make_mode(1920, 1080, 60), make_mode(1920, 1080, 144)];
        let m = highest_resolution_mode(&modes).expect("some");
        assert_eq!(m.refresh_hz, 144);
    }

    #[test]
    fn closest_refresh_mode_picks_nearest() {
        let modes = vec![
            make_mode(1920, 1080, 30),
            make_mode(1920, 1080, 60),
            make_mode(1920, 1080, 144),
        ];
        let m = closest_refresh_mode(&modes, 90).expect("some");
        assert_eq!(m.refresh_hz, 60);
    }

    // -------------------------------------------------------------------
    // Mock ioctl dispatch (host-safe regressions for the t40 mock layer).
    // -------------------------------------------------------------------

    #[repr(C)]
    struct DummyArg {
        magic: u32,
    }

    #[test]
    fn mock_ioctl_handler_intercepts_call() {
        use crate::ioctl::{drm_ioctl, mock};
        use std::sync::{Arc, Mutex};

        let captured: Arc<Mutex<Option<(i32, core::ffi::c_ulong, String)>>> =
            Arc::new(Mutex::new(None));
        let captured_clone = Arc::clone(&captured);
        let _guard = mock::install_scoped(move |call| {
            *captured_clone.lock().unwrap() =
                Some((call.fd, call.request, call.name.clone()));
            Ok(())
        });

        let mut arg = DummyArg { magic: 0 };
        let result = drm_ioctl(7, 0xABCD_u64 as core::ffi::c_ulong, "TEST_REQ", &mut arg);
        assert!(result.is_ok(), "expected handler to return Ok, got {result:?}");
        let captured = captured.lock().unwrap().clone().expect("handler should run");
        assert_eq!(captured.0, 7);
        assert_eq!(captured.1, 0xABCD_u64 as core::ffi::c_ulong);
        assert_eq!(captured.2, "TEST_REQ");
    }

    #[test]
    fn mock_ioctl_handler_writes_response_into_arg() {
        use crate::ioctl::{drm_ioctl, mock};

        let _guard = mock::install_scoped(|call| {
            // SAFETY: tests pass a `&mut DummyArg` whose first/only field is `u32`.
            unsafe {
                let p = call.arg as *mut u32;
                *p = 0xDEAD_BEEF;
            }
            Ok(())
        });

        let mut arg = DummyArg { magic: 0 };
        drm_ioctl(0, 0, "WRITE_RESP", &mut arg).expect("handler ok");
        assert_eq!(arg.magic, 0xDEAD_BEEF);
    }

    #[test]
    fn mock_ioctl_handler_can_return_err() {
        use crate::ioctl::{drm_ioctl, mock};

        let _guard = mock::install_scoped(|call| {
            Err(DrmError::Ioctl {
                name: call.name,
                reason: "synthetic kernel failure".to_string(),
            })
        });

        let mut arg = DummyArg { magic: 0 };
        let err = drm_ioctl(0, 0, "FAILING_REQ", &mut arg).expect_err("should error");
        match err {
            DrmError::Ioctl { name, reason } => {
                assert_eq!(name, "FAILING_REQ");
                assert_eq!(reason, "synthetic kernel failure");
            }
            other => panic!("expected DrmError::Ioctl, got {other:?}"),
        }
    }

    #[test]
    fn mock_ioctl_handler_clears_on_drop() {
        use crate::ioctl::{drm_ioctl, mock};

        {
            let _guard = mock::install_scoped(|_call| Ok(()));
            let mut arg = DummyArg { magic: 0 };
            drm_ioctl(1, 1, "WHILE_INSTALLED", &mut arg).expect("ok while installed");
        }

        let mut arg = DummyArg { magic: 0 };
        let result = drm_ioctl(1, 1, "AFTER_DROP", &mut arg);

        // On Windows host (and any non-Linux test build), the fallback path
        // returns the synthetic "no mock ioctl handler installed" error. On a
        // Linux test build, it would attempt a real `libc::ioctl(1, ...)` —
        // which will fail because fd=1 is stdout, not a DRM device. Either
        // way the call must return `DrmError::Ioctl{ name: "AFTER_DROP", .. }`.
        let err = result.expect_err("fallback path should error after guard dropped");
        match err {
            DrmError::Ioctl { name, reason } => {
                assert_eq!(name, "AFTER_DROP");
                #[cfg(not(target_os = "linux"))]
                assert_eq!(reason, "no mock ioctl handler installed");
                #[cfg(target_os = "linux")]
                let _ = reason; // libc-driven message is environment-dependent.
            }
            other => panic!("expected DrmError::Ioctl, got {other:?}"),
        }
    }

    // -------------------------------------------------------------------
    // t31-e1: CREATE_DUMB / DESTROY_DUMB ioctl wiring regressions.
    // -------------------------------------------------------------------
    //
    // These run on any host (incl. Windows) by routing the real
    // `crate::ioctl::drm_ioctl` calls inside `allocate_dumb_buffer_via_fd`
    // and `destroy_dumb_via_ioctl` through the t40 mock dispatch layer.

    /// `DRM_IOWR(0xB2, sizeof(drm_mode_create_dumb))` — must match the
    /// constant in `framebuffer.rs`. Recomputed here so a regression in
    /// either the encoding helper or the literal `0xB2` is caught.
    fn expected_create_dumb_request() -> core::ffi::c_ulong {
        crate::ioctl::drm_iowr(
            0xB2,
            std::mem::size_of::<crate::framebuffer::DrmModeCreateDumb>(),
        )
    }

    /// `DRM_IOWR(0xB4, sizeof(drm_mode_destroy_dumb))`.
    fn expected_destroy_dumb_request() -> core::ffi::c_ulong {
        crate::ioctl::drm_iowr(
            0xB4,
            std::mem::size_of::<crate::framebuffer::DrmModeDestroyDumb>(),
        )
    }

    #[test]
    fn create_dumb_invokes_ioctl_with_correct_request() {
        use crate::framebuffer::allocate_dumb_buffer_via_fd;
        use crate::ioctl::mock;
        use std::sync::{Arc, Mutex};

        let captured: Arc<Mutex<Option<(i32, core::ffi::c_ulong, String)>>> =
            Arc::new(Mutex::new(None));
        let captured_clone = Arc::clone(&captured);
        let _guard = mock::install_scoped(move |call| {
            *captured_clone.lock().unwrap() =
                Some((call.fd, call.request, call.name.clone()));
            Ok(())
        });

        let buf =
            allocate_dumb_buffer_via_fd(42, 1920, 1080, 32).expect("mock returns Ok");
        // Drop happens at end of test; no DESTROY_DUMB capture in this guard
        // is fine because the same handler accepts and ignores it.
        drop(buf);

        let captured = captured.lock().unwrap().clone().expect("handler ran");
        assert_eq!(captured.0, 42, "fd must be the sentinel passed in");
        assert_eq!(
            captured.1,
            expected_create_dumb_request(),
            "request must be DRM_IOCTL_MODE_CREATE_DUMB"
        );
        // The captured name reflects the *last* ioctl observed by the
        // handler. The DumbBuffer drop above issues a DESTROY_DUMB last,
        // so capture only the CREATE_DUMB call by capturing only on first
        // sight. Re-run the assertion on a fresh capture that records only
        // the create call:
        let _ = captured.2;

        // Fresh, single-shot capture for name assertion.
        let name_capture: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let name_clone = Arc::clone(&name_capture);
        let _guard2 = mock::install_scoped(move |call| {
            let mut slot = name_clone.lock().unwrap();
            if slot.is_none() {
                *slot = Some(call.name.clone());
            }
            Ok(())
        });
        let _b =
            allocate_dumb_buffer_via_fd(42, 1920, 1080, 32).expect("mock returns Ok");
        assert_eq!(
            name_capture.lock().unwrap().as_deref(),
            Some("MODE_CREATE_DUMB"),
        );
    }

    #[test]
    fn create_dumb_populates_handle_pitch_size_from_kernel_response() {
        use crate::framebuffer::{DrmModeCreateDumb, allocate_dumb_buffer_via_fd};
        use crate::ioctl::mock;

        let _guard = mock::install_scoped(|call| {
            assert_eq!(call.name, "MODE_CREATE_DUMB");
            // SAFETY: caller's arg is a `&mut DrmModeCreateDumb`. We re-cast
            // the raw `*mut u8` back to that `#[repr(C)]` type and write a
            // synthetic kernel response into the output fields. Input
            // fields (`width`, `height`, `bpp`) are preserved.
            unsafe {
                let p = call.arg as *mut DrmModeCreateDumb;
                (*p).handle = 7;
                (*p).pitch = 8192;
                (*p).size = 8192u64 * 1080;
            }
            Ok(())
        });

        let buf =
            allocate_dumb_buffer_via_fd(42, 1920, 1080, 32).expect("mock ok");
        assert_eq!(buf.handle, 7);
        assert_eq!(buf.pitch, 8192);
        assert_eq!(buf.size, 8192u64 * 1080);
        assert_eq!(buf.width, 1920);
        assert_eq!(buf.height, 1080);
        assert_eq!(buf.bpp, 32);
        assert_eq!(buf.device_fd, 42);
    }

    #[test]
    fn destroy_dumb_invokes_ioctl_with_correct_handle() {
        use crate::framebuffer::{DrmModeDestroyDumb, destroy_dumb_via_ioctl};
        use crate::ioctl::mock;
        use std::sync::{Arc, Mutex};

        #[derive(Default)]
        struct Capture {
            fd: Option<i32>,
            request: Option<core::ffi::c_ulong>,
            name: Option<String>,
            handle: Option<u32>,
        }
        let captured: Arc<Mutex<Capture>> = Arc::new(Mutex::new(Capture::default()));
        let captured_clone = Arc::clone(&captured);
        let _guard = mock::install_scoped(move |call| {
            // SAFETY: caller passes `&mut DrmModeDestroyDumb`.
            let handle = unsafe {
                let p = call.arg as *const DrmModeDestroyDumb;
                (*p).handle
            };
            let mut c = captured_clone.lock().unwrap();
            c.fd = Some(call.fd);
            c.request = Some(call.request);
            c.name = Some(call.name.clone());
            c.handle = Some(handle);
            Ok(())
        });

        destroy_dumb_via_ioctl(42, 7).expect("mock ok");

        let c = captured.lock().unwrap();
        assert_eq!(c.fd, Some(42));
        assert_eq!(c.request, Some(expected_destroy_dumb_request()));
        assert_eq!(c.name.as_deref(), Some("MODE_DESTROY_DUMB"));
        assert_eq!(c.handle, Some(7));
    }

    #[test]
    fn create_dumb_propagates_ioctl_error_from_mock() {
        use crate::framebuffer::allocate_dumb_buffer_via_fd;
        use crate::ioctl::mock;

        let _guard = mock::install_scoped(|call| {
            Err(DrmError::Ioctl {
                name: call.name,
                reason: "synthetic ENOMEM".to_string(),
            })
        });

        let err = allocate_dumb_buffer_via_fd(42, 1920, 1080, 32)
            .expect_err("must surface kernel error");
        match err {
            DrmError::Ioctl { name, reason } => {
                assert_eq!(name, "MODE_CREATE_DUMB");
                assert_eq!(reason, "synthetic ENOMEM");
            }
            other => panic!("expected DrmError::Ioctl, got {other:?}"),
        }
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn dumb_buffer_drop_invokes_destroy_dumb_on_linux() {
        use crate::framebuffer::{DrmModeDestroyDumb, allocate_dumb_buffer_via_fd};
        use crate::ioctl::mock;
        use std::sync::{Arc, Mutex};

        // Capture the handle that DESTROY_DUMB sees, ignoring the prior
        // CREATE_DUMB call.
        let destroyed: Arc<Mutex<Option<u32>>> = Arc::new(Mutex::new(None));
        let destroyed_clone = Arc::clone(&destroyed);
        let _guard = mock::install_scoped(move |call| {
            if call.name == "MODE_CREATE_DUMB" {
                // SAFETY: synthetic kernel response.
                unsafe {
                    let p = call.arg as *mut crate::framebuffer::DrmModeCreateDumb;
                    (*p).handle = 0xC0FFEE;
                    (*p).pitch = 4096;
                    (*p).size = 4096u64 * 768;
                }
                Ok(())
            } else if call.name == "MODE_DESTROY_DUMB" {
                // SAFETY: caller passes `&mut DrmModeDestroyDumb`.
                let handle = unsafe {
                    let p = call.arg as *const DrmModeDestroyDumb;
                    (*p).handle
                };
                *destroyed_clone.lock().unwrap() = Some(handle);
                Ok(())
            } else {
                Ok(())
            }
        });

        {
            let _buf = allocate_dumb_buffer_via_fd(99, 1024, 768, 32)
                .expect("create mock ok");
            // Drop here triggers DESTROY_DUMB via DumbBuffer::drop on Linux.
        }

        assert_eq!(*destroyed.lock().unwrap(), Some(0xC0FFEE));
    }

    // -------------------------------------------------------------------
    // t32-e1: ADDFB2 / RMFB ioctl wiring + full-lifecycle regressions.
    // -------------------------------------------------------------------

    /// `DRM_IOWR(0xB8, sizeof(drm_mode_fb_cmd2))` — must match the constant
    /// in `framebuffer.rs`.
    fn expected_addfb2_request() -> core::ffi::c_ulong {
        crate::ioctl::drm_iowr(
            0xB8,
            std::mem::size_of::<crate::framebuffer::DrmModeFbCmd2>(),
        )
    }

    /// `DRM_IOWR(0xAF, sizeof(unsigned int))`.
    fn expected_rmfb_request() -> core::ffi::c_ulong {
        crate::ioctl::drm_iowr(0xAF, std::mem::size_of::<core::ffi::c_uint>())
    }

    #[test]
    fn addfb2_invokes_ioctl_with_correct_request() {
        use crate::framebuffer::add_fb2_via_fd;
        use crate::ioctl::mock;
        use std::sync::{Arc, Mutex};

        let captured: Arc<Mutex<Option<(i32, core::ffi::c_ulong, String)>>> =
            Arc::new(Mutex::new(None));
        let captured_clone = Arc::clone(&captured);
        let _guard = mock::install_scoped(move |call| {
            let mut slot = captured_clone.lock().unwrap();
            if slot.is_none() {
                *slot = Some((call.fd, call.request, call.name.clone()));
            }
            Ok(())
        });

        let dumb = fixture_dumb_buffer(0xAB, 7680, 1920, 1080);
        let id = add_fb2_via_fd(42, &dumb, Fourcc::XRGB8888, 0).expect("mock ok");
        // No fb_id was written, so kernel default zero is returned.
        assert_eq!(id, FramebufferId(0));
        drop(dumb);

        let captured = captured.lock().unwrap().clone().expect("handler ran");
        assert_eq!(captured.0, 42, "fd must be the sentinel passed in");
        assert_eq!(
            captured.1,
            expected_addfb2_request(),
            "request must be DRM_IOCTL_MODE_ADDFB2"
        );
        assert_eq!(captured.2, "MODE_ADDFB2");
    }

    #[test]
    fn addfb2_populates_fb_id_from_kernel_response() {
        use crate::framebuffer::{DrmModeFbCmd2, add_fb2_via_fd};
        use crate::ioctl::mock;

        let _guard = mock::install_scoped(|call| {
            assert_eq!(call.name, "MODE_ADDFB2");
            // SAFETY: caller passes `&mut DrmModeFbCmd2`. Re-cast and
            // write a synthetic kernel response into `fb_id`.
            unsafe {
                let p = call.arg as *mut DrmModeFbCmd2;
                (*p).fb_id = 0x12345678;
            }
            Ok(())
        });

        let dumb = fixture_dumb_buffer(0xAB, 7680, 1920, 1080);
        let id = add_fb2_via_fd(42, &dumb, Fourcc::XRGB8888, 0).expect("mock ok");
        assert_eq!(id, FramebufferId(0x12345678));
        drop(dumb);
    }

    #[test]
    fn rmfb_invokes_ioctl_with_correct_fb_id() {
        use crate::framebuffer::rmfb_via_ioctl;
        use crate::ioctl::mock;
        use std::sync::{Arc, Mutex};

        #[derive(Default)]
        struct Capture {
            fd: Option<i32>,
            request: Option<core::ffi::c_ulong>,
            name: Option<String>,
            fb_id: Option<u32>,
        }
        let captured: Arc<Mutex<Capture>> = Arc::new(Mutex::new(Capture::default()));
        let captured_clone = Arc::clone(&captured);
        let _guard = mock::install_scoped(move |call| {
            // SAFETY: caller passes `&mut u32` (the fb_id arg).
            let fb_id = unsafe {
                let p = call.arg as *const u32;
                *p
            };
            let mut c = captured_clone.lock().unwrap();
            c.fd = Some(call.fd);
            c.request = Some(call.request);
            c.name = Some(call.name.clone());
            c.fb_id = Some(fb_id);
            Ok(())
        });

        rmfb_via_ioctl(42, 99).expect("mock ok");

        let c = captured.lock().unwrap();
        assert_eq!(c.fd, Some(42));
        assert_eq!(c.request, Some(expected_rmfb_request()));
        assert_eq!(c.name.as_deref(), Some("MODE_RMFB"));
        assert_eq!(c.fb_id, Some(99));
    }

    #[test]
    fn framebuffer_create_succeeds_full_lifecycle_under_mock() {
        use crate::framebuffer::{
            DrmModeCreateDumb, DrmModeFbCmd2, create_via_fd,
        };
        use crate::ioctl::mock;

        let _guard = mock::install_scoped(|call| {
            match call.name.as_str() {
                "MODE_CREATE_DUMB" => {
                    // SAFETY: synthetic kernel response.
                    unsafe {
                        let p = call.arg as *mut DrmModeCreateDumb;
                        (*p).handle = 0xBEEF;
                        (*p).pitch = 7680;
                        (*p).size = 7680u64 * 1080;
                    }
                    Ok(())
                }
                "MODE_ADDFB2" => {
                    // SAFETY: synthetic kernel response.
                    unsafe {
                        let p = call.arg as *mut DrmModeFbCmd2;
                        (*p).fb_id = 0xCAFE;
                    }
                    Ok(())
                }
                // Drop-time DESTROY_DUMB / RMFB calls are accepted silently.
                _ => Ok(()),
            }
        });

        let fb = create_via_fd(42, 1920, 1080, 32).expect("full lifecycle ok");
        assert_eq!(fb.id, FramebufferId(0xCAFE));
        assert_eq!(fb.width, 1920);
        assert_eq!(fb.height, 1080);
        assert_eq!(fb.pixel_format, Fourcc::XRGB8888);
        assert_eq!(fb.dumb.handle, 0xBEEF);
        assert_eq!(fb.dumb.pitch, 7680);
        assert_eq!(fb.dumb.size, 7680u64 * 1080);
        assert_eq!(fb.dumb.device_fd, 42);
        // FB drop here issues RMFB then DESTROY_DUMB through the same
        // mock handler — both accepted silently.
    }

    // -------------------------------------------------------------------
    // t33-e1: PAGE_FLIP ioctl wiring regressions.
    // -------------------------------------------------------------------
    //
    // Host-safe regressions exercising `request_page_flip_via_fd` through
    // the t40 mock dispatch layer.

    /// `DRM_IOWR(0xB0, sizeof(drm_mode_crtc_page_flip))` — must match the
    /// constant in `pageflip.rs`. Recomputed here so a regression in either
    /// the encoding helper or the literal `0xB0` is caught.
    fn expected_page_flip_request() -> core::ffi::c_ulong {
        crate::ioctl::drm_iowr(
            0xB0,
            std::mem::size_of::<crate::pageflip::DrmModeCrtcPageFlip>(),
        )
    }

    #[test]
    fn page_flip_invokes_ioctl_with_correct_request() {
        use crate::ioctl::mock;
        use crate::pageflip::request_page_flip_via_fd;
        use std::sync::{Arc, Mutex};

        let captured: Arc<Mutex<Option<(i32, core::ffi::c_ulong, String)>>> =
            Arc::new(Mutex::new(None));
        let captured_clone = Arc::clone(&captured);
        let _guard = mock::install_scoped(move |call| {
            *captured_clone.lock().unwrap() =
                Some((call.fd, call.request, call.name.clone()));
            Ok(())
        });

        request_page_flip_via_fd(
            42,
            CrtcId(7),
            crate::framebuffer::FramebufferId(99),
            PageFlipFlags::EVENT,
            0xDEAD,
        )
        .expect("mock returns Ok");

        let captured = captured.lock().unwrap().clone().expect("handler ran");
        assert_eq!(captured.0, 42, "fd must be the sentinel passed in");
        assert_eq!(
            captured.1,
            expected_page_flip_request(),
            "request must be DRM_IOCTL_MODE_PAGE_FLIP"
        );
        assert_eq!(captured.2, "MODE_PAGE_FLIP");
    }

    #[test]
    fn page_flip_args_carry_typed_inputs() {
        use crate::ioctl::mock;
        use crate::pageflip::{DrmModeCrtcPageFlip, request_page_flip_via_fd};
        use std::sync::{Arc, Mutex};

        let captured: Arc<Mutex<Option<DrmModeCrtcPageFlip>>> = Arc::new(Mutex::new(None));
        let captured_clone = Arc::clone(&captured);
        let _guard = mock::install_scoped(move |call| {
            // SAFETY: caller passes `&mut DrmModeCrtcPageFlip`.
            let args = unsafe {
                let p = call.arg as *const DrmModeCrtcPageFlip;
                *p
            };
            *captured_clone.lock().unwrap() = Some(args);
            Ok(())
        });

        request_page_flip_via_fd(
            42,
            CrtcId(7),
            crate::framebuffer::FramebufferId(99),
            PageFlipFlags::EVENT,
            0xDEAD,
        )
        .expect("mock returns Ok");

        let args = captured.lock().unwrap().expect("handler ran");
        assert_eq!(args.crtc_id, 7);
        assert_eq!(args.fb_id, 99);
        assert_eq!(args.flags, 0x01, "EVENT bit");
        assert_eq!(args.user_data, 0xDEAD);
        assert_eq!(args.reserved, 0);
    }

    #[test]
    fn page_flip_propagates_kernel_error() {
        use crate::ioctl::mock;
        use crate::pageflip::request_page_flip_via_fd;

        let _guard = mock::install_scoped(|_call| {
            Err(DrmError::Ioctl {
                name: "MODE_PAGE_FLIP".to_string(),
                reason: "EBUSY".to_string(),
            })
        });

        let err = request_page_flip_via_fd(
            42,
            CrtcId(7),
            crate::framebuffer::FramebufferId(99),
            PageFlipFlags::EVENT,
            0,
        )
        .expect_err("mock returns Err");
        match err {
            DrmError::Ioctl { name, reason } => {
                assert_eq!(name, "MODE_PAGE_FLIP");
                assert_eq!(reason, "EBUSY");
            }
            other => panic!("expected DrmError::Ioctl, got {other:?}"),
        }
    }

    #[test]
    fn page_flip_async_flag_propagates() {
        use crate::ioctl::mock;
        use crate::pageflip::{DrmModeCrtcPageFlip, request_page_flip_via_fd};
        use std::sync::{Arc, Mutex};

        let captured: Arc<Mutex<Option<DrmModeCrtcPageFlip>>> = Arc::new(Mutex::new(None));
        let captured_clone = Arc::clone(&captured);
        let _guard = mock::install_scoped(move |call| {
            // SAFETY: caller passes `&mut DrmModeCrtcPageFlip`.
            let args = unsafe {
                let p = call.arg as *const DrmModeCrtcPageFlip;
                *p
            };
            *captured_clone.lock().unwrap() = Some(args);
            Ok(())
        });

        request_page_flip_via_fd(
            5,
            CrtcId(11),
            crate::framebuffer::FramebufferId(22),
            PageFlipFlags::EVENT | PageFlipFlags::ASYNC,
            0xBEEF_CAFE,
        )
        .expect("mock returns Ok");

        let args = captured.lock().unwrap().expect("handler ran");
        assert_eq!(args.crtc_id, 11);
        assert_eq!(args.fb_id, 22);
        assert_eq!(args.flags, 0x01 | 0x02);
        assert_eq!(args.user_data, 0xBEEF_CAFE);
    }

    // -------------------------------------------------------------------
    // t34-e1: ATOMIC ioctl wiring regressions.
    // -------------------------------------------------------------------
    //
    // Host-safe regressions exercising `commit_atomic_via_fd` through the
    // t40 mock dispatch layer.

    /// `DRM_IOWR(0xBC, sizeof(drm_mode_atomic))` — must match the constant
    /// in `atomic.rs`. Recomputed here so a regression in either the
    /// encoding helper or the literal `0xBC` is caught.
    fn expected_atomic_request() -> core::ffi::c_ulong {
        crate::ioctl::drm_iowr(
            0xBC,
            std::mem::size_of::<crate::atomic::DrmModeAtomic>(),
        )
    }

    fn sample_atomic_encoded() -> EncodedAtomicRequest {
        // 2 objects, 3 props on first, 2 props on second → 5 flat props/values.
        let changes = vec![
            PropertyChange {
                object_id: ObjectId(10),
                property_id: PropertyId(100),
                value: 1000,
            },
            PropertyChange {
                object_id: ObjectId(20),
                property_id: PropertyId(200),
                value: 2000,
            },
            PropertyChange {
                object_id: ObjectId(10),
                property_id: PropertyId(101),
                value: 1001,
            },
            PropertyChange {
                object_id: ObjectId(10),
                property_id: PropertyId(102),
                value: 1002,
            },
            PropertyChange {
                object_id: ObjectId(20),
                property_id: PropertyId(201),
                value: 2001,
            },
        ];
        encode_atomic_request(&changes)
    }

    #[test]
    fn atomic_invokes_ioctl_with_correct_request() {
        use crate::atomic::commit_atomic_via_fd;
        use crate::ioctl::mock;
        use std::sync::{Arc, Mutex};

        let captured: Arc<Mutex<Option<(i32, core::ffi::c_ulong, String)>>> =
            Arc::new(Mutex::new(None));
        let captured_clone = Arc::clone(&captured);
        let _guard = mock::install_scoped(move |call| {
            *captured_clone.lock().unwrap() =
                Some((call.fd, call.request, call.name.clone()));
            Ok(())
        });

        let encoded = sample_atomic_encoded();
        commit_atomic_via_fd(42, &encoded, AtomicFlags::PAGE_FLIP_EVENT, 0xCAFE)
            .expect("mock returns Ok");

        let captured = captured.lock().unwrap().clone().expect("handler ran");
        assert_eq!(captured.0, 42, "fd must be the sentinel passed in");
        assert_eq!(
            captured.1,
            expected_atomic_request(),
            "request must be DRM_IOCTL_MODE_ATOMIC"
        );
        assert_eq!(captured.2, "MODE_ATOMIC");
    }

    #[test]
    fn atomic_args_carry_array_pointers_and_counts() {
        use crate::atomic::{DrmModeAtomic, commit_atomic_via_fd};
        use crate::ioctl::mock;
        use std::sync::{Arc, Mutex};

        // Capture: (args copy, dereferenced array contents read inside the
        // handler while the owned arrays are still alive on the helper's
        // stack).
        #[derive(Default)]
        struct Capture {
            args: Option<DrmModeAtomic>,
            objs: Vec<u32>,
            count_props: Vec<u32>,
            props: Vec<u32>,
            prop_values: Vec<u64>,
        }
        let captured: Arc<Mutex<Capture>> = Arc::new(Mutex::new(Capture::default()));
        let captured_clone = Arc::clone(&captured);
        let _guard = mock::install_scoped(move |call| {
            // SAFETY: caller passes `&mut DrmModeAtomic`. The pointer
            // fields borrow into the helper's local `OwnedAtomicArrays`,
            // which is still alive for the duration of this handler.
            let args = unsafe {
                let p = call.arg as *const DrmModeAtomic;
                *p
            };
            let objs = unsafe {
                core::slice::from_raw_parts(
                    args.objs_ptr as *const u32,
                    args.count_objs as usize,
                )
                .to_vec()
            };
            let count_props = unsafe {
                core::slice::from_raw_parts(
                    args.count_props_ptr as *const u32,
                    args.count_objs as usize,
                )
                .to_vec()
            };
            let total_props: u32 = count_props.iter().sum();
            let props = unsafe {
                core::slice::from_raw_parts(
                    args.props_ptr as *const u32,
                    total_props as usize,
                )
                .to_vec()
            };
            let prop_values = unsafe {
                core::slice::from_raw_parts(
                    args.prop_values_ptr as *const u64,
                    total_props as usize,
                )
                .to_vec()
            };
            let mut c = captured_clone.lock().unwrap();
            c.args = Some(args);
            c.objs = objs;
            c.count_props = count_props;
            c.props = props;
            c.prop_values = prop_values;
            Ok(())
        });

        let encoded = sample_atomic_encoded();
        commit_atomic_via_fd(7, &encoded, AtomicFlags::empty(), 0)
            .expect("mock returns Ok");

        let c = captured.lock().unwrap();
        let args = c.args.expect("handler ran");
        assert_eq!(args.count_objs, 2);
        assert_eq!(c.objs, vec![10, 20]);
        assert_eq!(c.count_props, vec![3, 2]);
        assert_eq!(c.props, vec![100, 101, 102, 200, 201]);
        assert_eq!(c.prop_values, vec![1000, 1001, 1002, 2000, 2001]);
        assert_eq!(args.reserved, 0);
    }

    #[test]
    fn atomic_propagates_kernel_error() {
        use crate::atomic::commit_atomic_via_fd;
        use crate::ioctl::mock;

        let _guard = mock::install_scoped(|_call| {
            Err(DrmError::Ioctl {
                name: "MODE_ATOMIC".to_string(),
                reason: "EINVAL".to_string(),
            })
        });

        let encoded = sample_atomic_encoded();
        let err = commit_atomic_via_fd(42, &encoded, AtomicFlags::empty(), 0)
            .expect_err("mock returns Err");
        match err {
            DrmError::Ioctl { name, reason } => {
                assert_eq!(name, "MODE_ATOMIC");
                assert_eq!(reason, "EINVAL");
            }
            other => panic!("expected DrmError::Ioctl, got {other:?}"),
        }
    }

    #[test]
    fn atomic_args_carry_typed_flags_and_user_data() {
        use crate::atomic::{DrmModeAtomic, commit_atomic_via_fd};
        use crate::ioctl::mock;
        use std::sync::{Arc, Mutex};

        let captured: Arc<Mutex<Option<DrmModeAtomic>>> = Arc::new(Mutex::new(None));
        let captured_clone = Arc::clone(&captured);
        let _guard = mock::install_scoped(move |call| {
            // SAFETY: caller passes `&mut DrmModeAtomic`.
            let args = unsafe {
                let p = call.arg as *const DrmModeAtomic;
                *p
            };
            *captured_clone.lock().unwrap() = Some(args);
            Ok(())
        });

        let encoded = sample_atomic_encoded();
        let flags = AtomicFlags::NONBLOCK
            | AtomicFlags::ALLOW_MODESET
            | AtomicFlags::PAGE_FLIP_EVENT;
        commit_atomic_via_fd(3, &encoded, flags, 0xDEAD_BEEF_CAFE_BABE)
            .expect("mock returns Ok");

        let args = captured.lock().unwrap().expect("handler ran");
        assert_eq!(
            args.flags,
            (AtomicFlags::NONBLOCK
                | AtomicFlags::ALLOW_MODESET
                | AtomicFlags::PAGE_FLIP_EVENT)
                .bits()
        );
        assert_eq!(args.user_data, 0xDEAD_BEEF_CAFE_BABE);
        assert_eq!(args.reserved, 0);
    }

    // -------------------------------------------------------------------
    // t35a-e1: WAIT_VBLANK typed ioctl surface — host-safe encoding tests.
    // -------------------------------------------------------------------
    //
    // These exercise the pure translation helpers
    // (`vblank_args_from_request`, `vblank_reply_from_args`). The actual
    // ioctl wiring lands in t35-e1.

    #[test]
    fn vblank_args_encodes_relative_mode() {
        use crate::pageflip::{
            VblankFlags, VblankMode, VblankRequest, vblank_args_from_request,
        };

        let req = VblankRequest {
            crtc: CrtcId(0),
            mode: VblankMode::Relative,
            sequence: 5,
            user_data: 0xDEAD,
            flags: VblankFlags::EVENT,
        };
        let args = vblank_args_from_request(&req);

        // EVENT (0x4000_0000) | RELATIVE (0x1) and pipe-0 leaves high bits clear.
        assert_eq!(args.kind, 0x4000_0001);
        assert_eq!(args.sequence, 5);
        // `signal` overlaps `tval_sec` on the request union arm.
        assert_eq!(args.tval_sec, 0xDEAD);
        assert_eq!(args.tval_usec, 0);
    }

    #[test]
    fn vblank_args_encodes_pipe_high_crtc_bits() {
        use crate::pageflip::{
            VblankFlags, VblankMode, VblankRequest, vblank_args_from_request,
        };

        let req = VblankRequest {
            crtc: CrtcId(2),
            mode: VblankMode::Absolute,
            sequence: 0,
            user_data: 0,
            flags: VblankFlags::empty(),
        };
        let args = vblank_args_from_request(&req);

        // High pipe bits live in 0x003F_0000 (bits 16-21); decode by
        // shifting right 16 to recover the pipe index.
        assert_eq!((args.kind & 0x003F_0000) >> 16, 2);
    }

    #[test]
    fn vblank_args_encodes_nextonmiss_flag() {
        use crate::pageflip::{
            VblankFlags, VblankMode, VblankRequest, vblank_args_from_request,
        };

        let req = VblankRequest {
            crtc: CrtcId(0),
            mode: VblankMode::Relative,
            sequence: 1,
            user_data: 0,
            flags: VblankFlags::EVENT | VblankFlags::NEXTONMISS,
        };
        let args = vblank_args_from_request(&req);

        // EVENT | NEXTONMISS | RELATIVE.
        assert_eq!(args.kind & 0x4000_0000, 0x4000_0000);
        assert_eq!(args.kind & 0x0000_0004, 0x0000_0004);
        assert_eq!(args.kind & 0x0000_0001, 0x0000_0001);
    }

    #[test]
    fn vblank_reply_round_trip() {
        use crate::pageflip::{DrmVblank, vblank_reply_from_args};

        let raw = DrmVblank {
            kind: 0x1,
            sequence: 999,
            tval_sec: 5,
            tval_usec: 12_345,
        };
        let reply = vblank_reply_from_args(&raw);

        assert_eq!(reply.kind, 0x1);
        assert_eq!(reply.sequence, 999);
        assert_eq!(reply.tval_sec, 5);
        assert_eq!(reply.tval_usec, 12_345);
    }

    // -------------------------------------------------------------------
    // t35-e1: WAIT_VBLANK ioctl wiring regressions.
    // -------------------------------------------------------------------
    //
    // Host-safe regressions exercising `wait_vblank_via_fd` through the
    // t40 mock dispatch layer.

    /// `DRM_IOWR(0x3A, sizeof(DrmVblank))` — must match the constant in
    /// `pageflip.rs`. Recomputed here so a regression in either the
    /// encoding helper or the literal `0x3A` is caught.
    fn expected_wait_vblank_request() -> core::ffi::c_ulong {
        crate::ioctl::drm_iowr(0x3A, std::mem::size_of::<crate::pageflip::DrmVblank>())
    }

    #[test]
    fn wait_vblank_invokes_ioctl_with_correct_request() {
        use crate::ioctl::mock;
        use crate::pageflip::{VblankFlags, VblankMode, VblankRequest, wait_vblank_via_fd};
        use std::sync::{Arc, Mutex};

        let captured: Arc<Mutex<Option<(i32, core::ffi::c_ulong, String)>>> =
            Arc::new(Mutex::new(None));
        let captured_clone = Arc::clone(&captured);
        let _guard = mock::install_scoped(move |call| {
            *captured_clone.lock().unwrap() =
                Some((call.fd, call.request, call.name.clone()));
            Ok(())
        });

        let req = VblankRequest {
            crtc: CrtcId(0),
            mode: VblankMode::Relative,
            sequence: 1,
            user_data: 0,
            flags: VblankFlags::EVENT,
        };
        wait_vblank_via_fd(42, &req).expect("mock returns Ok");

        let captured = captured.lock().unwrap().clone().expect("handler ran");
        assert_eq!(captured.0, 42, "fd must be the sentinel passed in");
        assert_eq!(
            captured.1,
            expected_wait_vblank_request(),
            "request must be DRM_IOCTL_WAIT_VBLANK"
        );
        assert_eq!(captured.2, "WAIT_VBLANK");
    }

    #[test]
    fn wait_vblank_args_carry_typed_inputs() {
        use crate::ioctl::mock;
        use crate::pageflip::{
            DrmVblank, VblankFlags, VblankMode, VblankRequest, wait_vblank_via_fd,
        };
        use std::sync::{Arc, Mutex};

        let captured: Arc<Mutex<Option<DrmVblank>>> = Arc::new(Mutex::new(None));
        let captured_clone = Arc::clone(&captured);
        let _guard = mock::install_scoped(move |call| {
            // SAFETY: caller passes `&mut DrmVblank`.
            let args = unsafe {
                let p = call.arg as *const DrmVblank;
                *p
            };
            *captured_clone.lock().unwrap() = Some(args);
            Ok(())
        });

        let req = VblankRequest {
            crtc: CrtcId(0),
            mode: VblankMode::Relative,
            sequence: 5,
            user_data: 0xDEAD,
            flags: VblankFlags::EVENT,
        };
        wait_vblank_via_fd(42, &req).expect("mock returns Ok");

        let args = captured.lock().unwrap().expect("handler ran");
        assert_eq!(args.kind, 0x4000_0001, "EVENT | RELATIVE, pipe 0");
        assert_eq!(args.sequence, 5);
        assert_eq!(args.tval_sec, 0xDEAD, "user_data lands in signal slot");
    }

    #[test]
    fn wait_vblank_writeback_populates_reply() {
        use crate::ioctl::mock;
        use crate::pageflip::{
            DrmVblank, VblankFlags, VblankMode, VblankRequest, wait_vblank_via_fd,
        };

        let _guard = mock::install_scoped(|call| {
            // SAFETY: caller passes `&mut DrmVblank`; we overwrite the
            // entire struct in-place to simulate the kernel's reply
            // arm writeback.
            unsafe {
                *(call.arg as *mut DrmVblank) = DrmVblank {
                    kind: 0x1,
                    sequence: 999,
                    tval_sec: 5,
                    tval_usec: 12_345,
                };
            }
            Ok(())
        });

        let req = VblankRequest {
            crtc: CrtcId(0),
            mode: VblankMode::Relative,
            sequence: 1,
            user_data: 0,
            flags: VblankFlags::empty(),
        };
        let reply = wait_vblank_via_fd(42, &req).expect("mock returns Ok");

        assert_eq!(reply.kind, 0x1);
        assert_eq!(reply.sequence, 999);
        assert_eq!(reply.tval_sec, 5);
        assert_eq!(reply.tval_usec, 12_345);
    }

    #[test]
    fn wait_vblank_propagates_kernel_error() {
        use crate::ioctl::mock;
        use crate::pageflip::{
            VblankFlags, VblankMode, VblankRequest, wait_vblank_via_fd,
        };

        let _guard = mock::install_scoped(|_call| {
            Err(DrmError::Ioctl {
                name: "WAIT_VBLANK".to_string(),
                reason: "EINVAL".to_string(),
            })
        });

        let req = VblankRequest {
            crtc: CrtcId(0),
            mode: VblankMode::Relative,
            sequence: 1,
            user_data: 0,
            flags: VblankFlags::empty(),
        };
        let err = wait_vblank_via_fd(42, &req).expect_err("mock returns Err");
        match err {
            DrmError::Ioctl { name, reason } => {
                assert_eq!(name, "WAIT_VBLANK");
                assert_eq!(reason, "EINVAL");
            }
            other => panic!("expected DrmError::Ioctl, got {other:?}"),
        }
    }
}
