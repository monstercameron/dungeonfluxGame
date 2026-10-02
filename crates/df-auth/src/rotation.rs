use df_types::{ClientBindingId, SessionId};

/// A single credential's fence, owned by its authoritative credential record.
/// Possessing this value does not authenticate a principal or grant an audience.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialFence<G> {
    generation: G,
    active: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RotationRefusal {
    Revoked,
    NonIncreasingGeneration,
}

impl<G: Ord> CredentialFence<G> {
    pub fn new(generation: G) -> Self {
        Self {
            generation,
            active: true,
        }
    }

    pub fn generation(&self) -> &G {
        &self.generation
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Advance the authoritative generation before accepting a replacement credential.
    /// The credential owner commits this transition with its issue/result records.
    pub fn rotate(&mut self, generation: G) -> Result<(), RotationRefusal> {
        if !self.active {
            return Err(RotationRefusal::Revoked);
        }
        if generation <= self.generation {
            return Err(RotationRefusal::NonIncreasingGeneration);
        }
        self.generation = generation;
        Ok(())
    }

    /// Fence all future publications, including tickets for the current generation.
    /// Already committed gameplay decisions and delivered bytes are unaffected.
    pub fn revoke(&mut self) {
        self.active = false;
    }
}

/// Nonsecret scope captured by the authorized subscription owner.
/// Caller-owned principal/credential identities are not replaced by trace IDs.
#[derive(Clone, Eq, PartialEq)]
pub struct PublicationScope<P, C, U> {
    pub audience: U,
    pub principal: P,
    pub credential: C,
    pub session: SessionId,
    pub binding: ClientBindingId,
}

/// A subscription snapshot that always requires current authority at publication.
/// Constructing or retaining a lease grants no continuing access.
pub struct PublicationLease<A: PublicationAuthority> {
    scope: PublicationScope<A::Principal, A::Credential, A::Audience>,
    credential_generation: A::CredentialGeneration,
    binding_generation: A::BindingGeneration,
    audience_version: A::AudienceVersion,
}

impl<A: PublicationAuthority> PublicationLease<A> {
    pub fn new(
        scope: PublicationScope<A::Principal, A::Credential, A::Audience>,
        credential_generation: A::CredentialGeneration,
        binding_generation: A::BindingGeneration,
        audience_version: A::AudienceVersion,
    ) -> Self {
        Self {
            scope,
            credential_generation,
            binding_generation,
            audience_version,
        }
    }

    pub fn scope(&self) -> &PublicationScope<A::Principal, A::Credential, A::Audience> {
        &self.scope
    }
}

/// Trusted current facts borrowed while their owner serializes publication.
/// Audience validity includes current membership, requested permission and expiry;
/// a payer/tenant grant or a cached membership capability cannot set it to true.
pub struct CurrentPublicationState<'a, A: PublicationAuthority> {
    pub scope: &'a PublicationScope<A::Principal, A::Credential, A::Audience>,
    pub credential: &'a CredentialFence<A::CredentialGeneration>,
    pub binding_generation: &'a A::BindingGeneration,
    pub binding_active: bool,
    pub credential_unexpired: bool,
    pub binding_unexpired: bool,
    pub audience_version: &'a A::AudienceVersion,
    pub audience_authorized: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationRefusal {
    MissingAuthority,
    ScopeChanged,
    CredentialRevoked,
    CredentialRotated,
    CredentialExpired,
    BindingRevoked,
    BindingReplaced,
    BindingExpired,
    AudienceDenied,
    AudienceChanged,
}

/// Authority boundary for a single synchronous publication.
/// Implementations serialize credential, binding and audience changes with the
/// callback, supply current facts, and invoke it exactly once. They must not use
/// detached/cached facts or unlock between validation and the callback's return.
/// The callback must not await, reenter this owner, or enqueue unguarded later I/O.
/// Asynchronous transfers call the boundary again for each admitted byte chunk.
pub trait PublicationAuthority: Sized {
    type Principal: Eq;
    type Credential: Eq;
    type Audience: Eq;
    type CredentialGeneration: Ord;
    type BindingGeneration: Eq;
    type AudienceVersion: Eq;
    type Error;

    fn with_current<R>(
        &mut self,
        scope: &PublicationScope<Self::Principal, Self::Credential, Self::Audience>,
        publish: impl FnOnce(Option<CurrentPublicationState<'_, Self>>) -> R,
    ) -> Result<R, Self::Error>;
}

#[derive(Debug, Eq, PartialEq)]
pub enum PublicationError<A, P> {
    Authority(A),
    Refused(PublicationRefusal),
    Publisher(P),
}

/// Produce and synchronously publish bytes only inside the current authority fence.
/// Refusal never calls the producer, so unauthorized data is not serialized or sent.
/// A publisher failure remains distinct from an authorization or source failure.
pub fn publish_current<A: PublicationAuthority, R, E>(
    authority: &mut A,
    lease: &PublicationLease<A>,
    publish: impl FnOnce() -> Result<R, E>,
) -> Result<R, PublicationError<A::Error, E>> {
    authority
        .with_current(lease.scope(), |current| {
            let current = current.ok_or(PublicationError::Refused(
                PublicationRefusal::MissingAuthority,
            ))?;
            let refusal = if current.scope != lease.scope() {
                Some(PublicationRefusal::ScopeChanged)
            } else if !current.credential.is_active() {
                Some(PublicationRefusal::CredentialRevoked)
            } else if !current.credential_unexpired {
                Some(PublicationRefusal::CredentialExpired)
            } else if current.credential.generation() != &lease.credential_generation {
                Some(PublicationRefusal::CredentialRotated)
            } else if !current.binding_active {
                Some(PublicationRefusal::BindingRevoked)
            } else if !current.binding_unexpired {
                Some(PublicationRefusal::BindingExpired)
            } else if current.binding_generation != &lease.binding_generation {
                Some(PublicationRefusal::BindingReplaced)
            } else if !current.audience_authorized {
                Some(PublicationRefusal::AudienceDenied)
            } else if current.audience_version != &lease.audience_version {
                Some(PublicationRefusal::AudienceChanged)
            } else {
                None
            };
            if let Some(refusal) = refusal {
                return Err(PublicationError::Refused(refusal));
            }
            publish().map_err(PublicationError::Publisher)
        })
        .map_err(PublicationError::Authority)?
}
