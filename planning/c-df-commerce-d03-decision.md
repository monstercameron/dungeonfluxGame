# C-df-commerce D03: hierarchical liability and settlement decision

Task: `B-C-df-commerce-D03/a1`  
Owner: `df-commerce` policy design  
Status: proposed source-backed contract; production types, persistence and provider behavior remain unimplemented.

## Decision

Treat a paid effect as one stable, attempt-bound obligation whose maximum supplier liability is admitted atomically against every applicable budget scope. The policy hierarchy is **platform period → supplier/account → tenant/payer → campaign → job**. A supplier concurrency slot is a separate resource counter, not money. A spend grant expresses the customer's consented cap; it is a further hard ceiling, never a source of platform or supplier authority. Keep campaign payer, host, tenant owner and platform operator as separate capabilities.

Use `df-types`' existing exact money contract: currency-tagged nonnegative `u128` micro-units, where 1,000,000 micro-units represent one whole tagged currency unit; check every operation; no floats or implicit FX. The three-letter tag is syntactic only. It does not imply a supported currency or gateway scale. `df-commerce` owns quote/version/consent, hierarchy counters, reservations, attempt-bound ledger settlement and the customer's cap. Quote/provider adapters pin billable usage semantics and rate facts. `df-server`/`df-providers` own gateway conversion. `df-persistence` implements the repository transaction and protected recovery journal. `df-auth` owns the versioned entitlement reader; `df-server` supplies the native facade. `df-provider-api`'s existing `BudgetStore` delegates to this one authority; do not create a second wallet or competing production API.

An admitted record carries the existing commerce concepts: tenant, payer, campaign, job/effect, source/config/provider/price/quote versions, maximum units, `max_supplier_liability`, currency, mode, expiry and consented `SpendGrant`. Under one serializable transaction or equivalent explicit stable-order locks, check all hierarchy currency headroom, concurrency and customer consent; only if every check passes reserve the maximum at each money scope, insert unique reservation plus accepted intent linkage, and return a revisioned permit. Any failed check leaves all counters unchanged and selects a prepared fallback or typed budget/capacity refusal. Never do provider I/O under these locks. All rows use a common explicit currency for a given counter; no cross-currency aggregation occurs without a separately approved pinned FX policy.

Model the liability as a stateful obligation, not as a number that disappears on timeout:

- `Reserved` may proceed to `Dispatching` only with the current durable reservation/owner fence and journal-confirmed permit. `UnsentCanceled` is releasable only when the dispatcher proves no socket send began.
- Once egress may have begun, missing, timed-out, lost, canceled, stale or ambiguous outcomes are **`UnknownLiability`**. Preserve the full maximum amount against every hierarchy scope and hold supplier concurrency. Expiry, owner loss, cancellation, process restart or an old PostgreSQL snapshot does not prove non-send and never releases it. Reconcile against the same stable provider key/attempt identity. If the provider cannot prove the result, keep the obligation Unknown and fail closed for the affected admission; operator-approved closed-world policy is a separate explicit scoped decision, never an automatic lease cleanup.
- `SettledKnown` requires verified attempt-bound usage/invoice facts. Settle actual cost **plus billed failed/regenerated waste** once, append ledger entries, and release only the known unused remainder. Reversals, refund/dispute adjustments append typed reversing entries; never edit old charges. A late result may settle the old attempt but cannot commit an obsolete gameplay result.
- If a verified invoice exceeds the reservation or customer's consent, retain the actual supplier liability as an explicit platform loss/overrun incident, block affected supplier admissions, and never charge the customer above their consent. Do not clamp the supplier fact to the cap or fabricate a successful customer settlement.

Money liabilities are rounded upward at quote calculation; later settlement has its own explicit policy and observed amount. A quote ratio or gateway receipt is not by itself an authorized price or entitlement. Top-up cash and its refundable/deferred-credit liability are not earned revenue. A verified paid invoice and unique `AllowanceGrantId(subscription, paid_invoice, period, price_version)` can create an allowance exactly once; wall-clock counter reset cannot. No negative balance funds paid work. Payment/gateway `Unknown` is distinct from `UnknownLiability`: it blocks grant/credit visibility until authoritative payment reconciliation, while an egress-ambiguous supplier obligation additionally retains the worst-case reservation.

The irreversible boundary depends on the protected, append-only, nonregressing recovery journal: reserve/claim in PostgreSQL, append and confirm journal entry plus protected head, mark the matching row journal-confirmed, then permit egress or expose financial credit/entitlement. On restore, authenticate the latest protected head and reconcile its post-backup entries before serving commercial scopes. Missing, stale, conflicting or unverifiable head/key/journal state keeps the scope closed; absence from an old backup is not evidence of no payment or send. Journal failure yields pending/unknown, no dispatch, no grant and no deletion-complete acknowledgement. This contract does not claim the planned object backing, head integrity, database atomicity, concurrency or restore behavior is implemented or qualified.

## Alternatives considered

A single global balance was rejected: it cannot attribute tenant consent, campaign exposure or supplier-specific overruns, and lets unrelated obligations consume or release one another's authority. Duplicated per-feature wallets were rejected because the architecture names one `df-commerce` ledger and requires `BudgetStore` to delegate to it. Reserving only after a provider response was rejected because multiple individually valid requests could collectively exceed a shared ceiling. Releasing on timeout/lease expiry was rejected because dispatch may already have occurred. Treating maximum exposure as the final customer charge was rejected because only verified actual usage plus billed waste settles. Negative balances/refunds as mutation of historic charges were rejected; append a typed reversal and preserve the original entry. Floating point, implicit currency conversion, and guessing currency minor units were rejected by the exact value contract and commerce's pinned-rate boundary.

## Finite executable contract example

This `std`-only model exercises the policy boundary, not production `df-commerce`, `df-types`, a repository, a provider or runtime behavior. Counter values below are synthetic fixture amounts chosen to demonstrate admission ordering; they are not measured service limits or commercial defaults. `Money.micros` means tagged-currency micro-units. The production hierarchy, IDs, quote binding, transaction, journal and adapter contracts still require their owners' implementation and review.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Money {
    currency: [u8; 3],
    micros: u128,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Error {
    CurrencyMismatch,
    Overflow,
    BudgetExceeded,
    CustomerConsentExceeded,
    UnknownCannotRelease,
    ActualExceedsMaximum,
    InvalidTransition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Obligation {
    Reserved,
    UnknownLiability,
    SettledKnown { actual: u128 },
    Released,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Reservation {
    currency: [u8; 3],
    maximum_micros: u128,
    customer_cap_micros: u128,
    obligation: Obligation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Hierarchy {
    // Platform, supplier, tenant/payer, campaign, job, in that order.
    used: [u128; 5],
    limit: [u128; 5],
}

impl Hierarchy {
    fn reserve(
        &mut self,
        currency: [u8; 3],
        maximum: Money,
        customer_cap: Money,
    ) -> Result<Reservation, Error> {
        if maximum.currency != currency || customer_cap.currency != currency {
            return Err(Error::CurrencyMismatch);
        }
        if maximum.micros > customer_cap.micros {
            return Err(Error::CustomerConsentExceeded);
        }
        let mut next = self.used;
        for index in 0..next.len() {
            next[index] = next[index]
                .checked_add(maximum.micros)
                .ok_or(Error::Overflow)?;
            if next[index] > self.limit[index] {
                return Err(Error::BudgetExceeded);
            }
        }
        self.used = next;
        Ok(Reservation {
            currency,
            maximum_micros: maximum.micros,
            customer_cap_micros: customer_cap.micros,
            obligation: Obligation::Reserved,
        })
    }

    fn mark_unknown(&self, reservation: &mut Reservation) -> Result<(), Error> {
        if reservation.obligation != Obligation::Reserved {
            return Err(Error::InvalidTransition);
        }
        reservation.obligation = Obligation::UnknownLiability;
        Ok(())
    }

    fn release_unsent(
        &mut self,
        reservation: &mut Reservation,
        dispatcher_proved_no_send: bool,
    ) -> Result<(), Error> {
        if reservation.obligation == Obligation::UnknownLiability {
            return Err(Error::UnknownCannotRelease);
        }
        if reservation.obligation != Obligation::Reserved || !dispatcher_proved_no_send {
            return Err(Error::InvalidTransition);
        }
        self.adjust_all(reservation.maximum_micros, 0)?;
        reservation.obligation = Obligation::Released;
        Ok(())
    }

    fn settle_known(
        &mut self,
        reservation: &mut Reservation,
        actual_including_billed_waste: Money,
    ) -> Result<(), Error> {
        if actual_including_billed_waste.currency != reservation.currency {
            return Err(Error::CurrencyMismatch);
        }
        if !matches!(
            reservation.obligation,
            Obligation::Reserved | Obligation::UnknownLiability
        ) {
            return Err(Error::InvalidTransition);
        }
        if actual_including_billed_waste.micros > reservation.maximum_micros {
            return Err(Error::ActualExceedsMaximum);
        }
        if actual_including_billed_waste.micros > reservation.customer_cap_micros {
            return Err(Error::CustomerConsentExceeded);
        }
        self.adjust_all(
            reservation.maximum_micros,
            actual_including_billed_waste.micros,
        )?;
        reservation.obligation = Obligation::SettledKnown {
            actual: actual_including_billed_waste.micros,
        };
        Ok(())
    }

    fn adjust_all(&mut self, from: u128, to: u128) -> Result<(), Error> {
        let mut next = self.used;
        for value in &mut next {
            *value = value
                .checked_sub(from)
                .ok_or(Error::InvalidTransition)?
                .checked_add(to)
                .ok_or(Error::Overflow)?;
        }
        self.used = next;
        Ok(())
    }
}

fn main() {
    const USD: [u8; 3] = *b"USD";
    let mut budgets = Hierarchy {
        used: [0; 5],
        limit: [100, 80, 60, 50, 40],
    };
    let max = Money {
        currency: USD,
        micros: 40,
    };
    let cap = Money {
        currency: USD,
        micros: 45,
    };

    // Every scope reserves atomically; a dispatched ambiguous attempt stays held.
    let mut unknown = budgets.reserve(USD, max, cap).unwrap();
    assert_eq!(budgets.used, [40; 5]);
    budgets.mark_unknown(&mut unknown).unwrap();
    assert_eq!(
        budgets.settle_known(
            &mut unknown,
            Money {
                currency: USD,
                micros: 41
            }
        ),
        Err(Error::ActualExceedsMaximum)
    );
    assert_eq!(unknown.obligation, Obligation::UnknownLiability);
    assert_eq!(
        budgets.release_unsent(&mut unknown, true),
        Err(Error::UnknownCannotRelease)
    );
    assert_eq!(budgets.used, [40; 5]);

    // A customer cap refusal and any hierarchy refusal leave all scopes unchanged.
    assert_eq!(
        budgets.reserve(
            USD,
            Money {
                currency: USD,
                micros: 10
            },
            Money {
                currency: USD,
                micros: 9
            }
        ),
        Err(Error::CustomerConsentExceeded)
    );
    assert_eq!(
        budgets.reserve(
            USD,
            Money {
                currency: USD,
                micros: 11
            },
            Money {
                currency: USD,
                micros: 11
            }
        ),
        Err(Error::BudgetExceeded)
    );
    assert_eq!(budgets.used, [40; 5]);

    // Verified actual plus billed waste settles once and releases only the known remainder.
    budgets
        .settle_known(
            &mut unknown,
            Money {
                currency: USD,
                micros: 27,
            },
        )
        .unwrap();
    assert_eq!(budgets.used, [27; 5]);
    assert_eq!(unknown.obligation, Obligation::SettledKnown { actual: 27 });
    assert_eq!(
        budgets.settle_known(
            &mut unknown,
            Money {
                currency: USD,
                micros: 27
            }
        ),
        Err(Error::InvalidTransition)
    );

    // A proved-unsent reservation is releasable; currency cannot silently change.
    let mut empty_hierarchy = Hierarchy {
        used: [0; 5],
        limit: [100, 80, 60, 50, 40],
    };
    let mut unsent = empty_hierarchy.reserve(USD, max, cap).unwrap();
    empty_hierarchy.release_unsent(&mut unsent, true).unwrap();
    assert_eq!(empty_hierarchy.used, [0; 5]);
    assert_eq!(
        budgets.reserve(
            USD,
            Money {
                currency: *b"EUR",
                micros: 1
            },
            cap
        ),
        Err(Error::CurrencyMismatch)
    );
}
```

## Unresolved production decisions and gates

- `df-commerce` implementation must freeze concrete validated ID wrappers, exact public field/error names, reservation/ledger row relationships, idempotency/attempt uniqueness and quote/consent version binding after prerequisite source contracts are reviewed. This policy does not define an alternate API or prove all referenced contracts exist in code.
- `df-persistence`/G05 must select the real PostgreSQL rows, serializable/locking strategy, isolation/retry behavior, durable uniqueness, repository transaction result and migration. Concurrent many-campaign admission and rollback must prove no partial multi-scope reservation.
- `df-persistence`/X02 must qualify protected journal append/head durability, authentication, nonregression, partial failure, restore-before-send/payment-credit/deletion-ack behavior and cost. These guarantees are planned only.
- `df-providers` plus commerce must bind provider attempt/idempotency, supplier invoices, billed retries/waste, late responses, cancellation, reconciliation deadlines and operator closed-world policy; unsupported idempotency keeps the result Unknown. No live provider facts or calls were used.
- Commerce/quote owners must settle quote validity, allowed currencies and pinned FX, provider usage units, gateway-minor-unit conversion, taxes/fees, display amounts, rounding, top-up/refund terms and the required treatment of verified actual charges beyond estimate. No arbitrary commercial bounds are inferred here.
- `df-auth`, `df-server`, `df-api`, `df-client` and `df-persistence` own current entitlement revalidation, tenant/audience isolation and eventual boundary codecs/projections. A stale cache, caller-supplied tenant ID or client payment screen cannot authorize money or access.
- The proposed launch ceilings, grant sizes, concurrency values and schedules in `commerce-service.md` remain unmeasured policy candidates pending G08 capacity and funded exposure evidence. No paid release or demand/profitability claim follows from this decision.
- No application source/workspace, production database, recovery journal, payment integration, provider behavior or customer-facing flow was exercised. Browser/device/payment/restore/load checks are pending with their named owners; no Rust runtime check can establish those behaviors.

## Governing source basis

- [Customer, commerce and admission contracts](commerce-service.md): `df-commerce` authority and boundaries; exact money; payment state; atomic lock order; reservation transitions, known/unknown liability, customer ceiling and overrun policy; allowance identity.
- [Money, usage and elapsed-time units](typed-units-policy.md): the canonical tagged currency micro-unit and checked integer arithmetic; gateway conversions remain consumer-owned.
- [Protected journal durability boundary](protected-journal-policy.md) and [Service operations](service-operations.md): possible sends, money and privacy suppression require append/head confirmation and fail-closed restore/reconciliation.
- [Subsystem architecture](subsystem-architecture.md), [Subsystem interfaces](subsystem-interfaces.md), [Storage architecture](storage-architecture.md): one pure commerce authority, existing `BudgetStore` delegation, native I/O composition, and persistence/journal owners.
- [Runtime reliability](runtime-reliability.md): accepted work lifetime is independent of caller cancellation; attempt/run fencing and Unknown paid calls cannot be replayed or refunded by inference.
- [Implementation roadmap](implementation-roadmap.md): prerequisite gates and later integration ownership; planning contract is not deployed behavior.

Source hashes are retained in the attempt evidence. This proposal preserves the original objective and remains subject to independent frontier review and root integration.
