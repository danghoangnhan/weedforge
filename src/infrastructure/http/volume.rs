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
}

impl HttpVolumeClient {
    /// Creates a new HTTP volume client.
    #[must_use]
    pub const fn new(client: Client) -> Self {
        Self { client }
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

        let response = self.client.get(&download_url).send().await.map_err(|e| {
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

        let bytes = response
            .bytes()
            .await
            .map_err(|e| DomainError::DownloadFailed {
                reason: format!("Failed to read response body: {e}"),
            })?;

        Ok(bytes.to_vec())
    }

    async fn delete_impl(&self, url: &str, file_id: &FileId) -> DomainResult<()> {
        let delete_url = Self::build_file_url(url, file_id);

        let response = self.client.delete(&delete_url).send().await.map_err(|e| {
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
