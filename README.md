# weedforge

**A lightweight Rust SDK with Python bindings** for [SeaweedFS](https://github.com/seaweedfs/seaweedfs).

[![Crates.io](https://img.shields.io/crates/v/weedforge.svg)](https://crates.io/crates/weedforge)
[![PyPI](https://img.shields.io/pypi/v/weedforge.svg)](https://pypi.org/project/weedforge/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Features

- **Clean Architecture**: Domain, Application, Infrastructure, Python layers
- **HA-aware**: Multiple master support with automatic failover
- **Async + Sync**: Both async and blocking Rust APIs
- **Type-safe**: `FileId` as a first-class entity, not an opaque string
- **Python bindings**: Native Python SDK via PyO3
- **Production-ready**: Retry logic, error handling, tracing

## Installation

### Rust

```toml
[dependencies]
weedforge = "0.1"
```

### Python

```bash
pip install weedforge
```

## Quick Start

### Rust (Async)

```rust
use weedforge::WeedClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client with HA support
    let client = WeedClient::builder()
        .master_urls(["http://master1:9333", "http://master2:9333"])
        .build()?;

    // Upload a file
    let file_id = client.write(b"Hello, SeaweedFS!".to_vec(), Some("hello.txt")).await?;
    println!("Uploaded: {}", file_id);

    // Download the file
    let data = client.read(&file_id).await?;
    println!("Downloaded: {} bytes", data.len());

    // Get public URL
    let url = client.public_url(&file_id).await?;
    println!("Public URL: {}", url);

    // Delete the file
    client.delete(&file_id).await?;

    Ok(())
}
```

### Rust (Blocking)

```rust
use weedforge::BlockingWeedClient;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = BlockingWeedClient::builder()
        .master_url("http://localhost:9333")
        .build()?;

    let file_id = client.write(b"Hello!".to_vec(), Some("hello.txt"))?;
    let data = client.read(&file_id)?;

    Ok(())
}
```

### Python

```python
from weedforge import WeedClient, FileId

# Create client
client = WeedClient(
    master_urls=["http://localhost:9333"],
    strategy="round_robin",  # or "failover", "random"
    max_retries=3
)

# Upload bytes
file_id = client.write(b"Hello, SeaweedFS!", filename="hello.txt")
print(f"Uploaded: {file_id}")

# Download
data = client.read(file_id)
print(f"Downloaded: {len(data)} bytes")

# Get public URL
url = client.public_url(file_id)
print(f"Public URL: {url}")

# With image resize
url = client.public_url_resized(file_id, width=200, height=200)

# Delete
client.delete(file_id)

# Parse file ID from string
fid = FileId.parse("3,01637037d6")
print(f"Volume: {fid.volume_id}, Key: {fid.file_key}")
```

## Architecture

weedforge follows **Clean Architecture** principles:

```
┌─────────────────────────────────────────┐
│             Python Bindings             │  ← Thin wrappers (PyO3)
├─────────────────────────────────────────┤
│            Application Layer            │  ← Use cases (WriteFile, ReadFile)
├─────────────────────────────────────────┤
│              Domain Layer               │  ← Entities (FileId), Ports (traits)
├─────────────────────────────────────────┤
│           Infrastructure Layer          │  ← HTTP clients, HA logic
└─────────────────────────────────────────┘
```

Dependencies flow **downward only**:
- Python → Application → Domain → Infrastructure
- Domain layer has **no external dependencies**
- Application layer is **fully testable with mocks**

## Configuration

### Master Selection Strategies

| Strategy | Description |
|----------|-------------|
| `round_robin` | Cycle through masters (default) |
| `failover` | Try masters in order, failover on error |
| `random` | Random selection |

### Rust Builder Options

```rust
let client = WeedClient::builder()
    .master_urls(["http://master1:9333", "http://master2:9333"])
    .strategy(MasterSelectionStrategy::RoundRobin)
    .max_retries(3)
    .http_config(HttpClientConfig::default()
        .with_connect_timeout(Duration::from_secs(5))
        .with_request_timeout(Duration::from_secs(30)))
    .build()?;
```

## SeaweedFS Protocol

weedforge implements the official SeaweedFS protocol:

### Write Flow
1. `GET /dir/assign` → Get file ID and volume URL
2. `POST {volume_url}/{fid}` → Upload file (multipart)
3. Return `fid` for storage

### Read Flow
1. `GET /dir/lookup?volumeId=X` → Get volume locations
2. Select replica (random or deterministic)
3. `GET {volume_url}/{fid}` → Download file

## Development

### Prerequisites

- Rust 1.75+
- Python 3.9+ (for Python bindings)
- maturin (for building Python wheels)

### Build

```bash
# Rust
cargo build --release

# Python (development)
cd crates/weedforge-python
maturin develop

# Python (release wheel)
maturin build --release
```

### Test

```bash
# Rust tests
cargo test

# Clippy
cargo clippy --all-targets --all-features

# Format check
cargo fmt --check

# Security audit
cargo deny check
```
