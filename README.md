# weedforge

**A Rust-first SDK for [SeaweedFS](https://github.com/seaweedfs/seaweedfs)**, with native clients for Python, Go, and Java.

[![Crates.io](https://img.shields.io/crates/v/weedforge.svg)](https://crates.io/crates/weedforge)
[![PyPI](https://img.shields.io/pypi/v/weedforge.svg)](https://pypi.org/project/weedforge/)
[![CI](https://github.com/danghoangnhan/weedforge/actions/workflows/ci.yml/badge.svg)](https://github.com/danghoangnhan/weedforge/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

- **Clean architecture** — domain / application / infrastructure layers, fully testable with mocks
- **HA-aware** — multiple masters with automatic failover and bounded retries
- **Async + blocking** Rust APIs; native clients for **Python, Go, and Java**
- **Type-safe `FileId`** — a first-class, byte-accurate SeaweedFS entity, not an opaque string

## Install

| Language | |
|----------|--|
| **Rust** | `weedforge = "0.1"` |
| **Python** | `pip install weedforge` |
| **Go** | pure Go (no cgo) — see [`clients/go`](clients/go) |
| **Java** | JDK 8+, zero runtime deps — see [`clients/java`](clients/java) |

## Quick start (Rust)

```rust
use weedforge::WeedClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = WeedClient::builder()
        .master_urls(["http://master1:9333", "http://master2:9333"])
        .build()?;

    let fid = client.write(b"Hello, SeaweedFS!".to_vec(), Some("hello.txt")).await?;
    let data = client.read(&fid).await?;
    let url = client.public_url(&fid).await?;
    client.delete(&fid).await?;
    Ok(())
}
```

## Documentation

Full documentation lives in the **[project wiki](https://github.com/danghoangnhan/weedforge/wiki)**:

- **[Installation](https://github.com/danghoangnhan/weedforge/wiki/Installation)**
- **Usage:** [Rust](https://github.com/danghoangnhan/weedforge/wiki/Usage-Rust) · [Python](https://github.com/danghoangnhan/weedforge/wiki/Usage-Python) · [Go](https://github.com/danghoangnhan/weedforge/wiki/Usage-Go) · [Java](https://github.com/danghoangnhan/weedforge/wiki/Usage-Java)
- **[Architecture](https://github.com/danghoangnhan/weedforge/wiki/Architecture)**
- **[Configuration](https://github.com/danghoangnhan/weedforge/wiki/Configuration)**
- **[SeaweedFS Protocol](https://github.com/danghoangnhan/weedforge/wiki/SeaweedFS-Protocol)**
- **[Development](https://github.com/danghoangnhan/weedforge/wiki/Development)**

## License

Licensed under the [MIT License](LICENSE).
