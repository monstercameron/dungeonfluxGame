//! Private executable D01 design contract. This fixture owns synthetic credential,
//! membership, binding and projection truth; it is not production authentication
//! or a generated Resume service. Client IDs/stamps only correlate admitted data.
use df_client::{
    connection::ConnectionGeneration,
    connection_views::{ConnectionViewAcceptance, ConnectionViews},
    revisions::ViewAcceptance,
};
use df_types::{ClientBindingId, MemberId, SessionId, SessionRevision};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Role {
    Player,
    Display,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MemberReference {
    pub(super) session: SessionId,
    pub(super) member: MemberId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MemberState {
    pub(super) reference: MemberReference,
    pub(super) role: Role,
    pub(super) ready: bool,
}

// A synthetic lookup key, deliberately absent from observations and diagnostics.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct FixtureCredential(pub(super) u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Refusal {
    RecoveryRequired,
    WrongIdentity,
    WrongRole,
    InactiveBinding,
    UnavailableConnection,
    GenerationExhausted,
    SnapshotRequired,
    RevisionRegressed,
    PresentationEpochRegressed,
    ViewSequenceNotIncreasing,
    Consumer(ConnectionViewAcceptance),
}

#[derive(Default, Debug, PartialEq, Eq)]
pub(super) struct Observations {
    pub(super) last_resume: Option<MemberReference>,
    pub(super) resumes: u32,
    pub(super) binds: u32,
    pub(super) joins: u32,
    pub(super) allocations: u32,
    pub(super) publications: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PermittedPayload {
    Player { own_clue: &'static str },
    Display { public_story: &'static str },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Stamp {
    pub(super) binding_generation: u64,
    pub(super) revision: SessionRevision,
    pub(super) presentation_epoch: u64,
    pub(super) view_sequence: u64,
    pub(super) full_snapshot: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Snapshot {
    pub(super) member: MemberState,
    pub(super) stamp: Stamp,
    pub(super) payload: PermittedPayload,
}

/// One pre-existing fixture member, independently looked up by its credential.
/// Join is an explicit positive control; Resume and Bind cannot allocate seats.
/// Projection checks current fixture access before constructing a payload.
pub(super) struct FixtureAuthority {
    pub(super) member: MemberState,
    pub(super) binding: ClientBindingId,
    pub(super) generation: u64,
    pub(super) credential_live: bool,
    pub(super) reply_override: Option<MemberState>,
    pub(super) private_clue: &'static str,
    pub(super) observations: Observations,
    pub(super) seats: u32,
    credential: FixtureCredential,
    resumed: bool,
}

impl FixtureAuthority {
    pub(super) fn new(
        member: MemberState,
        binding: ClientBindingId,
        credential: FixtureCredential,
    ) -> Self {
        Self {
            member,
            binding,
            generation: 7,
            credential_live: true,
            reply_override: None,
            private_clue: "synthetic own-member clue",
            observations: Observations::default(),
            seats: 1,
            credential,
            resumed: false,
        }
    }

    fn permitted(&self, reference: MemberReference, credential: FixtureCredential) -> bool {
        self.credential_live && credential == self.credential && reference == self.member.reference
    }

    fn resume(
        &mut self,
        reference: MemberReference,
        credential: FixtureCredential,
    ) -> Result<MemberState, Refusal> {
        self.observations.resumes += 1;
        self.observations.last_resume = Some(reference);
        self.resumed = self.permitted(reference, credential);
        if !self.resumed {
            return Err(Refusal::RecoveryRequired);
        }
        Ok(self.reply_override.unwrap_or(self.member))
    }

    fn bind(&mut self, reference: MemberReference) -> Result<(ClientBindingId, u64), Refusal> {
        self.observations.binds += 1;
        if !self.resumed || reference != self.member.reference || !self.credential_live {
            return Err(Refusal::RecoveryRequired);
        }
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or(Refusal::GenerationExhausted)?;
        Ok((self.binding, self.generation))
    }

    pub(super) fn explicit_join_control(&mut self) {
        self.observations.joins += 1;
        self.observations.allocations += 1;
        self.seats += 1;
    }

    pub(super) fn snapshot(
        &mut self,
        reference: MemberReference,
        role: Role,
        credential: FixtureCredential,
        stamp: Stamp,
    ) -> Result<Snapshot, Refusal> {
        if !self.permitted(reference, credential) {
            return Err(Refusal::RecoveryRequired);
        }
        if role != self.member.role {
            return Err(Refusal::WrongRole);
        }
        let payload = match role {
            Role::Player => PermittedPayload::Player {
                own_clue: self.private_clue,
            },
            Role::Display => PermittedPayload::Display {
                public_story: "synthetic public story",
            },
        };
        self.observations.publications += 1;
        Ok(Snapshot {
            member: self.member,
            stamp,
            payload,
        })
    }
}

/// Uses actual client connection fencing and canonical durable ordering. The
/// separate private presentation cursor models the planned SnapshotStamp fields,
/// which are absent from current gameplay protobufs. ConnectionViews<()> retains
/// the durable watermark; equal durable revisions can advance only the validated
/// presentation cursor, without replacing or resetting canonical ViewStore state.
pub(super) struct ClientContract {
    pub(super) member: MemberState,
    pub(super) binding: ClientBindingId,
    pub(super) binding_generation: u64,
    views: ConnectionViews<()>,
    current: Option<Snapshot>,
    access_resumed: bool,
}

impl ClientContract {
    pub(super) fn new(authority: &FixtureAuthority, generation: ConnectionGeneration) -> Self {
        Self {
            member: authority.member,
            binding: authority.binding,
            binding_generation: authority.generation,
            views: ConnectionViews::new(authority.binding, generation),
            current: None,
            access_resumed: true,
        }
    }

    pub(super) fn disconnect(&mut self) {
        self.current = None;
        self.access_resumed = false;
    }

    pub(super) fn resume(
        &mut self,
        authority: &mut FixtureAuthority,
        credential: FixtureCredential,
        generation: ConnectionGeneration,
    ) -> Result<(), Refusal> {
        self.disconnect();
        if !generation.is_active() || self.views.generation().same_scope(&generation) {
            return Err(Refusal::UnavailableConnection);
        }
        let resumed = authority.resume(self.member.reference, credential)?;
        if resumed.reference != self.member.reference {
            return Err(Refusal::WrongIdentity);
        }
        if resumed.role != self.member.role {
            return Err(Refusal::WrongRole);
        }
        let (binding, binding_generation) = authority.bind(resumed.reference)?;
        if binding != self.binding {
            return Err(Refusal::InactiveBinding);
        }
        if !self.views.reconnect(generation) {
            return Err(Refusal::UnavailableConnection);
        }
        self.member = resumed;
        self.binding_generation = binding_generation;
        self.access_resumed = true;
        Ok(())
    }

    pub(super) fn accept(
        &mut self,
        generation: &ConnectionGeneration,
        snapshot: Snapshot,
    ) -> Result<ViewAcceptance, Refusal> {
        if !self.access_resumed {
            return Err(Refusal::RecoveryRequired);
        }
        if !self.views.generation().same_scope(generation) || !generation.is_active() {
            return Err(Refusal::Consumer(self.views.accept(
                generation,
                self.binding,
                snapshot.stamp.revision,
                (),
            )));
        }
        if snapshot.member.reference != self.member.reference {
            return Err(Refusal::WrongIdentity);
        }
        if snapshot.member.role != self.member.role {
            return Err(Refusal::WrongRole);
        }
        let incoming = snapshot.stamp;
        if incoming.binding_generation != self.binding_generation {
            return Err(Refusal::InactiveBinding);
        }
        if self
            .watermark()
            .is_some_and(|revision| incoming.revision < revision)
        {
            return Err(Refusal::RevisionRegressed);
        }
        if let Some(previous) = self.current.map(|frame| frame.stamp) {
            if incoming.presentation_epoch < previous.presentation_epoch {
                return Err(Refusal::PresentationEpochRegressed);
            }
            if incoming.presentation_epoch > previous.presentation_epoch && !incoming.full_snapshot
            {
                return Err(Refusal::SnapshotRequired);
            }
            if incoming.presentation_epoch == previous.presentation_epoch
                && incoming.view_sequence <= previous.view_sequence
            {
                return Err(Refusal::ViewSequenceNotIncreasing);
            }
        } else if !incoming.full_snapshot {
            return Err(Refusal::SnapshotRequired);
        }
        match self
            .views
            .accept(generation, self.binding, incoming.revision, ())
        {
            ConnectionViewAcceptance::View(
                admission @ (ViewAcceptance::Applied | ViewAcceptance::Duplicate { .. }),
            ) => {
                self.current = Some(snapshot);
                Ok(admission)
            }
            rejected => Err(Refusal::Consumer(rejected)),
        }
    }

    pub(super) fn current(&self) -> Option<&Snapshot> {
        self.current.as_ref()
    }

    pub(super) fn watermark(&self) -> Option<SessionRevision> {
        self.views.current().map(|(revision, ())| revision)
    }
}
