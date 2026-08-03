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
        let url = format!("{}/dir/assign", self.base_url);
        let opts = options.unwrap_or_default();

        let mut params: Vec<(&str, &str)> = Vec::new();
        if let Some(ref replication) = opts.replication {
            params.push(("replication", replication.as_str()));
        }
        if let Some(ref dc) = opts.data_center {
            params.push(("dataCenter", dc.as_str()));
        }
        // Previously dropped on the floor: AssignOptions carried a rack and
        // nothing ever sent it, so rack-targeted placement silently did nothing.
        if let Some(ref rack) = opts.rack {
            params.push(("rack", rack.as_str()));
        }
        if let Some(ref ttl) = opts.ttl {
            params.push(("ttl", ttl.as_str()));
        }
        if let Some(ref collection) = opts.collection {
            params.push(("collection", collection.as_str()));
        }

        let response = self
            .client
            .get(&url)
            // Percent-encoded by reqwest. Concatenating these by hand let a
            // value containing '&' or '=' inject its own parameters -- a
            // collection of "logs&replication=000" quietly turned a replicated
            // write into an unreplicated one.
            .query(&params)
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
        let url = format!("{}/dir/lookup", self.base_url);

        // Every failure below used to collapse into VolumeNotFound. The HA layer
        // reads that as "this master is broken", so one genuinely missing volume
        // cost a full retry sweep and marked all three healthy masters failed.
        // Only the master's own error field means the volume is really absent.
        let response = self
            .client
            .get(&url)
            .query(&[("volumeId", volume_id)])
            .send()
            .await
            .map_err(|e| DomainError::LookupFailed {
                volume_id,
                reason: format!("HTTP request failed: {e}"),
            })?;

        if !response.status().is_success() {
            return Err(DomainError::LookupFailed {
                volume_id,
                reason: format!("HTTP status: {}", response.status()),
            });
        }

        let lookup_resp: LookupResponse =
            response
                .json()
                .await
                .map_err(|e| DomainError::LookupFailed {
                    volume_id,
                    reason: format!("Failed to parse response: {e}"),
                })?;

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
