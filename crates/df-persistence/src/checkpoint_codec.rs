use std::mem::size_of;

use df_model::checkpoint::*;
use df_types::{
    BuildIdentity, BuildRevision, ClientBindingId, LocaleTag, MemberId, OperationId, RecoveryEpoch,
    RevisionLabel, RunId, SessionId, SessionRevision,
};

// Version 2 preserves the current canonical optional character appearance.
// Version 1 is incompatible and refuses rather than synthesizing absent cosmetics.
const CODEC_VERSION: u16 = 2;
pub(crate) const STORAGE_CODEC_VERSION: i32 = CODEC_VERSION as i32;
const MAGIC: &[u8; 4] = b"DFCP";

// PostgreSQL metadata describes this private byte format, not the canonical
// model schema. Refuse legacy/mismatched metadata before decoding any payload.
pub(crate) fn validate_storage_format(
    version: i32,
    magic: &[u8; 4],
    document: &[u8],
) -> Result<(), CodecError> {
    let header = document.get(..6).ok_or(CodecError::Truncated)?;
    if version != STORAGE_CODEC_VERSION
        || header.get(..4) != Some(magic.as_slice())
        || header.get(4..6) != Some(CODEC_VERSION.to_be_bytes().as_slice())
    {
        return Err(CodecError::UnsupportedCodec);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CodecError {
    Capacity,
    Truncated,
    InvalidValue,
    UnsupportedCodec,
    TrailingBytes,
    Checkpoint(CheckpointError),
}

/// Every allocation and collection is bounded before reading storage-controlled lengths.
#[derive(Clone, Copy)]
pub struct CodecLimits {
    pub maximum_document_bytes: usize,
    pub maximum_allocated_bytes: usize,
    pub maximum_collection_items: usize,
    pub maximum_text_bytes: usize,
}
impl CodecLimits {
    pub(crate) fn validate(self) -> Result<Self, CodecError> {
        if self.maximum_document_bytes == 0
            || self.maximum_allocated_bytes == 0
            || self.maximum_collection_items == 0
            || self.maximum_text_bytes == 0
        {
            return Err(CodecError::Capacity);
        }
        Ok(self)
    }
}
struct Encoder {
    bytes: Vec<u8>,
    maximum: usize,
    allocation_remaining: usize,
    items_remaining: usize,
    maximum_text_bytes: usize,
}
impl Encoder {
    fn allocate(&mut self, count: usize) -> Result<(), CodecError> {
        self.allocation_remaining = self
            .allocation_remaining
            .checked_sub(count)
            .ok_or(CodecError::Capacity)?;
        Ok(())
    }
    fn put(&mut self, bytes: &[u8]) -> Result<(), CodecError> {
        let next = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or(CodecError::Capacity)?;
        if next > self.maximum {
            return Err(CodecError::Capacity);
        }
        if next > self.bytes.capacity() {
            let target = self
                .bytes
                .capacity()
                .max(64)
                .checked_mul(2)
                .unwrap_or(self.maximum)
                .min(self.maximum)
                .max(next);
            self.bytes
                .try_reserve_exact(
                    target
                        .checked_sub(self.bytes.len())
                        .ok_or(CodecError::Capacity)?,
                )
                .map_err(|_| CodecError::Capacity)?;
            if self.bytes.capacity() > self.maximum {
                return Err(CodecError::Capacity);
            }
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
}
struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
    allocation_remaining: usize,
    items_remaining: usize,
    maximum_text_bytes: usize,
}
impl<'a> Decoder<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], CodecError> {
        let end = self.offset.checked_add(count).ok_or(CodecError::Capacity)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(CodecError::Truncated)?;
        self.offset = end;
        Ok(bytes)
    }
    fn allocate(&mut self, count: usize) -> Result<(), CodecError> {
        self.allocation_remaining = self
            .allocation_remaining
            .checked_sub(count)
            .ok_or(CodecError::Capacity)?;
        Ok(())
    }
}
trait Wire: Sized {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError>;
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError>;
}
macro_rules! integer_wire {
    ($($integer:ty),+ $(,)?) => {$ (
        impl Wire for $integer {
            fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
                output.put(&self.to_be_bytes())
            }
            fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
                let bytes = input.take(size_of::<Self>())?.try_into()
                    .map_err(|_| CodecError::Truncated)?;
                Ok(Self::from_be_bytes(bytes))
            }
        }
    )+};
}
integer_wire!(u8, u16, u32, u64, i64);
impl Wire for bool {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        u8::from(*self).write(output)
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        match u8::read(input)? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(CodecError::InvalidValue),
        }
    }
}
impl<const N: usize> Wire for [u8; N] {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        output.put(self)
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        input.take(N)?.try_into().map_err(|_| CodecError::Truncated)
    }
}
impl<T: Wire> Wire for Vec<T> {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        output.items_remaining = output
            .items_remaining
            .checked_sub(self.len())
            .ok_or(CodecError::Capacity)?;
        output.allocate(
            self.len()
                .checked_mul(size_of::<T>())
                .ok_or(CodecError::Capacity)?,
        )?;
        u32::try_from(self.len())
            .map_err(|_| CodecError::Capacity)?
            .write(output)?;
        for item in self {
            item.write(output)?;
        }
        Ok(())
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        let count = usize::try_from(u32::read(input)?).map_err(|_| CodecError::Capacity)?;
        input.items_remaining = input
            .items_remaining
            .checked_sub(count)
            .ok_or(CodecError::Capacity)?;
        let bytes = count
            .checked_mul(size_of::<T>())
            .ok_or(CodecError::Capacity)?;
        input.allocate(bytes)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| CodecError::Capacity)?;
        let actual_bytes = values
            .capacity()
            .checked_mul(size_of::<T>())
            .ok_or(CodecError::Capacity)?;
        input.allocate(
            actual_bytes
                .checked_sub(bytes)
                .ok_or(CodecError::Capacity)?,
        )?;
        for _ in 0..count {
            values.push(T::read(input)?);
        }
        Ok(values)
    }
}
impl<T: Wire> Wire for Option<T> {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        match self {
            None => 0_u8.write(output),
            Some(value) => {
                1_u8.write(output)?;
                value.write(output)
            }
        }
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        match u8::read(input)? {
            0 => Ok(None),
            1 => Ok(Some(T::read(input)?)),
            _ => Err(CodecError::InvalidValue),
        }
    }
}
impl<A: Wire, B: Wire> Wire for (A, B) {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        self.0.write(output)?;
        self.1.write(output)
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        Ok((A::read(input)?, B::read(input)?))
    }
}
impl Wire for String {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        if self.len() > output.maximum_text_bytes {
            return Err(CodecError::Capacity);
        }
        output.allocate(self.len())?;
        u32::try_from(self.len())
            .map_err(|_| CodecError::Capacity)?
            .write(output)?;
        output.put(self.as_bytes())
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        let count = usize::try_from(u32::read(input)?).map_err(|_| CodecError::Capacity)?;
        if count > input.maximum_text_bytes {
            return Err(CodecError::Capacity);
        }
        input.allocate(count)?;
        let value =
            std::str::from_utf8(input.take(count)?).map_err(|_| CodecError::InvalidValue)?;
        let mut text = String::new();
        text.try_reserve_exact(count)
            .map_err(|_| CodecError::Capacity)?;
        input.allocate(
            text.capacity()
                .checked_sub(count)
                .ok_or(CodecError::Capacity)?,
        )?;
        text.push_str(value);
        Ok(text)
    }
}
macro_rules! identity_wire {
    ($($identity:ty),+ $(,)?) => {$ (
        impl Wire for $identity {
            fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
                output.put(self.as_bytes())
            }
            fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
                Self::from_bytes(input.take(16)?).map_err(|_| CodecError::InvalidValue)
            }
        }
    )+};
}
identity_wire!(
    SessionId,
    MemberId,
    RunId,
    OperationId,
    ClientBindingId,
    EntityId,
    FactId,
    ResolutionId,
    WindowId,
    JobId,
    TimerId,
    EffectId,
    RecordId
);
impl Wire for RevisionLabel {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        let bytes = self.as_str().as_bytes();
        output.allocate(bytes.len())?;
        u16::try_from(bytes.len())
            .map_err(|_| CodecError::Capacity)?
            .write(output)?;
        output.put(bytes)
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        let count = usize::from(u16::read(input)?);
        if count == 0 || count > 128 {
            return Err(CodecError::InvalidValue);
        }
        input.allocate(count)?;
        let text = std::str::from_utf8(input.take(count)?).map_err(|_| CodecError::InvalidValue)?;
        let label = Self::new(Some(text)).map_err(|_| CodecError::InvalidValue)?;
        input.allocate(
            label
                .retained_heap_bytes()
                .checked_sub(count)
                .ok_or(CodecError::Capacity)?,
        )?;
        Ok(label)
    }
}
impl Wire for LocaleTag {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        let bytes = self.as_str().as_bytes();
        output.allocate(bytes.len())?;
        u16::try_from(bytes.len())
            .map_err(|_| CodecError::Capacity)?
            .write(output)?;
        output.put(bytes)
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        let count = usize::from(u16::read(input)?);
        if count == 0 || count > Self::MAX_BYTES {
            return Err(CodecError::InvalidValue);
        }
        input.allocate(count)?;
        let text = std::str::from_utf8(input.take(count)?).map_err(|_| CodecError::InvalidValue)?;
        let locale = Self::parse(text).map_err(|_| CodecError::InvalidValue)?;
        if locale.as_str() != text {
            return Err(CodecError::InvalidValue);
        }
        input.allocate(
            locale
                .retained_heap_bytes()
                .checked_sub(count)
                .ok_or(CodecError::Capacity)?,
        )?;
        Ok(locale)
    }
}
impl Wire for RecoveryEpoch {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        self.get().write(output)
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        Self::new(u64::read(input)?).map_err(|_| CodecError::InvalidValue)
    }
}
impl Wire for SessionRevision {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        self.epoch().write(output)?;
        self.sequence().write(output)
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        let epoch = RecoveryEpoch::read(input)?;
        Ok(Self::new(epoch, u64::read(input)?))
    }
}
impl Wire for BuildIdentity {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        for kind in [
            BuildRevision::Source,
            BuildRevision::Native,
            BuildRevision::Wasm,
            BuildRevision::Configuration,
            BuildRevision::Content,
        ] {
            let label = self.revision(kind);
            output.allocate(label.as_str().len())?;
            label.write(output)?;
        }
        Ok(())
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        let source = RevisionLabel::read(input)?;
        let native = RevisionLabel::read(input)?;
        let wasm = RevisionLabel::read(input)?;
        let configuration = RevisionLabel::read(input)?;
        let content = RevisionLabel::read(input)?;
        // BuildIdentity owns copies: account for those allocations before constructing it.
        for label in [&source, &native, &wasm, &configuration, &content] {
            input.allocate(label.retained_heap_bytes())?;
        }
        Self::new(
            Some(source.as_str()),
            Some(native.as_str()),
            Some(wasm.as_str()),
            Some(configuration.as_str()),
            Some(content.as_str()),
        )
        .map_err(|_| CodecError::InvalidValue)
    }
}
impl Wire for ContentDigest {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        self.0.write(output)
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        Ok(Self(<[u8; 32]>::read(input)?))
    }
}
macro_rules! record_wire {
    ($record:ty { $($field:ident),* $(,)? }) => {
        impl Wire for $record {
            fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
                $(self.$field.write(output)?;)*
                Ok(())
            }
            fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
                Ok(Self { $($field: Wire::read(input)?,)* })
            }
        }
    };
}

pub(crate) fn encode_checkpoint(
    checkpoint: &Checkpoint,
    limits: CodecLimits,
) -> Result<Vec<u8>, CodecError> {
    let limits = limits.validate()?;
    let mut output = Encoder {
        bytes: Vec::new(),
        maximum: limits.maximum_document_bytes,
        allocation_remaining: limits.maximum_allocated_bytes,
        items_remaining: limits.maximum_collection_items,
        maximum_text_bytes: limits.maximum_text_bytes,
    };
    output.put(MAGIC)?;
    CODEC_VERSION.write(&mut output)?;
    checkpoint.schema().write(&mut output)?;
    checkpoint.basis().write(&mut output)?;
    checkpoint.pins().write(&mut output)?;
    checkpoint.state().write(&mut output)?;
    Ok(output.bytes)
}

/// Row keys/pins are trusted selected recovery basis; checkpoint bytes cannot supply authority.
pub(crate) fn decode_checkpoint(
    bytes: &[u8],
    expected: Basis,
    pins: &CheckpointPins,
    inventory: ReferenceInventory<'_>,
    checkpoint_limits: CheckpointLimits,
    codec_limits: CodecLimits,
) -> Result<Checkpoint, CodecError> {
    let limits = codec_limits.validate()?;
    if bytes.len() > limits.maximum_document_bytes {
        return Err(CodecError::Capacity);
    }
    let mut input = Decoder {
        bytes,
        offset: 0,
        allocation_remaining: limits.maximum_allocated_bytes,
        items_remaining: limits.maximum_collection_items,
        maximum_text_bytes: limits.maximum_text_bytes,
    };
    if input.take(MAGIC.len())? != MAGIC {
        return Err(CodecError::UnsupportedCodec);
    }
    if u16::read(&mut input)? != CODEC_VERSION {
        return Err(CodecError::UnsupportedCodec);
    }
    let schema = u16::read(&mut input)?;
    if schema != CHECKPOINT_SCHEMA {
        return Err(CodecError::Checkpoint(CheckpointError::UnsupportedSchema));
    }
    let basis = Basis::read(&mut input)?;
    let decoded_pins = CheckpointPins::read(&mut input)?;
    let state = GameState::read(&mut input)?;
    if input.offset != bytes.len() {
        return Err(CodecError::TrailingBytes);
    }
    let checkpoint = Checkpoint::new(
        schema,
        basis,
        decoded_pins,
        state,
        inventory,
        checkpoint_limits,
    )
    .map_err(CodecError::Checkpoint)?;
    checkpoint
        .validate_resume(expected, pins)
        .map_err(CodecError::Checkpoint)?;
    Ok(checkpoint)
}

// Explicit schema-1 mapping of accepted canonical fields; change only with codec migration review.
record_wire!(AcceptedChoice {
    participant,
    offer,
    selected,
    source
});
record_wire!(AcceptedDecision {
    operation,
    revision,
    facts,
    draws,
    effects,
    source_policy,
    semantic_output
});
record_wire!(ActiveEffect {
    id,
    source,
    origin,
    source_entity,
    targets,
    starts,
    expires,
    concentration_owner,
    choices
});
record_wire!(ActivityWindow {
    member,
    started,
    ends,
    observed_facts,
    spotlight_opt_in
});
record_wire!(ActualDraw {
    operation,
    ordinal,
    resolution,
    window,
    sides,
    value,
    source
});
record_wire!(AssetDemand {
    id,
    basis,
    key,
    priority,
    mode,
    expires,
    budget_reservation,
    maximum_bytes,
    policy
});
record_wire!(AssetDependency {
    asset,
    prerequisites
});
record_wire!(AssetJobState {
    job,
    demand,
    generation,
    state,
    published,
    dispatch
});
impl Wire for AssetKind {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        match self {
            Self::Image => 1_u16.write(output),
            Self::Audio => 2_u16.write(output),
            Self::Video => 3_u16.write(output),
            Self::TacticalGeometry => 4_u16.write(output),
        }
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        match u16::read(input)? {
            1 => Ok(Self::Image),
            2 => Ok(Self::Audio),
            3 => Ok(Self::Video),
            4 => Ok(Self::TacticalGeometry),
            _ => Err(CodecError::InvalidValue),
        }
    }
}
impl Wire for AssetLifecycle {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        match self {
            Self::Missing => 1_u16.write(output),
            Self::Queued => 2_u16.write(output),
            Self::Generating => 3_u16.write(output),
            Self::Ready => 4_u16.write(output),
            Self::Failed => 5_u16.write(output),
            Self::Cancelled => 6_u16.write(output),
            Self::Stale => 7_u16.write(output),
            Self::Superseded => 8_u16.write(output),
        }
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        match u16::read(input)? {
            1 => Ok(Self::Missing),
            2 => Ok(Self::Queued),
            3 => Ok(Self::Generating),
            4 => Ok(Self::Ready),
            5 => Ok(Self::Failed),
            6 => Ok(Self::Cancelled),
            7 => Ok(Self::Stale),
            8 => Ok(Self::Superseded),
            _ => Err(CodecError::InvalidValue),
        }
    }
}
record_wire!(AssetReference {
    key,
    digest,
    byte_length,
    kind
});
record_wire!(AssetRequestKey {
    schema,
    source,
    moment,
    identity,
    style,
    voice,
    provider,
    model,
    format,
    references,
    audience,
    parameters
});
record_wire!(AttributedClaim {
    id,
    holder,
    subject,
    claim,
    evidence,
    audience,
    source
});
impl Wire for AudienceScope {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        match self {
            Self::Shared => 1_u16.write(output),
            Self::Members(value) => {
                2_u16.write(output)?;
                value.write(output)
            }
            Self::Host => 3_u16.write(output),
        }
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        match u16::read(input)? {
            1 => Ok(Self::Shared),
            2 => Ok(Self::Members(Wire::read(input)?)),
            3 => Ok(Self::Host),
            _ => Err(CodecError::InvalidValue),
        }
    }
}
impl Wire for AudioDestination {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        match self {
            Self::PublicRoom(value) => {
                1_u16.write(output)?;
                value.write(output)
            }
            Self::PrivateListener { member, binding } => {
                2_u16.write(output)?;
                member.write(output)?;
                binding.write(output)?;
                Ok(())
            }
        }
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        match u16::read(input)? {
            1 => Ok(Self::PublicRoom(Wire::read(input)?)),
            2 => Ok(Self::PrivateListener {
                member: Wire::read(input)?,
                binding: Wire::read(input)?,
            }),
            _ => Err(CodecError::InvalidValue),
        }
    }
}
record_wire!(AudioOutputLease {
    id,
    destination,
    generation,
    audience,
    device_unlocked
});
record_wire!(AudioTopology {
    policy,
    outputs,
    captures
});
record_wire!(Basis {
    session,
    run,
    revision
});
impl Wire for BookendKind {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        match self {
            Self::Recap => 1_u16.write(output),
            Self::SpeculativeTrailer => 2_u16.write(output),
        }
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        match u16::read(input)? {
            1 => Ok(Self::Recap),
            2 => Ok(Self::SpeculativeTrailer),
            _ => Err(CodecError::InvalidValue),
        }
    }
}
record_wire!(BookendPlan {
    spec,
    selection,
    shots,
    captions,
    fallback
});
record_wire!(BookendSpec {
    id,
    basis,
    kind,
    from,
    through,
    audience,
    locale,
    policy,
    maximum_shots,
    maximum_duration_ticks,
    maximum_text_bytes,
    maximum_asset_bytes,
    expires,
    mode,
    budget_reservation
});
record_wire!(CanonicalPack {
    revision,
    digest,
    bible,
    identities
});
record_wire!(CaptureLease {
    id,
    member,
    binding,
    generation,
    permitted_offer,
    format
});
record_wire!(CatchUpCursor {
    last_processed,
    pending_events,
    policy,
    time
});
record_wire!(CharacterDraft {
    entity,
    member,
    ancestry,
    background,
    classes,
    choices,
    phase
});
record_wire!(CharacterHook {
    id,
    member,
    definition,
    source_facts,
    consent_generation,
    audience
});
record_wire!(CharacterState {
    entity,
    build,
    owner,
    choices
});
record_wire!(CheckpointPins {
    rules,
    content,
    build
});
record_wire!(ConsolidationCandidate {
    id,
    job,
    basis,
    episodes,
    summaries,
    source_digest,
    source_revision,
    access_generation,
    index_generation
});
record_wire!(ContentAdmission {
    candidate,
    result,
    definition,
    definition_digest,
    source,
    handler,
    ruleset,
    approval,
    disclosed_to,
    operation
});
record_wire!(ContentCandidate {
    id,
    basis,
    generator,
    model,
    policy,
    template,
    source,
    origin,
    audience,
    bounded_parameters
});
record_wire!(ContentPins {
    content,
    content_digest,
    package,
    package_digest
});
record_wire!(ContentReference { package, entry });
impl Wire for ContentValidation {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        match self {
            Self::StandardCompatible => 1_u16.write(output),
            Self::NeedsCustomApproval => 2_u16.write(output),
            Self::UnsupportedMechanic => 3_u16.write(output),
            Self::SourceGap => 4_u16.write(output),
            Self::Rejected => 5_u16.write(output),
        }
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        match u16::read(input)? {
            1 => Ok(Self::StandardCompatible),
            2 => Ok(Self::NeedsCustomApproval),
            3 => Ok(Self::UnsupportedMechanic),
            4 => Ok(Self::SourceGap),
            5 => Ok(Self::Rejected),
            _ => Err(CodecError::InvalidValue),
        }
    }
}
record_wire!(ContextualPrivateOffer {
    id,
    member,
    resolution,
    window,
    facts,
    policy,
    access_generation
});
record_wire!(ContinuityState {
    creation,
    simulation,
    catch_up,
    environment,
    travel,
    witnesses,
    rumors,
    journal,
    summaries,
    retrieval,
    retrieved,
    consolidation,
    npcs,
    hooks,
    arcs,
    remote,
    presence,
    audio,
    private_offers,
    knowledge_cues,
    moments,
    demands,
    asset_jobs,
    asset_dependencies,
    canonical_packs,
    shots,
    prefetch,
    scenes,
    item_origins,
    bookends,
    exports,
    critical_cues,
    content_candidates,
    content_admissions,
    recovery
});
record_wire!(ConversationState {
    id,
    participants,
    topic,
    accepted_facts
});
impl Wire for CreationPhase {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        match self {
            Self::Selecting => 1_u16.write(output),
            Self::AwaitingValidation => 2_u16.write(output),
            Self::Accepted => 3_u16.write(output),
        }
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        match u16::read(input)? {
            1 => Ok(Self::Selecting),
            2 => Ok(Self::AwaitingValidation),
            3 => Ok(Self::Accepted),
            _ => Err(CodecError::InvalidValue),
        }
    }
}
record_wire!(CriticalCueEligibility {
    id,
    fact,
    resolution,
    audience,
    ready_assets,
    policy,
    maximum_duration_ticks
});
impl Wire for DemandPriority {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        match self {
            Self::InteractionCritical => 1_u16.write(output),
            Self::SoonLikely => 2_u16.write(output),
            Self::Optional => 3_u16.write(output),
        }
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        match u16::read(input)? {
            1 => Ok(Self::InteractionCritical),
            2 => Ok(Self::SoonLikely),
            3 => Ok(Self::Optional),
            _ => Err(CodecError::InvalidValue),
        }
    }
}
record_wire!(DurableIntent {
    id,
    basis,
    operation,
    slot,
    kind,
    job,
    timer,
    generation,
    status,
    definition
});
impl Wire for DurableStatus {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        match self {
            Self::Pending => 1_u16.write(output),
            Self::Claimed => 2_u16.write(output),
            Self::SentUnknown => 3_u16.write(output),
            Self::Completed => 4_u16.write(output),
            Self::Failed => 5_u16.write(output),
            Self::Cancelled => 6_u16.write(output),
        }
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        match u16::read(input)? {
            1 => Ok(Self::Pending),
            2 => Ok(Self::Claimed),
            3 => Ok(Self::SentUnknown),
            4 => Ok(Self::Completed),
            5 => Ok(Self::Failed),
            6 => Ok(Self::Cancelled),
            _ => Err(CodecError::InvalidValue),
        }
    }
}
impl Wire for EffectKind {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        match self {
            Self::RunAi => 1_u16.write(output),
            Self::RunMedia => 2_u16.write(output),
            Self::LoadMemoryCandidates => 3_u16.write(output),
            Self::ArmTimer => 4_u16.write(output),
            Self::CancelJob => 5_u16.write(output),
            Self::CancelTimer => 6_u16.write(output),
            Self::PublishPresentation => 7_u16.write(output),
        }
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        match u16::read(input)? {
            1 => Ok(Self::RunAi),
            2 => Ok(Self::RunMedia),
            3 => Ok(Self::LoadMemoryCandidates),
            4 => Ok(Self::ArmTimer),
            5 => Ok(Self::CancelJob),
            6 => Ok(Self::CancelTimer),
            7 => Ok(Self::PublishPresentation),
            _ => Err(CodecError::InvalidValue),
        }
    }
}
record_wire!(EncounterState {
    id,
    definition,
    participants,
    turn_order,
    active_turn,
    objectives,
    combat_policy
});
record_wire!(CharacterAppearance {
    features,
    outfit,
    outfit_revision
});
record_wire!(EntityIdentityRevision {
    entity,
    revision,
    character_appearance,
    source_facts,
    appearances,
    voice,
    sound
});
record_wire!(EntitySimulation {
    entity,
    tier,
    policy,
    last_advanced
});
record_wire!(EnvironmentalState {
    location,
    definition,
    change_facts
});
impl Wire for ExecutionMode {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        match self {
            Self::Live => 1_u16.write(output),
            Self::PreparedOnly => 2_u16.write(output),
            Self::Replay => 3_u16.write(output),
        }
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        match u16::read(input)? {
            1 => Ok(Self::Live),
            2 => Ok(Self::PreparedOnly),
            3 => Ok(Self::Replay),
            _ => Err(CodecError::InvalidValue),
        }
    }
}
record_wire!(ExportGrant {
    id,
    bookend,
    recipients,
    selection,
    assets,
    rights,
    source_revision,
    access_generation,
    suppression_generation,
    expires,
    revoked,
    downloaded_copy_retraction_supported
});
record_wire!(FactSelection {
    facts,
    attributed_claims,
    audience
});
impl Wire for FactValue {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        match self {
            Self::EntityCreated { entity, definition } => {
                1_u16.write(output)?;
                entity.write(output)?;
                definition.write(output)?;
                Ok(())
            }
            Self::EntityMoved {
                entity,
                destination,
                position,
            } => {
                2_u16.write(output)?;
                entity.write(output)?;
                destination.write(output)?;
                position.write(output)?;
                Ok(())
            }
            Self::ResourceChanged {
                entity,
                resource,
                before,
                after,
                source,
            } => {
                3_u16.write(output)?;
                entity.write(output)?;
                resource.write(output)?;
                before.write(output)?;
                after.write(output)?;
                source.write(output)?;
                Ok(())
            }
            Self::DrawAccepted { operation, ordinal } => {
                4_u16.write(output)?;
                operation.write(output)?;
                ordinal.write(output)?;
                Ok(())
            }
            Self::ChoiceAccepted {
                resolution,
                window,
                choice,
            } => {
                5_u16.write(output)?;
                resolution.write(output)?;
                window.write(output)?;
                choice.write(output)?;
                Ok(())
            }
            Self::RulingAccepted {
                resolution,
                window,
                ruling,
            } => {
                6_u16.write(output)?;
                resolution.write(output)?;
                window.write(output)?;
                ruling.write(output)?;
                Ok(())
            }
            Self::TimeAdvanced { before, after } => {
                7_u16.write(output)?;
                before.write(output)?;
                after.write(output)?;
                Ok(())
            }
            Self::ContentEvent {
                definition,
                subjects,
            } => {
                8_u16.write(output)?;
                definition.write(output)?;
                subjects.write(output)?;
                Ok(())
            }
        }
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        match u16::read(input)? {
            1 => Ok(Self::EntityCreated {
                entity: Wire::read(input)?,
                definition: Wire::read(input)?,
            }),
            2 => Ok(Self::EntityMoved {
                entity: Wire::read(input)?,
                destination: Wire::read(input)?,
                position: Wire::read(input)?,
            }),
            3 => Ok(Self::ResourceChanged {
                entity: Wire::read(input)?,
                resource: Wire::read(input)?,
                before: Wire::read(input)?,
                after: Wire::read(input)?,
                source: Wire::read(input)?,
            }),
            4 => Ok(Self::DrawAccepted {
                operation: Wire::read(input)?,
                ordinal: Wire::read(input)?,
            }),
            5 => Ok(Self::ChoiceAccepted {
                resolution: Wire::read(input)?,
                window: Wire::read(input)?,
                choice: Wire::read(input)?,
            }),
            6 => Ok(Self::RulingAccepted {
                resolution: Wire::read(input)?,
                window: Wire::read(input)?,
                ruling: Wire::read(input)?,
            }),
            7 => Ok(Self::TimeAdvanced {
                before: Wire::read(input)?,
                after: Wire::read(input)?,
            }),
            8 => Ok(Self::ContentEvent {
                definition: Wire::read(input)?,
                subjects: Wire::read(input)?,
            }),
            _ => Err(CodecError::InvalidValue),
        }
    }
}
record_wire!(GameFact {
    id,
    revision,
    operation,
    ordinal,
    cause,
    audience,
    value
});
record_wire!(GameState {
    mode,
    logical_time,
    members,
    entities,
    characters,
    resources,
    inventory,
    facts,
    draws,
    decisions,
    pending,
    intents,
    timers,
    active_effects,
    knowledge,
    beliefs,
    memories,
    schedules,
    threats,
    relationships,
    conversations,
    obligations,
    narrative,
    encounters,
    activity,
    tempo,
    presentation,
    continuity
});
record_wire!(InventoryItem {
    item,
    owner,
    quantity,
    source,
    origin,
    attunement_owner
});
record_wire!(ItemOrigin {
    item,
    award_operation,
    source_fact,
    definition,
    identity_revision
});
record_wire!(JournalEntry {
    id,
    source_facts,
    attributed_claims,
    audience,
    text
});
record_wire!(KnowledgeCue {
    id,
    source_facts,
    audience,
    definition
});
record_wire!(KnowledgeGrant {
    observer,
    fact,
    source
});
record_wire!(LogicalTime {
    ticks,
    ticks_per_second
});
record_wire!(LostGameRange { from, through });
record_wire!(MembershipLink { member, character });
record_wire!(MemoryEpisode {
    id,
    holder,
    source_facts,
    retained_text,
    audience,
    source_revision
});
record_wire!(MemorySummary {
    id,
    episodes,
    derived_claims,
    source_digest,
    source_revision,
    summarizer,
    model,
    policy,
    audience,
    text,
    incomplete
});
record_wire!(NarrativeMoment {
    id,
    location,
    characters,
    facts,
    attributed_claims,
    audience,
    semantic_focus,
    identity_revision
});
record_wire!(NarrativeState {
    definition,
    active_beats,
    completed_beats,
    open_threads,
    accepted_facts,
    remaining_budget
});
record_wire!(NpcState {
    entity,
    personality,
    motivations,
    known_facts,
    beliefs,
    secrets
});
record_wire!(Obligation {
    id,
    obligor,
    beneficiary,
    definition,
    due,
    fulfilled
});
record_wire!(OfferedResponse {
    participant,
    offer,
    options,
    source
});
record_wire!(OwnedTimer {
    id,
    basis,
    generation,
    due,
    source,
    status
});
record_wire!(ParticipantPresence {
    member,
    bindings,
    state,
    policy
});
impl Wire for PendingInput {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        match self {
            Self::Choice { remaining } => {
                1_u16.write(output)?;
                remaining.write(output)?;
                Ok(())
            }
            Self::Roll {
                participant,
                sides,
                source,
            } => {
                2_u16.write(output)?;
                participant.write(output)?;
                sides.write(output)?;
                source.write(output)?;
                Ok(())
            }
            Self::Reaction { remaining } => {
                3_u16.write(output)?;
                remaining.write(output)?;
                Ok(())
            }
            Self::Ruling { permitted, source } => {
                4_u16.write(output)?;
                permitted.write(output)?;
                source.write(output)?;
                Ok(())
            }
        }
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        match u16::read(input)? {
            1 => Ok(Self::Choice {
                remaining: Wire::read(input)?,
            }),
            2 => Ok(Self::Roll {
                participant: Wire::read(input)?,
                sides: Wire::read(input)?,
                source: Wire::read(input)?,
            }),
            3 => Ok(Self::Reaction {
                remaining: Wire::read(input)?,
            }),
            4 => Ok(Self::Ruling {
                permitted: Wire::read(input)?,
                source: Wire::read(input)?,
            }),
            _ => Err(CodecError::InvalidValue),
        }
    }
}
record_wire!(PendingResolution {
    id,
    basis,
    continuation,
    window,
    next,
    choices,
    draw_ordinals,
    spent,
    rulings
});
record_wire!(PerformanceHint {
    definition,
    voice,
    emphasis_facts
});
record_wire!(Position { x, y, z });
record_wire!(PrefetchPolicy {
    definition,
    maximum_candidates,
    maximum_branches,
    maximum_bytes,
    maximum_duration_ticks
});
impl Wire for PresenceKind {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        match self {
            Self::Connected => 1_u16.write(output),
            Self::Sleeping => 2_u16.write(output),
            Self::Disconnected => 3_u16.write(output),
            Self::VoluntaryAfk => 4_u16.write(output),
        }
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        match u16::read(input)? {
            1 => Ok(Self::Connected),
            2 => Ok(Self::Sleeping),
            3 => Ok(Self::Disconnected),
            4 => Ok(Self::VoluntaryAfk),
            _ => Err(CodecError::InvalidValue),
        }
    }
}
record_wire!(PresentationDemand {
    id,
    definition,
    audience,
    causal_facts,
    source_revision
});
record_wire!(RecoveryState {
    origin,
    retired_epochs,
    lost_ranges,
    suppression_generation,
    redacted_records,
    unavailable_sources
});
record_wire!(Relationship {
    subject,
    object,
    policy,
    state
});
record_wire!(RemotePlayPolicy {
    definition,
    maximum_participants,
    maximum_connections,
    maximum_capture_leases,
    device_matrix
});
record_wire!(ResolutionWindow {
    id,
    phase,
    causal_fact,
    source,
    timer
});
record_wire!(ResourceSpend {
    owner,
    resource,
    amount,
    source
});
record_wire!(ResourceState {
    owner,
    resource,
    value,
    minimum,
    maximum,
    source
});
impl Wire for RestoreKind {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        match self {
            Self::DebugFork => 1_u16.write(output),
            Self::Disaster => 2_u16.write(output),
        }
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        match u16::read(input)? {
            1 => Ok(Self::DebugFork),
            2 => Ok(Self::Disaster),
            _ => Err(CodecError::InvalidValue),
        }
    }
}
record_wire!(RestoreOrigin {
    kind,
    checkpoint_digest,
    origin,
    process_generation
});
impl Wire for RetrievalPurpose {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        match self {
            Self::NpcContext => 1_u16.write(output),
            Self::PlayerRecall => 2_u16.write(output),
            Self::RecapHistory => 3_u16.write(output),
        }
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        match u16::read(input)? {
            1 => Ok(Self::NpcContext),
            2 => Ok(Self::PlayerRecall),
            3 => Ok(Self::RecapHistory),
            _ => Err(CodecError::InvalidValue),
        }
    }
}
record_wire!(RetrievalRequest {
    id,
    basis,
    observer,
    purpose,
    topics,
    entities,
    from,
    through,
    maximum_items,
    maximum_bytes,
    maximum_tokens,
    access_generation,
    index_generation,
    source_digest,
    policy
});
record_wire!(RetrievedMemory {
    request,
    episodes,
    facts,
    attributed_claims,
    snippets,
    source_revision,
    index_generation,
    access_generation,
    incomplete
});
record_wire!(RuleReference {
    catalog,
    source,
    entry,
    clause
});
impl Wire for RulesMode {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        match self {
            Self::Standard2024 => 1_u16.write(output),
            Self::DisclosedCustom => 2_u16.write(output),
        }
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        match u16::read(input)? {
            1 => Ok(Self::Standard2024),
            2 => Ok(Self::DisclosedCustom),
            _ => Err(CodecError::InvalidValue),
        }
    }
}
record_wire!(RulesPins {
    mode,
    ruleset,
    catalog,
    catalog_digest,
    source_manifest,
    source_manifest_digest,
    handler,
    handler_digest
});
record_wire!(RumorTransmission {
    id,
    claim,
    sender,
    recipient,
    evidence,
    policy,
    remaining_hops,
    audience
});
record_wire!(SceneIdentityRevision {
    scene,
    revision,
    source_facts,
    geometry,
    canonical_pack
});
record_wire!(ScheduledEvent {
    id,
    entity,
    due,
    definition
});
record_wire!(ScopedRuling {
    adjudicator,
    selected,
    source,
    audience
});
record_wire!(SecretPolicy {
    holder,
    claims,
    policy,
    permitted_audience
});
record_wire!(ShotPlan {
    id,
    moment,
    subjects,
    audience,
    duration_ticks,
    definition,
    references,
    performance
});
impl Wire for SimulationTier {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        match self {
            Self::Active => 1_u16.write(output),
            Self::Scheduled => 2_u16.write(output),
            Self::Dormant => 3_u16.write(output),
        }
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        match u16::read(input)? {
            1 => Ok(Self::Active),
            2 => Ok(Self::Scheduled),
            3 => Ok(Self::Dormant),
            _ => Err(CodecError::InvalidValue),
        }
    }
}
record_wire!(StoryArc {
    id,
    definition,
    phase,
    source_facts,
    active_hooks
});
record_wire!(TempoState {
    policy,
    presentation_ticks,
    intensity,
    inertia,
    fatigue
});
record_wire!(ThreatClock {
    id,
    definition,
    progress,
    capacity
});
record_wire!(TravelLeg {
    id,
    travelers,
    from,
    destination,
    route,
    starts,
    arrives,
    source
});
impl Wire for TriggerPhase {
    fn write(&self, output: &mut Encoder) -> Result<(), CodecError> {
        match self {
            Self::BeforeDraw => 1_u16.write(output),
            Self::AfterDraw => 2_u16.write(output),
            Self::BeforeConsequence => 3_u16.write(output),
            Self::AfterConsequence => 4_u16.write(output),
        }
    }
    fn read(input: &mut Decoder<'_>) -> Result<Self, CodecError> {
        match u16::read(input)? {
            1 => Ok(Self::BeforeDraw),
            2 => Ok(Self::AfterDraw),
            3 => Ok(Self::BeforeConsequence),
            4 => Ok(Self::AfterConsequence),
            _ => Err(CodecError::InvalidValue),
        }
    }
}
record_wire!(VisualBible {
    revision,
    definition,
    palette,
    style,
    references
});
record_wire!(WitnessRecord {
    id,
    observer,
    fact,
    perceived_at,
    source
});
record_wire!(WorldEntity {
    id,
    definition,
    location,
    position,
    identity_revision
});

fn encode_record<T: Wire>(
    magic: &[u8; 4],
    value: &T,
    limits: CodecLimits,
) -> Result<Vec<u8>, CodecError> {
    let limits = limits.validate()?;
    let mut output = Encoder {
        bytes: Vec::new(),
        maximum: limits.maximum_document_bytes,
        allocation_remaining: limits.maximum_allocated_bytes,
        items_remaining: limits.maximum_collection_items,
        maximum_text_bytes: limits.maximum_text_bytes,
    };
    output.put(magic)?;
    CODEC_VERSION.write(&mut output)?;
    value.write(&mut output)?;
    Ok(output.bytes)
}
fn decode_record<T: Wire>(
    magic: &[u8; 4],
    bytes: &[u8],
    limits: CodecLimits,
) -> Result<T, CodecError> {
    let limits = limits.validate()?;
    if bytes.len() > limits.maximum_document_bytes {
        return Err(CodecError::Capacity);
    }
    let mut input = Decoder {
        bytes,
        offset: 0,
        allocation_remaining: limits.maximum_allocated_bytes,
        items_remaining: limits.maximum_collection_items,
        maximum_text_bytes: limits.maximum_text_bytes,
    };
    if input.take(magic.len())? != magic || u16::read(&mut input)? != CODEC_VERSION {
        return Err(CodecError::UnsupportedCodec);
    }
    let value = T::read(&mut input)?;
    if input.offset != bytes.len() {
        return Err(CodecError::TrailingBytes);
    }
    Ok(value)
}
pub(crate) fn encode_fact(fact: &GameFact, limits: CodecLimits) -> Result<Vec<u8>, CodecError> {
    encode_record(b"DFFA", fact, limits)
}
pub(crate) fn encode_intent(
    intent: &DurableIntent,
    limits: CodecLimits,
) -> Result<Vec<u8>, CodecError> {
    encode_record(b"DFIT", intent, limits)
}
pub(crate) fn encode_receipt(
    receipt: &df_session::submission::DecisionReceipt,
    limits: CodecLimits,
) -> Result<Vec<u8>, CodecError> {
    let limits = limits.validate()?;
    let mut output = Encoder {
        bytes: Vec::new(),
        maximum: limits.maximum_document_bytes,
        allocation_remaining: limits.maximum_allocated_bytes,
        items_remaining: limits.maximum_collection_items,
        maximum_text_bytes: limits.maximum_text_bytes,
    };
    output.put(b"DFRC")?;
    CODEC_VERSION.write(&mut output)?;
    receipt.basis().write(&mut output)?;
    receipt.decision().write(&mut output)?;
    Ok(output.bytes)
}
pub(crate) fn decode_receipt(
    bytes: &[u8],
    expected_session: SessionId,
    expected_operation: OperationId,
    maximum_receipt_bytes: usize,
    limits: CodecLimits,
) -> Result<df_session::submission::DecisionReceipt, CodecError> {
    let (basis, decision): (Basis, AcceptedDecision) = decode_record(b"DFRC", bytes, limits)?;
    if basis.session != expected_session || decision.operation != expected_operation {
        return Err(CodecError::InvalidValue);
    }
    df_session::submission::DecisionReceipt::new(basis, decision, maximum_receipt_bytes)
        .map_err(|_| CodecError::InvalidValue)
}

/// Diagnostic capacity of admitted pins, including actual owned label capacities.
pub(crate) fn pins_retained_heap_bytes(pins: &CheckpointPins) -> Option<usize> {
    let mut total = 0_usize;
    for label in [
        &pins.rules.ruleset,
        &pins.rules.catalog,
        &pins.rules.source_manifest,
        &pins.rules.handler,
        &pins.content.content,
        &pins.content.package,
    ] {
        total = total.checked_add(label.retained_heap_bytes())?;
    }
    for kind in [
        BuildRevision::Source,
        BuildRevision::Native,
        BuildRevision::Wasm,
        BuildRevision::Configuration,
        BuildRevision::Content,
    ] {
        total = total.checked_add(pins.build.revision(kind).retained_heap_bytes())?;
    }
    Some(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label(value: &str) -> RevisionLabel {
        RevisionLabel::new(Some(value)).unwrap()
    }

    fn entity(value: u8) -> EntityId {
        EntityId::from_bytes(&[value; 16]).unwrap()
    }

    fn member(value: u8) -> MemberId {
        MemberId::from_bytes(&[value; 16]).unwrap()
    }

    fn revision(epoch: u64, sequence: u64) -> SessionRevision {
        SessionRevision::new(RecoveryEpoch::new(epoch).unwrap(), sequence)
    }

    fn basis() -> Basis {
        Basis {
            session: SessionId::from_bytes(&[1; 16]).unwrap(),
            run: RunId::from_bytes(&[2; 16]).unwrap(),
            revision: revision(2, 8),
        }
    }

    fn content() -> ContentReference {
        ContentReference {
            package: label("fixture-package-1"),
            entry: label("fixture-entry-1"),
        }
    }

    fn rule() -> RuleReference {
        RuleReference {
            catalog: label("fixture-catalog-1"),
            source: label("fixture-source-1"),
            entry: label("fixture-entry-1"),
            clause: label("fixture-clause-1"),
        }
    }

    fn pins() -> CheckpointPins {
        CheckpointPins {
            rules: RulesPins {
                mode: RulesMode::Standard2024,
                ruleset: label("fixture-rules-1"),
                catalog: label("fixture-catalog-1"),
                catalog_digest: ContentDigest([1; 32]),
                source_manifest: label("fixture-sources-1"),
                source_manifest_digest: ContentDigest([2; 32]),
                handler: label("fixture-handler-1"),
                handler_digest: ContentDigest([3; 32]),
            },
            content: ContentPins {
                content: label("fixture-content-1"),
                content_digest: ContentDigest([4; 32]),
                package: label("fixture-package-1"),
                package_digest: ContentDigest([5; 32]),
            },
            build: BuildIdentity::new(
                Some("fixture-source-1"),
                Some("fixture-native-1"),
                Some("fixture-wasm-1"),
                Some("fixture-config-1"),
                Some("fixture-content-1"),
            )
            .unwrap(),
        }
    }

    fn resource_constraints() -> Vec<ResourceConstraint> {
        vec![ResourceConstraint {
            owner: entity(4),
            resource: label("fixture-resource-1"),
            minimum: 0,
            maximum: 8,
            source: rule(),
        }]
    }

    fn limits() -> CheckpointLimits {
        CheckpointLimits {
            maximum_records: 100,
            maximum_text_bytes: 256,
            maximum_total_text_bytes: 1024,
            maximum_retained_bytes: 1024 * 1024,
        }
    }

    fn state() -> GameState {
        GameState {
            mode: ExecutionMode::Replay,
            logical_time: LogicalTime {
                ticks: 120,
                ticks_per_second: 10,
            },
            members: vec![MembershipLink {
                member: member(3),
                character: Some(entity(4)),
            }],
            entities: vec![WorldEntity {
                id: entity(4),
                definition: content(),
                location: None,
                position: Some(Position { x: 0, y: 0, z: 0 }),
                identity_revision: label("fixture-entity-1"),
            }],
            characters: vec![CharacterState {
                entity: entity(4),
                build: content(),
                owner: member(3),
                choices: vec![],
            }],
            resources: vec![ResourceState {
                owner: entity(4),
                resource: label("fixture-resource-1"),
                value: 4,
                minimum: 0,
                maximum: 8,
                source: rule(),
            }],
            inventory: vec![],
            facts: vec![],
            draws: vec![],
            decisions: vec![],
            pending: vec![],
            intents: vec![],
            timers: vec![],
            active_effects: vec![],
            knowledge: vec![],
            beliefs: vec![],
            memories: vec![],
            schedules: vec![],
            threats: vec![],
            relationships: vec![],
            conversations: vec![],
            obligations: vec![],
            narrative: NarrativeState {
                definition: content(),
                active_beats: vec![],
                completed_beats: vec![],
                open_threads: vec![],
                accepted_facts: vec![],
                remaining_budget: 0,
            },
            encounters: vec![],
            activity: vec![],
            tempo: TempoState {
                policy: content(),
                presentation_ticks: 0,
                intensity: 0,
                inertia: 0,
                fatigue: vec![],
            },
            presentation: vec![],
            continuity: continuity(),
        }
    }

    fn checkpoint(state: GameState) -> Result<Checkpoint, CheckpointError> {
        let rules = vec![rule()];
        let content_entries = vec![content()];
        let resource_constraints = resource_constraints();
        Checkpoint::new(
            CHECKPOINT_SCHEMA,
            basis(),
            pins(),
            state,
            ReferenceInventory {
                rules: &rules,
                content: &content_entries,
                resources: &resource_constraints,
                assets: &[],
            },
            limits(),
        )
    }

    fn fact(value: u8, ordinal: u32) -> GameFact {
        GameFact {
            id: FactId::from_bytes(&[value; 16]).unwrap(),
            revision: basis().revision,
            operation: OperationId::from_bytes(&[6; 16]).unwrap(),
            ordinal,
            cause: None,
            audience: AudienceScope::Shared,
            value: FactValue::ContentEvent {
                definition: content(),
                subjects: vec![entity(4)],
            },
        }
    }

    fn continuity() -> ContinuityState {
        ContinuityState {
            creation: vec![],
            simulation: vec![],
            catch_up: None,
            environment: vec![],
            travel: vec![],
            witnesses: vec![],
            rumors: vec![],
            journal: vec![],
            summaries: vec![],
            retrieval: vec![],
            retrieved: vec![],
            consolidation: vec![],
            npcs: vec![],
            hooks: vec![],
            arcs: vec![],
            remote: None,
            presence: vec![],
            audio: None,
            private_offers: vec![],
            knowledge_cues: vec![],
            moments: vec![],
            demands: vec![],
            asset_jobs: vec![],
            asset_dependencies: vec![],
            canonical_packs: vec![],
            shots: vec![],
            prefetch: None,
            scenes: vec![],
            item_origins: vec![],
            bookends: vec![],
            exports: vec![],
            critical_cues: vec![],
            content_candidates: vec![],
            content_admissions: vec![],
            recovery: RecoveryState {
                origin: None,
                retired_epochs: vec![],
                lost_ranges: vec![],
                suppression_generation: 0,
                redacted_records: vec![],
                unavailable_sources: vec![],
            },
        }
    }
    fn codec_limits() -> CodecLimits {
        CodecLimits {
            maximum_document_bytes: 1024 * 1024,
            maximum_allocated_bytes: 8 * 1024 * 1024,
            maximum_collection_items: 4096,
            maximum_text_bytes: 256,
        }
    }
    fn decode(
        bytes: &[u8],
        expected: Basis,
        admitted: &CheckpointPins,
    ) -> Result<Checkpoint, CodecError> {
        let rules = vec![rule()];
        let content_entries = vec![content()];
        let resources = resource_constraints();
        decode_checkpoint(
            bytes,
            expected,
            admitted,
            ReferenceInventory {
                rules: &rules,
                content: &content_entries,
                resources: &resources,
                assets: &[],
            },
            limits(),
            codec_limits(),
        )
    }
    fn value_round_trip<T: Wire + std::fmt::Debug + PartialEq>(value: &T) {
        let bounds = codec_limits();
        let mut output = Encoder {
            bytes: Vec::new(),
            maximum: bounds.maximum_document_bytes,
            allocation_remaining: bounds.maximum_allocated_bytes,
            items_remaining: bounds.maximum_collection_items,
            maximum_text_bytes: bounds.maximum_text_bytes,
        };
        value.write(&mut output).unwrap();
        let mut input = Decoder {
            bytes: &output.bytes,
            offset: 0,
            allocation_remaining: bounds.maximum_allocated_bytes,
            items_remaining: bounds.maximum_collection_items,
            maximum_text_bytes: bounds.maximum_text_bytes,
        };
        assert_eq!(T::read(&mut input).unwrap(), *value);
        assert_eq!(input.offset, output.bytes.len());
    }

    #[test]
    fn recovery_epoch_wire_preserves_nonzero_full_unsigned_domain() {
        value_round_trip(&RecoveryEpoch::new(1).unwrap());
        value_round_trip(&RecoveryEpoch::new(u64::MAX).unwrap());
        value_round_trip(&vec![
            RecoveryEpoch::new(1).unwrap(),
            RecoveryEpoch::new(u64::MAX).unwrap(),
        ]);
        let bounds = codec_limits();
        let mut output = Encoder {
            bytes: Vec::new(),
            maximum: 8,
            allocation_remaining: bounds.maximum_allocated_bytes,
            items_remaining: bounds.maximum_collection_items,
            maximum_text_bytes: bounds.maximum_text_bytes,
        };
        RecoveryEpoch::new(u64::MAX)
            .unwrap()
            .write(&mut output)
            .unwrap();
        assert_eq!(output.bytes, u64::MAX.to_be_bytes());
        for bytes in [&[0_u8; 8][..], &[0_u8; 7][..]] {
            let mut input = Decoder {
                bytes,
                offset: 0,
                allocation_remaining: bounds.maximum_allocated_bytes,
                items_remaining: bounds.maximum_collection_items,
                maximum_text_bytes: bounds.maximum_text_bytes,
            };
            assert_eq!(
                RecoveryEpoch::read(&mut input),
                Err(if bytes.len() == 8 {
                    CodecError::InvalidValue
                } else {
                    CodecError::Truncated
                })
            );
        }
    }
    #[test]
    fn complete_recovery_state_preserves_retired_epochs_exactly() {
        let mut supplied = state();
        supplied.continuity.recovery.retired_epochs = vec![RecoveryEpoch::new(1).unwrap()];
        let checkpoint = checkpoint(supplied).unwrap();
        let bytes = encode_checkpoint(&checkpoint, codec_limits()).unwrap();
        assert_eq!(
            decode(&bytes, checkpoint.basis(), checkpoint.pins()).unwrap(),
            checkpoint
        );
    }
    fn appearance_state(appearance: Option<CharacterAppearance>) -> GameState {
        let mut supplied = state();
        supplied.continuity.canonical_packs.push(CanonicalPack {
            revision: label("fixture-pack-1"),
            digest: ContentDigest([12; 32]),
            bible: VisualBible {
                revision: label("fixture-bible-1"),
                definition: content(),
                palette: vec![],
                style: "fixture style".to_owned(),
                references: vec![],
            },
            identities: vec![EntityIdentityRevision {
                entity: entity(4),
                revision: label("fixture-identity-1"),
                character_appearance: appearance,
                source_facts: vec![],
                appearances: vec![],
                voice: None,
                sound: vec![],
            }],
        });
        supplied
    }

    fn appearance() -> CharacterAppearance {
        CharacterAppearance {
            features: "Copper curls and a scar over the left eyebrow.".to_owned(),
            outfit: "A green coat with silver clasps.".to_owned(),
            outfit_revision: label("fixture-outfit-2"),
        }
    }

    #[test]
    fn optional_character_appearance_survives_complete_checkpoint_round_trip() {
        for appearance in [None, Some(appearance())] {
            let checkpoint = checkpoint(appearance_state(appearance)).unwrap();
            let bytes = encode_checkpoint(&checkpoint, codec_limits()).unwrap();
            assert_eq!(&bytes[4..6], &2_u16.to_be_bytes());
            let restored = decode(&bytes, checkpoint.basis(), checkpoint.pins()).unwrap();
            assert_eq!(restored, checkpoint);
            assert_eq!(encode_checkpoint(&restored, codec_limits()).unwrap(), bytes);
        }
    }

    #[test]
    fn legacy_codec_refuses_instead_of_defaulting_character_appearance() {
        let checkpoint = checkpoint(appearance_state(Some(appearance()))).unwrap();
        let mut bytes = encode_checkpoint(&checkpoint, codec_limits()).unwrap();
        bytes[4..6].copy_from_slice(&1_u16.to_be_bytes());
        assert_eq!(
            decode(&bytes, checkpoint.basis(), checkpoint.pins()),
            Err(CodecError::UnsupportedCodec)
        );
    }

    #[test]
    fn malformed_and_oversized_character_appearance_storage_refuses() {
        for (bytes, expected) in [
            (vec![2], CodecError::InvalidValue),
            (vec![1, 0, 0, 0, 1, 255], CodecError::InvalidValue),
            (vec![1, 0, 0, 0, 2, b'a'], CodecError::Truncated),
            (vec![1, 0, 0, 1, 1], CodecError::Capacity),
            (
                vec![1, 0, 0, 0, 1, b'a', 0, 0, 0, 1, b'b', 0, 0],
                CodecError::InvalidValue,
            ),
        ] {
            let bounds = codec_limits();
            let mut input = Decoder {
                bytes: &bytes,
                offset: 0,
                allocation_remaining: bounds.maximum_allocated_bytes,
                items_remaining: bounds.maximum_collection_items,
                maximum_text_bytes: bounds.maximum_text_bytes,
            };
            assert_eq!(
                Option::<CharacterAppearance>::read(&mut input),
                Err(expected)
            );
        }
    }

    #[test]
    fn character_appearance_encode_caps_and_canonical_text_validation_remain_required() {
        for field in [0, 1] {
            let mut appearance = appearance();
            if field == 0 {
                appearance.features = "x".repeat(codec_limits().maximum_text_bytes + 1);
            } else {
                appearance.outfit = "x".repeat(codec_limits().maximum_text_bytes + 1);
            }
            let bounds = codec_limits();
            let mut output = Encoder {
                bytes: Vec::new(),
                maximum: bounds.maximum_document_bytes,
                allocation_remaining: bounds.maximum_allocated_bytes,
                items_remaining: bounds.maximum_collection_items,
                maximum_text_bytes: bounds.maximum_text_bytes,
            };
            assert_eq!(appearance.write(&mut output), Err(CodecError::Capacity));
        }
        let mut empty = appearance();
        empty.features = " ".to_owned();
        assert_eq!(
            checkpoint(appearance_state(Some(empty))),
            Err(CheckpointError::InvalidReference)
        );
    }

    #[test]
    fn persisted_checkpoint_codec_matches_complete_current_envelope() {
        let checkpoint = checkpoint(appearance_state(Some(appearance()))).unwrap();
        let document = encode_checkpoint(&checkpoint, codec_limits()).unwrap();
        assert_eq!(STORAGE_CODEC_VERSION, 2);
        assert_eq!(checkpoint.schema(), CHECKPOINT_SCHEMA);
        assert_eq!(validate_storage_format(2, b"DFCP", &document), Ok(()));
        assert_eq!(
            decode(&document, checkpoint.basis(), checkpoint.pins()).unwrap(),
            checkpoint
        );
        assert_eq!(
            validate_storage_format(1, b"DFCP", &document),
            Err(CodecError::UnsupportedCodec)
        );
        assert_eq!(
            validate_storage_format(3, b"DFCP", &document),
            Err(CodecError::UnsupportedCodec)
        );
        let mut legacy = document;
        legacy[4..6].copy_from_slice(&1_u16.to_be_bytes());
        assert_eq!(
            validate_storage_format(1, b"DFCP", &legacy),
            Err(CodecError::UnsupportedCodec)
        );
        assert_eq!(
            validate_storage_format(2, b"DFCP", &legacy),
            Err(CodecError::UnsupportedCodec)
        );
    }

    #[test]
    fn stored_record_formats_refuse_mismatched_metadata_magic_and_header() {
        for magic in [b"DFFA", b"DFRC", b"DFIT"] {
            let mut document = magic.to_vec();
            document.extend_from_slice(&2_u16.to_be_bytes());
            assert_eq!(validate_storage_format(2, magic, &document), Ok(()));
            assert_eq!(
                validate_storage_format(1, magic, &document),
                Err(CodecError::UnsupportedCodec)
            );
            assert_eq!(
                validate_storage_format(2, b"DFCP", &document),
                Err(CodecError::UnsupportedCodec)
            );
            document[4..6].copy_from_slice(&1_u16.to_be_bytes());
            assert_eq!(
                validate_storage_format(2, magic, &document),
                Err(CodecError::UnsupportedCodec)
            );
            assert_eq!(
                validate_storage_format(2, magic, &document[..5]),
                Err(CodecError::Truncated)
            );
        }
    }

    #[test]
    fn complete_owned_checkpoint_round_trips_exactly() {
        let checkpoint = checkpoint(state()).unwrap();
        let bytes = encode_checkpoint(&checkpoint, codec_limits()).unwrap();
        let restored = decode(&bytes, checkpoint.basis(), checkpoint.pins()).unwrap();
        assert_eq!(restored, checkpoint);
        assert_eq!(encode_checkpoint(&restored, codec_limits()).unwrap(), bytes);
    }
    #[test]
    fn canonical_facts_actual_draws_and_unknown_intents_round_trip_without_reconstruction() {
        let mut supplied = state();
        supplied.facts.push(fact(7, 0));
        let draw = ActualDraw {
            operation: OperationId::from_bytes(&[6; 16]).unwrap(),
            ordinal: 0,
            resolution: ResolutionId::from_bytes(&[7; 16]).unwrap(),
            window: WindowId::from_bytes(&[8; 16]).unwrap(),
            sides: 20,
            value: 13,
            source: rule(),
        };
        supplied.draws.push(draw);
        supplied.intents.push(DurableIntent {
            id: EffectId::from_bytes(&[11; 16]).unwrap(),
            basis: basis(),
            operation: OperationId::from_bytes(&[6; 16]).unwrap(),
            slot: 0,
            kind: EffectKind::RunAi,
            job: Some(JobId::from_bytes(&[12; 16]).unwrap()),
            timer: None,
            generation: 2,
            status: DurableStatus::SentUnknown,
            definition: content(),
        });
        let checkpoint = checkpoint(supplied).unwrap();
        let bytes = encode_checkpoint(&checkpoint, codec_limits()).unwrap();
        assert_eq!(
            decode(&bytes, checkpoint.basis(), checkpoint.pins()).unwrap(),
            checkpoint
        );
    }
    #[test]
    fn full_unsigned_epoch_sequence_and_signed_position_round_trip() {
        let mut supplied = state();
        supplied.entities[0].position = Some(Position {
            x: i64::MIN,
            y: i64::MAX,
            z: -1,
        });
        let expected = Basis {
            revision: revision(u64::MAX, u64::MAX),
            ..basis()
        };
        let rules = vec![rule()];
        let entries = vec![content()];
        let resources = resource_constraints();
        let checkpoint = Checkpoint::new(
            CHECKPOINT_SCHEMA,
            expected,
            pins(),
            supplied,
            ReferenceInventory {
                rules: &rules,
                content: &entries,
                resources: &resources,
                assets: &[],
            },
            limits(),
        )
        .unwrap();
        let bytes = encode_checkpoint(&checkpoint, codec_limits()).unwrap();
        assert_eq!(
            decode(&bytes, expected, checkpoint.pins()).unwrap(),
            checkpoint
        );
    }
    #[test]
    fn every_truncated_boundary_and_trailing_storage_byte_refuses() {
        let checkpoint = checkpoint(state()).unwrap();
        let bytes = encode_checkpoint(&checkpoint, codec_limits()).unwrap();
        for end in 0..bytes.len() {
            assert!(
                decode(&bytes[..end], basis(), &pins()).is_err(),
                "offset {end}"
            );
        }
        let mut trailing = bytes;
        trailing.push(0);
        assert_eq!(
            decode(&trailing, basis(), &pins()),
            Err(CodecError::TrailingBytes)
        );
    }
    #[test]
    fn unknown_document_and_model_versions_refuse() {
        let checkpoint = checkpoint(state()).unwrap();
        let bytes = encode_checkpoint(&checkpoint, codec_limits()).unwrap();
        for offset in [0, 5] {
            let mut changed = bytes.clone();
            changed[offset] = 255;
            assert_eq!(
                decode(&changed, basis(), &pins()),
                Err(CodecError::UnsupportedCodec)
            );
        }
        let mut changed = bytes;
        changed[7] = 255;
        assert_eq!(
            decode(&changed, basis(), &pins()),
            Err(CodecError::Checkpoint(CheckpointError::UnsupportedSchema))
        );
    }
    #[test]
    fn row_basis_and_admitted_pins_cannot_be_overwritten_by_envelope() {
        let checkpoint = checkpoint(state()).unwrap();
        let bytes = encode_checkpoint(&checkpoint, codec_limits()).unwrap();
        let mut expected = basis();
        expected.run = RunId::from_bytes(&[99; 16]).unwrap();
        assert_eq!(
            decode(&bytes, expected, &pins()),
            Err(CodecError::Checkpoint(CheckpointError::WrongRun))
        );
        expected = basis();
        expected.revision = revision(3, 8);
        assert_eq!(
            decode(&bytes, expected, &pins()),
            Err(CodecError::Checkpoint(CheckpointError::StaleBasis))
        );
        let mut admitted = pins();
        admitted.content.package_digest = ContentDigest([99; 32]);
        assert_eq!(
            decode(&bytes, basis(), &admitted),
            Err(CodecError::Checkpoint(CheckpointError::ContentMismatch))
        );
    }
    #[test]
    fn missing_trusted_inventory_refuses_otherwise_well_formed_envelope() {
        let checkpoint = checkpoint(state()).unwrap();
        let bytes = encode_checkpoint(&checkpoint, codec_limits()).unwrap();
        let result = decode_checkpoint(
            &bytes,
            basis(),
            &pins(),
            ReferenceInventory {
                rules: &[],
                content: &[],
                resources: &[],
                assets: &[],
            },
            limits(),
            codec_limits(),
        );
        assert_eq!(
            result,
            Err(CodecError::Checkpoint(CheckpointError::InvalidReference))
        );
    }
    #[test]
    fn decode_length_and_collection_budget_fail_before_allocation() {
        let data = u32::MAX.to_be_bytes();
        let mut input = Decoder {
            bytes: &data,
            offset: 0,
            allocation_remaining: 64,
            items_remaining: 8,
            maximum_text_bytes: 32,
        };
        assert_eq!(Vec::<String>::read(&mut input), Err(CodecError::Capacity));
        assert_eq!(input.offset, 4);
        assert_eq!(input.allocation_remaining, 64);
    }
    #[test]
    fn output_and_input_document_capacity_are_independently_enforced() {
        let checkpoint = checkpoint(state()).unwrap();
        let mut bounds = codec_limits();
        bounds.maximum_document_bytes = 7;
        assert_eq!(
            encode_checkpoint(&checkpoint, bounds),
            Err(CodecError::Capacity)
        );
        let bytes = encode_checkpoint(&checkpoint, codec_limits()).unwrap();
        let result = decode_checkpoint(
            &bytes,
            basis(),
            &pins(),
            ReferenceInventory {
                rules: &[],
                content: &[],
                resources: &[],
                assets: &[],
            },
            limits(),
            bounds,
        );
        assert_eq!(result, Err(CodecError::Capacity));
    }
    #[test]
    fn unicode_private_text_and_typed_audiences_remain_exact() {
        value_round_trip(&AudienceScope::Members(vec![member(3), member(4)]));
        value_round_trip(&"private é 日本語".to_owned());
        value_round_trip(&AudioDestination::PrivateListener {
            member: member(3),
            binding: ClientBindingId::from_bytes(&[5; 16]).unwrap(),
        });
    }
    #[test]
    fn unknown_enum_option_boolean_and_invalid_utf8_are_typed_failures() {
        let mut input = Decoder {
            bytes: &[0, 99],
            offset: 0,
            allocation_remaining: 64,
            items_remaining: 8,
            maximum_text_bytes: 32,
        };
        assert_eq!(
            DurableStatus::read(&mut input),
            Err(CodecError::InvalidValue)
        );
        input = Decoder {
            bytes: &[2],
            offset: 0,
            allocation_remaining: 64,
            items_remaining: 8,
            maximum_text_bytes: 32,
        };
        assert_eq!(bool::read(&mut input), Err(CodecError::InvalidValue));
        input.offset = 0;
        assert_eq!(
            Option::<u64>::read(&mut input),
            Err(CodecError::InvalidValue)
        );
        input = Decoder {
            bytes: &[0, 0, 0, 1, 255],
            offset: 0,
            allocation_remaining: 64,
            items_remaining: 8,
            maximum_text_bytes: 32,
        };
        assert_eq!(String::read(&mut input), Err(CodecError::InvalidValue));
    }

    #[test]
    fn canonical_receipt_round_trip_preserves_semantics_and_exact_scope() {
        let operation = OperationId::from_bytes(&[6; 16]).unwrap();
        let decision = AcceptedDecision {
            operation,
            revision: basis().revision,
            facts: vec![],
            draws: vec![],
            effects: vec![],
            source_policy: label("fixture-policy"),
            semantic_output: Some("private é 日本語".to_owned()),
        };
        let receipt =
            df_session::submission::DecisionReceipt::new(basis(), decision, 4096).unwrap();
        let bytes = encode_receipt(&receipt, codec_limits()).unwrap();
        assert_eq!(
            decode_receipt(&bytes, basis().session, operation, 4096, codec_limits()).unwrap(),
            receipt
        );
        assert_eq!(
            decode_receipt(
                &bytes,
                SessionId::from_bytes(&[99; 16]).unwrap(),
                operation,
                4096,
                codec_limits()
            ),
            Err(CodecError::InvalidValue)
        );
        assert_eq!(
            decode_receipt(
                &bytes,
                basis().session,
                OperationId::from_bytes(&[99; 16]).unwrap(),
                4096,
                codec_limits()
            ),
            Err(CodecError::InvalidValue)
        );
        assert_eq!(
            decode_receipt(&bytes, basis().session, operation, 1, codec_limits()),
            Err(CodecError::InvalidValue)
        );
        for end in 0..bytes.len() {
            assert!(
                decode_receipt(
                    &bytes[..end],
                    basis().session,
                    operation,
                    4096,
                    codec_limits()
                )
                .is_err()
            );
        }
    }

    #[test]
    fn standalone_fact_and_intent_documents_keep_their_actual_domain_records() {
        let fact = fact(7, 0);
        let bytes = encode_fact(&fact, codec_limits()).unwrap();
        assert_eq!(
            decode_record::<GameFact>(b"DFFA", &bytes, codec_limits()).unwrap(),
            fact
        );
        assert_eq!(
            decode_record::<GameFact>(b"DFIT", &bytes, codec_limits()),
            Err(CodecError::UnsupportedCodec)
        );
        let intent = DurableIntent {
            id: EffectId::from_bytes(&[11; 16]).unwrap(),
            basis: basis(),
            operation: OperationId::from_bytes(&[6; 16]).unwrap(),
            slot: u32::MAX,
            kind: EffectKind::CancelJob,
            job: Some(JobId::from_bytes(&[12; 16]).unwrap()),
            timer: None,
            generation: u64::MAX,
            status: DurableStatus::Pending,
            definition: content(),
        };
        let bytes = encode_intent(&intent, codec_limits()).unwrap();
        assert_eq!(
            decode_record::<DurableIntent>(b"DFIT", &bytes, codec_limits()).unwrap(),
            intent
        );
    }

    #[test]
    fn allocated_capacity_and_collection_budget_are_finite_on_valid_documents() {
        let checkpoint = checkpoint(state()).unwrap();
        let bytes = encode_checkpoint(&checkpoint, codec_limits()).unwrap();
        let mut bounds = codec_limits();
        bounds.maximum_allocated_bytes = 1;
        assert_eq!(
            encode_checkpoint(&checkpoint, bounds),
            Err(CodecError::Capacity)
        );
        let rules = vec![rule()];
        let entries = vec![content()];
        let resources = resource_constraints();
        assert_eq!(
            decode_checkpoint(
                &bytes,
                basis(),
                &pins(),
                ReferenceInventory {
                    rules: &rules,
                    content: &entries,
                    resources: &resources,
                    assets: &[]
                },
                limits(),
                bounds
            ),
            Err(CodecError::Capacity)
        );
        bounds = codec_limits();
        bounds.maximum_collection_items = 1;
        assert_eq!(
            encode_checkpoint(&checkpoint, bounds),
            Err(CodecError::Capacity)
        );
    }
}
