//! Export and report generation.

use anyhow::Result;
use std::time::Duration;
use tokio::time;

use crate::collector::TelemetryCollector;
use crate::types::TelemetrySnapshot;

/// Export telemetry data to JSON file.
pub async fn export_json(output_path: &str, duration_secs: u64) -> Result<()> {
    tracing::info!("collecting data for {} seconds...", duration_secs);
    
    let collector = TelemetryCollector::local();
    let mut snapshots = Vec::new();
    
    let mut interval = time::interval(Duration::from_millis(100));
    let end_time = tokio::time::Instant::now() + Duration::from_secs(duration_secs);
    
    while tokio::time::Instant::now() < end_time {
        interval.tick().await;
        
        match collector.collect().await {
            Ok(snapshot) => snapshots.push(snapshot),
            Err(e) => tracing::warn!("collection error: {}", e),
        }
    }
    
    tracing::info!("collected {} snapshots, writing to {}", snapshots.len(), output_path);
    
    let json = serde_json::to_string_pretty(&snapshots)?;
    tokio::fs::write(output_path, json).await?;
    
    tracing::info!("export complete!");
    
    Ok(())
}

/// Generate an HTML performance report.
pub async fn generate_report(output_path: &str, duration_secs: u64) -> Result<()> {
    tracing::info!("collecting data for report ({} seconds)...", duration_secs);
    
    let collector = TelemetryCollector::local();
    let mut snapshots = Vec::new();
    
    let mut interval = time::interval(Duration::from_millis(100));
    let end_time = tokio::time::Instant::now() + Duration::from_secs(duration_secs);
    
    while tokio::time::Instant::now() < end_time {
        interval.tick().await;
        
        match collector.collect().await {
            Ok(snapshot) => snapshots.push(snapshot),
            Err(e) => tracing::warn!("collection error: {}", e),
        }
    }
    
    tracing::info!("collected {} snapshots, generating report...", snapshots.len());
    
    // Calculate statistics
    let stats = compute_statistics(&snapshots);
    
    // Generate HTML
    let html = generate_html_report(&stats, &snapshots);
    
    tokio::fs::write(output_path, html).await?;
    
    tracing::info!("report saved to {}", output_path);
    
    Ok(())
}

/// Statistical summary of telemetry data.
struct TelemetryStats {
    avg_fps: f64,
    min_fps: f64,
    max_fps: f64,
    avg_frame_time: f64,
    min_frame_time: f64,
    max_frame_time: f64,
    p95_frame_time: f64,
    p99_frame_time: f64,
    total_frames: usize,
    health_distribution: std::collections::HashMap<String, usize>,
}

/// Compute statistics from snapshots.
fn compute_statistics(snapshots: &[TelemetrySnapshot]) -> TelemetryStats {
    use crate::types::HealthStatus;
    
    let mut frame_times: Vec<f64> = Vec::new();
    let mut fps_values: Vec<f64> = Vec::new();
    let mut health_dist = std::collections::HashMap::new();
    
    for snapshot in snapshots {
        fps_values.push(snapshot.frames.fps);
        frame_times.extend(snapshot.frames.history.iter());
        
        let health_str = format!("{:?}", snapshot.health);
        *health_dist.entry(health_str).or_insert(0) += 1;
    }
    
    frame_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    fps_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    let p95_idx = (frame_times.len() as f64 * 0.95) as usize;
    let p99_idx = (frame_times.len() as f64 * 0.99) as usize;
    
    TelemetryStats {
        avg_fps: fps_values.iter().sum::<f64>() / fps_values.len().max(1) as f64,
        min_fps: fps_values.first().copied().unwrap_or(0.0),
        max_fps: fps_values.last().copied().unwrap_or(0.0),
        avg_frame_time: frame_times.iter().sum::<f64>() / frame_times.len().max(1) as f64,
        min_frame_time: frame_times.first().copied().unwrap_or(0.0),
        max_frame_time: frame_times.last().copied().unwrap_or(0.0),
        p95_frame_time: frame_times.get(p95_idx).copied().unwrap_or(0.0),
        p99_frame_time: frame_times.get(p99_idx).copied().unwrap_or(0.0),
        total_frames: frame_times.len(),
        health_distribution: health_dist,
    }
}

/// Generate HTML report.
fn generate_html_report(stats: &TelemetryStats, snapshots: &[TelemetrySnapshot]) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Liquide Performance Report</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 40px; background: #f5f5f5; }}
        .container {{ max-width: 1200px; margin: 0 auto; background: white; padding: 30px; border-radius: 8px; }}
        h1 {{ color: #00bcd4; }}
        h2 {{ color: #333; border-bottom: 2px solid #00bcd4; padding-bottom: 10px; }}
        .stat-grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 20px; margin: 20px 0; }}
        .stat-box {{ background: #f9f9f9; padding: 15px; border-radius: 4px; border-left: 4px solid #00bcd4; }}
        .stat-label {{ color: #666; font-size: 14px; }}
        .stat-value {{ font-size: 24px; font-weight: bold; color: #333; }}
        .health-status {{ padding: 10px; margin: 10px 0; border-radius: 4px; }}
        .healthy {{ background: #e8f5e9; color: #2e7d32; }}
        .degraded {{ background: #fff3e0; color: #e65100; }}
        .slow {{ background: #fce4ec; color: #c2185b; }}
        .critical {{ background: #ffebee; color: #c62828; }}
    </style>
</head>
<body>
    <div class="container">
        <h1>Liquide Performance Report</h1>
        <p>Generated: {}</p>
        <p>Duration: {} snapshots</p>

        <h2>Frame Performance</h2>
        <div class="stat-grid">
            <div class="stat-box">
                <div class="stat-label">Average FPS</div>
                <div class="stat-value">{:.1}</div>
            </div>
            <div class="stat-box">
                <div class="stat-label">Min FPS</div>
                <div class="stat-value">{:.1}</div>
            </div>
            <div class="stat-box">
                <div class="stat-label">Max FPS</div>
                <div class="stat-value">{:.1}</div>
            </div>
            <div class="stat-box">
                <div class="stat-label">Avg Frame Time</div>
                <div class="stat-value">{:.2}ms</div>
            </div>
            <div class="stat-box">
                <div class="stat-label">P95 Frame Time</div>
                <div class="stat-value">{:.2}ms</div>
            </div>
            <div class="stat-box">
                <div class="stat-label">P99 Frame Time</div>
                <div class="stat-value">{:.2}ms</div>
            </div>
        </div>

        <h2>Health Distribution</h2>
        {}

        <h2>Summary</h2>
        <p>Total frames captured: {}</p>
        <p>Frame time range: {:.2}ms - {:.2}ms</p>
    </div>
</body>
</html>"#,
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        snapshots.len(),
        stats.avg_fps,
        stats.min_fps,
        stats.max_fps,
        stats.avg_frame_time,
        stats.p95_frame_time,
        stats.p99_frame_time,
        generate_health_html(&stats.health_distribution),
        stats.total_frames,
        stats.min_frame_time,
        stats.max_frame_time
    )
}

/// Generate health distribution HTML.
fn generate_health_html(distribution: &std::collections::HashMap<String, usize>) -> String {
    let mut html = String::new();
    
    for (health, count) in distribution {
        let class = health.to_lowercase();
        html.push_str(&format!(
            r#"<div class="health-status {}">{}: {} snapshots</div>"#,
            class, health, count
        ));
    }
    
    html
}
