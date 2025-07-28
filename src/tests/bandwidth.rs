use crate::{NetworkTestError, Result, Socks5Client};
use rand::Rng;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{sleep, timeout};
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone)]
pub struct BandwidthTest {
    proxy_addr: String,
    target_addr: String,
    chunk_size: usize,
    test_duration: Duration,
}

#[derive(Debug, Clone)]
pub struct BandwidthResult {
    pub test_duration: Duration,
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,
    pub average_upload_speed: f64,
    pub average_download_speed: f64,
    pub upload_samples: Vec<SpeedSample>,
    pub download_samples: Vec<SpeedSample>,
    pub connection_interruptions: u32,
    pub data_integrity_errors: u32,
}

#[derive(Debug, Clone)]
pub struct SpeedSample {
    pub timestamp: Instant,
    pub bytes_per_second: f64,
    pub chunk_size: usize,
    pub duration: Duration,
}

impl BandwidthTest {
    pub fn new(
        proxy_addr: &str,
        target_addr: &str,
        chunk_size: usize,
        test_duration_sec: u64,
    ) -> Self {
        Self {
            proxy_addr: proxy_addr.to_string(),
            target_addr: target_addr.to_string(),
            chunk_size,
            test_duration: Duration::from_secs(test_duration_sec),
        }
    }

    pub async fn run(&self) -> Result<()> {
        info!("Starting bandwidth test");
        info!("Proxy: {}, Target: {}", self.proxy_addr, self.target_addr);
        info!(
            "Chunk size: {} bytes, Test duration: {:?}",
            self.chunk_size, self.test_duration
        );

        let proxy_addr = self
            .proxy_addr
            .parse()
            .map_err(|e| NetworkTestError::Config(format!("Invalid proxy address: {e}")))?;

        let client = Socks5Client::new(proxy_addr).with_timeout(Duration::from_secs(10));

        let result = self.run_bandwidth_test(&client).await?;

        self.print_results(&result);

        Ok(())
    }

    async fn run_bandwidth_test(&self, client: &Socks5Client) -> Result<BandwidthResult> {
        let start_time = Instant::now();
        let end_time = start_time + self.test_duration;

        let mut result = BandwidthResult {
            test_duration: self.test_duration,
            total_bytes_sent: 0,
            total_bytes_received: 0,
            average_upload_speed: 0.0,
            average_download_speed: 0.0,
            upload_samples: Vec::new(),
            download_samples: Vec::new(),
            connection_interruptions: 0,
            data_integrity_errors: 0,
        };

        let mut stream = client.connect(&self.target_addr).await?;
        info!("Connected to target via SOCKS5 proxy");

        let http_request = self.create_http_request();
        stream.write_all(http_request.as_bytes()).await?;

        let _headers_received = false;
        let _content_length: Option<usize> = None;
        let _response_buffer: Vec<u8> = Vec::new();

        while Instant::now() < end_time {
            let chunk_start = Instant::now();

            match self
                .perform_data_transfer(&mut stream, &mut result, chunk_start)
                .await
            {
                Ok(_) => {
                    debug!("Data transfer chunk completed successfully");
                }
                Err(e) => {
                    warn!("Data transfer error: {}. Attempting to reconnect...", e);
                    result.connection_interruptions += 1;

                    match client.connect(&self.target_addr).await {
                        Ok(new_stream) => {
                            stream = new_stream;
                            let http_request = self.create_http_request();
                            if let Err(e) = stream.write_all(http_request.as_bytes()).await {
                                error!("Failed to send HTTP request after reconnection: {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            error!("Failed to reconnect: {}", e);
                            break;
                        }
                    }
                }
            }

            sleep(Duration::from_millis(100)).await;
        }

        result.average_upload_speed = self.calculate_average_speed(&result.upload_samples);
        result.average_download_speed = self.calculate_average_speed(&result.download_samples);

        Ok(result)
    }

    async fn perform_data_transfer(
        &self,
        stream: &mut tokio::net::TcpStream,
        result: &mut BandwidthResult,
        _chunk_start: Instant,
    ) -> Result<()> {
        let test_data = self.generate_test_data();
        let _data_checksum = self.calculate_checksum(&test_data);

        let upload_start = Instant::now();

        let upload_request = format!(
            "POST /post HTTP/1.1\r\nHost: {}\r\nContent-Length: {}\r\nConnection: keep-alive\r\n\r\n",
            self.get_host_from_addr(),
            test_data.len()
        );

        stream.write_all(upload_request.as_bytes()).await?;
        stream.write_all(&test_data).await?;

        let upload_duration = upload_start.elapsed();
        let upload_speed = test_data.len() as f64 / upload_duration.as_secs_f64();

        result.total_bytes_sent += test_data.len() as u64;
        result.upload_samples.push(SpeedSample {
            timestamp: upload_start,
            bytes_per_second: upload_speed,
            chunk_size: test_data.len(),
            duration: upload_duration,
        });

        let download_start = Instant::now();
        let mut response_buffer = Vec::with_capacity(8192);
        let mut bytes_read = 0;

        loop {
            let mut buffer = [0u8; 4096];
            match timeout(Duration::from_secs(5), stream.read(&mut buffer)).await {
                Ok(Ok(n)) if n > 0 => {
                    response_buffer.extend_from_slice(&buffer[..n]);
                    bytes_read += n;

                    if response_buffer.len() >= 4 && response_buffer.ends_with(b"\r\n\r\n") {
                        break;
                    }

                    if bytes_read >= self.chunk_size * 2 {
                        break;
                    }
                }
                Ok(Ok(0)) => {
                    break;
                }
                Ok(Ok(_)) => {
                    // Handle any other positive read size
                    break;
                }
                Ok(Err(e)) => {
                    return Err(NetworkTestError::Io(e));
                }
                Err(_) => {
                    warn!("Download timeout");
                    break;
                }
            }
        }

        let download_duration = download_start.elapsed();
        let download_speed = if download_duration.as_secs_f64() > 0.0 {
            bytes_read as f64 / download_duration.as_secs_f64()
        } else {
            0.0
        };

        result.total_bytes_received += bytes_read as u64;
        result.download_samples.push(SpeedSample {
            timestamp: download_start,
            bytes_per_second: download_speed,
            chunk_size: bytes_read,
            duration: download_duration,
        });

        if self.verify_response_integrity(&response_buffer) {
            debug!("Response integrity verified");
        } else {
            result.data_integrity_errors += 1;
            warn!("Data integrity error detected");
        }

        Ok(())
    }

    fn generate_test_data(&self) -> Vec<u8> {
        let mut rng = rand::thread_rng();
        let mut data = Vec::with_capacity(self.chunk_size);

        for _ in 0..self.chunk_size {
            data.push(rng.gen::<u8>());
        }

        data
    }

    fn calculate_checksum(&self, data: &[u8]) -> u32 {
        data.iter().map(|&b| b as u32).sum()
    }

    fn verify_response_integrity(&self, response: &[u8]) -> bool {
        let response_str = String::from_utf8_lossy(response);
        response_str.contains("HTTP/")
            && (response_str.contains("200 OK")
                || response_str.contains("201 Created")
                || response_str.contains("204 No Content"))
    }

    fn create_http_request(&self) -> String {
        format!(
            "GET /stream-bytes/{} HTTP/1.1\r\nHost: {}\r\nConnection: keep-alive\r\nUser-Agent: NetworkStabilityTest/1.0\r\n\r\n",
            self.chunk_size,
            self.get_host_from_addr()
        )
    }

    fn get_host_from_addr(&self) -> &str {
        if let Some(colon_pos) = self.target_addr.rfind(':') {
            &self.target_addr[..colon_pos]
        } else {
            &self.target_addr
        }
    }

    fn calculate_average_speed(&self, samples: &[SpeedSample]) -> f64 {
        if samples.is_empty() {
            return 0.0;
        }

        let total_speed: f64 = samples.iter().map(|s| s.bytes_per_second).sum();
        total_speed / samples.len() as f64
    }

    fn print_results(&self, result: &BandwidthResult) {
        println!("\n=== Bandwidth Test Results ===");
        println!("Test Duration: {:?}", result.test_duration);
        println!("Chunk Size: {} bytes", self.chunk_size);
        println!();

        println!("Data Transfer Statistics:");
        println!(
            "  Total Bytes Sent: {} ({:.2} MB)",
            result.total_bytes_sent,
            result.total_bytes_sent as f64 / 1_048_576.0
        );
        println!(
            "  Total Bytes Received: {} ({:.2} MB)",
            result.total_bytes_received,
            result.total_bytes_received as f64 / 1_048_576.0
        );
        println!();

        println!("Speed Statistics:");
        println!(
            "  Average Upload Speed: {:.2} KB/s ({:.2} Mbps)",
            result.average_upload_speed / 1024.0,
            (result.average_upload_speed * 8.0) / 1_000_000.0
        );
        println!(
            "  Average Download Speed: {:.2} KB/s ({:.2} Mbps)",
            result.average_download_speed / 1024.0,
            (result.average_download_speed * 8.0) / 1_000_000.0
        );

        if !result.upload_samples.is_empty() {
            let max_upload = result
                .upload_samples
                .iter()
                .map(|s| s.bytes_per_second)
                .fold(0.0f64, f64::max);
            let min_upload = result
                .upload_samples
                .iter()
                .map(|s| s.bytes_per_second)
                .fold(f64::INFINITY, f64::min);

            println!(
                "  Upload Speed Range: {:.2} - {:.2} KB/s",
                min_upload / 1024.0,
                max_upload / 1024.0
            );
        }

        if !result.download_samples.is_empty() {
            let max_download = result
                .download_samples
                .iter()
                .map(|s| s.bytes_per_second)
                .fold(0.0f64, f64::max);
            let min_download = result
                .download_samples
                .iter()
                .map(|s| s.bytes_per_second)
                .fold(f64::INFINITY, f64::min);

            println!(
                "  Download Speed Range: {:.2} - {:.2} KB/s",
                min_download / 1024.0,
                max_download / 1024.0
            );
        }
        println!();

        println!("Connection Quality:");
        println!(
            "  Connection Interruptions: {}",
            result.connection_interruptions
        );
        println!("  Data Integrity Errors: {}", result.data_integrity_errors);

        let total_samples = result.upload_samples.len() + result.download_samples.len();
        let error_rate = if total_samples > 0 {
            (result.data_integrity_errors as f64 / total_samples as f64) * 100.0
        } else {
            0.0
        };
        println!("  Error Rate: {error_rate:.2}%");

        let stability_score = if total_samples > 0 {
            let connection_stability = if result.connection_interruptions == 0 {
                1.0
            } else {
                1.0 / (1.0 + result.connection_interruptions as f64 * 0.2)
            };
            let integrity_score = 1.0 - (error_rate / 100.0);
            (connection_stability * integrity_score * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        };

        println!("  Bandwidth Stability Score: {stability_score:.1}/100");
        println!();

        let speed_consistency = if result.upload_samples.len() > 1 {
            let mean = result.average_upload_speed;
            let variance: f64 = result
                .upload_samples
                .iter()
                .map(|s| (s.bytes_per_second - mean).powi(2))
                .sum::<f64>()
                / result.upload_samples.len() as f64;
            let std_dev = variance.sqrt();
            let coefficient_of_variation = if mean > 0.0 { std_dev / mean } else { 0.0 };
            1.0 - coefficient_of_variation.min(1.0)
        } else {
            1.0
        };

        println!("Performance Metrics:");
        println!("  Speed Consistency: {:.1}%", speed_consistency * 100.0);

        if !result.upload_samples.is_empty() && !result.download_samples.is_empty() {
            let total_transfer_time: Duration = result
                .upload_samples
                .iter()
                .chain(result.download_samples.iter())
                .map(|s| s.duration)
                .sum();
            let avg_transfer_time = total_transfer_time
                / (result.upload_samples.len() + result.download_samples.len()) as u32;
            println!("  Average Transfer Time: {avg_transfer_time:?}");
        }
    }
}
