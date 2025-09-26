use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::debug;

pub async fn handle_client(mut stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    // For connection performance testing, we need to handle PING/PONG heartbeat
    
    let mut buffer = [0u8; 64];
    
    // Read the PING message
    match stream.read(&mut buffer).await {
        Ok(bytes_read) => {
            let message = std::str::from_utf8(&buffer[..bytes_read])
                .unwrap_or("")
                .trim();
            
            debug!("Connection performance test - received: {}", message);
            
            if message == "PING" {
                // Send PONG response
                let response = b"PONG\n";
                stream.write_all(response).await?;
                debug!("Connection performance test - sent PONG response");
            } else {
                // Send error response for unexpected message
                let response = b"ERROR\n";
                stream.write_all(response).await?;
                debug!("Connection performance test - unexpected message: {}", message);
            }
        }
        Err(e) => {
            debug!("Connection performance test - read error: {}", e);
            return Err(e.into());
        }
    }

    // Close connection after handling the heartbeat
    debug!("Connection performance test - heartbeat complete, closing");
    Ok(())
}
