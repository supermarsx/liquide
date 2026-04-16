#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::DrmError;
    use crate::mode::{DrmMode, ModeFlags};
    use crate::connector::{ConnectorId, ConnectorType, ConnectorStatus, SubpixelOrder};
    use crate::crtc::CrtcId;
    use crate::atomic::{AtomicRequest, AtomicFlags};
    use crate::pageflip::PageFlipFlags;

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
                width: 1280, height: 720, refresh_hz: 60,
                clock_khz: 74250, flags: ModeFlags::empty(),
                name: "720p".to_string(),
            },
            DrmMode {
                width: 1920, height: 1080, refresh_hz: 60,
                clock_khz: 148500, flags: ModeFlags::PREFERRED,
                name: "1080p".to_string(),
            },
        ];
        let preferred = crate::mode::preferred_mode(&modes);
        assert!(preferred.is_some());
        assert_eq!(preferred.unwrap().width, 1920);
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
    fn test_subpixel_order() {
        let order = SubpixelOrder::HorizontalRgb;
        assert_eq!(format!("{:?}", order), "HorizontalRgb");
    }
}
