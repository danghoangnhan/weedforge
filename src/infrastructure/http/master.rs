//! HTTP implementation of the master port.

use crate::domain::{
    AssignOptions, AssignResult, DomainError, DomainResult, FileId, LookupResult, MasterPort,
    VolumeLocation,
};
use reqwest::Client;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct AssignResponse {
    fid: String,
    url: String,
    #[serde(rename = "publicUrl")]
    public_url: Option<String>,
    count: Option<u32>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LookupResponse {
    #[serde(rename = "volumeId")]
    #[allow(dead_code)]
    volume_id: Option<String>,
    locations: Option<Vec<LocationResponse>>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LocationResponse {
    url: String,
    #[serde(rename = "publicUrl")]
    public_url: Option<String>,
}

/// HTTP implementation of `MasterPort`.
#[derive(Debug, Clone)]
pub struct HttpMasterClient {
    client: Client,
    base_url: String,
}

impl HttpMasterClient {
    /// Creates a new HTTP master client.
    #[must_use]
    pub fn new(client: Client, base_url: impl Into<String>) -> Self {
        let mut base_url = base_url.into();
        if base_url.ends_with('/') {
            base_url.pop();
        }
        Self { client, base_url }
    }

    /// Returns the base URL of this client.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    async fn assign_impl(&self, options: Option<AssignOptions>) -> DomainResult<AssignResult> {
        let mut url = format!("{}/dir/assign", self.base_url);
        let opts = options.unwrap_or_default();

        let mut params = Vec::new();
        if let Some(ref replication) = opts.replication {
            params.push(format!("replication={replication}"));
        }
        if let Some(ref dc) = opts.data_center {
            params.push(format!("dataCenter={dc}"));
        }
        if let Some(ref ttl) = opts.ttl {
            params.push(format!("ttl={ttl}"));
        }
        if let Some(ref collection) = opts.collection {
            params.push(format!("collection={collection}"));
        }

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        let response =
            self.client
                .get(&url)
                .send()
                .await
                .map_err(|e| DomainError::AssignmentFailed {
                    reason: format!("HTTP request failed: {e}"),
                })?;

        if !response.status().is_success() {
            return Err(DomainError::AssignmentFailed {
                reason: format!("HTTP status: {}", response.status()),
            });
        }

        let assign_resp: AssignResponse =
            response
                .json()
                .await
                .map_err(|e| DomainError::AssignmentFailed {
                    reason: format!("Failed to parse response: {e}"),
                })?;

        if let Some(error) = assign_resp.error {
            return Err(DomainError::AssignmentFailed { reason: error });
        }

        let file_id = FileId::parse(&assign_resp.fid)?;

        Ok(AssignResult {
            file_id,
            url: assign_resp.url,
            public_url: assign_resp.public_url,
            count: assign_resp.count.unwrap_or(1),
        })
    }

    async fn lookup_impl(&self, volume_id: u32) -> DomainResult<LookupResult> {
        let url = format!("{}/dir/lookup?volumeId={}", self.base_url, volume_id);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|_| DomainError::VolumeNotFound { volume_id })?;

        if !response.status().is_success() {
            return Err(DomainError::VolumeNotFound { volume_id });
        }

        let lookup_resp: LookupResponse = response
            .json()
            .await
            .map_err(|_| DomainError::VolumeNotFound { volume_id })?;

        if lookup_resp.error.is_some() {
            return Err(DomainError::VolumeNotFound { volume_id });
        }

        let locations = lookup_resp
            .locations
            .unwrap_or_default()
            .into_iter()
            .map(|loc| VolumeLocation {
                url: loc.url,
                public_url: loc.public_url,
            })
            .collect();

        Ok(LookupResult {
            volume_id,
            locations,
        })
    }
}

impl MasterPort for HttpMasterClient {
    async fn assign(&self, options: Option<AssignOptions>) -> DomainResult<AssignResult> {
        self.assign_impl(options).await
    }

    async fn lookup(&self, volume_id: u32) -> DomainResult<LookupResult> {
        self.lookup_impl(volume_id).await
    }
}

impl MasterPort for &HttpMasterClient {
    async fn assign(&self, options: Option<AssignOptions>) -> DomainResult<AssignResult> {
        self.assign_impl(options).await
    }

    async fn lookup(&self, volume_id: u32) -> DomainResult<LookupResult> {
        self.lookup_impl(volume_id).await
    }
}
