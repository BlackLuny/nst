use clap::{Parser, ValueEnum};
use std::net::SocketAddr;
use tokio::signal;
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;

mod bandwidth_server;
mod connection_perf_server;
mod dns_stability_server;
mod network_jitter_server;
mod tcp_stability_server;

#[derive(Parser)]
#[command(name = "nst-server")]
#[command(about = "Network Stability Test Server - companion server for NST testing")]
#[command(version)]
struct Args {
    /// Server mode - which type of test server to run
    #[arg(short, long, value_enum, default_value_t = ServerMode::All)]
    mode: ServerMode,

    /// Bind address for the server
    #[arg(long, default_value = "0.0.0.0")]
    host: String,

    /// Base port (individual services use base_port + offset)
    #[arg(short, long, default_value_t = 8000)]
    port: u16,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Clone, Debug, ValueEnum)]
enum ServerMode {
    /// Run all test servers
    All,
    /// TCP stability test server only
    TcpStability,
    /// Bandwidth test server only
    Bandwidth,
    /// Connection performance test server only
    ConnectionPerf,
    /// DNS stability test server only
    DnsStability,
    /// Network jitter test server only
    NetworkJitter,
}

#[derive(Debug, Clone)]
pub enum ServerType {
    TcpStability,
    Bandwidth,
    ConnectionPerf,
    DnsStability,
    NetworkJitter,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Initialize tracing
    let level = if args.verbose {
        Level::DEBUG
    } else {
        Level::INFO
    };
    let subscriber = FmtSubscriber::builder().with_max_level(level).finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting NST Server");
    info!("Mode: {:?}", args.mode);

    let _base_addr = format!("{}:{}", args.host, args.port);

    match args.mode {
        ServerMode::All => {
            start_all_servers(&args.host, args.port).await?;
        }
        ServerMode::TcpStability => {
            let addr: SocketAddr = format!("{}:{}", args.host, args.port + 1).parse()?;
            start_server(addr, ServerType::TcpStability).await?;
        }
        ServerMode::Bandwidth => {
            let addr: SocketAddr = format!("{}:{}", args.host, args.port + 2).parse()?;
            start_server(addr, ServerType::Bandwidth).await?;
        }
        ServerMode::ConnectionPerf => {
            let addr: SocketAddr = format!("{}:{}", args.host, args.port + 3).parse()?;
            start_server(addr, ServerType::ConnectionPerf).await?;
        }
        ServerMode::DnsStability => {
            let addr: SocketAddr = format!("{}:{}", args.host, args.port + 4).parse()?;
            start_server(addr, ServerType::DnsStability).await?;
        }
        ServerMode::NetworkJitter => {
            let addr: SocketAddr = format!("{}:{}", args.host, args.port + 5).parse()?;
            start_server(addr, ServerType::NetworkJitter).await?;
        }
    }

    Ok(())
}

async fn start_all_servers(host: &str, base_port: u16) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting all NST test servers");

    let servers = vec![
        (base_port + 1, ServerType::TcpStability, "TCP Stability"),
        (base_port + 2, ServerType::Bandwidth, "Bandwidth"),
        (
            base_port + 3,
            ServerType::ConnectionPerf,
            "Connection Performance",
        ),
        (base_port + 4, ServerType::DnsStability, "DNS Stability"),
        (base_port + 5, ServerType::NetworkJitter, "Network Jitter"),
    ];

    let mut tasks = vec![];

    for (port, server_type, name) in servers {
        let addr: SocketAddr = format!("{host}:{port}").parse()?;
        info!("Starting {} server on {}", name, addr);

        let task = tokio::spawn(async move {
            if let Err(e) = start_server(addr, server_type).await {
                error!("Server {} failed: {}", name, e);
            }
        });

        tasks.push(task);
    }

    info!("All servers started. Press Ctrl+C to stop.");

    // Wait for shutdown signal
    match signal::ctrl_c().await {
        Ok(()) => {
            info!("Shutdown signal received");
        }
        Err(err) => {
            error!("Unable to listen for shutdown signal: {}", err);
        }
    }

    // Cancel all tasks
    for task in tasks {
        task.abort();
    }

    info!("All servers stopped");
    Ok(())
}

async fn start_server(
    addr: SocketAddr,
    server_type: ServerType,
) -> Result<(), Box<dyn std::error::Error>> {
    match server_type {
        ServerType::DnsStability => {
            dns_stability_server::run_dns_server(addr.port())
                .await
                .map_err(|e| format!("DNS server error: {e}"))?;
            Ok(())
        }
        _ => {
            use tokio::net::TcpListener;

            let listener = TcpListener::bind(addr).await?;
            info!("Server listening on {} for {:?}", addr, server_type);

            loop {
                match listener.accept().await {
                    Ok((stream, peer_addr)) => {
                        info!(
                            "New connection from {} to {:?} server",
                            peer_addr, server_type
                        );

                        let server_type = server_type.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(stream, server_type).await {
                                error!("Error handling connection from {}: {}", peer_addr, e);
                            }
                        });
                    }
                    Err(e) => {
                        error!("Failed to accept connection: {}", e);
                    }
                }
            }
        }
    }
}

async fn handle_connection(
    stream: tokio::net::TcpStream,
    server_type: ServerType,
) -> Result<(), Box<dyn std::error::Error>> {
    match server_type {
        ServerType::TcpStability => tcp_stability_server::handle_client(stream).await,
        ServerType::Bandwidth => bandwidth_server::handle_client(stream).await,
        ServerType::ConnectionPerf => connection_perf_server::handle_client(stream).await,
        ServerType::NetworkJitter => network_jitter_server::handle_client(stream).await,
        ServerType::DnsStability => {
            // DNS server is handled separately as UDP, this should never be reached
            Ok(())
        }
    }
}
