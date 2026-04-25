//! Hardware encoder probing and capability discovery.
//!
//! Exposes two probing APIs:
//!
//! * [`EncoderProber::probe_all`] — returns the legacy [`ProbeResult`] list
//!   (VA-API only; kept for backwards compatibility).
//! * [`EncoderProber::probe_matrix`] — returns a structured
//!   [`EncoderProbeResult`] per encoder kind with `{ supported, caps, error }`
//!   suitable for the fallback manager's capability-aware decisions.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::api::{CodecCapability, CodecId, HwEncoderApi};

/// Result of probing a single hardware encoder device (legacy shape).
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

/// A single capability flag recorded on a probed encoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProbeCapability {
    /// Codec support.
    Codec(CodecId),
    /// Zero-copy framebuffer import is available.
    ZeroCopy,
    /// 10-bit colour depth is available.
    TenBit,
    /// B-frames are available.
    BFrames,
    /// HDR metadata signalling is available.
    Hdr,
}

/// Structured probe result emitted by [`EncoderProber::probe_matrix`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncoderProbeResult {
    /// Which encoder API this result describes.
    pub encoder: HwEncoderApi,
    /// Whether this encoder is usable on the host.
    pub supported: bool,
    /// Capability flags advertised by the encoder.
    pub caps: HashSet<ProbeCapability>,
    /// If `supported == false`, the reason.
    pub error: Option<String>,
}

impl EncoderProbeResult {
    /// Whether this encoder is supported *and* advertises the given codec.
    #[must_use]
    pub fn supports_codec(&self, codec: CodecId) -> bool {
        self.supported && self.caps.contains(&ProbeCapability::Codec(codec))
    }

    fn unsupported(encoder: HwEncoderApi, reason: impl Into<String>) -> Self {
        Self {
            encoder,
            supported: false,
            caps: HashSet::new(),
            error: Some(reason.into()),
        }
    }
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
    /// Returns the legacy [`ProbeResult`] list (VA-API only). For the full
    /// structured matrix including unsupported encoders, use
    /// [`Self::probe_matrix`].
    #[must_use]
    pub fn probe_all(&self) -> Vec<ProbeResult> {
        let mut results = Vec::new();
        if let Some(vaapi) = self.probe_vaapi() {
            results.push(vaapi);
        }
        results
    }

    /// Probe every known encoder kind and return a structured matrix. Every
    /// encoder is represented in the output, even if unsupported (with an
    /// `error` explaining why).
    #[must_use]
    pub fn probe_matrix(&self) -> Vec<EncoderProbeResult> {
        vec![
            self.matrix_vaapi(),
            self.matrix_nvenc(),
            self.matrix_amf(),
            self.matrix_v4l2(),
        ]
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
    ///
    /// For the legacy signature this returns `false` (no runtime encode trial
    /// is performed). [`Self::probe_matrix`] returns richer information.
    #[must_use]
    pub fn test_encode(&self, _api: HwEncoderApi, _codec: CodecId) -> bool {
        false
    }

    // ---------------------------------------------------------------------
    // Structured matrix probes
    // ---------------------------------------------------------------------

    fn matrix_vaapi(&self) -> EncoderProbeResult {
        #[cfg(target_os = "linux")]
        {
            if !has_drm_render_node() {
                return EncoderProbeResult::unsupported(
                    HwEncoderApi::Vaapi,
                    "no /dev/dri/renderD* device found",
                );
            }
            match self.probe_vaapi() {
                Some(legacy) => {
                    let mut caps = HashSet::new();
                    for c in &legacy.codecs {
                        caps.insert(ProbeCapability::Codec(c.codec));
                        if c.supports_10bit {
                            caps.insert(ProbeCapability::TenBit);
                        }
                        if c.supports_bframes {
                            caps.insert(ProbeCapability::BFrames);
                        }
                    }
                    caps.insert(ProbeCapability::ZeroCopy);
                    EncoderProbeResult {
                        encoder: HwEncoderApi::Vaapi,
                        supported: true,
                        caps,
                        error: None,
                    }
                }
                None => EncoderProbeResult::unsupported(
                    HwEncoderApi::Vaapi,
                    "libva not available or no encode-capable GPU",
                ),
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            EncoderProbeResult::unsupported(HwEncoderApi::Vaapi, "VA-API is Linux-only")
        }
    }

    fn matrix_nvenc(&self) -> EncoderProbeResult {
        // NVENC = Windows or Linux with NVIDIA GPU. Real detection requires
        // loading the Video Codec SDK; the default build reports it as
        // unsupported unless an explicit env-var override is present (for
        // CI test harnesses that bring their own emitter).
        #[cfg(target_os = "windows")]
        {
            if std::env::var("LIQUIDE_NVENC_OVERRIDE").is_ok() {
                let mut caps = HashSet::new();
                caps.insert(ProbeCapability::Codec(CodecId::H264));
                caps.insert(ProbeCapability::Codec(CodecId::H265));
                caps.insert(ProbeCapability::BFrames);
                caps.insert(ProbeCapability::Hdr);
                return EncoderProbeResult {
                    encoder: HwEncoderApi::Nvenc,
                    supported: true,
                    caps,
                    error: None,
                };
            }
            EncoderProbeResult::unsupported(
                HwEncoderApi::Nvenc,
                "NVIDIA Video Codec SDK not wired in default build (real-codecs feature)",
            )
        }
        #[cfg(target_os = "linux")]
        {
            EncoderProbeResult::unsupported(
                HwEncoderApi::Nvenc,
                "libnvidia-encode.so loading deferred to real-codecs feature",
            )
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            EncoderProbeResult::unsupported(
                HwEncoderApi::Nvenc,
                "NVENC requires Windows or Linux with NVIDIA GPU",
            )
        }
    }

    fn matrix_amf(&self) -> EncoderProbeResult {
        #[cfg(target_os = "windows")]
        {
            if std::env::var("LIQUIDE_AMF_OVERRIDE").is_ok() {
                let mut caps = HashSet::new();
                caps.insert(ProbeCapability::Codec(CodecId::H264));
                caps.insert(ProbeCapability::Codec(CodecId::H265));
                return EncoderProbeResult {
                    encoder: HwEncoderApi::Amf,
                    supported: true,
                    caps,
                    error: None,
                };
            }
            EncoderProbeResult::unsupported(
                HwEncoderApi::Amf,
                "AMD AMF SDK not wired in default build (real-codecs feature)",
            )
        }
        #[cfg(target_os = "linux")]
        {
            EncoderProbeResult::unsupported(
                HwEncoderApi::Amf,
                "AMF on Linux requires amfrt64 shared library (real-codecs feature)",
            )
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            EncoderProbeResult::unsupported(
                HwEncoderApi::Amf,
                "AMF requires Windows or Linux with AMD GPU",
            )
        }
    }

    fn matrix_v4l2(&self) -> EncoderProbeResult {
        #[cfg(target_os = "linux")]
        {
            if !has_v4l2_device() {
                return EncoderProbeResult::unsupported(
                    HwEncoderApi::V4l2,
                    "no /dev/video* device found",
                );
            }
            EncoderProbeResult::unsupported(
                HwEncoderApi::V4l2,
                "V4L2 stateful encoder ioctls not wired in default build (real-codecs feature)",
            )
        }
        #[cfg(not(target_os = "linux"))]
        {
            EncoderProbeResult::unsupported(HwEncoderApi::V4l2, "V4L2 is Linux-only")
        }
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
            let status = unsafe { (va.va_initialize)(display, &mut major, &mut minor) };
            if status != vaapi_ffi::VA_STATUS_SUCCESS {
                vaapi_ffi::close_fd(fd);
                return None;
            }

            // Query supported profiles.
            let max_profiles = unsafe { (va.va_max_num_profiles)(display) } as usize;
            let mut profiles = vec![0i32; max_profiles];
            let mut num_profiles: i32 = 0;
            let status = unsafe {
                (va.va_query_config_profiles)(display, profiles.as_mut_ptr(), &mut num_profiles)
            };
            if status != vaapi_ffi::VA_STATUS_SUCCESS {
                unsafe {
                    (va.va_terminate)(display);
                }
                vaapi_ffi::close_fd(fd);
                return None;
            }

            let profile_slice = &profiles[..num_profiles as usize];

            // For each interesting profile, check if ENCSLICE entrypoint exists.
            let mut codecs = Vec::new();

            if profile_slice.contains(&vaapi_ffi::VA_PROFILE_H264_HIGH)
                && self.has_encode_entrypoint(va, display, vaapi_ffi::VA_PROFILE_H264_HIGH)
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
                && self.has_encode_entrypoint(va, display, vaapi_ffi::VA_PROFILE_HEVC_MAIN)
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
        let max_ep = unsafe { (va.va_max_num_entrypoints)(display) } as usize;
        let mut eps = vec![0i32; max_ep];
        let mut num_ep: i32 = 0;
        let status = unsafe {
            (va.va_query_config_entrypoints)(display, profile, eps.as_mut_ptr(), &mut num_ep)
        };
        if status != crate::vaapi_ffi::VA_STATUS_SUCCESS {
            return false;
        }
        eps[..num_ep as usize].contains(&crate::vaapi_ffi::VA_ENTRYPOINT_ENCSLICE)
    }
}

impl Default for EncoderProber {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "linux")]
fn has_drm_render_node() -> bool {
    use std::path::Path;
    for n in 128u32..=135 {
        if Path::new(&format!("/dev/dri/renderD{}", n)).exists() {
            return true;
        }
    }
    false
}

#[cfg(target_os = "linux")]
fn has_v4l2_device() -> bool {
    use std::path::Path;
    for n in 0u32..=15 {
        if Path::new(&format!("/dev/video{}", n)).exists() {
            return true;
        }
    }
    false
}
