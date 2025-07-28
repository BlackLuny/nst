use crate::{NetworkTestError, Result, Socks5Client};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{interval, timeout};
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct NetworkJitterTest {
    proxy_addr: String,
    targets: Vec<String>,
    ping_interval: Duration,
    test_duration: Duration,
}

#[derive(Debug, Clone)]
pub struct NetworkJitterResult {
    pub total_pings: u64,
    pub successful_pings: u64,
    pub failed_pings: u64,
    pub timeout_pings: u64,
    pub rtt_samples: Vec<Duration>,
    pub average_rtt: Duration,
    pub min_rtt: Duration,
    pub max_rtt: Duration,
    pub median_rtt: Duration,
    pub jitter: Duration,
    pub packet_loss_rate: f64,
    pub target_results: std::collections::HashMap<String, TargetJitterResult>,
}

#[derive(Debug, Clone)]
pub struct TargetJitterResult {
    pub target: String,
    pub total_pings: u64,
    pub successful_pings: u64,
    pub failed_pings: u64,
    pub rtt_samples: Vec<Duration>,
    pub average_rtt: Duration,
    pub jitter: Duration,
    pub packet_loss_rate: f64,
}

#[derive(Debug, Clone)]
struct PingResult {
    pub success: bool,
    pub rtt: Option<Duration>,
    pub _timestamp: Instant,
    pub error: Option<String>,
}

impl NetworkJitterTest {
    pub fn new(
        proxy_addr: &str,
        targets: Vec<String>,
        ping_interval_ms: u64,
        test_duration_sec: u64,
    ) -> Self {
        Self {
            proxy_addr: proxy_addr.to_string(),
            targets,
            ping_interval: Duration::from_millis(ping_interval_ms),
            test_duration: Duration::from_secs(test_duration_sec),
        }
    }

    pub async fn run(&self) -> Result<()> {
        info!("Starting network jitter test");
        info!("Proxy: {}", self.proxy_addr);
        info!("Targets: {:?}", self.targets);
        info!(
            "Ping interval: {:?}, Test duration: {:?}",
            self.ping_interval, self.test_duration
        );

        let proxy_addr = self
            .proxy_addr
            .parse()
            .map_err(|e| NetworkTestError::Config(format!("Invalid proxy address: {e}")))?;

        let client = Socks5Client::new(proxy_addr).with_timeout(Duration::from_secs(10));

        let result = self.run_jitter_test(&client).await?;

        self.print_results(&result);

        Ok(())
    }

    async fn run_jitter_test(&self, client: &Socks5Client) -> Result<NetworkJitterResult> {
        let start_time = Instant::now();
        let end_time = start_time + self.test_duration;

        let mut target_results = std::collections::HashMap::new();
        for target in &self.targets {
            target_results.insert(
                target.clone(),
                TargetJitterResult {
                    target: target.clone(),
                    total_pings: 0,
                    successful_pings: 0,
                    failed_pings: 0,
                    rtt_samples: Vec::new(),
                    average_rtt: Duration::ZERO,
                    jitter: Duration::ZERO,
                    packet_loss_rate: 0.0,
                },
            );
        }

        let mut total_pings = 0u64;
        let mut successful_pings = 0u64;
        let mut failed_pings = 0u64;
        let mut timeout_pings = 0u64;
        let mut all_rtt_samples = Vec::new();

        let mut ping_interval = interval(self.ping_interval);
        let mut target_index = 0;

        while Instant::now() < end_time {
            ping_interval.tick().await;

            if self.targets.is_empty() {
                break;
            }

            let target = &self.targets[target_index % self.targets.len()];
            target_index += 1;

            total_pings += 1;
            let target_result = target_results.get_mut(target).unwrap();
            target_result.total_pings += 1;

            match self.perform_ping(client, target).await {
                Ok(PingResult {
                    success: true,
                    rtt: Some(rtt),
                    ..
                }) => {
                    successful_pings += 1;
                    target_result.successful_pings += 1;
                    target_result.rtt_samples.push(rtt);
                    all_rtt_samples.push(rtt);

                    debug!("Ping to {} successful: {:?}", target, rtt);
                }
                Ok(PingResult {
                    success: false,
                    error: Some(error),
                    ..
                }) => {
                    if error.contains("timeout") {
                        timeout_pings += 1;
                    } else {
                        failed_pings += 1;
                    }
                    target_result.failed_pings += 1;
                    warn!("Ping to {} failed: {}", target, error);
                }
                Err(e) => {
                    failed_pings += 1;
                    target_result.failed_pings += 1;
                    warn!("Ping to {} error: {}", target, e);
                }
                _ => {
                    failed_pings += 1;
                    target_result.failed_pings += 1;
                }
            }
        }

        for target_result in target_results.values_mut() {
            if !target_result.rtt_samples.is_empty() {
                target_result.average_rtt = target_result.rtt_samples.iter().sum::<Duration>()
                    / target_result.rtt_samples.len() as u32;
                target_result.jitter = self.calculate_jitter(&target_result.rtt_samples);
            }

            target_result.packet_loss_rate = if target_result.total_pings > 0 {
                target_result.failed_pings as f64 / target_result.total_pings as f64 * 100.0
            } else {
                0.0
            };
        }

        let average_rtt = if !all_rtt_samples.is_empty() {
            all_rtt_samples.iter().sum::<Duration>() / all_rtt_samples.len() as u32
        } else {
            Duration::ZERO
        };

        let min_rtt = all_rtt_samples
            .iter()
            .min()
            .copied()
            .unwrap_or(Duration::ZERO);
        let max_rtt = all_rtt_samples
            .iter()
            .max()
            .copied()
            .unwrap_or(Duration::ZERO);
        let median_rtt = self.calculate_median(&all_rtt_samples);
        let jitter = self.calculate_jitter(&all_rtt_samples);

        let packet_loss_rate = if total_pings > 0 {
            (failed_pings + timeout_pings) as f64 / total_pings as f64 * 100.0
        } else {
            0.0
        };

        Ok(NetworkJitterResult {
            total_pings,
            successful_pings,
            failed_pings,
            timeout_pings,
            rtt_samples: all_rtt_samples,
            average_rtt,
            min_rtt,
            max_rtt,
            median_rtt,
            jitter,
            packet_loss_rate,
            target_results,
        })
    }

    async fn perform_ping(&self, client: &Socks5Client, target: &str) -> Result<PingResult> {
        let ping_start = Instant::now();

        let ping_result = timeout(
            Duration::from_secs(5),
            self.tcp_ping_via_proxy(client, target),
        )
        .await;

        match ping_result {
            Ok(Ok(())) => {
                let rtt = ping_start.elapsed();
                Ok(PingResult {
                    success: true,
                    rtt: Some(rtt),
                    _timestamp: ping_start,
                    error: None,
                })
            }
            Ok(Err(e)) => Ok(PingResult {
                success: false,
                rtt: None,
                _timestamp: ping_start,
                error: Some(e.to_string()),
            }),
            Err(_) => Ok(PingResult {
                success: false,
                rtt: None,
                _timestamp: ping_start,
                error: Some("timeout".to_string()),
            }),
        }
    }

    async fn tcp_ping_via_proxy(&self, client: &Socks5Client, target: &str) -> Result<()> {
        let mut stream = client.connect(target).await.map_err(|e| {
            NetworkTestError::Connection(format!("Failed to connect to target: {e}"))
        })?;

        let ping_data = b"PING\n";
        stream
            .write_all(ping_data)
            .await
            .map_err(NetworkTestError::Io)?;

        let mut response_buffer = [0u8; 64];
        let bytes_read = timeout(
            Duration::from_millis(1000),
            stream.read(&mut response_buffer),
        )
        .await
        .map_err(|_| NetworkTestError::Timeout("Response timeout".to_string()))?
        .map_err(NetworkTestError::Io)?;

        if bytes_read == 0 {
            return Err(NetworkTestError::Connection(
                "Connection closed by peer".to_string(),
            ));
        }

        debug!("TCP ping to {} successful", target);
        Ok(())
    }

    fn calculate_jitter(&self, rtt_samples: &[Duration]) -> Duration {
        if rtt_samples.len() < 2 {
            return Duration::ZERO;
        }

        let mut jitter_sum = Duration::ZERO;
        let mut jitter_count = 0;

        for i in 1..rtt_samples.len() {
            let diff = rtt_samples[i].abs_diff(rtt_samples[i - 1]);
            jitter_sum += diff;
            jitter_count += 1;
        }

        if jitter_count > 0 {
            jitter_sum / jitter_count as u32
        } else {
            Duration::ZERO
        }
    }

    fn calculate_median(&self, rtt_samples: &[Duration]) -> Duration {
        if rtt_samples.is_empty() {
            return Duration::ZERO;
        }

        let mut sorted_samples = rtt_samples.to_vec();
        sorted_samples.sort();

        let mid = sorted_samples.len() / 2;
        if sorted_samples.len() % 2 == 0 {
            (sorted_samples[mid - 1] + sorted_samples[mid]) / 2
        } else {
            sorted_samples[mid]
        }
    }

    fn calculate_percentile(&self, rtt_samples: &[Duration], percentile: f64) -> Duration {
        if rtt_samples.is_empty() {
            return Duration::ZERO;
        }

        let mut sorted_samples = rtt_samples.to_vec();
        sorted_samples.sort();

        let index = ((percentile / 100.0) * (sorted_samples.len() - 1) as f64).round() as usize;
        sorted_samples[index.min(sorted_samples.len() - 1)]
    }

    fn print_results(&self, result: &NetworkJitterResult) {
        println!("\n=== Network Jitter Test Results ===");
        println!("Test Duration: {:?}", self.test_duration);
        println!("Ping Interval: {:?}", self.ping_interval);
        println!("Tested Targets: {}", self.targets.len());
        println!();

        println!("Overall Statistics:");
        println!("  Total Pings: {}", result.total_pings);
        println!(
            "  Successful: {} ({:.1}%)",
            result.successful_pings,
            if result.total_pings > 0 {
                result.successful_pings as f64 / result.total_pings as f64 * 100.0
            } else {
                0.0
            }
        );
        println!(
            "  Failed: {} ({:.1}%)",
            result.failed_pings,
            if result.total_pings > 0 {
                result.failed_pings as f64 / result.total_pings as f64 * 100.0
            } else {
                0.0
            }
        );
        println!(
            "  Timeouts: {} ({:.1}%)",
            result.timeout_pings,
            if result.total_pings > 0 {
                result.timeout_pings as f64 / result.total_pings as f64 * 100.0
            } else {
                0.0
            }
        );
        println!("  Packet Loss Rate: {:.2}%", result.packet_loss_rate);
        println!();

        if result.successful_pings > 0 {
            println!("Latency Statistics:");
            println!("  Average RTT: {:?}", result.average_rtt);
            println!("  Median RTT: {:?}", result.median_rtt);
            println!("  Min RTT: {:?}", result.min_rtt);
            println!("  Max RTT: {:?}", result.max_rtt);
            println!("  Jitter (Avg Deviation): {:?}", result.jitter);

            let p95_rtt = self.calculate_percentile(&result.rtt_samples, 95.0);
            let p99_rtt = self.calculate_percentile(&result.rtt_samples, 99.0);
            println!("  95th Percentile: {p95_rtt:?}");
            println!("  99th Percentile: {p99_rtt:?}");
            println!();
        }

        println!("Per-Target Results:");
        println!("  Target                    | Pings | Success | Loss% | Avg RTT | Jitter");
        println!("  --------------------------|-------|---------|-------|---------|--------");

        let mut sorted_targets: Vec<_> = result.target_results.iter().collect();
        sorted_targets.sort_by_key(|(target, _)| target.as_str());

        for (target, target_result) in sorted_targets {
            println!(
                "  {:25} | {:5} | {:6.1}% | {:4.1}% | {:6.0}ms | {:5.0}ms",
                self.truncate_target(target, 25),
                target_result.total_pings,
                if target_result.total_pings > 0 {
                    target_result.successful_pings as f64 / target_result.total_pings as f64 * 100.0
                } else {
                    0.0
                },
                target_result.packet_loss_rate,
                target_result.average_rtt.as_millis(),
                target_result.jitter.as_millis()
            );
        }
        println!();

        self.print_network_quality_analysis(result);

        let network_score = self.calculate_network_quality_score(result);
        println!("Network Quality Score: {network_score:.1}/100");
    }

    fn print_network_quality_analysis(&self, result: &NetworkJitterResult) {
        println!("Network Quality Analysis:");

        if result.packet_loss_rate <= 0.1 {
            println!(
                "  ✓ Packet Loss: Excellent ({:.2}%)",
                result.packet_loss_rate
            );
        } else if result.packet_loss_rate <= 1.0 {
            println!("  ⚠ Packet Loss: Good ({:.2}%)", result.packet_loss_rate);
        } else if result.packet_loss_rate <= 5.0 {
            println!("  ⚠ Packet Loss: Fair ({:.2}%)", result.packet_loss_rate);
        } else {
            println!("  ✗ Packet Loss: Poor ({:.2}%)", result.packet_loss_rate);
        }

        if result.average_rtt <= Duration::from_millis(50) {
            println!("  ✓ Average Latency: Excellent ({:?})", result.average_rtt);
        } else if result.average_rtt <= Duration::from_millis(150) {
            println!("  ⚠ Average Latency: Good ({:?})", result.average_rtt);
        } else if result.average_rtt <= Duration::from_millis(300) {
            println!("  ⚠ Average Latency: Fair ({:?})", result.average_rtt);
        } else {
            println!("  ✗ Average Latency: Poor ({:?})", result.average_rtt);
        }

        if result.jitter <= Duration::from_millis(10) {
            println!("  ✓ Jitter: Excellent ({:?})", result.jitter);
        } else if result.jitter <= Duration::from_millis(30) {
            println!("  ⚠ Jitter: Good ({:?})", result.jitter);
        } else if result.jitter <= Duration::from_millis(100) {
            println!("  ⚠ Jitter: Fair ({:?})", result.jitter);
        } else {
            println!("  ✗ Jitter: Poor ({:?})", result.jitter);
        }

        let latency_consistency = self.calculate_latency_consistency(result);
        if latency_consistency >= 0.9 {
            println!("  ✓ Latency Consistency: Excellent");
        } else if latency_consistency >= 0.8 {
            println!("  ⚠ Latency Consistency: Good");
        } else if latency_consistency >= 0.7 {
            println!("  ⚠ Latency Consistency: Fair");
        } else {
            println!("  ✗ Latency Consistency: Poor");
        }

        let target_consistency = self.calculate_target_consistency(result);
        if target_consistency >= 0.9 {
            println!("  ✓ Cross-Target Consistency: Excellent");
        } else if target_consistency >= 0.8 {
            println!("  ⚠ Cross-Target Consistency: Good");
        } else {
            println!("  ✗ Cross-Target Consistency: Poor");
        }

        println!();
    }

    fn calculate_latency_consistency(&self, result: &NetworkJitterResult) -> f64 {
        if result.rtt_samples.len() <= 1 {
            return 1.0;
        }

        let mean_rtt = result.average_rtt.as_secs_f64();
        if mean_rtt == 0.0 {
            return 0.0;
        }

        let variance = result
            .rtt_samples
            .iter()
            .map(|rtt| (rtt.as_secs_f64() - mean_rtt).powi(2))
            .sum::<f64>()
            / result.rtt_samples.len() as f64;

        let std_dev = variance.sqrt();
        let coefficient_of_variation = std_dev / mean_rtt;

        (1.0 - coefficient_of_variation.min(1.0)).max(0.0)
    }

    fn calculate_target_consistency(&self, result: &NetworkJitterResult) -> f64 {
        if result.target_results.len() <= 1 {
            return 1.0;
        }

        let avg_rtts: Vec<f64> = result
            .target_results
            .values()
            .filter(|tr| tr.successful_pings > 0)
            .map(|tr| tr.average_rtt.as_secs_f64())
            .collect();

        if avg_rtts.is_empty() {
            return 0.0;
        }

        let mean_avg_rtt = avg_rtts.iter().sum::<f64>() / avg_rtts.len() as f64;
        if mean_avg_rtt == 0.0 {
            return 1.0;
        }

        let variance = avg_rtts
            .iter()
            .map(|&rtt| (rtt - mean_avg_rtt).powi(2))
            .sum::<f64>()
            / avg_rtts.len() as f64;

        let std_dev = variance.sqrt();
        let coefficient_of_variation = std_dev / mean_avg_rtt;

        (1.0 - coefficient_of_variation.min(1.0)).max(0.0)
    }

    fn calculate_network_quality_score(&self, result: &NetworkJitterResult) -> f64 {
        let packet_loss_score = (100.0 - result.packet_loss_rate * 10.0).max(0.0);

        let latency_score = if result.average_rtt <= Duration::from_millis(50) {
            100.0
        } else if result.average_rtt <= Duration::from_millis(150) {
            80.0
        } else if result.average_rtt <= Duration::from_millis(300) {
            60.0
        } else if result.average_rtt <= Duration::from_millis(500) {
            40.0
        } else {
            20.0
        };

        let jitter_score = if result.jitter <= Duration::from_millis(10) {
            100.0
        } else if result.jitter <= Duration::from_millis(30) {
            80.0
        } else if result.jitter <= Duration::from_millis(100) {
            60.0
        } else if result.jitter <= Duration::from_millis(200) {
            40.0
        } else {
            20.0
        };

        let consistency_score = self.calculate_latency_consistency(result) * 100.0;

        (packet_loss_score * 0.3
            + latency_score * 0.3
            + jitter_score * 0.25 + consistency_score * 0.15).clamp(0.0, 100.0)
    }

    fn truncate_target(&self, target: &str, max_len: usize) -> String {
        if target.len() <= max_len {
            target.to_string()
        } else {
            format!("{}...", &target[..max_len - 3])
        }
    }
}
