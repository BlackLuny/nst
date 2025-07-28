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
                debug!("Jitter test client disconnected");
                break;
            }
            Ok(_) => {
                let line = line.trim();
                if line == "PING" {
                    let response = b"PONG\n";

                    if let Err(e) = writer.write_all(response).await {
                        warn!("Failed to send PONG response: {}", e);
                        break;
                    }

                    debug!("Responded to PING with PONG");
                } else {
                    warn!("Unknown jitter test command: {}", line);
                }
            }
            Err(e) => {
                warn!("Error reading from jitter test client: {}", e);
                break;
            }
        }
    }

    Ok(())
}
