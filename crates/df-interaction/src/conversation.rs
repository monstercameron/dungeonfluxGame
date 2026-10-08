//! Pure current conversation admission. Selection never grants knowledge or commits a reveal.
use std::fmt;

use df_model::checkpoint::{
    Basis, CheckpointPins, ContentReference, ConversationState, EntityId, LogicalTime, RecordId,
};

use crate::speech::{
    ExpressionClaim, ExpressionContext, ExpressionMetadata, ObserverScope, SpeechActPlan,
    SpeechError, SpeechObservation, SpeechProposal, admit_speech, project_expression_context,
};

/// Start is inclusive and expiry exclusive, in the checkpoint's logical timebase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PermissionWindow {
    pub opens_at: LogicalTime,
    pub expires_at: LogicalTime,
}

/// Explicit selection of an existing claim and its existing secret policy.
#[derive(Clone, Eq, PartialEq)]
pub struct SecretDisclosure {
    pub claim: RecordId,
    pub policy: ContentReference,
}

/// Server-side candidate scope. These values confer no permission by themselves.
#[derive(Clone, Eq, PartialEq)]
pub struct ConversationPermission {
    pub policy: ContentReference,
    pub conversation: RecordId,
    pub topic: ContentReference,
    pub speaker: EntityId,
    pub recipient: ObserverScope,
    pub window: PermissionWindow,
    pub disclosures: Vec<SecretDisclosure>,
}

impl fmt::Debug for ConversationPermission {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConversationPermission")
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConversationSourceError {
    AccessDenied,
    Unsupported,
    Stale,
}

/// The trusted source owner validates actual policy and current checkpoint permissions.
/// It must explicitly admit the literal speaker, topic, recipient, window and selected
/// disclosures; Shared publication requires its own explicit admission. Content pins,
/// trust scores, caller flags and equality are never substitutes. No issuer is supplied here.
pub trait ConversationSourceOwner {
    fn validate(
        &self,
        basis: Basis,
        pins: &CheckpointPins,
        conversation: &ConversationState,
        permission: &ConversationPermission,
        claims: &[RecordId],
    ) -> Result<(), ConversationSourceError>;
}

/// Finite per-invocation structural work. Speech and Knowledge retain their own bounds.
#[derive(Clone, Copy, Debug)]
pub struct ConversationLimits {
    pub maximum_participants: usize,
    pub maximum_accepted_facts: usize,
    pub maximum_disclosures: usize,
    pub maximum_comparisons: usize,
    pub maximum_window_ticks: u64,
}

/// Both authenticated owners are required; no client identity or role flag is authority.
pub struct ConversationObservation<'a> {
    pub speech: SpeechObservation<'a>,
    pub source_owner: Option<&'a dyn ConversationSourceOwner>,
    pub limits: ConversationLimits,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ConversationError {
    Speech(SpeechError),
    SourceUnavailable,
    Source(ConversationSourceError),
    Capacity,
    Allocation,
    UnknownConversation,
    TopicChanged,
    SpeakerUnavailable,
    RecipientUnavailable,
    RecipientChanged,
    InvalidWindow,
    WindowNotOpen,
    WindowExpired,
    StalePlan,
    ContextChanged,
    ImplicitDisclosure,
    InvalidDisclosure,
    DuplicateDisclosure,
    SecretOwnership,
    AudienceDenied,
}

/// Immutable proposal, not a canonical transition, knowledge grant or committed reveal.
/// A topic transition requires a newly admitted checkpoint and fresh admission.
pub struct ConversationPlan {
    basis: Basis,
    pins: CheckpointPins,
    conversation: ConversationState,
    permission: ConversationPermission,
    claims: Vec<RecordId>,
    speech: SpeechActPlan,
}

impl fmt::Debug for ConversationPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConversationPlan")
            .finish_non_exhaustive()
    }
}

fn spend(remaining: &mut usize) -> Result<(), ConversationError> {
    *remaining = remaining
        .checked_sub(1)
        .ok_or(ConversationError::Capacity)?;
    Ok(())
}

fn validate_window(
    window: PermissionWindow,
    now: LogicalTime,
    maximum: u64,
) -> Result<(), ConversationError> {
    if window.opens_at.ticks_per_second != now.ticks_per_second
        || window.expires_at.ticks_per_second != now.ticks_per_second
    {
        return Err(ConversationError::InvalidWindow);
    }
    let duration = window
        .expires_at
        .ticks
        .checked_sub(window.opens_at.ticks)
        .ok_or(ConversationError::InvalidWindow)?;
    if duration == 0 {
        return Err(ConversationError::InvalidWindow);
    }
    if duration > maximum {
        return Err(ConversationError::Capacity);
    }
    if now.ticks < window.opens_at.ticks {
        return Err(ConversationError::WindowNotOpen);
    }
    if now.ticks >= window.expires_at.ticks {
        return Err(ConversationError::WindowExpired);
    }
    Ok(())
}

fn validate_permission<'a>(
    permission: &ConversationPermission,
    claims: &[RecordId],
    current: &ConversationObservation<'a>,
) -> Result<&'a ConversationState, ConversationError> {
    let speech = &current.speech;
    speech
        .checkpoint
        .validate_resume(speech.basis, speech.pins)
        .map_err(|error| ConversationError::Speech(SpeechError::Checkpoint(error)))?;
    if permission.recipient != speech.observer {
        return Err(ConversationError::RecipientChanged);
    }
    if claims.len() > speech.limits.maximum_plan_claims
        || permission.disclosures.len() > current.limits.maximum_disclosures
    {
        return Err(ConversationError::Capacity);
    }
    validate_window(
        permission.window,
        speech.checkpoint.state().logical_time,
        current.limits.maximum_window_ticks,
    )?;
    let owner = current
        .source_owner
        .ok_or(ConversationError::SourceUnavailable)?;
    let state = speech.checkpoint.state();
    let mut work = current.limits.maximum_comparisons;
    let mut found = None;
    for conversation in &state.conversations {
        spend(&mut work)?;
        if conversation.id == permission.conversation {
            found = Some(conversation);
            break;
        }
    }
    let conversation = found.ok_or(ConversationError::UnknownConversation)?;
    if conversation.participants.len() > current.limits.maximum_participants
        || conversation.accepted_facts.len() > current.limits.maximum_accepted_facts
    {
        return Err(ConversationError::Capacity);
    }
    if conversation.topic != permission.topic {
        return Err(ConversationError::TopicChanged);
    }
    let mut speaker_found = false;
    for participant in &conversation.participants {
        spend(&mut work)?;
        speaker_found |= *participant == permission.speaker;
    }
    if !speaker_found {
        return Err(ConversationError::SpeakerUnavailable);
    }
    if let ObserverScope::Member(member) = permission.recipient {
        let mut character = None;
        for link in &state.members {
            spend(&mut work)?;
            if link.member == member {
                character = link.character;
                break;
            }
        }
        let character = character.ok_or(ConversationError::RecipientUnavailable)?;
        let mut participant_found = false;
        for participant in &conversation.participants {
            spend(&mut work)?;
            participant_found |= *participant == character;
        }
        if !participant_found {
            return Err(ConversationError::RecipientUnavailable);
        }
    }
    for (index, disclosure) in permission.disclosures.iter().enumerate() {
        for prior in &permission.disclosures[..index] {
            spend(&mut work)?;
            if prior.claim == disclosure.claim {
                return Err(ConversationError::DuplicateDisclosure);
            }
        }
        let mut selected = false;
        for id in claims {
            spend(&mut work)?;
            selected |= *id == disclosure.claim;
        }
        if !selected {
            return Err(ConversationError::InvalidDisclosure);
        }
    }
    for id in claims {
        let mut claim = None;
        for candidate in &state.beliefs {
            spend(&mut work)?;
            if candidate.id == *id {
                claim = Some(candidate);
                break;
            }
        }
        let claim = claim.ok_or(ConversationError::Speech(SpeechError::UnknownClaim))?;
        if claim.holder != permission.speaker {
            return Err(ConversationError::SpeakerUnavailable);
        }
        let mut linked = None;
        for npc in &state.continuity.npcs {
            spend(&mut work)?;
            for secret in &npc.secrets {
                spend(&mut work)?;
                for secret_claim in &secret.claims {
                    spend(&mut work)?;
                    if *secret_claim == *id {
                        if linked.is_some()
                            || secret.holder != claim.holder
                            || npc.entity != secret.holder
                        {
                            return Err(ConversationError::SecretOwnership);
                        }
                        linked = Some(secret);
                    }
                }
            }
        }
        let mut explicit = None;
        for disclosure in &permission.disclosures {
            spend(&mut work)?;
            if disclosure.claim == *id {
                explicit = Some(disclosure);
            }
        }
        match (linked, explicit) {
            (Some(secret), Some(disclosure)) if secret.policy == disclosure.policy => {}
            (Some(_), None) => return Err(ConversationError::ImplicitDisclosure),
            (None, None) => {}
            _ => return Err(ConversationError::InvalidDisclosure),
        }
    }
    owner
        .validate(speech.basis, speech.pins, conversation, permission, claims)
        .map_err(ConversationError::Source)?;
    Ok(conversation)
}

fn retain_conversation(
    current: &ConversationState,
) -> Result<ConversationState, ConversationError> {
    let mut participants = Vec::new();
    participants
        .try_reserve_exact(current.participants.len())
        .map_err(|_| ConversationError::Allocation)?;
    participants.extend_from_slice(&current.participants);
    let mut accepted_facts = Vec::new();
    accepted_facts
        .try_reserve_exact(current.accepted_facts.len())
        .map_err(|_| ConversationError::Allocation)?;
    accepted_facts.extend_from_slice(&current.accepted_facts);
    Ok(ConversationState {
        id: current.id,
        participants,
        topic: current.topic.clone(),
        accepted_facts,
    })
}

/// Admit only source-authorized scope and claims currently permitted to its exact recipient.
/// This never changes SecretPolicy, claim/evidence audiences or canonical conversation state.
pub fn admit_conversation(
    permission: ConversationPermission,
    proposal: SpeechProposal,
    current: &ConversationObservation<'_>,
) -> Result<ConversationPlan, ConversationError> {
    if proposal.claims.len() > current.speech.limits.maximum_plan_claims {
        return Err(ConversationError::Capacity);
    }
    let mut claims = Vec::new();
    claims
        .try_reserve_exact(proposal.claims.len())
        .map_err(|_| ConversationError::Allocation)?;
    claims.extend(proposal.claims.iter().map(|claim| claim.claim));
    let conversation = retain_conversation(validate_permission(&permission, &claims, current)?)?;
    let speech = admit_speech(proposal, &current.speech).map_err(ConversationError::Speech)?;
    let context =
        project_expression_context(&speech, &current.speech).map_err(ConversationError::Speech)?;
    if context.claims().len() != claims.len() {
        return Err(ConversationError::AudienceDenied);
    }
    Ok(ConversationPlan {
        basis: current.speech.basis,
        pins: current.speech.pins.clone(),
        conversation,
        permission,
        claims,
        speech,
    })
}

/// Audience-safe expression remains private so later consumption must revalidate
/// conversation permissions as well as the existing speech/Knowledge projection.
pub struct ConversationContext<'a> {
    plan: &'a ConversationPlan,
    expression: ExpressionContext<'a>,
}

impl fmt::Debug for ConversationContext<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConversationContext")
            .finish_non_exhaustive()
    }
}

impl<'a> ConversationContext<'a> {
    pub fn claims(&self) -> &[ExpressionClaim<'a>] {
        self.expression.claims()
    }
    pub fn metadata(&self) -> ExpressionMetadata<'_> {
        self.expression.metadata()
    }
    pub fn speaker(&self) -> EntityId {
        self.plan.permission.speaker
    }
    pub fn validate_current(
        &self,
        current: &ConversationObservation<'_>,
    ) -> Result<(), ConversationError> {
        project_conversation(self.plan, current)?;
        self.expression
            .validate_current(&current.speech)
            .map_err(ConversationError::Speech)
    }
}

/// Revalidate current participants/topic, finite logical window, both source owners
/// and every selected claim before exposing any text. Refusals have no partial output.
pub fn project_conversation<'a>(
    plan: &'a ConversationPlan,
    current: &ConversationObservation<'a>,
) -> Result<ConversationContext<'a>, ConversationError> {
    if plan.basis != current.speech.basis || &plan.pins != current.speech.pins {
        return Err(ConversationError::StalePlan);
    }
    let conversation = validate_permission(&plan.permission, &plan.claims, current)?;
    if conversation != &plan.conversation {
        return Err(ConversationError::ContextChanged);
    }
    let expression = project_expression_context(&plan.speech, &current.speech)
        .map_err(ConversationError::Speech)?;
    if expression.claims().len() != plan.claims.len() {
        return Err(ConversationError::AudienceDenied);
    }
    Ok(ConversationContext { plan, expression })
}
