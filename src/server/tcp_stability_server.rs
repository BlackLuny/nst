use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tracing::{debug, warn};

pub async fn handle_client(mut stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let (reader, mut writer) = stream.split();
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        match buf_reader.read_line(&mut line).await {
            Ok(0) => {
                debug!("Client disconnected");
                break;
            }
            Ok(_) => {
                let line = line.trim();
                if let Some(number) = line.strip_prefix("PING-") {
                    let response = format!("PONG-{number}\n");

                    if let Err(e) = writer.write_all(response.as_bytes()).await {
                        warn!("Failed to send response: {}", e);
                        break;
                    }

                    debug!("Responded to {}", line);
                } else {
                    warn!("Unknown command: {}", line);
                }
            }
            Err(e) => {
                warn!("Error reading from client: {}", e);
                break;
            }
        }
    }

    Ok(())
}
