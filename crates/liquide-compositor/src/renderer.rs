//! Renderer-facing contract, backend metadata, and fallback selection.
//!
//! ## Staging status (t49-e1-F21 / plan B5a) — NOT a behaviour change
//!
//! The negotiation surface in this module — [`FallbackRenderer`],
//! [`RendererSelector`], and the capability/negotiation methods on the
//! [`Renderer`] trait — is a **staged negotiation layer with no runtime
//! consumer yet**. Nothing in the live present/render path (the session render
//! thread or app harness) currently drives renderer selection through it; the
//! windowed present path is owned separately and is intentionally untouched
//! here. These types are exercised by this crate's tests only. This note
//! documents that fact; it does not alter the present path or any current
//! behaviour (owned by t55-eF / t50).

use std::error::Error;
use std::fmt;

use crate::damage::{DamageSet, DamageTile};
use crate::framebuffer::{FrameBuffer, FrameMemory};
use crate::pixel::PixelFormat;
use crate::scene::FlatNode;

/// Error returned by renderer implementations.
pub type RenderError = Box<dyn Error + Send + Sync>;

/// Result type for renderer operations.
pub type RenderResult<T> = std::result::Result<T, RenderError>;

/// Quality / performance trade-off hint for renderers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RenderQuality {
    /// Prefer visual quality over performance.
    Quality,
    /// Balanced quality and performance (default).
    #[default]
    Balanced,
    /// Prefer performance over visual quality.
    Performance,
}

/// Broad renderer backend family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RendererBackendKind {
    /// Backend has not identified itself.
    #[default]
    Unknown,
    /// CPU/software renderer.
    Software,
    /// WGPU-backed renderer.
    Wgpu,
    /// Other hardware renderer.
    Hardware,
    /// Remote or delegated renderer.
    Remote,
    /// Fallback wrapper over two renderers.
    Fallback,
}

/// Backend metadata reported by a renderer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererBackendInfo {
    /// Broad backend family.
    pub kind: RendererBackendKind,
    /// Human-readable backend name.
    pub name: String,
    /// Optional backend or driver version.
    pub version: Option<String>,
    /// Optional adapter/device name.
    pub adapter: Option<String>,
}

impl RendererBackendInfo {
    /// Create backend metadata with no version or adapter details.
    #[must_use]
    pub fn new(kind: RendererBackendKind, name: impl Into<String>) -> Self {
        Self {
            kind,
            name: name.into(),
            version: None,
            adapter: None,
        }
    }

    /// Conservative unknown backend metadata.
    #[must_use]
    pub fn unknown() -> Self {
        Self::new(RendererBackendKind::Unknown, "unknown")
    }
}

impl Default for RendererBackendInfo {
    fn default() -> Self {
        Self::unknown()
    }
}

/// Memory backing kind required by a framebuffer target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FrameMemoryKind {
    /// CPU-addressable heap memory.
    #[default]
    Cpu,
    /// Opaque GPU memory.
    Gpu,
    /// GPU memory with DMA-BUF export available.
    DmaBuf,
}

impl FrameMemoryKind {
    /// Classify a framebuffer's backing memory.
    #[must_use]
    pub fn of_framebuffer(fb: &FrameBuffer) -> Self {
        Self::from(&fb.memory)
    }
}

impl From<&FrameMemory> for FrameMemoryKind {
    fn from(memory: &FrameMemory) -> Self {
        match memory {
            FrameMemory::Cpu(_) => Self::Cpu,
            FrameMemory::Gpu { dmabuf_fd, .. } if *dmabuf_fd >= 0 => Self::DmaBuf,
            FrameMemory::Gpu { .. } => Self::Gpu,
        }
    }
}

/// Capabilities a renderer is willing to accept for direct rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererCapabilities {
    /// Supported framebuffer memory kinds.
    pub frame_memory_kinds: Vec<FrameMemoryKind>,
    /// Supported output pixel formats.
    pub pixel_formats: Vec<PixelFormat>,
    /// Whether non-full damage sets can be rendered incrementally.
    pub supports_partial_damage: bool,
    /// Whether the backend can render real blur effects.
    pub supports_blur: bool,
    /// Whether the backend can render a window in skeleton/outline mode.
    pub supports_skeleton_window: bool,
    /// Whether the backend may report pending async glyph rasterization.
    pub supports_async_glyphs: bool,
    /// Optional maximum framebuffer width.
    pub max_framebuffer_width: Option<u32>,
    /// Optional maximum framebuffer height.
    pub max_framebuffer_height: Option<u32>,
}

impl RendererCapabilities {
    /// Conservative CPU framebuffer defaults used by legacy renderers.
    #[must_use]
    pub fn conservative_cpu() -> Self {
        Self {
            frame_memory_kinds: vec![FrameMemoryKind::Cpu],
            pixel_formats: vec![PixelFormat::Bgra8, PixelFormat::Rgba8, PixelFormat::Rgb8],
            supports_partial_damage: true,
            supports_blur: false,
            supports_skeleton_window: false,
            supports_async_glyphs: false,
            max_framebuffer_width: None,
            max_framebuffer_height: None,
        }
    }

    /// Whether this capability set supports the requested memory kind.
    #[must_use]
    pub fn supports_frame_memory(&self, kind: FrameMemoryKind) -> bool {
        self.frame_memory_kinds.contains(&kind)
            || (kind == FrameMemoryKind::DmaBuf
                && self.frame_memory_kinds.contains(&FrameMemoryKind::Gpu))
    }

    /// Whether this capability set supports the requested pixel format.
    #[must_use]
    pub fn supports_pixel_format(&self, format: PixelFormat) -> bool {
        self.pixel_formats.contains(&format)
    }

    /// Negotiate a concrete render target against these capabilities.
    #[must_use]
    pub fn negotiate(&self, fb: &FrameBuffer, damage: &DamageSet) -> RendererNegotiation {
        let memory = FrameMemoryKind::of_framebuffer(fb);
        if !self.supports_frame_memory(memory) {
            return RendererNegotiation::rejected(RendererRejectReason::UnsupportedFrameMemory {
                memory,
            });
        }

        if !self.supports_pixel_format(fb.format) {
            return RendererNegotiation::rejected(RendererRejectReason::UnsupportedPixelFormat {
                format: fb.format,
            });
        }

        let width_too_large = self
            .max_framebuffer_width
            .is_some_and(|max_width| fb.width > max_width);
        let height_too_large = self
            .max_framebuffer_height
            .is_some_and(|max_height| fb.height > max_height);
        if width_too_large || height_too_large {
            return RendererNegotiation::rejected(RendererRejectReason::FramebufferTooLarge {
                width: fb.width,
                height: fb.height,
                max_width: self.max_framebuffer_width,
                max_height: self.max_framebuffer_height,
            });
        }

        if !self.supports_partial_damage && !damage.is_empty() && !damage.is_full() {
            return RendererNegotiation::rejected(RendererRejectReason::PartialDamageUnsupported);
        }

        RendererNegotiation::accepted()
    }

    /// Union two capability sets for a wrapper that can use either backend.
    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        fn push_unique<T: Copy + Eq>(values: &mut Vec<T>, value: T) {
            if !values.contains(&value) {
                values.push(value);
            }
        }

        fn max_limit(left: Option<u32>, right: Option<u32>) -> Option<u32> {
            match (left, right) {
                (Some(left), Some(right)) => Some(left.max(right)),
                (None, _) | (_, None) => None,
            }
        }

        let mut frame_memory_kinds = self.frame_memory_kinds.clone();
        for kind in &other.frame_memory_kinds {
            push_unique(&mut frame_memory_kinds, *kind);
        }

        let mut pixel_formats = self.pixel_formats.clone();
        for format in &other.pixel_formats {
            push_unique(&mut pixel_formats, *format);
        }

        Self {
            frame_memory_kinds,
            pixel_formats,
            supports_partial_damage: self.supports_partial_damage || other.supports_partial_damage,
            supports_blur: self.supports_blur || other.supports_blur,
            supports_skeleton_window: self.supports_skeleton_window
                || other.supports_skeleton_window,
            supports_async_glyphs: self.supports_async_glyphs || other.supports_async_glyphs,
            max_framebuffer_width: max_limit(
                self.max_framebuffer_width,
                other.max_framebuffer_width,
            ),
            max_framebuffer_height: max_limit(
                self.max_framebuffer_height,
                other.max_framebuffer_height,
            ),
        }
    }
}

impl Default for RendererCapabilities {
    fn default() -> Self {
        Self::conservative_cpu()
    }
}

/// Reason a renderer declined a render request during negotiation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RendererRejectReason {
    /// Framebuffer memory is not supported by this renderer.
    UnsupportedFrameMemory { memory: FrameMemoryKind },
    /// Output pixel format is not supported by this renderer.
    UnsupportedPixelFormat { format: PixelFormat },
    /// Framebuffer dimensions exceed backend limits.
    FramebufferTooLarge {
        width: u32,
        height: u32,
        max_width: Option<u32>,
        max_height: Option<u32>,
    },
    /// Renderer only accepts full-frame damage for this target.
    PartialDamageUnsupported,
    /// Backend is currently unavailable.
    BackendUnavailable(String),
    /// Backend-specific rejection reason.
    Other(String),
}

impl fmt::Display for RendererRejectReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedFrameMemory { memory } => {
                write!(formatter, "unsupported framebuffer memory: {memory:?}")
            }
            Self::UnsupportedPixelFormat { format } => {
                write!(formatter, "unsupported pixel format: {format:?}")
            }
            Self::FramebufferTooLarge {
                width,
                height,
                max_width,
                max_height,
            } => write!(
                formatter,
                "framebuffer {width}x{height} exceeds backend limit {max_width:?}x{max_height:?}"
            ),
            Self::PartialDamageUnsupported => write!(formatter, "partial damage is unsupported"),
            Self::BackendUnavailable(reason) | Self::Other(reason) => formatter.write_str(reason),
        }
    }
}

/// Accept/reject decision for a render negotiation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RendererDecision {
    /// Renderer accepts the render target and damage contract.
    Accepted,
    /// Renderer declines with a concrete reason.
    Rejected(RendererRejectReason),
}

impl RendererDecision {
    /// Whether this decision accepts rendering.
    #[must_use]
    pub fn is_accepted(&self) -> bool {
        matches!(self, Self::Accepted)
    }

    /// Rejection reason, if any.
    #[must_use]
    pub fn reject_reason(&self) -> Option<&RendererRejectReason> {
        match self {
            Self::Accepted => None,
            Self::Rejected(reason) => Some(reason),
        }
    }
}

/// Result of renderer capability negotiation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererNegotiation {
    /// Accept/reject decision.
    pub decision: RendererDecision,
}

impl RendererNegotiation {
    /// Accepted negotiation result.
    #[must_use]
    pub fn accepted() -> Self {
        Self {
            decision: RendererDecision::Accepted,
        }
    }

    /// Rejected negotiation result.
    #[must_use]
    pub fn rejected(reason: RendererRejectReason) -> Self {
        Self {
            decision: RendererDecision::Rejected(reason),
        }
    }

    /// Whether this negotiation accepts rendering.
    #[must_use]
    pub fn is_accepted(&self) -> bool {
        self.decision.is_accepted()
    }

    /// Rejection reason, if any.
    #[must_use]
    pub fn reject_reason(&self) -> Option<&RendererRejectReason> {
        self.decision.reject_reason()
    }

    fn into_decision(self) -> RendererDecision {
        self.decision
    }
}

/// Error produced when no renderer in a fallback chain accepts a target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererNegotiationError {
    /// Backend that rejected the final render attempt.
    pub backend: RendererBackendInfo,
    /// Rejection reason.
    pub reason: RendererRejectReason,
}

impl fmt::Display for RendererNegotiationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "renderer '{}' rejected render request: {}",
            self.backend.name, self.reason
        )
    }
}

impl Error for RendererNegotiationError {}

/// Why a fallback renderer switched away from its primary backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackReason {
    /// Primary backend rejected the target during negotiation.
    NegotiationRejected(RendererRejectReason),
    /// Primary backend accepted the target but failed while rendering.
    PrimaryRenderFailed(String),
}

/// Last backend selected by a [`FallbackRenderer`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum FallbackState {
    /// Last successful render used the primary backend, or no render ran yet.
    #[default]
    Primary,
    /// Last render path attempted the fallback backend.
    Fallback {
        /// Reason the fallback path was selected.
        reason: FallbackReason,
        /// Primary backend metadata captured at selection time.
        primary: RendererBackendInfo,
        /// Fallback backend metadata captured at selection time.
        fallback: RendererBackendInfo,
    },
}

impl FallbackState {
    /// Whether the last render used or attempted the fallback backend.
    #[must_use]
    pub fn is_fallback(&self) -> bool {
        matches!(self, Self::Fallback { .. })
    }
}

/// The renderer trait: processes a flattened scene into a frame buffer.
///
/// Implementors convert a list of [`FlatNode`]s into pixel data inside a
/// [`FrameBuffer`].  Optional metadata, negotiation, and quality-control
/// methods have conservative defaults so existing renderers stay usable as
/// object-safe trait values.
pub trait Renderer: Send {
    /// Render the visible scene nodes into the frame buffer.
    ///
    /// Only tiles listed in `damage` need re-rendering.  Returns per-tile
    /// damage classifications for the encoder.
    fn render(
        &mut self,
        nodes: &[FlatNode],
        fb: &mut FrameBuffer,
        damage: &DamageSet,
    ) -> RenderResult<Vec<DamageTile>>;

    /// Renderer backend metadata.
    fn backend_info(&self) -> RendererBackendInfo {
        RendererBackendInfo::default()
    }

    /// Renderer capability metadata.
    fn capabilities(&self) -> RendererCapabilities {
        RendererCapabilities::default()
    }

    /// Negotiate whether this renderer can render a concrete target.
    fn negotiate_render(
        &self,
        _nodes: &[FlatNode],
        fb: &FrameBuffer,
        damage: &DamageSet,
    ) -> RendererNegotiation {
        self.capabilities().negotiate(fb, damage)
    }

    /// Whether real blur is enabled (Glass nodes, etc.).
    fn blur_enabled(&self) -> bool {
        false
    }

    /// Enable or disable blur.
    fn set_blur_enabled(&mut self, _enabled: bool) {}

    /// Whether the last render had text glyphs still being rasterised.
    fn has_pending_glyphs(&self) -> bool {
        false
    }

    /// Report the last render time (ms) for adaptive quality decisions.
    fn report_render_time(&mut self, _ms: f64) {}

    /// Set a window to render in skeleton mode (outline-only during drag).
    fn set_skeleton_window(&mut self, _window_id: Option<u64>) {}

    /// Get the current quality / performance mode.
    fn get_quality_mode(&self) -> RenderQuality {
        RenderQuality::Balanced
    }

    /// Set the quality / performance mode.
    fn set_quality_mode(&mut self, _mode: RenderQuality) {}
}

/// Renderer wrapper that tries a primary backend and falls back when needed.
pub struct FallbackRenderer {
    primary: Box<dyn Renderer>,
    fallback: Box<dyn Renderer>,
    state: FallbackState,
}

impl FallbackRenderer {
    /// Create a fallback wrapper around primary and fallback renderers.
    #[must_use]
    pub fn new(primary: Box<dyn Renderer>, fallback: Box<dyn Renderer>) -> Self {
        Self {
            primary,
            fallback,
            state: FallbackState::default(),
        }
    }

    /// Last fallback selection state.
    #[must_use]
    pub fn fallback_state(&self) -> &FallbackState {
        &self.state
    }

    /// Backend currently represented by the last render state.
    #[must_use]
    pub fn active_backend_info(&self) -> RendererBackendInfo {
        match &self.state {
            FallbackState::Primary => self.primary.backend_info(),
            FallbackState::Fallback { fallback, .. } => fallback.clone(),
        }
    }

    /// Primary backend metadata.
    #[must_use]
    pub fn primary_backend_info(&self) -> RendererBackendInfo {
        self.primary.backend_info()
    }

    /// Fallback backend metadata.
    #[must_use]
    pub fn fallback_backend_info(&self) -> RendererBackendInfo {
        self.fallback.backend_info()
    }

    fn select_fallback(
        &mut self,
        nodes: &[FlatNode],
        fb: &mut FrameBuffer,
        damage: &DamageSet,
        reason: FallbackReason,
    ) -> RenderResult<Vec<DamageTile>> {
        let primary = self.primary.backend_info();
        let fallback = self.fallback.backend_info();
        self.state = FallbackState::Fallback {
            reason,
            primary,
            fallback: fallback.clone(),
        };

        if let RendererDecision::Rejected(reason) = self
            .fallback
            .negotiate_render(nodes, fb, damage)
            .into_decision()
        {
            return Err(Box::new(RendererNegotiationError {
                backend: fallback,
                reason,
            }));
        }

        self.fallback.render(nodes, fb, damage)
    }

    fn active_renderer(&self) -> &dyn Renderer {
        match self.state {
            FallbackState::Primary => self.primary.as_ref(),
            FallbackState::Fallback { .. } => self.fallback.as_ref(),
        }
    }

    fn active_renderer_mut(&mut self) -> &mut dyn Renderer {
        match self.state {
            FallbackState::Primary => self.primary.as_mut(),
            FallbackState::Fallback { .. } => self.fallback.as_mut(),
        }
    }
}

impl Renderer for FallbackRenderer {
    fn render(
        &mut self,
        nodes: &[FlatNode],
        fb: &mut FrameBuffer,
        damage: &DamageSet,
    ) -> RenderResult<Vec<DamageTile>> {
        match self
            .primary
            .negotiate_render(nodes, fb, damage)
            .into_decision()
        {
            RendererDecision::Accepted => match self.primary.render(nodes, fb, damage) {
                Ok(tiles) => {
                    self.state = FallbackState::Primary;
                    Ok(tiles)
                }
                Err(error) => self.select_fallback(
                    nodes,
                    fb,
                    damage,
                    FallbackReason::PrimaryRenderFailed(error.to_string()),
                ),
            },
            RendererDecision::Rejected(reason) => self.select_fallback(
                nodes,
                fb,
                damage,
                FallbackReason::NegotiationRejected(reason),
            ),
        }
    }

    fn backend_info(&self) -> RendererBackendInfo {
        let primary = self.primary.backend_info();
        let fallback = self.fallback.backend_info();
        RendererBackendInfo::new(
            RendererBackendKind::Fallback,
            format!("{} with {} fallback", primary.name, fallback.name),
        )
    }

    fn capabilities(&self) -> RendererCapabilities {
        self.primary
            .capabilities()
            .union(&self.fallback.capabilities())
    }

    fn negotiate_render(
        &self,
        nodes: &[FlatNode],
        fb: &FrameBuffer,
        damage: &DamageSet,
    ) -> RendererNegotiation {
        let primary = self.primary.negotiate_render(nodes, fb, damage);
        if primary.is_accepted() {
            return primary;
        }

        let fallback = self.fallback.negotiate_render(nodes, fb, damage);
        if fallback.is_accepted() {
            return fallback;
        }

        let primary_reason = primary
            .reject_reason()
            .map(ToString::to_string)
            .unwrap_or_else(|| "unknown primary rejection".to_string());
        let fallback_reason = fallback
            .reject_reason()
            .map(ToString::to_string)
            .unwrap_or_else(|| "unknown fallback rejection".to_string());

        RendererNegotiation::rejected(RendererRejectReason::Other(format!(
            "primary rejected: {primary_reason}; fallback rejected: {fallback_reason}"
        )))
    }

    fn blur_enabled(&self) -> bool {
        self.active_renderer().blur_enabled()
    }

    fn set_blur_enabled(&mut self, enabled: bool) {
        self.primary.set_blur_enabled(enabled);
        self.fallback.set_blur_enabled(enabled);
    }

    fn has_pending_glyphs(&self) -> bool {
        self.active_renderer().has_pending_glyphs()
    }

    fn report_render_time(&mut self, ms: f64) {
        self.active_renderer_mut().report_render_time(ms);
    }

    fn set_skeleton_window(&mut self, window_id: Option<u64>) {
        self.primary.set_skeleton_window(window_id);
        self.fallback.set_skeleton_window(window_id);
    }

    fn get_quality_mode(&self) -> RenderQuality {
        self.active_renderer().get_quality_mode()
    }

    fn set_quality_mode(&mut self, mode: RenderQuality) {
        self.primary.set_quality_mode(mode);
        self.fallback.set_quality_mode(mode);
    }
}

/// Result of trying one backend inside a [`RendererSelector`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RendererSelectionResult {
    /// The backend accepted negotiation and completed rendering.
    Accepted,
    /// The backend declined the target before rendering.
    NegotiationRejected(RendererRejectReason),
    /// The backend accepted the target but failed while rendering.
    RenderFailed(String),
}

/// One backend attempt recorded by [`RendererSelector`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererSelectionAttempt {
    /// Backend index in selector priority order.
    pub index: usize,
    /// Backend metadata captured at attempt time.
    pub backend: RendererBackendInfo,
    /// Attempt outcome.
    pub result: RendererSelectionResult,
}

/// Error returned when no backend in a selector can render a frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererSelectionError {
    /// Ordered attempt history for diagnostics.
    pub attempts: Vec<RendererSelectionAttempt>,
}

impl fmt::Display for RendererSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.attempts.is_empty() {
            return formatter.write_str("no renderer backends are registered");
        }

        formatter.write_str("all renderer backends failed")?;
        for attempt in &self.attempts {
            match &attempt.result {
                RendererSelectionResult::Accepted => {
                    write!(formatter, "; {} accepted", attempt.backend.name)?;
                }
                RendererSelectionResult::NegotiationRejected(reason) => {
                    write!(
                        formatter,
                        "; {} rejected negotiation: {}",
                        attempt.backend.name, reason
                    )?;
                }
                RendererSelectionResult::RenderFailed(reason) => {
                    write!(
                        formatter,
                        "; {} failed during render: {}",
                        attempt.backend.name, reason
                    )?;
                }
            }
        }
        Ok(())
    }
}

impl Error for RendererSelectionError {}

/// Ordered renderer manager with capability negotiation and graceful fallback.
pub struct RendererSelector {
    renderers: Vec<Box<dyn Renderer>>,
    active_index: Option<usize>,
    last_attempts: Vec<RendererSelectionAttempt>,
}

impl RendererSelector {
    /// Create a selector from a primary renderer and ordered fallbacks.
    #[must_use]
    pub fn new(primary: Box<dyn Renderer>, fallbacks: Vec<Box<dyn Renderer>>) -> Self {
        let mut renderers = Vec::with_capacity(fallbacks.len() + 1);
        renderers.push(primary);
        renderers.extend(fallbacks);
        Self::from_renderers(renderers)
    }

    /// Create a selector from renderers already in priority order.
    #[must_use]
    pub fn from_renderers(renderers: Vec<Box<dyn Renderer>>) -> Self {
        Self {
            renderers,
            active_index: None,
            last_attempts: Vec::new(),
        }
    }

    /// Append a fallback renderer at the lowest priority.
    pub fn push_fallback(&mut self, renderer: Box<dyn Renderer>) {
        self.renderers.push(renderer);
    }

    /// Number of backends in this selector.
    #[must_use]
    pub fn len(&self) -> usize {
        self.renderers.len()
    }

    /// Whether the selector has no registered backends.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.renderers.is_empty()
    }

    /// Last selected backend index, if any frame has rendered successfully.
    #[must_use]
    pub fn active_index(&self) -> Option<usize> {
        self.active_index
    }

    /// Backend metadata for the last successful renderer, or first configured backend.
    #[must_use]
    pub fn active_backend_info(&self) -> Option<RendererBackendInfo> {
        let index = self.active_index.unwrap_or(0);
        self.renderers
            .get(index)
            .map(|renderer| renderer.backend_info())
    }

    /// Ordered attempt history from the most recent render call.
    #[must_use]
    pub fn last_attempts(&self) -> &[RendererSelectionAttempt] {
        &self.last_attempts
    }

    fn renderer_summary(&self) -> String {
        if self.renderers.is_empty() {
            return "empty renderer selector".to_string();
        }

        self.renderers
            .iter()
            .map(|renderer| renderer.backend_info().name)
            .collect::<Vec<_>>()
            .join(" -> ")
    }

    fn active_renderer(&self) -> Option<&(dyn Renderer + '_)> {
        let index = self.active_index.unwrap_or(0);
        self.renderers.get(index).map(|renderer| renderer.as_ref())
    }
}

impl Renderer for RendererSelector {
    fn render(
        &mut self,
        nodes: &[FlatNode],
        fb: &mut FrameBuffer,
        damage: &DamageSet,
    ) -> RenderResult<Vec<DamageTile>> {
        let mut attempts = Vec::with_capacity(self.renderers.len());

        for index in 0..self.renderers.len() {
            let backend = self.renderers[index].backend_info();
            let negotiation = self.renderers[index].negotiate_render(nodes, fb, damage);

            if let Some(reason) = negotiation.reject_reason().cloned() {
                attempts.push(RendererSelectionAttempt {
                    index,
                    backend,
                    result: RendererSelectionResult::NegotiationRejected(reason),
                });
                continue;
            }

            match self.renderers[index].render(nodes, fb, damage) {
                Ok(tiles) => {
                    attempts.push(RendererSelectionAttempt {
                        index,
                        backend,
                        result: RendererSelectionResult::Accepted,
                    });
                    self.active_index = Some(index);
                    self.last_attempts = attempts;
                    return Ok(tiles);
                }
                Err(error) => {
                    attempts.push(RendererSelectionAttempt {
                        index,
                        backend,
                        result: RendererSelectionResult::RenderFailed(error.to_string()),
                    });
                }
            }
        }

        self.active_index = None;
        self.last_attempts = attempts.clone();
        Err(Box::new(RendererSelectionError { attempts }))
    }

    fn backend_info(&self) -> RendererBackendInfo {
        RendererBackendInfo::new(RendererBackendKind::Fallback, self.renderer_summary())
    }

    fn capabilities(&self) -> RendererCapabilities {
        let mut capabilities = RendererCapabilities {
            frame_memory_kinds: Vec::new(),
            pixel_formats: Vec::new(),
            supports_partial_damage: false,
            supports_blur: false,
            supports_skeleton_window: false,
            supports_async_glyphs: false,
            max_framebuffer_width: Some(0),
            max_framebuffer_height: Some(0),
        };

        let mut renderers = self.renderers.iter();
        if let Some(first) = renderers.next() {
            capabilities = first.capabilities();
        }
        for renderer in renderers {
            capabilities = capabilities.union(&renderer.capabilities());
        }
        capabilities
    }

    fn negotiate_render(
        &self,
        nodes: &[FlatNode],
        fb: &FrameBuffer,
        damage: &DamageSet,
    ) -> RendererNegotiation {
        let mut reasons = Vec::new();
        for renderer in &self.renderers {
            let negotiation = renderer.negotiate_render(nodes, fb, damage);
            if negotiation.is_accepted() {
                return negotiation;
            }
            if let Some(reason) = negotiation.reject_reason() {
                reasons.push(format!("{}: {}", renderer.backend_info().name, reason));
            }
        }

        if reasons.is_empty() {
            RendererNegotiation::rejected(RendererRejectReason::BackendUnavailable(
                "no renderer backends are registered".to_string(),
            ))
        } else {
            RendererNegotiation::rejected(RendererRejectReason::Other(reasons.join("; ")))
        }
    }

    fn blur_enabled(&self) -> bool {
        self.active_renderer().is_some_and(Renderer::blur_enabled)
    }

    fn set_blur_enabled(&mut self, enabled: bool) {
        for renderer in &mut self.renderers {
            renderer.set_blur_enabled(enabled);
        }
    }

    fn has_pending_glyphs(&self) -> bool {
        self.active_renderer()
            .is_some_and(Renderer::has_pending_glyphs)
    }

    fn report_render_time(&mut self, ms: f64) {
        let index = self.active_index.unwrap_or(0);
        if let Some(renderer) = self.renderers.get_mut(index) {
            renderer.report_render_time(ms);
        }
    }

    fn set_skeleton_window(&mut self, window_id: Option<u64>) {
        for renderer in &mut self.renderers {
            renderer.set_skeleton_window(window_id);
        }
    }

    fn get_quality_mode(&self) -> RenderQuality {
        self.active_renderer()
            .map_or(RenderQuality::Balanced, Renderer::get_quality_mode)
    }

    fn set_quality_mode(&mut self, mode: RenderQuality) {
        for renderer in &mut self.renderers {
            renderer.set_quality_mode(mode);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::damage::{DamageClass, DamageSet, DamageTile};
    use crate::framebuffer::{FrameBuffer, FrameMemory};
    use crate::pixel::PixelFormat;

    #[derive(Debug)]
    struct MockRenderError(&'static str);

    impl fmt::Display for MockRenderError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(self.0)
        }
    }

    impl Error for MockRenderError {}

    #[derive(Debug)]
    struct MockState {
        renders: usize,
        blur_enabled: bool,
        quality: RenderQuality,
        skeleton_window: Option<u64>,
    }

    impl Default for MockState {
        fn default() -> Self {
            Self {
                renders: 0,
                blur_enabled: true,
                quality: RenderQuality::Balanced,
                skeleton_window: None,
            }
        }
    }

    struct MockRenderer {
        info: RendererBackendInfo,
        capabilities: RendererCapabilities,
        negotiation: Option<RendererNegotiation>,
        render_error: Option<&'static str>,
        render_tile: DamageTile,
        state: Arc<Mutex<MockState>>,
    }

    impl MockRenderer {
        fn new(name: &'static str, kind: RendererBackendKind) -> (Self, Arc<Mutex<MockState>>) {
            let state = Arc::new(Mutex::new(MockState::default()));
            (
                Self {
                    info: RendererBackendInfo::new(kind, name),
                    capabilities: RendererCapabilities::default(),
                    negotiation: None,
                    render_error: None,
                    render_tile: DamageTile {
                        x: 0,
                        y: 0,
                        class: DamageClass::UiPrimitive,
                    },
                    state: state.clone(),
                },
                state,
            )
        }

        fn rejecting(mut self, reason: RendererRejectReason) -> Self {
            self.negotiation = Some(RendererNegotiation::rejected(reason));
            self
        }

        fn failing_render(mut self, message: &'static str) -> Self {
            self.render_error = Some(message);
            self
        }

        fn with_capabilities(mut self, capabilities: RendererCapabilities) -> Self {
            self.capabilities = capabilities;
            self
        }

        fn with_tile_class(mut self, class: DamageClass) -> Self {
            self.render_tile.class = class;
            self
        }
    }

    struct DefaultOnlyRenderer;

    impl Renderer for DefaultOnlyRenderer {
        fn render(
            &mut self,
            _nodes: &[FlatNode],
            _fb: &mut FrameBuffer,
            _damage: &DamageSet,
        ) -> RenderResult<Vec<DamageTile>> {
            Ok(Vec::new())
        }
    }

    impl Renderer for MockRenderer {
        fn render(
            &mut self,
            _nodes: &[FlatNode],
            _fb: &mut FrameBuffer,
            _damage: &DamageSet,
        ) -> RenderResult<Vec<DamageTile>> {
            self.state.lock().unwrap().renders += 1;
            if let Some(message) = self.render_error {
                return Err(Box::new(MockRenderError(message)));
            }
            Ok(vec![self.render_tile])
        }

        fn backend_info(&self) -> RendererBackendInfo {
            self.info.clone()
        }

        fn capabilities(&self) -> RendererCapabilities {
            self.capabilities.clone()
        }

        fn negotiate_render(
            &self,
            _nodes: &[FlatNode],
            fb: &FrameBuffer,
            damage: &DamageSet,
        ) -> RendererNegotiation {
            self.negotiation
                .clone()
                .unwrap_or_else(|| self.capabilities.negotiate(fb, damage))
        }

        fn blur_enabled(&self) -> bool {
            self.state.lock().unwrap().blur_enabled
        }

        fn set_blur_enabled(&mut self, enabled: bool) {
            self.state.lock().unwrap().blur_enabled = enabled;
        }

        fn set_skeleton_window(&mut self, window_id: Option<u64>) {
            self.state.lock().unwrap().skeleton_window = window_id;
        }

        fn get_quality_mode(&self) -> RenderQuality {
            self.state.lock().unwrap().quality
        }

        fn set_quality_mode(&mut self, mode: RenderQuality) {
            self.state.lock().unwrap().quality = mode;
        }
    }

    fn test_damage() -> DamageSet {
        DamageSet::from_tiles(
            8,
            vec![DamageTile {
                x: 0,
                y: 0,
                class: DamageClass::UiPrimitive,
            }],
        )
    }

    #[test]
    fn fallback_renderer_uses_fallback_when_primary_negotiation_rejects() {
        let reject_reason = RendererRejectReason::BackendUnavailable("device lost".to_string());
        let (primary, primary_state) = MockRenderer::new("primary", RendererBackendKind::Wgpu);
        let (fallback, fallback_state) =
            MockRenderer::new("software", RendererBackendKind::Software);
        let mut renderer = FallbackRenderer::new(
            Box::new(primary.rejecting(reject_reason.clone())),
            Box::new(fallback.with_tile_class(DamageClass::BitmapRegion)),
        );
        let mut fb = FrameBuffer::new(16, 16, PixelFormat::Bgra8);

        let tiles = renderer.render(&[], &mut fb, &test_damage()).unwrap();

        assert_eq!(primary_state.lock().unwrap().renders, 0);
        assert_eq!(fallback_state.lock().unwrap().renders, 1);
        assert_eq!(tiles[0].class, DamageClass::BitmapRegion);
        assert!(matches!(
            renderer.fallback_state(),
            FallbackState::Fallback {
                reason: FallbackReason::NegotiationRejected(reason),
                ..
            } if *reason == reject_reason
        ));
    }

    #[test]
    fn fallback_renderer_uses_fallback_when_primary_render_fails() {
        let (primary, primary_state) = MockRenderer::new("primary", RendererBackendKind::Wgpu);
        let (fallback, fallback_state) =
            MockRenderer::new("software", RendererBackendKind::Software);
        let mut renderer = FallbackRenderer::new(
            Box::new(primary.failing_render("primary failed")),
            Box::new(fallback),
        );
        let mut fb = FrameBuffer::new(16, 16, PixelFormat::Bgra8);

        renderer.render(&[], &mut fb, &test_damage()).unwrap();

        assert_eq!(primary_state.lock().unwrap().renders, 1);
        assert_eq!(fallback_state.lock().unwrap().renders, 1);
        assert!(matches!(
            renderer.fallback_state(),
            FallbackState::Fallback {
                reason: FallbackReason::PrimaryRenderFailed(message),
                ..
            } if message == "primary failed"
        ));
    }

    #[test]
    fn fallback_renderer_reports_wrapper_and_active_backend_metadata() {
        let reject_reason = RendererRejectReason::BackendUnavailable("no adapter".to_string());
        let (primary, _primary_state) = MockRenderer::new("wgpu", RendererBackendKind::Wgpu);
        let (fallback, _fallback_state) = MockRenderer::new("cpu", RendererBackendKind::Software);
        let mut renderer = FallbackRenderer::new(
            Box::new(primary.rejecting(reject_reason)),
            Box::new(fallback),
        );

        let wrapper_info = renderer.backend_info();
        assert_eq!(wrapper_info.kind, RendererBackendKind::Fallback);
        assert!(wrapper_info.name.contains("wgpu"));
        assert!(wrapper_info.name.contains("cpu"));
        assert_eq!(
            renderer.active_backend_info().kind,
            RendererBackendKind::Wgpu
        );

        let mut fb = FrameBuffer::new(16, 16, PixelFormat::Bgra8);
        renderer.render(&[], &mut fb, &test_damage()).unwrap();

        assert_eq!(
            renderer.active_backend_info().kind,
            RendererBackendKind::Software
        );
    }

    #[test]
    fn fallback_renderer_forwards_quality_blur_and_skeleton_controls() {
        let (primary, primary_state) = MockRenderer::new("primary", RendererBackendKind::Wgpu);
        let (fallback, fallback_state) =
            MockRenderer::new("software", RendererBackendKind::Software);
        let mut renderer = FallbackRenderer::new(Box::new(primary), Box::new(fallback));

        renderer.set_quality_mode(RenderQuality::Performance);
        renderer.set_blur_enabled(false);
        renderer.set_skeleton_window(Some(42));

        for state in [&primary_state, &fallback_state] {
            let state = state.lock().unwrap();
            assert_eq!(state.quality, RenderQuality::Performance);
            assert!(!state.blur_enabled);
            assert_eq!(state.skeleton_window, Some(42));
        }
        assert_eq!(renderer.get_quality_mode(), RenderQuality::Performance);
        assert!(!renderer.blur_enabled());
    }

    #[test]
    fn default_negotiation_rejects_gpu_framebuffer_conservatively() {
        let renderer = DefaultOnlyRenderer;
        let fb = FrameBuffer {
            memory: FrameMemory::Gpu {
                handle: 1,
                dmabuf_fd: -1,
                width: 16,
                height: 16,
            },
            width: 16,
            height: 16,
            stride: 64,
            format: PixelFormat::Bgra8,
        };

        let negotiation = renderer.negotiate_render(&[], &fb, &test_damage());

        assert!(matches!(
            negotiation.reject_reason(),
            Some(RendererRejectReason::UnsupportedFrameMemory {
                memory: FrameMemoryKind::Gpu
            })
        ));
    }

    #[test]
    fn fallback_capabilities_are_union_of_primary_and_fallback() {
        let mut gpu_caps = RendererCapabilities::default();
        gpu_caps.frame_memory_kinds = vec![FrameMemoryKind::Gpu];
        gpu_caps.pixel_formats = vec![PixelFormat::Bgra8];
        let mut cpu_caps = RendererCapabilities::default();
        cpu_caps.frame_memory_kinds = vec![FrameMemoryKind::Cpu];
        cpu_caps.pixel_formats = vec![PixelFormat::Rgba8];

        let (primary, _primary_state) = MockRenderer::new("primary", RendererBackendKind::Wgpu);
        let (fallback, _fallback_state) =
            MockRenderer::new("software", RendererBackendKind::Software);
        let renderer = FallbackRenderer::new(
            Box::new(primary.with_capabilities(gpu_caps)),
            Box::new(fallback.with_capabilities(cpu_caps)),
        );

        let capabilities = renderer.capabilities();

        assert!(capabilities.supports_frame_memory(FrameMemoryKind::Gpu));
        assert!(capabilities.supports_frame_memory(FrameMemoryKind::Cpu));
        assert!(capabilities.supports_pixel_format(PixelFormat::Bgra8));
        assert!(capabilities.supports_pixel_format(PixelFormat::Rgba8));
    }

    #[test]
    fn renderer_selector_walks_ordered_backends_until_one_renders() {
        let reject_reason = RendererRejectReason::BackendUnavailable("no adapter".to_string());
        let (first, first_state) = MockRenderer::new("wgpu", RendererBackendKind::Wgpu);
        let (second, second_state) = MockRenderer::new("vulkan", RendererBackendKind::Hardware);
        let (third, third_state) = MockRenderer::new("cpu", RendererBackendKind::Software);
        let mut selector = RendererSelector::from_renderers(vec![
            Box::new(first.rejecting(reject_reason.clone())),
            Box::new(second.failing_render("device lost during submit")),
            Box::new(third.with_tile_class(DamageClass::TextGlyph)),
        ]);
        let mut fb = FrameBuffer::new(16, 16, PixelFormat::Bgra8);

        let tiles = selector.render(&[], &mut fb, &test_damage()).unwrap();

        assert_eq!(tiles[0].class, DamageClass::TextGlyph);
        assert_eq!(selector.active_index(), Some(2));
        assert_eq!(selector.active_backend_info().unwrap().name, "cpu");
        assert_eq!(selector.last_attempts().len(), 3);
        assert_eq!(first_state.lock().unwrap().renders, 0);
        assert_eq!(second_state.lock().unwrap().renders, 1);
        assert_eq!(third_state.lock().unwrap().renders, 1);
        assert!(matches!(
            &selector.last_attempts()[0].result,
            RendererSelectionResult::NegotiationRejected(reason) if *reason == reject_reason
        ));
        assert!(matches!(
            &selector.last_attempts()[1].result,
            RendererSelectionResult::RenderFailed(reason) if reason.contains("device lost")
        ));
        assert_eq!(
            selector.last_attempts()[2].result,
            RendererSelectionResult::Accepted
        );
    }

    #[test]
    fn renderer_selector_reports_all_backend_failures() {
        let (first, _first_state) = MockRenderer::new("wgpu", RendererBackendKind::Wgpu);
        let (second, _second_state) = MockRenderer::new("cpu", RendererBackendKind::Software);
        let mut selector = RendererSelector::from_renderers(vec![
            Box::new(first.rejecting(RendererRejectReason::BackendUnavailable(
                "adapter missing".to_string(),
            ))),
            Box::new(second.failing_render("panic-free render failure")),
        ]);
        let mut fb = FrameBuffer::new(16, 16, PixelFormat::Bgra8);

        let error = selector.render(&[], &mut fb, &test_damage()).unwrap_err();
        let message = error.to_string();

        assert!(message.contains("wgpu rejected negotiation"));
        assert!(message.contains("cpu failed during render"));
        assert_eq!(selector.active_index(), None);
        assert_eq!(selector.last_attempts().len(), 2);
    }
}
