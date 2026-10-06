use crate::SceneOwner;
use df_types::{RevisionLabel, SessionRevision};
use std::collections::VecDeque;
use std::rc::Rc;

/// Actual prepared surface, with the canonical byte-cache key retained unchanged.
/// The current lease/access guard must fail closed; labels never grant access.
pub trait RendererResource {
    type Key: Clone + Eq;
    fn key(&self) -> &Self::Key;
    fn decoded_bytes(&self) -> usize;
    fn is_current(&self) -> bool;
    fn retire(self);
}

struct Owned<R: RendererResource>(Option<R>);

impl<R: RendererResource> Drop for Owned<R> {
    fn drop(&mut self) {
        if let Some(resource) = self.0.take() {
            resource.retire();
        }
    }
}

/// Distinct operation identity remains unique while any late callback holds a clone.
#[derive(Clone, Debug)]
pub struct DecodeToken<K> {
    identity: Rc<()>,
    key: K,
}

impl<K> DecodeToken<K> {
    pub(crate) fn same_operation(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.identity, &other.identity)
    }

    pub fn key(&self) -> &K {
        &self.key
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceLimits {
    pub max_references: usize,
    pub max_resident: usize,
    pub max_pending: usize,
    pub max_decoded_bytes: usize,
    pub max_work_bytes: usize,
}

/// Work includes output surface plus every retained input/copy/temporary allocation.
/// Resident decoded surfaces and in-flight work have separate finite byte budgets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecodeBudget {
    pub decoded_bytes: usize,
    pub work_bytes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceError {
    InvalidLimits,
    Closed,
    WrongOwner,
    SceneAlreadyInitialized,
    StaleRevision,
    ReferenceCapacity,
    DuplicateReference,
    NotCurrent,
    AlreadyResident,
    AlreadyPending,
    PendingCapacity,
    ByteCapacity,
    StaleDecode,
    Cancelled,
    RevokedResource,
    WrongResource,
    ReservationExceeded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceReadiness {
    Missing,
    Pending,
    Resident,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkOutcome {
    Failed,
    Cancelled,
}

struct Pending<K> {
    token: DecodeToken<K>,
    budget: DecodeBudget,
    cancelled: bool,
    abort: Option<Box<dyn FnOnce()>>,
}

impl<K> Pending<K> {
    fn cancel(&mut self) {
        self.cancelled = true;
        if let Some(abort) = self.abort.take() {
            abort();
        }
    }
}

struct Resident<R: RendererResource> {
    key: R::Key,
    bytes: usize,
    resource: Owned<R>,
}

/// One owner, immutable decoder configuration, complete current references, and bounded
/// decoded surfaces. The caller's canonical key must have bounded metadata/clones.
///
/// Cancellation invalidates publication immediately but DOES NOT free a work reservation.
/// Only an actual terminal completion/failure removes it. This prevents abort requests
/// from admitting uncounted concurrent replacement decodes. The browser future retains
/// this owner until its terminal callback; it must never detach preparation work.
pub struct ResourceCache<R: RendererResource> {
    owner: SceneOwner,
    limits: ResourceLimits,
    preparation_revision: RevisionLabel,
    revision: Option<SessionRevision>,
    scene: Option<RevisionLabel>,
    references: Box<[R::Key]>,
    pending: Vec<Pending<R::Key>>,
    resident: VecDeque<Resident<R>>,
    resident_bytes: usize,
    work_bytes: usize,
    closed: bool,
}

impl<R: RendererResource> ResourceCache<R> {
    pub fn new(
        owner: SceneOwner,
        limits: ResourceLimits,
        preparation_revision: RevisionLabel,
    ) -> Result<Self, ResourceError> {
        if limits.max_references == 0
            || limits.max_resident == 0
            || limits.max_pending == 0
            || limits.max_decoded_bytes == 0
            || limits.max_work_bytes == 0
        {
            return Err(ResourceError::InvalidLimits);
        }
        Ok(Self {
            owner,
            limits,
            preparation_revision,
            revision: None,
            scene: None,
            references: Box::default(),
            pending: Vec::new(),
            resident: VecDeque::new(),
            resident_bytes: 0,
            work_bytes: 0,
            closed: false,
        })
    }

    pub fn validate_scene(
        &self,
        owner: SceneOwner,
        revision: SessionRevision,
        references: &[R::Key],
    ) -> Result<(), ResourceError> {
        self.check_open()?;
        if owner != self.owner {
            return Err(ResourceError::WrongOwner);
        }
        if self.revision.is_some_and(|current| revision <= current) {
            return Err(ResourceError::StaleRevision);
        }
        if references.len() > self.limits.max_references {
            return Err(ResourceError::ReferenceCapacity);
        }
        for (index, key) in references.iter().enumerate() {
            if references.iter().take(index).any(|other| other == key) {
                return Err(ResourceError::DuplicateReference);
            }
        }
        Ok(())
    }

    pub fn apply_scene(
        &mut self,
        owner: SceneOwner,
        revision: SessionRevision,
        scene: RevisionLabel,
        references: &[R::Key],
    ) -> Result<(), ResourceError> {
        self.apply_references(owner, revision, Some(scene), references)
    }

    /// Illustration references have no geometry or scene label. Scope and accepted
    /// revision still fence every resource, including real cancelled work reservations.
    pub fn apply_current(
        &mut self,
        owner: SceneOwner,
        revision: SessionRevision,
        references: &[R::Key],
    ) -> Result<(), ResourceError> {
        self.apply_references(owner, revision, None, references)
    }

    fn apply_references(
        &mut self,
        owner: SceneOwner,
        revision: SessionRevision,
        scene: Option<RevisionLabel>,
        references: &[R::Key],
    ) -> Result<(), ResourceError> {
        self.validate_scene(owner, revision, references)?;
        let same_generation = self.scene.as_ref() == scene.as_ref()
            && self
                .revision
                .is_some_and(|current| current.epoch() == revision.epoch());
        self.references = references.to_vec().into_boxed_slice();
        self.scene = scene;
        self.revision = Some(revision);
        for pending in &mut self.pending {
            if !same_generation || !self.references.contains(&pending.token.key) {
                pending.cancel();
            }
        }
        self.resident.retain(|entry| {
            same_generation
                && self.references.contains(&entry.key)
                && entry
                    .resource
                    .0
                    .as_ref()
                    .is_some_and(RendererResource::is_current)
        });
        self.resident_bytes = self.resident.iter().map(|entry| entry.bytes).sum();
        Ok(())
    }

    /// Failed admission preserves every previous resident entry, pending operation and byte.
    /// Caller creates bounded input/decoder work only AFTER successful admission.
    pub fn begin(
        &mut self,
        key: &R::Key,
        budget: DecodeBudget,
        abort: impl FnOnce() + 'static,
    ) -> Result<DecodeToken<R::Key>, ResourceError> {
        self.check_current(key)?;
        if self.pending.iter().any(|pending| &pending.token.key == key) {
            return Err(ResourceError::AlreadyPending);
        }
        if self.resident.iter().any(|entry| {
            &entry.key == key
                && entry
                    .resource
                    .0
                    .as_ref()
                    .is_some_and(RendererResource::is_current)
        }) {
            return Err(ResourceError::AlreadyResident);
        }
        if self.pending.len() >= self.limits.max_pending {
            return Err(ResourceError::PendingCapacity);
        }
        if budget.decoded_bytes == 0
            || budget.decoded_bytes > self.limits.max_decoded_bytes
            || budget.work_bytes < budget.decoded_bytes
            || budget.work_bytes > self.limits.max_work_bytes - self.work_bytes
        {
            return Err(ResourceError::ByteCapacity);
        }
        let token = DecodeToken {
            identity: Rc::new(()),
            key: key.clone(),
        };
        self.work_bytes += budget.work_bytes;
        self.pending.push(Pending {
            token: token.clone(),
            budget,
            cancelled: false,
            abort: Some(Box::new(abort)),
        });
        Ok(token)
    }

    /// Removes only this actual terminal operation. Every rejected incoming surface retires.
    /// Wrong key/lease/size cannot replace a valid previous cache entry or cause eviction.
    pub fn complete(
        &mut self,
        token: &DecodeToken<R::Key>,
        resource: R,
    ) -> Result<(), ResourceError> {
        let resource = Owned(Some(resource));
        let pending = self.take_terminal(token)?;
        if self.closed {
            return Err(ResourceError::Closed);
        }
        if pending.cancelled {
            return Err(ResourceError::Cancelled);
        }
        self.check_current(&token.key)?;
        let Some(surface) = resource.0.as_ref() else {
            return Err(ResourceError::StaleDecode);
        };
        if surface.key() != &token.key {
            return Err(ResourceError::WrongResource);
        }
        if !surface.is_current() {
            return Err(ResourceError::RevokedResource);
        }
        let bytes = surface.decoded_bytes();
        if bytes > pending.budget.decoded_bytes {
            return Err(ResourceError::ReservationExceeded);
        }
        self.remove_resident(&token.key);
        while self.resident.len() >= self.limits.max_resident
            || bytes > self.limits.max_decoded_bytes - self.resident_bytes
        {
            if let Some(entry) = self.resident.pop_front() {
                self.resident_bytes -= entry.bytes;
            }
        }
        self.resident_bytes += bytes;
        self.resident.push_back(Resident {
            key: token.key.clone(),
            bytes,
            resource,
        });
        Ok(())
    }

    /// Terminal decode failure or confirmed aborted completion, never an abort request alone.
    pub fn finish_failed(
        &mut self,
        token: &DecodeToken<R::Key>,
    ) -> Result<WorkOutcome, ResourceError> {
        let pending = self.take_terminal(token)?;
        Ok(if pending.cancelled || self.closed {
            WorkOutcome::Cancelled
        } else {
            WorkOutcome::Failed
        })
    }

    pub fn cancel(&mut self, token: &DecodeToken<R::Key>) -> Result<(), ResourceError> {
        let index = self.pending_index(token)?;
        if let Some(pending) = self.pending.get_mut(index) {
            pending.cancel();
        }
        Ok(())
    }

    pub fn get(&mut self, key: &R::Key) -> Result<Option<&R>, ResourceError> {
        self.check_current(key)?;
        let Some(index) = self.resident.iter().position(|entry| &entry.key == key) else {
            return Ok(None);
        };
        if !self
            .resident
            .get(index)
            .and_then(|entry| entry.resource.0.as_ref())
            .is_some_and(RendererResource::is_current)
        {
            self.remove_resident(key);
            return Err(ResourceError::RevokedResource);
        }
        if let Some(entry) = self.resident.remove(index) {
            self.resident.push_back(entry);
        }
        Ok(self
            .resident
            .back()
            .and_then(|entry| entry.resource.0.as_ref()))
    }

    pub fn readiness(&self, key: &R::Key) -> Result<ResourceReadiness, ResourceError> {
        self.check_current(key)?;
        if self.resident.iter().any(|entry| {
            &entry.key == key
                && entry
                    .resource
                    .0
                    .as_ref()
                    .is_some_and(RendererResource::is_current)
        }) {
            return Ok(ResourceReadiness::Resident);
        }
        if self
            .pending
            .iter()
            .any(|pending| &pending.token.key == key && !pending.cancelled)
        {
            return Ok(ResourceReadiness::Pending);
        }
        Ok(ResourceReadiness::Missing)
    }

    pub fn release(&mut self, key: &R::Key) -> Result<(), ResourceError> {
        self.check_open()?;
        for pending in &mut self.pending {
            if &pending.token.key == key {
                pending.cancel();
            }
        }
        self.remove_resident(key);
        Ok(())
    }

    /// Admission closes immediately. Cancelled in-flight work stays counted to its terminal
    /// callback, including callbacks arriving after disposal; none can publish a surface.
    pub fn dispose(&mut self) {
        self.closed = true;
        for pending in &mut self.pending {
            pending.cancel();
        }
        self.resident = VecDeque::new();
        self.references = Box::default();
        self.resident_bytes = 0;
    }

    pub fn decoded_bytes(&self) -> usize {
        self.resident_bytes
    }
    pub fn work_bytes(&self) -> usize {
        self.work_bytes
    }
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
    pub fn remaining_decoded_bytes(&self) -> usize {
        self.limits.max_decoded_bytes - self.resident_bytes
    }
    pub fn remaining_work_bytes(&self) -> usize {
        self.limits.max_work_bytes - self.work_bytes
    }
    pub fn preparation_revision(&self) -> &RevisionLabel {
        &self.preparation_revision
    }

    fn check_open(&self) -> Result<(), ResourceError> {
        if self.closed {
            return Err(ResourceError::Closed);
        }
        Ok(())
    }
    fn check_current(&self, key: &R::Key) -> Result<(), ResourceError> {
        self.check_open()?;
        if !self.references.contains(key) {
            return Err(ResourceError::NotCurrent);
        }
        Ok(())
    }
    fn pending_index(&self, token: &DecodeToken<R::Key>) -> Result<usize, ResourceError> {
        self.pending
            .iter()
            .position(|pending| Rc::ptr_eq(&pending.token.identity, &token.identity))
            .ok_or(ResourceError::StaleDecode)
    }
    fn take_terminal(
        &mut self,
        token: &DecodeToken<R::Key>,
    ) -> Result<Pending<R::Key>, ResourceError> {
        let index = self.pending_index(token)?;
        let mut pending = self.pending.remove(index);
        self.work_bytes -= pending.budget.work_bytes;
        pending.abort = None;
        Ok(pending)
    }
    fn remove_resident(&mut self, key: &R::Key) {
        if let Some(index) = self.resident.iter().position(|entry| &entry.key == key)
            && let Some(entry) = self.resident.remove(index)
        {
            self.resident_bytes -= entry.bytes;
        }
    }
}

impl<R: RendererResource> Drop for ResourceCache<R> {
    fn drop(&mut self) {
        self.dispose();
    }
}
