//! Port traits (interfaces) for the domain layer.

use super::entities::FileId;
use super::errors::DomainResult;

/// Result of a file assignment from the master server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignResult {
    /// The assigned file ID.
    pub file_id: FileId,
    /// The URL of the volume server to upload to.
    pub url: String,
    /// Optional public URL for the volume server.
    pub public_url: Option<String>,
    /// Number of file IDs assigned.
    pub count: u32,
}

/// Location information for a volume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeLocation {
    /// The internal URL of the volume server.
    pub url: String,
    /// Optional public URL of the volume server.
    pub public_url: Option<String>,
}

/// Result of a volume lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LookupResult {
    /// The volume ID that was looked up.
    pub volume_id: u32,
    /// List of volume server locations.
    pub locations: Vec<VolumeLocation>,
}

/// Options for file assignment.
#[derive(Debug, Clone, Default)]
pub struct AssignOptions {
    /// Replication strategy (e.g., "000", "001", "010").
    pub replication: Option<String>,
    /// Preferred data center.
    pub data_center: Option<String>,
    /// Preferred rack within the data center.
    pub rack: Option<String>,
    /// Time-to-live for the file.
    pub ttl: Option<String>,
    /// Collection name for grouping files.
    pub collection: Option<String>,
}

/// Upload result from the volume server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadResult {
    /// The file ID of the uploaded file.
    pub file_id: FileId,
    /// The size of the uploaded file in bytes.
    pub size: u64,
    /// Optional `ETag` returned by the server.
    pub etag: Option<String>,
}

/// Port trait for master server operations.
pub trait MasterPort: Send + Sync {
    /// Assigns a new file ID for uploading.
    fn assign(
        &self,
        options: Option<AssignOptions>,
    ) -> impl std::future::Future<Output = DomainResult<AssignResult>> + Send;

    /// Looks up the locations for a volume ID.
    fn lookup(
        &self,
        volume_id: u32,
    ) -> impl std::future::Future<Output = DomainResult<LookupResult>> + Send;
}

/// Port trait for volume server operations.
pub trait VolumePort: Send + Sync {
    /// Uploads a file to the volume server.
    fn upload(
        &self,
        url: &str,
        file_id: &FileId,
        data: Vec<u8>,
        filename: Option<&str>,
        content_type: Option<&str>,
    ) -> impl std::future::Future<Output = DomainResult<UploadResult>> + Send;

    /// Downloads a file from the volume server.
    fn download(
        &self,
        url: &str,
        file_id: &FileId,
    ) -> impl std::future::Future<Output = DomainResult<Vec<u8>>> + Send;

    /// Deletes a file from the volume server.
    fn delete(
        &self,
        url: &str,
        file_id: &FileId,
    ) -> impl std::future::Future<Output = DomainResult<()>> + Send;
}
