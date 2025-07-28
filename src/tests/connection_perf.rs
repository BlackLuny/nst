use crate::{NetworkTestError, Result, Socks5Client};
use futures::future::join_all;
use std::time::{Duration, Instant};
use tokio::time::{sleep, timeout};
use tracing::{debug, info};

#[derive(Debug, Clone)]
pub struct ConnectionPerfTest {
    proxy_addr: String,
    target_addr: String,
    concurrent_connections: usize,
    total_connections: usize,
}

#[derive(Debug, Clone)]
pub struct ConnectionPerfResult {
    pub total_attempts: usize,
    pub successful_connections: usize,
    pub failed_connections: usize,
    pub connection_times: Vec<Duration>,
    pub socks5_handshake_times: Vec<Duration>,
    pub target_connect_times: Vec<Duration>,
    pub concurrent_test_results: Vec<ConcurrentTestResult>,
    pub average_connection_time: Duration,
    pub min_connection_time: Duration,
    pub max_connection_time: Duration,
    pub connection_success_rate: f64,
}

#[derive(Debug, Clone)]
pub struct ConcurrentTestResult {
    pub concurrent_level: usize,
    pub successful_connections: usize,
    pub failed_connections: usize,
    pub average_time: Duration,
    pub total_time: Duration,
}

#[derive(Debug, Clone)]
struct ConnectionAttempt {
    pub success: bool,
    pub total_time: Duration,
    pub socks5_time: Option<Duration>,
    pub target_time: Option<Duration>,
    pub _error: Option<String>,
    pub _timestamp: Instant,
}

impl ConnectionPerfTest {
    pub fn new(proxy_addr: &str, target_addr: &str, concurrent: usize, total: usize) -> Self {
        Self {
            proxy_addr: proxy_addr.to_string(),
            target_addr: target_addr.to_string(),
            concurrent_connections: concurrent,
            total_connections: total,
        }
    }

    pub async fn run(&self) -> Result<()> {
        info!("Starting connection performance test");
        info!("Proxy: {}, Target: {}", self.proxy_addr, self.target_addr);
        info!(
            "Concurrent: {}, Total: {}",
            self.concurrent_connections, self.total_connections
        );

        let proxy_addr = self
            .proxy_addr
            .parse()
            .map_err(|e| NetworkTestError::Config(format!("Invalid proxy address: {e}")))?;

        let client = Socks5Client::new(proxy_addr).with_timeout(Duration::from_secs(10));

        let result = self.run_connection_perf_test(&client).await?;

        self.print_results(&result);

        Ok(())
    }

    async fn run_connection_perf_test(
        &self,
        client: &Socks5Client,
    ) -> Result<ConnectionPerfResult> {
        info!("Running sequential connection test");
        let sequential_results = self.run_sequential_test(client).await;

        info!("Running concurrent connection tests");
        let concurrent_results = self.run_concurrent_tests(client).await;

        let successful_connections = sequential_results.iter().filter(|r| r.success).count();
        let failed_connections = sequential_results.len() - successful_connections;

        let connection_times: Vec<Duration> = sequential_results
            .iter()
            .filter(|r| r.success)
            .map(|r| r.total_time)
            .collect();

        let socks5_handshake_times: Vec<Duration> = sequential_results
            .iter()
            .filter_map(|r| r.socks5_time)
            .collect();

        let target_connect_times: Vec<Duration> = sequential_results
            .iter()
            .filter_map(|r| r.target_time)
            .collect();

        let average_connection_time = if !connection_times.is_empty() {
            connection_times.iter().sum::<Duration>() / connection_times.len() as u32
        } else {
            Duration::ZERO
        };

        let min_connection_time = connection_times
            .iter()
            .min()
            .copied()
            .unwrap_or(Duration::ZERO);
        let max_connection_time = connection_times
            .iter()
            .max()
            .copied()
            .unwrap_or(Duration::ZERO);

        let connection_success_rate = if self.total_connections > 0 {
            successful_connections as f64 / self.total_connections as f64 * 100.0
        } else {
            0.0
        };

        Ok(ConnectionPerfResult {
            total_attempts: self.total_connections,
            successful_connections,
            failed_connections,
            connection_times,
            socks5_handshake_times,
            target_connect_times,
            concurrent_test_results: concurrent_results,
            average_connection_time,
            min_connection_time,
            max_connection_time,
            connection_success_rate,
        })
    }

    async fn run_sequential_test(&self, client: &Socks5Client) -> Vec<ConnectionAttempt> {
        let mut results = Vec::with_capacity(self.total_connections);

        for i in 0..self.total_connections {
            debug!(
                "Sequential connection attempt {}/{}",
                i + 1,
                self.total_connections
            );

            let _start_time = Instant::now();
            let result = self.attempt_single_connection(client).await;

            results.push(result);

            if i < self.total_connections - 1 {
                sleep(Duration::from_millis(100)).await;
            }
        }

        results
    }

    async fn run_concurrent_tests(&self, client: &Socks5Client) -> Vec<ConcurrentTestResult> {
        let mut results = Vec::new();
        let concurrent_levels = vec![2, 5, 10, 20, 50];

        for &concurrent_level in &concurrent_levels {
            if concurrent_level > self.total_connections {
                continue;
            }

            info!("Testing {} concurrent connections", concurrent_level);

            let test_start = Instant::now();
            let mut tasks = Vec::new();

            for _ in 0..concurrent_level {
                let client_clone = client.clone();
                let target_addr = self.target_addr.clone();

                let task = tokio::spawn(async move {
                    Self::attempt_single_connection_static(&client_clone, &target_addr).await
                });

                tasks.push(task);
            }

            let concurrent_results = join_all(tasks).await;
            let test_duration = test_start.elapsed();

            let successful = concurrent_results
                .iter()
                .filter(|r| r.is_ok() && r.as_ref().unwrap().success)
                .count();
            let failed = concurrent_level - successful;

            let successful_times: Vec<Duration> = concurrent_results
                .iter()
                .filter_map(|r| r.as_ref().ok())
                .filter(|r| r.success)
                .map(|r| r.total_time)
                .collect();

            let average_time = if !successful_times.is_empty() {
                successful_times.iter().sum::<Duration>() / successful_times.len() as u32
            } else {
                Duration::ZERO
            };

            results.push(ConcurrentTestResult {
                concurrent_level,
                successful_connections: successful,
                failed_connections: failed,
                average_time,
                total_time: test_duration,
            });
        }

        results
    }

    async fn attempt_single_connection(&self, client: &Socks5Client) -> ConnectionAttempt {
        Self::attempt_single_connection_static(client, &self.target_addr).await
    }

    async fn attempt_single_connection_static(
        client: &Socks5Client,
        target_addr: &str,
    ) -> ConnectionAttempt {
        let start_time = Instant::now();
        let timestamp = start_time;

        match timeout(Duration::from_secs(15), client.connect(target_addr)).await {
            Ok(Ok(_stream)) => {
                let total_time = start_time.elapsed();

                ConnectionAttempt {
                    success: true,
                    total_time,
                    socks5_time: Some(total_time),
                    target_time: None,
                    _error: None,
                    _timestamp: timestamp,
                }
            }
            Ok(Err(e)) => {
                let total_time = start_time.elapsed();

                ConnectionAttempt {
                    success: false,
                    total_time,
                    socks5_time: None,
                    target_time: None,
                    _error: Some(e.to_string()),
                    _timestamp: timestamp,
                }
            }
            Err(_) => {
                let total_time = start_time.elapsed();

                ConnectionAttempt {
                    success: false,
                    total_time,
                    socks5_time: None,
                    target_time: None,
                    _error: Some("Connection timeout".to_string()),
                    _timestamp: timestamp,
                }
            }
        }
    }

    fn print_results(&self, result: &ConnectionPerfResult) {
        println!("\n=== Connection Performance Test Results ===");
        println!("Test Configuration:");
        println!(
            "  Target Concurrent Connections: {}",
            self.concurrent_connections
        );
        println!("  Total Test Connections: {}", self.total_connections);
        println!();

        println!("Sequential Connection Test:");
        println!("  Total Attempts: {}", result.total_attempts);
        println!(
            "  Successful: {} ({:.1}%)",
            result.successful_connections, result.connection_success_rate
        );
        println!(
            "  Failed: {} ({:.1}%)",
            result.failed_connections,
            100.0 - result.connection_success_rate
        );
        println!();

        if !result.connection_times.is_empty() {
            println!("Connection Timing Statistics:");
            println!(
                "  Average Connection Time: {:?}",
                result.average_connection_time
            );
            println!("  Min Connection Time: {:?}", result.min_connection_time);
            println!("  Max Connection Time: {:?}", result.max_connection_time);

            let median_time = self.calculate_median(&result.connection_times);
            println!("  Median Connection Time: {median_time:?}");

            let p95_time = self.calculate_percentile(&result.connection_times, 95.0);
            println!("  95th Percentile: {p95_time:?}");

            let p99_time = self.calculate_percentile(&result.connection_times, 99.0);
            println!("  99th Percentile: {p99_time:?}");
            println!();
        }

        if !result.concurrent_test_results.is_empty() {
            println!("Concurrent Connection Test Results:");
            println!("  Level | Success | Failed | Success Rate | Avg Time | Total Time");
            println!("  ------|---------|--------|--------------|----------|------------");

            for test_result in &result.concurrent_test_results {
                let success_rate = if test_result.concurrent_level > 0 {
                    test_result.successful_connections as f64 / test_result.concurrent_level as f64
                        * 100.0
                } else {
                    0.0
                };

                println!(
                    "  {:5} | {:7} | {:6} | {:9.1}% | {:8.0}ms | {:9.0}ms",
                    test_result.concurrent_level,
                    test_result.successful_connections,
                    test_result.failed_connections,
                    success_rate,
                    test_result.average_time.as_millis(),
                    test_result.total_time.as_millis()
                );
            }
            println!();
        }

        self.print_performance_analysis(result);

        let overall_score = self.calculate_performance_score(result);
        println!("Overall Performance Score: {overall_score:.1}/100");
    }

    fn print_performance_analysis(&self, result: &ConnectionPerfResult) {
        println!("Performance Analysis:");

        if result.connection_success_rate >= 95.0 {
            println!("  ✓ Connection reliability: Excellent");
        } else if result.connection_success_rate >= 90.0 {
            println!("  ⚠ Connection reliability: Good");
        } else {
            println!("  ✗ Connection reliability: Poor");
        }

        if result.average_connection_time <= Duration::from_millis(500) {
            println!("  ✓ Connection speed: Fast");
        } else if result.average_connection_time <= Duration::from_secs(2) {
            println!("  ⚠ Connection speed: Moderate");
        } else {
            println!("  ✗ Connection speed: Slow");
        }

        if let Some(best_concurrent) = result
            .concurrent_test_results
            .iter()
            .filter(|r| r.successful_connections == r.concurrent_level)
            .max_by_key(|r| r.concurrent_level)
        {
            println!(
                "  ✓ Max reliable concurrent connections: {}",
                best_concurrent.concurrent_level
            );
        }

        let time_variance = self.calculate_variance(&result.connection_times);
        if time_variance <= 0.1 {
            println!("  ✓ Connection time consistency: Excellent");
        } else if time_variance <= 0.3 {
            println!("  ⚠ Connection time consistency: Good");
        } else {
            println!("  ✗ Connection time consistency: Poor");
        }

        println!();
    }

    fn calculate_median(&self, times: &[Duration]) -> Duration {
        if times.is_empty() {
            return Duration::ZERO;
        }

        let mut sorted_times = times.to_vec();
        sorted_times.sort();

        let mid = sorted_times.len() / 2;
        if sorted_times.len() % 2 == 0 {
            (sorted_times[mid - 1] + sorted_times[mid]) / 2
        } else {
            sorted_times[mid]
        }
    }

    fn calculate_percentile(&self, times: &[Duration], percentile: f64) -> Duration {
        if times.is_empty() {
            return Duration::ZERO;
        }

        let mut sorted_times = times.to_vec();
        sorted_times.sort();

        let index = ((percentile / 100.0) * (sorted_times.len() - 1) as f64).round() as usize;
        sorted_times[index.min(sorted_times.len() - 1)]
    }

    fn calculate_variance(&self, times: &[Duration]) -> f64 {
        if times.len() <= 1 {
            return 0.0;
        }

        let mean = times.iter().sum::<Duration>().as_secs_f64() / times.len() as f64;
        let variance = times
            .iter()
            .map(|t| (t.as_secs_f64() - mean).powi(2))
            .sum::<f64>()
            / times.len() as f64;

        variance.sqrt() / mean
    }

    fn calculate_performance_score(&self, result: &ConnectionPerfResult) -> f64 {
        let success_score = result.connection_success_rate;

        let speed_score = if result.average_connection_time <= Duration::from_millis(500) {
            100.0
        } else if result.average_connection_time <= Duration::from_secs(2) {
            70.0
        } else if result.average_connection_time <= Duration::from_secs(5) {
            40.0
        } else {
            10.0
        };

        let consistency_score = {
            let variance = self.calculate_variance(&result.connection_times);
            if variance <= 0.1 {
                100.0
            } else if variance <= 0.3 {
                70.0
            } else {
                30.0
            }
        };

        let concurrent_score = if let Some(max_concurrent) = result
            .concurrent_test_results
            .iter()
            .filter(|r| r.successful_connections == r.concurrent_level)
            .map(|r| r.concurrent_level)
            .max()
        {
            (max_concurrent as f64 / 50.0 * 100.0).min(100.0)
        } else {
            0.0
        };

        (success_score * 0.4 + speed_score * 0.3 + consistency_score * 0.2 + concurrent_score * 0.1).clamp(0.0, 100.0)
    }
}
