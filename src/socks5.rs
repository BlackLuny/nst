use crate::{NetworkTestError, Result};
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tracing::{debug, info};

#[derive(Debug, Clone)]
pub struct Socks5Client {
    proxy_addr: SocketAddr,
    username: Option<String>,
    password: Option<String>,
    timeout: std::time::Duration,
}

#[derive(Debug)]
pub struct Socks5UdpRelay {
    pub socket: UdpSocket,
    pub relay_addr: SocketAddr,
    _control_stream: TcpStream,
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

        let mut stream = tokio::time::timeout(self.timeout, TcpStream::connect(self.proxy_addr))
            .await
            .map_err(|_| {
                NetworkTestError::Timeout("Failed to connect to SOCKS5 proxy".to_string())
            })?
            .map_err(|e| {
                NetworkTestError::Connection(format!("Failed to connect to proxy: {e}"))
            })?;

        self.socks5_handshake(&mut stream).await?;

        self.socks5_connect(&mut stream, target_addr).await?;

        info!("Successfully connected to {} via SOCKS5 proxy", target_addr);
        Ok(stream)
    }

    pub async fn udp_associate(&self) -> Result<Socks5UdpRelay> {
        debug!(
            "Creating UDP association with SOCKS5 proxy at {}",
            self.proxy_addr
        );

        let mut stream = tokio::time::timeout(self.timeout, TcpStream::connect(self.proxy_addr))
            .await
            .map_err(|_| {
                NetworkTestError::Timeout("Failed to connect to SOCKS5 proxy".to_string())
            })?
            .map_err(|e| {
                NetworkTestError::Connection(format!("Failed to connect to proxy: {e}"))
            })?;

        self.socks5_handshake(&mut stream).await?;

        let relay_addr = self.socks5_udp_associate(&mut stream).await?;

        let socket = UdpSocket::bind("0.0.0.0:0").await.map_err(|e| {
            NetworkTestError::Connection(format!("Failed to bind UDP socket: {e}"))
        })?;

        info!(
            "Successfully created UDP association via SOCKS5 proxy, relay at {}",
            relay_addr
        );

        Ok(Socks5UdpRelay {
            socket,
            relay_addr,
            _control_stream: stream,
        })
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
            return Err(NetworkTestError::Socks5(
                "Invalid SOCKS version in response".to_string(),
            ));
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
                return Err(NetworkTestError::Socks5(
                    "No acceptable authentication methods".to_string(),
                ));
            }
            _ => {
                return Err(NetworkTestError::Socks5(format!(
                    "Unknown authentication method: {}",
                    response[1]
                )));
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
            return Err(NetworkTestError::Socks5(
                "Invalid authentication response version".to_string(),
            ));
        }

        if response[1] != 0x00 {
            return Err(NetworkTestError::Socks5(
                "Authentication failed".to_string(),
            ));
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
            return Err(NetworkTestError::Socks5(
                "Invalid SOCKS version in connect response".to_string(),
            ));
        }

        match response[1] {
            0x00 => debug!("Connection established"),
            0x01 => {
                return Err(NetworkTestError::Socks5(
                    "General SOCKS server failure".to_string(),
                ))
            }
            0x02 => {
                return Err(NetworkTestError::Socks5(
                    "Connection not allowed by ruleset".to_string(),
                ))
            }
            0x03 => return Err(NetworkTestError::Socks5("Network unreachable".to_string())),
            0x04 => return Err(NetworkTestError::Socks5("Host unreachable".to_string())),
            0x05 => return Err(NetworkTestError::Socks5("Connection refused".to_string())),
            0x06 => return Err(NetworkTestError::Socks5("TTL expired".to_string())),
            0x07 => {
                return Err(NetworkTestError::Socks5(
                    "Command not supported".to_string(),
                ))
            }
            0x08 => {
                return Err(NetworkTestError::Socks5(
                    "Address type not supported".to_string(),
                ))
            }
            _ => {
                return Err(NetworkTestError::Socks5(format!(
                    "Unknown error code: {}",
                    response[1]
                )))
            }
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
                return Err(NetworkTestError::Socks5(format!(
                    "Unknown address type: {addr_type}"
                )));
            }
        }

        Ok(())
    }

    async fn socks5_udp_associate(&self, stream: &mut TcpStream) -> Result<SocketAddr> {
        debug!("Requesting UDP association");

        let mut request = Vec::new();
        request.extend_from_slice(&[0x05, 0x03, 0x00]);

        request.push(0x01);
        request.extend_from_slice(&[0, 0, 0, 0]);
        request.extend_from_slice(&[0, 0]);

        stream.write_all(&request).await?;

        let mut response = [0u8; 4];
        stream.read_exact(&mut response).await?;

        if response[0] != 0x05 {
            return Err(NetworkTestError::Socks5(
                "Invalid SOCKS version in UDP associate response".to_string(),
            ));
        }

        match response[1] {
            0x00 => debug!("UDP association established"),
            0x01 => {
                return Err(NetworkTestError::Socks5(
                    "General SOCKS server failure".to_string(),
                ))
            }
            0x02 => {
                return Err(NetworkTestError::Socks5(
                    "Connection not allowed by ruleset".to_string(),
                ))
            }
            0x03 => return Err(NetworkTestError::Socks5("Network unreachable".to_string())),
            0x04 => return Err(NetworkTestError::Socks5("Host unreachable".to_string())),
            0x05 => return Err(NetworkTestError::Socks5("Connection refused".to_string())),
            0x06 => return Err(NetworkTestError::Socks5("TTL expired".to_string())),
            0x07 => {
                return Err(NetworkTestError::Socks5(
                    "Command not supported".to_string(),
                ))
            }
            0x08 => {
                return Err(NetworkTestError::Socks5(
                    "Address type not supported".to_string(),
                ))
            }
            _ => {
                return Err(NetworkTestError::Socks5(format!(
                    "Unknown error code: {}",
                    response[1]
                )))
            }
        }

        let addr_type = response[3];
        let relay_addr = match addr_type {
            0x01 => {
                let mut addr = [0u8; 6];
                stream.read_exact(&mut addr).await?;
                let ip = std::net::Ipv4Addr::new(addr[0], addr[1], addr[2], addr[3]);
                let port = u16::from_be_bytes([addr[4], addr[5]]);
                SocketAddr::new(ip.into(), port)
            }
            0x04 => {
                let mut addr = [0u8; 18];
                stream.read_exact(&mut addr).await?;
                let ip = std::net::Ipv6Addr::from([
                    addr[0], addr[1], addr[2], addr[3], addr[4], addr[5], addr[6], addr[7],
                    addr[8], addr[9], addr[10], addr[11], addr[12], addr[13], addr[14], addr[15],
                ]);
                let port = u16::from_be_bytes([addr[16], addr[17]]);
                SocketAddr::new(ip.into(), port)
            }
            _ => {
                return Err(NetworkTestError::Socks5(format!(
                    "Unsupported address type for UDP relay: {addr_type}"
                )));
            }
        };

        Ok(relay_addr)
    }

    fn parse_address(&self, addr: &str) -> Result<(String, u16)> {
        let parts: Vec<&str> = addr.rsplitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(NetworkTestError::Config(format!(
                "Invalid address format: {addr}"
            )));
        }

        let port = parts[0]
            .parse::<u16>()
            .map_err(|_| NetworkTestError::Config(format!("Invalid port: {}", parts[0])))?;
        let host = parts[1].to_string();

        Ok((host, port))
    }
}

impl Socks5UdpRelay {
    pub async fn send_to(&self, data: &[u8], target_addr: &str) -> Result<()> {
        let packet = self.encapsulate_udp_packet(data, target_addr)?;
        self.socket
            .send_to(&packet, self.relay_addr)
            .await
            .map_err(NetworkTestError::Io)?;
        Ok(())
    }

    pub async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, String)> {
        let (n, _) = self
            .socket
            .recv_from(buf)
            .await
            .map_err(NetworkTestError::Io)?;

        if n < 10 {
            return Err(NetworkTestError::Connection(
                "Invalid SOCKS5 UDP packet: too short".to_string(),
            ));
        }

        if buf[0] != 0x00 || buf[1] != 0x00 {
            return Err(NetworkTestError::Connection(
                "Invalid SOCKS5 UDP packet: bad header".to_string(),
            ));
        }

        if buf[2] != 0x00 {
            return Err(NetworkTestError::Connection(
                "Fragmentation not supported".to_string(),
            ));
        }

        let addr_type = buf[3];
        let (header_len, target_addr) = match addr_type {
            0x01 => {
                if n < 10 {
                    return Err(NetworkTestError::Connection(
                        "Invalid IPv4 UDP packet".to_string(),
                    ));
                }
                let ip = std::net::Ipv4Addr::new(buf[4], buf[5], buf[6], buf[7]);
                let port = u16::from_be_bytes([buf[8], buf[9]]);
                (10, format!("{ip}:{port}"))
            }
            0x03 => {
                if n < 5 {
                    return Err(NetworkTestError::Connection(
                        "Invalid domain UDP packet".to_string(),
                    ));
                }
                let domain_len = buf[4] as usize;
                if n < 5 + domain_len + 2 {
                    return Err(NetworkTestError::Connection(
                        "Invalid domain UDP packet: too short".to_string(),
                    ));
                }
                let domain = String::from_utf8_lossy(&buf[5..5 + domain_len]);
                let port = u16::from_be_bytes([buf[5 + domain_len], buf[5 + domain_len + 1]]);
                (5 + domain_len + 2, format!("{domain}:{port}"))
            }
            0x04 => {
                if n < 22 {
                    return Err(NetworkTestError::Connection(
                        "Invalid IPv6 UDP packet".to_string(),
                    ));
                }
                let ip = std::net::Ipv6Addr::from([
                    buf[4], buf[5], buf[6], buf[7], buf[8], buf[9], buf[10], buf[11], buf[12],
                    buf[13], buf[14], buf[15], buf[16], buf[17], buf[18], buf[19],
                ]);
                let port = u16::from_be_bytes([buf[20], buf[21]]);
                (22, format!("{ip}:{port}"))
            }
            _ => {
                return Err(NetworkTestError::Connection(format!(
                    "Unsupported address type: {addr_type}"
                )));
            }
        };

        let data_len = n - header_len;
        buf.copy_within(header_len..n, 0);

        Ok((data_len, target_addr))
    }

    fn encapsulate_udp_packet(&self, data: &[u8], target_addr: &str) -> Result<Vec<u8>> {
        let mut packet = Vec::new();

        packet.extend_from_slice(&[0x00, 0x00]);
        packet.push(0x00);

        let (host, port) = self.parse_address(target_addr)?;

        if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            match ip {
                std::net::IpAddr::V4(ipv4) => {
                    packet.push(0x01);
                    packet.extend_from_slice(&ipv4.octets());
                }
                std::net::IpAddr::V6(ipv6) => {
                    packet.push(0x04);
                    packet.extend_from_slice(&ipv6.octets());
                }
            }
        } else {
            packet.push(0x03);
            packet.push(host.len() as u8);
            packet.extend_from_slice(host.as_bytes());
        }

        packet.extend_from_slice(&port.to_be_bytes());
        packet.extend_from_slice(data);

        Ok(packet)
    }

    fn parse_address(&self, addr: &str) -> Result<(String, u16)> {
        let parts: Vec<&str> = addr.rsplitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(NetworkTestError::Config(format!(
                "Invalid address format: {addr}"
            )));
        }

        let port = parts[0]
            .parse::<u16>()
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
