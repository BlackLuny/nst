use crate::{NetworkTestError, Result, Socks5Client};
use std::time::{Duration, Instant};
use tokio::time::{interval, timeout};
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct DnsStabilityTest {
    proxy_addr: String,
    domains: Vec<String>,
    query_interval: Duration,
    test_duration: Duration,
}

#[derive(Debug, Clone)]
pub struct DnsStabilityResult {
    pub total_queries: u64,
    pub successful_queries: u64,
    pub failed_queries: u64,
    pub timeout_queries: u64,
    pub domain_results: std::collections::HashMap<String, DomainResult>,
    pub average_query_time: Duration,
    pub min_query_time: Duration,
    pub max_query_time: Duration,
    pub success_rate: f64,
}

#[derive(Debug, Clone)]
pub struct DomainResult {
    pub domain: String,
    pub total_queries: u64,
    pub successful_queries: u64,
    pub failed_queries: u64,
    pub average_query_time: Duration,
    pub query_times: Vec<Duration>,
}

impl DnsStabilityTest {
    pub fn new(
        proxy_addr: &str,
        domains: Vec<String>,
        query_interval_ms: u64,
        test_duration_sec: u64,
    ) -> Self {
        Self {
            proxy_addr: proxy_addr.to_string(),
            domains,
            query_interval: Duration::from_millis(query_interval_ms),
            test_duration: Duration::from_secs(test_duration_sec),
        }
    }

    pub async fn run(&self) -> Result<()> {
        info!("Starting DNS stability test");
        info!("Proxy: {}", self.proxy_addr);
        info!("Domains: {:?}", self.domains);
        info!(
            "Query interval: {:?}, Test duration: {:?}",
            self.query_interval, self.test_duration
        );

        let proxy_addr = self
            .proxy_addr
            .parse()
            .map_err(|e| NetworkTestError::Config(format!("Invalid proxy address: {e}")))?;

        let client = Socks5Client::new(proxy_addr).with_timeout(Duration::from_secs(10));

        let result = self.run_dns_test(&client).await?;

        self.print_results(&result);

        Ok(())
    }

    async fn run_dns_test(&self, client: &Socks5Client) -> Result<DnsStabilityResult> {
        let start_time = Instant::now();
        let end_time = start_time + self.test_duration;

        let mut domain_results = std::collections::HashMap::new();
        for domain in &self.domains {
            domain_results.insert(
                domain.clone(),
                DomainResult {
                    domain: domain.clone(),
                    total_queries: 0,
                    successful_queries: 0,
                    failed_queries: 0,
                    average_query_time: Duration::ZERO,
                    query_times: Vec::new(),
                },
            );
        }

        let mut total_queries = 0u64;
        let mut successful_queries = 0u64;
        let mut failed_queries = 0u64;
        let mut timeout_queries = 0u64;
        let mut all_query_times = Vec::new();

        let mut query_interval = interval(self.query_interval);
        let mut domain_index = 0;

        while Instant::now() < end_time {
            query_interval.tick().await;

            if self.domains.is_empty() {
                break;
            }

            let domain = &self.domains[domain_index % self.domains.len()];
            domain_index += 1;

            let _query_start = Instant::now();
            total_queries += 1;

            let domain_result = domain_results.get_mut(domain).unwrap();
            domain_result.total_queries += 1;

            match self.perform_dns_query(client, domain).await {
                Ok(query_time) => {
                    successful_queries += 1;
                    domain_result.successful_queries += 1;
                    domain_result.query_times.push(query_time);
                    all_query_times.push(query_time);

                    debug!("DNS query for {} successful: {:?}", domain, query_time);
                }
                Err(NetworkTestError::Timeout(_)) => {
                    timeout_queries += 1;
                    domain_result.failed_queries += 1;
                    warn!("DNS query for {} timed out", domain);
                }
                Err(e) => {
                    failed_queries += 1;
                    domain_result.failed_queries += 1;
                    warn!("DNS query for {} failed: {}", domain, e);
                }
            }
        }

        for domain_result in domain_results.values_mut() {
            if !domain_result.query_times.is_empty() {
                domain_result.average_query_time =
                    domain_result.query_times.iter().sum::<Duration>()
                        / domain_result.query_times.len() as u32;
            }
        }

        let average_query_time = if !all_query_times.is_empty() {
            all_query_times.iter().sum::<Duration>() / all_query_times.len() as u32
        } else {
            Duration::ZERO
        };

        let min_query_time = all_query_times
            .iter()
            .min()
            .copied()
            .unwrap_or(Duration::ZERO);
        let max_query_time = all_query_times
            .iter()
            .max()
            .copied()
            .unwrap_or(Duration::ZERO);

        let success_rate = if total_queries > 0 {
            successful_queries as f64 / total_queries as f64 * 100.0
        } else {
            0.0
        };

        Ok(DnsStabilityResult {
            total_queries,
            successful_queries,
            failed_queries,
            timeout_queries,
            domain_results,
            average_query_time,
            min_query_time,
            max_query_time,
            success_rate,
        })
    }

    async fn perform_dns_query(&self, client: &Socks5Client, domain: &str) -> Result<Duration> {
        let query_start = Instant::now();

        let dns_server = "8.8.8.8:53";

        let query_result = timeout(
            Duration::from_secs(5),
            self.dns_query_via_proxy(client, dns_server, domain),
        )
        .await;

        match query_result {
            Ok(Ok(())) => Ok(query_start.elapsed()),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(NetworkTestError::Timeout(format!(
                "DNS query timeout for {domain}"
            ))),
        }
    }

    async fn dns_query_via_proxy(
        &self,
        client: &Socks5Client,
        dns_server: &str,
        domain: &str,
    ) -> Result<()> {
        let udp_relay = client.udp_associate().await.map_err(|e| {
            NetworkTestError::Connection(format!("Failed to create UDP association: {e}"))
        })?;

        let query_packet = self.create_dns_query_packet(domain)?;

        udp_relay
            .send_to(&query_packet, dns_server)
            .await
            .map_err(|e| {
                NetworkTestError::Connection(format!("Failed to send DNS query: {e}"))
            })?;

        let mut response_buffer = [0u8; 512];
        let (bytes_read, _source_addr) =
            udp_relay
                .recv_from(&mut response_buffer)
                .await
                .map_err(|e| {
                    NetworkTestError::Connection(format!("Failed to receive DNS response: {e}"))
                })?;

        if bytes_read < 12 {
            return Err(NetworkTestError::Connection(
                "Invalid DNS response".to_string(),
            ));
        }

        let response_code = response_buffer[3] & 0x0F;
        if response_code != 0 {
            return Err(NetworkTestError::Connection(format!(
                "DNS query failed with code: {response_code}"
            )));
        }

        debug!("DNS query successful for domain: {}", domain);
        Ok(())
    }

    fn create_dns_query_packet(&self, domain: &str) -> Result<Vec<u8>> {
        let mut packet = Vec::new();

        packet.extend_from_slice(&[
            0x12, 0x34, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ]);

        for part in domain.split('.') {
            if part.len() > 63 {
                return Err(NetworkTestError::Config("Domain part too long".to_string()));
            }
            packet.push(part.len() as u8);
            packet.extend_from_slice(part.as_bytes());
        }
        packet.push(0);

        packet.extend_from_slice(&[0x00, 0x01]);
        packet.extend_from_slice(&[0x00, 0x01]);

        Ok(packet)
    }

    fn print_results(&self, result: &DnsStabilityResult) {
        println!("\n=== DNS Stability Test Results ===");
        println!("Test Duration: {:?}", self.test_duration);
        println!("Query Interval: {:?}", self.query_interval);
        println!("Tested Domains: {}", self.domains.len());
        println!();

        println!("Overall Statistics:");
        println!("  Total Queries: {}", result.total_queries);
        println!(
            "  Successful: {} ({:.1}%)",
            result.successful_queries, result.success_rate
        );
        println!(
            "  Failed: {} ({:.1}%)",
            result.failed_queries,
            if result.total_queries > 0 {
                result.failed_queries as f64 / result.total_queries as f64 * 100.0
            } else {
                0.0
            }
        );
        println!(
            "  Timeouts: {} ({:.1}%)",
            result.timeout_queries,
            if result.total_queries > 0 {
                result.timeout_queries as f64 / result.total_queries as f64 * 100.0
            } else {
                0.0
            }
        );
        println!();

        if result.successful_queries > 0 {
            println!("Query Performance:");
            println!("  Average Query Time: {:?}", result.average_query_time);
            println!("  Min Query Time: {:?}", result.min_query_time);
            println!("  Max Query Time: {:?}", result.max_query_time);
            println!();
        }

        println!("Per-Domain Results:");
        println!("  Domain                    | Queries | Success | Failed | Avg Time");
        println!("  --------------------------|---------|---------|--------|----------");

        let mut sorted_domains: Vec<_> = result.domain_results.iter().collect();
        sorted_domains.sort_by_key(|(domain, _)| domain.as_str());

        for (domain, domain_result) in sorted_domains {
            let success_rate = if domain_result.total_queries > 0 {
                domain_result.successful_queries as f64 / domain_result.total_queries as f64 * 100.0
            } else {
                0.0
            };

            println!(
                "  {:25} | {:7} | {:6.1}% | {:6} | {:7.0}ms",
                self.truncate_domain(domain, 25),
                domain_result.total_queries,
                success_rate,
                domain_result.failed_queries,
                domain_result.average_query_time.as_millis()
            );
        }
        println!();

        self.print_dns_analysis(result);

        let dns_score = self.calculate_dns_score(result);
        println!("DNS Stability Score: {dns_score:.1}/100");
    }

    fn print_dns_analysis(&self, result: &DnsStabilityResult) {
        println!("DNS Performance Analysis:");

        if result.success_rate >= 98.0 {
            println!("  ✓ DNS Resolution Reliability: Excellent");
        } else if result.success_rate >= 95.0 {
            println!("  ⚠ DNS Resolution Reliability: Good");
        } else if result.success_rate >= 90.0 {
            println!("  ⚠ DNS Resolution Reliability: Fair");
        } else {
            println!("  ✗ DNS Resolution Reliability: Poor");
        }

        if result.average_query_time <= Duration::from_millis(50) {
            println!("  ✓ DNS Query Speed: Excellent");
        } else if result.average_query_time <= Duration::from_millis(200) {
            println!("  ⚠ DNS Query Speed: Good");
        } else if result.average_query_time <= Duration::from_millis(500) {
            println!("  ⚠ DNS Query Speed: Fair");
        } else {
            println!("  ✗ DNS Query Speed: Poor");
        }

        let timeout_rate = if result.total_queries > 0 {
            result.timeout_queries as f64 / result.total_queries as f64 * 100.0
        } else {
            0.0
        };

        if timeout_rate <= 1.0 {
            println!("  ✓ Timeout Rate: Excellent ({timeout_rate:.1}%)");
        } else if timeout_rate <= 5.0 {
            println!("  ⚠ Timeout Rate: Acceptable ({timeout_rate:.1}%)");
        } else {
            println!("  ✗ Timeout Rate: High ({timeout_rate:.1}%)");
        }

        let domain_consistency = self.calculate_domain_consistency(result);
        if domain_consistency >= 0.9 {
            println!("  ✓ Cross-Domain Consistency: Excellent");
        } else if domain_consistency >= 0.8 {
            println!("  ⚠ Cross-Domain Consistency: Good");
        } else {
            println!("  ✗ Cross-Domain Consistency: Poor");
        }

        println!();
    }

    fn calculate_domain_consistency(&self, result: &DnsStabilityResult) -> f64 {
        if result.domain_results.len() <= 1 {
            return 1.0;
        }

        let success_rates: Vec<f64> = result
            .domain_results
            .values()
            .map(|dr| {
                if dr.total_queries > 0 {
                    dr.successful_queries as f64 / dr.total_queries as f64
                } else {
                    0.0
                }
            })
            .collect();

        if success_rates.is_empty() {
            return 0.0;
        }

        let mean_success_rate = success_rates.iter().sum::<f64>() / success_rates.len() as f64;
        let variance = success_rates
            .iter()
            .map(|&rate| (rate - mean_success_rate).powi(2))
            .sum::<f64>()
            / success_rates.len() as f64;

        let std_dev = variance.sqrt();
        let coefficient_of_variation = if mean_success_rate > 0.0 {
            std_dev / mean_success_rate
        } else {
            1.0
        };

        (1.0 - coefficient_of_variation).max(0.0)
    }

    fn calculate_dns_score(&self, result: &DnsStabilityResult) -> f64 {
        let success_score = result.success_rate;

        let speed_score = if result.average_query_time <= Duration::from_millis(50) {
            100.0
        } else if result.average_query_time <= Duration::from_millis(200) {
            80.0
        } else if result.average_query_time <= Duration::from_millis(500) {
            60.0
        } else if result.average_query_time <= Duration::from_secs(1) {
            40.0
        } else {
            20.0
        };

        let timeout_rate = if result.total_queries > 0 {
            result.timeout_queries as f64 / result.total_queries as f64 * 100.0
        } else {
            0.0
        };
        let timeout_score = (100.0 - timeout_rate * 10.0).max(0.0);

        let consistency_score = self.calculate_domain_consistency(result) * 100.0;

        (success_score * 0.4 + speed_score * 0.3 + timeout_score * 0.2 + consistency_score * 0.1).clamp(0.0, 100.0)
    }

    fn truncate_domain(&self, domain: &str, max_len: usize) -> String {
        if domain.len() <= max_len {
            domain.to_string()
        } else {
            format!("{}...", &domain[..max_len - 3])
        }
    }
}
