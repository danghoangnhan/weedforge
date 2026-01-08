//! Read file use case.

use crate::domain::{DomainError, DomainResult, FileId, LookupResult, MasterPort, VolumePort};

/// Strategy for selecting a replica.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ReplicaSelection {
    #[default]
    Random,
    First,
}

/// Options for reading a file.
#[derive(Debug, Clone, Default)]
pub struct ReadOptions {
    pub replica_selection: ReplicaSelection,
}

/// Result of a read operation.
#[derive(Debug, Clone)]
pub struct ReadResult {
    pub data: Vec<u8>,
    pub lookup: LookupResult,
    pub source_url: String,
}

/// Use case for reading files from SeaweedFS.
pub struct ReadFileUseCase<M, V> {
    master: M,
    volume: V,
}

impl<M, V> ReadFileUseCase<M, V>
where
    M: MasterPort,
    V: VolumePort,
{
    pub const fn new(master: M, volume: V) -> Self {
        Self { master, volume }
    }

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

    pub async fn lookup(&self, file_id: &FileId) -> DomainResult<LookupResult> {
        self.master.lookup(file_id.volume_id()).await
    }
}

fn simple_hash(file_id: &FileId) -> usize {
    let combined =
        (u64::from(file_id.volume_id()) << 32) | (file_id.file_key() ^ u64::from(file_id.cookie()));
    combined as usize
}
