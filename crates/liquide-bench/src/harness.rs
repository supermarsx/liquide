//! Benchmark harness that orchestrates benchmark runs.
//!
//! Each suite method generates deterministic synthetic timing data based on
//! workload parameters for repeatable, testable benchmarks.

use tracing::info;

use crate::config::BenchConfig;
use crate::measurement::BenchMetrics;
use crate::network::{NetworkEmulator, NetworkPreset};
use crate::report::BenchResult;
use crate::slo::SloSet;
use crate::workload::WorkloadParams;

/// Orchestrates a single benchmark suite run.
#[derive(Debug)]
pub struct BenchHarness {
    config: BenchConfig,
    workload: WorkloadParams,
    network: NetworkEmulator,
    metrics: BenchMetrics,
}

impl BenchHarness {
    /// Create a new harness with the given configuration.
    #[must_use]
    pub fn new(config: &BenchConfig) -> Self {
        let network_preset =
            NetworkPreset::from_name(&config.network_profile).unwrap_or(NetworkPreset::Lan);
        Self {
            config: config.clone(),
            workload: WorkloadParams::default(),
            network: NetworkEmulator::from_preset(network_preset),
            metrics: BenchMetrics::new(),
        }
    }

    /// Create a harness with custom workload parameters.
    #[must_use]
    pub fn with_workload(config: &BenchConfig, workload: WorkloadParams) -> Self {
        let network_preset =
            NetworkPreset::from_name(&config.network_profile).unwrap_or(NetworkPreset::Lan);
        Self {
            config: config.clone(),
            workload,
            network: NetworkEmulator::from_preset(network_preset),
            metrics: BenchMetrics::new(),
        }
    }

    /// Run the compositor benchmark suite.
    ///
    /// Simulates frame composition by generating deterministic timing data
    /// based on workload parameters. Measures compose time, damage
    /// computation time, and frame throughput.
    pub fn run_compositor_suite(&mut self) -> crate::Result<BenchResult> {
        info!("Running compositor benchmark suite");
        self.metrics = BenchMetrics::new();
        self.network.reset();

        let iterations = self.config.iterations;
        let damaged_tiles = self.workload.damaged_tiles_per_frame();
        let damage_fraction = self.workload.profile.damage_fraction();

        for i in 0..iterations {
            let timestamp = i as u64 * 16_667; // ~60fps in microseconds

            // Simulate compose time: base cost + per-tile cost.
            // Base cost: 2.0ms, per damaged tile: 0.05ms.
            let compose_time_ms =
                2.0 + (damaged_tiles as f64 * 0.05) + Self::deterministic_jitter(i, 0.3);
            self.metrics
                .record("compose_time", timestamp, compose_time_ms);

            // Simulate damage computation time: proportional to total tiles.
            let damage_time_ms = 0.5 + (damage_fraction * 1.0) + Self::deterministic_jitter(i, 0.1);
            self.metrics
                .record("damage_compute_time", timestamp, damage_time_ms);

            // Simulate input-to-photon latency: compose + damage + overhead.
            let input_to_photon = compose_time_ms
                + damage_time_ms
                + 3.0
                + self.network.simulated_rtt(i as u64) * 0.5
                + Self::deterministic_jitter(i, 0.5);
            self.metrics
                .record("input_to_photon", timestamp, input_to_photon);

            // Simulate cursor latency: fast path.
            let cursor_latency = 1.5 + Self::deterministic_jitter(i, 0.3);
            self.metrics.record("cursor", timestamp, cursor_latency);

            // Simulate FPS.
            let fps = 60.0 - (damage_fraction * 5.0) + Self::deterministic_jitter(i, 1.0);
            self.metrics.record("fps", timestamp, fps);
        }

        // Simulate first-frame time.
        let first_frame_ms = 150.0 + (damage_fraction * 100.0);
        self.metrics.record("first_frame", 0, first_frame_ms);

        self.build_result("compositor")
    }

    /// Run the encoder benchmark suite.
    ///
    /// Simulates tile encoding by generating deterministic timing data.
    /// Measures encode time, compression ratio, and encoding throughput.
    pub fn run_encoder_suite(&mut self) -> crate::Result<BenchResult> {
        info!("Running encoder benchmark suite");
        self.metrics = BenchMetrics::new();
        self.network.reset();

        let iterations = self.config.iterations;
        let damaged_tiles = self.workload.damaged_tiles_per_frame();
        let tile_bytes = (self.workload.tile_size * self.workload.tile_size * 4) as f64;

        for i in 0..iterations {
            let timestamp = i as u64 * 16_667;

            // Encode time: base cost + per-tile.
            let encode_time_ms =
                0.5 + (damaged_tiles as f64 * 0.1) + Self::deterministic_jitter(i, 0.2);
            self.metrics
                .record("encode_time", timestamp, encode_time_ms);

            // Compression ratio: depends on workload (more change = worse ratio).
            let damage_fraction = self.workload.profile.damage_fraction();
            let compression_ratio =
                3.0 + (1.0 - damage_fraction) * 7.0 + Self::deterministic_jitter(i, 0.5);
            self.metrics
                .record("compression_ratio", timestamp, compression_ratio);

            // Compressed size per tile in bytes.
            let compressed_size = tile_bytes / compression_ratio;
            self.metrics
                .record("compressed_size", timestamp, compressed_size);

            // Total bytes per frame.
            let frame_bytes = compressed_size * damaged_tiles as f64;
            self.metrics.record("frame_bytes", timestamp, frame_bytes);

            // Encoding throughput in megabytes/sec.
            let throughput_mbps = if encode_time_ms > 0.0 {
                (frame_bytes / (encode_time_ms / 1000.0)) / (1024.0 * 1024.0)
            } else {
                0.0
            };
            self.metrics
                .record("encode_throughput_mbps", timestamp, throughput_mbps);
        }

        self.build_result("encoder")
    }

    /// Run the protocol benchmark suite.
    ///
    /// Simulates protocol frame serialization and deserialization.
    /// Measures serialize/deserialize times and protocol throughput.
    pub fn run_protocol_suite(&mut self) -> crate::Result<BenchResult> {
        info!("Running protocol benchmark suite");
        self.metrics = BenchMetrics::new();
        self.network.reset();

        let iterations = self.config.iterations;
        let rtt_base = self.network.profile().rtt_ms;

        for i in 0..iterations {
            let timestamp = i as u64 * 16_667;

            // Serialize time: relatively constant.
            let serialize_time_us = 50.0 + Self::deterministic_jitter(i, 10.0);
            self.metrics
                .record("serialize_time_us", timestamp, serialize_time_us);

            // Deserialize time: slightly more than serialize.
            let deserialize_time_us = 60.0 + Self::deterministic_jitter(i, 12.0);
            self.metrics
                .record("deserialize_time_us", timestamp, deserialize_time_us);

            // Round-trip time including network.
            let rtt = self.network.simulated_rtt(i as u64);
            self.metrics.record("rtt", timestamp, rtt);

            // Protocol overhead (serialize + deserialize as fraction of RTT).
            let protocol_overhead_pct = if rtt > 0.0 {
                ((serialize_time_us + deserialize_time_us) / 1000.0) / rtt * 100.0
            } else {
                0.0
            };
            self.metrics
                .record("protocol_overhead_pct", timestamp, protocol_overhead_pct);

            // Simulate packet loss.
            let dropped = if self.network.should_drop_packet() {
                1.0
            } else {
                0.0
            };
            self.metrics.record("packet_dropped", timestamp, dropped);

            // Messages per second estimate.
            let msg_per_sec = if rtt > 0.0 {
                1000.0 / rtt * (1.0 - self.network.profile().packet_loss_percent / 100.0)
            } else {
                100_000.0
            };
            self.metrics
                .record("messages_per_sec", timestamp, msg_per_sec);

            // Effective bandwidth utilisation.
            let effective_bw = self.network.effective_bandwidth();
            let bandwidth_mbps = effective_bw / (1024.0 * 1024.0) * 8.0;
            self.metrics
                .record("bandwidth_mbps", timestamp, bandwidth_mbps);

            // Simulate input-to-photon for protocol layer (rtt/2 + processing).
            let input_to_photon = rtt_base / 2.0
                + (serialize_time_us + deserialize_time_us) / 1000.0
                + 3.0
                + Self::deterministic_jitter(i, 0.5);
            self.metrics
                .record("input_to_photon", timestamp, input_to_photon);

            // FPS estimate under this network condition.
            let fps = if rtt_base < 20.0 {
                60.0 + Self::deterministic_jitter(i, 1.0)
            } else if rtt_base < 100.0 {
                60.0 - (rtt_base - 20.0) * 0.05 + Self::deterministic_jitter(i, 1.0)
            } else {
                55.0 - (rtt_base - 100.0) * 0.02 + Self::deterministic_jitter(i, 1.5)
            };
            self.metrics.record("fps", timestamp, fps.max(1.0));

            // First frame time: RTT + setup overhead.
            if i == 0 {
                let first_frame = rtt_base + 100.0;
                self.metrics.record("first_frame", 0, first_frame);
            }

            // Cursor latency through protocol path.
            let cursor = rtt_base / 2.0 + 1.0 + Self::deterministic_jitter(i, 0.2);
            self.metrics.record("cursor", timestamp, cursor);
        }

        self.build_result("protocol")
    }

    /// Build a `BenchResult` from the currently recorded metrics.
    fn build_result(&self, suite_name: &str) -> crate::Result<BenchResult> {
        let summaries = self.metrics.summary();
        let slo_set = self.select_slo_set();
        let slo_results = slo_set.check_all(&self.metrics);
        let passed = slo_set.all_passed(&slo_results);

        Ok(BenchResult {
            suite_name: suite_name.to_string(),
            workload: self.workload.profile.label().to_string(),
            samples: self.config.iterations,
            metrics: summaries,
            slo_results,
            passed,
        })
    }

    /// Select the appropriate SLO set based on the network profile.
    fn select_slo_set(&self) -> SloSet {
        match self.config.network_profile.as_str() {
            "lan" | "datacenter" => SloSet::default_lan(),
            _ => SloSet::default_wan(),
        }
    }

    /// Generate deterministic jitter for the given iteration.
    ///
    /// Produces a value in the range `[-amplitude, +amplitude]` that varies
    /// smoothly across iterations.
    fn deterministic_jitter(iteration: u32, amplitude: f64) -> f64 {
        // Use a simple hash-like function to produce deterministic variation.
        let x = iteration as f64;
        let raw = ((x * 2.654_435_761).fract() - 0.5) * 2.0;
        raw * amplitude
    }
}
