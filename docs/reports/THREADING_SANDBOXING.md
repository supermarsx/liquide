# Threading and Sandboxing Architecture

## Overview

LiquiDE now implements full threading for shell elements and application sandboxing for security isolation.

## Threading Architecture

Each major shell element runs on its own dedicated thread with independent DOM and rendering pipeline:

### Thread Structure

```
Main Thread (Shell Coordinator)
├── Dock Thread (dedicated DOM + CSS pipeline)
├── StatusBar Thread (dedicated DOM + CSS pipeline)  
├── Launcher Thread (dedicated DOM + CSS pipeline)
└── Notification Thread (dedicated DOM + CSS pipeline)
```

### Implementation Details

- **Module**: `crates/liquide-shell/src/threading.rs`
- **Key Type**: `ShellThreadCoordinator`
- Each thread maintains:
  - Its own `DesktopDocument` (DOM tree)
  - Its own `DesktopPipeline` (Style → Layout → Paint)
  - Message passing via `mpsc::channel` for updates
  
### Message Flow

1. **Update Phase**: Main thread sends `ElementUpdate` messages to each thread
2. **Render Phase**: Main thread requests renders via `ElementMessage::Render`
3. **Collection Phase**: Main thread collects `Vec<SceneNode>` from each thread
4. **Composition**: Main thread composites all scene nodes into final frame

### Usage Example

```rust
// In Shell::from_config()
let thread_css = theme_loader::default_theme_css().to_string();
let thread_coordinator = ShellThreadCoordinator::new(
    thread_css,
    screen_width as u32,
    screen_height as u32,
);

// Update dock thread
thread_coordinator.update_dock(dock_items, hover_index);

// Render all elements (non-blocking)
let scene_nodes = thread_coordinator.render_all();
```

## Sandboxing Architecture

Applications are isolated into two security levels:

### Security Levels

1. **System Apps** (`SandboxLevel::System`)
   - Full access to main desktop DOM
   - No isolation
   - Examples: Files, Terminal, Settings, System Monitor
   
2. **Isolated Apps** (`SandboxLevel::Isolated`)
   - Private DOM instance
   - No access to desktop DOM
   - Cannot manipulate shell elements
   - Render to Surface nodes only

### Implementation Details

- **Module**: `crates/liquide-shell/src/sandboxing.rs`
- **Key Type**: `SandboxManager`
- System apps are whitelisted:
  ```rust
  com.liquide.files
  com.liquide.terminal
  com.liquide.settings
  com.liquide.system-monitor
  com.liquide.dock
  com.liquide.statusbar
  com.liquide.launcher
  com.liquide.notifications
  ```

### Sandbox Operations

```rust
let sandbox_manager = SandboxManager::new();

// Register an app (auto-detects system vs isolated)
sandbox_manager.register_app("com.example.app".to_string());

// Check isolation level
if sandbox_manager.is_system_app("com.liquide.files") {
    // Grant desktop DOM access
} else {
    // Use isolated DOM
}

// Use sandbox context
sandbox_manager.with_sandbox("com.example.app", |sandbox| {
    if let Some(doc) = &sandbox.document {
        // Work with isolated DOM
    }
});
```

### Statistics

```rust
let stats = sandbox_manager.stats();
println!("Total: {}, System: {}, Isolated: {}", 
    stats.total, stats.system, stats.isolated);
```

## Integration with Shell

The `Shell` struct now includes:

```rust
pub struct Shell {
    // ... existing fields ...
    
    /// Thread coordinator for shell elements
    thread_coordinator: Option<ShellThreadCoordinator>,
    
    /// Sandbox manager for application isolation
    sandbox_manager: SandboxManager,
}
```

### Initialization

Both systems are initialized automatically in `Shell::from_config()` and `Shell::with_history_capacity()`:

```rust
// Threading
let thread_css = theme_loader::default_theme_css().to_string();
let thread_coordinator = ShellThreadCoordinator::new(
    thread_css,
    screen_width as u32,
    screen_height as u32,
);

// Sandboxing  
let sandbox_manager = SandboxManager::new();
```

## Default Theme Change

The default theme has been changed from **Liquid Glass** to **Night** (OLED-optimized):

- **Old**: `theme_loader::default_liquid_glass_css()`
- **New**: `theme_loader::default_theme_css()` → returns `themes::night::CSS`

### Theme Functions

```rust
pub fn default_theme_css() -> &'static str {
    themes::night::CSS  // Default: Night theme
}

pub fn default_liquid_glass_css() -> &'static str {
    themes::liquid_glass::CSS
}

pub fn night_css() -> &'static str {
    themes::night::CSS
}

pub fn sunset_css() -> &'static str {
    themes::sunset::CSS
}

pub fn midday_css() -> &'static str {
    themes::midday::CSS
}
```

## Performance Characteristics

### Threading Benefits

- **Parallel Rendering**: Dock, statusbar, launcher, and notifications render simultaneously
- **Non-Blocking**: Main thread doesn't wait for slow renders (16ms timeout per element)
- **Isolation**: Crash in one element thread doesn't affect others

### Sandboxing Benefits

- **Security**: Untrusted apps cannot manipulate desktop chrome
- **Stability**: App DOM mutations isolated from system
- **Performance**: Smaller DOMs for isolated apps (faster restyle/layout)

## Testing

All tests pass (690 liquide-shell tests, 52 workspace tests):

```bash
cargo test -p liquide-shell        # 690 passed
cargo test --workspace --lib       # 52 passed
```

New tests added:
- `test_system_app_detection`
- `test_sandbox_registration`
- `test_sandbox_unregistration`
- `test_isolated_app_has_document`
- `test_system_app_no_document`
- `test_sandbox_stats`
- `test_add_custom_system_app`

## Future Enhancements

### Threading
- [ ] Integrate with `liquide-render-coordinator` for GPU acceleration
- [ ] Per-window render threads (currently only for shell chrome)
- [ ] Adaptive thread pool sizing based on CPU count
- [ ] Thread priority scheduling for focused elements

### Sandboxing
- [ ] IPC protocol for app↔shell communication
- [ ] Permission system for filesystem/network access
- [ ] Resource quotas (CPU, memory, GPU) per sandbox
- [ ] App capability manifest (declare required permissions)

## References

- Threading infrastructure: `crates/liquide-render-coordinator/`
- Pipeline architecture: `PIPELINE_SPEC.md`
- Theme system: `crates/liquide-shell/src/themes/`
