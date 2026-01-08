//! Main client facade for weedforge.
//!
//! Provides both async and blocking APIs for `SeaweedFS` operations.

use std::sync::Arc;

use crate::application::{
    DeleteFileUseCase, ImageParams, PublicUrlBuilder, PublicUrlOptions, ReadFileUseCase,
    ReadOptions, WriteFileUseCase, WriteOptions, WriteResult,
};
use crate::domain::{DomainError, DomainResult, FileId, LookupResult};
use crate::infrastructure::{
    create_http_client, HaMasterClient, HaMasterClientBuilder, HttpClientConfig, HttpVolumeClient,
    MasterSelectionStrategy,
};

/// Builder for creating `WeedClient` instances.
#[derive(Debug, Default)]
pub struct WeedClientBuilder {
    master_urls: Vec<String>,
    strategy: MasterSelectionStrategy,
    max_retries: usize,
    http_config: HttpClientConfig,
}

impl WeedClientBuilder {
    /// Creates a new builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            master_urls: Vec::new(),
            strategy: MasterSelectionStrategy::RoundRobin,
            max_retries: 3,
            http_config: HttpClientConfig::default(),
        }
    }

    /// Adds a master URL.
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

    /// Sets the HTTP client configuration.
    #[must_use]
    pub fn http_config(mut self, config: HttpClientConfig) -> Self {
        self.http_config = config;
        self
    }

    /// Builds an async `WeedClient`.
    ///
    /// # Errors
    ///
    /// Returns an error if no master URLs are configured or if the HTTP client
    /// cannot be created.
    pub fn build(self) -> DomainResult<WeedClient> {
        if self.master_urls.is_empty() {
            return Err(DomainError::ConfigurationError {
                reason: "At least one master URL is required".to_string(),
            });
        }

        let http_client =
            create_http_client(&self.http_config).map_err(|e| DomainError::ConfigurationError {
                reason: format!("Failed to create HTTP client: {e}"),
            })?;

        let master = HaMasterClientBuilder::new()
            .master_urls(self.master_urls)
            .strategy(self.strategy)
            .max_retries(self.max_retries)
            .build(http_client.clone())?;

        let volume = HttpVolumeClient::new(http_client);

        Ok(WeedClient {
            master: Arc::new(master),
            volume: Arc::new(volume),
        })
    }

    /// Builds a blocking `BlockingWeedClient`.
    ///
    /// # Errors
    ///
    /// Returns an error if no master URLs are configured or if the HTTP client
    /// cannot be created.
    pub fn build_blocking(self) -> DomainResult<BlockingWeedClient> {
        let client = self.build()?;
        let runtime =
            tokio::runtime::Runtime::new().map_err(|e| DomainError::ConfigurationError {
                reason: format!("Failed to create runtime: {e}"),
            })?;

        Ok(BlockingWeedClient { client, runtime })
    }
}

/// Async client for `SeaweedFS` operations.
#[derive(Clone)]
pub struct WeedClient {
    master: Arc<HaMasterClient>,
    volume: Arc<HttpVolumeClient>,
}

impl WeedClient {
    /// Creates a new builder for configuring the client.
    #[must_use]
    pub fn builder() -> WeedClientBuilder {
        WeedClientBuilder::new()
    }

    /// Writes data to `SeaweedFS` and returns the file ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the upload fails.
    pub async fn write(&self, data: Vec<u8>, filename: Option<&str>) -> DomainResult<FileId> {
        let options = filename.map(WriteOptions::with_filename);
        let use_case = WriteFileUseCase::new(self.master.as_ref(), self.volume.as_ref());
        let result = use_case.execute(data, options).await?;
        Ok(result.file_id)
    }

    /// Writes data with full options and returns detailed result.
    ///
    /// # Errors
    ///
    /// Returns an error if the upload fails.
    pub async fn write_with_options(
        &self,
        data: Vec<u8>,
        options: WriteOptions,
    ) -> DomainResult<WriteResult> {
        let use_case = WriteFileUseCase::new(self.master.as_ref(), self.volume.as_ref());
        use_case.execute(data, Some(options)).await
    }

    /// Reads data from `SeaweedFS`.
    ///
    /// # Errors
    ///
    /// Returns an error if the download fails.
    pub async fn read(&self, file_id: &FileId) -> DomainResult<Vec<u8>> {
        let use_case = ReadFileUseCase::new(self.master.as_ref(), self.volume.as_ref());
        let result = use_case.execute(file_id, None).await?;
        Ok(result.data)
    }

    /// Reads data with options.
    ///
    /// # Errors
    ///
    /// Returns an error if the download fails.
    pub async fn read_with_options(
        &self,
        file_id: &FileId,
        options: ReadOptions,
    ) -> DomainResult<Vec<u8>> {
        let use_case = ReadFileUseCase::new(self.master.as_ref(), self.volume.as_ref());
        let result = use_case.execute(file_id, Some(options)).await?;
        Ok(result.data)
    }

    /// Deletes a file from `SeaweedFS`.
    ///
    /// # Errors
    ///
    /// Returns an error if the deletion fails.
    pub async fn delete(&self, file_id: &FileId) -> DomainResult<()> {
        let use_case = DeleteFileUseCase::new(self.master.as_ref(), self.volume.as_ref());
        use_case.execute(file_id).await
    }

    /// Gets a public URL for a file.
    ///
    /// # Errors
    ///
    /// Returns an error if the lookup fails.
    pub async fn public_url(&self, file_id: &FileId) -> DomainResult<String> {
        let builder = PublicUrlBuilder::new(self.master.as_ref());
        builder.build(file_id, None).await
    }

    /// Gets a public URL with image resize parameters.
    ///
    /// # Errors
    ///
    /// Returns an error if the lookup fails.
    pub async fn public_url_resized(
        &self,
        file_id: &FileId,
        width: u32,
        height: u32,
    ) -> DomainResult<String> {
        let builder = PublicUrlBuilder::new(self.master.as_ref());
        let options = PublicUrlOptions {
            image_params: Some(ImageParams::dimensions(width, height)),
            prefer_public: true,
        };
        builder.build(file_id, Some(options)).await
    }

    /// Looks up volume locations for a file ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the lookup fails.
    pub async fn lookup(&self, file_id: &FileId) -> DomainResult<LookupResult> {
        let use_case = ReadFileUseCase::new(self.master.as_ref(), self.volume.as_ref());
        use_case.lookup(file_id).await
    }

    /// Parses a file ID string.
    ///
    /// # Errors
    ///
    /// Returns an error if the string format is invalid.
    pub fn parse_file_id(fid: &str) -> DomainResult<FileId> {
        FileId::parse(fid)
    }
}

/// Blocking client for `SeaweedFS` operations.
pub struct BlockingWeedClient {
    client: WeedClient,
    runtime: tokio::runtime::Runtime,
}

impl BlockingWeedClient {
    /// Creates a new builder for configuring the client.
    #[must_use]
    pub fn builder() -> WeedClientBuilder {
        WeedClientBuilder::new()
    }

    /// Writes data to `SeaweedFS` and returns the file ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the upload fails.
    pub fn write(&self, data: Vec<u8>, filename: Option<&str>) -> DomainResult<FileId> {
        self.runtime.block_on(self.client.write(data, filename))
    }

    /// Writes data with full options.
    ///
    /// # Errors
    ///
    /// Returns an error if the upload fails.
    pub fn write_with_options(
        &self,
        data: Vec<u8>,
        options: WriteOptions,
    ) -> DomainResult<WriteResult> {
        self.runtime
            .block_on(self.client.write_with_options(data, options))
    }

    /// Reads data from `SeaweedFS`.
    ///
    /// # Errors
    ///
    /// Returns an error if the download fails.
    pub fn read(&self, file_id: &FileId) -> DomainResult<Vec<u8>> {
        self.runtime.block_on(self.client.read(file_id))
    }

    /// Reads data with options.
    ///
    /// # Errors
    ///
    /// Returns an error if the download fails.
    pub fn read_with_options(
        &self,
        file_id: &FileId,
        options: ReadOptions,
    ) -> DomainResult<Vec<u8>> {
        self.runtime
            .block_on(self.client.read_with_options(file_id, options))
    }

    /// Deletes a file from `SeaweedFS`.
    ///
    /// # Errors
    ///
    /// Returns an error if the deletion fails.
    pub fn delete(&self, file_id: &FileId) -> DomainResult<()> {
        self.runtime.block_on(self.client.delete(file_id))
    }

    /// Gets a public URL for a file.
    ///
    /// # Errors
    ///
    /// Returns an error if the lookup fails.
    pub fn public_url(&self, file_id: &FileId) -> DomainResult<String> {
        self.runtime.block_on(self.client.public_url(file_id))
    }

    /// Gets a public URL with image resize parameters.
    ///
    /// # Errors
    ///
    /// Returns an error if the lookup fails.
    pub fn public_url_resized(
        &self,
        file_id: &FileId,
        width: u32,
        height: u32,
    ) -> DomainResult<String> {
        self.runtime
            .block_on(self.client.public_url_resized(file_id, width, height))
    }

    /// Looks up volume locations for a file ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the lookup fails.
    pub fn lookup(&self, file_id: &FileId) -> DomainResult<LookupResult> {
        self.runtime.block_on(self.client.lookup(file_id))
    }

    /// Parses a file ID string.
    ///
    /// # Errors
    ///
    /// Returns an error if the string format is invalid.
    pub fn parse_file_id(fid: &str) -> DomainResult<FileId> {
        FileId::parse(fid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_no_urls() {
        let result = WeedClientBuilder::new().build();
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_with_url() {
        let result = WeedClientBuilder::new()
            .master_url("http://localhost:9333")
            .build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_builder_multiple_urls() {
        let result = WeedClientBuilder::new()
            .master_urls(["http://master1:9333", "http://master2:9333"])
            .strategy(MasterSelectionStrategy::Failover)
            .max_retries(5)
            .build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_file_id() {
        let result = WeedClient::parse_file_id("3,0000016300007037");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_file_id_invalid() {
        let result = WeedClient::parse_file_id("invalid");
        assert!(result.is_err());
    }
}
