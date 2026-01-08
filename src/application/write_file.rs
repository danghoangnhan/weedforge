//! Write file use case.

use crate::domain::{AssignOptions, AssignResult, DomainResult, FileId, MasterPort, VolumePort};

/// Options for writing a file.
#[derive(Debug, Clone, Default)]
pub struct WriteOptions {
    pub filename: Option<String>,
    pub content_type: Option<String>,
    pub replication: Option<String>,
    pub data_center: Option<String>,
    pub collection: Option<String>,
    pub ttl: Option<String>,
}

impl WriteOptions {
    #[must_use]
    pub fn with_filename(filename: impl Into<String>) -> Self {
        Self {
            filename: Some(filename.into()),
            ..Default::default()
        }
    }

    #[must_use]
    pub fn content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }
}

/// Result of a write operation.
#[derive(Debug, Clone)]
pub struct WriteResult {
    pub file_id: FileId,
    pub size: u64,
    pub etag: Option<String>,
    pub assignment: AssignResult,
}

/// Use case for writing files to SeaweedFS.
pub struct WriteFileUseCase<M, V> {
    master: M,
    volume: V,
}

impl<M, V> WriteFileUseCase<M, V>
where
    M: MasterPort,
    V: VolumePort,
{
    pub const fn new(master: M, volume: V) -> Self {
        Self { master, volume }
    }

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
