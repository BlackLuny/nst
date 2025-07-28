use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tracing::debug;

pub async fn handle_client(mut stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    // For connection performance testing, we just need to accept the connection
    // and immediately close it or send a minimal response
    
    let response = b"OK\n";
    stream.write_all(response).await?;
    
    debug!("Connection performance test - responded and closing");
    
    // Close connection immediately to test connection establishment speed
    Ok(())
}