# Win32 GDI Compatibility Layer Design

## Overview
Enable Liquide desktop to seamlessly integrate native Win32 applications by intercepting window creation, replicating their chrome and content onto Liquide surfaces, and adding them to the dock.

## Architecture

### 1. Core Components

#### `liquide-platform-win32-compat` (New Crate)
- **Purpose**: Hook Win32 APIs to capture window creation and GDI drawing calls
- **Location**: `crates/liquide-platform-win32-compat/`
- **Dependencies**:
  - `winapi` or `windows-rs` for Win32 API bindings
  - `detours` or `minhook-rs` for API hooking
  - `liquide-platform` for surface integration

#### Hooked APIs
```rust
// Window creation/management
CreateWindowExA/W
CreateWindowA/W
ShowWindow
SetWindowPos
DestroyWindow

// GDI rendering
BeginPaint
EndPaint
BitBlt
StretchBlt
PatBlt
TextOutA/W
DrawTextA/W

// Message handling
GetMessageA/W
PeekMessageA/W
DispatchMessageA/W
```

### 2. Window Replication Pipeline

```
Native Win32 Window Creation
    ↓
Hook intercepts CreateWindowEx
    ↓
Extract window properties:
  - HWND handle
  - Window title
  - Icon resource
  - Size and position
    ↓
Create shadow Liquide window
    ↓
Set up GDI capture:
  - Hook BeginPaint/EndPaint
  - Allocate shared surface buffer
  - Copy GDI context to Liquide surface
    ↓
Add to Shell dock:
  - Extract .exe icon
  - Generate app_id from process path
  - Register with dock manager
```

### 3. GDI Surface Bridge

#### Shared Memory Surface
```rust
pub struct Win32Surface {
    hwnd: HWND,
    hdc: HDC,
    width: u32,
    height: u32,
    
    // Shared BGRA buffer for Liquide compositor
    buffer: Vec<u32>,
    
    // Dirty region tracking
    dirty_rect: Option<Rect>,
    
    // Frame throttling
    last_draw_time: Instant,
    min_frame_interval: Duration,
}

impl Win32Surface {
    /// Capture GDI content from native DC to Liquide buffer
    fn capture_frame(&mut self) -> bool {
        // Use BitBlt to copy from window DC to compatible DC
        // Convert to BGRA format for Liquide
        // Mark dirty region for compositor update
    }
    
    /// Hook entry point for BeginPaint
    fn on_begin_paint(&mut self) {
        // Record drawing operations start
    }
    
    /// Hook entry point for EndPaint
    fn on_end_paint(&mut self) {
        // Capture final frame to Liquide surface
        // Throttle based on config.window_management.anti_flicker_min_frame_interval_ms
    }
}
```

#### Integration with Liquide Compositor
```rust
// In liquide-shell/src/shell.rs
pub struct Shell {
    // ... existing fields ...
    
    /// Win32 native windows being replicated
    win32_windows: HashMap<HWND, Win32WindowState>,
}

pub struct Win32WindowState {
    surface: Win32Surface,
    liquide_window_id: WindowId,
    process_path: PathBuf,
    icon: Option<Vec<u8>>, // Extracted from .exe resources
}
```

### 4. App Menu Integration

#### Icon Extraction
```rust
pub fn extract_exe_icon(exe_path: &Path) -> Option<Vec<u8>> {
    // Use ExtractIconEx from shell32.dll
    // Convert HICON to BGRA pixel data
    // Return 32x32 or 48x48 icon for dock
}
```

#### Dynamic Dock Items
```rust
impl Dock {
    pub fn add_win32_app(&mut self, hwnd: HWND, app_id: String, icon: Vec<u8>) {
        // Create DockItem with:
        // - app_id from exe path (e.g., "win32.notepad.exe")
        // - Dynamic icon from extracted resources
        // - Running state = true
        // - window_count tracking
    }
    
    pub fn remove_win32_app(&mut self, hwnd: HWND) {
        // Called when DestroyWindow hook fires
    }
}
```

### 5. DockClickBehavior Implementation

```rust
// In shell.rs handle_platform_event()
if let Some(dock_item) = clicked_item {
    match self.config.dock.click_running_behavior {
        DockClickBehavior::ToggleMinimize => {
            if dock_item.running {
                // Find associated window(s)
                // Toggle minimize state
            } else {
                // Launch new instance
            }
        }
        DockClickBehavior::AlwaysNew => {
            // Always spawn new process
        }
        DockClickBehavior::SmartToggle => {
            if dock_item.running && dock_item.window_count == 1 {
                // Single window: minimize
            } else if dock_item.running && dock_item.window_count > 1 {
                // Multiple windows: show expose-style overview
            } else {
                // Launch new
            }
        }
        DockClickBehavior::ShowAllWindows => {
            if dock_item.running {
                // Trigger window switcher filtered to this app
            } else {
                // Launch new
            }
        }
    }
}
```

### 6. Anti-Flicker Insurance Thread

```rust
// In liquide-desktop/src/main.rs or compositor
pub struct AntiFlickerGuard {
    min_frame_interval: Duration,
    last_frame_time: Instant,
    pending_redraw: AtomicBool,
}

impl AntiFlickerGuard {
    pub fn request_frame(&self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_frame_time);
        
        if elapsed >= self.min_frame_interval {
            self.pending_redraw.store(false, Ordering::Relaxed);
            true
        } else {
            // Schedule deferred redraw
            self.pending_redraw.store(true, Ordering::Relaxed);
            false
        }
    }
    
    pub fn tick(&mut self) {
        if self.pending_redraw.load(Ordering::Relaxed) {
            let now = Instant::now();
            let elapsed = now.duration_since(self.last_frame_time);
            
            if elapsed >= self.min_frame_interval {
                self.last_frame_time = now;
                self.pending_redraw.store(false, Ordering::Relaxed);
                // Trigger compositor redraw
            }
        }
    }
}
```

### 7. Configuration Defaults

```rust
// Already implemented in config.rs
pub struct WindowManagementConfig {
    pub auto_repatriate: bool, // true
    pub repatriation_threshold_px: f32, // 50.0
    pub anti_flicker_min_frame_interval_ms: u64, // 8 (~120Hz cap)
    pub enable_anti_flicker_insurance: bool, // true
}

pub struct DockConfig {
    pub click_running_behavior: DockClickBehavior, // SmartToggle
    // ... existing fields ...
}

pub enum DockClickBehavior {
    ToggleMinimize,
    AlwaysNew,
    SmartToggle,
    ShowAllWindows,
}
```

## Implementation Phases

### Phase 1: Hook Infrastructure (1-2 days)
- [ ] Create `liquide-platform-win32-compat` crate
- [ ] Set up API hooking library (minhook-rs recommended)
- [ ] Hook CreateWindowEx and DestroyWindow
- [ ] Basic HWND tracking

### Phase 2: Surface Replication (2-3 days)
- [ ] Implement Win32Surface with GDI capture
- [ ] Hook BeginPaint/EndPaint for frame sync
- [ ] Convert GDI DC to BGRA buffer
- [ ] Integrate with Liquide compositor surface API

### Phase 3: Dock Integration (1 day)
- [ ] Extract icons from .exe resources
- [ ] Generate app_id from process path
- [ ] Add/remove Win32 apps to dock dynamically
- [ ] Track window counts per app

### Phase 4: DockClickBehavior (1 day)
- [ ] Implement ToggleMinimize logic
- [ ] Implement SmartToggle with window counting
- [ ] Handle AlwaysNew (process spawning)
- [ ] ShowAllWindows integration with launcher

### Phase 5: Anti-Flicker (1 day)
- [ ] Implement AntiFlickerGuard
- [ ] Integrate with compositor event loop
- [ ] Add frame rate limiting based on config
- [ ] Test with high-frequency window updates

### Phase 6: Testing & Polish (2-3 days)
- [ ] Test with diverse Win32 apps (Notepad, Explorer, VS Code, browsers)
- [ ] Handle edge cases (fullscreen, borderless, topmost windows)
- [ ] Performance profiling and optimization
- [ ] Documentation

## Technical Challenges

### Challenge 1: API Hooking Safety
- **Problem**: Hooking can cause instability if not done carefully
- **Solution**: Use mature library (minhook-rs), hook only necessary APIs, extensive error handling

### Challenge 2: GDI Performance
- **Problem**: Capturing every frame from GDI can be slow
- **Solution**: 
  - Only capture on EndPaint (not every draw call)
  - Use dirty region tracking
  - Enforce minimum frame interval (8ms default)
  - Skip captures if window is minimized/occluded

### Challenge 3: Icon Extraction
- **Problem**: .exe icon extraction needs Win32 shell APIs
- **Solution**: Use `ExtractIconEx` from shell32.dll, fall back to generic icon if extraction fails

### Challenge 4: Window Ownership
- **Problem**: Native Win32 windows have their own message pump
- **Solution**: 
  - Don't try to control native window lifecycle
  - Shadow window in Liquide purely for chrome replication
  - Let native window handle input directly

## Testing Strategy

### Unit Tests
- Win32Surface buffer conversion
- DockClickBehavior state machine
- AntiFlickerGuard timing logic

### Integration Tests
- Launch Notepad, verify dock icon appears
- Click dock icon, verify minimize/restore behavior
- Close native window, verify dock icon removal
- Rapidly update native window, verify no flicker

### Manual Testing
- Run diverse Win32 apps
- Test with multiple monitors
- High DPI scaling scenarios
- Rapid window creation/destruction

## Performance Targets

- **Frame capture overhead**: < 1ms per window per frame
- **Memory overhead**: < 5MB per replicated window
- **CPU usage**: < 2% idle, < 10% during active drawing
- **Frame rate**: Match native app or 120Hz, whichever is lower

## Security Considerations

- Only hook process's own APIs (no global hooks)
- Validate all HWND handles before access
- Boundary checks on buffer sizes
- No elevation required (user-space only)

## Future Enhancements

1. **Wayland/X11 Compatibility**: Extend design to Linux
2. **GPU Acceleration**: Use DirectX shared surfaces instead of BitBlt
3. **Chrome Customization**: Allow Liquide to render custom decorations over native windows
4. **Input Forwarding**: Route input through Liquide for unified gestures
5. **Window Effects**: Apply Liquide glass/blur effects to native windows

## Related Files

- Configuration: [config.rs](crates/liquide-shell/src/config.rs)
- Window repatriation: [shell.rs](crates/liquide-shell/src/shell.rs) lines 2283-2333
- Status bar auto-hide: [shell.rs](crates/liquide-shell/src/shell.rs) lines 2335-2365
- Dock management: [dock.rs](crates/liquide-shell/src/dock.rs)
- Cursor shapes: [scene.rs](crates/liquide-compositor/src/scene.rs) lines 159-185

## Status

✅ Configuration layer complete (WindowManagementConfig, DockClickBehavior)
✅ Window repatriation logic implemented
✅ Status bar auto-hide implemented
✅ App menu dropdown rendering added
✅ Hover bounds validation fixed
⬜ Win32 compat crate creation pending
⬜ GDI hook implementation pending
⬜ Icon extraction pending
⬜ DockClickBehavior handlers pending
⬜ Anti-flicker thread pending
