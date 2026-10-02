use std::mem::size_of;
use std::time::Duration;

use df_model::checkpoint::{
    Basis, CatchUpCursor, Checkpoint, CheckpointError, CheckpointPins, ContentReference,
    LogicalTime, ScheduledEvent,
};

const MAX_QUEUE_EVENTS: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DueSelectionLimits {
    pub queue_events: usize,
    pub selected_events: usize,
    pub output_bytes: usize,
}

/// The expected canonical checkpoint basis and already-admitted content policy.
/// Deadline remaining is supplied by the native owner; no clock is read here.
pub struct DueSelectionRequest<'a> {
    pub expected_basis: Basis,
    pub target_time: LogicalTime,
    pub paused: bool,
    pub deadline_remaining: Duration,
    pub policy: &'a ContentReference,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DueSelectionError {
    InvalidLimits,
    InvalidTime,
    Deadline,
    QueueCapacity,
    OutputCapacity,
    StaleCursor,
    AllocationCapacity,
    Checkpoint(CheckpointError),
}

/// Owned canonical proposals against the exact checkpoint pins, with no effects.
/// The engine/session owner commits time, consequences and catch-up atomically,
/// rechecks the basis/pins and deduplicates stable continuation operation IDs.
#[derive(Debug, PartialEq, Eq)]
pub struct DueSelection<'a> {
    pub basis: Basis,
    pub pins: &'a CheckpointPins,
    pub proposed_time: LogicalTime,
    pub events: Vec<ScheduledEvent>,
    pub cursor: CatchUpCursor,
    pub accounted_output_bytes: usize,
}

impl DueSelection<'_> {
    pub fn remaining_due(&self) -> usize {
        self.cursor.pending_events.len()
    }
}

fn reference_bytes(reference: &ContentReference) -> Result<usize, DueSelectionError> {
    reference
        .package
        .retained_heap_bytes()
        .checked_add(reference.entry.retained_heap_bytes())
        .ok_or(DueSelectionError::OutputCapacity)
}

/// Selects the due prefix ordered by logical tick and canonical RecordId bytes.
/// Events at the target are due; future records are never reported as backlog.
/// Retains the immutable queue, including its last processed record, for cursor
/// reconciliation. The canonical pending list must exactly match the ordered due
/// remainder at the checkpoint time; changing the queue requires owner reconciliation.
/// Pause reports backlog without consuming events. Selection never applies effects.
/// Input work is capped at 4096 schedules. Output accounting includes owned vector
/// capacities and cloned reference capacities, but is not a serialized-size claim.
/// The native owner still enforces its actual execution deadline.
pub fn select_due_events<'a>(
    checkpoint: &'a Checkpoint,
    admitted_pins: &CheckpointPins,
    request: DueSelectionRequest<'_>,
    limits: DueSelectionLimits,
) -> Result<DueSelection<'a>, DueSelectionError> {
    if limits.queue_events == 0
        || limits.queue_events > MAX_QUEUE_EVENTS
        || limits.selected_events == 0
        || limits.selected_events > limits.queue_events
        || limits.output_bytes == 0
    {
        return Err(DueSelectionError::InvalidLimits);
    }
    if request.deadline_remaining.is_zero() {
        return Err(DueSelectionError::Deadline);
    }
    let state = checkpoint.state();
    if state.schedules.len() > limits.queue_events {
        return Err(DueSelectionError::QueueCapacity);
    }
    checkpoint
        .validate_resume(request.expected_basis, admitted_pins)
        .map_err(DueSelectionError::Checkpoint)?;
    let current = state.logical_time;
    if current.ticks_per_second == 0
        || request.target_time.ticks_per_second != current.ticks_per_second
        || request.target_time.ticks < current.ticks
        || (request.paused && request.target_time != current)
        || state
            .schedules
            .iter()
            .any(|event| event.due.ticks_per_second != current.ticks_per_second)
    {
        return Err(DueSelectionError::InvalidTime);
    }
    if request.policy.package != checkpoint.pins().content.package {
        return Err(DueSelectionError::Checkpoint(
            CheckpointError::ContentMismatch,
        ));
    }
    let mut ordered = Vec::new();
    ordered
        .try_reserve_exact(state.schedules.len())
        .map_err(|_| DueSelectionError::AllocationCapacity)?;
    ordered.extend(state.schedules.iter());
    ordered.sort_unstable_by_key(|event| (event.due.ticks, event.id));
    let previous = if let Some(cursor) = &state.continuity.catch_up {
        if cursor.policy != *request.policy
            || cursor.time != current
            || cursor.pending_events.len() > limits.queue_events
        {
            return Err(DueSelectionError::StaleCursor);
        }
        let key = match cursor.last_processed {
            Some(id) => {
                let event = ordered
                    .iter()
                    .find(|event| event.id == id)
                    .ok_or(DueSelectionError::StaleCursor)?;
                if event.due.ticks > current.ticks {
                    return Err(DueSelectionError::StaleCursor);
                }
                Some((event.due.ticks, event.id))
            }
            None => None,
        };
        let remaining = ordered
            .iter()
            .filter(|event| {
                event.due.ticks <= current.ticks
                    && key.is_none_or(|key| (event.due.ticks, event.id) > key)
            })
            .map(|event| event.id);
        if !remaining.eq(cursor.pending_events.iter().copied()) {
            return Err(DueSelectionError::StaleCursor);
        }
        key
    } else {
        None
    };
    let due = |event: &&ScheduledEvent| {
        event.due.ticks <= request.target_time.ticks
            && previous.is_none_or(|key| (event.due.ticks, event.id) > key)
    };
    let mut events = Vec::new();
    events
        .try_reserve_exact(limits.selected_events.min(state.schedules.len()))
        .map_err(|_| DueSelectionError::AllocationCapacity)?;
    let mut pending_events = Vec::new();
    pending_events
        .try_reserve_exact(state.schedules.len())
        .map_err(|_| DueSelectionError::AllocationCapacity)?;
    let policy = request.policy.clone();
    let mut bytes = size_of::<DueSelection<'_>>()
        .checked_add(reference_bytes(&policy)?)
        .and_then(|bytes| {
            size_of::<ScheduledEvent>()
                .checked_mul(events.capacity())
                .and_then(|records| bytes.checked_add(records))
        })
        .and_then(|bytes| {
            size_of::<df_model::checkpoint::RecordId>()
                .checked_mul(pending_events.capacity())
                .and_then(|records| bytes.checked_add(records))
        })
        .ok_or(DueSelectionError::OutputCapacity)?;
    if bytes > limits.output_bytes {
        return Err(DueSelectionError::OutputCapacity);
    }
    // Individually admitted records must fit an empty selected batch with its cursor.
    for event in &ordered {
        if reference_bytes(&event.definition)? > limits.output_bytes - bytes {
            return Err(DueSelectionError::OutputCapacity);
        }
    }
    let mut last_processed = previous.map(|key| key.1);
    let mut stopped = request.paused;
    for event in ordered.iter().copied().filter(due) {
        let cost = reference_bytes(&event.definition)?;
        if stopped || events.len() == limits.selected_events || cost > limits.output_bytes - bytes {
            stopped = true;
            pending_events.push(event.id);
            continue;
        }
        let selected = event.clone();
        let retained = reference_bytes(&selected.definition)?;
        if retained > limits.output_bytes - bytes {
            return Err(DueSelectionError::OutputCapacity);
        }
        bytes += retained;
        last_processed = Some(event.id);
        events.push(selected);
    }
    Ok(DueSelection {
        basis: checkpoint.basis(),
        pins: checkpoint.pins(),
        proposed_time: request.target_time,
        events,
        cursor: CatchUpCursor {
            last_processed,
            pending_events,
            policy,
            time: request.target_time,
        },
        accounted_output_bytes: bytes,
    })
}
