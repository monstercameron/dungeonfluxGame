# Money, usage and elapsed-time units

Task/attempt: `B-C-df-types-D03/a1`  
Owner: `df-types`  
Status: Pure value-contract decision; production implementation and consuming
adapters remain pending.

## Decision

Represent nonnegative currency amounts as an unsigned `u128` count of millionths
of the tagged currency unit. The scale is fixed: `1_000_000` currency micro-units
equal one whole unit (for USD, `$1.00` is `1_000_000` USD micro-units). This is a
storage/arithmetic scale, not a claim that a currency, gateway, provider, or
customer-facing price supports six decimal places. `u128` is available on the
planned native and WASM targets and leaves a large, explicit bound without an
arbitrary signed range. The maximum is still finite; every arithmetic operation
must be checked and return a typed overflow/underflow error. Zero amounts are
valid values. Negative values are not representable; adjustments that reverse a
historic ledger entry belong to the commerce ledger's typed entry model.

Every amount carries an explicit currency tag containing exactly three uppercase
ASCII letters. Construction rejects any other length or byte. The tag is only a
syntactic discriminator: `USD`, `EUR`, or any other syntactically valid tag does
not prove that the currency is supported, that its scale is known, or that a quote
or conversion is authorized. Addition and subtraction require equal tags and
return a typed currency-mismatch error otherwise. Subtraction that would produce
a negative amount returns a typed underflow error. No implicit conversion exists.

For a liability quote, retain an integer rational rate and its pinned quote
identity/version outside or alongside the pure rate value. The rate is
`numerator / denominator` currency micro-units per one declared usage unit; the
quote's currency and usage unit are explicit. A zero denominator returns a typed
error. Calculate a nonnegative liability with checked integer multiplication,
integer division and a checked increment when a remainder exists, which is exact
ceiling division. Thus every fractional micro-unit is rounded up, never down,
when reserving maximum supplier liability. Overflow in either multiplication or
the ceiling increment is a typed error. Exact divisibility is unchanged, and zero
usage has zero liability. No floating point, silent truncation, or automatic
currency conversion is permitted. The commerce policy separately requires
liability rounding up and deliberate settlement; this value contract does not
invent a settlement rounding rule.

Currency amounts are not a gateway's minor-unit representation. A native gateway
adapter owns conversion to the selected gateway's scale and policy, with
liabilities rounded conservatively and the rate/scale pinned to the approved
quote. A conversion that cannot be represented safely must fail explicitly; it
must not round a liability downward. Customer consent caps, supplier estimates,
worst-case reservations and audited actual invoice settlement remain distinct
commerce concepts.

Usage is a nonnegative integer quantity paired with a closed unit kind: token,
character, byte, audio millisecond, video millisecond, or image count. Addition
requires the same unit and uses checked arithmetic; unlike quantities cannot be
summed and return a typed unit-mismatch error. Zero is valid. The unit enum
prevents mixing counts such as audio milliseconds and video milliseconds even
though both use an integer millisecond quantity. Provider-specific definitions
of such details as tokenization, character counting, billable minimums, and
rounding to billed units must be supplied and pinned by the quote-owning adapter;
the primitive does not guess them.

Use Rust `std::time::Duration` for elapsed intervals, with its standard checked
operations when combining or scaling values. It does not identify a clock,
schedule a timer, or encode a wall-clock instant. Logical game time remains the
explicit input owned by `df-engine`/`df-session`; wall-clock timestamps and their
wire/storage units are separately named and owned by their consumer. A duration
must not be substituted for either timestamp or logical time.

The existing architecture keeps `df-types` pure, small and dependency-free.
It owns the currency tag/value, usage quantity/unit, rational rate arithmetic,
and typed mismatch/overflow/zero-denominator errors. `df-commerce` owns quote
identity and version, consent, reservations, ledger settlement and payment
policy. The provider/quote adapter owns provider-specific usage semantics and
the pinned rate facts. `df-server`/`df-providers` native gateway adapters own
gateway scale conversion. `df-api`, `df-client`, and `df-persistence` owners
later define explicit boundary codecs/mappings; this decision adds no wire,
database, SDK, payment, provider, clock, or storage contract.

## Rationale and alternatives

The commerce service requires exact integer currency/subcurrency units, says
fixed-point liability calculations round upward, and forbids currency conversion
without a pinned rate. Pricing research separately says monetary implementation
must use exact integer/fixed-point values and treats current supplier rates as
dated research rather than approved customer quotes. This representation makes
those constraints visible while remaining pure. A micro-unit scale is chosen to
represent finer supplier-cost fractions before a gateway conversion; it does not
change any real currency's minor-unit rules.

Binary floating point was rejected because equality, multiplication and rounding
would not preserve exact monetary liability. A signed integer was rejected
because the selected primitive represents nonnegative amounts; refunds and
reversals are separate commerce events, not negative balances. A fixed-width
`u64` was rejected because it imposes a smaller maximum without a domain-derived
limit. Arbitrary-precision decimal was rejected because it adds a dependency and
unbounded representation to a small native/WASM primitive when a fixed scale and
checked bound suffice. Inferring a currency's registry validity or minor-unit
scale from its three-letter syntax was rejected because that would confer
unsupported commercial authority.

## Bounded executable contract example

This stand-alone example checks the value-level rules only. It is not production
`df-types` code or evidence of a quote, gateway, ledger, provider, persistence,
or end-to-end service. Save the literal Rust fence to a fresh source file, format
it with the repository `rustfmt.toml`, and compile/run it with cached Rust
1.98.1 and warnings denied; do not use Cargo or download tools/dependencies.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Currency([u8; 3]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ContractError {
    InvalidCurrencyTag,
    CurrencyMismatch,
    UnitMismatch,
    Overflow,
    Underflow,
    ZeroDenominator,
}

impl Currency {
    fn parse(value: &str) -> Result<Self, ContractError> {
        let bytes = value.as_bytes();
        if bytes.len() != 3 || !bytes.iter().all(u8::is_ascii_uppercase) {
            return Err(ContractError::InvalidCurrencyTag);
        }
        Ok(Self([bytes[0], bytes[1], bytes[2]]))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Money {
    currency: Currency,
    micros: u128,
}

impl Money {
    fn checked_add(self, other: Self) -> Result<Self, ContractError> {
        if self.currency != other.currency {
            return Err(ContractError::CurrencyMismatch);
        }
        Ok(Self {
            currency: self.currency,
            micros: self
                .micros
                .checked_add(other.micros)
                .ok_or(ContractError::Overflow)?,
        })
    }

    fn checked_sub(self, other: Self) -> Result<Self, ContractError> {
        if self.currency != other.currency {
            return Err(ContractError::CurrencyMismatch);
        }
        Ok(Self {
            currency: self.currency,
            micros: self
                .micros
                .checked_sub(other.micros)
                .ok_or(ContractError::Underflow)?,
        })
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
    fn checked_add(self, other: Self) -> Result<Self, ContractError> {
        if self.unit != other.unit {
            return Err(ContractError::UnitMismatch);
        }
        Ok(Self {
            quantity: self
                .quantity
                .checked_add(other.quantity)
                .ok_or(ContractError::Overflow)?,
            unit: self.unit,
        })
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
    fn liability(self, usage: Usage) -> Result<Money, ContractError> {
        if self.unit != usage.unit {
            return Err(ContractError::UnitMismatch);
        }
        if self.denominator_usage_units == 0 {
            return Err(ContractError::ZeroDenominator);
        }
        let product = usage
            .quantity
            .checked_mul(self.numerator_micros)
            .ok_or(ContractError::Overflow)?;
        let quotient = product / self.denominator_usage_units;
        let micros = if product % self.denominator_usage_units == 0 {
            quotient
        } else {
            quotient.checked_add(1).ok_or(ContractError::Overflow)?
        };
        Ok(Money {
            currency: self.currency,
            micros,
        })
    }
}

fn main() {
    let usd = Currency::parse("USD").unwrap();
    let eur = Currency::parse("EUR").unwrap();
    assert_eq!(
        Currency::parse("Usd"),
        Err(ContractError::InvalidCurrencyTag)
    );
    assert_eq!(
        Money {
            currency: usd,
            micros: 1
        }
        .checked_add(Money {
            currency: eur,
            micros: 1
        }),
        Err(ContractError::CurrencyMismatch)
    );
    assert_eq!(
        Money {
            currency: usd,
            micros: u128::MAX
        }
        .checked_add(Money {
            currency: usd,
            micros: 1
        }),
        Err(ContractError::Overflow)
    );
    assert_eq!(
        Money {
            currency: usd,
            micros: 1
        }
        .checked_sub(Money {
            currency: usd,
            micros: 2
        }),
        Err(ContractError::Underflow)
    );
    assert_eq!(
        Usage {
            quantity: 1,
            unit: UsageUnit::AudioMillisecond
        }
        .checked_add(Usage {
            quantity: 1,
            unit: UsageUnit::VideoMillisecond
        }),
        Err(ContractError::UnitMismatch)
    );
    assert_eq!(
        Usage {
            quantity: 2,
            unit: UsageUnit::Character
        }
        .checked_add(Usage {
            quantity: 3,
            unit: UsageUnit::Character
        })
        .unwrap()
        .quantity,
        5
    );
    assert_eq!(
        Usage {
            quantity: 2,
            unit: UsageUnit::Image
        }
        .checked_add(Usage {
            quantity: 3,
            unit: UsageUnit::Image
        })
        .unwrap()
        .quantity,
        5
    );

    let rate = LiabilityRate {
        currency: usd,
        unit: UsageUnit::Token,
        numerator_micros: 2,
        denominator_usage_units: 3,
    };
    assert_eq!(
        rate.liability(Usage {
            quantity: 5,
            unit: UsageUnit::Token
        })
        .unwrap()
        .micros,
        4
    );
    assert_eq!(
        rate.liability(Usage {
            quantity: 6,
            unit: UsageUnit::Token
        })
        .unwrap()
        .micros,
        4
    );
    assert_eq!(
        rate.liability(Usage {
            quantity: 0,
            unit: UsageUnit::Token
        })
        .unwrap()
        .micros,
        0
    );
    assert_eq!(
        rate.liability(Usage {
            quantity: 1,
            unit: UsageUnit::Byte
        }),
        Err(ContractError::UnitMismatch)
    );
    assert_eq!(
        LiabilityRate {
            denominator_usage_units: 0,
            ..rate
        }
        .liability(Usage {
            quantity: 1,
            unit: UsageUnit::Token
        }),
        Err(ContractError::ZeroDenominator)
    );
    assert_eq!(
        LiabilityRate {
            numerator_micros: 2,
            denominator_usage_units: 3,
            ..rate
        }
        .liability(Usage {
            quantity: u128::MAX,
            unit: UsageUnit::Token
        }),
        Err(ContractError::Overflow)
    );
}
```

## Deferred decisions and consumers

- The supported-currency registry, who approves it, each currency's gateway
  minor-unit scale, and exact external cash/credit rounding remain commercial and
  gateway-owner decisions. The tag is not a registry lookup.
- Quote lifecycle/authority, stable quote IDs and revisions, rate effective
  interval/expiry, supplier estimate versus invoice facts, and who may pin or
  approve a quote remain with `df-commerce` and the quote/provider owners. A
  ratio without those approved facts is arithmetic input, not an authorized
  price.
- Provider billing semantics, token/character counting, billed-unit minimums,
  and provider-side rounding remain adapter-owned, captured by the quote before
  admission. The consumer may not infer them from the `UsageUnit` label.
- Gateway conversion from micro-units to a currency's external minor units,
  fee/tax treatment, customer display formatting, and settlement rounding need
  explicit adapter/commerce decisions and tests. The adapter must never reduce
  an admitted maximum liability through rounding.
- Wire/JSON/protobuf and database encodings of `u128`, tag validation at those
  boundaries, migration/version policy, and public API error mapping belong to
  `df-api`, `df-client`, and `df-persistence` consumer waves. No serializer or
  schema is selected here.
- The exact character-count definition and any sub-millisecond provider billing
  unit are unresolved. Do not approximate these by silently changing unit scale;
  a consumer must add an explicitly reviewed unit/quote contract if needed.

## Governing sources

- [Subsystem architecture](subsystem-architecture.md): `df-types` owns small
  pure units; commerce composition has its own native policy boundary.
- [Subsystem interfaces](subsystem-interfaces.md): timestamps/durations have
  explicit units; money/usage records and quotes are exact and consumer-owned.
- [Commerce service](commerce-service.md): integer currency/subcurrency units,
  upward fixed-point liability rounding, deliberate settlement, and no
  conversion without a pinned rate.
- [Pricing and costs](pricing-and-costs.md): exact integer/fixed-point monetary
  implementation; supplier prices are dated research inputs, and versioned
  quotes pin actual supplier/model/specification before admission.
- [Shared contract waves](shared-contract-waves.md), [recovery revision
  policy](recovery-revision-policy.md), and [shared contracts](../development/shared-contracts.md):
  keep later contracts scoped, consumers coordinated, elapsed `Duration`
  distinct, and pure types free of runtime/storage authority.

The cited frozen inputs and exact source hashes are preserved in
`development/evidence/types-decisions/B-C-df-types-D03/a1/brief.json`. This
decision does not claim production source, quote, provider, gateway, storage, or
integrated behavior.
