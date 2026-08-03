//! Read file use case.

use crate::domain::{DomainError, DomainResult, FileId, LookupResult, MasterPort, VolumePort};
use rand::RngExt;

/// Strategy for selecting which replica a read *starts* from.
///
/// With `-replication=010` a volume has one replica per rack, and `lookup`
/// returns all of them. This only chooses where to start: a read falls back to
/// the remaining replicas if the first one cannot be reached.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ReplicaSelection {
    /// Map a file id deterministically onto one replica.
    ///
    /// Stable and cache-friendly -- the same object always starts at the same
    /// replica -- but it does not spread read load, because a hot object is
    /// always served by the same server. Pick [`ReplicaSelection::Random`] when
    /// spreading matters more than cache locality.
    ///
    /// This was previously called `Random`, which it never was.
    #[default]
    Sticky,
    /// Always start at the first replica the master returned.
    First,
    /// Choose a uniformly random replica per request.
    ///
    /// Genuinely random, unlike the old `Random`. This is what actually spreads
    /// reads across racks.
    Random,
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
    /// Tries every replica the master returned, starting from the one
    /// [`ReplicaSelection`] chooses, and only fails once all of them have. A
    /// replicated object stays readable while any one of its replicas is up,
    /// which is the entire reason for replicating it.
    ///
    /// # Errors
    ///
    /// Returns an error if the volume lookup fails, the volume has no replicas,
    /// or every replica failed to serve the object.
    pub async fn execute(
        &self,
        file_id: &FileId,
        options: Option<ReadOptions>,
    ) -> DomainResult<ReadResult> {
        let opts = options.unwrap_or_default();
        let lookup = self.master.lookup(file_id.volume_id()).await?;

        let count = lookup.locations.len();
        if count == 0 {
            return Err(DomainError::NoReplicasAvailable {
                volume_id: file_id.volume_id(),
            });
        }

        let start = select_replica(opts.replica_selection, file_id, count);
        let mut last_error = None;
        let mut fetched = None;

        for offset in 0..count {
            let index = (start + offset) % count;
            let source_url = lookup.locations[index].url.clone();
            match self.volume.download(&source_url, file_id).await {
                Ok(data) => {
                    fetched = Some((source_url, data));
                    break;
                }
                // A 404 from a replica that answered is a real answer: the
                // object is gone. Asking its peers would only make a definite
                // result slower.
                Err(e @ DomainError::FileNotFound { .. }) => return Err(e),
                Err(e) => last_error = Some(e),
            }
        }

        match fetched {
            Some((source_url, data)) => Ok(ReadResult {
                data,
                lookup,
                source_url,
            }),
            None => Err(last_error.unwrap_or_else(|| DomainError::NoReplicasAvailable {
                volume_id: file_id.volume_id(),
            })),
        }
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

/// Index of the replica a read should start at. `count` must be non-zero.
fn select_replica(selection: ReplicaSelection, file_id: &FileId, count: usize) -> usize {
    match selection {
        ReplicaSelection::First => 0,
        ReplicaSelection::Sticky => {
            let modulus = u64::try_from(count).unwrap_or(u64::MAX).max(1);
            usize::try_from(sticky_hash(file_id) % modulus).unwrap_or(0)
        }
        ReplicaSelection::Random => rand::rng().random_range(0..count),
    }
}

/// FNV-1a over the file id's 16 canonical bytes.
///
/// Byte-for-byte identical to the Go client's `stickyHash`, so both pick the
/// same replica for the same object. The previous implementation folded the
/// volume id into the high 32 bits and xor'd key with cookie, which meant that
/// modulo two -- the replica count under `-replication=010` -- the choice
/// depended only on the low bit of `file_key ^ cookie` and the volume id
/// contributed nothing at all.
fn sticky_hash(file_id: &FileId) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    let mut bytes = [0u8; 16];
    bytes[0..4].copy_from_slice(&file_id.volume_id().to_be_bytes());
    bytes[4..12].copy_from_slice(&file_id.file_key().to_be_bytes());
    bytes[12..16].copy_from_slice(&file_id.cookie().to_be_bytes());

    let mut hash = OFFSET_BASIS;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sticky_is_stable_for_the_same_file() {
        let fid = FileId::new(3, 1, 0x6370_37d6);
        let first = select_replica(ReplicaSelection::Sticky, &fid, 2);
        for _ in 0..16 {
            assert_eq!(select_replica(ReplicaSelection::Sticky, &fid, 2), first);
        }
    }

    #[test]
    fn sticky_uses_the_volume_id() {
        // The old hash shifted volume_id into the high 32 bits, so it could not
        // affect `% 2`. These two ids differ only by volume, and must be able to
        // land on different replicas.
        let a = FileId::new(1, 7, 7);
        let b = FileId::new(2, 7, 7);
        assert_ne!(sticky_hash(&a), sticky_hash(&b));
    }

    #[test]
    fn sticky_spreads_across_two_replicas() {
        // Not a distribution test -- just proof that both replicas are reachable,
        // which the pre-fix hash could fail for whole classes of file id.
        let mut seen = [false; 2];
        for key in 0..64u64 {
            seen[select_replica(ReplicaSelection::Sticky, &FileId::new(9, key, 3), 2)] = true;
        }
        assert!(seen[0] && seen[1], "sticky hash never reached one replica");
    }

    #[test]
    fn first_always_starts_at_zero() {
        let fid = FileId::new(3, 1, 0x6370_37d6);
        assert_eq!(select_replica(ReplicaSelection::First, &fid, 3), 0);
    }

    #[test]
    fn random_stays_in_range() {
        let fid = FileId::new(3, 1, 0x6370_37d6);
        for _ in 0..64 {
            assert!(select_replica(ReplicaSelection::Random, &fid, 3) < 3);
        }
    }
}
