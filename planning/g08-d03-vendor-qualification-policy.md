# G08 vendor qualification policy

Date: 2026-10-01  
Status: Future qualification protocol; no vendor is selected or approved.

This document defines evidence required before `df-provider-api` and `df-media`
freeze a provider/model, execution mode, spend/deadline/concurrency bounds, media
format, fallback, or capacity target. Official vendor statements are dated claims,
not proof that DungeonFlux has account access, production capacity, a working
adapter, acceptable output, rights to reuse output, or positive unit economics.
No paid calls, secret access, production implementation, or spend decision took
place for this design item. The source-backed objective and launch gates remain
unchanged.

## Decision and alternatives

Use one versioned qualification record per **capability × provider product/model
revision × endpoint/region × account/plan × execution mode × request profile**.
Qualify each required capability independently (for example live transcription,
validated text, live speech, canonical still, SFX, or optional cinema). Do not
transfer a text-model result to its speech model, one region to another, one plan
to another, or one request shape to another. Keep provider details behind the
existing `df-provider-api`/`df-media` ports and their single canonical owners.
Do not create a parallel provider service or implementation.

The record combines dated official claims with local source/build-bound evidence.
Prefer a prepared/licensed reusable asset when it satisfies the use case; compare
streaming and batch modes only where the capability permits both. A vendor's
marketing or list-price page alone cannot select a live route. Keep a candidate
pending if any required fact is unsupported, stale, failed, or unperformed. Reject
it when a mandatory safety, rights, budget, privacy, or quality gate fails. No
candidate is called qualified until the intended product owner approves a complete
record and an independent reviewer can reproduce its evidence.

## Evidence record and freshness

Each assertion records: vendor and exact product/model/version; source URL and
source title; source publisher; page revision/effective date if stated; retrieval
UTC timestamp; verbatim claim or faithful short paraphrase; claim scope and limits;
our interpretation; owner; and recheck date. Preserve a local source snapshot or
content hash when policy allows. Record account/plan/region and configuration
separately from public claims. Redact credentials and private customer content.

Classify every required criterion as `verified`, `unsupported`, `not_run`, `failed`,
or `stale`. `verified` must point to direct evidence tied to the tested source
revision, adapter/configuration, request fixture, build, and environment. Vendor
statements are `official_claim`, not local measurements. A missing value is
`unsupported` or `not_run`, never a favorable default. Public claims have a
90-day maximum age at decision time unless the contract/version has a shorter
effective window; prices, availability, model lifecycle, account limits, and
terms are rechecked immediately before selection and before a paid release. An
explicit expiry overrides this default. A page with no published effective date
is dated by retrieval only and must not be represented as an effective-date
guarantee.

For each criterion preserve expected value, observed value, units, sample count,
method, threshold, pass/fail result, and an immutable evidence reference. Separate
vendor-stated latency from measured end-to-end latency; report sample count and
p50/p95/p99, errors, retries, and deadline misses for local runs. Bind measured
results to source commit, native/WASM build where relevant, provider adapter and
configuration, content/fixture revision, device matrix, region, time window, and
concurrency. A stale build or changed request/config invalidates only the affected
evidence and requires a rerun. A benchmark cannot certify legal rights or invoice
reconciliation.

## Qualification gates

1. **Capability and lifecycle.** Confirm official endpoint/model identity, input
   and output contract, streaming/batch behavior, context/duration/resolution,
   locale/voice/reference support, version lifecycle and deprecation dates. Mark
   every unsupported feature explicitly. Define typed request/response/error,
   cancellation, deadline, retry and idempotency behavior against the actual
   port; ordinary tests use faithful local fixtures, not live calls.
2. **Scope and quality.** Freeze representative, rights-cleared fixtures for the
   intended capability and failure cases. Define acceptance before running them:
   source-valid names/numbers/negation for language; locale/noise and partial vs
   final transcription; approved stable speech clauses, intelligibility and
   timing; identity/style/reference continuity for canonical assets; decoded
   format/duration/size; or synchronized loop/stem behavior. A model critic score
   is supplemental. Required human/frontier review observes the rendered result;
   audio claims need playback and an audio-capable observation. No required game
   rule, reward, or committed outcome depends on provider success.
3. **Measured service behavior.** Exercise cold/warm paths, representative
   regions/devices, realistic bounded concurrency and hostile ordering. Measure
   dispatch-to-validated-result and, where applicable, first audible/visible
   output through completion. Record throughput, tails, availability, throttles,
   overload, cancellation, fallback, stale-result fencing, byte/frame/memory use
   and cleanup. Never convert vendor token speed or internal inference into a
   player-visible latency claim.
4. **Exact economics and liability.** Pin published unit and minimum billing,
   included allowance, overage, cache/read/write, retries, rejected and unused
   work, references, storage/egress, taxes/FX, and any contract floor. Reconcile
   bounded test invoices or authoritative usage records against the request
   ledger. Compute conservative upper liability for the proposed admission,
   retries and worst-case completion; include private parallel scenes and shared
   playback delivery as separate cost drivers. Use exact currency arithmetic and
   a versioned price snapshot. Research prices and editable scenario models are
   not customer prices, capacity, realized costs, margins, willingness-to-pay, or
   proof of profitability.
5. **Rights, privacy, and safety.** Obtain reviewed written terms for the exact
   input/output, use, territory, commercial game distribution, caching/reuse,
   modification, attribution, generated music/audio/video, training/data use,
   retention/deletion and subprocessors. Rights apply to each asset and cache
   scope; byte identity does not grant cross-campaign reuse. Verify authorized
   audience before private data leaves the server; minimize/redact inputs and
   keep secrets and raw private speech out of ordinary logs. Validate content and
   safety before captions/audio/assets become visible. Any ambiguous entitlement
   or rights interpretation stays pending for counsel/contract owner review.
6. **Operational bounds and fallback.** Set a hard per-job deadline, maximum
   attempts, request/byte/output bounds, per-provider and per-tenant concurrency,
   circuit-breaker behavior, daily/monthly liability caps, queue limits, and a
   safe deterministic/prepared fallback. Bound both individual work and
   aggregate exposure. Provider failure, late output or fallback must not withhold
   required rewards, alter source outcomes, expose private state, or block play.
   Confirm region, credentials lifecycle, telemetry and operator reconciliation
   path without recording credentials.
7. **Capacity and release.** Derive initial capacity, p95/p99 deadline, frame,
   audio drift, bytes, memory, and supported-device targets from representative
   devices and load; verify them on the actual target matrix. An account's current
   quota is not a vendor guarantee. Obtain written capacity/SLA commitments when
   the release promise requires them. Keep optional/preview media behind an
   explicit allowance and prepared fallback.

## Spend uncertainty and stop rules

Reserve worst-case liability before a paid dispatch using the commerce authority
and an immutable price/config revision. Assign a stable request/operation identity
and record the maximum billable amount, currency, admission, provider request ID
when known, and reconciliation state. A timeout, disconnect, cancellation, or
missing receipt after send is `Unknown`, not `NotSent` or `Refunded`. Preserve the
full reserved liability and block automatic replay with a new identity. Reconcile
through provider request/usage records or invoice evidence; only a confirmed
non-billable result or confirmed final amount can release/reconcile the reserve.
If records remain unavailable or ambiguous, retain the worst-case liability,
close affected paid admissions at the configured cap, alert the operator, and
keep the qualification/release gate pending. Do not let lease expiry, customer
disconnect, retry, restore, or a stale payment snapshot erase the uncertainty.

Stop a run when the deadline, retry, per-job cap, tenant/provider concurrency,
aggregate liability, or safety/rights gate is reached. Return the declared typed
failure/fallback; do not silently exceed a limit. The proposed caps are selected
from approved budgets and measurements, not invented here. A live paid benchmark
requires its own scoped task with explicit account, fixture, total spend ceiling,
deadline, concurrency, operator, and reconciliation procedure. This design task
made no such call.

## Official dated illustrative claims (reviewed 2026-10-01)

These examples demonstrate how a future record should preserve scope and limits;
they do not qualify OpenAI, ElevenLabs, or any other provider for DungeonFlux.

| Official source and date | Claim recorded for illustration | What it does not establish |
| --- | --- | --- |
| [OpenAI API rate limits](https://developers.openai.com/api/docs/guides/rate-limits), retrieved 2026-10-01; page observed as crawled today | The documentation describes request/token/image/audio rate dimensions, says limits vary by model and are applied at organization/project scope, and distinguishes configured spend limits from the organization's approved monthly usage limit. It documents `Retry-After` and rate-limit headers. | No DungeonFlux account's actual model limit, regional availability, reserved capacity, measured response time, or permission to assume a retry is billable-safe. Must verify account/project settings and test bounded behavior. |
| [OpenAI API pricing](https://developers.openai.com/api/docs/pricing), retrieved 2026-10-01; page observed as crawled today | The page publishes product/unit prices and differentiates rates and service modes. Capture the exact model, unit, tier, context/feature, and date from the live page when making a decision. | A mutable list price is not an account quote, complete workload cost, stable future price, allowance, or profitability result. No prices are transcribed here as a DungeonFlux budget. |
| [OpenAI Services Agreement](https://openai.com/policies/services-agreement/), updated 2025-12-01; effective 2026-01-01 | For the covered business/developer services, the agreement says customer retains input ownership and owns output as between the parties, and places responsibility for rights/permissions in submitted input on the customer. | Does not determine rights in third-party source material, model output legality, another supplier's terms, exact negotiated order terms, or DungeonFlux's approved use. Legal review still required. |
| [ElevenAPI pricing](https://elevenlabs.io/pricing/api), retrieved 2026-10-01; browser index said “crawled 2 weeks ago” without a publication date | The public page presents API plan and feature pricing, including character-unit pricing. | It does not prove plan availability, account-specific quotas, exact duration/minimum billing for a selected API, contractual game rights, or latency/capacity. Confirm the exact endpoint, terms, and invoice units. |

The date “retrieved” is our observation date; crawler timestamps above describe
the browser index and are not vendor publication dates. The official page governs
the claim. Capture a current dated snapshot and applicable contract at decision
time. Existing [asset-provider research](asset-provider-research.md) and
[pricing model](pricing-and-costs.md) remain research snapshots; their candidate
prices and modeled costs are not silently promoted into approved bounds.

## Literal std-only evidence contract example

This illustrates a narrow policy boundary, not a production API. It accepts a
complete, fresh, exact-scope record with official dated support for every required
criterion. Unsupported, not-run, failed, stale, wrong-scope, undated, or
non-official assertions refuse qualification. `day` is an integer UTC day in a
real implementation it must be a validated date type. The accepted fixture is
synthetic and proves no vendor fact.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Supported,
    Unsupported,
    NotRun,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Evidence<'a> {
    criterion: &'a str,
    scope: &'a str,
    source_url: &'a str,
    official: bool,
    observed_day: i32,
    state: State,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    Missing,
    Duplicate,
    WrongScope,
    UndatedOrFuture,
    Stale,
    NonOfficial,
    Unsupported,
    NotRun,
    Failed,
}

fn qualify<'a>(
    required: &[&str],
    scope: &str,
    today: i32,
    max_age_days: i32,
    evidence: &'a [Evidence<'a>],
) -> Result<(), Refusal> {
    for criterion in required {
        let mut matching = evidence.iter().filter(|item| item.criterion == *criterion);
        let item = matching.next().ok_or(Refusal::Missing)?;
        if matching.next().is_some() {
            return Err(Refusal::Duplicate);
        }
        if item.scope != scope {
            return Err(Refusal::WrongScope);
        }
        if item.observed_day <= 0 || item.observed_day > today || item.source_url.is_empty() {
            return Err(Refusal::UndatedOrFuture);
        }
        if today - item.observed_day > max_age_days {
            return Err(Refusal::Stale);
        }
        if !item.official {
            return Err(Refusal::NonOfficial);
        }
        match item.state {
            State::Supported => {}
            State::Unsupported => return Err(Refusal::Unsupported),
            State::NotRun => return Err(Refusal::NotRun),
            State::Failed => return Err(Refusal::Failed),
        }
    }
    Ok(())
}

fn main() {
    let scope = "speech:model-x:region-a:project-7:stream-v1";
    let required = ["price-unit", "rights", "account-limit"];
    let accepted = [
        Evidence {
            criterion: "price-unit",
            scope,
            source_url: "https://vendor.example/pricing",
            official: true,
            observed_day: 20_000,
            state: State::Supported,
        },
        Evidence {
            criterion: "rights",
            scope,
            source_url: "https://vendor.example/terms",
            official: true,
            observed_day: 20_000,
            state: State::Supported,
        },
        Evidence {
            criterion: "account-limit",
            scope,
            source_url: "https://vendor.example/limits",
            official: true,
            observed_day: 20_000,
            state: State::Supported,
        },
    ];
    assert_eq!(qualify(&required, scope, 20_010, 90, &accepted), Ok(()));

    let mut unperformed = accepted;
    unperformed[2].state = State::NotRun;
    assert_eq!(
        qualify(&required, scope, 20_010, 90, &unperformed),
        Err(Refusal::NotRun)
    );

    let mut unsupported = accepted;
    unsupported[1].state = State::Unsupported;
    assert_eq!(
        qualify(&required, scope, 20_010, 90, &unsupported),
        Err(Refusal::Unsupported)
    );

    let mut failed = accepted;
    failed[0].state = State::Failed;
    assert_eq!(
        qualify(&required, scope, 20_010, 90, &failed),
        Err(Refusal::Failed)
    );

    assert_eq!(
        qualify(&required, scope, 20_200, 90, &accepted),
        Err(Refusal::Stale)
    );
    assert_eq!(
        qualify(
            &required,
            "speech:model-y:region-a:project-7:stream-v1",
            20_010,
            90,
            &accepted
        ),
        Err(Refusal::WrongScope)
    );
}
```

The example's acceptance is only acceptance of synthetic evidence-shape input.
It does not implement dates, prove source authenticity, qualify a vendor, or
replace versioned contracts, exact monetary admission, rights review, invoice
reconciliation, adapter fixtures, integrated runtime checks, or independent
output evaluation. A production contract must distinguish dated public claims
from performed local checks and written legal/commercial decisions rather than
mark every gate `official`.

## Required future handoff and unresolved work

Before G08 selection, deliver the approved record, source snapshots/hashes,
decision and rejected alternatives, frozen fixture and adapter/config revision,
native/WASM/source/build identity, exact bounded test and its result, retained
output/audio observations where relevant, usage/invoice reconciliation, rights
decision, budget/deadline/concurrency/fallback, unsupported facts, owner and
independent review. The implementation owners remain `df-provider-api` and
`df-media`; the client/device owner supplies actual browser/device measurements.
No source, build, app workspace, selected account, rights grant, supported device
matrix, provider benchmark, or production qualification result was available in
this design attempt. G01/G03/G04/G05/G08 and applicable commerce, rights,
observability, and frontier-evaluation work remain gates. This policy preserves
the roadmap's objective and does not close those tasks.
