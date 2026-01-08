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
//! ```ignore
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
//! ```ignore
//! use weedforge::BlockingWeedClient;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = BlockingWeedClient::builder()
//!         .master_url("http://localhost:9333")
//!         .build()?;
//!
//!     let file_id = client.write(b"Hello!".to_vec(), Some("hello.txt"))?;
//!     let data = client.read(&file_id)?;
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
