//! Win32 platform backend — placeholder wrapping NullPlatform.
//! The agent will replace this entire file with the real implementation.

use crate::{
    DisplayBackend, KeymapTranslator, NativeDragDrop, NativeNotifications, NativeWindowHost,
    NativeTray, NullPlatform, PlatformBackend, PlatformResult, TaskbarIntegration,
};

/// Placeholder — wraps [`NullPlatform`] until the real Win32 implementation
/// replaces this file.
pub struct Win32Platform {
    inner: NullPlatform,
}

impl Win32Platform {
    pub fn new() -> PlatformResult<Self> {
        Ok(Self { inner: NullPlatform::new() })
    }
}

impl PlatformBackend for Win32Platform {
    fn display(&self) -> &dyn DisplayBackend { self.inner.display() }
    fn window_host(&mut self) -> &mut dyn NativeWindowHost { self.inner.window_host() }
    fn taskbar(&mut self) -> &mut dyn TaskbarIntegration { self.inner.taskbar() }
    fn tray(&mut self) -> &mut dyn NativeTray { self.inner.tray() }
    fn notifications(&mut self) -> &mut dyn NativeNotifications { self.inner.notifications() }
    fn drag_drop(&mut self) -> &mut dyn NativeDragDrop { self.inner.drag_drop() }
    fn keymap(&self) -> &dyn KeymapTranslator { self.inner.keymap() }
    fn platform_name(&self) -> &str { "win32-placeholder" }
}
