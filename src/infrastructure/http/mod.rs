//! HTTP client implementations.

pub mod master;
pub mod volume;

use reqwest::Client;
use std::time::Duration;

/// Configuration for the HTTP client.
#[derive(Debug, Clone)]
pub struct HttpClientConfig {
    /// Timeout for establishing a connection.
    pub connect_timeout: Duration,
    /// Timeout for the entire request.
    pub request_timeout: Duration,
    /// Whether to accept invalid TLS certificates (use with caution).
    pub danger_accept_invalid_certs: bool,
    /// Caps a single download, in bytes. `None` means unlimited.
    ///
    /// A download is buffered in memory, so without a cap a hostile or merely
    /// buggy volume server can force an arbitrarily large allocation. The
    /// default stays `None` to preserve existing behaviour; set it to something
    /// larger than your biggest object.
    pub max_download_bytes: Option<u64>,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(30),
            danger_accept_invalid_certs: false,
            max_download_bytes: None,
        }
    }
}

impl HttpClientConfig {
    /// Sets the connection timeout.
    #[must_use]
    pub const fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Sets the request timeout.
    #[must_use]
    pub const fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Caps a single download, in bytes. `None` means unlimited.
    #[must_use]
    pub const fn with_max_download_bytes(mut self, limit: Option<u64>) -> Self {
        self.max_download_bytes = limit;
        self
    }
}

/// Creates a configured HTTP client.
///
/// # Errors
///
/// Returns an error if the HTTP client cannot be built.
pub fn create_http_client(config: &HttpClientConfig) -> Result<Client, reqwest::Error> {
    Client::builder()
        .connect_timeout(config.connect_timeout)
        .timeout(config.request_timeout)
        .danger_accept_invalid_certs(config.danger_accept_invalid_certs)
        .build()
}

pub use master::HttpMasterClient;
pub use volume::HttpVolumeClient;
