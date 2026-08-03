//! Domain errors for weedforge.

use thiserror::Error;

/// Result type for domain operations.
pub type DomainResult<T> = Result<T, DomainError>;

/// Domain-level errors.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainError {
    /// The file ID string format is invalid.
    #[error("invalid file ID '{value}': {reason}")]
    InvalidFileId {
        /// The invalid file ID value.
        value: String,
        /// The reason why the file ID is invalid.
        reason: String,
    },

    /// The volume ID is unknown or not found.
    #[error("volume {volume_id} not found")]
    VolumeNotFound {
        /// The volume ID that was not found.
        volume_id: u32,
    },

    /// No replicas are available for the volume.
    #[error("no replicas available for volume {volume_id}")]
    NoReplicasAvailable {
        /// The volume ID with no available replicas.
        volume_id: u32,
    },

    /// The file was not found.
    #[error("file not found: {file_id}")]
    FileNotFound {
        /// The file ID that was not found.
        file_id: String,
    },

    /// Assignment failed.
    #[error("assignment failed: {reason}")]
    AssignmentFailed {
        /// The reason for the assignment failure.
        reason: String,
    },

    /// Upload failed.
    #[error("upload failed: {reason}")]
    UploadFailed {
        /// The reason for the upload failure.
        reason: String,
    },

    /// Download failed.
    #[error("download failed: {reason}")]
    DownloadFailed {
        /// The reason for the download failure.
        reason: String,
    },

    /// Deletion failed.
    ///
    /// Distinct from [`DomainError::DownloadFailed`], which delete used to
    /// report: a failed delete and a failed read are different incidents and
    /// were indistinguishable in logs and in `match` arms.
    #[error("delete failed: {reason}")]
    DeleteFailed {
        /// The reason for the delete failure.
        reason: String,
    },

    /// A volume lookup could not be completed.
    ///
    /// This means the master could not be reached or did not answer usefully.
    /// It is deliberately NOT [`DomainError::VolumeNotFound`], which means the
    /// master answered and said the volume does not exist -- a terminal result
    /// that no amount of failing over to other masters can change.
    #[error("lookup of volume {volume_id} failed: {reason}")]
    LookupFailed {
        /// The volume ID that was being looked up.
        volume_id: u32,
        /// The reason the lookup could not be completed.
        reason: String,
    },

    /// Invalid URL format.
    #[error("invalid URL: {reason}")]
    InvalidUrl {
        /// The reason why the URL is invalid.
        reason: String,
    },

    /// All masters are unavailable.
    #[error("all masters unavailable")]
    AllMastersUnavailable,

    /// Configuration error.
    #[error("configuration error: {reason}")]
    ConfigurationError {
        /// The reason for the configuration error.
        reason: String,
    },
}
