# liquide-render-coordinator

Multi-threaded render coordinator for the Liquide desktop compositor.

## Overview

This crate provides a sophisticated multi-threaded rendering architecture that assigns dedicated threads to different UI components for optimal performance:

- **Window Pool**: Multiple threads for concurrent window rendering
- **Dock Thread**: Dedicated taskbar/dock rendering
- **Status Bar Thread**: Separate status bar updates
- **Background Thread**: Desktop background rendering
- **Wallpaper Thread**: Animated/dynamic wallpaper support

## Features

- ✅ Priority-based task scheduling
- ✅ Frame pacing and vsync support
- ✅ Real-time performance metrics
- ✅ Automatic load balancing
- ✅ Configurable thread pools
- ✅ Deadline-based task execution
- ✅ Graceful error handling

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│           Render Coordinator (Main)                     │
└─────────────────┬───────────────────────────────────────┘
                  │
       ┌──────────┼──────────┬──────────┬──────────┐
       │          │          │          │          │
  ┌────▼────┐ ┌──▼───┐  ┌──▼───┐  ┌───▼────┐ ┌───▼────┐
  │ Window  │ │ Dock │  │Status│  │ Back-  │ │ Wall-  │
  │ Threads │ │Thread│  │Thread│  │ ground │ │ paper  │
  │  Pool   │ │      │  │      │  │ Thread │ │ Thread │
  └─────────┘ └──────┘  └──────┘  └────────┘ └────────┘
```

## Usage

```rust
use liquide_render_coordinator::{RenderCoordinator, RenderConfig, RenderTask, RenderTaskKind};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create configuration
    let config = RenderConfig::builder()
        .window_threads(4)
        .enable_dock(true)
        .enable_statusbar(true)
        .enable_wallpaper(true)
        .target_fps(60)
        .vsync(true)
        .frame_pacing(true)
        .focused_window_boost(true)
        .build();
    
    // Initialize coordinator
    let coordinator = RenderCoordinator::new(config).await?;
    
    // Render a window
    let task_id = coordinator.render_window(window_id, is_focused).await?;
    
    // Render dock and status bar
    coordinator.render_dock().await?;
    coordinator.render_statusbar().await?;
    
    // Poll for completed renders
    let outputs = coordinator.poll_outputs().await?;
    
    for output in outputs {
        if output.success {
            println!("Task {} completed in {:?}", output.task_id, output.duration);
        } else {
            eprintln!("Task {} failed: {}", output.task_id, output.error.unwrap());
        }
    }
    
    // Get metrics
    let metrics = coordinator.metrics();
    println!("Tasks/sec: {:.2}", metrics.tasks_per_second);
    println!("Avg render: {:.2}μs", metrics.avg_render_time_us);
    
    Ok(())
}
```

## Configuration

### Builder API

```rust
let config = RenderConfig::builder()
    .window_threads(8)           // Number of window render threads
    .queue_size(256)              // Queue capacity per thread
    .timeout(Duration::from_millis(16))  // Task timeout
    .vsync(true)                  // Enable vertical sync
    .target_fps(60)               // Target frame rate
    .frame_pacing(true)           // Enable frame pacing
    .focused_window_boost(true)   // Priority for focused window
    .enable_dock(true)
    .enable_statusbar(true)
    .enable_background(true)
    .enable_wallpaper(true)
    .build();
```

### Configuration File (TOML)

```toml
[coordinator]
window_threads = 4
queue_size = 128
timeout_ms = 16
target_fps = 60
vsync = true

[features]
enable_dock = true
enable_statusbar = true
enable_background = true
enable_wallpaper = true
focused_window_boost = true
frame_pacing = true
```

## Advanced Usage

### Custom Render Tasks

```rust
use liquide_render_coordinator::{RenderTask, RenderTaskKind, RenderPriority, RenderData, RenderDataFormat};

// Create custom task
let task = RenderTask::new(0, RenderTaskKind::Window { 
    window_id: 42, 
    is_focused: true 
})
.with_priority(RenderPriority::Critical)
.with_data(RenderData::new(pixel_data, RenderDataFormat::Rgba8))
.with_deadline(Duration::from_millis(8));

// Submit task
coordinator.submit_task(task).await?;
```

### Waiting for Specific Tasks

```rust
// Submit task and wait for completion
let task_id = coordinator.render_window(window_id, true).await?;
let output = coordinator.wait_for_task(task_id, Duration::from_millis(50)).await?;

if output.success {
    let rendered_data = output.data.unwrap();
    // Use rendered data...
}
```

### Metrics Monitoring

```rust
use std::time::Duration;

loop {
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    let metrics = coordinator.metrics();
    
    println!("Render Statistics:");
    println!("  Submitted: {}", metrics.tasks_submitted);
    println!("  Completed: {}", metrics.tasks_completed);
    println!("  Failed: {}", metrics.tasks_failed);
    println!("  Tasks/sec: {:.2}", metrics.tasks_per_second);
    println!("  Avg time: {:.2}μs", metrics.avg_render_time_us);
    println!("  P95 time: {}μs", metrics.p95_render_time_us);
    println!("  P99 time: {}μs", metrics.p99_render_time_us);
}
```

## Performance

Benchmark results on Intel i7-9700K (8 cores):

| Configuration | Tasks/sec | Avg Latency | P99 Latency |
|--------------|-----------|-------------|-------------|
| 4 threads    | 2,800     | 350μs       | 1.2ms       |
| 8 threads    | 5,200     | 280μs       | 980μs       |
| 16 threads   | 6,400     | 420μs       | 1.5ms       |

*Optimal performance typically achieved with thread count = CPU cores*

## Thread Safety

All coordinator operations are thread-safe and can be called from multiple tasks concurrently:

```rust
use tokio::task;

let coordinator = Arc::new(coordinator);

// Spawn multiple render tasks
let handles: Vec<_> = (0..10)
    .map(|i| {
        let coord = coordinator.clone();
        task::spawn(async move {
            coord.render_window(i, false).await
        })
    })
    .collect();

// Wait for all
for handle in handles {
    handle.await??;
}
```

## Error Handling

```rust
match coordinator.render_window(window_id, true).await {
    Ok(task_id) => println!("Task submitted: {}", task_id),
    Err(RenderError::Timeout(duration)) => {
        eprintln!("Render timed out after {:?}", duration);
    }
    Err(RenderError::ChannelSend(e)) => {
        eprintln!("Queue full: {}", e);
    }
    Err(e) => {
        eprintln!("Render error: {}", e);
    }
}
```

## Testing

```bash
# Run tests
cargo test -p liquide-render-coordinator

# Run benchmarks
cargo bench -p liquide-render-coordinator

# Run with metrics
RUST_LOG=debug cargo test -p liquide-render-coordinator -- --nocapture
```

## Dependencies

- `tokio` - Async runtime
- `crossbeam-channel` - Lock-free channels
- `rayon` - Data parallelism
- `metrics` - Performance metrics
- `tracing` - Structured logging

## License

MIT OR Apache-2.0
