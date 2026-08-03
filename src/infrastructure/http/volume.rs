//! HTTP implementation of the volume port.

use crate::domain::{DomainError, DomainResult, FileId, UploadResult, VolumePort};
use reqwest::multipart::{Form, Part};
use reqwest::Client;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct UploadResponse {
    #[serde(default)]
    size: u64,
    #[serde(rename = "eTag")]
    etag: Option<String>,
    error: Option<String>,
}

/// HTTP implementation of `VolumePort`.
#[derive(Debug, Clone)]
pub struct HttpVolumeClient {
    client: Client,
    max_download_bytes: Option<u64>,
}

impl HttpVolumeClient {
    /// Creates a new HTTP volume client with no download size limit.
    #[must_use]
    pub const fn new(client: Client) -> Self {
        Self {
            client,
            max_download_bytes: None,
        }
    }

    /// Caps a single download, in bytes. `None` means unlimited.
    #[must_use]
    pub const fn with_max_download_bytes(mut self, limit: Option<u64>) -> Self {
        self.max_download_bytes = limit;
        self
    }

    fn build_file_url(base_url: &str, file_id: &FileId) -> String {
        let base = base_url.strip_suffix('/').unwrap_or(base_url);

        let base = if !base.starts_with("http://") && !base.starts_with("https://") {
            format!("http://{base}")
        } else {
            base.to_string()
        };

        format!("{}/{}", base, file_id.render())
    }

    async fn upload_impl(
        &self,
        url: &str,
        file_id: &FileId,
        data: Vec<u8>,
        filename: Option<&str>,
        content_type: Option<&str>,
    ) -> DomainResult<UploadResult> {
        let upload_url = Self::build_file_url(url, file_id);
        let filename = filename.unwrap_or("file");
        let content_type = content_type.unwrap_or("application/octet-stream");

        let part = Part::bytes(data)
            .file_name(filename.to_string())
            .mime_str(content_type)
            .map_err(|e| DomainError::UploadFailed {
                reason: format!("Invalid content type: {e}"),
            })?;

        let form = Form::new().part("file", part);

        let response = self
            .client
            .post(&upload_url)
            .multipart(form)
            .send()
            .await
            .map_err(|e| DomainError::UploadFailed {
                reason: format!("HTTP request failed: {e}"),
            })?;

        if !response.status().is_success() {
            return Err(DomainError::UploadFailed {
                reason: format!("HTTP status: {}", response.status()),
            });
        }

        let upload_resp: UploadResponse =
            response
                .json()
                .await
                .map_err(|e| DomainError::UploadFailed {
                    reason: format!("Failed to parse response: {e}"),
                })?;

        if let Some(error) = upload_resp.error {
            return Err(DomainError::UploadFailed { reason: error });
        }

        Ok(UploadResult {
            file_id: file_id.clone(),
            size: upload_resp.size,
            etag: upload_resp.etag,
        })
    }

    async fn download_impl(&self, url: &str, file_id: &FileId) -> DomainResult<Vec<u8>> {
        let download_url = Self::build_file_url(url, file_id);

        // `mut` because the body is consumed chunk by chunk below rather than
        // buffered in one shot.
        let mut response = self.client.get(&download_url).send().await.map_err(|e| {
            DomainError::DownloadFailed {
                reason: format!("HTTP request failed: {e}"),
            }
        })?;

        if response.status().as_u16() == 404 {
            return Err(DomainError::FileNotFound {
                file_id: file_id.to_string(),
            });
        }

        if !response.status().is_success() {
            return Err(DomainError::DownloadFailed {
                reason: format!("HTTP status: {}", response.status()),
            });
        }

        // Reject on the advertised length before allocating anything at all.
        // A cap that only trips after the buffer has already grown is not a cap.
        if let (Some(limit), Some(advertised)) =
            (self.max_download_bytes, response.content_length())
        {
            if advertised > limit {
                return Err(DomainError::DownloadFailed {
                    reason: format!(
                        "body advertises {advertised} bytes, over the {limit} byte limit"
                    ),
                });
            }
        }

        // Streamed rather than buffered whole, so a server that lies about (or
        // omits) Content-Length still cannot force an unbounded allocation.
        let mut buffer: Vec<u8> = Vec::new();
        while let Some(chunk) =
            response
                .chunk()
                .await
                .map_err(|e| DomainError::DownloadFailed {
                    reason: format!("Failed to read response body: {e}"),
                })?
        {
            if let Some(limit) = self.max_download_bytes {
                let total = u64::try_from(buffer.len().saturating_add(chunk.len()))
                    .unwrap_or(u64::MAX);
                if total > limit {
                    return Err(DomainError::DownloadFailed {
                        reason: format!("body exceeded the {limit} byte limit"),
                    });
                }
            }
            buffer.extend_from_slice(&chunk);
        }

        Ok(buffer)
    }

    async fn delete_impl(&self, url: &str, file_id: &FileId) -> DomainResult<()> {
        let delete_url = Self::build_file_url(url, file_id);

        // DeleteFailed, not DownloadFailed. These used to be the same variant,
        // so a failed delete was indistinguishable from a failed read.
        let response = self.client.delete(&delete_url).send().await.map_err(|e| {
            DomainError::DeleteFailed {
                reason: format!("HTTP request failed: {e}"),
            }
        })?;

        if response.status().as_u16() == 404 {
            return Err(DomainError::FileNotFound {
                file_id: file_id.to_string(),
            });
        }

        if !response.status().is_success() {
            return Err(DomainError::DeleteFailed {
                reason: format!("HTTP status: {}", response.status()),
            });
        }

        Ok(())
    }
}

impl VolumePort for HttpVolumeClient {
    async fn upload(
        &self,
        url: &str,
        file_id: &FileId,
        data: Vec<u8>,
        filename: Option<&str>,
        content_type: Option<&str>,
    ) -> DomainResult<UploadResult> {
        self.upload_impl(url, file_id, data, filename, content_type)
            .await
    }

    async fn download(&self, url: &str, file_id: &FileId) -> DomainResult<Vec<u8>> {
        self.download_impl(url, file_id).await
    }

    async fn delete(&self, url: &str, file_id: &FileId) -> DomainResult<()> {
        self.delete_impl(url, file_id).await
    }
}

impl VolumePort for &HttpVolumeClient {
    async fn upload(
        &self,
        url: &str,
        file_id: &FileId,
        data: Vec<u8>,
        filename: Option<&str>,
        content_type: Option<&str>,
    ) -> DomainResult<UploadResult> {
        self.upload_impl(url, file_id, data, filename, content_type)
            .await
    }

    async fn download(&self, url: &str, file_id: &FileId) -> DomainResult<Vec<u8>> {
        self.download_impl(url, file_id).await
    }

    async fn delete(&self, url: &str, file_id: &FileId) -> DomainResult<()> {
        self.delete_impl(url, file_id).await
    }
}
