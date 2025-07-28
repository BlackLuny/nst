# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Network Stability Test (NST) is a Rust CLI tool for testing SOCKS5 proxy network stability. The binary is named `nst` and provides comprehensive testing capabilities including TCP stability, bandwidth testing, connection performance, DNS stability, and network jitter analysis.

## Common Development Commands

### Building and Running
```bash
# Build in debug mode
cargo build

# Build release version
cargo build --release

# Run client with help
cargo run -- --help

# Run specific tests
cargo run -- tcp-stability -p 127.0.0.1:1080 -t 127.0.0.1:8001
cargo run -- bandwidth -p 127.0.0.1:1080 -t 127.0.0.1:8002
cargo run -- connection-perf -p 127.0.0.1:1080 -t 127.0.0.1:8003
cargo run -- all -p 127.0.0.1:1080

# Run tests with verbose logging
cargo run -- tcp-stability -v -p 127.0.0.1:1080

# Run server with help
cargo run --bin nst-server -- --help

# Start all test servers (default ports 8001-8005)
cargo run --bin nst-server

# Start specific server type
cargo run --bin nst-server -- --mode tcp-stability --port 8001
cargo run --bin nst-server -- --mode bandwidth --port 8002

# Start all servers with verbose logging
cargo run --bin nst-server -- --mode all --verbose
```

### Testing and Development
```bash
# Run unit tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Check code formatting
cargo fmt --check

# Apply formatting
cargo fmt

# Run clippy lints
cargo clippy

# Check without building
cargo check
```

## Architecture

### Core Modules Structure
- `src/main.rs` - CLI entry point with command parsing using clap
- `src/lib.rs` - Main library exports and error types
- `src/config.rs` - Configuration structures and JSON config file handling
- `src/socks5.rs` - SOCKS5 client implementation for proxy connections
- `src/metrics.rs` - Metrics collection and calculation
- `src/report.rs` - Test result reporting and output formatting
- `src/tests/` - Test modules directory containing:
  - `bandwidth.rs` - Bandwidth testing implementation
  - `connection_perf.rs` - Connection performance testing
  - `dns_stability.rs` - DNS resolution stability testing
  - `network_jitter.rs` - Network jitter and RTT testing
  - `tcp_stability.rs` - TCP connection stability testing

### Key Design Patterns
- **Async Architecture**: Built on Tokio runtime for high-concurrency network operations
- **Command Pattern**: Each test type is implemented as a separate module with consistent `run()` interface
- **Configuration-Driven**: JSON configuration files support detailed test parameter customization
- **Error Handling**: Custom error types with thiserror for structured error management
- **Modular Testing**: Each test type can run independently or as part of comprehensive suite

### Dependencies
- `tokio` - Async runtime with full features
- `clap` - Command line argument parsing with derive macros
- `serde` + `serde_json` - Configuration serialization/deserialization
- `anyhow` + `thiserror` - Error handling
- `tracing` + `tracing-subscriber` - Structured logging
- `chrono` - Time handling for metrics
- `uuid` - Unique identifiers

### SOCKS5 Implementation
The project includes a custom SOCKS5 client implementation supporting:
- No authentication and username/password authentication
- IPv4/IPv6 and domain name resolution
- Connection timeouts and error handling
- Connection state management

### Test Types and Scoring
The tool provides a 0-100 scoring system with weighted contributions:
- TCP Stability: 25% - Long connection maintenance and heartbeat detection
- Bandwidth: 20% - Continuous small-flow data transfer testing
- Connection Performance: 20% - Concurrent connection establishment testing
- DNS Stability: 15% - DNS resolution through proxy testing
- Network Jitter: 20% - RTT variation and packet loss detection

### Binary Names
The project builds two binaries:
- `nst` - The main test client (defined in Cargo.toml [[bin]] section)
- `nst-server` - The companion test server for all test types

## NST Server (nst-server)

The NST server provides test endpoints for all 5 test types that the client can test against. This is essential for proper testing since the tests need specific server behavior.

### Server Ports and Services
When running `nst-server --mode all`, the following services are started:

- **Port 8001**: TCP Stability Test Server
  - Handles heartbeat packets: `PING-{number}` → `PONG-{number}`
  - Supports long-lived connections for stability testing

- **Port 8002**: Bandwidth Test Server  
  - HTTP-style protocol
  - GET `/stream-bytes/{size}` - Returns specified bytes of test data
  - POST `/post` - Accepts upload data and returns HTTP 200

- **Port 8003**: Connection Performance Test Server
  - Fast connection acceptance and immediate response
  - Optimized for high-concurrency connection testing

- **Port 8004**: DNS Stability Test Server
  - Simulates DNS server functionality  
  - Accepts DNS query packets and returns A record responses
  - Returns 8.8.8.8 as default IP for all queries

- **Port 8005**: Network Jitter Test Server
  - TCP ping protocol: `PING` → `PONG`
  - Low-latency response for RTT and jitter measurements

### Server Usage Examples
```bash
# Start all servers (recommended for full testing)
nst-server --mode all

# Start individual server types for focused testing
nst-server --mode tcp-stability --port 8001
nst-server --mode bandwidth --port 8002  
nst-server --mode connection-perf --port 8003
nst-server --mode dns-stability --port 8004
nst-server --mode network-jitter --port 8005

# Custom host binding (default is 0.0.0.0)
nst-server --host 127.0.0.1 --port 8000

# Enable verbose logging for debugging
nst-server --mode all --verbose
```

### Testing with Server
1. Start the test server: `cargo run --bin nst-server`
2. Run client tests against localhost: `cargo run -- tcp-stability -p 127.0.0.1:1080 -t 127.0.0.1:8001`
3. The client will connect through the SOCKS5 proxy to test the server endpoints