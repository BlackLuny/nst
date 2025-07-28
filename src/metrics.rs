use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metrics {
    pub session_id: String,
    pub test_start_time: DateTime<Utc>,
    pub test_end_time: Option<DateTime<Utc>>,
    pub proxy_config: ProxyMetrics,
    pub tcp_stability: Option<TcpStabilityMetrics>,
    pub bandwidth: Option<BandwidthMetrics>,
    pub connection_perf: Option<ConnectionPerfMetrics>,
    pub dns_stability: Option<DnsStabilityMetrics>,
    pub network_jitter: Option<NetworkJitterMetrics>,
    pub overall_score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyMetrics {
    pub proxy_address: String,
    pub proxy_type: String,
    pub auth_required: bool,
    pub connection_timeout: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpStabilityMetrics {
    pub test_duration: Duration,
    pub heartbeat_interval: Duration,
    pub total_heartbeats: u64,
    pub successful_heartbeats: u64,
    pub failed_heartbeats: u64,
    pub reconnections: u64,
    pub total_downtime: Duration,
    pub uptime_percentage: f64,
    pub average_rtt: Duration,
    pub min_rtt: Duration,
    pub max_rtt: Duration,
    pub rtt_variance: f64,
    pub stability_score: f64,
    pub connection_drops: Vec<ConnectionDropMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionDropMetrics {
    pub timestamp: DateTime<Utc>,
    pub duration: Duration,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandwidthMetrics {
    pub test_duration: Duration,
    pub chunk_size: usize,
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,
    pub average_upload_speed: f64,
    pub average_download_speed: f64,
    pub max_upload_speed: f64,
    pub max_download_speed: f64,
    pub min_upload_speed: f64,
    pub min_download_speed: f64,
    pub speed_consistency_score: f64,
    pub connection_interruptions: u32,
    pub data_integrity_errors: u32,
    pub bandwidth_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionPerfMetrics {
    pub total_attempts: usize,
    pub successful_connections: usize,
    pub failed_connections: usize,
    pub success_rate: f64,
    pub average_connection_time: Duration,
    pub min_connection_time: Duration,
    pub max_connection_time: Duration,
    pub median_connection_time: Duration,
    pub p95_connection_time: Duration,
    pub p99_connection_time: Duration,
    pub connection_time_variance: f64,
    pub max_concurrent_successful: usize,
    pub performance_score: f64,
    pub concurrent_results: Vec<ConcurrentMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcurrentMetrics {
    pub concurrent_level: usize,
    pub successful_connections: usize,
    pub failed_connections: usize,
    pub success_rate: f64,
    pub average_time: Duration,
    pub total_time: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsStabilityMetrics {
    pub test_duration: Duration,
    pub query_interval: Duration,
    pub domains_tested: usize,
    pub total_queries: u64,
    pub successful_queries: u64,
    pub failed_queries: u64,
    pub timeout_queries: u64,
    pub success_rate: f64,
    pub timeout_rate: f64,
    pub average_query_time: Duration,
    pub min_query_time: Duration,
    pub max_query_time: Duration,
    pub query_time_variance: f64,
    pub domain_consistency_score: f64,
    pub dns_score: f64,
    pub per_domain_metrics: HashMap<String, DomainMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainMetrics {
    pub domain: String,
    pub total_queries: u64,
    pub successful_queries: u64,
    pub failed_queries: u64,
    pub success_rate: f64,
    pub average_query_time: Duration,
    pub min_query_time: Duration,
    pub max_query_time: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkJitterMetrics {
    pub test_duration: Duration,
    pub ping_interval: Duration,
    pub targets_tested: usize,
    pub total_pings: u64,
    pub successful_pings: u64,
    pub failed_pings: u64,
    pub timeout_pings: u64,
    pub packet_loss_rate: f64,
    pub average_rtt: Duration,
    pub median_rtt: Duration,
    pub min_rtt: Duration,
    pub max_rtt: Duration,
    pub p95_rtt: Duration,
    pub p99_rtt: Duration,
    pub jitter: Duration,
    pub rtt_variance: f64,
    pub latency_consistency_score: f64,
    pub target_consistency_score: f64,
    pub network_quality_score: f64,
    pub per_target_metrics: HashMap<String, TargetMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetMetrics {
    pub target: String,
    pub total_pings: u64,
    pub successful_pings: u64,
    pub failed_pings: u64,
    pub packet_loss_rate: f64,
    pub average_rtt: Duration,
    pub jitter: Duration,
}

impl Metrics {
    pub fn new(proxy_address: String) -> Self {
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            test_start_time: Utc::now(),
            test_end_time: None,
            proxy_config: ProxyMetrics {
                proxy_address,
                proxy_type: "SOCKS5".to_string(),
                auth_required: false,
                connection_timeout: Duration::from_secs(5),
            },
            tcp_stability: None,
            bandwidth: None,
            connection_perf: None,
            dns_stability: None,
            network_jitter: None,
            overall_score: None,
        }
    }

    pub fn finalize(&mut self) {
        self.test_end_time = Some(Utc::now());
        self.calculate_overall_score();
    }

    pub fn calculate_overall_score(&mut self) {
        let mut scores = Vec::new();
        let mut weights = Vec::new();

        if let Some(ref tcp) = self.tcp_stability {
            scores.push(tcp.stability_score);
            weights.push(0.25);
        }

        if let Some(ref bandwidth) = self.bandwidth {
            scores.push(bandwidth.bandwidth_score);
            weights.push(0.20);
        }

        if let Some(ref conn_perf) = self.connection_perf {
            scores.push(conn_perf.performance_score);
            weights.push(0.20);
        }

        if let Some(ref dns) = self.dns_stability {
            scores.push(dns.dns_score);
            weights.push(0.15);
        }

        if let Some(ref jitter) = self.network_jitter {
            scores.push(jitter.network_quality_score);
            weights.push(0.20);
        }

        if !scores.is_empty() {
            let total_weight: f64 = weights.iter().sum();
            let weighted_sum: f64 = scores
                .iter()
                .zip(weights.iter())
                .map(|(score, weight)| score * weight)
                .sum();

            self.overall_score = Some(weighted_sum / total_weight);
        }
    }

    pub fn get_test_duration(&self) -> Option<Duration> {
        if let Some(end_time) = self.test_end_time {
            let duration = end_time.signed_duration_since(self.test_start_time);
            Some(Duration::from_secs(duration.num_seconds().max(0) as u64))
        } else {
            None
        }
    }

    pub fn get_summary(&self) -> MetricsSummary {
        MetricsSummary {
            session_id: self.session_id.clone(),
            proxy_address: self.proxy_config.proxy_address.clone(),
            test_duration: self.get_test_duration(),
            overall_score: self.overall_score,
            tests_run: self.count_tests_run(),
            tcp_stability_score: self.tcp_stability.as_ref().map(|t| t.stability_score),
            bandwidth_score: self.bandwidth.as_ref().map(|b| b.bandwidth_score),
            connection_perf_score: self.connection_perf.as_ref().map(|c| c.performance_score),
            dns_stability_score: self.dns_stability.as_ref().map(|d| d.dns_score),
            network_quality_score: self
                .network_jitter
                .as_ref()
                .map(|n| n.network_quality_score),
        }
    }

    fn count_tests_run(&self) -> usize {
        let mut count = 0;
        if self.tcp_stability.is_some() {
            count += 1;
        }
        if self.bandwidth.is_some() {
            count += 1;
        }
        if self.connection_perf.is_some() {
            count += 1;
        }
        if self.dns_stability.is_some() {
            count += 1;
        }
        if self.network_jitter.is_some() {
            count += 1;
        }
        count
    }

    pub fn export_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    pub fn export_csv(&self) -> String {
        let mut csv = String::new();

        csv.push_str("metric_type,metric_name,value,unit\n");

        csv.push_str(&format!("session,session_id,{},\n", self.session_id));
        csv.push_str(&format!(
            "session,proxy_address,{},\n",
            self.proxy_config.proxy_address
        ));

        if let Some(score) = self.overall_score {
            csv.push_str(&format!("overall,score,{score:.2},points\n"));
        }

        if let Some(ref tcp) = self.tcp_stability {
            csv.push_str(&format!(
                "tcp_stability,stability_score,{:.2},points\n",
                tcp.stability_score
            ));
            csv.push_str(&format!(
                "tcp_stability,uptime_percentage,{:.2},percent\n",
                tcp.uptime_percentage
            ));
            csv.push_str(&format!(
                "tcp_stability,average_rtt,{},ms\n",
                tcp.average_rtt.as_millis()
            ));
            csv.push_str(&format!(
                "tcp_stability,reconnections,{},count\n",
                tcp.reconnections
            ));
        }

        if let Some(ref bandwidth) = self.bandwidth {
            csv.push_str(&format!(
                "bandwidth,bandwidth_score,{:.2},points\n",
                bandwidth.bandwidth_score
            ));
            csv.push_str(&format!(
                "bandwidth,average_upload_speed,{:.2},bytes_per_sec\n",
                bandwidth.average_upload_speed
            ));
            csv.push_str(&format!(
                "bandwidth,average_download_speed,{:.2},bytes_per_sec\n",
                bandwidth.average_download_speed
            ));
            csv.push_str(&format!(
                "bandwidth,connection_interruptions,{},count\n",
                bandwidth.connection_interruptions
            ));
        }

        if let Some(ref conn_perf) = self.connection_perf {
            csv.push_str(&format!(
                "connection_perf,performance_score,{:.2},points\n",
                conn_perf.performance_score
            ));
            csv.push_str(&format!(
                "connection_perf,success_rate,{:.2},percent\n",
                conn_perf.success_rate
            ));
            csv.push_str(&format!(
                "connection_perf,average_connection_time,{},ms\n",
                conn_perf.average_connection_time.as_millis()
            ));
            csv.push_str(&format!(
                "connection_perf,max_concurrent_successful,{},count\n",
                conn_perf.max_concurrent_successful
            ));
        }

        if let Some(ref dns) = self.dns_stability {
            csv.push_str(&format!(
                "dns_stability,dns_score,{:.2},points\n",
                dns.dns_score
            ));
            csv.push_str(&format!(
                "dns_stability,success_rate,{:.2},percent\n",
                dns.success_rate
            ));
            csv.push_str(&format!(
                "dns_stability,average_query_time,{},ms\n",
                dns.average_query_time.as_millis()
            ));
            csv.push_str(&format!(
                "dns_stability,timeout_rate,{:.2},percent\n",
                dns.timeout_rate
            ));
        }

        if let Some(ref jitter) = self.network_jitter {
            csv.push_str(&format!(
                "network_jitter,network_quality_score,{:.2},points\n",
                jitter.network_quality_score
            ));
            csv.push_str(&format!(
                "network_jitter,packet_loss_rate,{:.2},percent\n",
                jitter.packet_loss_rate
            ));
            csv.push_str(&format!(
                "network_jitter,average_rtt,{},ms\n",
                jitter.average_rtt.as_millis()
            ));
            csv.push_str(&format!(
                "network_jitter,jitter,{},ms\n",
                jitter.jitter.as_millis()
            ));
        }

        csv
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSummary {
    pub session_id: String,
    pub proxy_address: String,
    pub test_duration: Option<Duration>,
    pub overall_score: Option<f64>,
    pub tests_run: usize,
    pub tcp_stability_score: Option<f64>,
    pub bandwidth_score: Option<f64>,
    pub connection_perf_score: Option<f64>,
    pub dns_stability_score: Option<f64>,
    pub network_quality_score: Option<f64>,
}

impl MetricsSummary {
    pub fn print_summary(&self) {
        println!("\n=== Test Session Summary ===");
        println!("Session ID: {}", self.session_id);
        println!("Proxy Address: {}", self.proxy_address);

        if let Some(duration) = self.test_duration {
            println!("Total Test Duration: {duration:?}");
        }

        println!("Tests Run: {}", self.tests_run);
        println!();

        if let Some(score) = self.overall_score {
            println!("Overall Network Stability Score: {score:.1}/100");

            let rating = match score {
                s if s >= 90.0 => "Excellent",
                s if s >= 80.0 => "Good",
                s if s >= 70.0 => "Fair",
                s if s >= 60.0 => "Poor",
                _ => "Very Poor",
            };
            println!("Rating: {rating}");
        }
        println!();

        println!("Individual Test Scores:");
        if let Some(score) = self.tcp_stability_score {
            println!("  TCP Stability: {score:.1}/100");
        }
        if let Some(score) = self.bandwidth_score {
            println!("  Bandwidth: {score:.1}/100");
        }
        if let Some(score) = self.connection_perf_score {
            println!("  Connection Performance: {score:.1}/100");
        }
        if let Some(score) = self.dns_stability_score {
            println!("  DNS Stability: {score:.1}/100");
        }
        if let Some(score) = self.network_quality_score {
            println!("  Network Quality: {score:.1}/100");
        }
        println!();
    }
}

pub struct MetricsCollector {
    metrics: Metrics,
}

impl MetricsCollector {
    pub fn new(proxy_address: String) -> Self {
        Self {
            metrics: Metrics::new(proxy_address),
        }
    }

    pub fn set_tcp_stability_metrics(&mut self, metrics: TcpStabilityMetrics) {
        self.metrics.tcp_stability = Some(metrics);
    }

    pub fn set_bandwidth_metrics(&mut self, metrics: BandwidthMetrics) {
        self.metrics.bandwidth = Some(metrics);
    }

    pub fn set_connection_perf_metrics(&mut self, metrics: ConnectionPerfMetrics) {
        self.metrics.connection_perf = Some(metrics);
    }

    pub fn set_dns_stability_metrics(&mut self, metrics: DnsStabilityMetrics) {
        self.metrics.dns_stability = Some(metrics);
    }

    pub fn set_network_jitter_metrics(&mut self, metrics: NetworkJitterMetrics) {
        self.metrics.network_jitter = Some(metrics);
    }

    pub fn finalize(mut self) -> Metrics {
        self.metrics.finalize();
        self.metrics
    }

    pub fn get_metrics(&self) -> &Metrics {
        &self.metrics
    }

    pub fn get_summary(&self) -> MetricsSummary {
        self.metrics.get_summary()
    }
}
