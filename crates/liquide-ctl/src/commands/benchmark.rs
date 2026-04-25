use serde::{Deserialize, Serialize};

use crate::cli::BenchmarkArgs;
use crate::client::{ApiResponse, Client};
use crate::error::Result;
use crate::output::Output;

#[derive(Debug, Serialize, Deserialize)]
struct BenchmarkRequest {
    quick: bool,
    full: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct BenchmarkResult {
    pub score: f64,
    pub latency_ms: f64,
    pub throughput_mbps: f64,
    pub duration_seconds: f64,
}

pub async fn execute(client: &Client, output: &Output, args: &BenchmarkArgs) -> Result<()> {
    output.message("Running LiquiDE Performance Benchmark...");
    let req = BenchmarkRequest {
        quick: args.quick,
        full: args.full,
    };
    let resp: ApiResponse<BenchmarkResult> = client.post("/api/v1/benchmark", &req).await?;
    match resp.data {
        Some(result) => {
            output.message(&format!("  Score:       {:.1}", result.score));
            output.message(&format!("  Latency:     {:.2} ms", result.latency_ms));
            output.message(&format!(
                "  Throughput:  {:.1} Mbps",
                result.throughput_mbps
            ));
            output.message(&format!("  Duration:    {:.1}s", result.duration_seconds));
            output.success("Benchmark complete.");
        }
        None => {
            if let Some(err) = resp.error {
                output.error(&err);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_benchmark_result() {
        let json = r#"{
            "success": true,
            "data": {
                "score": 95.5,
                "latency_ms": 1.23,
                "throughput_mbps": 850.0,
                "duration_seconds": 12.5
            }
        }"#;
        let resp: ApiResponse<BenchmarkResult> = serde_json::from_str(json).unwrap();
        assert!(resp.success);
        let data = resp.data.unwrap();
        assert!((data.score - 95.5).abs() < f64::EPSILON);
        assert!((data.throughput_mbps - 850.0).abs() < f64::EPSILON);
    }

    #[test]
    fn serialize_benchmark_request() {
        let req = BenchmarkRequest {
            quick: true,
            full: false,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["quick"], true);
        assert_eq!(json["full"], false);
    }
}
