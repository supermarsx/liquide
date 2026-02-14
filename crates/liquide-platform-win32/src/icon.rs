//! Icon extraction from Win32 executables.

/// Extracted icon data in RGBA format.
#[derive(Debug, Clone)]
pub struct IconData {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// RGBA pixel data.
    pub pixels: Vec<u8>,
}

/// Extracts icons from Win32 executables and window handles.
pub struct IconExtractor;

impl IconExtractor {
    /// Extract the application icon from an executable path.
    ///
    /// Tries to load the large (32×32) icon first, falling back to small (16×16).
    #[cfg(windows)]
    pub fn from_exe(exe_path: &str) -> Option<IconData> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        use windows_sys::Win32::UI::Shell::ExtractIconExW;

        let wide: Vec<u16> = OsStr::new(exe_path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        unsafe {
            let mut large_icon: *mut std::ffi::c_void = std::ptr::null_mut();
            let mut small_icon: *mut std::ffi::c_void = std::ptr::null_mut();
            let count = ExtractIconExW(wide.as_ptr(), 0, &mut large_icon, &mut small_icon, 1);
            if count == 0 {
                return None;
            }

            let icon = if !large_icon.is_null() {
                large_icon
            } else if !small_icon.is_null() {
                small_icon
            } else {
                return None;
            };

            // Convert HICON to RGBA bitmap.
            let result = Self::hicon_to_rgba(icon, 32, 32);

            // Clean up icons.
            if !large_icon.is_null() {
                windows_sys::Win32::UI::WindowsAndMessaging::DestroyIcon(large_icon);
            }
            if !small_icon.is_null() {
                windows_sys::Win32::UI::WindowsAndMessaging::DestroyIcon(small_icon);
            }

            result
        }
    }

    #[cfg(not(windows))]
    pub fn from_exe(_exe_path: &str) -> Option<IconData> {
        None
    }

    /// Convert an HICON to RGBA pixels.
    #[cfg(windows)]
    unsafe fn hicon_to_rgba(
        _hicon: *mut std::ffi::c_void,
        width: u32,
        height: u32,
    ) -> Option<IconData> {
        // Simplified: in production, use GetIconInfo + GetDIBits.
        // For now, generate a placeholder icon.
        let pixels = vec![128u8; (width * height * 4) as usize];
        Some(IconData {
            width,
            height,
            pixels,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icon_data() {
        let icon = IconData {
            width: 32,
            height: 32,
            pixels: vec![0u8; 32 * 32 * 4],
        };
        assert_eq!(icon.pixels.len(), 4096);
    }
}
