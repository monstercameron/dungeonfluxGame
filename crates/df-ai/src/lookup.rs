/// Borrowed identity, approved basis, and artifact from one immutable stored entry.
///
/// This is a lookup view, not a recording-admission or artifact-publication contract.
pub type ReadEntry<'a, Key, Basis, Artifact> = (&'a Key, &'a Basis, &'a Artifact);

/// Stored-entry absence and storage failure remain separate at the read boundary.
pub type ReadResult<'a, Key, Basis, Artifact, Failure> =
    Result<Option<ReadEntry<'a, Key, Basis, Artifact>>, Failure>;

/// Read-only access to complete, previously admitted prepared and replay artifacts.
///
/// The owner supplies canonical key, basis, and artifact types. Implementations must
/// read stored entries without generating content, invoking a live provider, changing
/// execution mode, or publishing a partial/unapproved recording. Each call performs
/// one bounded lookup and preserves storage failure rather than reporting a miss.
/// The borrowed key and basis identify the same immutable entry as the artifact.
pub trait PreparedRead {
    type Key: Eq;
    type Basis: Eq;
    type Artifact;
    type Failure;

    fn read_prepared(
        &self,
        key: &Self::Key,
    ) -> ReadResult<'_, Self::Key, Self::Basis, Self::Artifact, Self::Failure>;

    fn read_replay(
        &self,
        key: &Self::Key,
    ) -> ReadResult<'_, Self::Key, Self::Basis, Self::Artifact, Self::Failure>;
}

/// Honest outcomes when a stored artifact cannot satisfy this admitted lookup.
#[derive(Debug, Eq, PartialEq)]
pub enum LookupError<Failure> {
    Missing,
    Stale,
    Unavailable(Failure),
}

/// Reads a prepared artifact only when its entire canonical key and basis match.
///
/// No fallback or live-provider callable is accepted. The caller supplies the exact
/// current approved basis; freshness is never inferred from elapsed time or a prefix.
pub fn lookup_prepared<'a, Store: PreparedRead>(
    store: &'a Store,
    key: &Store::Key,
    current_basis: &Store::Basis,
) -> Result<&'a Store::Artifact, LookupError<Store::Failure>> {
    resolve(store.read_prepared(key), key, current_basis)
}

/// Reads a complete replay artifact without starting live work on any outcome.
///
/// Replay and prepared storage are deliberately queried through separate read ports;
/// a replay miss does not search another mode or retry a storage failure.
pub fn lookup_replay<'a, Store: PreparedRead>(
    store: &'a Store,
    key: &Store::Key,
    current_basis: &Store::Basis,
) -> Result<&'a Store::Artifact, LookupError<Store::Failure>> {
    resolve(store.read_replay(key), key, current_basis)
}

fn resolve<'a, Key: Eq, Basis: Eq, Artifact, Failure>(
    entry: Result<Option<ReadEntry<'a, Key, Basis, Artifact>>, Failure>,
    key: &Key,
    current_basis: &Basis,
) -> Result<&'a Artifact, LookupError<Failure>> {
    let (stored_key, stored_basis, artifact) = entry
        .map_err(LookupError::Unavailable)?
        .ok_or(LookupError::Missing)?;
    if stored_key != key || stored_basis != current_basis {
        return Err(LookupError::Stale);
    }
    Ok(artifact)
}
