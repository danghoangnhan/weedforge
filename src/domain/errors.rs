//! Domain errors for weedforge.

use thiserror::Error;

/// Result type for domain operations.
pub type DomainResult<T> = Result<T, DomainError>;

/// Domain-level errors.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainError {
    /// The file ID string format is invalid.
    #[error("invalid file ID '{value}': {reason}")]
    InvalidFileId { value: String, reason: String },

    /// The volume ID is unknown or not found.
    #[error("volume {volume_id} not found")]
    VolumeNotFound { volume_id: u32 },

    /// No replicas are available for the volume.
    #[error("no replicas available for volume {volume_id}")]
    NoReplicasAvailable { volume_id: u32 },

    /// The file was not found.
    #[error("file not found: {file_id}")]
    FileNotFound { file_id: String },

    /// Assignment failed.
    #[error("assignment failed: {reason}")]
    AssignmentFailed { reason: String },

    /// Upload failed.
    #[error("upload failed: {reason}")]
    UploadFailed { reason: String },

    /// Download failed.
    #[error("download failed: {reason}")]
    DownloadFailed { reason: String },

    /// Invalid URL format.
    #[error("invalid URL: {reason}")]
    InvalidUrl { reason: String },

    /// All masters are unavailable.
    #[error("all masters unavailable")]
    AllMastersUnavailable,

    /// Configuration error.
    #[error("configuration error: {reason}")]
    ConfigurationError { reason: String },
}
