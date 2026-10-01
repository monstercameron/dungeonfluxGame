# G10-D03: Replay compatibility and logical-time policy

Status: Design decision for G10; production types, reducers, migrations and
integrated evidence remain pending their owning gates.

This document resolves the G10-D03 design outcome: accepted logical time and the
compatibility conditions under which recovery may replay a campaign. It does not
define a second state writer, persistence format, timer service, or production
Rust API. The owning contract path is `df-model` for versioned state/decision
records, `df-content` for immutable versioned authoring data, and `df-engine` for
deterministic candidate transitions. `df-session`/storage retain commit,
checkpoint, fencing and migration authority.

## Decision

### Accepted logical time

World/game time is canonical integer logical time with an explicit unit in its
eventual `df-types` contract. It advances only as part of an authorized, validated
and durably accepted transition, such as travel, rest, or an explicit time-advance
decision. The accepted transition records its resulting logical time and due-event
ordering inputs. A rejected, stale, unauthorized, or uncommitted proposal changes
neither canonical time nor scheduled world consequences.

Paused and stopped campaigns do not derive world time from process uptime, wall
clock, reconnect duration, browser visibility, or presentation frames. Pause and
resume are authorized host/session operations and are committed with the campaign
revision. Resume does not convert elapsed wall time into game time. It makes
already committed due work eligible for bounded catch-up, ordered by logical due
time and stable event identity; any remaining work is explicit and resumable.
Accepted travel/rest/time-advance may move time while a campaign is active, subject
to source and rules validation. Presentation time is a separate monotonic cosmetic
anchor; interpolation, narration and cinematic pauses cannot advance world time,
consume rules timers, delay legal input, or alter an accepted result.

This preserves player agency and source authority: elapsed real time cannot harm a
paused character or create an unaccepted consequence. A rules timer advances only
when the owning accepted game transition says so. Catch-up work is bounded per
decision and is committed through stable deduplicated identities, never an
unbounded restart loop.

### Replay and compatibility

Recovery starts from a committed versioned snapshot/checkpoint and the ordered
committed decision/fact/effect-intent records after it. Each replay input must
retain, at minimum, the campaign/run identity, expected revision, schema and
snapshot versions, immutable content-pack hash/version, source/catalog and
ruleset revisions, relevant director/policy and reducer versions, accepted logical
time, causal ordering identifiers, actual ordered draws, and accepted semantic
outputs. Build/configuration provenance is retained for diagnosis and evidence;
runtime replay does not call an LLM, reroll dice, regenerate content, or invoke a
paid provider to fill missing history.

There are two deliberately different recovery claims:

1. **Recorded-decision recovery** applies the stored accepted outcomes to the
   matching compatible state contract. It reuses the actual recorded choices,
   draws and semantic outputs. It never asks a model to choose again.
2. **Deterministic reconstruction** reruns a reducer only when every required
   schema, content/rules/policy revision and reducer is supported and the record
   contains every deterministic input. The recovered checkpoint/state hash must
   match the recorded hash at each required boundary. A matching current version
   label alone is insufficient.

An old record is compatible only if the current implementation explicitly
supports its complete required version set, or a reviewed, deterministic,
source/state/effect-compatible migration chain maps it to a supported checkpoint.
Migration is explicit, versioned and hash-checked; it cannot silently reinterpret
field meaning, replace missing content, retcon committed facts, or invent draws.
The session/storage owner commits migrated state behind its normal revision fence.
If support, provenance, a required reducer/input, or a hash check is absent or
fails, return a typed unsupported-replay/migration gap with the affected version
and checkpoint. Do not partially publish reconstructed state as success. Start a
new run only through an explicit authorized operation.

Redacted or rights-revoked private payloads are not regenerated during replay.
Replay reports the resulting scoped gap while retaining only the minimum
authorized nonprivate decision provenance. Optional presentation replay is a
separate claim: it additionally requires retained cue, asset and presentation
timebase references; missing assets limit presentation recovery but never change
canonical game outcomes. Cache misses in prepared-only or replay mode make no live
provider calls.

### Alternatives considered

- **Advance from wall time, then catch up on resume:** rejected because pause,
  downtime and reconnect would create unaccepted world consequences and could
  damage paused characters.
- **Regenerate missing narrative/semantic results during replay:** rejected
  because it changes history, can disclose revoked information, introduces
  nondeterminism and may incur provider calls or cost.
- **Treat any older version as compatible by best-effort decoding:** rejected
  because field meaning and reducer behavior can change without a detectable gap.
- **Replay only snapshots and ignore decision provenance:** rejected for
  deterministic reconstruction because snapshots cannot establish the accepted
  causal inputs, draws or semantic outputs after the checkpoint.

## Illustrative std-only contract example

The literal example below demonstrates this policy boundary only. Its structs and
functions are not frozen application APIs, persistence codecs, or evidence of
production reducer/migration support. `AcceptedTime` is represented in
milliseconds solely to make the unit visible in this example.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AcceptedTime(u64); // milliseconds in this illustrative contract

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CampaignStatus {
    Active,
    Paused,
    Stopped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReplayVersions {
    snapshot: u16,
    content: u16,
    rules: u16,
    reducer: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    NotAuthorized,
    CampaignNotActive,
    UnsupportedReplayVersion,
    TimeOverflow,
}

fn accept_time_advance(
    current: AcceptedTime,
    requested_ms: u64,
    authorized: bool,
    status: CampaignStatus,
) -> Result<AcceptedTime, Refusal> {
    if !authorized {
        return Err(Refusal::NotAuthorized);
    }
    if status != CampaignStatus::Active {
        return Err(Refusal::CampaignNotActive);
    }
    current
        .0
        .checked_add(requested_ms)
        .map(AcceptedTime)
        .ok_or(Refusal::TimeOverflow)
}

fn validate_replay_versions(
    recorded: ReplayVersions,
    supported: ReplayVersions,
) -> Result<(), Refusal> {
    if recorded == supported {
        Ok(())
    } else {
        Err(Refusal::UnsupportedReplayVersion)
    }
}

fn main() {
    let now = AcceptedTime(1_000);
    assert_eq!(
        accept_time_advance(now, 500, true, CampaignStatus::Active),
        Ok(AcceptedTime(1_500))
    );
    assert_eq!(
        accept_time_advance(now, 500, true, CampaignStatus::Paused),
        Err(Refusal::CampaignNotActive)
    );
    assert_eq!(
        accept_time_advance(now, 500, true, CampaignStatus::Stopped),
        Err(Refusal::CampaignNotActive)
    );
    assert_eq!(
        accept_time_advance(now, 500, false, CampaignStatus::Active),
        Err(Refusal::NotAuthorized)
    );
    assert_eq!(
        accept_time_advance(AcceptedTime(u64::MAX), 1, true, CampaignStatus::Active),
        Err(Refusal::TimeOverflow)
    );

    let recorded = ReplayVersions {
        snapshot: 2,
        content: 4,
        rules: 7,
        reducer: 3,
    };
    assert_eq!(validate_replay_versions(recorded, recorded), Ok(()));
    assert_eq!(
        validate_replay_versions(
            recorded,
            ReplayVersions {
                reducer: 4,
                ..recorded
            }
        ),
        Err(Refusal::UnsupportedReplayVersion)
    );
}
```

The example's refusal cases are unauthorized time change, pause, stopped campaign,
overflow, and an unsupported reducer version. It intentionally does not claim that equality is a
sufficient production compatibility test: actual compatibility also binds all
required schema/source/policy inputs and verified state hashes as specified above.

## Acceptance and unresolved production work

This policy satisfies the design criterion **accepted logical time** by making
logical time a committed result of an authorized active-campaign transition and
refusing time advance while paused or stopped. The bounded example exercises both
acceptance and refusal paths. Before dependent production work can claim G10
complete, the owners must still freeze concrete versioned state/content/decision
schemas and typed outcomes; ordered event, semantic and draw provenance; reducer
coverage and hash fixtures; deterministic migration policy and storage commit
boundary; and supported bounds/ordering for resume catch-up. No migration chain,
production reducer, durable checkpoint implementation, or integrated pause/resume
behavior is delivered here. Existing plan criteria remain: unsupported versions
must be explicit gaps, replay must make zero provider calls, and integrated
recovery/pause behavior needs source/build-bound evidence.

The literal example is to be checked with the repository-pinned rustfmt and
`rustc --edition 2024 -D warnings` by the coordinator after resource admission.
This worker has not run a compiler, formatter, Cargo, browser, or application
check; no workspace/runtime behavior is claimed verified. The document and its
example are not a substitute for G03/G05/G07/G10 contracts or the roadmap's
integrated S05 replay and resume acceptance.

## Governing sources

- [Implementation roadmap](implementation-roadmap.md): G10 prerequisite decision,
  runtime-director delivery, S05 continuity and replay, and integration evidence.
- [Subsystem interfaces](subsystem-interfaces.md): pure engine inputs, explicit
  logical time, recorded draws/provenance, versioned state, and unsupported replay.
- [Runtime directors](runtime-directors.md): accepted time advances, pause policy,
  committed decision records, bounded catch-up, no-provider replay and explicit
  gaps.
- [Campaign authoring](campaign-authoring.md): immutable pack versions and
  explicit migration for live content changes.
- [Long-horizon state](long-horizon-state.md): wall-clock pause, due-event
  ordering, bounded catch-up and replay without generation.
- [Campaign cinematics](campaign-cinematics.md): cosmetic pauses cannot alter
  mechanical authority or accepted outcomes.
- [Generated content](generated-content.md): replay pins chosen definitions and
  handler revisions; content changes require migration or explicit rejection.
- [Coding style](coding-style.md), [ADR 0001](../ADR/0001-sqlite-agent-workflow.md),
  [ADR 0002](../ADR/0002-resource-scheduling-and-cleanup.md),
  [ADR 0003](../ADR/0003-agent-devlog.md),
  [ADR 0004](../ADR/0004-development-reliability.md), and
  [ADR 0005](../ADR/0005-frontier-output-evaluation.md): bounded work, precise
  handoff, append-only observations, meaningful verification, and explicit gaps.
