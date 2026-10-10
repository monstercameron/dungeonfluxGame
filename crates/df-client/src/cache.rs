use df_assets::AssetManifest;
use df_types::{ClientBindingId, RevisionLabel, RunId, SessionId, SessionRevision};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::rc::{Rc, Weak};

/// One already audience-safe immutable server reference. The label and digest grant no access.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheKey {
    pub version: RevisionLabel,
    pub bytes: AssetManifest,
}

/// A mounted authenticated scope. Scope changes require disposing this cache.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheScope {
    pub session: SessionId,
    pub run: RunId,
    pub binding: ClientBindingId,
}

/// Finite caller-selected bounds, not measured device capacity promises.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheLimits {
    pub max_assets: usize,
    pub max_pending: usize,
    pub max_leases: usize,
    pub max_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheError {
    InvalidLimits,
    Closed,
    WrongScope,
    StaleRevision,
    ReferenceCapacity,
    ConflictingReference,
    NotCurrent,
    AlreadyPending,
    PendingCapacity,
    ByteCapacity,
    StaleFetch,
    LeaseCapacity,
    StaleLease,
    Incomplete,
    HashMismatch,
}

/// Identity of one owned fetch. Pointer identity cannot repeat while a late token exists,
/// including after release, revision replacement, teardown, or reconstruction of the cache.
/// The browser request owner must cancel transport work on release/update/disposal.
#[derive(Clone, Debug)]
pub struct FetchToken {
    identity: Rc<()>,
    scope: CacheScope,
    key: CacheKey,
}

impl FetchToken {
    pub fn key(&self) -> &CacheKey {
        &self.key
    }
}

/// Revocable consumer reference to cache-owned verified bytes. This handle owns no bytes;
/// every decode completion/resource use must recheck `AssetCache::lease_bytes`. Clones share
/// one lease slot. Dropping the final handle frees admission capacity on the next acquire.
#[derive(Clone, Debug)]
pub struct CacheLease {
    identity: Rc<()>,
    key: CacheKey,
}

impl CacheLease {
    /// Compares existing lease identity without granting access.
    pub fn same_identity(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.identity, &other.identity)
    }

    pub fn key(&self) -> &CacheKey {
        &self.key
    }
}

struct LeaseRecord {
    identity: Weak<()>,
    key: CacheKey,
}

struct Entry {
    key: CacheKey,
    bytes: Box<[u8]>,
}

/// Memory-only presentation optimization; never server publication or rights authority.
///
/// `apply_current` receives complete, already permitted server references from the accepted
/// view owner. It neither sees private server state nor derives permission. Removed/revoked
/// references immediately lose resident bytes and pending work. Unchanged keys retain work
/// within an epoch; a recovery epoch change fences all previous fetches.
/// Byte slices are borrowed, so cache eviction/release cannot leave cache-owned byte handles.
/// The cache creates no blob URLs, decoded resources, network tasks or playback effects;
/// their browser owners retain explicit cancellation and cleanup responsibilities.
pub struct AssetCache {
    scope: CacheScope,
    limits: CacheLimits,
    current: Option<SessionRevision>,
    references: Box<[CacheKey]>,
    pending: Vec<FetchToken>,
    leases: Vec<LeaseRecord>,
    entries: VecDeque<Entry>,
    resident_bytes: usize,
    closed: bool,
}

impl AssetCache {
    pub fn new(scope: CacheScope, limits: CacheLimits) -> Result<Self, CacheError> {
        if limits.max_assets == 0
            || limits.max_pending == 0
            || limits.max_leases == 0
            || limits.max_bytes == 0
        {
            return Err(CacheError::InvalidLimits);
        }
        Ok(Self {
            scope,
            limits,
            current: None,
            references: Box::default(),
            pending: Vec::new(),
            leases: Vec::new(),
            entries: VecDeque::new(),
            resident_bytes: 0,
            closed: false,
        })
    }

    /// Atomically installs a strictly newer complete permitted reference set. Rejected
    /// updates preserve the previous scope, current bytes, and outstanding fetch ownership.
    /// A still-current version cannot change its immutable manifest, including across recovery.
    pub fn apply_current(
        &mut self,
        scope: CacheScope,
        revision: SessionRevision,
        references: &[CacheKey],
    ) -> Result<(), CacheError> {
        self.check_scope(scope)?;
        if self.current.is_some_and(|current| revision <= current) {
            return Err(CacheError::StaleRevision);
        }
        if references.len() > self.limits.max_assets {
            return Err(CacheError::ReferenceCapacity);
        }
        for (index, key) in references.iter().enumerate() {
            if references
                .iter()
                .take(index)
                .any(|other| other.version == key.version)
            {
                return Err(CacheError::ConflictingReference);
            }
            if self
                .references
                .iter()
                .any(|current| current.version == key.version && current.bytes != key.bytes)
            {
                return Err(CacheError::ConflictingReference);
            }
        }
        let same_epoch = self
            .current
            .is_some_and(|current| current.epoch() == revision.epoch());
        self.references = references.to_vec().into_boxed_slice();
        self.current = Some(revision);
        if same_epoch {
            self.pending
                .retain(|token| self.references.contains(&token.key));
            self.leases.retain(|lease| {
                lease.identity.strong_count() != 0 && self.references.contains(&lease.key)
            });
        } else {
            self.pending.clear();
            self.leases.clear();
        }
        self.entries
            .retain(|entry| self.references.contains(&entry.key));
        self.resident_bytes = self.entries.iter().map(|entry| entry.bytes.len()).sum();
        Ok(())
    }

    /// Borrows only bytes still named by the current permitted view, updating LRU order.
    pub fn get(&mut self, key: &CacheKey) -> Result<Option<&[u8]>, CacheError> {
        self.check_current(key)?;
        let Some(index) = self.entries.iter().position(|entry| &entry.key == key) else {
            return Ok(None);
        };
        if let Some(entry) = self.entries.remove(index) {
            self.entries.push_back(entry);
        }
        Ok(self.entries.back().map(|entry| entry.bytes.as_ref()))
    }

    /// Acquires a bounded consumer lease only after a verified cache hit. Acquisition
    /// updates LRU order. No lease pins bytes past eviction/revocation or owns a byte clone.
    pub fn acquire(&mut self, key: &CacheKey) -> Result<Option<CacheLease>, CacheError> {
        self.check_current(key)?;
        if !self.entries.iter().any(|entry| &entry.key == key) {
            return Ok(None);
        }
        self.leases
            .retain(|lease| lease.identity.strong_count() != 0);
        if self.leases.len() >= self.limits.max_leases {
            return Err(CacheError::LeaseCapacity);
        }
        let lease = CacheLease {
            identity: Rc::new(()),
            key: key.clone(),
        };
        self.leases.push(LeaseRecord {
            identity: Rc::downgrade(&lease.identity),
            key: key.clone(),
        });
        if let Some(index) = self.entries.iter().position(|entry| &entry.key == key)
            && let Some(entry) = self.entries.remove(index)
        {
            self.entries.push_back(entry);
        }
        Ok(Some(lease))
    }

    /// Revalidates consumer lifetime against this exact cache and current resident key.
    /// The returned bytes borrow the cache, so mutation cannot free them while borrowed.
    /// After revocation, eviction, recovery, release or disposal, an old consumer cannot
    /// use this lease even if the identical asset is subsequently fetched again.
    pub fn lease_bytes(&self, lease: &CacheLease) -> Result<&[u8], CacheError> {
        if self.closed {
            return Err(CacheError::Closed);
        }
        let identity = Rc::downgrade(&lease.identity);
        if !self
            .leases
            .iter()
            .any(|active| Weak::ptr_eq(&active.identity, &identity))
        {
            return Err(CacheError::StaleLease);
        }
        self.check_current(&lease.key)?;
        self.entries
            .iter()
            .find(|entry| entry.key == lease.key)
            .map(|entry| entry.bytes.as_ref())
            .ok_or(CacheError::StaleLease)
    }

    /// Releases one consumer lease without evicting bytes used by another consumer.
    /// Releasing any clone invalidates the whole lease identity. Dropping all clones also
    /// frees admission capacity; no strong registry reference retains abandoned handles.
    pub fn release_lease(&mut self, lease: &CacheLease) -> Result<(), CacheError> {
        if self.closed {
            return Err(CacheError::Closed);
        }
        let identity = Rc::downgrade(&lease.identity);
        let index = self
            .leases
            .iter()
            .position(|active| Weak::ptr_eq(&active.identity, &identity))
            .ok_or(CacheError::StaleLease)?;
        self.leases.remove(index);
        Ok(())
    }

    /// Registers one complete-byte fetch; callers perform bounded transport outside this
    /// object. No partial buffers are retained or exposed by the cache.
    pub fn fetch(&mut self, key: &CacheKey) -> Result<FetchToken, CacheError> {
        self.check_current(key)?;
        if key.bytes.byte_len > self.limits.max_bytes as u64 {
            return Err(CacheError::ByteCapacity);
        }
        if self.pending.iter().any(|pending| &pending.key == key) {
            return Err(CacheError::AlreadyPending);
        }
        if self.pending.len() >= self.limits.max_pending {
            return Err(CacheError::PendingCapacity);
        }
        let token = FetchToken {
            identity: Rc::new(()),
            scope: self.scope,
            key: key.clone(),
        };
        self.pending.push(token.clone());
        Ok(token)
    }

    /// Consumes a terminal complete result. Length and SHA-256 of exact bytes precede
    /// cache publication. Every failure drops supplied bytes; stale results cannot clear
    /// replacement work. Incoming allocation capacity is bounded as well as byte length.
    pub fn complete(&mut self, token: &FetchToken, bytes: Vec<u8>) -> Result<(), CacheError> {
        if self.closed {
            return Err(CacheError::Closed);
        }
        if token.scope != self.scope {
            return Err(CacheError::StaleFetch);
        }
        let index = self
            .pending
            .iter()
            .position(|pending| Rc::ptr_eq(&pending.identity, &token.identity))
            .ok_or(CacheError::StaleFetch)?;
        self.pending.remove(index);
        self.check_current(&token.key)?;
        if bytes.capacity() > self.limits.max_bytes {
            return Err(CacheError::ByteCapacity);
        }
        if bytes.len() as u64 != token.key.bytes.byte_len {
            return Err(CacheError::Incomplete);
        }
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        if digest != token.key.bytes.sha256 {
            return Err(CacheError::HashMismatch);
        }
        // An exact immutable-key refresh retains consumers; bytes are independently verified.
        if let Some(index) = self.entries.iter().position(|entry| entry.key == token.key)
            && let Some(entry) = self.entries.remove(index)
        {
            self.resident_bytes -= entry.bytes.len();
        }
        while self.entries.len() >= self.limits.max_assets
            || bytes.len() > self.limits.max_bytes - self.resident_bytes
        {
            if let Some(entry) = self.entries.pop_front() {
                self.resident_bytes -= entry.bytes.len();
                self.leases.retain(|lease| lease.key != entry.key);
            }
        }
        self.resident_bytes += bytes.len();
        self.entries.push_back(Entry {
            key: token.key.clone(),
            bytes: bytes.into_boxed_slice(),
        });
        Ok(())
    }

    /// Terminal transport failure/cancellation fences the token before any late completion.
    pub fn cancel(&mut self, token: &FetchToken) -> Result<(), CacheError> {
        if self.closed {
            return Err(CacheError::Closed);
        }
        if token.scope != self.scope {
            return Err(CacheError::StaleFetch);
        }
        let index = self
            .pending
            .iter()
            .position(|pending| Rc::ptr_eq(&pending.identity, &token.identity))
            .ok_or(CacheError::StaleFetch)?;
        self.pending.remove(index);
        Ok(())
    }

    /// Discards this cache's matching resource and fetches without changing server access.
    /// Another cache with the same key retains its own bytes and fetch ownership.
    pub fn release(&mut self, key: &CacheKey) -> Result<(), CacheError> {
        if self.closed {
            return Err(CacheError::Closed);
        }
        self.pending.retain(|pending| &pending.key != key);
        self.remove_entry(key);
        Ok(())
    }

    /// Idempotent teardown: releases every owned allocation and permanently closes admission.
    pub fn dispose(&mut self) {
        self.pending = Vec::new();
        self.leases = Vec::new();
        self.entries = VecDeque::new();
        self.references = Box::default();
        self.current = None;
        self.resident_bytes = 0;
        self.closed = true;
    }

    /// Returns the canonical owner scope; this grants no source authorization.
    pub fn scope(&self) -> CacheScope {
        self.scope
    }

    pub fn resident_bytes(&self) -> usize {
        self.resident_bytes
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub fn lease_count(&self) -> usize {
        self.leases
            .iter()
            .filter(|lease| lease.identity.strong_count() != 0)
            .count()
    }

    fn check_scope(&self, scope: CacheScope) -> Result<(), CacheError> {
        if self.closed {
            return Err(CacheError::Closed);
        }
        if self.scope != scope {
            return Err(CacheError::WrongScope);
        }
        Ok(())
    }

    fn check_current(&self, key: &CacheKey) -> Result<(), CacheError> {
        if self.closed {
            return Err(CacheError::Closed);
        }
        if !self.references.contains(key) {
            return Err(CacheError::NotCurrent);
        }
        Ok(())
    }

    fn remove_entry(&mut self, key: &CacheKey) {
        self.leases.retain(|lease| &lease.key != key);
        if let Some(index) = self.entries.iter().position(|entry| &entry.key == key)
            && let Some(entry) = self.entries.remove(index)
        {
            self.resident_bytes -= entry.bytes.len();
        }
    }
}
