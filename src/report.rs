use std::fs;
use std::path::Path;
use chrono::Utc;
use crate::{Result, NetworkTestError, Metrics};

#[derive(Debug, Clone)]
pub struct Report {
    metrics: Metrics,
    output_format: OutputFormat,
    output_file: Option<String>,
}

#[derive(Debug, Clone)]
pub enum OutputFormat {
    Json,
    Csv,
    Html,
    Text,
}

impl Report {
    pub fn new(metrics: Metrics) -> Self {
        Self {
            metrics,
            output_format: OutputFormat::Json,
            output_file: None,
        }
    }
    
    pub fn with_format(mut self, format: OutputFormat) -> Self {
        self.output_format = format;
        self
    }
    
    pub fn with_output_file(mut self, file_path: String) -> Self {
        self.output_file = Some(file_path);
        self
    }
    
    pub fn generate_and_save(&self) -> Result<()> {
        let content = match self.output_format {
            OutputFormat::Json => self.generate_json()?,
            OutputFormat::Csv => self.generate_csv(),
            OutputFormat::Html => self.generate_html(),
            OutputFormat::Text => self.generate_text(),
        };
        
        if let Some(ref file_path) = self.output_file {
            self.save_to_file(&content, file_path)?;
            println!("Report saved to: {}", file_path);
        } else {
            println!("{}", content);
        }
        
        Ok(())
    }
    
    fn generate_json(&self) -> Result<String> {
        self.metrics.export_json()
            .map_err(|e| NetworkTestError::Config(format!("Failed to serialize JSON: {}", e)))
    }
    
    fn generate_csv(&self) -> String {
        self.metrics.export_csv()
    }
    
    fn generate_html(&self) -> String {
        let mut html = String::new();
        
        html.push_str("<!DOCTYPE html>\n");
        html.push_str("<html lang=\"en\">\n");
        html.push_str("<head>\n");
        html.push_str("    <meta charset=\"UTF-8\">\n");
        html.push_str("    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
        html.push_str("    <title>Network Stability Test Report</title>\n");
        html.push_str("    <style>\n");
        html.push_str(include_str!("../templates/report.css"));
        html.push_str("    </style>\n");
        html.push_str("</head>\n");
        html.push_str("<body>\n");
        
        html.push_str(&self.generate_html_header());
        html.push_str(&self.generate_html_summary());
        
        if self.metrics.tcp_stability.is_some() {
            html.push_str(&self.generate_html_tcp_stability());
        }
        
        if self.metrics.bandwidth.is_some() {
            html.push_str(&self.generate_html_bandwidth());
        }
        
        if self.metrics.connection_perf.is_some() {
            html.push_str(&self.generate_html_connection_perf());
        }
        
        if self.metrics.dns_stability.is_some() {
            html.push_str(&self.generate_html_dns_stability());
        }
        
        if self.metrics.network_jitter.is_some() {
            html.push_str(&self.generate_html_network_jitter());
        }
        
        html.push_str("</body>\n");
        html.push_str("</html>\n");
        
        html
    }
    
    fn generate_text(&self) -> String {
        let mut text = String::new();
        
        text.push_str("NETWORK STABILITY TEST REPORT\n");
        text.push_str("=============================\n\n");
        
        text.push_str(&format!("Session ID: {}\n", self.metrics.session_id));
        text.push_str(&format!("Proxy Address: {}\n", self.metrics.proxy_config.proxy_address));
        text.push_str(&format!("Test Start Time: {}\n", self.metrics.test_start_time.format("%Y-%m-%d %H:%M:%S UTC")));
        
        if let Some(end_time) = self.metrics.test_end_time {
            text.push_str(&format!("Test End Time: {}\n", end_time.format("%Y-%m-%d %H:%M:%S UTC")));
        }
        
        if let Some(duration) = self.metrics.get_test_duration() {
            text.push_str(&format!("Total Duration: {:?}\n", duration));
        }
        
        text.push_str("\n");
        
        if let Some(overall_score) = self.metrics.overall_score {
            text.push_str(&format!("OVERALL SCORE: {:.1}/100\n", overall_score));
            text.push_str(&format!("Rating: {}\n", self.get_rating(overall_score)));
            text.push_str("\n");
        }
        
        if let Some(ref tcp) = self.metrics.tcp_stability {
            text.push_str("TCP STABILITY TEST\n");
            text.push_str("------------------\n");
            text.push_str(&format!("Score: {:.1}/100\n", tcp.stability_score));
            text.push_str(&format!("Uptime: {:.2}%\n", tcp.uptime_percentage));
            text.push_str(&format!("Total Heartbeats: {}\n", tcp.total_heartbeats));
            text.push_str(&format!("Successful Heartbeats: {}\n", tcp.successful_heartbeats));
            text.push_str(&format!("Reconnections: {}\n", tcp.reconnections));
            text.push_str(&format!("Average RTT: {:?}\n", tcp.average_rtt));
            text.push_str("\n");
        }
        
        if let Some(ref bandwidth) = self.metrics.bandwidth {
            text.push_str("BANDWIDTH TEST\n");
            text.push_str("--------------\n");
            text.push_str(&format!("Score: {:.1}/100\n", bandwidth.bandwidth_score));
            text.push_str(&format!("Average Upload Speed: {:.2} KB/s\n", bandwidth.average_upload_speed / 1024.0));
            text.push_str(&format!("Average Download Speed: {:.2} KB/s\n", bandwidth.average_download_speed / 1024.0));
            text.push_str(&format!("Total Bytes Sent: {} ({:.2} MB)\n", 
                bandwidth.total_bytes_sent,
                bandwidth.total_bytes_sent as f64 / 1_048_576.0));
            text.push_str(&format!("Total Bytes Received: {} ({:.2} MB)\n", 
                bandwidth.total_bytes_received,
                bandwidth.total_bytes_received as f64 / 1_048_576.0));
            text.push_str(&format!("Connection Interruptions: {}\n", bandwidth.connection_interruptions));
            text.push_str("\n");
        }
        
        if let Some(ref conn_perf) = self.metrics.connection_perf {
            text.push_str("CONNECTION PERFORMANCE TEST\n");
            text.push_str("---------------------------\n");
            text.push_str(&format!("Score: {:.1}/100\n", conn_perf.performance_score));
            text.push_str(&format!("Success Rate: {:.2}%\n", conn_perf.success_rate));
            text.push_str(&format!("Total Attempts: {}\n", conn_perf.total_attempts));
            text.push_str(&format!("Successful Connections: {}\n", conn_perf.successful_connections));
            text.push_str(&format!("Average Connection Time: {:?}\n", conn_perf.average_connection_time));
            text.push_str(&format!("Max Concurrent Successful: {}\n", conn_perf.max_concurrent_successful));
            text.push_str("\n");
        }
        
        if let Some(ref dns) = self.metrics.dns_stability {
            text.push_str("DNS STABILITY TEST\n");
            text.push_str("------------------\n");
            text.push_str(&format!("Score: {:.1}/100\n", dns.dns_score));
            text.push_str(&format!("Success Rate: {:.2}%\n", dns.success_rate));
            text.push_str(&format!("Total Queries: {}\n", dns.total_queries));
            text.push_str(&format!("Successful Queries: {}\n", dns.successful_queries));
            text.push_str(&format!("Timeout Rate: {:.2}%\n", dns.timeout_rate));
            text.push_str(&format!("Average Query Time: {:?}\n", dns.average_query_time));
            text.push_str("\n");
        }
        
        if let Some(ref jitter) = self.metrics.network_jitter {
            text.push_str("NETWORK JITTER TEST\n");
            text.push_str("-------------------\n");
            text.push_str(&format!("Score: {:.1}/100\n", jitter.network_quality_score));
            text.push_str(&format!("Packet Loss Rate: {:.2}%\n", jitter.packet_loss_rate));
            text.push_str(&format!("Total Pings: {}\n", jitter.total_pings));
            text.push_str(&format!("Successful Pings: {}\n", jitter.successful_pings));
            text.push_str(&format!("Average RTT: {:?}\n", jitter.average_rtt));
            text.push_str(&format!("Jitter: {:?}\n", jitter.jitter));
            text.push_str("\n");
        }
        
        text.push_str("Report generated at: ");
        text.push_str(&Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string());
        text.push_str("\n");
        
        text
    }
    
    fn generate_html_header(&self) -> String {
        format!(
            r#"
    <header class="header">
        <h1>Network Stability Test Report</h1>
        <div class="header-info">
            <div>Session ID: {}</div>
            <div>Proxy: {}</div>
            <div>Generated: {}</div>
        </div>
    </header>
"#,
            self.metrics.session_id,
            self.metrics.proxy_config.proxy_address,
            Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        )
    }
    
    fn generate_html_summary(&self) -> String {
        let mut summary = String::from(r#"
    <section class="summary">
        <h2>Test Summary</h2>
        <div class="summary-grid">
"#);
        
        if let Some(overall_score) = self.metrics.overall_score {
            summary.push_str(&format!(
                r#"
            <div class="score-card overall">
                <h3>Overall Score</h3>
                <div class="score">{:.1}</div>
                <div class="rating">{}</div>
            </div>
"#,
                overall_score,
                self.get_rating(overall_score)
            ));
        }
        
        if let Some(ref tcp) = self.metrics.tcp_stability {
            summary.push_str(&format!(
                r#"
            <div class="score-card">
                <h3>TCP Stability</h3>
                <div class="score">{:.1}</div>
                <div class="detail">Uptime: {:.1}%</div>
            </div>
"#,
                tcp.stability_score,
                tcp.uptime_percentage
            ));
        }
        
        summary.push_str("        </div>\n    </section>\n");
        summary
    }
    
    fn generate_html_tcp_stability(&self) -> String {
        if let Some(ref tcp) = self.metrics.tcp_stability {
            format!(
                r#"
    <section class="test-section">
        <h2>TCP Stability Test</h2>
        <div class="metrics-grid">
            <div class="metric">
                <span class="label">Stability Score:</span>
                <span class="value">{:.1}/100</span>
            </div>
            <div class="metric">
                <span class="label">Uptime:</span>
                <span class="value">{:.2}%</span>
            </div>
            <div class="metric">
                <span class="label">Total Heartbeats:</span>
                <span class="value">{}</span>
            </div>
            <div class="metric">
                <span class="label">Successful Heartbeats:</span>
                <span class="value">{}</span>
            </div>
            <div class="metric">
                <span class="label">Reconnections:</span>
                <span class="value">{}</span>
            </div>
            <div class="metric">
                <span class="label">Average RTT:</span>
                <span class="value">{:?}</span>
            </div>
        </div>
    </section>
"#,
                tcp.stability_score,
                tcp.uptime_percentage,
                tcp.total_heartbeats,
                tcp.successful_heartbeats,
                tcp.reconnections,
                tcp.average_rtt
            )
        } else {
            String::new()
        }
    }
    
    fn generate_html_bandwidth(&self) -> String {
        if let Some(ref bandwidth) = self.metrics.bandwidth {
            format!(
                r#"
    <section class="test-section">
        <h2>Bandwidth Test</h2>
        <div class="metrics-grid">
            <div class="metric">
                <span class="label">Bandwidth Score:</span>
                <span class="value">{:.1}/100</span>
            </div>
            <div class="metric">
                <span class="label">Avg Upload Speed:</span>
                <span class="value">{:.2} KB/s</span>
            </div>
            <div class="metric">
                <span class="label">Avg Download Speed:</span>
                <span class="value">{:.2} KB/s</span>
            </div>
            <div class="metric">
                <span class="label">Total Sent:</span>
                <span class="value">{:.2} MB</span>
            </div>
            <div class="metric">
                <span class="label">Total Received:</span>
                <span class="value">{:.2} MB</span>
            </div>
            <div class="metric">
                <span class="label">Interruptions:</span>
                <span class="value">{}</span>
            </div>
        </div>
    </section>
"#,
                bandwidth.bandwidth_score,
                bandwidth.average_upload_speed / 1024.0,
                bandwidth.average_download_speed / 1024.0,
                bandwidth.total_bytes_sent as f64 / 1_048_576.0,
                bandwidth.total_bytes_received as f64 / 1_048_576.0,
                bandwidth.connection_interruptions
            )
        } else {
            String::new()
        }
    }
    
    fn generate_html_connection_perf(&self) -> String {
        if let Some(ref conn_perf) = self.metrics.connection_perf {
            format!(
                r#"
    <section class="test-section">
        <h2>Connection Performance Test</h2>
        <div class="metrics-grid">
            <div class="metric">
                <span class="label">Performance Score:</span>
                <span class="value">{:.1}/100</span>
            </div>
            <div class="metric">
                <span class="label">Success Rate:</span>
                <span class="value">{:.2}%</span>
            </div>
            <div class="metric">
                <span class="label">Total Attempts:</span>
                <span class="value">{}</span>
            </div>
            <div class="metric">
                <span class="label">Successful:</span>
                <span class="value">{}</span>
            </div>
            <div class="metric">
                <span class="label">Avg Connection Time:</span>
                <span class="value">{:?}</span>
            </div>
            <div class="metric">
                <span class="label">Max Concurrent:</span>
                <span class="value">{}</span>
            </div>
        </div>
    </section>
"#,
                conn_perf.performance_score,
                conn_perf.success_rate,
                conn_perf.total_attempts,
                conn_perf.successful_connections,
                conn_perf.average_connection_time,
                conn_perf.max_concurrent_successful
            )
        } else {
            String::new()
        }
    }
    
    fn generate_html_dns_stability(&self) -> String {
        if let Some(ref dns) = self.metrics.dns_stability {
            format!(
                r#"
    <section class="test-section">
        <h2>DNS Stability Test</h2>
        <div class="metrics-grid">
            <div class="metric">
                <span class="label">DNS Score:</span>
                <span class="value">{:.1}/100</span>
            </div>
            <div class="metric">
                <span class="label">Success Rate:</span>
                <span class="value">{:.2}%</span>
            </div>
            <div class="metric">
                <span class="label">Total Queries:</span>
                <span class="value">{}</span>
            </div>
            <div class="metric">
                <span class="label">Successful:</span>
                <span class="value">{}</span>
            </div>
            <div class="metric">
                <span class="label">Timeout Rate:</span>
                <span class="value">{:.2}%</span>
            </div>
            <div class="metric">
                <span class="label">Avg Query Time:</span>
                <span class="value">{:?}</span>
            </div>
        </div>
    </section>
"#,
                dns.dns_score,
                dns.success_rate,
                dns.total_queries,
                dns.successful_queries,
                dns.timeout_rate,
                dns.average_query_time
            )
        } else {
            String::new()
        }
    }
    
    fn generate_html_network_jitter(&self) -> String {
        if let Some(ref jitter) = self.metrics.network_jitter {
            format!(
                r#"
    <section class="test-section">
        <h2>Network Jitter Test</h2>
        <div class="metrics-grid">
            <div class="metric">
                <span class="label">Quality Score:</span>
                <span class="value">{:.1}/100</span>
            </div>
            <div class="metric">
                <span class="label">Packet Loss:</span>
                <span class="value">{:.2}%</span>
            </div>
            <div class="metric">
                <span class="label">Total Pings:</span>
                <span class="value">{}</span>
            </div>
            <div class="metric">
                <span class="label">Successful:</span>
                <span class="value">{}</span>
            </div>
            <div class="metric">
                <span class="label">Average RTT:</span>
                <span class="value">{:?}</span>
            </div>
            <div class="metric">
                <span class="label">Jitter:</span>
                <span class="value">{:?}</span>
            </div>
        </div>
    </section>
"#,
                jitter.network_quality_score,
                jitter.packet_loss_rate,
                jitter.total_pings,
                jitter.successful_pings,
                jitter.average_rtt,
                jitter.jitter
            )
        } else {
            String::new()
        }
    }
    
    fn get_rating(&self, score: f64) -> &'static str {
        match score {
            s if s >= 90.0 => "Excellent",
            s if s >= 80.0 => "Good",
            s if s >= 70.0 => "Fair", 
            s if s >= 60.0 => "Poor",
            _ => "Very Poor"
        }
    }
    
    fn save_to_file(&self, content: &str, file_path: &str) -> Result<()> {
        if let Some(parent) = Path::new(file_path).parent() {
            fs::create_dir_all(parent)
                .map_err(|e| NetworkTestError::Io(e))?;
        }
        
        fs::write(file_path, content)
            .map_err(|e| NetworkTestError::Io(e))?;
        
        Ok(())
    }
}