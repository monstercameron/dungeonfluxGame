# C-df-commerce D03: hierarchical liability and settlement decision

Task: `B-C-df-commerce-D03/a2`
Owner: `df-commerce` policy design  
Status: proposed source-backed contract; production types, persistence and provider behavior remain unimplemented.

## Decision

Treat a paid effect as one stable, attempt-bound obligation whose maximum supplier liability is admitted atomically against every applicable budget scope. The policy hierarchy is **platform period → supplier/account → tenant/payer → campaign → job**. A supplier concurrency slot is a separate resource counter, not money. A spend grant expresses the customer's consented cap; it is a further hard ceiling, never a source of platform or supplier authority. Keep campaign payer, host, tenant owner and platform operator as separate capabilities.

Use `df-types`' existing exact money contract: currency-tagged nonnegative `u128` micro-units, where 1,000,000 micro-units represent one whole tagged currency unit; check every operation; no floats or implicit FX. The three-letter tag is syntactic only. It does not imply a supported currency or gateway scale. `df-commerce` owns quote/version/consent, hierarchy counters, reservations, attempt-bound ledger settlement and the customer's cap. Quote/provider adapters pin billable usage semantics and rate facts. `df-server`/`df-providers` own gateway conversion. `df-persistence` implements the repository transaction and protected recovery journal. `df-auth` owns the versioned entitlement reader; `df-server` supplies the native facade. `df-provider-api`'s existing `BudgetStore` delegates to this one authority; do not create a second wallet or competing production API.

An admitted record carries the existing commerce concepts: tenant, payer, campaign, job/effect, source/config/provider/price/quote versions, maximum units, `max_supplier_liability`, currency, mode, expiry and consented `SpendGrant`. Under one serializable transaction or equivalent explicit stable-order locks, check all hierarchy currency headroom, concurrency and customer consent; only if every check passes reserve the maximum at each money scope, insert unique reservation plus accepted intent linkage, and return a revisioned permit. Any failed check leaves all counters unchanged and selects a prepared fallback or typed budget/capacity refusal. Never do provider I/O under these locks. Every `Hierarchy` and each of its five counter rows is bound to one explicit `Currency` for its lifetime. Each counter stores a canonical `Money`, not an untagged integer. Reservation and settlement validate the hierarchy, existing rows, reservation maximum, customer cap and actual amount against that currency before preparing any changed state. A different currency, including a mismatched configured counter limit, returns typed `CurrencyMismatch` before a hierarchy is created or any reservation/settlement mutation occurs; existing counters and obligation state remain unchanged. No cross-currency aggregation occurs without a separately approved pinned FX policy; that would require distinct currency-specific counters or an explicitly governed conversion contract, never adding unlike tagged values.

Model the liability as a stateful obligation, not as a number that disappears on timeout:

- `Reserved` may proceed to `Dispatching` only with the current durable reservation/owner fence and journal-confirmed permit. `UnsentCanceled` is releasable only when the dispatcher proves no socket send began.
- Once egress may have begun, missing, timed-out, lost, canceled, stale or ambiguous outcomes are **`UnknownLiability`**. Preserve the full maximum amount against every hierarchy scope and hold supplier concurrency. Expiry, owner loss, cancellation, process restart or an old PostgreSQL snapshot does not prove non-send and never releases it. Reconcile against the same stable provider key/attempt identity. If the provider cannot prove the result, keep the obligation Unknown and fail closed for the affected admission; operator-approved closed-world policy is a separate explicit scoped decision, never an automatic lease cleanup.
- `SettledKnown` requires verified attempt-bound usage/invoice facts. Settle actual cost **plus billed failed/regenerated waste** once, append ledger entries, and release only the known unused remainder. Reversals, refund/dispute adjustments append typed reversing entries; never edit old charges. A late result may settle the old attempt but cannot commit an obsolete gameplay result.
- If a verified invoice exceeds the reservation or customer's consent, retain the actual supplier liability as an explicit platform loss/overrun incident, block affected supplier admissions, and never charge the customer above their consent. Do not clamp the supplier fact to the cap or fabricate a successful customer settlement.

Money liabilities are rounded upward at quote calculation; later settlement has its own explicit policy and observed amount. A quote ratio or gateway receipt is not by itself an authorized price or entitlement. Top-up cash and its refundable/deferred-credit liability are not earned revenue. A verified paid invoice and unique `AllowanceGrantId(subscription, paid_invoice, period, price_version)` can create an allowance exactly once; wall-clock counter reset cannot. No negative balance funds paid work. Payment/gateway `Unknown` is distinct from `UnknownLiability`: it blocks grant/credit visibility until authoritative payment reconciliation, while an egress-ambiguous supplier obligation additionally retains the worst-case reservation.

The irreversible boundary depends on the protected, append-only, nonregressing recovery journal: reserve/claim in PostgreSQL, append and confirm journal entry plus protected head, mark the matching row journal-confirmed, then permit egress or expose financial credit/entitlement. On restore, authenticate the latest protected head and reconcile its post-backup entries before serving commercial scopes. Missing, stale, conflicting or unverifiable head/key/journal state keeps the scope closed; absence from an old backup is not evidence of no payment or send. Journal failure yields pending/unknown, no dispatch, no grant and no deletion-complete acknowledgement. This contract does not claim the planned object backing, head integrity, database atomicity, concurrency or restore behavior is implemented or qualified.

## Alternatives considered

A single global balance was rejected: it cannot attribute tenant consent, campaign exposure or supplier-specific overruns, and lets unrelated obligations consume or release one another's authority. Duplicated per-feature wallets were rejected because the architecture names one `df-commerce` ledger and requires `BudgetStore` to delegate to it. Untagged aggregate counters were rejected: adding USD and EUR amounts does not produce a meaningful exact liability. Each hierarchy is bound to one `Currency`, each money counter is a canonical tagged `Money`, and a currency mismatch refuses before any reservation or settlement mutation. Per-currency counter partitions would need to be distinct rows with their own tag and cannot act as an implicit conversion. Reserving only after a provider response was rejected because multiple individually valid requests could collectively exceed a shared ceiling. Releasing on timeout/lease expiry was rejected because dispatch may already have occurred. Treating maximum exposure as the final customer charge was rejected because only verified actual usage plus billed waste settles. Negative balances/refunds as mutation of historic charges were rejected; append a typed reversal and preserve the original entry. Floating point, implicit currency conversion, and guessing currency minor units were rejected by the exact value contract and commerce's pinned-rate boundary.

## Finite executable contract example

This `std`-only model includes the exact `Currency`, `Money`, and `MoneyError` value contract excerpt frozen in `planning/money-arithmetic-contract.md` at commit `924af3ec6cbfd08f62259720299e6d335af986ef` (SHA-256 `a20e8a0c8c1d33572868dbb145c7c30e4962c83a8ef30239c699b35711c69437`). Those canonical value definitions are reused unchanged; only the commerce hierarchy and obligation policy are illustrated here. It is not production `df-commerce`, a repository, provider or runtime behavior. Counter values are synthetic fixture amounts, not measured service limits or commercial defaults. The hierarchy is currency-bound and every counter is represented by canonical tagged `Money`. The production hierarchy, IDs, quote binding, transaction, journal and adapter contracts still require their owners' implementation and review.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MoneyError {
    InvalidCurrencyTag,
    CurrencyMismatch,
    UnitMismatch,
    Overflow,
    Underflow,
    ZeroDenominator,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Currency([u8; 3]);

impl Currency {
    fn parse(value: &str) -> Result<Self, MoneyError> {
        let bytes = value.as_bytes();
        if bytes.len() != 3 || !bytes.iter().all(u8::is_ascii_uppercase) {
            return Err(MoneyError::InvalidCurrencyTag);
        }
        Ok(Self([bytes[0], bytes[1], bytes[2]]))
    }

    fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Money {
    currency: Currency,
    micros: u128,
}

impl Money {
    fn new(currency: Currency, micros: u128) -> Self {
        Self { currency, micros }
    }

    fn currency(self) -> Currency {
        self.currency
    }

    fn micros(self) -> u128 {
        self.micros
    }

    fn checked_add(self, other: Self) -> Result<Self, MoneyError> {
        if self.currency != other.currency {
            return Err(MoneyError::CurrencyMismatch);
        }
        Ok(Self::new(
            self.currency,
            self.micros
                .checked_add(other.micros)
                .ok_or(MoneyError::Overflow)?,
        ))
    }

    fn checked_sub(self, other: Self) -> Result<Self, MoneyError> {
        if self.currency != other.currency {
            return Err(MoneyError::CurrencyMismatch);
        }
        Ok(Self::new(
            self.currency,
            self.micros
                .checked_sub(other.micros)
                .ok_or(MoneyError::Underflow)?,
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UsageUnit {
    Token,
    Character,
    Byte,
    AudioMillisecond,
    VideoMillisecond,
    Image,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Usage {
    quantity: u128,
    unit: UsageUnit,
}

impl Usage {
    fn new(quantity: u128, unit: UsageUnit) -> Self {
        Self { quantity, unit }
    }

    fn quantity(self) -> u128 {
        self.quantity
    }

    fn unit(self) -> UsageUnit {
        self.unit
    }

    fn checked_add(self, other: Self) -> Result<Self, MoneyError> {
        if self.unit != other.unit {
            return Err(MoneyError::UnitMismatch);
        }
        Ok(Self::new(
            self.quantity
                .checked_add(other.quantity)
                .ok_or(MoneyError::Overflow)?,
            self.unit,
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LiabilityRate {
    currency: Currency,
    unit: UsageUnit,
    numerator_micros: u128,
    denominator_usage_units: u128,
}

impl LiabilityRate {
    fn new(
        currency: Currency,
        unit: UsageUnit,
        numerator_micros: u128,
        denominator_usage_units: u128,
    ) -> Result<Self, MoneyError> {
        if denominator_usage_units == 0 {
            return Err(MoneyError::ZeroDenominator);
        }
        Ok(Self {
            currency,
            unit,
            numerator_micros,
            denominator_usage_units,
        })
    }

    fn currency(self) -> Currency {
        self.currency
    }

    fn unit(self) -> UsageUnit {
        self.unit
    }

    fn numerator_micros(self) -> u128 {
        self.numerator_micros
    }

    fn denominator_usage_units(self) -> u128 {
        self.denominator_usage_units
    }

    fn liability(self, usage: Usage) -> Result<Money, MoneyError> {
        if self.unit != usage.unit {
            return Err(MoneyError::UnitMismatch);
        }
        let product = usage
            .quantity
            .checked_mul(self.numerator_micros)
            .ok_or(MoneyError::Overflow)?;
        let quotient = product / self.denominator_usage_units;
        let micros = if product % self.denominator_usage_units == 0 {
            quotient
        } else {
            quotient.checked_add(1).ok_or(MoneyError::Overflow)?
        };
        Ok(Money::new(self.currency, micros))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Error {
    Money(MoneyError),
    BudgetExceeded,
    CustomerConsentExceeded,
    UnknownCannotRelease,
    ActualExceedsMaximum,
    InvalidTransition,
}

impl From<MoneyError> for Error {
    fn from(error: MoneyError) -> Self {
        Self::Money(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Obligation {
    Reserved,
    UnknownLiability,
    SettledKnown { actual: Money },
    Released,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Reservation {
    maximum: Money,
    customer_cap: Money,
    obligation: Obligation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Hierarchy {
    currency: Currency,
    // Platform, supplier, tenant/payer, campaign, job, in that order.
    used: [Money; 5],
    limit: [Money; 5],
}

impl Hierarchy {
    fn new(currency: Currency, limits: [Money; 5]) -> Result<Self, Error> {
        if limits.iter().any(|amount| amount.currency() != currency) {
            return Err(MoneyError::CurrencyMismatch.into());
        }
        Ok(Self {
            currency,
            used: [Money::new(currency, 0); 5],
            limit: limits,
        })
    }

    fn validate_currency(&self, currency: Currency) -> Result<(), Error> {
        if currency != self.currency
            || self
                .used
                .iter()
                .any(|amount| amount.currency() != self.currency)
            || self
                .limit
                .iter()
                .any(|amount| amount.currency() != self.currency)
        {
            return Err(MoneyError::CurrencyMismatch.into());
        }
        Ok(())
    }

    fn reserve(&mut self, maximum: Money, customer_cap: Money) -> Result<Reservation, Error> {
        self.validate_currency(maximum.currency())?;
        self.validate_currency(customer_cap.currency())?;
        if maximum.micros() > customer_cap.micros() {
            return Err(Error::CustomerConsentExceeded);
        }

        let mut next = self.used;
        for (index, used) in next.iter_mut().enumerate() {
            *used = used.checked_add(maximum)?;
            if used.micros() > self.limit[index].micros() {
                return Err(Error::BudgetExceeded);
            }
        }
        self.used = next;
        Ok(Reservation {
            maximum,
            customer_cap,
            obligation: Obligation::Reserved,
        })
    }

    fn mark_unknown(&self, reservation: &mut Reservation) -> Result<(), Error> {
        self.validate_currency(reservation.maximum.currency())?;
        self.validate_currency(reservation.customer_cap.currency())?;
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
        self.validate_currency(reservation.maximum.currency())?;
        self.validate_currency(reservation.customer_cap.currency())?;
        if reservation.obligation == Obligation::UnknownLiability {
            return Err(Error::UnknownCannotRelease);
        }
        if reservation.obligation != Obligation::Reserved || !dispatcher_proved_no_send {
            return Err(Error::InvalidTransition);
        }
        self.adjust_all(reservation.maximum, Money::new(self.currency, 0))?;
        reservation.obligation = Obligation::Released;
        Ok(())
    }

    fn settle_known(
        &mut self,
        reservation: &mut Reservation,
        actual_including_billed_waste: Money,
    ) -> Result<(), Error> {
        self.validate_currency(reservation.maximum.currency())?;
        self.validate_currency(reservation.customer_cap.currency())?;
        self.validate_currency(actual_including_billed_waste.currency())?;
        if !matches!(
            reservation.obligation,
            Obligation::Reserved | Obligation::UnknownLiability
        ) {
            return Err(Error::InvalidTransition);
        }
        if actual_including_billed_waste.micros() > reservation.maximum.micros() {
            return Err(Error::ActualExceedsMaximum);
        }
        if actual_including_billed_waste.micros() > reservation.customer_cap.micros() {
            return Err(Error::CustomerConsentExceeded);
        }
        self.adjust_all(reservation.maximum, actual_including_billed_waste)?;
        reservation.obligation = Obligation::SettledKnown {
            actual: actual_including_billed_waste,
        };
        Ok(())
    }

    fn adjust_all(&mut self, from: Money, to: Money) -> Result<(), Error> {
        self.validate_currency(from.currency())?;
        self.validate_currency(to.currency())?;
        let mut next = self.used;
        for (index, used) in next.iter_mut().enumerate() {
            *used = used.checked_sub(from)?.checked_add(to)?;
            if used.micros() > self.limit[index].micros() {
                return Err(Error::BudgetExceeded);
            }
        }
        self.used = next;
        Ok(())
    }
}

fn tagged_limits(currency: Currency, micros: [u128; 5]) -> [Money; 5] {
    micros.map(|amount| Money::new(currency, amount))
}

fn main() {
    let usd = Currency::parse("USD").unwrap();
    let eur = Currency::parse("EUR").unwrap();
    assert_eq!(usd.as_str(), "USD");
    assert_eq!(
        Money::new(usd, 3).checked_add(Money::new(usd, 4)),
        Ok(Money::new(usd, 7))
    );
    assert_eq!(
        Money::new(usd, 0).checked_sub(Money::new(usd, 1)),
        Err(MoneyError::Underflow)
    );
    assert_eq!(
        Usage::new(2, UsageUnit::Token).checked_add(Usage::new(3, UsageUnit::Token)),
        Ok(Usage::new(5, UsageUnit::Token))
    );
    let usage = Usage::new(5, UsageUnit::Token);
    assert_eq!((usage.quantity(), usage.unit()), (5, UsageUnit::Token));
    for unit in [
        UsageUnit::Token,
        UsageUnit::Character,
        UsageUnit::Byte,
        UsageUnit::AudioMillisecond,
        UsageUnit::VideoMillisecond,
        UsageUnit::Image,
    ] {
        assert_eq!(Usage::new(0, unit).unit(), unit);
        assert_eq!(
            Usage::new(2, unit).checked_add(Usage::new(3, unit)),
            Ok(Usage::new(5, unit))
        );
    }
    assert_eq!(
        Usage::new(1, UsageUnit::AudioMillisecond)
            .checked_add(Usage::new(1, UsageUnit::VideoMillisecond)),
        Err(MoneyError::UnitMismatch)
    );
    assert_eq!(
        Usage::new(u128::MAX, UsageUnit::Token).checked_add(Usage::new(1, UsageUnit::Token)),
        Err(MoneyError::Overflow)
    );
    let rate = LiabilityRate::new(usd, UsageUnit::Token, 2, 3).unwrap();
    assert_eq!(
        (
            rate.currency(),
            rate.unit(),
            rate.numerator_micros(),
            rate.denominator_usage_units()
        ),
        (usd, UsageUnit::Token, 2, 3)
    );
    assert_eq!(
        rate.liability(Usage::new(5, UsageUnit::Token)),
        Ok(Money::new(usd, 4))
    );
    assert_eq!(
        rate.liability(Usage::new(1, UsageUnit::Byte)),
        Err(MoneyError::UnitMismatch)
    );
    assert_eq!(
        LiabilityRate::new(usd, UsageUnit::Token, 1, 2)
            .unwrap()
            .liability(Usage::new(1, UsageUnit::Token)),
        Ok(Money::new(usd, 1))
    );
    assert_eq!(
        LiabilityRate::new(usd, UsageUnit::Token, 0, 0),
        Err(MoneyError::ZeroDenominator)
    );
    assert_eq!(
        LiabilityRate::new(usd, UsageUnit::Token, 2, 3)
            .unwrap()
            .liability(Usage::new(u128::MAX, UsageUnit::Token)),
        Err(MoneyError::Overflow)
    );
    let zero = Money::new(usd, 0);
    let maximum = Money::new(usd, u128::MAX);
    assert_eq!(zero.micros(), 0);
    assert_eq!(maximum.micros(), u128::MAX);
    assert_eq!(
        Money::new(usd, 1).checked_add(Money::new(eur, 1)),
        Err(MoneyError::CurrencyMismatch)
    );
    assert_eq!(
        Money::new(usd, u128::MAX).checked_add(Money::new(usd, 1)),
        Err(MoneyError::Overflow)
    );

    // A single hierarchy is bound to USD. EUR admission refuses before any counter changes.
    assert_eq!(
        Hierarchy::new(
            usd,
            [
                Money::new(usd, 100),
                Money::new(eur, 100),
                Money::new(usd, 100),
                Money::new(usd, 100),
                Money::new(usd, 100)
            ]
        ),
        Err(Error::Money(MoneyError::CurrencyMismatch))
    );
    let mut budgets = Hierarchy::new(usd, tagged_limits(usd, [100, 80, 60, 50, 40])).unwrap();
    let usd_max = Money::new(usd, 40);
    let usd_cap = Money::new(usd, 45);
    let mut unknown = budgets.reserve(usd_max, usd_cap).unwrap();
    assert_eq!(budgets.used, [usd_max; 5]);
    let before_currency_refusal = budgets.used;
    assert_eq!(
        budgets.reserve(Money::new(eur, 40), Money::new(eur, 45)),
        Err(Error::Money(MoneyError::CurrencyMismatch))
    );
    assert_eq!(budgets.used, before_currency_refusal);
    assert_eq!(budgets.currency, usd);
    assert_eq!(
        budgets.reserve(usd_max, Money::new(eur, 45)),
        Err(Error::Money(MoneyError::CurrencyMismatch))
    );
    assert_eq!(budgets.used, before_currency_refusal);

    // A foreign settlement or reservation cannot mutate this USD hierarchy.
    let before_settlement_refusal = budgets.used;
    assert_eq!(
        budgets.settle_known(&mut unknown, Money::new(eur, 27)),
        Err(Error::Money(MoneyError::CurrencyMismatch))
    );
    assert_eq!(unknown.obligation, Obligation::Reserved);
    assert_eq!(budgets.used, before_settlement_refusal);
    let mut euro_budgets = Hierarchy::new(eur, tagged_limits(eur, [100; 5])).unwrap();
    assert_eq!(
        euro_budgets.settle_known(&mut unknown, Money::new(usd, 27)),
        Err(Error::Money(MoneyError::CurrencyMismatch))
    );
    assert_eq!(
        euro_budgets.mark_unknown(&mut unknown),
        Err(Error::Money(MoneyError::CurrencyMismatch))
    );
    assert_eq!(
        euro_budgets.release_unsent(&mut unknown, true),
        Err(Error::Money(MoneyError::CurrencyMismatch))
    );
    assert_eq!(unknown.obligation, Obligation::Reserved);
    assert_eq!(euro_budgets.used, [Money::new(eur, 0); 5]);

    // Unknown keeps the full maximum; over-maximum and timeout release both refuse atomically.
    budgets.mark_unknown(&mut unknown).unwrap();
    assert_eq!(
        budgets.settle_known(&mut unknown, Money::new(usd, 41)),
        Err(Error::ActualExceedsMaximum)
    );
    assert_eq!(unknown.obligation, Obligation::UnknownLiability);
    assert_eq!(
        budgets.release_unsent(&mut unknown, true),
        Err(Error::UnknownCannotRelease)
    );
    assert_eq!(budgets.used, before_currency_refusal);

    // Customer-consent and each of the five hierarchy limits refuse without partial mutation.
    assert_eq!(
        budgets.reserve(Money::new(usd, 10), Money::new(usd, 9)),
        Err(Error::CustomerConsentExceeded)
    );
    assert_eq!(budgets.used, before_currency_refusal);
    for index in 0..5 {
        let mut limits = [100; 5];
        limits[index] = 40;
        let mut one_tight_scope = Hierarchy::new(usd, tagged_limits(usd, limits)).unwrap();
        let before = one_tight_scope.used;
        assert_eq!(
            one_tight_scope.reserve(Money::new(usd, 41), Money::new(usd, 41)),
            Err(Error::BudgetExceeded)
        );
        assert_eq!(one_tight_scope.used, before);
    }
    assert_eq!(
        budgets.reserve(Money::new(usd, 11), Money::new(usd, 11)),
        Err(Error::BudgetExceeded)
    );
    assert_eq!(budgets.used, before_currency_refusal);

    // Checked overflow and adjustment failures preserve every counter.
    for index in 0..5 {
        let mut used = [Money::new(usd, 0); 5];
        let mut limits = [Money::new(usd, u128::MAX); 5];
        used[index] = Money::new(usd, u128::MAX);
        limits[index] = Money::new(usd, u128::MAX);
        let mut overflow_scope = Hierarchy {
            currency: usd,
            used,
            limit: limits,
        };
        let before = overflow_scope.used;
        assert_eq!(
            overflow_scope.reserve(Money::new(usd, 1), Money::new(usd, 1)),
            Err(Error::Money(MoneyError::Overflow))
        );
        assert_eq!(overflow_scope.used, before);
    }
    let mut adjustment_scope = Hierarchy::new(usd, tagged_limits(usd, [u128::MAX; 5])).unwrap();
    let before_adjustment = adjustment_scope.used;
    assert_eq!(
        adjustment_scope.adjust_all(Money::new(usd, 1), Money::new(usd, 0)),
        Err(Error::Money(MoneyError::Underflow))
    );
    assert_eq!(adjustment_scope.used, before_adjustment);

    // Verified actual plus billed waste settles once and releases only known unused maximum.
    budgets
        .settle_known(&mut unknown, Money::new(usd, 27))
        .unwrap();
    assert_eq!(budgets.used, [Money::new(usd, 27); 5]);
    assert_eq!(
        unknown.obligation,
        Obligation::SettledKnown {
            actual: Money::new(usd, 27)
        }
    );
    assert_eq!(
        budgets.settle_known(&mut unknown, Money::new(usd, 27)),
        Err(Error::InvalidTransition)
    );
    assert_eq!(budgets.used, [Money::new(usd, 27); 5]);

    // Explicitly proved unsent releases a same-currency reservation.
    let mut empty = Hierarchy::new(usd, tagged_limits(usd, [100, 80, 60, 50, 40])).unwrap();
    let mut unsent = empty.reserve(usd_max, usd_cap).unwrap();
    assert!(empty.release_unsent(&mut unsent, false).is_err());
    assert_eq!(empty.used, [usd_max; 5]);
    empty.release_unsent(&mut unsent, true).unwrap();
    assert_eq!(empty.used, [Money::new(usd, 0); 5]);
    assert_eq!(unsent.obligation, Obligation::Released);
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
- `planning/money-arithmetic-contract.md` at frozen source commit `924af3ec6cbfd08f62259720299e6d335af986ef` (SHA-256 `a20e8a0c8c1d33572868dbb145c7c30e4962c83a8ef30239c699b35711c69437`), together with [Money, usage and elapsed-time units](typed-units-policy.md): the canonical tagged `Currency`, `Money`, and `MoneyError` value contract; gateway conversions remain consumer-owned.
- [Protected journal durability boundary](protected-journal-policy.md) and [Service operations](service-operations.md): possible sends, money and privacy suppression require append/head confirmation and fail-closed restore/reconciliation.
- [Subsystem architecture](subsystem-architecture.md), [Subsystem interfaces](subsystem-interfaces.md), [Storage architecture](storage-architecture.md): one pure commerce authority, existing `BudgetStore` delegation, native I/O composition, and persistence/journal owners.
- [Runtime reliability](runtime-reliability.md): accepted work lifetime is independent of caller cancellation; attempt/run fencing and Unknown paid calls cannot be replayed or refunded by inference.
- [Implementation roadmap](implementation-roadmap.md): prerequisite gates and later integration ownership; planning contract is not deployed behavior.

Source hashes are retained in the attempt evidence. This proposal preserves the original objective and remains subject to independent frontier review and root integration.
