use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::net::SocketAddr;
use tracing::{debug, info};
use crate::{Result, NetworkTestError};

#[derive(Debug, Clone)]
pub struct Socks5Client {
    proxy_addr: SocketAddr,
    username: Option<String>,
    password: Option<String>,
    timeout: std::time::Duration,
}

impl Socks5Client {
    pub fn new(proxy_addr: SocketAddr) -> Self {
        Self {
            proxy_addr,
            username: None,
            password: None,
            timeout: std::time::Duration::from_secs(5),
        }
    }
    
    pub fn with_auth(mut self, username: String, password: String) -> Self {
        self.username = Some(username);
        self.password = Some(password);
        self
    }
    
    pub fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.timeout = timeout;
        self
    }
    
    pub async fn connect(&self, target_addr: &str) -> Result<TcpStream> {
        debug!("Connecting to SOCKS5 proxy at {}", self.proxy_addr);
        
        let mut stream = tokio::time::timeout(
            self.timeout,
            TcpStream::connect(self.proxy_addr)
        ).await
        .map_err(|_| NetworkTestError::Timeout("Failed to connect to SOCKS5 proxy".to_string()))?
        .map_err(|e| NetworkTestError::Connection(format!("Failed to connect to proxy: {}", e)))?;
        
        self.socks5_handshake(&mut stream).await?;
        
        self.socks5_connect(&mut stream, target_addr).await?;
        
        info!("Successfully connected to {} via SOCKS5 proxy", target_addr);
        Ok(stream)
    }
    
    async fn socks5_handshake(&self, stream: &mut TcpStream) -> Result<()> {
        debug!("Performing SOCKS5 handshake");
        
        let auth_method = if self.username.is_some() && self.password.is_some() {
            0x02u8
        } else {
            0x00u8
        };
        
        let handshake = [0x05, 0x01, auth_method];
        stream.write_all(&handshake).await?;
        
        let mut response = [0u8; 2];
        stream.read_exact(&mut response).await?;
        
        if response[0] != 0x05 {
            return Err(NetworkTestError::Socks5("Invalid SOCKS version in response".to_string()));
        }
        
        match response[1] {
            0x00 => {
                debug!("No authentication required");
            }
            0x02 => {
                debug!("Username/password authentication required");
                self.authenticate(stream).await?;
            }
            0xFF => {
                return Err(NetworkTestError::Socks5("No acceptable authentication methods".to_string()));
            }
            _ => {
                return Err(NetworkTestError::Socks5(format!("Unknown authentication method: {}", response[1])));
            }
        }
        
        Ok(())
    }
    
    async fn authenticate(&self, stream: &mut TcpStream) -> Result<()> {
        let username = self.username.as_ref().ok_or_else(|| {
            NetworkTestError::Socks5("Username required for authentication".to_string())
        })?;
        let password = self.password.as_ref().ok_or_else(|| {
            NetworkTestError::Socks5("Password required for authentication".to_string())
        })?;
        
        debug!("Authenticating with username/password");
        
        let mut auth_request = Vec::new();
        auth_request.push(0x01);
        auth_request.push(username.len() as u8);
        auth_request.extend_from_slice(username.as_bytes());
        auth_request.push(password.len() as u8);
        auth_request.extend_from_slice(password.as_bytes());
        
        stream.write_all(&auth_request).await?;
        
        let mut response = [0u8; 2];
        stream.read_exact(&mut response).await?;
        
        if response[0] != 0x01 {
            return Err(NetworkTestError::Socks5("Invalid authentication response version".to_string()));
        }
        
        if response[1] != 0x00 {
            return Err(NetworkTestError::Socks5("Authentication failed".to_string()));
        }
        
        debug!("Authentication successful");
        Ok(())
    }
    
    async fn socks5_connect(&self, stream: &mut TcpStream, target_addr: &str) -> Result<()> {
        debug!("Requesting connection to {}", target_addr);
        
        let (host, port) = self.parse_address(target_addr)?;
        
        let mut connect_request = Vec::new();
        connect_request.extend_from_slice(&[0x05, 0x01, 0x00]);
        
        if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            match ip {
                std::net::IpAddr::V4(ipv4) => {
                    connect_request.push(0x01);
                    connect_request.extend_from_slice(&ipv4.octets());
                }
                std::net::IpAddr::V6(ipv6) => {
                    connect_request.push(0x04);
                    connect_request.extend_from_slice(&ipv6.octets());
                }
            }
        } else {
            connect_request.push(0x03);
            connect_request.push(host.len() as u8);
            connect_request.extend_from_slice(host.as_bytes());
        }
        
        connect_request.extend_from_slice(&port.to_be_bytes());
        
        stream.write_all(&connect_request).await?;
        
        let mut response = [0u8; 4];
        stream.read_exact(&mut response).await?;
        
        if response[0] != 0x05 {
            return Err(NetworkTestError::Socks5("Invalid SOCKS version in connect response".to_string()));
        }
        
        match response[1] {
            0x00 => debug!("Connection established"),
            0x01 => return Err(NetworkTestError::Socks5("General SOCKS server failure".to_string())),
            0x02 => return Err(NetworkTestError::Socks5("Connection not allowed by ruleset".to_string())),
            0x03 => return Err(NetworkTestError::Socks5("Network unreachable".to_string())),
            0x04 => return Err(NetworkTestError::Socks5("Host unreachable".to_string())),
            0x05 => return Err(NetworkTestError::Socks5("Connection refused".to_string())),
            0x06 => return Err(NetworkTestError::Socks5("TTL expired".to_string())),
            0x07 => return Err(NetworkTestError::Socks5("Command not supported".to_string())),
            0x08 => return Err(NetworkTestError::Socks5("Address type not supported".to_string())),
            _ => return Err(NetworkTestError::Socks5(format!("Unknown error code: {}", response[1]))),
        }
        
        let addr_type = response[3];
        match addr_type {
            0x01 => {
                let mut addr = [0u8; 6];
                stream.read_exact(&mut addr).await?;
            }
            0x03 => {
                let mut len = [0u8; 1];
                stream.read_exact(&mut len).await?;
                let mut addr = vec![0u8; len[0] as usize + 2];
                stream.read_exact(&mut addr).await?;
            }
            0x04 => {
                let mut addr = [0u8; 18];
                stream.read_exact(&mut addr).await?;
            }
            _ => {
                return Err(NetworkTestError::Socks5(format!("Unknown address type: {}", addr_type)));
            }
        }
        
        Ok(())
    }
    
    fn parse_address(&self, addr: &str) -> Result<(String, u16)> {
        let parts: Vec<&str> = addr.rsplitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(NetworkTestError::Config(format!("Invalid address format: {}", addr)));
        }
        
        let port = parts[0].parse::<u16>()
            .map_err(|_| NetworkTestError::Config(format!("Invalid port: {}", parts[0])))?;
        let host = parts[1].to_string();
        
        Ok((host, port))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_address() {
        let client = Socks5Client::new("127.0.0.1:1080".parse().unwrap());
        
        let (host, port) = client.parse_address("example.com:80").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 80);
        
        let (host, port) = client.parse_address("192.168.1.1:443").unwrap();
        assert_eq!(host, "192.168.1.1");
        assert_eq!(port, 443);
    }
}