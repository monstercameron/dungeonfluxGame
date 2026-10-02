# G12-D03: Heuristic predictive demand policy

Status: Design decision; production contracts, policy values and calibration pending

## Decision

Predictive demand is a bounded ordering aid for optional reusable presentation assets. It answers “which already-permitted candidate should be considered first?” It does not estimate a calibrated probability, likelihood, confidence, expected utility, guaranteed reuse, fun, or user intent. A rank/score is comparable only within its named forecast policy revision and candidate set; it has no cross-version or percentage interpretation. Labels, APIs, telemetry and UI must call it a heuristic rank/score, never a probability or confidence. Even retrospective hit rates describe the measured cohort and do not turn an individual score into a calibrated probability.

`df-experience` may contribute only authorized, consented pacing observations and committed activity windows. `df-tempo` may supply its current versioned, bounded frame/profile and approved cue semantics. `df-presentation` builds candidates from committed or otherwise explicitly permitted moments, produces audience-scoped forecast candidates and ranks them with versioned bounded heuristic inputs. `df-media` considers only an admitted typed `AssetDemand`; it owns queueing, mode, budgets, provider dispatch, expiry, cancellation and reconciliation. Forecasting never dispatches generation, spends a budget or changes game state. These are the existing owners, not a second predictor/service.

## Forecast inputs and output

A forecast request binds the session/run, current committed state revision, presentation epoch, content/identity revision, policy revision, trusted audience/access generation and an explicit logical/presentation time basis. It also names its bounded horizon and candidate limit. Candidate evidence may include permitted public or audience-known semantic cues, current authorized profile, asset readiness/reusability, explicit dependency state and observed demand outcomes from an authorized aggregate. Tentative intent may prioritize only cheap cancellable local lookup; it cannot cause an action, public cue, irreversible commitment or unapproved cost.

Filter access and audience scope before scoring, serialization, telemetry fields, candidate identifiers exposed to a client, or prefetch manifests. Resolve a current trusted rights grant and intended-use permission, including suppression/revocation state, at admission; a cached/prepared hit does not bypass that check. Recheck rights before provider prompts and before delivery/export, so revocation or a changed purpose fences stale work. A hidden branch, secret stage, private belief/HP, another participant’s private preference, raw microphone, inferred emotion, or unconsented behavior cannot affect a public forecast. Paired states that differ only in unauthorized hidden data must yield identical public forecast output and demand. A separately authorized private audience may receive its own profile and forecast. Correlation/trace identifiers do not grant access.

The output is a bounded, deterministically ordered set of `ForecastCandidate` values carrying the typed asset/demand reference, ordinal heuristic score or rank, score-policy revision, source basis, audience scope, eligibility explanation code, expiry and any declared dependency. Stable tie-breaking is required. Missing inputs remain unknown; do not silently convert them to zero or invent a signal. Candidate count, horizon and score arithmetic are bounded. Values are tuning inputs, not rules mechanics, deadlines, or measurements of player emotion. The score must not encode sensitive information in ways observable through ordering, timing, cache behavior, or asset availability.

## Admission, cost and cancellation

Forecasting and media admission are separate stages. Before any prepared/cache/replay asset is used, or a candidate enters an optional queue, the session/media boundary revalidates the complete basis against current committed run/state/content/policy/identity and audience/access generation; validates the trusted current rights grant, intended use and suppression/revocation state; confirms demand eligibility; and checks the relevant cancellation/job generation. An asset being present or previously authorized does not prove current rights. A mismatch returns a typed stale/superseded outcome; it is not retried against new state implicitly. Recompute from the newly authorized basis if useful. Filter audience scope again immediately before any manifest or provider input.

Optional prefetch is strictly lower priority than interaction-critical speech/input and required prepared fallback. It is subject to explicit campaign authorization, a conservative versioned estimate, budget reservation, per-campaign/provider concurrency, byte, deadline, horizon, queue-aging/fairness and expiry limits. The cost owner must resolve the actual quote, reservation, allowance and liability contracts under G08 before production admission. No unpriced or unknown cost is treated as free. Admission must not consume a protected required-output allowance or another audience’s allowance. Denial, saturation, estimate overflow, exhausted allowance or uncertain authorization yields a typed defer/refusal and the current best available permitted presentation; no retry loop may evade the cap.

Before dispatch, cancellation or invalidation retires unstarted speculative demand. After dispatch, cancellation is best effort: it cannot assert a provider refund, erase an already-started cost, or roll back a committed decision. Preserve the job identity and reservation; record actual, cancellation-fee or unknown supplier usage and reconcile conservatively. Do not automatically repeat an ambiguous non-idempotent paid call. Generation/job/basis fences reject stale completions, and a late result cannot publish into a replaced audience, run, identity revision or cue epoch. Expiry makes an unused asset ineligible for this demand; it does not delete assets still referenced elsewhere or release unknown spend.

## Prepared-only and replay

`prepared_only` may select a verified, complete, currently rights-authorized prepared asset or matching fallback already available under its full versioned identity key. On a miss, return typed unavailability and render the best permitted prepared fallback or explicit gap. It never calls a live provider, silently changes execution mode, or waits for generation. `replay` uses only a complete recording whose full contract key matches the requested replay: contract/schema revision, role/channel, source/context/content/policy revisions, audience/access generation, locale and output parameters, with successful complete publication. A generic prepared-cache hit is not a replay hit, even when its bytes look usable; only a matching complete recording satisfies Replay. A missing, partial, stale, suppressed or identity-mismatched recording returns `ReplayMiss` and never goes live or falls through to the prepared cache. Changing mode applies to newly admitted jobs; replacing active work requires explicit cancel/reissue with a new identity. Live execution, when authorized, still requires all admission and budget checks. Forecast rank never overrides mode or current rights.

## Evidence and unresolved gates

This decision resolves only G12:D03 (“Define heuristic predictive demand”; expected outcome: “no calibrated probability claim”). It preserves G12’s broader objective: permitted pacing observations/consent, versioned intensity/cue/asset-demand APIs, accessibility/audio/UI budgets, hidden-state noninterference, inertia/fatigue/stem timing, fair prefetch/expiry/reservations/waste, measured session/device/cost calibration and stable-clause/barge-in safety. It does not claim those production gates are complete.

The contract follows [Runtime directors](runtime-directors.md) (pure bounded directors, heuristic forecast and admitted media effects), [Tempo engine](tempo-engine.md) (permitted inputs, hidden-state noninterference and measured budgets), [Subsystem interfaces](subsystem-interfaces.md) (media ownership, execution modes, budget and recording ports), [Asset engine](asset-engine.md) (forecast ranks, identity-bearing recording keys, priority/fairness and cost reconciliation), [Interaction engine](interaction-engine.md) (consent, tentative input and explicit intent), [Erasure and retention](erasure-retention-policy.md) (current `RightsGrant`, suppression generations and stale prepared/replay result rejection), and [Implementation roadmap](implementation-roadmap.md) (G08/G12 prerequisites and S03/S07 acceptance). The exact production types, numerical weights/bounds, cost quotes and device thresholds remain pending their named owners and G08/G12 review. Acceptance still requires measured useful-prefetch versus waste, actual spend, hidden-state paired cases, stale/cancel/mode misses and integrated native/WASM two-role output evaluation; none is evidenced by this policy document.

## Literal std-only contract example

The following standalone Rust example illustrates the decision boundary only. Its names, scalar identifiers and illustrative values are not production types, weights, budgets, rights APIs or calibrated measurements. The rank is deliberately unused for probability or cost admission. It accepts a fresh authorized live candidate and a complete matching replay recording, while adversarial assertions refuse generic prepared data in Replay, incomplete recordings, contract/audience identity mismatches, denied rights/suppression, stale bases, cancellation, and over-budget demand. The rights booleans stand in for trusted current server-side authorization results; client input cannot set them.

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
struct AssetIdentity {
    demand_key: u64,
    contract_revision: u32,
    role: u8,
    source_revision: u32,
    context_revision: u32,
    content_revision: u32,
    policy_revision: u32,
    audience_generation: u64,
    suppression_generation: u64,
    purpose_revision: u32,
    locale_revision: u32,
    provider_model_revision: u32,
    output_parameters: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TrustedRightsResult {
    grant_current: bool,
    intended_use_allowed: bool,
    suppression_clear: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Candidate {
    basis: Basis,
    identity: AssetIdentity,
    heuristic_rank: i32,
    estimated_micros: u64,
    audience_authorized: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Available {
    Missing,
    PreparedAsset {
        identity: AssetIdentity,
        complete: bool,
    },
    ReplayRecording {
        identity: AssetIdentity,
        complete: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    Unauthorized,
    RightsDenied,
    StaleBasis,
    Cancelled,
    BudgetExceeded,
    PreparedMiss,
    ReplayMiss,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Decision {
    UsePrepared,
    UseReplayRecording,
    DispatchLive { reserve_micros: u64 },
}

fn admit(
    candidate: Candidate,
    trusted_rights: TrustedRightsResult,
    current_basis: Basis,
    current_identity: AssetIdentity,
    mode: Mode,
    available: Available,
    already_reserved_micros: u64,
    budget_cap_micros: u64,
    cancelled: bool,
) -> Result<Decision, Refusal> {
    if !candidate.audience_authorized {
        return Err(Refusal::Unauthorized);
    }
    if !trusted_rights.grant_current
        || !trusted_rights.intended_use_allowed
        || !trusted_rights.suppression_clear
    {
        return Err(Refusal::RightsDenied);
    }
    if candidate.basis != current_basis || candidate.identity != current_identity {
        return Err(Refusal::StaleBasis);
    }
    if cancelled {
        return Err(Refusal::Cancelled);
    }
    match mode {
        Mode::Replay => match available {
            Available::ReplayRecording {
                identity,
                complete: true,
            } if identity == candidate.identity => Ok(Decision::UseReplayRecording),
            _ => Err(Refusal::ReplayMiss),
        },
        Mode::PreparedOnly => match available {
            Available::PreparedAsset {
                identity,
                complete: true,
            } if identity == candidate.identity => Ok(Decision::UsePrepared),
            _ => Err(Refusal::PreparedMiss),
        },
        Mode::Live => {
            if matches!(
                available,
                Available::PreparedAsset {
                    identity,
                    complete: true,
                } if identity == candidate.identity
            ) {
                return Ok(Decision::UsePrepared);
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
    }
}

fn main() {
    let basis = Basis {
        run: 7,
        state_revision: 11,
        audience_generation: 3,
        policy_revision: 2,
    };
    let identity = AssetIdentity {
        demand_key: 41,
        contract_revision: 5,
        role: 2,
        source_revision: 9,
        context_revision: 6,
        content_revision: 8,
        policy_revision: 2,
        audience_generation: 3,
        suppression_generation: 7,
        purpose_revision: 4,
        locale_revision: 4,
        provider_model_revision: 3,
        output_parameters: 55,
    };
    let rights = TrustedRightsResult {
        grant_current: true,
        intended_use_allowed: true,
        suppression_clear: true,
    };
    let candidate = Candidate {
        basis,
        identity,
        heuristic_rank: 4,
        estimated_micros: 20,
        audience_authorized: true,
    };
    let missing = Available::Missing;
    let prepared = Available::PreparedAsset {
        identity,
        complete: true,
    };
    let recording = Available::ReplayRecording {
        identity,
        complete: true,
    };
    assert_eq!(
        admit(
            candidate,
            rights,
            basis,
            identity,
            Mode::Live,
            missing,
            30,
            100,
            false
        ),
        Ok(Decision::DispatchLive { reserve_micros: 20 })
    );
    assert_eq!(
        admit(
            candidate,
            rights,
            basis,
            identity,
            Mode::PreparedOnly,
            prepared,
            0,
            100,
            false,
        ),
        Ok(Decision::UsePrepared)
    );
    assert_eq!(
        admit(
            candidate,
            rights,
            basis,
            identity,
            Mode::Replay,
            prepared,
            0,
            100,
            false,
        ),
        Err(Refusal::ReplayMiss),
        "a generic prepared asset cannot satisfy replay"
    );
    assert_eq!(
        admit(
            candidate,
            rights,
            basis,
            identity,
            Mode::Replay,
            recording,
            0,
            100,
            false,
        ),
        Ok(Decision::UseReplayRecording)
    );
    let incompatible_recordings = [
        AssetIdentity {
            contract_revision: 6,
            ..identity
        },
        AssetIdentity {
            role: 3,
            ..identity
        },
        AssetIdentity {
            source_revision: 10,
            ..identity
        },
        AssetIdentity {
            context_revision: 7,
            ..identity
        },
        AssetIdentity {
            content_revision: 9,
            ..identity
        },
        AssetIdentity {
            policy_revision: 3,
            ..identity
        },
        AssetIdentity {
            audience_generation: 4,
            ..identity
        },
        AssetIdentity {
            suppression_generation: 8,
            ..identity
        },
        AssetIdentity {
            purpose_revision: 5,
            ..identity
        },
        AssetIdentity {
            locale_revision: 5,
            ..identity
        },
        AssetIdentity {
            provider_model_revision: 4,
            ..identity
        },
        AssetIdentity {
            output_parameters: 56,
            ..identity
        },
        AssetIdentity {
            demand_key: 42,
            ..identity
        },
    ];
    for incompatible in incompatible_recordings {
        assert_eq!(
            admit(
                candidate,
                rights,
                basis,
                identity,
                Mode::Replay,
                Available::ReplayRecording {
                    identity: incompatible,
                    complete: true,
                },
                0,
                100,
                false,
            ),
            Err(Refusal::ReplayMiss),
            "every replay identity field must match"
        );
    }
    assert_eq!(
        admit(
            candidate,
            rights,
            basis,
            identity,
            Mode::Replay,
            Available::ReplayRecording {
                identity,
                complete: false,
            },
            0,
            100,
            false,
        ),
        Err(Refusal::ReplayMiss),
        "partial recording is not a replay hit"
    );
    assert_eq!(
        admit(
            candidate,
            rights,
            basis,
            identity,
            Mode::PreparedOnly,
            missing,
            0,
            100,
            false,
        ),
        Err(Refusal::PreparedMiss)
    );
    assert_eq!(
        admit(
            Candidate {
                audience_authorized: false,
                ..candidate
            },
            rights,
            basis,
            identity,
            Mode::Replay,
            recording,
            0,
            100,
            false,
        ),
        Err(Refusal::Unauthorized)
    );
    for denied_rights in [
        TrustedRightsResult {
            grant_current: false,
            ..rights
        },
        TrustedRightsResult {
            intended_use_allowed: false,
            ..rights
        },
        TrustedRightsResult {
            suppression_clear: false,
            ..rights
        },
    ] {
        assert_eq!(
            admit(
                candidate,
                denied_rights,
                basis,
                identity,
                Mode::Replay,
                recording,
                0,
                100,
                false,
            ),
            Err(Refusal::RightsDenied),
            "revoked, wrong-purpose, or suppressed rights block replay"
        );
    }
    assert_eq!(
        admit(
            candidate,
            rights,
            Basis {
                state_revision: 12,
                ..basis
            },
            identity,
            Mode::Live,
            missing,
            0,
            100,
            false,
        ),
        Err(Refusal::StaleBasis)
    );
    assert_eq!(
        admit(
            candidate,
            rights,
            basis,
            AssetIdentity {
                source_revision: 10,
                ..identity
            },
            Mode::Live,
            missing,
            0,
            100,
            false,
        ),
        Err(Refusal::StaleBasis)
    );
    assert_eq!(
        admit(
            candidate,
            rights,
            basis,
            identity,
            Mode::Live,
            missing,
            0,
            100,
            true,
        ),
        Err(Refusal::Cancelled)
    );
    assert_eq!(
        admit(
            candidate,
            rights,
            basis,
            identity,
            Mode::Live,
            missing,
            90,
            100,
            false,
        ),
        Err(Refusal::BudgetExceeded)
    );
    assert_eq!(
        admit(
            candidate,
            rights,
            basis,
            identity,
            Mode::Live,
            missing,
            u64::MAX,
            u64::MAX,
            false,
        ),
        Err(Refusal::BudgetExceeded)
    );
    for rank in [i32::MIN, -1, 0, i32::MAX] {
        assert_eq!(
            admit(
                Candidate {
                    heuristic_rank: rank,
                    ..candidate
                },
                rights,
                basis,
                identity,
                Mode::Live,
                missing,
                90,
                100,
                false,
            ),
            Err(Refusal::BudgetExceeded),
            "a heuristic rank cannot override a budget refusal"
        );
    }
}
```
