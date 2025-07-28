pub mod config;
pub mod socks5;
pub mod tests;
pub mod metrics;
pub mod report;

pub use config::Config;
pub use socks5::Socks5Client;
pub use metrics::Metrics;
pub use report::Report;

#[derive(Debug, thiserror::Error)]
pub enum NetworkTestError {
    #[error("Connection error: {0}")]
    Connection(String),
    #[error("SOCKS5 error: {0}")]
    Socks5(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Timeout error: {0}")]
    Timeout(String),
    #[error("Configuration error: {0}")]
    Config(String),
}

pub type Result<T> = std::result::Result<T, NetworkTestError>;