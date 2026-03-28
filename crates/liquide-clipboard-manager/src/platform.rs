//! Platform-specific clipboard bridge.
//!
//! Each target OS has a concrete implementation behind `cfg` gates.
//! The public API is a thin trait ([`PlatformClipboard`]) plus a factory
//! function [`create_platform_clipboard`] that returns the right
//! implementation.

use crate::entry::ClipboardContent;

/// Errors that can occur during platform clipboard operations.
#[derive(Debug)]
pub enum PlatformClipboardError {
    /// The clipboard could not be opened or is locked by another process.
    OpenFailed(String),
    /// Requested format is not available on the system clipboard.
    FormatUnavailable,
    /// An I/O or process error (e.g. xclip failed).
    IoError(String),
    /// The data retrieved from the OS was not valid (encoding / format).
    InvalidData(String),
}

impl std::fmt::Display for PlatformClipboardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OpenFailed(s) => write!(f, "clipboard open failed: {s}"),
            Self::FormatUnavailable => write!(f, "format not available"),
            Self::IoError(s) => write!(f, "I/O error: {s}"),
            Self::InvalidData(s) => write!(f, "invalid data: {s}"),
        }
    }
}

impl std::error::Error for PlatformClipboardError {}

/// Result type for platform clipboard operations.
pub type PlatformResult<T> = std::result::Result<T, PlatformClipboardError>;

/// Trait for reading/writing the OS clipboard.
pub trait PlatformClipboard {
    /// Read the current contents of the system clipboard.
    fn read(&self) -> PlatformResult<ClipboardContent>;

    /// Write content to the system clipboard.
    fn write(&self, content: &ClipboardContent) -> PlatformResult<()>;

    /// Check whether the system clipboard currently holds any content.
    fn has_content(&self) -> bool;
}

// ---------------------------------------------------------------------------
// Windows implementation (Platform clipboard via windows-sys)
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
mod win32 {
    use super::*;
    use std::ptr;

    use windows_sys::Win32::Foundation::{GlobalFree, HANDLE};
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable,
        OpenClipboard, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
    };
    use windows_sys::Win32::System::Ole::CF_HDROP;

    const CF_UNICODETEXT: u32 = 13;
    const CF_BITMAP: u32 = 2;
    const CF_DIB: u32 = 8;
    const CF_TEXT: u32 = 1;

    pub struct Win32Clipboard;

    impl Win32Clipboard {
        pub fn new() -> Self {
            Self
        }

        /// Open the clipboard, run `f`, then close it.
        fn with_clipboard<F, R>(&self, f: F) -> PlatformResult<R>
        where
            F: FnOnce() -> PlatformResult<R>,
        {
            unsafe {
                if OpenClipboard(ptr::null_mut()) == 0 {
                    return Err(PlatformClipboardError::OpenFailed(
                        "OpenClipboard returned 0".into(),
                    ));
                }
                let result = f();
                CloseClipboard();
                result
            }
        }
    }

    impl PlatformClipboard for Win32Clipboard {
        fn read(&self) -> PlatformResult<ClipboardContent> {
            self.with_clipboard(|| unsafe {
                // Try CF_UNICODETEXT first.
                if IsClipboardFormatAvailable(CF_UNICODETEXT) != 0 {
                    let handle: HANDLE = GetClipboardData(CF_UNICODETEXT);
                    if handle.is_null() {
                        return Err(PlatformClipboardError::FormatUnavailable);
                    }
                    let ptr = GlobalLock(handle as *mut _) as *const u16;
                    if ptr.is_null() {
                        return Err(PlatformClipboardError::InvalidData(
                            "GlobalLock returned null".into(),
                        ));
                    }
                    // Find the null terminator.
                    let mut len = 0usize;
                    while *ptr.add(len) != 0 {
                        len += 1;
                    }
                    let slice = std::slice::from_raw_parts(ptr, len);
                    let text = String::from_utf16_lossy(slice);
                    GlobalUnlock(handle as *mut _);
                    return Ok(ClipboardContent::Text(text));
                }

                // Try CF_TEXT (ANSI) as fallback.
                if IsClipboardFormatAvailable(CF_TEXT) != 0 {
                    let handle: HANDLE = GetClipboardData(CF_TEXT);
                    if handle.is_null() {
                        return Err(PlatformClipboardError::FormatUnavailable);
                    }
                    let ptr = GlobalLock(handle as *mut _) as *const u8;
                    if ptr.is_null() {
                        return Err(PlatformClipboardError::InvalidData(
                            "GlobalLock returned null".into(),
                        ));
                    }
                    let size = GlobalSize(handle as *mut _);
                    let slice = std::slice::from_raw_parts(ptr, size);
                    let end = slice.iter().position(|&b| b == 0).unwrap_or(size);
                    let text = String::from_utf8_lossy(&slice[..end]).into_owned();
                    GlobalUnlock(handle as *mut _);
                    return Ok(ClipboardContent::Text(text));
                }

                // Try CF_DIB for images.
                if IsClipboardFormatAvailable(CF_DIB) != 0 {
                    let handle: HANDLE = GetClipboardData(CF_DIB);
                    if handle.is_null() {
                        return Err(PlatformClipboardError::FormatUnavailable);
                    }
                    let ptr = GlobalLock(handle as *mut _) as *const u8;
                    if ptr.is_null() {
                        return Err(PlatformClipboardError::InvalidData(
                            "GlobalLock returned null".into(),
                        ));
                    }
                    let size = GlobalSize(handle as *mut _);
                    let data = std::slice::from_raw_parts(ptr, size).to_vec();
                    GlobalUnlock(handle as *mut _);

                    // Parse BITMAPINFOHEADER to get dimensions.
                    if data.len() < 16 {
                        return Err(PlatformClipboardError::InvalidData(
                            "DIB header too small".into(),
                        ));
                    }
                    let width =
                        i32::from_le_bytes([data[4], data[5], data[6], data[7]]) as u32;
                    let height_raw =
                        i32::from_le_bytes([data[8], data[9], data[10], data[11]]);
                    let height = height_raw.unsigned_abs();

                    return Ok(ClipboardContent::Image {
                        width,
                        height,
                        data,
                        format: crate::entry::ImageFormat::Bmp,
                    });
                }

                // Try CF_HDROP for file paths.
                if IsClipboardFormatAvailable(CF_HDROP as u32) != 0 {
                    let handle: HANDLE = GetClipboardData(CF_HDROP as u32);
                    if handle.is_null() {
                        return Err(PlatformClipboardError::FormatUnavailable);
                    }
                    let count =
                        windows_sys::Win32::UI::Shell::DragQueryFileW(handle as _, u32::MAX, ptr::null_mut(), 0);
                    let mut paths = Vec::new();
                    for i in 0..count {
                        let len = windows_sys::Win32::UI::Shell::DragQueryFileW(
                            handle as _,
                            i,
                            ptr::null_mut(),
                            0,
                        );
                        let mut buf = vec![0u16; (len + 1) as usize];
                        windows_sys::Win32::UI::Shell::DragQueryFileW(
                            handle as _,
                            i,
                            buf.as_mut_ptr(),
                            len + 1,
                        );
                        buf.truncate(len as usize);
                        paths.push(String::from_utf16_lossy(&buf));
                    }
                    return Ok(ClipboardContent::FilePaths(paths));
                }

                Err(PlatformClipboardError::FormatUnavailable)
            })
        }

        fn write(&self, content: &ClipboardContent) -> PlatformResult<()> {
            self.with_clipboard(|| unsafe {
                EmptyClipboard();

                match content {
                    ClipboardContent::Text(text)
                    | ClipboardContent::RichText {
                        plain_fallback: text,
                        ..
                    } => {
                        // Determine the text to write — for RichText we use the
                        // plain_fallback which is already bound as `text` by the
                        // match arm.
                        let wide: Vec<u16> = text
                            .encode_utf16()
                            .chain(std::iter::once(0))
                            .collect();
                        let bytes = wide.len() * 2;
                        let hmem = GlobalAlloc(GMEM_MOVEABLE, bytes);
                        if hmem.is_null() {
                            return Err(PlatformClipboardError::IoError(
                                "GlobalAlloc failed".into(),
                            ));
                        }
                        let dest = GlobalLock(hmem) as *mut u16;
                        if dest.is_null() {
                            GlobalFree(hmem);
                            return Err(PlatformClipboardError::IoError(
                                "GlobalLock failed".into(),
                            ));
                        }
                        ptr::copy_nonoverlapping(wide.as_ptr(), dest, wide.len());
                        GlobalUnlock(hmem);
                        if SetClipboardData(CF_UNICODETEXT, hmem as HANDLE).is_null() {
                            GlobalFree(hmem);
                            return Err(PlatformClipboardError::IoError(
                                "SetClipboardData failed".into(),
                            ));
                        }
                    }
                    ClipboardContent::Image { data, .. } => {
                        let hmem = GlobalAlloc(GMEM_MOVEABLE, data.len());
                        if hmem.is_null() {
                            return Err(PlatformClipboardError::IoError(
                                "GlobalAlloc failed".into(),
                            ));
                        }
                        let dest = GlobalLock(hmem) as *mut u8;
                        if dest.is_null() {
                            GlobalFree(hmem);
                            return Err(PlatformClipboardError::IoError(
                                "GlobalLock failed".into(),
                            ));
                        }
                        ptr::copy_nonoverlapping(data.as_ptr(), dest, data.len());
                        GlobalUnlock(hmem);
                        if SetClipboardData(CF_DIB, hmem as HANDLE).is_null() {
                            GlobalFree(hmem);
                            return Err(PlatformClipboardError::IoError(
                                "SetClipboardData failed".into(),
                            ));
                        }
                    }
                    ClipboardContent::FilePaths(_) => {
                        // CF_HDROP write requires constructing a DROPFILES
                        // struct; for now we serialise as text/uri-list.
                        let text = match content {
                            ClipboardContent::FilePaths(paths) => paths.join("\n"),
                            _ => unreachable!(),
                        };
                        let wide: Vec<u16> = text
                            .encode_utf16()
                            .chain(std::iter::once(0))
                            .collect();
                        let bytes = wide.len() * 2;
                        let hmem = GlobalAlloc(GMEM_MOVEABLE, bytes);
                        if hmem.is_null() {
                            return Err(PlatformClipboardError::IoError(
                                "GlobalAlloc failed".into(),
                            ));
                        }
                        let dest = GlobalLock(hmem) as *mut u16;
                        if dest.is_null() {
                            GlobalFree(hmem);
                            return Err(PlatformClipboardError::IoError(
                                "GlobalLock failed".into(),
                            ));
                        }
                        ptr::copy_nonoverlapping(wide.as_ptr(), dest, wide.len());
                        GlobalUnlock(hmem);
                        if SetClipboardData(CF_UNICODETEXT, hmem as HANDLE).is_null() {
                            GlobalFree(hmem);
                            return Err(PlatformClipboardError::IoError(
                                "SetClipboardData failed".into(),
                            ));
                        }
                    }
                    ClipboardContent::Color { r, g, b, a } => {
                        // Serialise colour as text "#RRGGBBAA".
                        let hex = format!("#{r:02x}{g:02x}{b:02x}{a:02x}");
                        let wide: Vec<u16> = hex
                            .encode_utf16()
                            .chain(std::iter::once(0))
                            .collect();
                        let bytes = wide.len() * 2;
                        let hmem = GlobalAlloc(GMEM_MOVEABLE, bytes);
                        if hmem.is_null() {
                            return Err(PlatformClipboardError::IoError(
                                "GlobalAlloc failed".into(),
                            ));
                        }
                        let dest = GlobalLock(hmem) as *mut u16;
                        if dest.is_null() {
                            GlobalFree(hmem);
                            return Err(PlatformClipboardError::IoError(
                                "GlobalLock failed".into(),
                            ));
                        }
                        ptr::copy_nonoverlapping(wide.as_ptr(), dest, wide.len());
                        GlobalUnlock(hmem);
                        if SetClipboardData(CF_UNICODETEXT, hmem as HANDLE).is_null() {
                            GlobalFree(hmem);
                            return Err(PlatformClipboardError::IoError(
                                "SetClipboardData failed".into(),
                            ));
                        }
                    }
                    ClipboardContent::Custom { data, .. } => {
                        // Write raw bytes as CF_PRIVATEFIRST (private format).
                        let hmem = GlobalAlloc(GMEM_MOVEABLE, data.len());
                        if hmem.is_null() {
                            return Err(PlatformClipboardError::IoError(
                                "GlobalAlloc failed".into(),
                            ));
                        }
                        let dest = GlobalLock(hmem) as *mut u8;
                        if dest.is_null() {
                            GlobalFree(hmem);
                            return Err(PlatformClipboardError::IoError(
                                "GlobalLock failed".into(),
                            ));
                        }
                        ptr::copy_nonoverlapping(data.as_ptr(), dest, data.len());
                        GlobalUnlock(hmem);
                        // 0x0200 = CF_PRIVATEFIRST
                        if SetClipboardData(0x0200, hmem as HANDLE).is_null() {
                            GlobalFree(hmem);
                            return Err(PlatformClipboardError::IoError(
                                "SetClipboardData failed".into(),
                            ));
                        }
                    }
                }
                Ok(())
            })
        }

        fn has_content(&self) -> bool {
            unsafe {
                IsClipboardFormatAvailable(CF_UNICODETEXT) != 0
                    || IsClipboardFormatAvailable(CF_TEXT) != 0
                    || IsClipboardFormatAvailable(CF_DIB) != 0
                    || IsClipboardFormatAvailable(CF_BITMAP) != 0
                    || IsClipboardFormatAvailable(CF_HDROP as u32) != 0
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Linux implementation (xclip / xsel for X11, wl-copy / wl-paste for Wayland)
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::process::Command;

    /// Detect whether the session is Wayland or X11.
    fn is_wayland() -> bool {
        std::env::var("WAYLAND_DISPLAY").is_ok()
    }

    pub struct LinuxClipboard;

    impl LinuxClipboard {
        pub fn new() -> Self {
            Self
        }

        fn read_x11() -> PlatformResult<String> {
            // Try xclip first, fall back to xsel.
            let output = Command::new("xclip")
                .args(["-selection", "clipboard", "-o"])
                .output()
                .or_else(|_| {
                    Command::new("xsel")
                        .args(["--clipboard", "--output"])
                        .output()
                })
                .map_err(|e| PlatformClipboardError::IoError(e.to_string()))?;

            if !output.status.success() {
                return Err(PlatformClipboardError::FormatUnavailable);
            }
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        }

        fn read_wayland() -> PlatformResult<String> {
            let output = Command::new("wl-paste")
                .args(["--no-newline"])
                .output()
                .map_err(|e| PlatformClipboardError::IoError(e.to_string()))?;

            if !output.status.success() {
                return Err(PlatformClipboardError::FormatUnavailable);
            }
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        }

        fn write_x11(text: &str) -> PlatformResult<()> {
            use std::io::Write;

            let mut child = Command::new("xclip")
                .args(["-selection", "clipboard", "-i"])
                .stdin(std::process::Stdio::piped())
                .spawn()
                .or_else(|_| {
                    Command::new("xsel")
                        .args(["--clipboard", "--input"])
                        .stdin(std::process::Stdio::piped())
                        .spawn()
                })
                .map_err(|e| PlatformClipboardError::IoError(e.to_string()))?;

            if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(text.as_bytes())
                    .map_err(|e| PlatformClipboardError::IoError(e.to_string()))?;
            }
            let status = child
                .wait()
                .map_err(|e| PlatformClipboardError::IoError(e.to_string()))?;
            if !status.success() {
                return Err(PlatformClipboardError::IoError(
                    "clipboard write command failed".into(),
                ));
            }
            Ok(())
        }

        fn write_wayland(text: &str) -> PlatformResult<()> {
            use std::io::Write;

            let mut child = Command::new("wl-copy")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| PlatformClipboardError::IoError(e.to_string()))?;

            if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(text.as_bytes())
                    .map_err(|e| PlatformClipboardError::IoError(e.to_string()))?;
            }
            let status = child
                .wait()
                .map_err(|e| PlatformClipboardError::IoError(e.to_string()))?;
            if !status.success() {
                return Err(PlatformClipboardError::IoError(
                    "wl-copy failed".into(),
                ));
            }
            Ok(())
        }
    }

    impl PlatformClipboard for LinuxClipboard {
        fn read(&self) -> PlatformResult<ClipboardContent> {
            let text = if is_wayland() {
                Self::read_wayland()?
            } else {
                Self::read_x11()?
            };
            Ok(ClipboardContent::Text(text))
        }

        fn write(&self, content: &ClipboardContent) -> PlatformResult<()> {
            let text = content_to_text(content);
            if is_wayland() {
                Self::write_wayland(&text)
            } else {
                Self::write_x11(&text)
            }
        }

        fn has_content(&self) -> bool {
            // Best-effort: try to read — if it succeeds, there is content.
            self.read().is_ok()
        }
    }
}

// ---------------------------------------------------------------------------
// macOS implementation (pbcopy / pbpaste)
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use std::process::Command;

    pub struct MacOSClipboard;

    impl MacOSClipboard {
        pub fn new() -> Self {
            Self
        }
    }

    impl PlatformClipboard for MacOSClipboard {
        fn read(&self) -> PlatformResult<ClipboardContent> {
            let output = Command::new("pbpaste")
                .output()
                .map_err(|e| PlatformClipboardError::IoError(e.to_string()))?;
            if !output.status.success() {
                return Err(PlatformClipboardError::FormatUnavailable);
            }
            Ok(ClipboardContent::Text(
                String::from_utf8_lossy(&output.stdout).into_owned(),
            ))
        }

        fn write(&self, content: &ClipboardContent) -> PlatformResult<()> {
            use std::io::Write;

            let text = content_to_text(content);
            let mut child = Command::new("pbcopy")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| PlatformClipboardError::IoError(e.to_string()))?;

            if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(text.as_bytes())
                    .map_err(|e| PlatformClipboardError::IoError(e.to_string()))?;
            }
            let status = child
                .wait()
                .map_err(|e| PlatformClipboardError::IoError(e.to_string()))?;
            if !status.success() {
                return Err(PlatformClipboardError::IoError(
                    "pbcopy failed".into(),
                ));
            }
            Ok(())
        }

        fn has_content(&self) -> bool {
            self.read().is_ok()
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert any clipboard content to a textual representation for
/// command-line tools that only support text.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn content_to_text(content: &ClipboardContent) -> String {
    match content {
        ClipboardContent::Text(s) => s.clone(),
        ClipboardContent::RichText { plain_fallback, .. } => plain_fallback.clone(),
        ClipboardContent::FilePaths(paths) => paths.join("\n"),
        ClipboardContent::Color { r, g, b, a } => {
            format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
        }
        ClipboardContent::Image { .. } => "[image data]".to_string(),
        ClipboardContent::Custom { mime_type, .. } => {
            format!("[custom: {mime_type}]")
        }
    }
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/// Create the platform clipboard implementation for the current OS.
#[must_use]
pub fn create_platform_clipboard() -> Box<dyn PlatformClipboard> {
    #[cfg(target_os = "windows")]
    {
        Box::new(win32::Win32Clipboard::new())
    }
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::LinuxClipboard::new())
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacOSClipboard::new())
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        Box::new(NullClipboard)
    }
}

/// No-op clipboard for unsupported platforms and testing.
pub struct NullClipboard;

impl PlatformClipboard for NullClipboard {
    fn read(&self) -> PlatformResult<ClipboardContent> {
        Err(PlatformClipboardError::FormatUnavailable)
    }

    fn write(&self, _content: &ClipboardContent) -> PlatformResult<()> {
        Ok(())
    }

    fn has_content(&self) -> bool {
        false
    }
}
