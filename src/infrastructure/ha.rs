//! High availability and failover logic.

use crate::domain::{
    AssignOptions, AssignResult, DomainError, DomainResult, LookupResult, MasterPort,
};
use crate::infrastructure::http::HttpMasterClient;
use rand::RngExt;
use reqwest::Client;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Strategy for selecting which master to use.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MasterSelectionStrategy {
    /// Distribute requests across masters in round-robin fashion.
    #[default]
    RoundRobin,
    /// Always try the first master, only fail over on errors.
    Failover,
    /// Select a random master for each request.
    Random,
}

/// Configuration for the HA master client.
#[derive(Debug, Clone)]
pub struct HaMasterConfig {
    /// List of master server URLs.
    pub master_urls: Vec<String>,
    /// Strategy for selecting which master to use.
    pub strategy: MasterSelectionStrategy,
    /// Maximum number of retry attempts.
    pub max_retries: usize,
}

impl HaMasterConfig {
    /// Creates a new HA master configuration.
    ///
    /// # Panics
    ///
    /// Panics if `master_urls` is empty.
    #[must_use]
    pub fn new(master_urls: Vec<String>) -> Self {
        assert!(!master_urls.is_empty(), "At least one master URL required");
        Self {
            master_urls,
            strategy: MasterSelectionStrategy::default(),
            max_retries: 3,
        }
    }

    /// Sets the master selection strategy.
    #[must_use]
    pub const fn with_strategy(mut self, strategy: MasterSelectionStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Sets the maximum number of retries.
    #[must_use]
    pub const fn with_max_retries(mut self, max_retries: usize) -> Self {
        self.max_retries = max_retries;
        self
    }
}

/// High-availability master client.
pub struct HaMasterClient {
    clients: Vec<HttpMasterClient>,
    strategy: MasterSelectionStrategy,
    max_retries: usize,
    current_index: AtomicUsize,
}

impl HaMasterClient {
    /// Creates a new HA master client with the given configuration.
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(http_client: Client, config: HaMasterConfig) -> Self {
        let clients = config
            .master_urls
            .iter()
            .map(|url| HttpMasterClient::new(http_client.clone(), url.clone()))
            .collect();

        Self {
            clients,
            strategy: config.strategy,
            max_retries: config.max_retries,
            current_index: AtomicUsize::new(0),
        }
    }

    /// Returns the number of configured master servers.
    #[must_use]
    pub fn master_count(&self) -> usize {
        self.clients.len()
    }

    fn next_master_index(&self) -> usize {
        match self.strategy {
            MasterSelectionStrategy::RoundRobin => {
                self.current_index.fetch_add(1, Ordering::Relaxed) % self.clients.len()
            }
            MasterSelectionStrategy::Failover => 0,
            // `clients` is guaranteed non-empty, so the range is valid.
            MasterSelectionStrategy::Random => rand::rng().random_range(0..self.clients.len()),
        }
    }
}

impl MasterPort for HaMasterClient {
    async fn assign(&self, options: Option<AssignOptions>) -> DomainResult<AssignResult> {
        let mut last_error = DomainError::AllMastersUnavailable;
        let start_index = self.next_master_index();

        for _ in 0..self.max_retries.max(1) {
            for offset in 0..self.clients.len() {
                let index = (start_index + offset) % self.clients.len();
                let client = &self.clients[index];

                match client.assign(options.clone()).await {
                    Ok(result) => return Ok(result),
                    Err(e) => last_error = e,
                }
            }
        }

        Err(last_error)
    }

    async fn lookup(&self, volume_id: u32) -> DomainResult<LookupResult> {
        let mut last_error = DomainError::AllMastersUnavailable;
        let start_index = self.next_master_index();

        for _ in 0..self.max_retries.max(1) {
            for offset in 0..self.clients.len() {
                let index = (start_index + offset) % self.clients.len();
                let client = &self.clients[index];

                match client.lookup(volume_id).await {
                    Ok(result) => return Ok(result),
                    // The master answered and said the volume does not exist.
                    // Every other master shares one raft-replicated topology, so
                    // asking them cannot produce a different answer -- it just
                    // costs max_retries * masters requests to reach the same
                    // conclusion.
                    Err(e @ DomainError::VolumeNotFound { .. }) => return Err(e),
                    Err(e) => last_error = e,
                }
            }
        }

        Err(last_error)
    }
}

impl MasterPort for &HaMasterClient {
    async fn assign(&self, options: Option<AssignOptions>) -> DomainResult<AssignResult> {
        (*self).assign(options).await
    }

    async fn lookup(&self, volume_id: u32) -> DomainResult<LookupResult> {
        (*self).lookup(volume_id).await
    }
}

/// Builder for creating HA master clients.
#[derive(Debug)]
pub struct HaMasterClientBuilder {
    master_urls: Vec<String>,
    strategy: MasterSelectionStrategy,
    max_retries: usize,
}

// Hand-written rather than derived: the derive gives max_retries = 0, so
// `default()` and `new()` disagreed about a field that governs failover.
impl Default for HaMasterClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl HaMasterClientBuilder {
    /// Creates a new builder with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            master_urls: Vec::new(),
            strategy: MasterSelectionStrategy::default(),
            max_retries: 3,
        }
    }

    /// Adds a single master URL.
    #[must_use]
    pub fn master_url(mut self, url: impl Into<String>) -> Self {
        self.master_urls.push(url.into());
        self
    }

    /// Adds multiple master URLs.
    #[must_use]
    pub fn master_urls<I, S>(mut self, urls: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.master_urls.extend(urls.into_iter().map(Into::into));
        self
    }

    /// Sets the master selection strategy.
    #[must_use]
    pub const fn strategy(mut self, strategy: MasterSelectionStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Sets the maximum number of retries.
    #[must_use]
    pub const fn max_retries(mut self, max_retries: usize) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Builds the HA master client.
    ///
    /// # Errors
    ///
    /// Returns an error if no master URLs have been configured.
    pub fn build(self, http_client: Client) -> DomainResult<HaMasterClient> {
        if self.master_urls.is_empty() {
            return Err(DomainError::ConfigurationError {
                reason: "At least one master URL is required".to_string(),
            });
        }

        let config = HaMasterConfig {
            master_urls: self.master_urls,
            strategy: self.strategy,
            max_retries: self.max_retries,
        };

        Ok(HaMasterClient::new(http_client, config))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::infrastructure::http::create_http_client;
    use crate::infrastructure::HttpClientConfig;
    use std::time::Duration;

    #[tokio::test]
    async fn max_retries_zero_still_attempts_once() {
        // With the bug, max_retries == 0 makes the retry loop run zero times and returns
        // AllMastersUnavailable without contacting any master. After the fix it makes one
        // full pass, so a single unreachable master surfaces the connection failure.
        let config = HttpClientConfig::default().with_connect_timeout(Duration::from_millis(200));
        let http = create_http_client(&config).expect("client");
        let client = HaMasterClientBuilder::new()
            .master_url("http://127.0.0.1:1")
            .max_retries(0)
            .build(http)
            .expect("ha client");

        let err = client
            .assign(None)
            .await
            .expect_err("should fail to connect");
        assert!(
            matches!(err, DomainError::AssignmentFailed { .. }),
            "expected an attempted-connection error, got {err:?}"
        );
    }
}
