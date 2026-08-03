//! # weedforge
//!
//! A Rust-first, Python-friendly SDK for **`SeaweedFS`**.
//!
//! ## Features
//!
//! - **Clean Architecture**: Domain, Application, Infrastructure layers
//! - **HA-aware**: Multiple master support with failover
//! - **Async + Sync**: Both async and blocking APIs
//! - **Type-safe**: Strong typing with `FileId` as first-class entity
//!
//! ## Quick Start
//!
//! `no_run` rather than `ignore`: these need a live `SeaweedFS` to execute, but
//! they are still compiled and linked, so an API change that invalidates them
//! breaks the build instead of quietly leaving the front page wrong.
//!
//! ```no_run
//! use weedforge::WeedClient;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create client
//!     let client = WeedClient::builder()
//!         .master_url("http://localhost:9333")
//!         .build()?;
//!
//!     // Upload a file
//!     let file_id = client.write(b"Hello, SeaweedFS!".to_vec(), Some("hello.txt")).await?;
//!     println!("Uploaded: {}", file_id);
//!
//!     // Download the file
//!     let data = client.read(&file_id).await?;
//!     println!("Downloaded: {} bytes", data.len());
//!
//!     // Get public URL
//!     let url = client.public_url(&file_id).await?;
//!     println!("Public URL: {}", url);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Blocking API
//!
//! ```no_run
//! use weedforge::BlockingWeedClient;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // build_blocking(), not build(): build() hands back an async WeedClient,
//!     // so every call below would return a Future and none of the `?` compile.
//!     let client = BlockingWeedClient::builder()
//!         .master_url("http://localhost:9333")
//!         .build_blocking()?;
//!
//!     let file_id = client.write(b"Hello!".to_vec(), Some("hello.txt"))?;
//!     println!("Uploaded: {file_id}");
//!
//!     let data = client.read(&file_id)?;
//!     println!("Downloaded: {} bytes", data.len());
//!
//!     Ok(())
//! }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]

pub mod application;
pub mod client;
pub mod domain;
pub mod infrastructure;

#[cfg(feature = "python")]
pub mod python;

// Re-export domain types
pub use domain::{
    AssignOptions, AssignResult, DomainError, DomainResult, FileId, LookupResult, UploadResult,
    VolumeLocation,
};

// Re-export application types
pub use application::{
    ImageParams, PublicUrlOptions, ReadOptions, ReadResult, ReplicaSelection, ResizeMode,
    WriteOptions, WriteResult,
};

// Re-export infrastructure types for advanced usage
pub use infrastructure::{
    HaMasterClient, HaMasterClientBuilder, HttpClientConfig, HttpMasterClient, HttpVolumeClient,
    MasterSelectionStrategy,
};

// Export main client types
pub use client::{BlockingWeedClient, WeedClient, WeedClientBuilder};
