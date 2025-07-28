use clap::{Parser, Subcommand};
use network_stable_test::{Config, Result};
use tracing::info;

#[derive(Parser)]
#[command(name = "nst")]
#[command(about = "Network Stability Test - A tool for testing SOCKS5 proxy stability")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    
    #[arg(short, long, value_name = "FILE")]
    config: Option<String>,
    
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    TcpStability {
        #[arg(short, long, default_value = "127.0.0.1:1080")]
        proxy: String,
        
        #[arg(short, long, default_value = "8.8.8.8:53")]
        target: String,
        
        #[arg(short, long, default_value = "30")]
        interval: u64,
        
        #[arg(short, long, default_value = "300")]
        duration: u64,
    },
    
    Bandwidth {
        #[arg(short, long, default_value = "127.0.0.1:1080")]
        proxy: String,
        
        #[arg(short, long, default_value = "httpbin.org:80")]
        target: String,
        
        #[arg(short, long, default_value = "1024")]
        size: usize,
        
        #[arg(short, long, default_value = "60")]
        duration: u64,
    },
    
    ConnectionPerf {
        #[arg(short, long, default_value = "127.0.0.1:1080")]
        proxy: String,
        
        #[arg(short, long, default_value = "8.8.8.8:53")]
        target: String,
        
        #[arg(short, long, default_value = "10")]
        concurrent: usize,
        
        #[arg(short, long, default_value = "100")]
        total: usize,
    },
    
    All {
        #[arg(short, long, default_value = "127.0.0.1:1080")]
        proxy: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    tracing_subscriber::fmt()
        .with_env_filter(if cli.verbose { "debug" } else { "info" })
        .init();
    
    let _config = if let Some(config_path) = cli.config {
        Config::from_file(&config_path)?
    } else {
        Config::default()
    };
    
    info!("Starting network stability test");
    
    match cli.command {
        Commands::TcpStability { proxy, target, interval, duration } => {
            info!("Running TCP stability test");
            run_tcp_stability_test(&proxy, &target, interval, duration).await?;
        }
        Commands::Bandwidth { proxy, target, size, duration } => {
            info!("Running bandwidth test");
            run_bandwidth_test(&proxy, &target, size, duration).await?;
        }
        Commands::ConnectionPerf { proxy, target, concurrent, total } => {
            info!("Running connection performance test");
            run_connection_perf_test(&proxy, &target, concurrent, total).await?;
        }
        Commands::All { proxy } => {
            info!("Running all tests");
            run_all_tests(&proxy).await?;
        }
    }
    
    info!("Test completed successfully");
    Ok(())
}

async fn run_tcp_stability_test(proxy: &str, target: &str, interval: u64, duration: u64) -> Result<()> {
    use network_stable_test::tests::tcp_stability::TcpStabilityTest;
    
    let test = TcpStabilityTest::new(proxy, target, interval, duration);
    test.run().await
}

async fn run_bandwidth_test(proxy: &str, target: &str, size: usize, duration: u64) -> Result<()> {
    use network_stable_test::tests::bandwidth::BandwidthTest;
    
    let test = BandwidthTest::new(proxy, target, size, duration);
    test.run().await
}

async fn run_connection_perf_test(proxy: &str, target: &str, concurrent: usize, total: usize) -> Result<()> {
    use network_stable_test::tests::connection_perf::ConnectionPerfTest;
    
    let test = ConnectionPerfTest::new(proxy, target, concurrent, total);
    test.run().await
}

async fn run_all_tests(proxy: &str) -> Result<()> {
    info!("Running comprehensive network stability tests");
    
    run_tcp_stability_test(proxy, "8.8.8.8:53", 30, 300).await?;
    run_bandwidth_test(proxy, "httpbin.org:80", 1024, 60).await?;
    run_connection_perf_test(proxy, "8.8.8.8:53", 10, 100).await?;
    
    Ok(())
}