use df_ai::lookup::{LookupError, PreparedRead, ReadEntry, lookup_prepared, lookup_replay};
use std::cell::Cell;

// Fixture-owned opaque identity dimensions, not a production cache-key definition.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Key {
    contract: u32,
    context_revision: u64,
    locale: &'static str,
    model: &'static str,
    output_parameter: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Basis {
    source_revision: u64,
    access_generation: u64,
}

#[derive(Debug, Eq, PartialEq)]
struct Artifact(Vec<u8>);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StorageFailure {
    Offline,
}

struct Entry {
    key: Key,
    basis: Basis,
    artifact: Artifact,
}

struct StoredArtifacts {
    prepared: Option<Entry>,
    replay: Option<Entry>,
    failure: bool,
    prepared_reads: Cell<usize>,
    replay_reads: Cell<usize>,
    live_calls: Cell<usize>,
}

impl StoredArtifacts {
    fn new(prepared: Option<Entry>, replay: Option<Entry>) -> Self {
        Self {
            prepared,
            replay,
            failure: false,
            prepared_reads: Cell::new(0),
            replay_reads: Cell::new(0),
            live_calls: Cell::new(0),
        }
    }

    fn read<'a>(
        &'a self,
        entry: &'a Option<Entry>,
    ) -> Result<Option<ReadEntry<'a, Key, Basis, Artifact>>, StorageFailure> {
        if self.failure {
            return Err(StorageFailure::Offline);
        }
        Ok(entry
            .as_ref()
            .map(|entry| (&entry.key, &entry.basis, &entry.artifact)))
    }
}

impl PreparedRead for StoredArtifacts {
    type Key = Key;
    type Basis = Basis;
    type Artifact = Artifact;
    type Failure = StorageFailure;

    fn read_prepared(
        &self,
        _: &Key,
    ) -> Result<Option<ReadEntry<'_, Key, Basis, Artifact>>, StorageFailure> {
        self.prepared_reads.set(self.prepared_reads.get() + 1);
        self.read(&self.prepared)
    }

    fn read_replay(
        &self,
        _: &Key,
    ) -> Result<Option<ReadEntry<'_, Key, Basis, Artifact>>, StorageFailure> {
        self.replay_reads.set(self.replay_reads.get() + 1);
        self.read(&self.replay)
    }
}

fn key() -> Key {
    Key {
        contract: 2,
        context_revision: 17,
        locale: "en-gb",
        model: "fixture-model",
        output_parameter: 9,
    }
}

fn basis() -> Basis {
    Basis {
        source_revision: 8,
        access_generation: 3,
    }
}

fn entry(bytes: &[u8]) -> Entry {
    Entry {
        key: key(),
        basis: basis(),
        artifact: Artifact(bytes.to_vec()),
    }
}

#[test]
fn exact_prepared_and_replay_hits_borrow_their_actual_distinct_artifacts() {
    let store = StoredArtifacts::new(Some(entry(b"prepared")), Some(entry(b"replay")));
    let prepared = lookup_prepared(&store, &key(), &basis()).unwrap();
    let replay = lookup_replay(&store, &key(), &basis()).unwrap();
    assert_eq!(prepared.0, b"prepared");
    assert_eq!(replay.0, b"replay");
    assert!(std::ptr::eq(
        prepared,
        &store.prepared.as_ref().unwrap().artifact
    ));
    assert!(std::ptr::eq(
        replay,
        &store.replay.as_ref().unwrap().artifact
    ));
    assert_eq!(store.prepared_reads.get(), 1);
    assert_eq!(store.replay_reads.get(), 1);
    assert_eq!(store.live_calls.get(), 0);
}

#[test]
fn misses_are_honest_and_never_search_another_mode_or_start_live_work() {
    let prepared_only = StoredArtifacts::new(Some(entry(b"prepared")), None);
    assert_eq!(
        lookup_replay(&prepared_only, &key(), &basis()),
        Err(LookupError::Missing)
    );
    assert_eq!(prepared_only.prepared_reads.get(), 0);
    assert_eq!(prepared_only.replay_reads.get(), 1);
    assert_eq!(prepared_only.live_calls.get(), 0);

    let replay_only = StoredArtifacts::new(None, Some(entry(b"replay")));
    assert_eq!(
        lookup_prepared(&replay_only, &key(), &basis()),
        Err(LookupError::Missing)
    );
    assert_eq!(replay_only.prepared_reads.get(), 1);
    assert_eq!(replay_only.replay_reads.get(), 0);
    assert_eq!(replay_only.live_calls.get(), 0);
}

#[test]
fn storage_failure_is_preserved_without_retry_or_fabricated_missing() {
    let mut store = StoredArtifacts::new(Some(entry(b"prepared")), Some(entry(b"replay")));
    store.failure = true;
    assert_eq!(
        lookup_prepared(&store, &key(), &basis()),
        Err(LookupError::Unavailable(StorageFailure::Offline))
    );
    assert_eq!(
        lookup_replay(&store, &key(), &basis()),
        Err(LookupError::Unavailable(StorageFailure::Offline))
    );
    assert_eq!(store.prepared_reads.get(), 1);
    assert_eq!(store.replay_reads.get(), 1);
    assert_eq!(store.live_calls.get(), 0);
}

#[test]
fn every_opaque_key_dimension_is_compared_exactly_before_artifact_release() {
    let mut variations = [key(), key(), key(), key(), key()];
    variations[0].contract += 1;
    variations[1].context_revision += 1;
    variations[2].locale = "en";
    variations[3].model = "other-model";
    variations[4].output_parameter += 1;
    for requested in variations {
        let store = StoredArtifacts::new(Some(entry(b"prepared")), Some(entry(b"replay")));
        assert_eq!(
            lookup_prepared(&store, &requested, &basis()),
            Err(LookupError::Stale)
        );
        assert_eq!(
            lookup_replay(&store, &requested, &basis()),
            Err(LookupError::Stale)
        );
        assert_eq!(store.prepared_reads.get(), 1);
        assert_eq!(store.replay_reads.get(), 1);
        assert_eq!(store.live_calls.get(), 0);
    }
}

#[test]
fn source_or_access_generation_change_refuses_old_artifact_without_mutation() {
    let mut variations = [basis(), basis()];
    variations[0].source_revision += 1;
    variations[1].access_generation += 1;
    for current in variations {
        let store = StoredArtifacts::new(Some(entry(b"prepared")), Some(entry(b"replay")));
        assert_eq!(
            lookup_prepared(&store, &key(), &current),
            Err(LookupError::Stale)
        );
        assert_eq!(
            lookup_replay(&store, &key(), &current),
            Err(LookupError::Stale)
        );
        assert_eq!(store.prepared.as_ref().unwrap().artifact.0, b"prepared");
        assert_eq!(store.replay.as_ref().unwrap().artifact.0, b"replay");
        assert_eq!(store.live_calls.get(), 0);
    }
}

#[test]
fn empty_admitted_artifact_is_preserved_without_synthesizing_a_fallback() {
    let store = StoredArtifacts::new(Some(entry(b"")), Some(entry(b"")));
    assert_eq!(lookup_prepared(&store, &key(), &basis()).unwrap().0, b"");
    assert_eq!(lookup_replay(&store, &key(), &basis()).unwrap().0, b"");
    assert_eq!(store.live_calls.get(), 0);
}
