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

    #[arg(short = 'j', long, default_value = "1")]
    parallel: usize,
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

        #[arg(short = 'n', long, default_value = "100")]
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
        Commands::TcpStability {
            proxy,
            target,
            interval,
            duration,
        } => {
            info!(
                "Running TCP stability test with {} parallel instances",
                cli.parallel
            );
            run_tcp_stability_test_parallel(&proxy, &target, interval, duration, cli.parallel)
                .await?;
        }
        Commands::Bandwidth {
            proxy,
            target,
            size,
            duration,
        } => {
            info!(
                "Running bandwidth test with {} parallel instances",
                cli.parallel
            );
            run_bandwidth_test_parallel(&proxy, &target, size, duration, cli.parallel).await?;
        }
        Commands::ConnectionPerf {
            proxy,
            target,
            concurrent,
            total,
        } => {
            info!(
                "Running connection performance test with {} parallel instances",
                cli.parallel
            );
            run_connection_perf_test_parallel(&proxy, &target, concurrent, total, cli.parallel)
                .await?;
        }
        Commands::All { proxy } => {
            info!("Running all tests with {} parallel instances", cli.parallel);
            run_all_tests_parallel(&proxy, cli.parallel).await?;
        }
    }

    info!("Test completed successfully");
    Ok(())
}

async fn run_tcp_stability_test_parallel(
    proxy: &str,
    target: &str,
    interval: u64,
    duration: u64,
    parallel: usize,
) -> Result<()> {
    use network_stable_test::tests::tcp_stability::TcpStabilityTest;
    use tokio::task::JoinSet;

    if parallel == 1 {
        let test = TcpStabilityTest::new(proxy, target, interval, duration);
        return test.run().await;
    }

    let mut join_set = JoinSet::new();

    for i in 0..parallel {
        let proxy = proxy.to_string();
        let target = target.to_string();

        join_set.spawn(async move {
            info!("Starting TCP stability test instance {}", i + 1);
            let test = TcpStabilityTest::new(&proxy, &target, interval, duration);
            test.run().await
        });
    }

    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(test_result) => test_result?,
            Err(join_error) => {
                return Err(network_stable_test::NetworkTestError::Connection(format!(
                    "Task join error: {join_error}"
                )));
            }
        }
    }

    Ok(())
}

async fn run_bandwidth_test_parallel(
    proxy: &str,
    target: &str,
    size: usize,
    duration: u64,
    parallel: usize,
) -> Result<()> {
    use network_stable_test::tests::bandwidth::BandwidthTest;
    use tokio::task::JoinSet;

    if parallel == 1 {
        let test = BandwidthTest::new(proxy, target, size, duration);
        return test.run().await;
    }

    let mut join_set = JoinSet::new();

    for i in 0..parallel {
        let proxy = proxy.to_string();
        let target = target.to_string();

        join_set.spawn(async move {
            info!("Starting bandwidth test instance {}", i + 1);
            let test = BandwidthTest::new(&proxy, &target, size, duration);
            test.run().await
        });
    }

    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(test_result) => test_result?,
            Err(join_error) => {
                return Err(network_stable_test::NetworkTestError::Connection(format!(
                    "Task join error: {join_error}"
                )));
            }
        }
    }

    Ok(())
}

async fn run_connection_perf_test_parallel(
    proxy: &str,
    target: &str,
    concurrent: usize,
    total: usize,
    parallel: usize,
) -> Result<()> {
    use network_stable_test::tests::connection_perf::ConnectionPerfTest;
    use tokio::task::JoinSet;

    if parallel == 1 {
        let test = ConnectionPerfTest::new(proxy, target, concurrent, total);
        return test.run().await;
    }

    let mut join_set = JoinSet::new();

    for i in 0..parallel {
        let proxy = proxy.to_string();
        let target = target.to_string();

        join_set.spawn(async move {
            info!("Starting connection performance test instance {}", i + 1);
            let test = ConnectionPerfTest::new(&proxy, &target, concurrent, total);
            test.run().await
        });
    }

    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(test_result) => test_result?,
            Err(join_error) => {
                return Err(network_stable_test::NetworkTestError::Connection(format!(
                    "Task join error: {join_error}"
                )));
            }
        }
    }

    Ok(())
}

async fn run_all_tests_parallel(proxy: &str, parallel: usize) -> Result<()> {
    info!("Running comprehensive network stability tests");

    run_tcp_stability_test_parallel(proxy, "8.8.8.8:53", 30, 300, parallel).await?;
    run_bandwidth_test_parallel(proxy, "httpbin.org:80", 1024, 60, parallel).await?;
    run_connection_perf_test_parallel(proxy, "8.8.8.8:53", 10, 100, parallel).await?;

    Ok(())
}
