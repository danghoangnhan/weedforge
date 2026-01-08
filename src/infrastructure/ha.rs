//! High availability and failover logic.

use crate::domain::{
    AssignOptions, AssignResult, DomainError, DomainResult, LookupResult, MasterPort,
};
use crate::infrastructure::http::HttpMasterClient;
use rand::seq::SliceRandom;
use reqwest::Client;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

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
    failed_masters: Arc<RwLock<Vec<usize>>>,
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
            failed_masters: Arc::new(RwLock::new(Vec::new())),
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
            MasterSelectionStrategy::Random => {
                let mut rng = rand::thread_rng();
                let indices: Vec<usize> = (0..self.clients.len()).collect();
                *indices.choose(&mut rng).unwrap_or(&0)
            }
        }
    }

    async fn mark_success(&self, index: usize) {
        let mut failed = self.failed_masters.write().await;
        failed.retain(|&i| i != index);
    }

    async fn mark_failed(&self, index: usize) {
        let mut failed = self.failed_masters.write().await;
        if !failed.contains(&index) {
            failed.push(index);
        }
    }
}

impl MasterPort for HaMasterClient {
    async fn assign(&self, options: Option<AssignOptions>) -> DomainResult<AssignResult> {
        let mut last_error = DomainError::AllMastersUnavailable;
        let start_index = self.next_master_index();

        for _ in 0..self.max_retries {
            for offset in 0..self.clients.len() {
                let index = (start_index + offset) % self.clients.len();
                let client = &self.clients[index];

                match client.assign(options.clone()).await {
                    Ok(result) => {
                        self.mark_success(index).await;
                        return Ok(result);
                    }
                    Err(e) => {
                        last_error = e;
                        self.mark_failed(index).await;
                    }
                }
            }
        }

        Err(last_error)
    }

    async fn lookup(&self, volume_id: u32) -> DomainResult<LookupResult> {
        let mut last_error = DomainError::AllMastersUnavailable;
        let start_index = self.next_master_index();

        for _ in 0..self.max_retries {
            for offset in 0..self.clients.len() {
                let index = (start_index + offset) % self.clients.len();
                let client = &self.clients[index];

                match client.lookup(volume_id).await {
                    Ok(result) => {
                        self.mark_success(index).await;
                        return Ok(result);
                    }
                    Err(e) => {
                        last_error = e;
                        self.mark_failed(index).await;
                    }
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
#[derive(Debug, Default)]
pub struct HaMasterClientBuilder {
    master_urls: Vec<String>,
    strategy: MasterSelectionStrategy,
    max_retries: usize,
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
