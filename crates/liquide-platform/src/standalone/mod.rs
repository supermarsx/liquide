//! Standalone compositor platform backend.
//!
//! Implements [`PlatformBackend`] using DRM/KMS for display output and
//! evdev for input. This backend is used when LiquiDE runs as a standalone
//! compositor launched from TTY, as opposed to the remote desktop path
//! (which uses the x11/wayland/win32 client backends).
//!
//! This module is compiled on all platforms but only functional on Linux
//! where DRM/KMS and evdev are available. On other platforms, creation
//! returns an error.
//!
//! # Usage
//!
//! The standalone compositor binary (`liquid-standalone`) creates this
//! backend directly rather than using `create_platform()`, which continues
//! to return the appropriate client-side backend as before.
//!
//! ```rust,ignore
//! use liquide_platform::standalone::StandalonePlatform;
//!
//! let mut platform = StandalonePlatform::new(StandaloneConfig::default())?;
//! desktop_compositor.run(&mut platform);
//! ```

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

#[cfg(target_os = "linux")]
use liquide_drm::{DrmEvent, PageFlipEvent, VblankEvent, drain_pending_events_from_fd};

use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::PixelFormat;

use crate::display::{DisplayBackend, MonitorInfo};
use crate::dnd::{NativeDragDrop, NullDragDrop};
use crate::event_loop::PlatformEvent;
use crate::keymap::{DefaultKeymap, KeymapTranslator};
use crate::notifications::{NativeNotifications, NullNativeNotifications};
use crate::taskbar::{NullTaskbar, TaskbarIntegration};
use crate::tray::{NativeTray, NullNativeTray};
use crate::window_host::{NativeWindowHandle, NativeWindowHost, NativeWindowParams};
use crate::{PlatformBackend, PlatformError, PlatformResult, PresentFeedback};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StandalonePresentMode {
    #[default]
    Immediate,
    Queued,
}

/// Optional submitter invoked once per accepted frame in queued present mode.
///
/// When `None`, the standalone backend's queued mode behaves exactly as today
/// (drains DRM events from `drm_event_fd`, scripted feedback, etc.). When
/// `Some`, the backend additionally calls [`Self::submit_present`] before
/// recording the frame as in-flight so a real page-flip (or other
/// backend-specific submission) can be issued.
///
/// On `Err`, the standalone backend logs the failure and refuses the present
/// (the frame is *not* recorded as in-flight, so queued pacing is not
/// deadlocked) and surfaces a `PlatformError::Presentation` to the caller.
///
/// Linux-only because `liquide-drm` (and hence `DrmError`) is a Linux-only
/// dependency of this crate.
#[cfg(target_os = "linux")]
pub trait StandalonePresentSubmitter: Send {
    /// Submit a present for the given frame sequence number.
    ///
    /// `frame_seq` is the 1-based sequence assigned to this frame by the
    /// standalone backend (matching the value `present_count` will hold once
    /// the frame is recorded).
    fn submit_present(&mut self, frame_seq: u64) -> Result<(), liquide_drm::DrmError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandalonePresentedFrame {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: PixelFormat,
    pub pixels: Vec<u8>,
}

#[derive(Debug, Clone, Copy, Default)]
struct StandalonePendingPresentAck {
    sequence: Option<u32>,
    timestamp_ns: Option<u64>,
    crtc_id: Option<u32>,
}

impl StandalonePendingPresentAck {
    const fn immediate() -> Self {
        Self {
            sequence: None,
            timestamp_ns: None,
            crtc_id: None,
        }
    }

    #[cfg(target_os = "linux")]
    fn from_page_flip_event(event: PageFlipEvent) -> Self {
        Self {
            sequence: Some(event.sequence),
            timestamp_ns: Some(event.timestamp_ns),
            crtc_id: Some(event.crtc_id.0),
        }
    }

    #[cfg(target_os = "linux")]
    fn from_vblank_event(event: VblankEvent) -> Self {
        Self {
            sequence: Some(event.sequence),
            timestamp_ns: Some(event.timestamp_ns),
            crtc_id: Some(event.crtc_id.0),
        }
    }

    #[cfg(target_os = "linux")]
    fn from_drm_event(event: DrmEvent) -> Option<Self> {
        match event {
            DrmEvent::PageFlip(event) => Some(Self::from_page_flip_event(event)),
            DrmEvent::Vblank(event) => Some(Self::from_vblank_event(event)),
            DrmEvent::Unknown(_) => None,
        }
    }
}

#[derive(Debug, Default)]
struct StandaloneSharedState {
    event_queue: VecDeque<PlatformEvent>,
    scripted_present_acks: VecDeque<StandalonePendingPresentAck>,
    present_feedback_queue: VecDeque<PresentFeedback>,
    last_frame: Option<StandalonePresentedFrame>,
    last_present_feedback: Option<PresentFeedback>,
    present_count: u64,
    pending_present_count: u64,
    acknowledged_present_count: u64,
    present_mode: StandalonePresentMode,
}

impl StandaloneSharedState {
    fn can_accept_present(&self) -> bool {
        match self.present_mode {
            StandalonePresentMode::Immediate => true,
            StandalonePresentMode::Queued => self.pending_present_count == 0,
        }
    }

    fn record_present_submission(&mut self, frame: StandalonePresentedFrame) {
        self.last_frame = Some(frame);
        self.present_count = self.present_count.saturating_add(1);
        self.pending_present_count = self.pending_present_count.saturating_add(1);
    }

    fn apply_present_feedback(
        &mut self,
        ack: StandalonePendingPresentAck,
    ) -> Option<PresentFeedback> {
        if self.pending_present_count == 0 {
            return None;
        }

        self.pending_present_count -= 1;
        self.acknowledged_present_count = self.acknowledged_present_count.saturating_add(1);

        let feedback = PresentFeedback {
            acknowledged_present_count: self.acknowledged_present_count,
            sequence: ack.sequence,
            timestamp_ns: ack.timestamp_ns,
            crtc_id: ack.crtc_id,
        };

        self.last_present_feedback = Some(feedback.clone());
        self.present_feedback_queue.push_back(feedback.clone());
        Some(feedback)
    }
}

#[derive(Debug, Clone)]
pub struct StandaloneScriptHandle {
    shared: Arc<Mutex<StandaloneSharedState>>,
}

impl StandaloneScriptHandle {
    pub fn push_event(&self, event: PlatformEvent) {
        lock_shared(&self.shared).event_queue.push_back(event);
    }

    pub fn push_events<I>(&self, events: I)
    where
        I: IntoIterator<Item = PlatformEvent>,
    {
        lock_shared(&self.shared).event_queue.extend(events);
    }

    pub fn push_present_ack(
        &self,
        sequence: Option<u32>,
        timestamp_ns: Option<u64>,
        crtc_id: Option<u32>,
    ) {
        lock_shared(&self.shared)
            .scripted_present_acks
            .push_back(StandalonePendingPresentAck {
                sequence,
                timestamp_ns,
                crtc_id,
            });
    }

    #[must_use]
    pub fn pending_events(&self) -> usize {
        lock_shared(&self.shared).event_queue.len()
    }

    #[must_use]
    pub fn last_presented_frame(&self) -> Option<StandalonePresentedFrame> {
        lock_shared(&self.shared).last_frame.clone()
    }

    #[must_use]
    pub fn last_frame(&self) -> Option<Vec<u8>> {
        self.last_presented_frame().map(|frame| frame.pixels)
    }

    #[must_use]
    pub fn present_count(&self) -> u64 {
        lock_shared(&self.shared).present_count
    }

    #[must_use]
    pub fn pending_present_count(&self) -> u64 {
        lock_shared(&self.shared).pending_present_count
    }

    #[must_use]
    pub fn acknowledged_present_count(&self) -> u64 {
        lock_shared(&self.shared).acknowledged_present_count
    }

    #[must_use]
    pub fn present_ready(&self) -> bool {
        lock_shared(&self.shared).can_accept_present()
    }

    #[must_use]
    pub fn last_present_feedback(&self) -> Option<PresentFeedback> {
        lock_shared(&self.shared).last_present_feedback.clone()
    }
}

fn lock_shared(
    shared: &Arc<Mutex<StandaloneSharedState>>,
) -> MutexGuard<'_, StandaloneSharedState> {
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Configuration for the standalone platform backend.
pub struct StandaloneConfig {
    /// Screen width in pixels.
    pub width: u32,
    /// Screen height in pixels.
    pub height: u32,
    /// Whether to use hardware cursor.
    pub hardware_cursor: bool,
    /// Whether presents are acknowledged immediately or after explicit feedback.
    pub present_mode: StandalonePresentMode,
    /// Optional DRM event fd used to observe queued present acknowledgements.
    pub drm_event_fd: Option<i32>,
    /// Optional present submitter invoked in queued mode for each accepted
    /// frame. Defaults to `None`, in which case the backend behaves exactly as
    /// before (no real page-flip is issued; feedback comes from
    /// `drm_event_fd` and/or scripted acks). See
    /// [`StandalonePresentSubmitter`].
    #[cfg(target_os = "linux")]
    pub submitter: Option<Box<dyn StandalonePresentSubmitter>>,
}

impl Default for StandaloneConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            hardware_cursor: true,
            present_mode: StandalonePresentMode::Immediate,
            drm_event_fd: None,
            #[cfg(target_os = "linux")]
            submitter: None,
        }
    }
}

#[cfg(target_os = "linux")]
impl StandaloneConfig {
    /// Builder-style setter installing a present submitter.
    ///
    /// The submitter is only invoked when [`Self::present_mode`] is
    /// [`StandalonePresentMode::Queued`]. In immediate mode it is carried but
    /// never called.
    #[must_use]
    pub fn with_present_submitter(
        mut self,
        submitter: Box<dyn StandalonePresentSubmitter>,
    ) -> Self {
        self.submitter = Some(submitter);
        self
    }
}

/// Standalone DRM/KMS display backend.
///
/// Reports the DRM output as a single monitor for DisplayBackend queries.
struct StandaloneDisplayBackend {
    width: u32,
    height: u32,
}

impl StandaloneDisplayBackend {
    fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

impl DisplayBackend for StandaloneDisplayBackend {
    fn monitors(&self) -> Vec<MonitorInfo> {
        vec![MonitorInfo {
            id: 1,
            name: "DRM-1".to_string(),
            geometry: Rect::new(0.0, 0.0, self.width as f32, self.height as f32),
            work_area: Rect::new(0.0, 0.0, self.width as f32, self.height as f32),
            dpi_scale: 1.0,
            primary: true,
            refresh_rate_hz: 60,
        }]
    }

    fn primary_monitor(&self) -> Option<MonitorInfo> {
        self.monitors().into_iter().next()
    }

    fn virtual_screen_rect(&self) -> Rect {
        Rect::new(0.0, 0.0, self.width as f32, self.height as f32)
    }
}

/// Standalone window host — manages a single fullscreen "window"
/// backed by the DRM framebuffer.
struct StandaloneWindowHost {
    /// The single fullscreen window handle.
    window: Option<NativeWindowHandle>,
    next_handle: u64,
}

impl StandaloneWindowHost {
    fn new() -> Self {
        Self {
            window: None,
            next_handle: 1,
        }
    }
}

impl NativeWindowHost for StandaloneWindowHost {
    fn create_window(&mut self, _params: NativeWindowParams) -> PlatformResult<NativeWindowHandle> {
        let handle = NativeWindowHandle(self.next_handle);
        self.next_handle += 1;
        self.window = Some(handle);
        Ok(handle)
    }

    fn destroy_window(&mut self, _handle: NativeWindowHandle) -> PlatformResult<()> {
        self.window = None;
        Ok(())
    }

    fn set_geometry(&mut self, _handle: NativeWindowHandle, _geometry: Rect) -> PlatformResult<()> {
        Ok(()) // Resolution changes go through DRM modesetting
    }

    fn set_title(&mut self, _handle: NativeWindowHandle, _title: &str) -> PlatformResult<()> {
        Ok(()) // No title bar in fullscreen DRM mode
    }

    fn set_icon(&mut self, _handle: NativeWindowHandle, _icon_data: &[u8]) -> PlatformResult<()> {
        Ok(()) // No window icon in fullscreen DRM mode
    }

    fn set_state(&mut self, _handle: NativeWindowHandle, _state: &str) -> PlatformResult<()> {
        Ok(()) // Always fullscreen in DRM mode
    }

    fn set_z_order(&mut self, _handle: NativeWindowHandle, _z_order: i32) -> PlatformResult<()> {
        Ok(()) // Single window, no z-ordering needed
    }

    fn set_focus(&mut self, _handle: NativeWindowHandle) -> PlatformResult<()> {
        Ok(()) // Single window always has focus
    }
}

/// The standalone platform backend.
///
/// Implements `PlatformBackend` for direct DRM/KMS output and evdev input.
/// This enables the existing `DesktopCompositor::run()` to work unchanged
/// with the standalone compositor.
pub struct StandalonePlatform {
    display: StandaloneDisplayBackend,
    window_host: StandaloneWindowHost,
    taskbar: NullTaskbar,
    tray: NullNativeTray,
    notifications: NullNativeNotifications,
    drag_drop: NullDragDrop,
    keymap: DefaultKeymap,
    shared: Arc<Mutex<StandaloneSharedState>>,
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    drm_event_fd: Option<i32>,
    /// Optional submitter invoked in queued present mode. `None` in
    /// production today (launcher does not install one), preserving the
    /// existing behaviour exactly.
    #[cfg(target_os = "linux")]
    submitter: Option<Box<dyn StandalonePresentSubmitter>>,
    /// Screen dimensions.
    width: u32,
    height: u32,
}

impl StandalonePlatform {
    /// Create a new standalone platform backend.
    ///
    /// In a full implementation, this would take DRM device and input
    /// device file descriptors. For now, it sets up the scaffolding
    /// that integrates with the existing DesktopCompositor.
    pub fn new(config: StandaloneConfig) -> PlatformResult<Self> {
        let shared = StandaloneSharedState {
            present_mode: config.present_mode,
            ..StandaloneSharedState::default()
        };

        Ok(Self {
            display: StandaloneDisplayBackend::new(config.width, config.height),
            window_host: StandaloneWindowHost::new(),
            taskbar: NullTaskbar,
            tray: NullNativeTray::new(),
            notifications: NullNativeNotifications::new(),
            drag_drop: NullDragDrop,
            keymap: DefaultKeymap,
            shared: Arc::new(Mutex::new(shared)),
            drm_event_fd: config.drm_event_fd,
            #[cfg(target_os = "linux")]
            submitter: config.submitter,
            width: config.width,
            height: config.height,
        })
    }

    #[must_use]
    pub fn script_handle(&self) -> StandaloneScriptHandle {
        StandaloneScriptHandle {
            shared: Arc::clone(&self.shared),
        }
    }

    /// Push an externally-generated event into the queue.
    ///
    /// The standalone compositor's event loop converts DRM/evdev events
    /// into PlatformEvents and pushes them here so that
    /// `DesktopCompositor::run()` can process them via `poll_event()`.
    pub fn push_event(&mut self, event: PlatformEvent) {
        self.script_handle().push_event(event);
    }

    pub fn push_events<I>(&mut self, events: I)
    where
        I: IntoIterator<Item = PlatformEvent>,
    {
        self.script_handle().push_events(events);
    }

    /// Access the last presented frame's pixel data.
    ///
    /// The standalone compositor reads this to copy pixels to the DRM
    /// framebuffer (Path B: local display). This coexists with the
    /// existing tile encoder path (Path A: remote transmission).
    #[must_use]
    pub fn last_presented_frame(&self) -> Option<StandalonePresentedFrame> {
        self.script_handle().last_presented_frame()
    }

    #[must_use]
    pub fn last_frame(&self) -> Option<Vec<u8>> {
        self.script_handle().last_frame()
    }

    #[must_use]
    pub fn present_count(&self) -> u64 {
        self.script_handle().present_count()
    }

    #[must_use]
    pub fn pending_present_count(&self) -> u64 {
        self.script_handle().pending_present_count()
    }

    #[must_use]
    pub fn acknowledged_present_count(&self) -> u64 {
        self.script_handle().acknowledged_present_count()
    }

    #[must_use]
    pub fn last_present_feedback(&self) -> Option<PresentFeedback> {
        self.script_handle().last_present_feedback()
    }

    /// Current screen dimensions.
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn drain_present_feedback(&mut self) {
        let scripted_acknowledgements = {
            let mut shared = lock_shared(&self.shared);
            shared.scripted_present_acks.drain(..).collect::<Vec<_>>()
        };

        #[cfg(target_os = "linux")]
        let acknowledgements = {
            let mut acknowledgements = scripted_acknowledgements;
            if let Some(fd) = self.drm_event_fd {
                acknowledgements.extend(drain_drm_present_feedback_from_fd(fd));
            }
            acknowledgements
        };

        #[cfg(not(target_os = "linux"))]
        let acknowledgements = scripted_acknowledgements;

        if acknowledgements.is_empty() {
            return;
        }

        let mut shared = lock_shared(&self.shared);
        for ack in acknowledgements {
            let _ = shared.apply_present_feedback(ack);
        }
    }
}

impl PlatformBackend for StandalonePlatform {
    fn display(&self) -> &dyn DisplayBackend {
        &self.display
    }

    fn window_host(&mut self) -> &mut dyn NativeWindowHost {
        &mut self.window_host
    }

    fn taskbar(&mut self) -> &mut dyn TaskbarIntegration {
        &mut self.taskbar
    }

    fn tray(&mut self) -> &mut dyn NativeTray {
        &mut self.tray
    }

    fn notifications(&mut self) -> &mut dyn NativeNotifications {
        &mut self.notifications
    }

    fn drag_drop(&mut self) -> &mut dyn NativeDragDrop {
        &mut self.drag_drop
    }

    fn keymap(&self) -> &dyn KeymapTranslator {
        &self.keymap
    }

    fn platform_name(&self) -> &str {
        "standalone-drm"
    }

    fn poll_event(&mut self) -> Option<PlatformEvent> {
        self.drain_present_feedback();
        lock_shared(&self.shared).event_queue.pop_front()
    }

    fn wait_event(&mut self) -> PlatformEvent {
        self.drain_present_feedback();
        // In a real implementation, this would block on epoll/poll
        // waiting for DRM, evdev, or Wayland client events.
        if let Some(event) = lock_shared(&self.shared).event_queue.pop_front() {
            event
        } else {
            // Yield briefly then return Quit if nothing pending.
            // The real implementation will use proper fd-based blocking.
            PlatformEvent::Quit
        }
    }

    fn can_accept_present(&mut self) -> bool {
        self.drain_present_feedback();
        lock_shared(&self.shared).can_accept_present()
    }

    fn take_present_feedback(&mut self) -> Option<PresentFeedback> {
        self.drain_present_feedback();
        lock_shared(&self.shared).present_feedback_queue.pop_front()
    }

    fn present_frame(
        &mut self,
        _handle: NativeWindowHandle,
        pixels: &[u8],
        width: u32,
        height: u32,
        stride: u32,
        format: PixelFormat,
    ) -> PlatformResult<()> {
        self.drain_present_feedback();

        // Path B: Local display output.
        // Store pixels for DRM scanout. In the full implementation,
        // this would copy to the DRM framebuffer and trigger a page flip.
        let (queued_mode, next_frame_seq) = {
            let shared = lock_shared(&self.shared);
            if !shared.can_accept_present() {
                return Err(PlatformError::Presentation(
                    "standalone present backpressure: previous queued frame has not been acknowledged"
                        .to_string(),
                ));
            }
            let queued = matches!(shared.present_mode, StandalonePresentMode::Queued);
            let next_seq = shared.present_count.saturating_add(1);
            (queued, next_seq)
        };

        // In queued mode, invoke the optional submitter BEFORE recording the
        // frame as in-flight so a real page-flip (or other backend submission)
        // can be issued. On error, the frame is *not* recorded as in-flight —
        // this surfaces the failure to the caller without deadlocking pacing
        // (subsequent `can_accept_present()` calls remain true).
        #[cfg(target_os = "linux")]
        {
            if queued_mode {
                if let Some(submitter) = self.submitter.as_mut() {
                    if let Err(err) = submitter.submit_present(next_frame_seq) {
                        eprintln!(
                            "liquide-platform: standalone present submitter failed for frame_seq={next_frame_seq}: {err:?}; dropping frame"
                        );
                        return Err(PlatformError::Presentation(format!(
                            "standalone present submitter failed: {err}"
                        )));
                    }
                }
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (queued_mode, next_frame_seq);
        }

        let mut shared = lock_shared(&self.shared);
        shared.record_present_submission(StandalonePresentedFrame {
            width,
            height,
            stride,
            format,
            pixels: pixels.to_vec(),
        });

        if matches!(shared.present_mode, StandalonePresentMode::Immediate) {
            let _ = shared.apply_present_feedback(StandalonePendingPresentAck::immediate());
        }

        Ok(())
    }

    fn request_redraw(&mut self, _handle: NativeWindowHandle) {
        lock_shared(&self.shared)
            .event_queue
            .push_back(PlatformEvent::WindowRedraw {
                handle: NativeWindowHandle(1),
            });
    }

    fn set_cursor_shape(&mut self, _handle: NativeWindowHandle, _shape: &str) -> bool {
        // In a full implementation, this would set the DRM hardware cursor plane.
        // For now, return false to use software cursor rendering.
        false
    }

    fn hide_cursor(&mut self, _handle: NativeWindowHandle) {
        // Hide DRM hardware cursor plane.
    }

    fn show_cursor(&mut self, _handle: NativeWindowHandle) {
        // Show DRM hardware cursor plane.
    }
}

#[cfg(target_os = "linux")]
fn drain_drm_present_feedback_from_fd(fd: i32) -> Vec<StandalonePendingPresentAck> {
    // Malformed, truncated, or unreadable DRM batches must not synthesize acknowledgements.
    drain_pending_events_from_fd(fd)
        .map(|events| {
            events
                .into_iter()
                .filter_map(StandalonePendingPresentAck::from_drm_event)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    const TEST_DRM_EVENT_VBLANK: u32 = 0x01;
    #[cfg(target_os = "linux")]
    const TEST_DRM_EVENT_FLIP_COMPLETE: u32 = 0x02;

    #[cfg(target_os = "linux")]
    struct TestPipe {
        read_fd: i32,
        write_fd: i32,
    }

    #[cfg(target_os = "linux")]
    impl TestPipe {
        fn new() -> Self {
            let mut fds = [0; 2];
            let result = unsafe { libc::pipe(fds.as_mut_ptr()) };
            assert_eq!(
                result,
                0,
                "pipe creation failed: {}",
                std::io::Error::last_os_error()
            );

            Self {
                read_fd: fds[0],
                write_fd: fds[1],
            }
        }

        fn read_fd(&self) -> i32 {
            self.read_fd
        }

        fn write_all(&self, bytes: &[u8]) {
            let mut written = 0;
            while written < bytes.len() {
                let result = unsafe {
                    libc::write(
                        self.write_fd,
                        bytes[written..].as_ptr().cast::<libc::c_void>(),
                        bytes.len() - written,
                    )
                };
                assert!(
                    result >= 0,
                    "pipe write failed: {}",
                    std::io::Error::last_os_error()
                );
                written += result as usize;
            }
        }

        fn close_read_end(&mut self) {
            if self.read_fd >= 0 {
                let result = unsafe { libc::close(self.read_fd) };
                assert_eq!(
                    result,
                    0,
                    "pipe close(read) failed: {}",
                    std::io::Error::last_os_error()
                );
                self.read_fd = -1;
            }
        }
    }

    #[cfg(target_os = "linux")]
    impl Drop for TestPipe {
        fn drop(&mut self) {
            if self.read_fd >= 0 {
                unsafe {
                    libc::close(self.read_fd);
                }
            }
            if self.write_fd >= 0 {
                unsafe {
                    libc::close(self.write_fd);
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn queued_platform_with_drm_fd(fd: i32) -> StandalonePlatform {
        StandalonePlatform::new(StandaloneConfig {
            present_mode: StandalonePresentMode::Queued,
            drm_event_fd: Some(fd),
            ..StandaloneConfig::default()
        })
        .unwrap()
    }

    #[cfg(target_os = "linux")]
    fn present_one_queued_frame(platform: &mut StandalonePlatform) {
        platform
            .present_frame(
                NativeWindowHandle(1),
                &[1, 2, 3, 4],
                1,
                1,
                4,
                PixelFormat::Bgra8,
            )
            .unwrap();

        assert_eq!(platform.present_count(), 1);
        assert_eq!(platform.pending_present_count(), 1);
        assert_eq!(platform.acknowledged_present_count(), 0);
        assert!(!platform.can_accept_present());
    }

    #[cfg(target_os = "linux")]
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

    #[cfg(target_os = "linux")]
    fn build_unknown_record(event_type: u32, payload: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        push_u32_native(&mut bytes, event_type);
        push_u32_native(&mut bytes, (8 + payload.len()) as u32);
        bytes.extend_from_slice(payload);
        bytes
    }

    #[cfg(target_os = "linux")]
    fn push_u32_native(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend_from_slice(&value.to_ne_bytes());
    }

    #[cfg(target_os = "linux")]
    fn push_u64_native(bytes: &mut Vec<u8>, value: u64) {
        bytes.extend_from_slice(&value.to_ne_bytes());
    }

    #[test]
    fn default_config_is_1920x1080() {
        let config = StandaloneConfig::default();
        assert_eq!(config.width, 1920);
        assert_eq!(config.height, 1080);
        assert!(config.hardware_cursor);
        assert_eq!(config.present_mode, StandalonePresentMode::Immediate);
        assert_eq!(config.drm_event_fd, None);
    }

    #[test]
    fn creates_successfully_with_default_config() {
        let platform = StandalonePlatform::new(StandaloneConfig::default());
        assert!(platform.is_ok());
    }

    #[test]
    fn creates_with_custom_dimensions() {
        let config = StandaloneConfig {
            width: 2560,
            height: 1440,
            hardware_cursor: false,
            present_mode: StandalonePresentMode::Queued,
            drm_event_fd: Some(7),
            #[cfg(target_os = "linux")]
            submitter: None,
        };
        let platform = StandalonePlatform::new(config).unwrap();
        assert_eq!(platform.dimensions(), (2560, 1440));
    }

    #[test]
    fn display_reports_single_monitor() {
        let platform = StandalonePlatform::new(StandaloneConfig::default()).unwrap();
        let monitors = platform.display().monitors();
        assert_eq!(monitors.len(), 1);
        assert_eq!(monitors[0].name, "DRM-1");
        assert!(monitors[0].primary);
        assert_eq!(monitors[0].refresh_rate_hz, 60);
    }

    #[test]
    fn display_primary_monitor_matches_first() {
        let platform = StandalonePlatform::new(StandaloneConfig::default()).unwrap();
        let primary = platform.display().primary_monitor();
        assert!(primary.is_some());
        assert_eq!(primary.unwrap().id, 1);
    }

    #[test]
    fn display_virtual_screen_matches_config() {
        let config = StandaloneConfig {
            width: 3840,
            height: 2160,
            hardware_cursor: true,
            present_mode: StandalonePresentMode::Immediate,
            drm_event_fd: None,
            #[cfg(target_os = "linux")]
            submitter: None,
        };
        let platform = StandalonePlatform::new(config).unwrap();
        let rect = platform.display().virtual_screen_rect();
        assert_eq!(rect, Rect::new(0.0, 0.0, 3840.0, 2160.0));
    }

    #[test]
    fn platform_name_is_standalone_drm() {
        let platform = StandalonePlatform::new(StandaloneConfig::default()).unwrap();
        assert_eq!(platform.platform_name(), "standalone-drm");
    }

    #[test]
    fn poll_event_returns_none_when_empty() {
        let mut platform = StandalonePlatform::new(StandaloneConfig::default()).unwrap();
        assert!(platform.poll_event().is_none());
    }

    #[test]
    fn push_event_and_poll() {
        let mut platform = StandalonePlatform::new(StandaloneConfig::default()).unwrap();
        let handle = NativeWindowHandle(1);
        platform.push_event(PlatformEvent::FocusGained { handle });
        let event = platform.poll_event();
        assert!(event.is_some());
        assert!(matches!(event.unwrap(), PlatformEvent::FocusGained { .. }));
    }

    #[test]
    fn push_multiple_events_fifo_order() {
        let mut platform = StandalonePlatform::new(StandaloneConfig::default()).unwrap();
        let h = NativeWindowHandle(1);
        platform.push_event(PlatformEvent::FocusGained { handle: h });
        platform.push_event(PlatformEvent::FocusLost { handle: h });
        platform.push_event(PlatformEvent::Quit);
        assert!(matches!(
            platform.poll_event().unwrap(),
            PlatformEvent::FocusGained { .. }
        ));
        assert!(matches!(
            platform.poll_event().unwrap(),
            PlatformEvent::FocusLost { .. }
        ));
        assert!(matches!(
            platform.poll_event().unwrap(),
            PlatformEvent::Quit
        ));
        assert!(platform.poll_event().is_none());
    }

    #[test]
    fn wait_event_returns_quit_when_empty() {
        let mut platform = StandalonePlatform::new(StandaloneConfig::default()).unwrap();
        let event = platform.wait_event();
        assert!(matches!(event, PlatformEvent::Quit));
    }

    #[test]
    fn wait_event_timeout_returns_pending_event_immediately() {
        // CONTRACT (t97-wakeup): a queued event must return with ~zero latency —
        // the timed wait must NOT wait out the timeout when work is ready. This
        // exercises the default (poll-based) impl's fast path; the Win32
        // override has the same contract via MsgWaitForMultipleObjectsEx.
        let mut platform = StandalonePlatform::new(StandaloneConfig::default()).unwrap();
        let h = NativeWindowHandle(1);
        platform.push_event(PlatformEvent::FocusGained { handle: h });

        let start = std::time::Instant::now();
        let ev = platform.wait_event_timeout(std::time::Duration::from_millis(500));
        let elapsed = start.elapsed();

        assert!(
            matches!(ev, Some(PlatformEvent::FocusGained { .. })),
            "a pending event must be returned, got {ev:?}"
        );
        assert!(
            elapsed < std::time::Duration::from_millis(100),
            "a ready event must return promptly, not wait out the timeout (took {elapsed:?})"
        );
    }

    #[test]
    fn wait_event_timeout_times_out_when_idle() {
        // CONTRACT (t97-wakeup): with nothing queued the timed wait must return
        // None AFTER (approximately) the timeout — it must block (park), not
        // return instantly (busy-spin) and not block forever.
        let mut platform = StandalonePlatform::new(StandaloneConfig::default()).unwrap();
        let timeout = std::time::Duration::from_millis(30);

        let start = std::time::Instant::now();
        let ev = platform.wait_event_timeout(timeout);
        let elapsed = start.elapsed();

        assert!(ev.is_none(), "idle timed wait must return None, got {ev:?}");
        assert!(
            elapsed >= std::time::Duration::from_millis(20),
            "idle timed wait must actually wait ~the timeout (parked), not spin/return instantly (took {elapsed:?})"
        );
    }

    #[test]
    fn wait_event_timeout_zero_is_nonblocking_poll() {
        // A zero timeout is a present-now non-blocking poll: returns whatever is
        // queued (or None) WITHOUT blocking.
        let mut platform = StandalonePlatform::new(StandaloneConfig::default()).unwrap();

        let start = std::time::Instant::now();
        let ev = platform.wait_event_timeout(std::time::Duration::ZERO);
        assert!(ev.is_none(), "empty zero-timeout poll must be None");
        assert!(
            start.elapsed() < std::time::Duration::from_millis(10),
            "zero timeout must not block"
        );

        let h = NativeWindowHandle(1);
        platform.push_event(PlatformEvent::FocusGained { handle: h });
        let ev = platform.wait_event_timeout(std::time::Duration::ZERO);
        assert!(
            matches!(ev, Some(PlatformEvent::FocusGained { .. })),
            "zero-timeout poll must still drain a ready event, got {ev:?}"
        );
    }

    #[test]
    fn present_frame_stores_pixels() {
        let mut platform = StandalonePlatform::new(StandaloneConfig::default()).unwrap();
        let handle = NativeWindowHandle(1);
        let pixels = vec![0xFFu8; 100];
        let result = platform.present_frame(handle, &pixels, 10, 10, 40, PixelFormat::Bgra8);
        assert!(result.is_ok());
        let frame = platform.last_presented_frame().unwrap();
        assert_eq!(frame.width, 10);
        assert_eq!(frame.height, 10);
        assert_eq!(frame.stride, 40);
        assert_eq!(frame.format, PixelFormat::Bgra8);
        assert_eq!(frame.pixels.len(), 100);
        assert_eq!(platform.present_count(), 1);
        assert_eq!(platform.pending_present_count(), 0);
        assert_eq!(platform.acknowledged_present_count(), 1);
    }

    #[test]
    fn last_frame_is_none_before_present() {
        let platform = StandalonePlatform::new(StandaloneConfig::default()).unwrap();
        assert!(platform.last_frame().is_none());
    }

    #[test]
    fn script_handle_reuses_event_queue_and_frame_capture() {
        let mut platform = StandalonePlatform::new(StandaloneConfig::default()).unwrap();
        let script = platform.script_handle();
        let handle = NativeWindowHandle(1);

        script.push_events([
            PlatformEvent::FocusGained { handle },
            PlatformEvent::FocusLost { handle },
        ]);
        assert_eq!(script.pending_events(), 2);
        assert!(matches!(
            platform.poll_event(),
            Some(PlatformEvent::FocusGained { .. })
        ));
        assert!(matches!(
            platform.poll_event(),
            Some(PlatformEvent::FocusLost { .. })
        ));

        platform
            .present_frame(handle, &[1, 2, 3, 4], 1, 1, 4, PixelFormat::Bgra8)
            .unwrap();
        let frame = script.last_presented_frame().unwrap();
        assert_eq!(frame.width, 1);
        assert_eq!(frame.height, 1);
        assert_eq!(frame.pixels, vec![1, 2, 3, 4]);
        assert_eq!(script.present_count(), 1);
        assert_eq!(script.pending_present_count(), 0);
        assert_eq!(script.acknowledged_present_count(), 1);
    }

    #[test]
    fn immediate_mode_preserves_compatibility_and_surfaces_feedback() {
        let mut platform = StandalonePlatform::new(StandaloneConfig::default()).unwrap();
        let handle = NativeWindowHandle(1);

        platform
            .present_frame(handle, &[1, 2, 3, 4], 1, 1, 4, PixelFormat::Bgra8)
            .unwrap();
        platform
            .present_frame(handle, &[5, 6, 7, 8], 1, 1, 4, PixelFormat::Bgra8)
            .unwrap();

        assert!(platform.can_accept_present());
        assert_eq!(platform.present_count(), 2);
        assert_eq!(platform.pending_present_count(), 0);
        assert_eq!(platform.acknowledged_present_count(), 2);

        let first_feedback = platform.take_present_feedback().unwrap();
        assert_eq!(first_feedback.acknowledged_present_count, 1);
        assert_eq!(first_feedback.sequence, None);

        let second_feedback = platform.take_present_feedback().unwrap();
        assert_eq!(second_feedback.acknowledged_present_count, 2);
        assert!(platform.take_present_feedback().is_none());
    }

    #[test]
    fn queued_mode_returns_backpressure_until_acknowledged() {
        let mut platform = StandalonePlatform::new(StandaloneConfig {
            present_mode: StandalonePresentMode::Queued,
            ..StandaloneConfig::default()
        })
        .unwrap();
        let handle = NativeWindowHandle(1);

        platform
            .present_frame(handle, &[1, 2, 3, 4], 1, 1, 4, PixelFormat::Bgra8)
            .unwrap();

        assert_eq!(platform.present_count(), 1);
        assert_eq!(platform.pending_present_count(), 1);
        assert_eq!(platform.acknowledged_present_count(), 0);
        assert!(!platform.can_accept_present());

        let error = platform
            .present_frame(handle, &[9, 9, 9, 9], 1, 1, 4, PixelFormat::Bgra8)
            .unwrap_err();

        assert!(matches!(
            error,
            PlatformError::Presentation(message) if message.contains("backpressure")
        ));
        assert_eq!(
            platform.last_presented_frame().unwrap().pixels,
            vec![1, 2, 3, 4]
        );
    }

    #[test]
    fn queued_mode_acknowledges_before_returning_platform_events() {
        let mut platform = StandalonePlatform::new(StandaloneConfig {
            present_mode: StandalonePresentMode::Queued,
            ..StandaloneConfig::default()
        })
        .unwrap();
        let script = platform.script_handle();
        let handle = NativeWindowHandle(1);

        platform
            .present_frame(handle, &[1, 2, 3, 4], 1, 1, 4, PixelFormat::Bgra8)
            .unwrap();
        script.push_event(PlatformEvent::Quit);
        script.push_present_ack(Some(17), Some(42), Some(3));

        assert_eq!(script.acknowledged_present_count(), 0);

        let event = platform.poll_event();

        assert!(matches!(event, Some(PlatformEvent::Quit)));
        assert_eq!(script.acknowledged_present_count(), 1);
        assert_eq!(script.pending_present_count(), 0);
        assert!(platform.can_accept_present());

        let feedback = platform.take_present_feedback().unwrap();
        assert_eq!(feedback.acknowledged_present_count, 1);
        assert_eq!(feedback.sequence, Some(17));
        assert_eq!(feedback.timestamp_ns, Some(42));
        assert_eq!(feedback.crtc_id, Some(3));
    }

    #[test]
    fn script_handle_exposes_queued_present_state_without_ui_events() {
        let mut platform = StandalonePlatform::new(StandaloneConfig {
            present_mode: StandalonePresentMode::Queued,
            ..StandaloneConfig::default()
        })
        .unwrap();
        let script = platform.script_handle();
        let handle = NativeWindowHandle(1);

        platform
            .present_frame(handle, &[5, 6, 7, 8], 1, 1, 4, PixelFormat::Bgra8)
            .unwrap();

        assert_eq!(script.present_count(), 1);
        assert_eq!(script.pending_present_count(), 1);
        assert_eq!(script.acknowledged_present_count(), 0);
        assert!(!script.present_ready());
        assert!(script.last_present_feedback().is_none());

        script.push_present_ack(Some(23), Some(99), Some(7));

        assert!(platform.poll_event().is_none());
        assert_eq!(script.pending_present_count(), 0);
        assert_eq!(script.acknowledged_present_count(), 1);
        assert!(script.present_ready());

        let feedback = script.last_present_feedback().unwrap();
        assert_eq!(feedback.acknowledged_present_count, 1);
        assert_eq!(feedback.sequence, Some(23));
        assert_eq!(feedback.timestamp_ns, Some(99));
        assert_eq!(feedback.crtc_id, Some(7));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn queued_mode_acknowledges_pageflip_records_from_drm_fd() {
        let pipe = TestPipe::new();
        let mut platform = queued_platform_with_drm_fd(pipe.read_fd());

        present_one_queued_frame(&mut platform);
        pipe.write_all(&build_vblank_like_record(
            TEST_DRM_EVENT_FLIP_COMPLETE,
            3,
            42_500,
            27,
            9,
        ));

        assert!(platform.can_accept_present());
        assert_eq!(platform.pending_present_count(), 0);
        assert_eq!(platform.acknowledged_present_count(), 1);

        let feedback = platform.take_present_feedback().unwrap();
        assert_eq!(feedback.acknowledged_present_count, 1);
        assert_eq!(feedback.sequence, Some(27));
        assert_eq!(feedback.timestamp_ns, Some(3_042_500_000));
        assert_eq!(feedback.crtc_id, Some(9));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn queued_mode_acknowledges_vblank_records_from_drm_fd() {
        let pipe = TestPipe::new();
        let mut platform = queued_platform_with_drm_fd(pipe.read_fd());

        present_one_queued_frame(&mut platform);
        pipe.write_all(&build_vblank_like_record(
            TEST_DRM_EVENT_VBLANK,
            11,
            125,
            91,
            4,
        ));

        assert!(platform.can_accept_present());
        assert_eq!(platform.pending_present_count(), 0);
        assert_eq!(platform.acknowledged_present_count(), 1);

        let feedback = platform.take_present_feedback().unwrap();
        assert_eq!(feedback.acknowledged_present_count, 1);
        assert_eq!(feedback.sequence, Some(91));
        assert_eq!(feedback.timestamp_ns, Some(11_000_125_000));
        assert_eq!(feedback.crtc_id, Some(4));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn queued_mode_ignores_unknown_drm_records() {
        let pipe = TestPipe::new();
        let mut platform = queued_platform_with_drm_fd(pipe.read_fd());

        present_one_queued_frame(&mut platform);
        pipe.write_all(&build_unknown_record(0x55, &[0xAA, 0xBB, 0xCC, 0xDD]));

        assert!(!platform.can_accept_present());
        assert_eq!(platform.pending_present_count(), 1);
        assert_eq!(platform.acknowledged_present_count(), 0);
        assert!(platform.take_present_feedback().is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn queued_mode_ignores_truncated_drm_records() {
        let pipe = TestPipe::new();
        let mut platform = queued_platform_with_drm_fd(pipe.read_fd());
        let mut truncated = build_vblank_like_record(TEST_DRM_EVENT_FLIP_COMPLETE, 1, 2, 3, 4);

        truncated.truncate(20);
        present_one_queued_frame(&mut platform);
        pipe.write_all(&truncated);

        assert!(!platform.can_accept_present());
        assert_eq!(platform.pending_present_count(), 1);
        assert_eq!(platform.acknowledged_present_count(), 0);
        assert!(platform.take_present_feedback().is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn queued_mode_ignores_unreadable_drm_fds() {
        let mut pipe = TestPipe::new();
        let mut platform = queued_platform_with_drm_fd(pipe.read_fd());

        present_one_queued_frame(&mut platform);
        pipe.close_read_end();

        assert!(!platform.can_accept_present());
        assert_eq!(platform.pending_present_count(), 1);
        assert_eq!(platform.acknowledged_present_count(), 0);
        assert!(platform.take_present_feedback().is_none());
    }

    #[test]
    fn request_redraw_queues_event() {
        let mut platform = StandalonePlatform::new(StandaloneConfig::default()).unwrap();
        let handle = NativeWindowHandle(1);
        platform.request_redraw(handle);
        let event = platform.poll_event();
        assert!(matches!(event, Some(PlatformEvent::WindowRedraw { .. })));
    }

    #[test]
    fn set_cursor_shape_returns_false() {
        let mut platform = StandalonePlatform::new(StandaloneConfig::default()).unwrap();
        let handle = NativeWindowHandle(1);
        assert!(!platform.set_cursor_shape(handle, "arrow"));
    }

    #[test]
    fn window_host_create_and_destroy() {
        let mut platform = StandalonePlatform::new(StandaloneConfig::default()).unwrap();
        let params = crate::window_host::NativeWindowParams {
            title: "Test".to_string(),
            geometry: Rect::new(0.0, 0.0, 1920.0, 1080.0),
            window_type: "normal".to_string(),
            parent: None,
            app_id: "test".to_string(),
        };
        let handle = platform.window_host().create_window(params).unwrap();
        assert!(platform.window_host().destroy_window(handle).is_ok());
    }

    #[test]
    fn keymap_returns_default() {
        let platform = StandalonePlatform::new(StandaloneConfig::default()).unwrap();
        assert_eq!(platform.keymap().platform_name(), "null");
        assert!(platform.keymap().translate_scancode(42).is_none());
    }

    // ------------------------------------------------------------------
    // StandalonePresentSubmitter seam regressions (t30).
    //
    // Linux-only because the submitter trait references `liquide_drm::DrmError`,
    // which is itself a Linux-only dependency of this crate.
    // ------------------------------------------------------------------
    #[cfg(target_os = "linux")]
    pub(crate) struct NoopPresentSubmitter {
        // Per the requested test-helper shape, kept as a public Vec<u64>; the
        // mirror below is what the host test inspects after the box is moved
        // into the platform.
        #[allow(dead_code)]
        pub calls: Vec<u64>,
        // Optional shared mirror so the test owning the platform can still
        // observe invocation counts after the box has been moved into config.
        mirror: Option<Arc<Mutex<Vec<u64>>>>,
    }

    #[cfg(target_os = "linux")]
    impl NoopPresentSubmitter {
        fn with_mirror(mirror: Arc<Mutex<Vec<u64>>>) -> Self {
            Self {
                calls: Vec::new(),
                mirror: Some(mirror),
            }
        }
    }

    #[cfg(target_os = "linux")]
    impl StandalonePresentSubmitter for NoopPresentSubmitter {
        fn submit_present(&mut self, frame_seq: u64) -> Result<(), liquide_drm::DrmError> {
            self.calls.push(frame_seq);
            if let Some(mirror) = self.mirror.as_ref() {
                mirror
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .push(frame_seq);
            }
            Ok(())
        }
    }

    #[cfg(target_os = "linux")]
    struct ScriptedSubmitter {
        shared: Arc<Mutex<ScriptedSubmitterState>>,
    }

    #[cfg(target_os = "linux")]
    #[derive(Default)]
    struct ScriptedSubmitterState {
        calls: Vec<u64>,
        next_error: Option<liquide_drm::DrmError>,
    }

    #[cfg(target_os = "linux")]
    impl StandalonePresentSubmitter for ScriptedSubmitter {
        fn submit_present(&mut self, frame_seq: u64) -> Result<(), liquide_drm::DrmError> {
            let mut state = self
                .shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state.calls.push(frame_seq);
            if let Some(err) = state.next_error.take() {
                return Err(err);
            }
            Ok(())
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn submitter_not_invoked_in_immediate_mode() {
        let mirror: Arc<Mutex<Vec<u64>>> = Arc::new(Mutex::new(Vec::new()));
        let submitter = Box::new(NoopPresentSubmitter::with_mirror(Arc::clone(&mirror)));
        let config = StandaloneConfig::default().with_present_submitter(submitter);
        assert_eq!(config.present_mode, StandalonePresentMode::Immediate);
        let mut platform = StandalonePlatform::new(config).unwrap();

        platform
            .present_frame(
                NativeWindowHandle(1),
                &[1, 2, 3, 4],
                1,
                1,
                4,
                PixelFormat::Bgra8,
            )
            .unwrap();

        let calls = mirror.lock().unwrap_or_else(|p| p.into_inner()).clone();
        assert!(
            calls.is_empty(),
            "submitter must not be invoked in Immediate mode, got {:?}",
            calls
        );
        assert_eq!(platform.present_count(), 1);
        assert_eq!(platform.acknowledged_present_count(), 1);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn submitter_invoked_in_queued_mode_with_scripted_feedback() {
        let mirror: Arc<Mutex<Vec<u64>>> = Arc::new(Mutex::new(Vec::new()));
        let submitter = Box::new(NoopPresentSubmitter::with_mirror(Arc::clone(&mirror)));
        let config = StandaloneConfig {
            present_mode: StandalonePresentMode::Queued,
            ..StandaloneConfig::default()
        }
        .with_present_submitter(submitter);
        let mut platform = StandalonePlatform::new(config).unwrap();
        let script = platform.script_handle();

        platform
            .present_frame(
                NativeWindowHandle(1),
                &[5, 6, 7, 8],
                1,
                1,
                4,
                PixelFormat::Bgra8,
            )
            .unwrap();

        let calls = mirror.lock().unwrap_or_else(|p| p.into_inner()).clone();
        assert_eq!(calls, vec![1u64]);
        assert_eq!(platform.present_count(), 1);
        assert_eq!(platform.pending_present_count(), 1);
        assert_eq!(platform.acknowledged_present_count(), 0);

        // Scripted feedback path still works with a submitter installed.
        script.push_present_ack(Some(11), Some(22), Some(3));
        assert!(platform.poll_event().is_none());
        assert_eq!(platform.acknowledged_present_count(), 1);
        assert_eq!(platform.pending_present_count(), 0);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn submitter_error_does_not_panic_pacing() {
        let state = Arc::new(Mutex::new(ScriptedSubmitterState {
            calls: Vec::new(),
            next_error: Some(liquide_drm::DrmError::PageFlip("scripted".to_string())),
        }));
        let submitter = Box::new(ScriptedSubmitter {
            shared: Arc::clone(&state),
        });
        let config = StandaloneConfig {
            present_mode: StandalonePresentMode::Queued,
            ..StandaloneConfig::default()
        }
        .with_present_submitter(submitter);
        let mut platform = StandalonePlatform::new(config).unwrap();

        let result = platform.present_frame(
            NativeWindowHandle(1),
            &[1, 2, 3, 4],
            1,
            1,
            4,
            PixelFormat::Bgra8,
        );

        assert!(matches!(
            result,
            Err(PlatformError::Presentation(ref msg))
                if msg.contains("standalone present submitter failed")
        ));

        // Frame is NOT recorded as in-flight on submitter error: pacing is
        // not deadlocked, and the next call is accepted.
        assert_eq!(platform.present_count(), 0);
        assert_eq!(platform.pending_present_count(), 0);
        assert_eq!(platform.acknowledged_present_count(), 0);
        assert!(platform.can_accept_present());

        let calls = state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .calls
            .clone();
        assert_eq!(calls, vec![1u64]);

        // A subsequent successful submission proceeds normally.
        let result = platform.present_frame(
            NativeWindowHandle(1),
            &[9, 9, 9, 9],
            1,
            1,
            4,
            PixelFormat::Bgra8,
        );
        assert!(result.is_ok());
        assert_eq!(platform.present_count(), 1);
        assert_eq!(platform.pending_present_count(), 1);
    }
}
