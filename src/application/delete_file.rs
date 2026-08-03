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
    /// Tries each replica in turn. `SeaweedFS` propagates a delete to a volume's
    /// peers, so reaching any one of them is enough -- but reaching the *first*
    /// one is not guaranteed, and the previous implementation gave up when the
    /// server holding `locations[0]` was down.
    ///
    /// # Errors
    ///
    /// Returns an error if the file lookup fails, the volume has no replicas,
    /// the object does not exist, or every replica failed to delete it.
    pub async fn execute(&self, file_id: &FileId) -> DomainResult<()> {
        let lookup = self.master.lookup(file_id.volume_id()).await?;

        if lookup.locations.is_empty() {
            return Err(DomainError::NoReplicasAvailable {
                volume_id: file_id.volume_id(),
            });
        }

        let mut last_error = None;
        for location in &lookup.locations {
            match self.volume.delete(&location.url, file_id).await {
                Ok(()) => return Ok(()),
                // A replica that answered 404 has told us the object is gone.
                // That is an answer, not a transport failure.
                Err(e @ DomainError::FileNotFound { .. }) => return Err(e),
                Err(e) => last_error = Some(e),
            }
        }

        Err(
            last_error.unwrap_or_else(|| DomainError::NoReplicasAvailable {
                volume_id: file_id.volume_id(),
            }),
        )
    }
}
