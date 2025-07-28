use serde::{Deserialize, Serialize};
use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub proxy: ProxyConfig,
    pub tests: TestConfig,
    pub reporting: ReportingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConfig {
    pub tcp_stability: TcpStabilityConfig,
    pub bandwidth: BandwidthConfig,
    pub connection_perf: ConnectionPerfConfig,
    pub dns_stability: DnsStabilityConfig,
    pub network_jitter: NetworkJitterConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpStabilityConfig {
    pub heartbeat_interval_ms: u64,
    pub test_duration_sec: u64,
    pub max_retries: u32,
    pub targets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandwidthConfig {
    pub chunk_size: usize,
    pub test_duration_sec: u64,
    pub targets: Vec<String>,
    pub upload_test: bool,
    pub download_test: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionPerfConfig {
    pub concurrent_connections: usize,
    pub total_connections: usize,
    pub connection_timeout_ms: u64,
    pub targets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsStabilityConfig {
    pub domains: Vec<String>,
    pub query_interval_ms: u64,
    pub test_duration_sec: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkJitterConfig {
    pub ping_interval_ms: u64,
    pub test_duration_sec: u64,
    pub targets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportingConfig {
    pub output_format: OutputFormat,
    pub output_file: Option<String>,
    pub real_time_metrics: bool,
    pub detailed_logs: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputFormat {
    Json,
    Csv,
    Text,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            proxy: ProxyConfig {
                host: "127.0.0.1".to_string(),
                port: 1080,
                username: None,
                password: None,
                timeout_ms: 5000,
            },
            tests: TestConfig {
                tcp_stability: TcpStabilityConfig {
                    heartbeat_interval_ms: 30000,
                    test_duration_sec: 300,
                    max_retries: 3,
                    targets: vec!["8.8.8.8:53".to_string(), "1.1.1.1:53".to_string()],
                },
                bandwidth: BandwidthConfig {
                    chunk_size: 1024,
                    test_duration_sec: 60,
                    targets: vec!["httpbin.org:80".to_string()],
                    upload_test: true,
                    download_test: true,
                },
                connection_perf: ConnectionPerfConfig {
                    concurrent_connections: 10,
                    total_connections: 100,
                    connection_timeout_ms: 5000,
                    targets: vec!["8.8.8.8:53".to_string()],
                },
                dns_stability: DnsStabilityConfig {
                    domains: vec![
                        "google.com".to_string(),
                        "github.com".to_string(),
                        "cloudflare.com".to_string(),
                    ],
                    query_interval_ms: 1000,
                    test_duration_sec: 60,
                },
                network_jitter: NetworkJitterConfig {
                    ping_interval_ms: 1000,
                    test_duration_sec: 60,
                    targets: vec!["8.8.8.8:53".to_string(), "1.1.1.1:53".to_string()],
                },
            },
            reporting: ReportingConfig {
                output_format: OutputFormat::Json,
                output_file: None,
                real_time_metrics: true,
                detailed_logs: false,
            },
        }
    }
}

impl Config {
    pub fn from_file(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| crate::NetworkTestError::Config(format!("Failed to read config file: {}", e)))?;
        
        let config: Config = serde_json::from_str(&content)
            .map_err(|e| crate::NetworkTestError::Config(format!("Failed to parse config file: {}", e)))?;
        
        Ok(config)
    }
    
    pub fn to_file(&self, path: &str) -> Result<()> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| crate::NetworkTestError::Config(format!("Failed to serialize config: {}", e)))?;
        
        std::fs::write(path, content)
            .map_err(|e| crate::NetworkTestError::Config(format!("Failed to write config file: {}", e)))?;
        
        Ok(())
    }
}