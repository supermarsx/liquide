#[cfg(test)]
mod tests {
    use crate::format::{DrmFourcc, DrmModifier};
    use crate::error::GbmError;

    #[test]
    fn test_drm_fourcc_constants() {
        assert_ne!(DrmFourcc::XRGB8888.0, 0);
        assert_ne!(DrmFourcc::ARGB8888.0, 0);
        assert_ne!(DrmFourcc::XRGB8888, DrmFourcc::ARGB8888);
    }

    #[test]
    fn test_drm_fourcc_name() {
        assert_eq!(DrmFourcc::XRGB8888.name(), "XR24");
        assert_eq!(DrmFourcc::ARGB8888.name(), "AR24");
    }

    #[test]
    fn test_drm_modifier_linear() {
        assert_eq!(DrmModifier::LINEAR.0, 0);
    }

    #[test]
    fn test_gbm_error_display() {
        let err = GbmError::NotSupported;
        assert_eq!(format!("{}", err), "not supported on this platform");
    }

    #[test]
    fn test_gbm_device_non_linux() {
        #[cfg(not(target_os = "linux"))]
        {
            let result = crate::device::GbmDevice::new(-1);
            assert!(result.is_err());
        }
    }
}
