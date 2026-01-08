//! Write file use case.

use crate::domain::{AssignOptions, AssignResult, DomainResult, FileId, MasterPort, VolumePort};

/// Options for writing a file.
#[derive(Debug, Clone, Default)]
pub struct WriteOptions {
    /// Optional filename for the uploaded file.
    pub filename: Option<String>,
    /// Optional content type (MIME type) for the file.
    pub content_type: Option<String>,
    /// Optional replication strategy.
    pub replication: Option<String>,
    /// Optional data center preference.
    pub data_center: Option<String>,
    /// Optional collection name.
    pub collection: Option<String>,
    /// Optional time-to-live for the file.
    pub ttl: Option<String>,
}

impl WriteOptions {
    /// Creates write options with the specified filename.
    #[must_use]
    pub fn with_filename(filename: impl Into<String>) -> Self {
        Self {
            filename: Some(filename.into()),
            ..Default::default()
        }
    }

    /// Sets the content type for the file.
    #[must_use]
    pub fn content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }
}

/// Result of a write operation.
#[derive(Debug, Clone)]
pub struct WriteResult {
    /// The assigned file ID.
    pub file_id: FileId,
    /// The size of the uploaded file in bytes.
    pub size: u64,
    /// Optional `ETag` from the server.
    pub etag: Option<String>,
    /// The assignment result from the master server.
    pub assignment: AssignResult,
}

/// Use case for writing files to `SeaweedFS`.
pub struct WriteFileUseCase<M, V> {
    master: M,
    volume: V,
}

impl<M, V> WriteFileUseCase<M, V>
where
    M: MasterPort,
    V: VolumePort,
{
    /// Creates a new `WriteFileUseCase`.
    pub const fn new(master: M, volume: V) -> Self {
        Self { master, volume }
    }

    /// Executes the write file use case.
    ///
    /// # Errors
    ///
    /// Returns an error if the assignment fails or the upload fails.
    pub async fn execute(
        &self,
        data: Vec<u8>,
        options: Option<WriteOptions>,
    ) -> DomainResult<WriteResult> {
        let opts = options.unwrap_or_default();

        let assign_options = AssignOptions {
            replication: opts.replication.clone(),
            data_center: opts.data_center.clone(),
            collection: opts.collection.clone(),
            ttl: opts.ttl.clone(),
            ..Default::default()
        };

        let assignment = self.master.assign(Some(assign_options)).await?;

        let upload_result = self
            .volume
            .upload(
                &assignment.url,
                &assignment.file_id,
                data,
                opts.filename.as_deref(),
                opts.content_type.as_deref(),
            )
            .await?;

        Ok(WriteResult {
            file_id: upload_result.file_id,
            size: upload_result.size,
            etag: upload_result.etag,
            assignment,
        })
    }
}
