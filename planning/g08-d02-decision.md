# G08 modality quote and fallback contract

Task/attempt: `B-G08-D02/a1`  
Source input: `308920e328ba6df85bea1e3d2abcbea5f1b796e6`  
Status: bounded design decision; no production API or provider is selected.

## Original scope and decision

The original objective is “Define modality quote and fallback contracts; expected: exact currency.” Its acceptance clauses are “exact currency” and “The named outcome has actual source/build-bound evidence; unsupported, pending, failed and unperformed checks remain explicit.” Its verification clauses are “Freeze the cited source decision and a bounded contract example; compare exact currency. Retain decision, alternatives and unresolved facts.” and “Exact executable commands: TBD at G01 and scoped prerequisite resolution; this planned procedure is not a claim that Rust/browser/provider checks ran.” This document preserves those criteria; this is source-backed design, not a runtime result.

**Decision:** An optional modality branch is admissible only from a versioned quote bound to the exact capability, provider/model revision, endpoint/region, account/plan, execution mode, and request/output profile. The quote states currency, provider-defined billable usage unit and rounding/minimum, exact rational rate or fixed maximum, expiry, and a conservative maximum supplier liability. The current task does not select a provider, rate, currency for production, quote TTL, usage bound, spend cap, or deadline. Quote values must be supplied from approved current evidence by the quote owner; absent or stale fields leave the route pending/unsupported and cannot be treated as zero.

Use the reviewed [typed-units policy](typed-units-policy.md): `df-types` owns currency-tagged nonnegative `u128` micro-units and exact checked rational arithmetic; a liability reservation rounds upward. The tag is syntactic and is not evidence that a currency, price or conversion is authorized. For example, USD 1 whole unit is 1,000,000 USD micro-units; preserve the currency with every amount and reject cross-currency arithmetic. No implicit FX or floating point. The quote/provider adapter owns provider-specific usage semantics and pinned rate facts. `df-commerce` owns quote identity/version, customer consent, atomic reservation, settlement, refunds and unknown liability through its existing spend authority. The existing `df-provider-api::BudgetStore` delegates to that authority; it is not a second wallet or authority. `df-media` owns modality orchestration and selects a matching prepared fallback under the already-admitted policy; it cannot mint a quote, reserve by itself, or change execution mode.

Before send, rejection, expiry, missing quote, currency/consent mismatch, or exhausted allowance means no provider dispatch and no supplier liability. It may select an already authorized matching prepared asset, otherwise return typed unavailability while gameplay continues. After dispatch, a known completed result settles observed usage against the pinned quote, retaining actual billable waste. A definitive known nonbillable failure can release only the reconciled unused reserve. Timeout, disconnect, cancellation, missing receipt, or ambiguous completion is `Unknown`: retain the full worst-case reservation, classify the supplier outcome as unknown, prohibit automatic retry under a fresh identity, and reconcile by the same request/usage/invoice evidence. A prepared fallback may still be served if already available and rights/audience/spec match; its use does not erase the unknown liability. If no matching fallback exists, report unavailable and preserve the source outcome. A fallback is a distinct branch/asset identity, not a price, quote, or implicit new paid call. A hedge requires a separate admission/reservation for every started branch under the same commerce cap.

## Alternatives and ownership

A single hard-coded global rate was rejected because modality units, account/region, output profiles, minimums, pricing revisions and provider semantics differ. Estimating cost from mutable list prices at dispatch was rejected because it cannot establish consent or bound admitted liability. Converting currencies implicitly was rejected because it can understate liability and lacks an approved rate. Retrying an ambiguous paid request as a new job was rejected because it can double-charge. Failing optional media closed without a prepared path was rejected because the source plans require play to continue with safe presentation fallback.

Integration owners are `df-provider-api` for quote lookup/versioning, provider identity/usage mapping and the existing `BudgetStore` facade; `df-commerce` for the authoritative consented spend admission/reservation/reconciliation (per [commerce-service](commerce-service.md)); `df-media` for matching fallback selection, job identity and result status; `df-assets`/`AssetStore` for complete durable prepared bytes and manifest/access checks; and `df-server` for composing trusted tenant/payer/session scope. `df-client` renders explicit pending/unavailable/fallback status and never authorizes spend. Reuse the existing `ExecutionMode`, `BudgetStore`, `AssetEngine` and asset manifest concepts from [subsystem interfaces](subsystem-interfaces.md) and [asset engine](asset-engine.md); this policy proposes no competing production API.

## Finite std-only contract example

This finite model demonstrates exact quote currency/liability and the decisive refusal/fallback outcomes. Amounts are micro-units; `maximum` is already computed from an integer rational quote by the canonical `df-types` rule and is not a production quote. It deliberately cannot identify a real provider or perform I/O.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Money {
    currency: &'static str,
    micro_units: u128,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Quote {
    currency: &'static str,
    maximum: Money,
    consent_cap: Money,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SendResult {
    Completed(Money),
    KnownNotBilled,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Decision {
    Dispatch {
        reserve: Money,
    },
    PreparedFallback,
    Unavailable,
    Settled {
        actual: Money,
    },
    Released,
    UnknownLiability {
        retained: Money,
        fallback_available: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Reject {
    CurrencyMismatch,
    ActualExceedsQuote,
}

fn admit(quote: Quote, prepared: bool) -> Result<Decision, Reject> {
    if quote.currency != quote.maximum.currency || quote.currency != quote.consent_cap.currency {
        return Err(Reject::CurrencyMismatch);
    }
    if quote.maximum.micro_units > quote.consent_cap.micro_units {
        return if prepared {
            Ok(Decision::PreparedFallback)
        } else {
            Ok(Decision::Unavailable)
        };
    }
    Ok(Decision::Dispatch {
        reserve: quote.maximum,
    })
}

fn finish(quote: Quote, result: SendResult, prepared: bool) -> Result<Decision, Reject> {
    match result {
        SendResult::Completed(actual) => {
            if actual.currency != quote.currency {
                return Err(Reject::CurrencyMismatch);
            }
            if actual.micro_units > quote.maximum.micro_units {
                return Err(Reject::ActualExceedsQuote);
            }
            Ok(Decision::Settled { actual })
        }
        SendResult::KnownNotBilled => Ok(Decision::Released),
        SendResult::Unknown => Ok(Decision::UnknownLiability {
            retained: quote.maximum,
            fallback_available: prepared,
        }),
    }
}

fn main() {
    let quote = Quote {
        currency: "USD",
        maximum: Money {
            currency: "USD",
            micro_units: 125_001,
        },
        consent_cap: Money {
            currency: "USD",
            micro_units: 200_000,
        },
    };
    assert_eq!(
        admit(quote, true),
        Ok(Decision::Dispatch {
            reserve: Money {
                currency: "USD",
                micro_units: 125_001
            },
        })
    );
    assert_eq!(
        finish(
            quote,
            SendResult::Completed(Money {
                currency: "USD",
                micro_units: 120_000,
            }),
            true
        ),
        Ok(Decision::Settled {
            actual: Money {
                currency: "USD",
                micro_units: 120_000
            },
        })
    );
    assert_eq!(
        finish(quote, SendResult::KnownNotBilled, true),
        Ok(Decision::Released)
    );
    assert_eq!(
        finish(quote, SendResult::Unknown, true),
        Ok(Decision::UnknownLiability {
            retained: quote.maximum,
            fallback_available: true,
        })
    );
    let too_expensive = Quote {
        consent_cap: Money {
            currency: "USD",
            micro_units: 100_000,
        },
        ..quote
    };
    assert_eq!(admit(too_expensive, true), Ok(Decision::PreparedFallback));
    assert_eq!(admit(too_expensive, false), Ok(Decision::Unavailable));
    let wrong_currency = Quote {
        consent_cap: Money {
            currency: "EUR",
            micro_units: 200_000,
        },
        ..quote
    };
    assert_eq!(admit(wrong_currency, true), Err(Reject::CurrencyMismatch));
    assert_eq!(
        finish(
            quote,
            SendResult::Completed(Money {
                currency: "USD",
                micro_units: 125_002,
            }),
            true
        ),
        Err(Reject::ActualExceedsQuote)
    );
}
```

The synthetic values exercise the contract shape only. They are not measured prices, approved caps or proof of `df-types` production behavior. The example's completion path rejects an actual above the quote; production policy must additionally record overrun as platform loss/incident, retain customer-consent limits, and stop affected admissions as commerce policy requires. The simplified `KnownNotBilled` outcome is available only after authoritative reconciliation; it must never represent a timeout or missing receipt.

## Unresolved production gates

Provider/model, region/account/plan, current rates and exact usage/minimum/rounding, quote source/revision/expiry, supported currency and gateway conversions, payer-vs-host authority, customer quote display/consent, liability caps, deadline/attempt/concurrency, fallback asset identity/rights/locale/codec/audience, cache/replay semantics, provider idempotency and invoice reconciliation are unresolved. G08 vendor qualification, commerce, persistence/recovery, rights/privacy/safety, real source/build integration, target capacity and latency, and independent media output review remain required. No provider account, secret, live/paid request, browser, audio device, or production service was used. Phone/device testing and browser support remain unperformed; no device support is asserted. Rust fixture results below verify only this literal policy model.

## Verification for this submission

The required example is extracted byte-for-byte from this document and checked with pinned Rust 1.98.1, edition 2024, repository `rustfmt.toml`, `rustfmt --check`, `rustc -D warnings`, and execution of its assertions. Exact guarded-command receipt paths and results are in this attempt's `handoff.json`. No Cargo, Clippy, WASM, browser, provider, integration, audible-output, or physical-device check is claimed. The original planned executable commands remain TBD at G01 and scoped prerequisite resolution.
