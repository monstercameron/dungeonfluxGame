# df-world D02: Due-event ordering and bounded catch-up

Task/attempt: `B-C-df-world-D02` / `B-C-df-world-D02-a1`

Input revision: `d870db00dc349425e8f977bdfb20c6ffb24bfd3e`

Status: Bounded source decision and executable example; independent review and
integrated consumer evidence remain required.

The original acceptance criteria are preserved:

- accepted ticks have bounded deterministic order
- The named outcome has actual source/build-bound evidence; unsupported, pending,
  failed and unperformed checks remain explicit.

## Governing authority and decision

[Runtime directors](runtime-directors.md), “Authority and composition”, “Pure
public boundaries” and “Time, recovery and bounded work”, requires an immutable
basis, pure owned proposals, accepted logical time, unique due-event IDs,
deterministic order and explicit remaining recovery work. [Long-horizon
state](long-horizon-state.md), “World granularity and pause” and “Acceptance”,
requires bounded event/CPU/output work, version-bound catch-up cursors, deduplicated
committed continuations and no wall-time advance while paused. [Subsystem
architecture](subsystem-architecture.md), “Server crates”, and [Subsystem
interfaces](subsystem-interfaces.md), “Common contract rules”, “Separate runtime
subsystem boundaries” and “Identity and session ownership”, keep state in
`df-model`, policy in `df-content`, pure staging in `df-world`/`df-engine`, and the
sole fenced PostgreSQL commit in `df-session`. The runtime-directors refinement
governs composition without recursive world/rules callbacks.

For one admitted, immutable due-event queue basis, sort lexicographically by
`(due logical tick, stable unique event ID)`. The ID tie break must have one frozen
canonical ordering; arrival order, actor iteration and wall time are irrelevant.
Events at the target tick are due. Return only a prefix after the last processed
key, stopping before the event-count or output-byte bound. Do not skip an expensive
earlier event to fit a later event. Return the last emitted key and the exact count
of still-due events in that admitted queue. Later events are not due backlog.

The cursor binds the immutable queue and its source/content/policy/run basis;
changing that basis requires explicit cursor reconciliation, not reuse. The
example's opaque basis token represents that complete check, not a new production
revision type. A cursor outside its queue or from another basis is refused.
Stable IDs are unique across the supplied queue, including already processed
events. Conflicting duplicate IDs are refused before any output is staged.

An accepted time advance may stage its target logical tick with a due backlog.
The engine/session must commit that backlog and cursor with the selected world
delta; time advance is not evidence that every consequence has completed. A
decision depending on still-due consequences remains explicitly pending; the
accepted target cannot bypass those causal/rules prerequisites. A later
same-tick catch-up continuation processes the next prefix against the committed
cursor; it is a separate accepted continuation, not a zero-duration time advance.
A paused request cannot increase the tick or consume due events; it exposes the
backlog without processing it. No wall-clock value enters this boundary.

Input, work and output limits are positive and mutually feasible: an individually
admitted event must fit an empty output batch. Refuse oversize input or an event
that cannot fit; do not loop forever on a zero-progress continuation. This example
uses at most four supplied queue records, two emitted IDs and 24 accounted payload
bytes, with each event at most 16 bytes. Validation, sorting and selection inspect
only these four records; no queue-wide unbounded scan or timer is implied. Those
numbers bound this fixture only. G10/G11/G12 still select and measure production
capacity/latency/encoded-byte bounds, and the native owner enforces execution
deadlines. Production output accounting must include all retained delta/cursor/
diagnostic bytes; a payload-size field alone cannot prove its serialized size.

`df-world` proposes IDs, logical time, cursor and backlog facts; it does not apply
effects, publish facts, roll dice, grant resources or invoke a provider. Mechanical
consequences remain source-legal through rules preparation/resolution in the
engine's candidate decision. Rejecting the whole decision publishes no candidate
change. Session fencing and stable operation/continuation IDs make duplicate
delivery return its committed result without a second effect; repeating this
pure example only reproduces a proposal. Diagnostic counts/refusal classifications
go to the native shared OTEL path, never a second logger or recovery authority.

## Exact bounded Rust example

This complete Rust 2024 executable uses private fixture-only tuples and errors.
It admits no public type, serializer, rules catalog, crate API or implementation.
An event tuple is `(due tick, stable ID, accounted fixture payload bytes)`; a cursor
is `(opaque complete basis token, last ordered event key)`. All time is supplied.
Assertions exercise the literal decision; CLI success is not production or player
experience qualification.

```rust
#[derive(Debug, PartialEq, Eq)]
enum Refusal {
    InvalidTime,
    Capacity,
    DuplicateId,
    StaleCursor,
}

#[derive(Debug, PartialEq, Eq)]
struct Batch {
    proposed_tick: u64,
    event_ids: Vec<u64>,
    last_key: Option<(u64, u64)>,
    remaining_due: usize,
}

fn advance(
    events: &[(u64, u64, u16)],
    current_tick: u64,
    target_tick: u64,
    paused: bool,
    basis: u64,
    cursor: Option<(u64, (u64, u64))>,
) -> Result<Batch, Refusal> {
    if target_tick < current_tick || (paused && target_tick != current_tick) {
        return Err(Refusal::InvalidTime);
    }
    if events.len() > 4 || events.iter().any(|event| event.2 > 16) {
        return Err(Refusal::Capacity);
    }
    for (index, event) in events.iter().enumerate() {
        if events
            .iter()
            .skip(index + 1)
            .any(|other| event.1 == other.1)
        {
            return Err(Refusal::DuplicateId);
        }
    }
    let previous = match cursor {
        Some((cursor_basis, key)) => {
            if cursor_basis != basis
                || key.0 > current_tick
                || !events.iter().any(|event| (event.0, event.1) == key)
            {
                return Err(Refusal::StaleCursor);
            }
            Some(key)
        }
        None => None,
    };
    let mut due: Vec<_> = events
        .iter()
        .copied()
        .filter(|event| {
            event.0 <= target_tick && previous.is_none_or(|key| (event.0, event.1) > key)
        })
        .collect();
    due.sort_unstable_by_key(|event| (event.0, event.1));
    let mut batch = Batch {
        proposed_tick: target_tick,
        event_ids: Vec::new(),
        last_key: previous,
        remaining_due: due.len(),
    };
    let mut bytes = 0_u16;
    if !paused {
        for event in due {
            if batch.event_ids.len() == 2 || bytes + event.2 > 24 {
                break;
            }
            bytes += event.2;
            batch.event_ids.push(event.1);
            batch.last_key = Some((event.0, event.1));
            batch.remaining_due -= 1;
        }
    }
    Ok(batch)
}

fn main() {
    let events = [(10, 3, 16), (10, 1, 16), (10, 2, 8), (12, 4, 8)];
    let first = advance(&events, 9, 10, false, 7, None);
    assert_eq!(
        first,
        Ok(Batch {
            proposed_tick: 10,
            event_ids: vec![1, 2],
            last_key: Some((10, 2)),
            remaining_due: 1,
        })
    );
    let shuffled = [events[3], events[2], events[0], events[1]];
    assert_eq!(advance(&shuffled, 9, 10, false, 7, None), first);
    assert_eq!(advance(&events, 9, 10, false, 7, None), first);
    let cursor = Some((7, (10, 2)));
    assert_eq!(
        advance(&events, 10, 10, false, 7, cursor),
        Ok(Batch {
            proposed_tick: 10,
            event_ids: vec![3],
            last_key: Some((10, 3)),
            remaining_due: 0,
        })
    );
    assert_eq!(
        advance(&events, 10, 12, false, 7, Some((7, (10, 3)))),
        Ok(Batch {
            proposed_tick: 12,
            event_ids: vec![4],
            last_key: Some((12, 4)),
            remaining_due: 0,
        })
    );
    assert_eq!(
        advance(&events, 10, 10, true, 7, None),
        Ok(Batch {
            proposed_tick: 10,
            event_ids: vec![],
            last_key: None,
            remaining_due: 3,
        })
    );
    let byte_bound = [(10, 1, 16), (10, 2, 16), (10, 3, 1)];
    assert_eq!(
        advance(&byte_bound, 9, 10, false, 7, None),
        Ok(Batch {
            proposed_tick: 10,
            event_ids: vec![1],
            last_key: Some((10, 1)),
            remaining_due: 2,
        })
    );
    let count_bound = [(10, 1, 8), (10, 2, 8), (10, 3, 8)];
    assert_eq!(
        advance(&count_bound, 9, 10, false, 7, None),
        Ok(Batch {
            proposed_tick: 10,
            event_ids: vec![1, 2],
            last_key: Some((10, 2)),
            remaining_due: 1,
        })
    );
    assert_eq!(
        advance(&events, 12, 12, false, 7, Some((7, (12, 4)))),
        Ok(Batch {
            proposed_tick: 12,
            event_ids: vec![],
            last_key: Some((12, 4)),
            remaining_due: 0,
        })
    );
    assert_eq!(
        advance(&events, 10, 9, false, 7, None),
        Err(Refusal::InvalidTime)
    );
    assert_eq!(
        advance(&events, 10, 11, true, 7, None),
        Err(Refusal::InvalidTime)
    );
    assert_eq!(
        advance(&events, 10, 10, false, 8, cursor),
        Err(Refusal::StaleCursor)
    );
    assert_eq!(
        advance(&events, 10, 10, false, 7, Some((7, (10, 99)))),
        Err(Refusal::StaleCursor)
    );
    assert_eq!(
        advance(&events, 10, 12, false, 7, Some((7, (12, 4)))),
        Err(Refusal::StaleCursor)
    );
    assert_eq!(
        advance(&[(10, 1, 8), (11, 1, 8)], 9, 12, false, 7, None),
        Err(Refusal::DuplicateId)
    );
    assert_eq!(
        advance(&[(10, 1, 17)], 9, 10, false, 7, None),
        Err(Refusal::Capacity)
    );
    assert_eq!(
        advance(&[(10, 1, 8); 5], 9, 10, false, 7, None),
        Err(Refusal::Capacity)
    );
    assert_eq!(events, [(10, 3, 16), (10, 1, 16), (10, 2, 8), (12, 4, 8)]);
    println!("PASS: deterministic bounded prefixes, continuation, pause and typed refusals");
}
```

## Alternatives and next actual consumer

Arrival order was rejected because restart and scheduling permutations change
consequences. Wall-clock catch-up was rejected because stopped campaigns cannot
advance threats. Draining all due work was rejected because it has no finite work
bound. Skipping events that do not fit was rejected because it changes causal
order. A new world timer/DB owner was rejected because engine/session owns admission
and commit. This decision creates none of those implementations.

At the input revision the workspace has no `df-model`, `df-content`, `df-world`,
`df-engine` or `df-session` crate. The next actual source consumer is the canonical
`B-C-df-world-I01`, “Implement due-event selection on supplied state” (capped work
returns a resumable cursor), followed by its engine/session connection;
there is no reachable production caller yet. Its minimum coordinated prerequisites
are owner-approved shared time/event identity/order and version-bound queue/cursor/
delta records in `crates/df-model/src/`, immutable limits/source event policy in
`crates/df-content/src/`, and the pure ordered advance in `crates/df-world/src/`.
Existing `df-types::{SessionId, RunId, OperationId, SessionRevision}` remain canonical
for those admitted identities/revision roles; the local scalars here do not replace
them. Exact new fields, modules and any additional shared units remain G03 owner
decisions. No public types are frozen by this example.

The subsequent connection belongs in `crates/df-engine/src/` candidate composition
and `crates/df-session/src/` serialized fenced commit/continuation admission. Their
owners must prove cursor/state/ordered consequences are committed atomically,
deduplicate stable operation and continuation IDs, reject stale run/recovery/basis,
and retain backlog on restart. No manifest, shared schema or source path above is
changed by D02. The future F35 WORLD-TIME-ACCEPT fixture must also cover travel,
threats, overnight pause, duplicate delivery/fencing and listener-safe projections;
player continuity/trust requires independent integrated frontier playtests.

## Verification scope

The retained handoff binds the exact Markdown Rust literal, extracted source,
pinned `1.98.1-aarch64-apple-darwin` rustfmt/rustc, root `rustfmt.toml`, argv,
guard receipts and executable output. Extraction must compare byte for byte;
format/check uses the root config; rustc uses Rust 2024 and `-D warnings`.
Execution checks the valid and refusal assertions above under the frozen finite
guard only after coordinator release. No Cargo, Clippy, WASM, database, provider,
live timer, production rule/catalog, integrated consumer or browser qualification
is implied. Pending, failed and unperformed checks stay explicit in the handoff;
independent review and integrated source verification belong to the coordinator.
