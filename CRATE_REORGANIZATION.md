# Crate Reorganization Plan

## Overview
Restructure liquide-shell to extract platform-specific and reusable components into independent crates for better modularity, testability, and cross-platform support.

## 1. Extract Status Bar to `liquide-status-bar`

### Motivation
- Status bar is currently embedded in `liquide-shell` (~527 lines)
- Could be reused by alternative shell implementations
- Contains complex features (app menu, auto-hide, notification integration)
- Deserves independent maintenance and testing

### New Crate Structure
```
crates/liquide-status-bar/
├── Cargo.toml
├── src/
│   ├── lib.rs           # Public API and StatusBar widget
│   ├── config.rs        # StatusBarConfig
│   ├── slot.rs          # StatusBarSlot enum
│   ├── items.rs         # StatusBarItemKind variants
│   ├── app_menu.rs      # App menu dropdown logic
│   ├── builder.rs       # Scene graph builder
│   └── tests/
│       ├── config_tests.rs
│       ├── app_menu_tests.rs
│       └── auto_hide_tests.rs
```

### API Design
```rust
pub struct StatusBar {
    config: StatusBarConfig,
    clock_text: String,
    notification_count: u32,
    app_menu_state: Option<AppMenuState>,
    dirty: bool,
}

impl StatusBar {
    pub fn new(config: StatusBarConfig) -> Self;
    pub fn update_clock(&mut self, now_us: u64);
    pub fn update_notification_count(&mut self, count: u32);
    pub fn open_app_menu(&mut self, app_id: String);
    pub fn close_app_menu(&mut self);
    pub fn toggle_app_menu(&mut self, app_id: String);
    pub fn should_auto_hide(&self, has_maximized: bool, cursor_y: f32) -> bool;
    pub fn build_scene(&self, screen: Rect, theme: &Theme) -> SceneNode;
    pub fn handle_click(&mut self, x: f32, y: f32, screen: Rect) -> Option<StatusBarAction>;
}

pub enum StatusBarAction {
    OpenAppMenu(String),
    CloseAppMenu,
    SessionMenuRequested,
    NotificationsCenterRequested,
}
```

### Migration Steps
1. Create new crate: `cargo new --lib crates/liquide-status-bar`
2. Move `status_bar.rs` → `liquide-status-bar/src/lib.rs`
3. Extract app menu logic from `shell.rs` → `app_menu.rs`
4. Update `liquide-shell/Cargo.toml`:
   ```toml
   [dependencies]
   liquide-status-bar = { path = "../liquide-status-bar" }
   ```
5. Update imports in `shell.rs`:
   ```rust
   use liquide_status_bar::{StatusBar, StatusBarConfig, StatusBarAction};
   ```
6. Remove `status_bar.rs` from `liquide-shell/src/`
7. Update tests and verify compilation

### Affected Files
- **Create**: `crates/liquide-status-bar/` (new crate)
- **Modify**: `crates/liquide-shell/Cargo.toml`
- **Modify**: `crates/liquide-shell/src/shell.rs` (remove status bar scene building)
- **Modify**: `crates/liquide-shell/src/lib.rs` (remove `pub mod status_bar;`)
- **Delete**: `crates/liquide-shell/src/status_bar.rs`

---

## 2. Create `liquide-platform-win32` for Win32 GDI Compatibility

### Motivation
- Windows native app integration is platform-specific
- Should not bloat `liquide-shell` with Windows-only code
- Enables conditional compilation and Windows feature gating
- Provides clean separation for maintenance

### New Crate Structure
```
crates/liquide-platform-win32/
├── Cargo.toml            # Optional feature: "gdi-compat"
├── src/
│   ├── lib.rs            # Public API and Win32Platform trait
│   ├── hooks.rs          # API hooking (CreateWindowEx, BeginPaint, etc.)
│   ├── surface.rs        # Win32Surface with GDI capture
│   ├── icon.rs           # Icon extraction from .exe resources
│   ├── chrome.rs         # Window chrome replication
│   ├── bridge.rs         # Integration with liquide-compositor
│   └── tests/
│       ├── surface_tests.rs
│       └── icon_tests.rs
├── examples/
│   ├── notepad_capture.rs
│   └── icon_extract.rs
└── README.md
```

### Dependencies
```toml
[dependencies]
liquide-compositor = { path = "../liquide-compositor" }
liquide-shell = { path = "../liquide-shell" }
liquide-platform = { path = "../liquide-platform" }

[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = [
    "Win32_Foundation",
    "Win32_Graphics_Gdi",
    "Win32_UI_WindowsAndMessaging",
    "Win32_System_LibraryLoader",
] }
minhook = "0.1"  # API hooking

[features]
default = []
gdi-compat = []  # Enable Win32 GDI compatibility layer
```

### API Design
```rust
#[cfg(windows)]
pub struct Win32Platform {
    windows: HashMap<HWND, Win32WindowState>,
    icon_cache: IconCache,
    config: Win32Config,
}

#[cfg(windows)]
impl Win32Platform {
    pub fn new(config: Win32Config) -> Result<Self>;
    pub fn install_hooks(&mut self) -> Result<()>;
    pub fn uninstall_hooks(&mut self) -> Result<()>;
    pub fn capture_window(&mut self, hwnd: HWND) -> Result<()>;
    pub fn get_replicated_windows(&self) -> Vec<Win32WindowState>;
    pub fn extract_icon(&self, exe_path: &Path) -> Result<Vec<u8>>;
}

pub struct Win32WindowState {
    pub hwnd: HWND,
    pub surface: Win32Surface,
    pub liquide_window_id: WindowId,
    pub app_id: String,
    pub icon: Option<Vec<u8>>,
}

pub struct Win32Surface {
    hwnd: HWND,
    width: u32,
    height: u32,
    buffer: Vec<u32>,  // BGRA pixel data
    dirty_rect: Option<Rect>,
}

impl Win32Surface {
    pub fn capture_frame(&mut self) -> bool;
    pub fn resize(&mut self, width: u32, height: u32);
    pub fn as_slice(&self) -> &[u32];
}
```

### Integration with liquide-shell
```rust
// In liquide-shell/src/shell.rs
#[cfg(windows)]
use liquide_platform_win32::{Win32Platform, Win32Config};

pub struct Shell {
    // ... existing fields ...
    
    #[cfg(windows)]
    win32_platform: Option<Win32Platform>,
}

impl Shell {
    #[cfg(windows)]
    pub fn enable_win32_compat(&mut self) -> Result<()> {
        let config = Win32Config::default();
        let mut platform = Win32Platform::new(config)?;
        platform.install_hooks()?;
        self.win32_platform = Some(platform);
        Ok(())
    }
    
    pub fn tick(&mut self, now_us: u64) -> bool {
        // ... existing tick logic ...
        
        #[cfg(windows)]
        if let Some(ref mut win32) = self.win32_platform {
            for win_state in win32.get_replicated_windows() {
                // Add to dock if not present
                // Update surfaces
            }
        }
        
        // ...
    }
}
```

### Migration Steps
1. Create new crate: `cargo new --lib crates/liquide-platform-win32`
2. Implement basic structure:
   - `lib.rs`: Public API and feature gates
   - `surface.rs`: Win32Surface with GDI BitBlt capture
   - `hooks.rs`: minhook-based API hooking
3. Add Windows-specific dependencies
4. Implement icon extraction using `ExtractIconEx`
5. Add to workspace `Cargo.toml`:
   ```toml
   [workspace]
   members = [
       # ... existing members ...
       "crates/liquide-platform-win32",
   ]
   ```
6. Update `liquide-shell` to optionally use Win32Platform
7. Write integration tests (mocked on non-Windows)
8. Document usage in README

### Conditional Compilation Strategy
```rust
// Provide no-op stubs on non-Windows platforms
#[cfg(not(windows))]
pub struct Win32Platform;

#[cfg(not(windows))]
impl Win32Platform {
    pub fn new(_config: Win32Config) -> Result<Self> {
        Err(Error::PlatformUnsupported)
    }
}
```

### Affected Files
- **Create**: `crates/liquide-platform-win32/` (new crate)
- **Modify**: `Cargo.toml` (workspace members)
- **Modify**: `crates/liquide-shell/Cargo.toml` (optional dependency)
- **Modify**: `crates/liquide-shell/src/shell.rs` (optional Win32Platform field)

---

## 3. Implementation Timeline

### Phase 1: Status Bar Extraction (1-2 days)
- [ ] Create `liquide-status-bar` crate boilerplate
- [ ] Move existing status_bar.rs code
- [ ] Extract app menu dropdown logic from shell.rs
- [ ] Implement click handling and action dispatch
- [ ] Write unit tests for new crate
- [ ] Update liquide-shell imports
- [ ] Verify all tests pass

### Phase 2: Win32 Platform Crate (3-5 days)
- [ ] Create `liquide-platform-win32` crate skeleton
- [ ] Implement Win32Surface with GDI capture
- [ ] Add minhook-based API hooking
- [ ] Implement icon extraction
- [ ] Write integration with liquide-compositor
- [ ] Add conditional compilation guards
- [ ] Write examples and documentation
- [ ] Test on Windows (Notepad, Explorer)

### Phase 3: Integration & Testing (1-2 days)
- [ ] Wire liquide-status-bar into Shell
- [ ] Add Win32Platform optional field to Shell
- [ ] Update configuration to enable/disable features
- [ ] Run full test suite
- [ ] Manual testing on Windows and Linux
- [ ] Update all documentation

---

## 4. Benefits

### Modularity
- Clear separation of concerns
- Easier to understand and maintain
- Independent versioning possible

### Testability
- Status bar testable in isolation
- Win32 code can be unit tested with mocks
- Reduced coupling between components

### Cross-Platform Support
- Windows-specific code cleanly isolated
- No `#[cfg(windows)]` scattered throughout shell
- Easier to add macOS platform crate later

### Reusability
- Status bar can be used in alternative shells
- Win32 platform layer could support other compositors
- Better API contracts

### Compilation Time
- Smaller crates = faster incremental builds
- Optional features reduce default build size

---

## 5. Compatibility & Migration

### Backwards Compatibility
- Public API of `liquide-shell` remains unchanged
- Configuration types stay compatible
- Only internal implementation changes

### Feature Flags
```toml
# In liquide-shell/Cargo.toml
[features]
default = ["status-bar"]
status-bar = ["liquide-status-bar"]
win32-compat = ["liquide-platform-win32"]
```

### Gradual Migration
1. Extract status bar first (lower risk)
2. Test thoroughly before Win32 extraction
3. Win32 crate starts as optional experimental feature
4. Both crates stabilize independently

---

## References

- [WIN32_COMPAT_DESIGN.md](WIN32_COMPAT_DESIGN.md) - Full Win32 architecture
- [IMPLEMENTATION_SUMMARY.md](IMPLEMENTATION_SUMMARY.md) - Recent shell enhancements
- [GAP_ANALYSIS.md](GAP_ANALYSIS.md) - Updated implementation status
