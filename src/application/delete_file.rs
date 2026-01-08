//! Delete file use case.

use crate::domain::{DomainError, DomainResult, FileId, MasterPort, VolumePort};

/// Use case for deleting files from `SeaweedFS`.
pub struct DeleteFileUseCase<M, V> {
    master: M,
    volume: V,
}

impl<M, V> DeleteFileUseCase<M, V>
where
    M: MasterPort,
    V: VolumePort,
{
    /// Creates a new `DeleteFileUseCase`.
    pub const fn new(master: M, volume: V) -> Self {
        Self { master, volume }
    }

    /// Executes the delete file use case.
    ///
    /// # Errors
    ///
    /// Returns an error if the file lookup fails or the deletion fails.
    pub async fn execute(&self, file_id: &FileId) -> DomainResult<()> {
        let lookup = self.master.lookup(file_id.volume_id()).await?;

        if lookup.locations.is_empty() {
            return Err(DomainError::NoReplicasAvailable {
                volume_id: file_id.volume_id(),
            });
        }

        let location = &lookup.locations[0];
        self.volume.delete(&location.url, file_id).await
    }
}
