use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::{debug, warn, error};

pub async fn handle_client(mut stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let mut buffer = [0u8; 1024];
    
    loop {
        match stream.read(&mut buffer).await {
            Ok(0) => {
                debug!("Client disconnected");
                break;
            }
            Ok(n) => {
                let request = String::from_utf8_lossy(&buffer[..n]);
                let lines: Vec<&str> = request.lines().collect();
                
                if lines.is_empty() {
                    continue;
                }
                
                let request_line = lines[0];
                debug!("Received request: {}", request_line);

                if request_line.starts_with("GET /stream-bytes/") {
                    let size_str = &request_line[18..].split_whitespace().next().unwrap_or("1024");
                    let size: usize = size_str.parse().unwrap_or(1024);
                    
                    if let Err(e) = handle_get_stream_bytes(&mut stream, size).await {
                        error!("Error handling GET request: {}", e);
                        break;
                    }
                } else if request_line.starts_with("POST /post") {
                    if let Err(e) = handle_post_request(&mut stream, &request).await {
                        error!("Error handling POST request: {}", e);
                        break;
                    }
                } else if request_line.is_empty() {
                    continue;
                } else {
                    warn!("Unknown request: {}", request_line);
                    let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";
                    stream.write_all(response.as_bytes()).await?;
                }
            }
            Err(e) => {
                warn!("Error reading request: {}", e);
                break;
            }
        }
    }

    Ok(())
}

async fn handle_get_stream_bytes(
    stream: &mut TcpStream,
    size: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    // Generate simple test data (pattern instead of random for Send safety)
    let mut data = Vec::with_capacity(size);
    for i in 0..size {
        data.push((i % 256) as u8);
    }

    // Send HTTP response
    let response_header = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: keep-alive\r\n\r\n",
        size
    );
    
    stream.write_all(response_header.as_bytes()).await?;
    stream.write_all(&data).await?;
    
    debug!("Sent {} bytes of data", size);
    Ok(())
}

async fn handle_post_request(
    stream: &mut TcpStream,
    request: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut content_length = 0;

    // Parse headers for content-length
    for line in request.lines() {
        if line.to_lowercase().starts_with("content-length:") {
            content_length = line[15..].trim().parse::<usize>().unwrap_or(0);
            break;
        }
    }

    // For simplicity, assume POST body follows immediately in the same buffer
    // In a real implementation, you'd need to handle cases where the body 
    // might come in separate reads
    debug!("Received POST request with content-length: {}", content_length);

    // Send response
    let response = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: keep-alive\r\n\r\n";
    stream.write_all(response.as_bytes()).await?;
    
    Ok(())
}