# Quick Start Guide

## Cursor System

### For Application Developers

```rust
use liquide_cursor::{CursorState, CursorShape, ResizeDirection};

// Create and manage cursor state
let mut cursor = CursorState::new(100.0, 200.0);

// Standard cursor shapes
cursor.set_shape(CursorShape::Arrow);        // Default
cursor.set_shape(CursorShape::Pointer);      // Clickable
cursor.set_shape(CursorShape::Text);         // Text editing

// Resize cursors for window edges
cursor.set_shape(CursorShape::Resize(ResizeDirection::North));
cursor.set_shape(CursorShape::Resize(ResizeDirection::NorthWest));

// Show/hide
cursor.hide();
cursor.show();

// Custom cursor from image
let image_data = load_cursor_image("custom.png"); // Your loading code
cursor.set_custom_image(1, image_data, 32, 32, 16, 16)?;
```

### For Compositor/Shell Developers

```rust
// In your hit testing code
fn get_cursor_for_zone(zone: HitZone) -> CursorShape {
    match zone {
        HitZone::ResizeTop => CursorShape::Resize(ResizeDirection::North),
        HitZone::ResizeTopLeft => CursorShape::Resize(ResizeDirection::NorthWest),
        HitZone::Button => CursorShape::Pointer,
        HitZone::TitleBar => CursorShape::Arrow,
        _ => CursorShape::Arrow,
    }
}
```

### Animated Cursors

```rust
use liquide_cursor::{AnimatedCursorBuilder, CursorShape};

let animated = AnimatedCursorBuilder::new(1, CursorShape::Wait)
    .add_frame(frame1, 32, 32, 16, 16, 100) // 100ms per frame
    .add_frame(frame2, 32, 32, 16, 16, 100)
    .add_frame(frame3, 32, 32, 16, 16, 100)
    .build();

// In your render loop
if animated.update(delta_time_ms) {
    animated.apply_to_state(&mut cursor_state);
}
```

## Telemetry Viewer

### Quick Monitor

Watch performance in real-time:

```bash
# Terminal dashboard (best for quick checks)
liquide-telemetry tui

# Web dashboard (best for remote monitoring)
liquide-telemetry web
# Then open: http://localhost:8080
```

### Data Collection

Collect performance data for analysis:

```bash
# 1 minute collection
liquide-telemetry export -o session-perf.json -d 60

# 5 minute HTML report
liquide-telemetry report -o perf-report.html -d 300
```

### Continuous Monitoring

Set up continuous monitoring with systemd or Windows service:

**Linux systemd:**
```ini
[Unit]
Description=Liquide Telemetry Web Viewer
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/liquide-telemetry web --port 8080
Restart=always

[Install]
WantedBy=multi-user.target
```

**Windows Task Scheduler:**
```powershell
$action = New-ScheduledTaskAction -Execute 'liquide-telemetry.exe' -Argument 'web --port 8080'
$trigger = New-ScheduledTaskTrigger -AtStartup
Register-ScheduledTask -Action $action -Trigger $trigger -TaskName "Liquide Telemetry"
```

### Integration with Session

Add to your session main loop:

```rust
use liquide_session::telemetry::export_telemetry;

// In your render loop
if frame_count % 10 == 0 {  // Every 10 frames (~166ms at 60fps)
    if let Ok(telemetry) = self.telemetry.read() {
        let snapshot = telemetry.snapshot();
        let _ = export_telemetry(&snapshot);
    }
}
```

## Performance Tuning Tips

### Using Telemetry Data

1. **Identify Slow Windows**
   - Check per-window metrics in TUI/Web viewer
   - Windows with high render times are bottlenecks
   - Look at node counts - high counts = complex scenes

2. **Track Frame Spikes**
   - Watch the frame time graph
   - Spikes indicate expensive operations
   - Check if spikes correlate with window interactions

3. **Monitor Thread Pool**
   - Idle threads = underutilized
   - High queue depth = thread starvation
   - Active threads should match CPU cores

4. **Health Status Trends**
   - Track health transitions over time
   - Degraded → Slow = performance worsening
   - Use reports to identify patterns

### Optimization Workflow

```bash
# 1. Start monitoring
liquide-telemetry tui

# 2. Reproduce performance issue
# (interact with desktop normally)

# 3. Identify problem
# - High frame times?
# - Specific window slow?
# - Thread pool saturated?

# 4. Generate detailed report
liquide-telemetry report -o issue-$(date +%s).html -d 60

# 5. Analyze report
# - Check P99 frame times
# - Identify worst-case scenarios
# - Review health distribution
```

## Troubleshooting

### Telemetry File Not Found

```bash
# Check if session is running
ps aux | grep liquid-session

# Check telemetry file exists
ls -la /tmp/liquide-telemetry.json      # Linux
dir %TEMP%\liquide-telemetry.json       # Windows

# Manually trigger telemetry export in session code
```

### Cursor Not Changing

```rust
// Verify cursor state is being updated
println!("Cursor shape: {:?}", cursor.shape());

// Check if cursor rendering is enabled
renderer.set_cursor_enabled(true);

// Ensure cursor position is within bounds
assert!(cursor.x >= 0.0 && cursor.x < width as f32);
```

### High Telemetry Overhead

```rust
// Reduce export frequency
if frame_count % 60 == 0 {  // Once per second at 60fps
    export_telemetry(&snapshot);
}

// Or disable in production
#[cfg(debug_assertions)]
{
    export_telemetry(&snapshot);
}
```

## Command Reference

### Cursor (Library API)

```rust
// Creation
CursorState::new(x, y)
CursorState::default()

// State management
cursor.set_position(x, y)
cursor.set_shape(shape)
cursor.set_visibility(visibility)
cursor.show() / cursor.hide()
cursor.is_visible()

// Custom images
cursor.set_custom_image(id, data, w, h, hx, hy)?

// Scaling
cursor.scale = 2.0
cursor.effective_size()
cursor.effective_hotspot()
```

### Telemetry (CLI)

```bash
# TUI mode
liquide-telemetry tui [--refresh MS] [--remote ADDR]

# Web mode
liquide-telemetry web [--port PORT] [--bind ADDR]

# Export mode
liquide-telemetry export --output FILE [--duration SECS]

# Report mode
liquide-telemetry report --output FILE [--duration SECS]
```

## Best Practices

### Cursor Management

1. **Update cursor based on hit testing**
   ```rust
   let zone = hit_test(mouse_x, mouse_y);
   let new_shape = cursor_for_zone(zone);
   if cursor.shape != new_shape {
       cursor.set_shape(new_shape);
   }
   ```

2. **Hide cursor during full-screen video**
   ```rust
   if video_playing && fullscreen {
       cursor.hide();
   }
   ```

3. **Use appropriate shapes for context**
   - Text fields → `CursorShape::Text`
   - Buttons/links → `CursorShape::Pointer`
   - Busy operations → `CursorShape::Wait`
   - Drag operations → `CursorShape::Grabbing`

### Performance Monitoring

1. **Monitor during development**
   - Run TUI viewer in split terminal
   - Watch for performance regressions

2. **Profile before release**
   - Generate 5-minute reports
   - Check P99 < 16ms
   - Ensure health status "Healthy"

3. **Production monitoring**
   - Deploy web viewer on monitoring instance
   - Set up automated reporting
   - Alert on health degradation

## Examples Repository

See `examples/` directory for:
- `cursor_demo.rs` - Interactive cursor showcase
- `telemetry_gen.rs` - Generate sample telemetry data
- `theme_viewer.rs` - Browse cursor themes

## Support

For issues or questions:
- GitHub Issues: https://github.com/liquide/liquide
- Documentation: https://docs.liquide.dev
- Discord: https://discord.gg/liquide
