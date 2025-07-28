use tokio::net::UdpSocket;
use tracing::{debug, error, info, warn};

pub async fn run_dns_server(port: u16) -> Result<(), String> {
    let bind_addr = format!("0.0.0.0:{port}");
    let socket = UdpSocket::bind(&bind_addr)
        .await
        .map_err(|e| format!("Failed to bind UDP socket: {e}"))?;
    info!("DNS stability server listening on UDP {}", bind_addr);

    let mut buffer = [0u8; 512];

    loop {
        match socket.recv_from(&mut buffer).await {
            Ok((n, client_addr)) => {
                debug!("Received DNS query from {}: {} bytes", client_addr, n);

                if n < 12 {
                    warn!("Invalid DNS query from {}: too short", client_addr);
                    continue;
                }

                // Parse basic DNS query
                let query_id = u16::from_be_bytes([buffer[0], buffer[1]]);
                let _flags = u16::from_be_bytes([buffer[2], buffer[3]]);
                let questions = u16::from_be_bytes([buffer[4], buffer[5]]);

                if questions == 0 {
                    warn!("No questions in DNS query from {}", client_addr);
                    continue;
                }

                // Create DNS response
                let response_result = create_dns_response(query_id, &buffer[12..n]);
                match response_result {
                    Ok(response) => {
                        if let Err(e) = socket.send_to(&response, client_addr).await {
                            error!("Failed to send DNS response to {}: {}", client_addr, e);
                        } else {
                            debug!(
                                "Sent DNS response to {} for query ID: {}",
                                client_addr, query_id
                            );
                        }
                    }
                    Err(e) => {
                        error!("Failed to create DNS response: {}", e);
                    }
                }
            }
            Err(e) => {
                warn!("Error receiving DNS query: {}", e);
            }
        }
    }
}

fn create_dns_response(query_id: u16, question: &[u8]) -> Result<Vec<u8>, String> {
    let mut response = Vec::new();

    // DNS Header
    response.extend_from_slice(&query_id.to_be_bytes()); // ID
    response.extend_from_slice(&[0x81, 0x80]); // Flags: response, recursion available
    response.extend_from_slice(&[0x00, 0x01]); // Questions: 1
    response.extend_from_slice(&[0x00, 0x01]); // Answers: 1
    response.extend_from_slice(&[0x00, 0x00]); // Authority RRs: 0
    response.extend_from_slice(&[0x00, 0x00]); // Additional RRs: 0

    // Question section (copy from query)
    response.extend_from_slice(question);

    // Answer section
    // Name pointer to question
    response.extend_from_slice(&[0xc0, 0x0c]);
    // Type A (0x0001)
    response.extend_from_slice(&[0x00, 0x01]);
    // Class IN (0x0001)
    response.extend_from_slice(&[0x00, 0x01]);
    // TTL (300 seconds)
    response.extend_from_slice(&[0x00, 0x00, 0x01, 0x2c]);
    // Data length (4 bytes for IPv4)
    response.extend_from_slice(&[0x00, 0x04]);
    // IP address (8.8.8.8 as example)
    response.extend_from_slice(&[8, 8, 8, 8]);

    Ok(response)
}
