# G08-D01 prepared initial provider mode decision

Date: 2026-10-01  
Status: source-backed design decision; no provider, account, model, paid call, or production API selected.

## Decision

For a newly admitted AI/media job with no explicit, authorized mode, choose
`prepared_only`. It may use only a complete, validated, rights-authorized,
identity-matching prepared response or asset already available locally through
the existing recording/asset boundary. A miss, stale or incompatible identity,
corrupt/incomplete entry, or denied access ends as a typed unavailable/refusal
outcome. It must not contact a provider, queue generation, or silently switch to
`live` or `replay`. Thus the ordinary initial path makes no paid provider call.

An explicitly admitted `replay` job may use a complete recording whose key
matches the request contract and revisions; a miss is unavailable and never
falls through to a live provider. `live` remains disabled for this initial
decision. Enabling it requires a separately reviewed qualification record and
the authorized host/session policy, budget reservation, and operation bounds
described below. A mode is pinned to each admitted job. Changing policy affects
new jobs; replacing active work requires explicit cancel and reissue under a new
identity. A live job's configured fallback is separately identified and does
not rewrite its execution mode.

This selects only the prepared-only default and refusal behavior. It does not
select a vendor/model, prepared pack, recording catalog, voice or audio format,
fallback bytes, locale, capacity, deadline, concurrency, spend ceiling, device
target, latency/frame/memory bound, or production Rust signature. No numerical
runtime limit is inferred from the planning examples.

## Alternatives and rationale

| Alternative | Decision | Rationale |
| --- | --- | --- |
| Default to `live` | Defer | No exact product/model, account/region availability, reviewed input/output rights, validated quality, measured latency/capacity, reconciled economics, or durable spend admission is evidenced. A provider call could incur liability even when output is rejected or never used. |
| Default to `replay` | Defer as the default; retain as an explicit mode | Replay is deterministic and can support repeatable fixtures, but there is no qualified recording corpus/key policy for the initial product. A miss must remain a miss. |
| `prepared_only` by default | Select | Uses available complete, pre-authorized content without network generation or new supplier spend. It has a finite miss outcome and allows play to continue with declared unavailable/fallback presentation. |
| Block all AI/media features | Reject as the initial mode policy | It would discard prepared assets and complete replay fixtures that already satisfy a use case. Source contracts explicitly support prepared-only and replay as execution modes. |

Vendor/provider identity is capability-specific; a text qualification does not
qualify speech, image, sound, or video. Public price, quota, speed, or contract
claims are inputs to later evidence, never proof of DungeonFlux account access,
rights, output quality, service behavior, invoice reconciliation, or capacity.
There is no live-provider ranking in this decision.

## Authority and integration

Reuse the existing owners and contracts; do not add a second provider authority.

- `df-model` owns the `ExecutionMode` primitive. The host policy decides which
  changes it permits; the session owner commits changes and pins the admitted
  mode/job identity.
- `df-provider-api` owns provider substitution ports and `BudgetStore`;
  `df-providers` maps actual provider fields/status/EOF semantics. No provider
  adapter or budget implementation is authorized here.
- `df-ai` and `df-media` enforce the pinned mode for their respective jobs;
  `df-media::AssetEngine` owns media admission/status/cancel/complete/reconcile.
  `df-assets` owns complete immutable bytes and metadata. The client renders
  authorized results and fallback status; it does not choose or invoke vendors.
- For a later live admission, reuse the existing commerce spend/concurrency
  authority and durable usage records. Do not create a second wallet, provider
  service, registry authority, or direct game-handler dispatch.

## Finite contract and failure behavior

An exact prepared/replay match is usable only after the owning service confirms
completeness, contract/request identity, source and content revisions, audience,
rights scope, and access. A miss, wrong scope, stale identity, incomplete bytes,
or denied audience is never a cache hit. Prepared-only and replay misses return
unavailable without a provider request. A live request without the separately
approved policy/admission returns a typed refusal. If a later live request was
sent but its outcome is uncertain, preserve `Unknown`, its reservation and
reconciliation obligation; do not report `NotSent`/`Refunded` or replay it under
a new identity. Provider/fallback failure cannot alter committed game state,
required outcomes, rewards, or block play.

The following finite std-only example is a policy illustration, not a frozen
production API. Its table-driven assertions exercise the selected default,
prepared and replay hits/misses, wrong-scope refusal, and live refusal. Synthetic
entries prove only this tiny decision contract.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    PreparedOnly,
    Replay,
    Live,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Entry {
    PreparedMatch,
    ReplayMatch,
    Miss,
    WrongScope,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResultKind {
    UsePrepared,
    UseReplay,
    Unavailable,
    RefusedWrongScope,
    RefusedLiveNotAdmitted,
}

fn decide(requested: Option<Mode>, entry: Entry) -> ResultKind {
    let mode = requested.unwrap_or(Mode::PreparedOnly);
    match (mode, entry) {
        (_, Entry::WrongScope) => ResultKind::RefusedWrongScope,
        (Mode::PreparedOnly, Entry::PreparedMatch) => ResultKind::UsePrepared,
        (Mode::Replay, Entry::ReplayMatch) => ResultKind::UseReplay,
        (Mode::Live, _) => ResultKind::RefusedLiveNotAdmitted,
        _ => ResultKind::Unavailable,
    }
}

fn main() {
    let cases = [
        (None, Entry::PreparedMatch, ResultKind::UsePrepared),
        (None, Entry::Miss, ResultKind::Unavailable),
        (
            Some(Mode::PreparedOnly),
            Entry::ReplayMatch,
            ResultKind::Unavailable,
        ),
        (
            Some(Mode::Replay),
            Entry::ReplayMatch,
            ResultKind::UseReplay,
        ),
        (Some(Mode::Replay), Entry::Miss, ResultKind::Unavailable),
        (
            Some(Mode::PreparedOnly),
            Entry::WrongScope,
            ResultKind::RefusedWrongScope,
        ),
        (
            Some(Mode::Live),
            Entry::PreparedMatch,
            ResultKind::RefusedLiveNotAdmitted,
        ),
    ];
    for (mode, entry, expected) in cases {
        assert_eq!(decide(mode, entry), expected);
    }
}
```

## Unresolved gates before live mode or product promises

The G08 qualification record must be completed per capability, exact product /
model revision, endpoint/region, account/plan, execution mode, and request
profile. Before enabling any paid route, resolve and source:

1. Exact provider product/model lifecycle, API contract, region/account access,
   supported formats/locales/voices, limits, retry/idempotency semantics, and
   current terms.
2. Reviewed rights and privacy for source inputs, provider processing, generated
   output, territory, distribution, attribution, caching/reuse, retention,
   deletion, subprocessors, and audience authorization.
3. Rights-cleared representative fixtures, accepted quality criteria, validated
   output/safety handling, complete publication and user-observed output review.
4. Current versioned price/allowance/minimums, bounded exact liability, atomic
   budget admission, actual usage/invoice reconciliation, unknown-outcome policy,
   retries and aggregate/tenant/provider spend and concurrency limits.
5. Measured end-to-end latency tails, throttles, cancellation/fallback/stale-job
   fencing, bytes/frame/memory/audio behavior, cleanup, and capacity on actual
   supported devices and regions.
6. Prepared fallback content and its rights/identity/quality evidence; explicit
   deadline, request/output bounds, attempts, queue/circuit-breaker, and
   operational alert/reconciliation procedure derived from measured/approved
   budgets.
7. G08 product/client owner acceptance and independent reproducible review. A
   later paid benchmark additionally needs its own scoped authorization, total
   spend ceiling, operator, fixtures, deadline, concurrency and reconciliation
   procedure.

Phone testing is deferred. Browser/device support, audio formats, latency,
capacity, frame and memory budgets have not been tested or selected here.
Native/WASM workspace integration and running-game behavior are also unverified;
this planning task does not implement an application adapter.

## Governing source basis

- [Implementation roadmap](implementation-roadmap.md), “Prerequisite decisions,”
  “Delivery slices,” and “Runtime director delivery”: G08 is a prerequisite;
  concrete bounds must be measured, implementation follows existing slices.
- [Subsystem interfaces](subsystem-interfaces.md), “AI/provider/media ports”:
  defines the existing ports, owners, modes, job identity, prepared/replay miss,
  fallback and unknown-spend behavior used above.
- [Runtime reliability](runtime-reliability.md), “Audio, media, and provider
  boundaries”: results are fenced, validated and committed through the session
  owner; cancellation and stale generations do not publish.
- [Asset engine](asset-engine.md), “Control plane and latency classes,”
  “Prediction, fairness and spend,” and “Nonblocking presentation and
  acceptance”: prepared assets, complete publication, typed misses, bounded
  spend and nonblocking fallback stay in `df-media`/`df-assets`.
- [G08 vendor qualification policy](g08-d03-vendor-qualification-policy.md):
  exact-scope evidence and seven gates; vendor statements do not qualify a route;
  unknown spend is retained and live benchmarks need separate authorization.
- [Pricing and costs](pricing-and-costs.md), “Required billing and acceptance
  work” and “Service spend and income refinement”: reuse `BudgetStore` and the
  commerce authority; estimates are not verified bounds.
- [Commerce service](commerce-service.md), “Independent authority and
  boundaries,” “Atomic hierarchical spend and concurrency,” and “Allowance
  renewal, repricing and fair admission”: the single durable liability/admission
  authority owns any eventual spend controls.
- [Service operations](service-operations.md), “Effect send boundary and
  uncertainty,” “Tenant isolation and hostile input,” and “Retention, deletion
  and provenance”: provider egress, uncertainty and privacy remain explicit.
- [Commercial validation](commercial-validation.md), “Rights-complete launch
  gate” and “Joint offer-cost qualification”: rights and measured economics
  remain launch evidence, not planning assumptions.
- [Remote play](remote-play.md) and [campaign cinematics](campaign-cinematics.md):
  device/audio topology and optional generated media remain gated by permission,
  prepared fallback and measured behavior.
- [Asset-provider research](asset-provider-research.md) and
  [competitive research](competitive-research.md): research narrows future
  candidates but is not account qualification or product selection.

The policy contract above was extracted and checked with the task-pinned Rust
1.98.1, edition 2024, repository `rustfmt.toml`, and wave command guard. The exact
commands, immutable receipts, input hashes, submitted revision, and remaining
checks are recorded in the task handoff evidence; those checks do not establish
provider behavior or native/WASM integration.
