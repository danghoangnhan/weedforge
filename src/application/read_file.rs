//! Read file use case.

use crate::domain::{DomainError, DomainResult, FileId, LookupResult, MasterPort, VolumePort};

/// Strategy for selecting a replica.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ReplicaSelection {
    /// Select a replica randomly (based on file ID hash).
    #[default]
    Random,
    /// Always select the first available replica.
    First,
}

/// Options for reading a file.
#[derive(Debug, Clone, Default)]
pub struct ReadOptions {
    /// Strategy for selecting which replica to read from.
    pub replica_selection: ReplicaSelection,
}

/// Result of a read operation.
#[derive(Debug, Clone)]
pub struct ReadResult {
    /// The file data.
    pub data: Vec<u8>,
    /// The lookup result containing volume locations.
    pub lookup: LookupResult,
    /// The URL from which the file was downloaded.
    pub source_url: String,
}

/// Use case for reading files from `SeaweedFS`.
pub struct ReadFileUseCase<M, V> {
    master: M,
    volume: V,
}

impl<M, V> ReadFileUseCase<M, V>
where
    M: MasterPort,
    V: VolumePort,
{
    /// Creates a new `ReadFileUseCase`.
    pub const fn new(master: M, volume: V) -> Self {
        Self { master, volume }
    }

    /// Executes the read file use case.
    ///
    /// # Errors
    ///
    /// Returns an error if the volume lookup fails or the download fails.
    pub async fn execute(
        &self,
        file_id: &FileId,
        options: Option<ReadOptions>,
    ) -> DomainResult<ReadResult> {
        let opts = options.unwrap_or_default();
        let lookup = self.master.lookup(file_id.volume_id()).await?;

        if lookup.locations.is_empty() {
            return Err(DomainError::NoReplicasAvailable {
                volume_id: file_id.volume_id(),
            });
        }

        let location_index = match opts.replica_selection {
            ReplicaSelection::First => 0,
            ReplicaSelection::Random => simple_hash(file_id) % lookup.locations.len(),
        };

        let source_url = lookup.locations[location_index].url.clone();
        let data = self.volume.download(&source_url, file_id).await?;

        Ok(ReadResult {
            data,
            lookup,
            source_url,
        })
    }

    /// Looks up the volume locations for a file ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the volume lookup fails.
    pub async fn lookup(&self, file_id: &FileId) -> DomainResult<LookupResult> {
        self.master.lookup(file_id.volume_id()).await
    }
}

#[allow(clippy::cast_possible_truncation)]
fn simple_hash(file_id: &FileId) -> usize {
    let combined =
        (u64::from(file_id.volume_id()) << 32) | (file_id.file_key() ^ u64::from(file_id.cookie()));
    combined as usize
}
