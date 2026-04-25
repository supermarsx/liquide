use liquide_a11y::AccessibilityTree;

use crate::{A11yBridgeBackend, A11yBridgeEvent, AnnouncePriority, BridgeError};

// Win32 constants for SystemParametersInfo.
const SPI_GETSCREENREADER: u32 = 0x0046;
const SPI_GETHIGHCONTRAST: u32 = 0x0042;
const SPI_GETCLIENTAREAANIMATION: u32 = 0x1042;

// HIGHCONTRAST flags.
const HCF_HIGHCONTRASTON: u32 = 0x0000_0001;

#[link(name = "user32")]
unsafe extern "system" {
    fn SystemParametersInfoW(
        ui_action: u32,
        ui_param: u32,
        pv_param: *mut std::ffi::c_void,
        f_win_ini: u32,
    ) -> i32;
}

#[link(name = "advapi32")]
unsafe extern "system" {
    fn RegOpenKeyExW(
        h_key: isize,
        lp_sub_key: *const u16,
        ul_options: u32,
        sam_desired: u32,
        phk_result: *mut isize,
    ) -> i32;

    fn RegQueryValueExW(
        h_key: isize,
        lp_value_name: *const u16,
        lp_reserved: *const u32,
        lp_type: *mut u32,
        lp_data: *mut u8,
        lpcb_data: *mut u32,
    ) -> i32;

    fn RegCloseKey(h_key: isize) -> i32;
}

const HKEY_CURRENT_USER: isize = -2_147_483_647; // 0x80000001u32 as isize
const KEY_READ: u32 = 0x20019;

/// UI Automation bridge for Windows.
///
/// Uses Win32 `SystemParametersInfo` for screen reader / high-contrast /
/// reduced-motion detection.  Full UIA provider registration is deferred
/// to a future iteration.
pub struct AccessibilityBridge {
    connected: bool,
    screen_reader_active: bool,
}

impl AccessibilityBridge {
    #[must_use]
    pub fn new() -> Self {
        Self {
            connected: false,
            screen_reader_active: false,
        }
    }

    fn check_screen_reader(&self) -> bool {
        let mut active: i32 = 0;
        let ok = unsafe {
            SystemParametersInfoW(
                SPI_GETSCREENREADER,
                0,
                &raw mut active as *mut std::ffi::c_void,
                0,
            )
        };
        ok != 0 && active != 0
    }

    fn check_reduced_motion(&self) -> bool {
        let mut anim: i32 = 1; // default: animations on
        let ok = unsafe {
            SystemParametersInfoW(
                SPI_GETCLIENTAREAANIMATION,
                0,
                &raw mut anim as *mut std::ffi::c_void,
                0,
            )
        };
        // If the call succeeds and animations are disabled, prefer reduced motion.
        ok != 0 && anim == 0
    }

    fn check_high_contrast(&self) -> bool {
        // HIGHCONTRASTW struct: cbSize (u32), dwFlags (u32), lpszDefaultScheme (*mut u16)
        #[repr(C)]
        struct HighContrastW {
            cb_size: u32,
            dw_flags: u32,
            lpsz_default_scheme: *mut u16,
        }

        let mut hc = HighContrastW {
            cb_size: std::mem::size_of::<HighContrastW>() as u32,
            dw_flags: 0,
            lpsz_default_scheme: std::ptr::null_mut(),
        };
        let ok = unsafe {
            SystemParametersInfoW(
                SPI_GETHIGHCONTRAST,
                hc.cb_size,
                &raw mut hc as *mut std::ffi::c_void,
                0,
            )
        };
        ok != 0 && (hc.dw_flags & HCF_HIGHCONTRASTON) != 0
    }

    fn get_font_scale(&self) -> f32 {
        // Read HKCU\Software\Microsoft\Accessibility\TextScaleFactor (DWORD, percentage).
        let sub_key: Vec<u16> = "Software\\Microsoft\\Accessibility\0"
            .encode_utf16()
            .collect();
        let value_name: Vec<u16> = "TextScaleFactor\0".encode_utf16().collect();

        let mut hkey: isize = 0;
        let rc = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                sub_key.as_ptr(),
                0,
                KEY_READ,
                &raw mut hkey,
            )
        };
        if rc != 0 {
            return 1.0;
        }

        let mut data: u32 = 100;
        let mut data_size: u32 = 4;
        let mut reg_type: u32 = 0;
        let rc = unsafe {
            RegQueryValueExW(
                hkey,
                value_name.as_ptr(),
                std::ptr::null(),
                &raw mut reg_type,
                &raw mut data as *mut u32 as *mut u8,
                &raw mut data_size,
            )
        };
        unsafe {
            RegCloseKey(hkey);
        }

        if rc == 0 { (data as f32) / 100.0 } else { 1.0 }
    }

    fn speak(&self, text: &str, priority: AnnouncePriority) {
        // Use PowerShell speech synthesis as a fallback until full UIA
        // notification support is wired up.
        let interrupt = match priority {
            AnnouncePriority::Assertive => "SpeakPurge",
            AnnouncePriority::Polite => "SpeakAsync",
        };
        // Escape single quotes for PowerShell.
        let escaped = text.replace('\'', "''");
        let script = format!(
            "Add-Type -AssemblyName System.Speech; \
             $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; \
             $s.{interrupt}('{escaped}')"
        );
        let _ = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .spawn();
    }
}

impl Default for AccessibilityBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl A11yBridgeBackend for AccessibilityBridge {
    fn init(&mut self) -> Result<(), BridgeError> {
        self.screen_reader_active = self.check_screen_reader();
        self.connected = true;
        Ok(())
    }

    fn shutdown(&mut self) {
        self.connected = false;
    }

    fn push_events(&mut self, events: &[A11yBridgeEvent]) -> Result<(), BridgeError> {
        if !self.connected {
            return Err(BridgeError::ConnectionFailed("not initialized".into()));
        }

        for event in events {
            match event {
                A11yBridgeEvent::Announce { text, priority } => {
                    if self.screen_reader_active {
                        self.speak(text, *priority);
                    }
                }
                A11yBridgeEvent::FocusChanged { id: _ } => {
                    // Full implementation: UiaRaiseAutomationEvent for focus.
                }
                A11yBridgeEvent::NodeCreated { .. }
                | A11yBridgeEvent::NodeDestroyed { .. }
                | A11yBridgeEvent::NodeChanged { .. }
                | A11yBridgeEvent::ValueChanged { .. } => {
                    // Full implementation: UIA structure-changed / property-changed events.
                }
            }
        }
        Ok(())
    }

    fn sync_tree(&mut self, _tree: &AccessibilityTree) -> Result<(), BridgeError> {
        // Full implementation would build the UIA provider tree from
        // the AccessibilityTree nodes.
        Ok(())
    }

    fn is_screen_reader_active(&self) -> bool {
        self.screen_reader_active
    }

    fn prefers_reduced_motion(&self) -> bool {
        self.check_reduced_motion()
    }

    fn prefers_high_contrast(&self) -> bool {
        self.check_high_contrast()
    }

    fn font_scale(&self) -> f32 {
        self.get_font_scale()
    }
}
