//! Fallback cascade for hardware encoder failures.

use std::collections::{HashMap, HashSet};

use crate::api::{CodecId, HwEncoderApi};
use crate::config::FallbackConfig;
use crate::probe::{EncoderProbeResult, ProbeCapability};

/// Reason for triggering the fallback cascade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackReason {
    /// Generic encoder error.
    EncoderError,
    /// Session limit reached on this API.
    SessionLimitReached,
    /// Codec not supported by the current API.
    CodecUnsupported,
    /// GPU device was lost.
    DeviceLost,
    /// VRAM budget exceeded.
    VramExhausted,
}

/// Current state of the fallback manager.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackState {
    /// Operating normally with no fallback.
    Normal,
    /// Retrying the same encoder.
    Retrying { attempt: u32 },
    /// Failed over to a different API (or software).
    FailedOver {
        from_api: HwEncoderApi,
        to_api: Option<HwEncoderApi>,
    },
    /// Running on the software encoder fallback.
    SoftwareFallback,
}

/// Action the caller should take after a failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackAction {
    /// Retry the same encoder configuration.
    Retry,
    /// Try a different codec on the same API.
    TryNextCodec { api: HwEncoderApi, codec: CodecId },
    /// Try a different API entirely.
    TryNextApi { api: HwEncoderApi },
    /// Fall back to the software encoder.
    UseSoftware,
    /// All options exhausted.
    GiveUp,
}

/// Manages the fallback cascade: retry → next codec → next API → software.
pub struct FallbackManager {
    config: FallbackConfig,
    state: FallbackState,
    failed_apis: HashSet<HwEncoderApi>,
    failed_codecs: HashMap<HwEncoderApi, HashSet<CodecId>>,
    available_apis: Vec<HwEncoderApi>,
    /// Probed capability matrix; when present, fallback skips unprobed codecs.
    probe_matrix: HashMap<HwEncoderApi, HashSet<ProbeCapability>>,
    retry_count: u32,
}

impl FallbackManager {
    /// Create a new fallback manager.
    #[must_use]
    pub fn new(config: FallbackConfig, available_apis: Vec<HwEncoderApi>) -> Self {
        Self {
            config,
            state: FallbackState::Normal,
            failed_apis: HashSet::new(),
            failed_codecs: HashMap::new(),
            available_apis,
            probe_matrix: HashMap::new(),
            retry_count: 0,
        }
    }

    /// Install a probe matrix. Once installed, [`Self::handle_failure`] will
    /// skip codecs and APIs that the prober reported as unsupported.
    pub fn set_probe_matrix(&mut self, matrix: &[EncoderProbeResult]) {
        self.probe_matrix.clear();
        for r in matrix {
            if r.supported {
                self.probe_matrix.insert(r.encoder, r.caps.clone());
            } else {
                // Mark unsupported APIs as failed so we never route to them.
                self.failed_apis.insert(r.encoder);
            }
        }
    }

    /// Whether a codec is permitted on an API given the probed matrix.
    /// If no matrix has been installed, all codecs are permitted.
    fn codec_permitted(&self, api: HwEncoderApi, codec: CodecId) -> bool {
        if self.probe_matrix.is_empty() {
            return true;
        }
        self.probe_matrix
            .get(&api)
            .map_or(false, |caps| caps.contains(&ProbeCapability::Codec(codec)))
    }

    /// Handle a failure and return the next action to take.
    pub fn handle_failure(
        &mut self,
        api: HwEncoderApi,
        codec: CodecId,
        _reason: FallbackReason,
    ) -> FallbackAction {
        if !self.config.enabled {
            return FallbackAction::GiveUp;
        }

        // Retry the same config if under the limit
        self.retry_count += 1;
        if self.retry_count <= self.config.max_retries {
            self.state = FallbackState::Retrying {
                attempt: self.retry_count,
            };
            return FallbackAction::Retry;
        }

        // Mark this codec as failed on this API
        self.failed_codecs.entry(api).or_default().insert(codec);
        self.retry_count = 0;

        // Try next codec on the same API — only consider codecs permitted
        // by the probe matrix (if installed).
        let all_codecs = [CodecId::H264, CodecId::H265, CodecId::Av1];
        let failed_on_api = self.failed_codecs.get(&api);
        if let Some(next_codec) = all_codecs.iter().find(|c| {
            let not_failed = failed_on_api.map_or(true, |f| !f.contains(c));
            not_failed && self.codec_permitted(api, **c)
        }) {
            return FallbackAction::TryNextCodec {
                api,
                codec: *next_codec,
            };
        }

        // All codecs failed on this API — mark API as failed
        self.failed_apis.insert(api);

        // Try next API — must not be in failed set and must have at least one
        // permitted codec if a probe matrix is installed.
        if let Some(next_api) = self.available_apis.iter().find(|a| {
            !self.failed_apis.contains(a)
                && (self.probe_matrix.is_empty()
                    || all_codecs.iter().any(|c| self.codec_permitted(**a, *c)))
        }) {
            self.state = FallbackState::FailedOver {
                from_api: api,
                to_api: Some(*next_api),
            };
            return FallbackAction::TryNextApi { api: *next_api };
        }

        // All hardware APIs exhausted — use software
        self.state = FallbackState::SoftwareFallback;
        FallbackAction::UseSoftware
    }

    /// Mark an entire API as failed (e.g. device lost).
    pub fn mark_api_failed(&mut self, api: HwEncoderApi) {
        self.failed_apis.insert(api);
    }

    /// Mark a specific codec on an API as failed.
    pub fn mark_codec_failed(&mut self, api: HwEncoderApi, codec: CodecId) {
        self.failed_codecs.entry(api).or_default().insert(codec);
    }

    /// Reset the fallback manager to clean state.
    pub fn reset(&mut self) {
        self.state = FallbackState::Normal;
        self.failed_apis.clear();
        self.failed_codecs.clear();
        self.retry_count = 0;
    }

    /// Current fallback state.
    #[must_use]
    pub fn state(&self) -> &FallbackState {
        &self.state
    }
}
