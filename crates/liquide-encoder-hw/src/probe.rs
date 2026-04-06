//! Hardware encoder probing and capability discovery.

use serde::{Deserialize, Serialize};

use crate::api::{CodecCapability, CodecId, HwEncoderApi};

/// Result of probing a single hardware encoder device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeResult {
    /// Which API was discovered.
    pub api: HwEncoderApi,
    /// Human-readable device name.
    pub device_name: String,
    /// Supported codecs with capability information.
    pub codecs: Vec<CodecCapability>,
    /// Maximum number of concurrent sessions.
    pub max_sessions: u32,
    /// Total VRAM in megabytes.
    pub vram_total_mb: u64,
}

/// Discovers available hardware encoders on the system.
pub struct EncoderProber;

impl EncoderProber {
    /// Create a new prober.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Probe all supported APIs in priority order: VAAPI -> NVENC -> AMF -> V4L2.
    ///
    /// Returns an empty list when no hardware encoder is available.
    #[must_use]
    pub fn probe_all(&self) -> Vec<ProbeResult> {
        let mut results = Vec::new();
        if let Some(vaapi) = self.probe_vaapi() {
            results.push(vaapi);
        }
        // Future: probe_nvenc(), probe_amf(), probe_v4l2()
        results
    }

    /// Probe a specific API.
    #[must_use]
    pub fn probe_api(&self, api: HwEncoderApi) -> Option<ProbeResult> {
        match api {
            HwEncoderApi::Vaapi => self.probe_vaapi(),
            _ => None,
        }
    }

    /// Attempt a test encode to verify the API/codec combination works.
    #[must_use]
    pub fn test_encode(&self, _api: HwEncoderApi, _codec: CodecId) -> bool {
        false
    }

    /// Probe VAAPI via runtime dlopen of libva.
    ///
    /// Opens `/dev/dri/renderD128`, initializes VA-API, queries profiles
    /// for encode support, then tears down. Returns `None` if libva is not
    /// installed or no encode-capable GPU is present.
    fn probe_vaapi(&self) -> Option<ProbeResult> {
        #[cfg(target_os = "linux")]
        {
            use crate::vaapi_ffi;

            let va = vaapi_ffi::VaLib::load()?;

            let fd = vaapi_ffi::open_render_node(b"/dev/dri/renderD128\0");
            if fd < 0 {
                return None;
            }

            let display = unsafe { (va.va_get_display_drm)(fd) };
            if display.is_null() {
                vaapi_ffi::close_fd(fd);
                return None;
            }

            let mut major: i32 = 0;
            let mut minor: i32 = 0;
            let status =
                unsafe { (va.va_initialize)(display, &mut major, &mut minor) };
            if status != vaapi_ffi::VA_STATUS_SUCCESS {
                vaapi_ffi::close_fd(fd);
                return None;
            }

            // Query supported profiles.
            let max_profiles =
                unsafe { (va.va_max_num_profiles)(display) } as usize;
            let mut profiles = vec![0i32; max_profiles];
            let mut num_profiles: i32 = 0;
            let status = unsafe {
                (va.va_query_config_profiles)(
                    display,
                    profiles.as_mut_ptr(),
                    &mut num_profiles,
                )
            };
            if status != vaapi_ffi::VA_STATUS_SUCCESS {
                unsafe { (va.va_terminate)(display); }
                vaapi_ffi::close_fd(fd);
                return None;
            }

            let profile_slice = &profiles[..num_profiles as usize];

            // For each interesting profile, check if ENCSLICE entrypoint exists.
            let mut codecs = Vec::new();

            if profile_slice.contains(&vaapi_ffi::VA_PROFILE_H264_HIGH)
                && self.has_encode_entrypoint(
                    va,
                    display,
                    vaapi_ffi::VA_PROFILE_H264_HIGH,
                )
            {
                codecs.push(CodecCapability {
                    codec: CodecId::H264,
                    max_width: 4096,
                    max_height: 4096,
                    max_fps: 120,
                    supports_10bit: false,
                    supports_bframes: true,
                });
            }

            if profile_slice.contains(&vaapi_ffi::VA_PROFILE_HEVC_MAIN)
                && self.has_encode_entrypoint(
                    va,
                    display,
                    vaapi_ffi::VA_PROFILE_HEVC_MAIN,
                )
            {
                codecs.push(CodecCapability {
                    codec: CodecId::H265,
                    max_width: 8192,
                    max_height: 8192,
                    max_fps: 120,
                    supports_10bit: false,
                    supports_bframes: true,
                });
            }

            unsafe {
                (va.va_terminate)(display);
            }
            vaapi_ffi::close_fd(fd);

            if codecs.is_empty() {
                return None;
            }

            Some(ProbeResult {
                api: HwEncoderApi::Vaapi,
                device_name: format!("VA-API {}.{}", major, minor),
                codecs,
                max_sessions: 8,
                vram_total_mb: 0, // not easily queryable via VA-API
            })
        }

        #[cfg(not(target_os = "linux"))]
        {
            None
        }
    }

    /// Check whether a profile supports the encode-slice entrypoint.
    #[cfg(target_os = "linux")]
    fn has_encode_entrypoint(
        &self,
        va: &crate::vaapi_ffi::VaLib,
        display: crate::vaapi_ffi::VADisplay,
        profile: crate::vaapi_ffi::VAProfile,
    ) -> bool {
        let max_ep =
            unsafe { (va.va_max_num_entrypoints)(display) } as usize;
        let mut eps = vec![0i32; max_ep];
        let mut num_ep: i32 = 0;
        let status = unsafe {
            (va.va_query_config_entrypoints)(
                display,
                profile,
                eps.as_mut_ptr(),
                &mut num_ep,
            )
        };
        if status != crate::vaapi_ffi::VA_STATUS_SUCCESS {
            return false;
        }
        eps[..num_ep as usize]
            .contains(&crate::vaapi_ffi::VA_ENTRYPOINT_ENCSLICE)
    }
}

impl Default for EncoderProber {
    fn default() -> Self {
        Self::new()
    }
}
