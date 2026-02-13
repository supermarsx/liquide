# liquide-telemetry-viewer

Real-time performance monitoring and debugging tool for Liquide desktop environment.

## Overview

The Liquide Telemetry Viewer provides comprehensive performance visualization and monitoring for the Liquide desktop, helping developers identify bottlenecks, track frame rates, and ensure optimal user experience.

## Features

- **Real-time Frame Time Graphs** - Visualize rendering performance
- **Per-Window Metrics** - Track individual window rendering costs
- **System Health Status** - Automatic classification (Healthy/Degraded/Slow/Critical)
- **Thread Pool Monitoring** - Track thread utilization and queue depth
- **Multiple Output Formats** - TUI, Web, JSON, HTML reports
- **Historical Data** - Track performance over time
- **Live Updates** - 100ms refresh rate for real-time monitoring

## Installation

```bash
cargo install --path crates/liquide-telemetry-viewer
```

## Usage

### Interactive TUI Dashboard

Launch the terminal-based interactive dashboard:

```bash
liquide-telemetry tui
```

Options:
- `--refresh <MS>` - Set refresh rate (default: 100ms)
- `--remote <ADDR>` - Connect to remote session (e.g., `192.168.1.100:8080`)

**Controls:**
- `q` or `ESC` - Quit
- Arrow keys - Navigate (future enhancement)

### Web-Based Viewer

Start a web server with live-updating dashboard:

```bash
liquide-telemetry web --port 8080 --bind 127.0.0.1
```

Then open http://localhost:8080 in your browser.

Features:
- Real-time chart updates
- Responsive design
- Color-coded health status
- Per-window breakdown
- Thread pool metrics

### Export to JSON

Collect and export telemetry data:

```bash
liquide-telemetry export --output telemetry.json --duration 60
```

Collects data for 60 seconds and exports to JSON format.

### Generate HTML Report

Create a comprehensive HTML performance report:

```bash
liquide-telemetry report --output report.html --duration 60
```

The report includes:
- Frame performance statistics
- Health distribution over time
- 95th/99th percentile metrics
- Min/max/average values

## Metrics Explained

### Frame Metrics

- **FPS** - Frames per second (target: 60)
- **Frame Time** - Time to render one frame in milliseconds (target: <16ms)
- **P95/P99** - 95th and 99th percentile frame times (identifies outliers)
- **Min/Max** - Best and worst frame times observed

### Window Metrics

- **Window ID** - Unique identifier for each window
- **Render Time** - Time to render this window's content
- **Node Count** - Number of scene graph nodes
- **Interactive** - Whether window is currently being dragged/resized

### Thread Pool Metrics

- **Active Threads** - Currently executing tasks
- **Idle Threads** - Available for work
- **Queue Depth** - Pending tasks waiting for threads
- **Tasks/sec** - Task throughput

### Health Status

**Healthy** - All frame times < 16ms (60 FPS maintained)  
**Degraded** - Some frame drops, 16-25ms range  
**Slow** - Noticeable lag, 25-50ms range  
**Critical** - Severe issues, > 50ms frame times  

## Architecture

The telemetry system uses a simple file-based communication:

1. **Session Process** writes telemetry data to `/tmp/liquide-telemetry.json` (Linux) or `%TEMP%\liquide-telemetry.json` (Windows)
2. **Viewer** reads and parses this file periodically
3. **Format**: JSON snapshot with frame metrics, window data, health status

### Telemetry Data Format

```json
{
  "timestamp": 1234567890,
  "frames": {
    "fps": 60.0,
    "avg_frame_time": 12.5,
    "min_frame_time": 8.2,
    "max_frame_time": 18.7,
    "p95_frame_time": 15.1,
    "p99_frame_time": 16.8,
    "history": [12.1, 13.2, 11.8, ...]
  },
  "windows": {
    "1": {
      "window_id": 1,
      "avg_render_time": 5.2,
      "node_count": 45,
      "interactive": true,
      "render_history": [5.1, 5.3, 5.0, ...]
    }
  },
  "health": "Healthy",
  "threads": {
    "active_threads": 4,
    "idle_threads": 8,
    "avg_queue_depth": 2.3,
    "tasks_per_second": 120
  }
}
```

## Session Integration

To enable telemetry in your session process:

```rust
use liquide_session::telemetry::{TelemetrySnapshot, export_telemetry};

// Create snapshot
let snapshot = TelemetrySnapshot {
    timestamp: now(),
    frames: frame_metrics,
    windows: window_metrics,
    health: calculate_health(),
    threads: thread_metrics,
};

// Export to standard location
export_telemetry(&snapshot)?;
```

## Examples

### Monitor Production Deployment

```bash
# Start web viewer on server
liquide-telemetry web --port 8080 --bind 0.0.0.0

# Access from remote machine
curl http://server:8080/api/telemetry
```

### Collect Performance Data for Analysis

```bash
# Collect 5 minutes of data
liquide-telemetry export --output perf-$(date +%Y%m%d-%H%M%S).json --duration 300
```

### Generate Daily Reports

```bash
#!/bin/bash
# daily-report.sh
DATE=$(date +%Y-%m-%d)
liquide-telemetry report --output "/reports/perf-$DATE.html" --duration 3600
```

### Debug Slow Frames

1. Launch TUI: `liquide-telemetry tui`
2. Watch for spikes in the frame time graph
3. Check window list to see which window is causing issues
4. Observe health status transitions

## Troubleshooting

### "Telemetry file not found"

The session process hasn't started or isn't writing telemetry data. Check:
- Session process is running
- Telemetry export is enabled in session code
- File permissions on `/tmp` or `%TEMP%`

### Web viewer shows stale data

- Check that session process is actively updating the telemetry file
- Verify file modification timestamp
- Ensure no file locking issues

### High CPU usage from viewer

- Increase refresh rate: `--refresh 500` (500ms)
- Use export mode instead of live viewing for long-term collection
- Check that multiple viewer instances aren't running

## Performance Impact

The telemetry system is designed to have minimal overhead:
- **Session overhead**: < 0.1ms per frame (telemetry collection)
- **File writes**: Asynchronous, non-blocking
- **Viewer impact**: Zero (reads data passively)

## Future Enhancements

- [ ] Network-based telemetry streaming
- [ ] Historical database storage (InfluxDB, Prometheus)
- [ ] Alert system for performance degradation
- [ ] Per-app breakdown
- [ ] GPU metrics integration
- [ ] Memory usage tracking
- [ ] Network bandwidth monitoring
- [ ] Power consumption metrics

## License

MIT
