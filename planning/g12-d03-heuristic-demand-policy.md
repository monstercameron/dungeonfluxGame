# G12-D03: Heuristic predictive demand policy

Status: Design decision; production contracts, policy values and calibration pending

## Decision

Predictive demand is a bounded ordering aid for optional reusable presentation assets. It answers “which already-permitted candidate should be considered first?” It does not estimate a calibrated probability, likelihood, confidence, expected utility, guaranteed reuse, fun, or user intent. A rank/score is comparable only within its named forecast policy revision and candidate set; it has no cross-version or percentage interpretation. Labels, APIs, telemetry and UI must call it a heuristic rank/score, never a probability or confidence. Even retrospective hit rates describe the measured cohort and do not turn an individual score into a calibrated probability.

`df-experience` may contribute only authorized, consented pacing observations and committed activity windows. `df-tempo` may supply its current versioned, bounded frame/profile and approved cue semantics. `df-presentation` builds candidates from committed or otherwise explicitly permitted moments, produces audience-scoped forecast candidates and ranks them with versioned bounded heuristic inputs. `df-media` considers only an admitted typed `AssetDemand`; it owns queueing, mode, budgets, provider dispatch, expiry, cancellation and reconciliation. Forecasting never dispatches generation, spends a budget or changes game state. These are the existing owners, not a second predictor/service.

## Forecast inputs and output

A forecast request binds the session/run, current committed state revision, presentation epoch, content/identity revision, policy revision, trusted audience/access generation and an explicit logical/presentation time basis. It also names its bounded horizon and candidate limit. Candidate evidence may include permitted public or audience-known semantic cues, current authorized profile, asset readiness/reusability, explicit dependency state and observed demand outcomes from an authorized aggregate. Tentative intent may prioritize only cheap cancellable local lookup; it cannot cause an action, public cue, irreversible commitment or unapproved cost.

Filter access and audience scope before scoring, serialization, telemetry fields, candidate identifiers exposed to a client, or prefetch manifests. A hidden branch, secret stage, private belief/HP, another participant’s private preference, raw microphone, inferred emotion, or unconsented behavior cannot affect a public forecast. Paired states that differ only in unauthorized hidden data must yield identical public forecast output and demand. A separately authorized private audience may receive its own profile and forecast. Correlation/trace identifiers do not grant access.

The output is a bounded, deterministically ordered set of `ForecastCandidate` values carrying the typed asset/demand reference, ordinal heuristic score or rank, score-policy revision, source basis, audience scope, eligibility explanation code, expiry and any declared dependency. Stable tie-breaking is required. Missing inputs remain unknown; do not silently convert them to zero or invent a signal. Candidate count, horizon and score arithmetic are bounded. Values are tuning inputs, not rules mechanics, deadlines, or measurements of player emotion. The score must not encode sensitive information in ways observable through ordering, timing, cache behavior, or asset availability.

## Admission, cost and cancellation

Forecasting and media admission are separate stages. Before a candidate can enter an optional queue, the session/media boundary revalidates its complete basis against current committed run/state/content/policy/identity and audience/access generation, confirms the demand is still eligible, and checks the relevant cancellation/job generation. A mismatch returns a typed stale/superseded outcome; it is not retried against new state implicitly. Recompute from the newly authorized basis if useful. Filter audience scope again immediately before any manifest or provider input.

Optional prefetch is strictly lower priority than interaction-critical speech/input and required prepared fallback. It is subject to explicit campaign authorization, a conservative versioned estimate, budget reservation, per-campaign/provider concurrency, byte, deadline, horizon, queue-aging/fairness and expiry limits. The cost owner must resolve the actual quote, reservation, allowance and liability contracts under G08 before production admission. No unpriced or unknown cost is treated as free. Admission must not consume a protected required-output allowance or another audience’s allowance. Denial, saturation, estimate overflow, exhausted allowance or uncertain authorization yields a typed defer/refusal and the current best available permitted presentation; no retry loop may evade the cap.

Before dispatch, cancellation or invalidation retires unstarted speculative demand. After dispatch, cancellation is best effort: it cannot assert a provider refund, erase an already-started cost, or roll back a committed decision. Preserve the job identity and reservation; record actual, cancellation-fee or unknown supplier usage and reconcile conservatively. Do not automatically repeat an ambiguous non-idempotent paid call. Generation/job/basis fences reject stale completions, and a late result cannot publish into a replaced audience, run, identity revision or cue epoch. Expiry makes an unused asset ineligible for this demand; it does not delete assets still referenced elsewhere or release unknown spend.

## Prepared-only and replay

`prepared_only` may select a verified, complete, audience-authorized prepared asset or matching fallback already available under its full versioned identity key. On a miss, return typed unavailability and render the best permitted prepared fallback or explicit gap. It never calls a live provider, silently changes execution mode, or waits for generation. `replay` uses only a complete matching recording; a miss is an explicit replay gap and never goes live. Changing mode applies to newly admitted jobs; replacing active work requires explicit cancel/reissue with a new identity. Live execution, when authorized, still requires all admission and budget checks. Forecast rank never overrides mode.

## Evidence and unresolved gates

This decision resolves only G12:D03 (“Define heuristic predictive demand”; expected outcome: “no calibrated probability claim”). It preserves G12’s broader objective: permitted pacing observations/consent, versioned intensity/cue/asset-demand APIs, accessibility/audio/UI budgets, hidden-state noninterference, inertia/fatigue/stem timing, fair prefetch/expiry/reservations/waste, measured session/device/cost calibration and stable-clause/barge-in safety. It does not claim those production gates are complete.

The contract follows [Runtime directors](runtime-directors.md) (pure bounded directors, heuristic forecast and admitted media effects), [Tempo engine](tempo-engine.md) (permitted inputs, hidden-state noninterference and measured budgets), [Subsystem interfaces](subsystem-interfaces.md) (media ownership, execution modes, budget and recording ports), [Asset engine](asset-engine.md) (forecast ranks, priority/fairness and cost reconciliation), [Interaction engine](interaction-engine.md) (consent, tentative input and explicit intent), and [Implementation roadmap](implementation-roadmap.md) (G08/G12 prerequisites and S03/S07 acceptance). The exact production types, numerical weights/bounds, cost quotes and device thresholds remain pending their named owners and G08/G12 review. Acceptance still requires measured useful-prefetch versus waste, actual spend, hidden-state paired cases, stale/cancel/mode misses and integrated native/WASM two-role output evaluation; none is evidenced by this policy document.

## Literal std-only contract example

The following standalone Rust example illustrates the decision boundary only. Its names, scalar identifiers and illustrative values are not production types, weights, budgets or calibrated measurements. The rank is deliberately unused for probability or cost admission. It accepts a fresh authorized live candidate within an explicit cap and refuses unauthorized, stale, cancelled, over-budget and prepared-only miss cases.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    PreparedOnly,
    Replay,
    Live,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Basis {
    run: u64,
    state_revision: u64,
    audience_generation: u64,
    policy_revision: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Candidate {
    basis: Basis,
    heuristic_rank: i32,
    estimated_micros: u64,
    audience_authorized: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    Unauthorized,
    StaleBasis,
    Cancelled,
    BudgetExceeded,
    PreparedMiss,
    ReplayMiss,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Decision {
    UsePrepared,
    DispatchLive { reserve_micros: u64 },
}

fn admit(
    candidate: Candidate,
    current: Basis,
    mode: Mode,
    already_reserved_micros: u64,
    budget_cap_micros: u64,
    cancelled: bool,
    matching_prepared_asset: bool,
) -> Result<Decision, Refusal> {
    if !candidate.audience_authorized {
        return Err(Refusal::Unauthorized);
    }
    if candidate.basis != current {
        return Err(Refusal::StaleBasis);
    }
    if cancelled {
        return Err(Refusal::Cancelled);
    }
    if matching_prepared_asset {
        return Ok(Decision::UsePrepared);
    }
    if mode == Mode::PreparedOnly {
        return Err(Refusal::PreparedMiss);
    }
    if mode == Mode::Replay {
        return Err(Refusal::ReplayMiss);
    }
    let total = already_reserved_micros
        .checked_add(candidate.estimated_micros)
        .ok_or(Refusal::BudgetExceeded)?;
    if total > budget_cap_micros {
        return Err(Refusal::BudgetExceeded);
    }
    Ok(Decision::DispatchLive {
        reserve_micros: candidate.estimated_micros,
    })
}

fn main() {
    let basis = Basis {
        run: 7,
        state_revision: 11,
        audience_generation: 3,
        policy_revision: 2,
    };
    let candidate = Candidate {
        basis,
        heuristic_rank: 4,
        estimated_micros: 20,
        audience_authorized: true,
    };
    assert_eq!(
        admit(candidate, basis, Mode::Live, 30, 100, false, false),
        Ok(Decision::DispatchLive { reserve_micros: 20 })
    );
    assert_eq!(
        admit(
            Candidate {
                audience_authorized: false,
                ..candidate
            },
            basis,
            Mode::Live,
            0,
            100,
            false,
            false,
        ),
        Err(Refusal::Unauthorized)
    );
    assert_eq!(
        admit(
            candidate,
            Basis {
                state_revision: 12,
                ..basis
            },
            Mode::Live,
            0,
            100,
            false,
            false,
        ),
        Err(Refusal::StaleBasis)
    );
    assert_eq!(
        admit(candidate, basis, Mode::Live, 0, 100, true, false),
        Err(Refusal::Cancelled)
    );
    assert_eq!(
        admit(candidate, basis, Mode::Live, 90, 100, false, false),
        Err(Refusal::BudgetExceeded)
    );
    assert_eq!(
        admit(candidate, basis, Mode::PreparedOnly, 0, 100, false, false),
        Err(Refusal::PreparedMiss)
    );
    assert_eq!(
        admit(candidate, basis, Mode::PreparedOnly, 0, 100, false, true),
        Ok(Decision::UsePrepared)
    );
    assert_eq!(
        admit(candidate, basis, Mode::Replay, 0, 100, false, false),
        Err(Refusal::ReplayMiss)
    );
}
```
