use std::time::{Duration, Instant};
use tokio::time::{sleep, timeout};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{info, warn, debug};
use crate::{Result, NetworkTestError, Socks5Client};

#[derive(Debug, Clone)]
pub struct TcpStabilityTest {
    proxy_addr: String,
    target_addr: String,
    heartbeat_interval: Duration,
    test_duration: Duration,
}

#[derive(Debug, Clone)]
pub struct TcpStabilityResult {
    pub total_heartbeats: u64,
    pub successful_heartbeats: u64,
    pub failed_heartbeats: u64,
    pub reconnections: u64,
    pub total_downtime: Duration,
    pub average_rtt: Duration,
    pub max_rtt: Duration,
    pub min_rtt: Duration,
    pub connection_drops: Vec<ConnectionDrop>,
}

#[derive(Debug, Clone)]
pub struct ConnectionDrop {
    pub timestamp: Instant,
    pub duration: Duration,
    pub reason: String,
}

impl TcpStabilityTest {
    pub fn new(proxy_addr: &str, target_addr: &str, heartbeat_interval_sec: u64, test_duration_sec: u64) -> Self {
        Self {
            proxy_addr: proxy_addr.to_string(),
            target_addr: target_addr.to_string(),
            heartbeat_interval: Duration::from_secs(heartbeat_interval_sec),
            test_duration: Duration::from_secs(test_duration_sec),
        }
    }
    
    pub async fn run(&self) -> Result<()> {
        info!("Starting TCP stability test");
        info!("Proxy: {}, Target: {}", self.proxy_addr, self.target_addr);
        info!("Heartbeat interval: {:?}, Test duration: {:?}", 
              self.heartbeat_interval, self.test_duration);
        
        let proxy_addr = self.proxy_addr.parse()
            .map_err(|e| NetworkTestError::Config(format!("Invalid proxy address: {}", e)))?;
        
        let client = Socks5Client::new(proxy_addr)
            .with_timeout(Duration::from_secs(10));
        
        let result = self.run_stability_test(&client).await?;
        
        self.print_results(&result);
        
        Ok(())
    }
    
    async fn run_stability_test(&self, client: &Socks5Client) -> Result<TcpStabilityResult> {
        let start_time = Instant::now();
        let end_time = start_time + self.test_duration;
        
        let mut result = TcpStabilityResult {
            total_heartbeats: 0,
            successful_heartbeats: 0,
            failed_heartbeats: 0,
            reconnections: 0,
            total_downtime: Duration::ZERO,
            average_rtt: Duration::ZERO,
            max_rtt: Duration::ZERO,
            min_rtt: Duration::from_secs(u64::MAX),
            connection_drops: Vec::new(),
        };
        
        let mut rtt_sum = Duration::ZERO;
        let mut stream = None;
        let mut last_connection_attempt = Instant::now();
        let mut connection_broken = false;
        
        // Establish initial connection
        info!("Establishing initial connection...");
        match client.connect(&self.target_addr).await {
            Ok(tcp_stream) => {
                stream = Some(tcp_stream);
                info!("Initial connection established successfully");
            }
            Err(e) => {
                return Err(NetworkTestError::Connection(format!("Failed to establish initial connection: {}", e)));
            }
        }
        
        while Instant::now() < end_time {
            // Only reconnect if connection was broken
            if stream.is_none() && connection_broken {
                let connection_start = Instant::now();
                
                match client.connect(&self.target_addr).await {
                    Ok(new_stream) => {
                        stream = Some(new_stream);
                        result.reconnections += 1;
                        let downtime = connection_start - last_connection_attempt;
                        result.total_downtime += downtime;
                        
                        result.connection_drops.push(ConnectionDrop {
                            timestamp: last_connection_attempt,
                            duration: downtime,
                            reason: "Connection lost - reconnected".to_string(),
                        });
                        
                        info!("Reconnected after {:?} downtime", downtime);
                        connection_broken = false;
                    }
                    Err(e) => {
                        warn!("Failed to reconnect: {}", e);
                        sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                }
            }
            
            if let Some(ref mut tcp_stream) = stream {
                let heartbeat_start = Instant::now();
                result.total_heartbeats += 1;
                
                let heartbeat_data = format!("PING-{}\n", result.total_heartbeats);
                
                let heartbeat_result = timeout(
                    Duration::from_secs(5),
                    self.send_heartbeat(tcp_stream, &heartbeat_data)
                ).await;
                
                match heartbeat_result {
                    Ok(Ok(_)) => {
                        let rtt = heartbeat_start.elapsed();
                        result.successful_heartbeats += 1;
                        rtt_sum += rtt;
                        
                        if rtt > result.max_rtt {
                            result.max_rtt = rtt;
                        }
                        if rtt < result.min_rtt {
                            result.min_rtt = rtt;
                        }
                        
                        debug!("Heartbeat {} successful, RTT: {:?}", result.total_heartbeats, rtt);
                    }
                    Ok(Err(e)) => {
                        result.failed_heartbeats += 1;
                        warn!("Heartbeat {} failed, connection broken: {}", result.total_heartbeats, e);
                        stream = None;
                        connection_broken = true;
                        last_connection_attempt = Instant::now();
                    }
                    Err(_) => {
                        result.failed_heartbeats += 1;
                        warn!("Heartbeat {} timed out, connection may be broken", result.total_heartbeats);
                        stream = None;
                        connection_broken = true;
                        last_connection_attempt = Instant::now();
                    }
                }
            }
            
            sleep(self.heartbeat_interval).await;
        }
        
        if result.successful_heartbeats > 0 {
            result.average_rtt = rtt_sum / result.successful_heartbeats as u32;
        }
        
        if result.min_rtt == Duration::from_secs(u64::MAX) {
            result.min_rtt = Duration::ZERO;
        }
        
        Ok(result)
    }
    
    async fn send_heartbeat(&self, stream: &mut tokio::net::TcpStream, data: &str) -> Result<()> {
        stream.write_all(data.as_bytes()).await?;
        
        let mut buffer = [0u8; 1024];
        let n = stream.read(&mut buffer).await?;
        
        if n == 0 {
            return Err(NetworkTestError::Connection("Connection closed by peer".to_string()));
        }
        
        let response = String::from_utf8_lossy(&buffer[..n]);
        debug!("Received response: {}", response.trim());
        
        Ok(())
    }
    
    fn print_results(&self, result: &TcpStabilityResult) {
        println!("\n=== TCP Stability Test Results ===");
        println!("Test Duration: {:?}", self.test_duration);
        println!("Heartbeat Interval: {:?}", self.heartbeat_interval);
        println!();
        
        println!("Connection Statistics:");
        println!("  Total Heartbeats: {}", result.total_heartbeats);
        println!("  Successful: {} ({:.2}%)", 
                 result.successful_heartbeats,
                 if result.total_heartbeats > 0 {
                     (result.successful_heartbeats as f64 / result.total_heartbeats as f64) * 100.0
                 } else { 0.0 });
        println!("  Failed: {} ({:.2}%)", 
                 result.failed_heartbeats,
                 if result.total_heartbeats > 0 {
                     (result.failed_heartbeats as f64 / result.total_heartbeats as f64) * 100.0
                 } else { 0.0 });
        println!("  Reconnections: {}", result.reconnections);
        println!();
        
        if result.successful_heartbeats > 0 {
            println!("Latency Statistics:");
            println!("  Average RTT: {:?}", result.average_rtt);
            println!("  Min RTT: {:?}", result.min_rtt);
            println!("  Max RTT: {:?}", result.max_rtt);
            println!();
        }
        
        if !result.connection_drops.is_empty() {
            println!("Connection Stability:");
            println!("  Total Downtime: {:?}", result.total_downtime);
            println!("  Connection Drops: {}", result.connection_drops.len());
            
            let uptime_percentage = if self.test_duration > result.total_downtime {
                ((self.test_duration - result.total_downtime).as_secs_f64() / self.test_duration.as_secs_f64()) * 100.0
            } else {
                0.0
            };
            println!("  Uptime: {:.2}%", uptime_percentage);
            
            if result.connection_drops.len() <= 5 {
                println!("\n  Connection Drop Details:");
                for (i, drop) in result.connection_drops.iter().enumerate() {
                    println!("    Drop {}: Duration {:?}, Reason: {}", 
                             i + 1, drop.duration, drop.reason);
                }
            }
        } else {
            println!("Connection Stability: Perfect (no drops detected)");
        }
        
        println!();
        
        let stability_score = if result.total_heartbeats > 0 {
            let success_rate = result.successful_heartbeats as f64 / result.total_heartbeats as f64;
            let connection_stability = if result.reconnections == 0 { 1.0 } else { 
                1.0 / (1.0 + result.reconnections as f64 * 0.1) 
            };
            (success_rate * connection_stability * 100.0).min(100.0)
        } else {
            0.0
        };
        
        println!("Overall Stability Score: {:.1}/100", stability_score);
    }
}