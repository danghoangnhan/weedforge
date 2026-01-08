//! Infrastructure layer for weedforge.
//!
//! This module implements the domain ports using HTTP communication.

pub mod ha;
pub mod http;

pub use ha::{HaMasterClient, HaMasterClientBuilder, HaMasterConfig, MasterSelectionStrategy};
pub use http::{create_http_client, HttpClientConfig, HttpMasterClient, HttpVolumeClient};
