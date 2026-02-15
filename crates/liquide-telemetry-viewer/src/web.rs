//! Web-based telemetry viewer using Axum.

use anyhow::Result;
use axum::{
    extract::State,
    response::{Html, IntoResponse},
    routing::get,
    Json, Router,
};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::collector::TelemetryCollector;
use crate::types::TelemetrySnapshot;

/// Shared application state.
#[derive(Clone)]
struct AppState {
    /// Latest telemetry snapshot.
    latest: Arc<RwLock<TelemetrySnapshot>>,
}

/// Run the web server.
pub async fn run_server(bind: &str, port: u16) -> Result<()> {
    let state = AppState {
        latest: Arc::new(RwLock::new(TelemetrySnapshot::default())),
    };

    // Spawn background collector
    let collector_state = state.clone();
    tokio::spawn(async move {
        let collector = TelemetryCollector::local();
        let _ = collector
            .collect_continuous(100, move |snapshot| {
                let state = collector_state.clone();
                tokio::spawn(async move {
                    *state.latest.write().await = snapshot;
                });
                true
            })
            .await;
    });

    // Build router
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/telemetry", get(telemetry_handler))
        .route("/api/health", get(health_handler))
        .with_state(state);

    // Bind and serve
    let addr = format!("{}:{}", bind, port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    tracing::info!("web viewer listening on http://{}", addr);
    axum::serve(listener, app).await?;

    Ok(())
}

/// Index page handler.
async fn index_handler() -> impl IntoResponse {
    Html(INDEX_HTML)
}

/// Telemetry API endpoint.
async fn telemetry_handler(State(state): State<AppState>) -> impl IntoResponse {
    let snapshot = state.latest.read().await.clone();
    Json(snapshot)
}

/// Health check endpoint.
async fn health_handler(State(state): State<AppState>) -> impl IntoResponse {
    let snapshot = state.latest.read().await;
    Json(serde_json::json!({
        "status": format!("{:?}", snapshot.health),
        "fps": snapshot.frames.fps,
        "avg_frame_time": snapshot.frames.avg_frame_time,
    }))
}

/// Embedded HTML dashboard.
const INDEX_HTML: &str = r#"
<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Liquide Telemetry Dashboard</title>
    <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
            background: #0a0a0a;
            color: #e0e0e0;
            padding: 20px;
        }
        .container {
            max-width: 1400px;
            margin: 0 auto;
        }
        header {
            padding: 20px;
            background: #1a1a1a;
            border-radius: 8px;
            margin-bottom: 20px;
            border: 1px solid #333;
        }
        h1 {
            color: #00bcd4;
            margin-bottom: 10px;
        }
        .status {
            display: flex;
            gap: 20px;
            font-size: 18px;
        }
        .status-item {
            padding: 5px 15px;
            background: #2a2a2a;
            border-radius: 4px;
        }
        .status-healthy { color: #4caf50; }
        .status-degraded { color: #ffeb3b; }
        .status-slow { color: #ff9800; }
        .status-critical { color: #f44336; }
        .grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
            gap: 20px;
            margin-bottom: 20px;
        }
        .card {
            background: #1a1a1a;
            padding: 20px;
            border-radius: 8px;
            border: 1px solid #333;
        }
        .card h2 {
            color: #00bcd4;
            margin-bottom: 15px;
            font-size: 18px;
        }
        .metric {
            display: flex;
            justify-content: space-between;
            padding: 8px 0;
            border-bottom: 1px solid #2a2a2a;
        }
        .metric:last-child { border-bottom: none; }
        .metric-label { color: #888; }
        .metric-value { font-weight: bold; }
        .chart-container {
            position: relative;
            height: 400px;
            margin-top: 20px;
        }
        .window-list {
            max-height: 300px;
            overflow-y: auto;
        }
        .window-item {
            padding: 10px;
            margin: 5px 0;
            background: #2a2a2a;
            border-radius: 4px;
            display: flex;
            justify-content: space-between;
        }
        .window-interactive { border-left: 3px solid #00bcd4; }
    </style>
</head>
<body>
    <div class="container">
        <header>
            <h1>Liquide Telemetry Dashboard</h1>
            <div class="status">
                <div class="status-item" id="health">Status: Loading...</div>
                <div class="status-item" id="fps">FPS: --</div>
                <div class="status-item" id="frame-time">Frame: --ms</div>
            </div>
        </header>

        <div class="grid">
            <div class="card">
                <h2>Frame Metrics</h2>
                <div id="frame-metrics">
                    <div class="metric">
                        <span class="metric-label">Average</span>
                        <span class="metric-value" id="avg-frame">--ms</span>
                    </div>
                    <div class="metric">
                        <span class="metric-label">Minimum</span>
                        <span class="metric-value" id="min-frame">--ms</span>
                    </div>
                    <div class="metric">
                        <span class="metric-label">Maximum</span>
                        <span class="metric-value" id="max-frame">--ms</span>
                    </div>
                    <div class="metric">
                        <span class="metric-label">P95</span>
                        <span class="metric-value" id="p95-frame">--ms</span>
                    </div>
                    <div class="metric">
                        <span class="metric-label">P99</span>
                        <span class="metric-value" id="p99-frame">--ms</span>
                    </div>
                </div>
            </div>

            <div class="card">
                <h2>Thread Pool</h2>
                <div id="thread-metrics">
                    <div class="metric">
                        <span class="metric-label">Active Threads</span>
                        <span class="metric-value" id="active-threads">--</span>
                    </div>
                    <div class="metric">
                        <span class="metric-label">Idle Threads</span>
                        <span class="metric-value" id="idle-threads">--</span>
                    </div>
                    <div class="metric">
                        <span class="metric-label">Queue Depth</span>
                        <span class="metric-value" id="queue-depth">--</span>
                    </div>
                    <div class="metric">
                        <span class="metric-label">Tasks/sec</span>
                        <span class="metric-value" id="tasks-per-sec">--</span>
                    </div>
                </div>
            </div>

            <div class="card">
                <h2>Windows</h2>
                <div class="window-list" id="window-list">
                    <p style="color: #666;">No windows</p>
                </div>
            </div>
        </div>

        <div class="card">
            <h2>Frame Time History</h2>
            <div class="chart-container">
                <canvas id="frame-chart"></canvas>
            </div>
        </div>
    </div>

    <script>
        // Initialize chart
        const ctx = document.getElementById('frame-chart').getContext('2d');
        const chart = new Chart(ctx, {
            type: 'line',
            data: {
                labels: [],
                datasets: [{
                    label: 'Frame Time (ms)',
                    data: [],
                    borderColor: '#00bcd4',
                    backgroundColor: 'rgba(0, 188, 212, 0.1)',
                    borderWidth: 2,
                    tension: 0.4
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                scales: {
                    y: {
                        beginAtZero: true,
                        grid: { color: '#2a2a2a' },
                        ticks: { color: '#888' }
                    },
                    x: {
                        grid: { color: '#2a2a2a' },
                        ticks: { color: '#888' }
                    }
                },
                plugins: {
                    legend: { labels: { color: '#e0e0e0' } }
                }
            }
        });

        // Update dashboard
        async function updateDashboard() {
            try {
                const response = await fetch('/api/telemetry');
                const data = await response.json();

                // Update health status
                const healthEl = document.getElementById('health');
                healthEl.textContent = `Status: ${data.health}`;
                healthEl.className = `status-item status-${data.health.toLowerCase()}`;

                // Update FPS and frame time
                document.getElementById('fps').textContent = `FPS: ${data.frames.fps.toFixed(1)}`;
                document.getElementById('frame-time').textContent = 
                    `Frame: ${data.frames.avg_frame_time.toFixed(2)}ms`;

                // Update frame metrics
                document.getElementById('avg-frame').textContent = 
                    `${data.frames.avg_frame_time.toFixed(2)}ms`;
                document.getElementById('min-frame').textContent = 
                    `${data.frames.min_frame_time.toFixed(2)}ms`;
                document.getElementById('max-frame').textContent = 
                    `${data.frames.max_frame_time.toFixed(2)}ms`;
                document.getElementById('p95-frame').textContent = 
                    `${data.frames.p95_frame_time.toFixed(2)}ms`;
                document.getElementById('p99-frame').textContent = 
                    `${data.frames.p99_frame_time.toFixed(2)}ms`;

                // Update thread metrics
                document.getElementById('active-threads').textContent = data.threads.active_threads;
                document.getElementById('idle-threads').textContent = data.threads.idle_threads;
                document.getElementById('queue-depth').textContent = 
                    data.threads.avg_queue_depth.toFixed(1);
                document.getElementById('tasks-per-sec').textContent = data.threads.tasks_per_second;

                // Update window list
                const windowList = document.getElementById('window-list');
                if (Object.keys(data.windows).length === 0) {
                    windowList.innerHTML = '<p style="color: #666;">No windows</p>';
                } else {
                    windowList.innerHTML = '';
                    Object.entries(data.windows).forEach(([id, metrics]) => {
                        const item = document.createElement('div');
                        item.className = 'window-item' + 
                            (metrics.interactive ? ' window-interactive' : '');
                        item.innerHTML = `
                            <span>Window ${id}</span>
                            <span>${metrics.avg_render_time.toFixed(2)}ms (${metrics.node_count} nodes)</span>
                        `;
                        windowList.appendChild(item);
                    });
                }

                // Update chart
                if (data.frames.history.length > 0) {
                    chart.data.labels = data.frames.history.map((_, i) => i);
                    chart.data.datasets[0].data = data.frames.history;
                    chart.update('none');
                }

            } catch (error) {
                console.error('Failed to update dashboard:', error);
            }
        }

        // Update every 100ms
        setInterval(updateDashboard, 100);
        updateDashboard();
    </script>
</body>
</html>
"#;
