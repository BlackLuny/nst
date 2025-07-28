use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::{debug, warn, error};

pub async fn handle_client(mut stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let mut buffer = [0u8; 512];
    
    loop {
        match stream.read(&mut buffer).await {
            Ok(0) => {
                debug!("DNS client disconnected");
                break;
            }
            Ok(n) => {
                debug!("Received DNS query: {} bytes", n);
                
                if n < 12 {
                    warn!("Invalid DNS query: too short");
                    continue;
                }
                
                // Parse basic DNS query
                let query_id = u16::from_be_bytes([buffer[0], buffer[1]]);
                let _flags = u16::from_be_bytes([buffer[2], buffer[3]]);
                let questions = u16::from_be_bytes([buffer[4], buffer[5]]);
                
                if questions == 0 {
                    warn!("No questions in DNS query");
                    continue;
                }
                
                // Create DNS response
                let response = create_dns_response(query_id, &buffer[12..n])?;
                
                if let Err(e) = stream.write_all(&response).await {
                    error!("Failed to send DNS response: {}", e);
                    break;
                }
                
                debug!("Sent DNS response for query ID: {}", query_id);
            }
            Err(e) => {
                warn!("Error reading DNS query: {}", e);
                break;
            }
        }
    }
    
    Ok(())
}

fn create_dns_response(query_id: u16, question: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
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