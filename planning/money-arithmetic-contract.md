# Exact money arithmetic contract for B-C-df-types-I03

Task/attempt: `MONEY-I03-CONTRACT-001/a1`
Input source: `af6d1bebbd702d1252cd091927e3fa1fad6a5aee`
Status: proposed bounded `df-types` contract and I03-only prerequisite decision; implementation, consuming source, graph migration and original I03 acceptance remain pending.

## Decision boundary and canonical linkage

The original `B-C-df-types-I03` objective is “Implement exact money arithmetic; expected: liability rounding is conservative and currency explicit.” Its unchanged acceptance also requires actual source/build-bound evidence, with unsupported, pending, failed and unperformed checks explicit. Its unchanged verification requires the smallest owned implementation or use-case wiring and an executed deterministic normal and failure fixture. This document freezes the proposed public contract and exact future checks. Its standalone literal only validates the proposed value rules; it is neither production `df-types` nor I03 acceptance.

`planning/typed-units-policy.md` is the accepted `B-C-df-types-D03/a1` value decision. It selects unsigned `u128` micro-units and quantity, three-uppercase-ASCII currency tags, closed usage units, checked arithmetic, and rational liability rounded upward. `planning/shared-contract-waves.md` is the accepted `B-G03-D01/a2` bounded-wave policy. It names `CONTRACT-G03-001` as the canonical first S00/S01 identity/recovery/provenance/protobuf contract and requires later waves to admit only exact source-backed fields and consumers. This money wave **links to that contract**; it does not alter its already frozen IDs, revision, build provenance, DTOs, descriptors, field ledger or compatibility fixture. `development/shared-contracts.md` and `development/evidence/contracts-g03/{brief.json,integration.json,integration-review.json}` document the accepted historical boundary and its limits. This document is a distinct later pure-value refinement, not a competing common schema or approval of production commerce.

At the input, `crates/df-types/src/lib.rs` exports identity, revision and provenance only. There is no Money/Usage production module. `crates/df-tools/tests/shared_contracts.rs` is the **actual existing** external consuming test source. `crates/df-tools/src/shared_contracts.rs` does not exist. `df-tools` already has `df-types` as a dev-dependency and the first-wave fixture maps types to generated DTOs. The proposed money fixture uses the same external-consumer file, but only direct `df-types` value APIs: no money protobuf, serializer, database, gateway, provider, quote, ledger, auth, clock or runtime I/O is admitted. Source/configuration identity for any later execution must be freshly pinned; this decision does not claim a current native/WASM run.

## Exact proposed public `df-types` API

I03 owns one new private `crates/df-types/src/money.rs` module and the narrow re-export in `crates/df-types/src/lib.rs`. Public names, signatures, representation and behavior below are the contract for implementation review. All structs have private fields and may derive `Clone`, `Copy`, `Debug`, `Eq`, `PartialEq`; `UsageUnit` and `MoneyError` are closed enums. The module has no crate dependency, serde, SDK, clock, database, socket, provider or browser import.

| Type and public member | Meaning and invariant |
| --- | --- |
| `Currency::parse(&str) -> Result<Currency, MoneyError>` | Accept exactly three ASCII uppercase bytes; preserve them verbatim. All other length/byte cases return `InvalidCurrencyTag`. No registry or real-world currency-scale claim. |
| `Currency::as_str(&self) -> &str` | Return the validated three-letter tag. Its UTF-8 validity follows from the private ASCII constructor. |
| `Money::new(Currency, u128) -> Money` | Every nonnegative `u128` micro-unit count including zero and `u128::MAX` is a valid value. The currency is required, never inferred. |
| `Money::currency(self) -> Currency`; `Money::micros(self) -> u128` | Expose the exact tag and integer micro-units, where 1,000,000 is one whole tagged currency unit. |
| `Money::checked_add(self, Money) -> Result<Money, MoneyError>`; `checked_sub` with the same signature | First reject unlike currency as `CurrencyMismatch`; then use checked integer arithmetic. Addition overflow is `Overflow`; negative subtraction is `Underflow`. Equal-currency zero remains valid. No conversion. |
| `UsageUnit::{Token, Character, Byte, AudioMillisecond, VideoMillisecond, Image}` | The closed kinds from D03. A provider-specific billing meaning is not implied by a kind. |
| `Usage::new(u128, UsageUnit) -> Usage`; `quantity(self) -> u128`; `unit(self) -> UsageUnit` | Exact nonnegative integer quantity, including zero and maximum. Audio and video milliseconds remain distinct. No negative Rust value can reach this constructor. |
| `Usage::checked_add(self, Usage) -> Result<Usage, MoneyError>` | Reject unlike units first as `UnitMismatch`; then checked sum or `Overflow`. |
| `LiabilityRate::new(Currency, UsageUnit, u128, u128) -> Result<LiabilityRate, MoneyError>` | Arguments are currency, usage unit, numerator in currency micro-units, denominator in usage units. Zero numerator is valid. Zero denominator rejects as `ZeroDenominator` at construction. No quote authority is conveyed. |
| `LiabilityRate::{currency,unit,numerator_micros,denominator_usage_units}(self)` | Four exact accessors with return types `Currency`, `UsageUnit`, `u128`, `u128`, respectively. Validated denominator is always positive. |
| `LiabilityRate::liability(self, Usage) -> Result<Money, MoneyError>` | Reject unit mismatch first. Checked product of usage quantity and numerator, divide by positive denominator, then checked increment only when remainder is nonzero. Return `ceil(product/denominator)` micro-units with the rate currency. |
| `MoneyError::{InvalidCurrencyTag,CurrencyMismatch,UnitMismatch,Overflow,Underflow,ZeroDenominator}` | Typed domain rejection without supplied string, provider data or secret payload. No string parsing to select behavior. |

The error precedence is part of the public contract: currency mismatch precedes arithmetic for Money add/sub; usage-unit mismatch precedes arithmetic for Usage add and rate application; zero denominator rejects at rate construction even if a later usage quantity would be zero. An invalid `LiabilityRate` cannot be constructed through the public API. After a valid rate, zero usage yields zero liability. Checked intermediate multiplication overflow returns `Overflow` **even if an unbounded rational quotient might fit**; this is D03's bounded arithmetic decision. Do not silently introduce cancellation/reduction or wider integers as an alternative. Exact divisibility never increments. Every positive remainder increments once, including fractional values below one micro-unit. The final `checked_add(1)` remains in the algorithm as D03 requires, although with a checked `u128` product and positive denominator it cannot overflow: if remainder is nonzero, denominator is at least two and quotient is strictly less than product, hence below `u128::MAX`. Do not fabricate a reachable ceiling-increment overflow test. `Money` itself does not round to gateway minor units or customer display precision.

The pure rate holds no quote ID, revision, supplier, invoice, effective interval or authority. D03 requires a pinned quote identity/version outside or alongside arithmetic; the production consumer must supply that through the canonical `df-commerce` and provider/quote owners. `B-C-df-commerce-D03` owns exact hierarchical liability/settlement and Unknown obligations; `B-C-df-provider-api-D03` owns BudgetStore admission; supplier adapters own billable usage and rate facts. `df-server`/`df-providers` gateway adapters own external minor-unit conversion. `df-api`, `df-client` and `df-persistence` own their later wire/storage mappings. This contract neither approves a currency registry nor determines a real currency's minor-unit scale, refund/ledger reversal, tax/fee treatment or settlement rounding.

## Exact future I03 implementation and consumer fixture

Proposed implementation paths are `crates/df-types/src/money.rs` and its re-export in `crates/df-types/src/lib.rs`; proposed canonical external consumer path is `crates/df-tools/tests/shared_contracts.rs`, with `crates/df-tools/Cargo.toml` unchanged because it already declares the dev-dependency. The root must freeze these permitted paths, `df-tools` consumer ownership, current source and resource limits before changing I03's blueprint/dispatch state. The consumer must import public `df_types::{Currency, Money, Usage, UsageUnit, LiabilityRate, MoneyError}` and exercise the real compiled implementation, not a second local type or copied example. Do not introduce a new protobuf Money DTO or change `common.proto`/`field-ledger.txt` for this pure boundary. The existing descriptor-ledger fixture remains a compatibility regression, not a money codec. If a later feature needs wire representation, the appropriate canonical protocol/API/client/persistence owners must admit that as a separate bounded wave with exact u128 encoding, presence, error and version behavior.

Required deterministic normal observations in both `df-types` tests and the external native fixture: valid `USD` and `EUR` tags and exact currency retention; zero and maximum Money and Usage; every closed UsageUnit; same-tag add/sub and same-unit add; `5 × 2/3` rounds from `10/3` to `4` micro-units; `6 × 2/3` stays `4`; `1 × 1/2` rounds to `1`; zero usage under a valid rate stays zero; zero numerator stays zero; maximum exactly representable arithmetic such as `u128::MAX × 1/1`; and a near-maximum fractional quotient whose checked product fits. Compare actual public accessor values, not only absence of a panic.

Required deterministic refusals: currency tags of lengths 0, 2 and 4, lowercase/mixed case, whitespace, non-ASCII; cross-currency add and subtract including arithmetic-overflow/underflow candidates to prove mismatch precedence; equal-currency add overflow and subtract underflow; distinct audio/video millisecond kinds and other unlike-unit addition/application; Usage addition overflow; zero denominator at rate construction (including numerator/usage zero scenarios); and intermediate multiplication overflow. A negative quantity is unrepresentable in the selected public `u128` constructor; if a later raw/wire ingress is admitted, `B-C-df-types-F03` and its consumer mapping must reject negative or oversized input before accounting. Do not invent a string/JSON parser merely to force that case into I03. An overflow from the final ceiling increment is mathematically unreachable under the selected checked-product invariant and is a code invariant, not an omitted required failing fixture. The external consumer should also demonstrate `df-types` has no server/database/provider dependency on native or WASM.

Future I03 evidence must execute the native `shared_contracts` fixture against the real crate and compile that **test target** for `wasm32-unknown-unknown`. A WASM build of `df-tools --lib` alone does not compile the money test fixture; it may be an additional target gate. A WASM test cross-compile is not a WASM test run, browser observation or commercial flow. The independent frontier evaluator for this internal crate task should run/inspect the actual fixture and refusal results under ADR 0005; no user-facing visual or audible behavior is claimed. Later feature/slice consumers require their own integrated acceptance.

With cached Rust 1.98.1, offline mode, tracked `Cargo.lock`, serial compiler lock, `CARGO_BUILD_JOBS=1`, attempt-owned `CARGO_TARGET_DIR`/`TMPDIR`, and `RUSTUP_AUTO_INSTALL=0`, the future implementation/reviewer must retain exact command, exit, stdout/stderr, source/configuration hash and output artifact identity for these commands from the repository root:

```text
cargo +1.98.1 fmt --all -- --check
cargo +1.98.1 test --locked --offline -p df-types --target aarch64-apple-darwin --jobs 1
cargo +1.98.1 test --locked --offline -p df-tools --test shared_contracts --target aarch64-apple-darwin --jobs 1
cargo +1.98.1 test --locked --offline -p df-tools --test schema_ledger --target aarch64-apple-darwin --jobs 1
cargo +1.98.1 clippy --locked --offline -p df-types -p df-tools --all-targets --target aarch64-apple-darwin --jobs 1 -- -D warnings
cargo +1.98.1 clippy --locked --offline -p df-types -p df-tools --lib --target wasm32-unknown-unknown --jobs 1 -- -D warnings
cargo +1.98.1 test --locked --offline -p df-tools --test shared_contracts --target wasm32-unknown-unknown --no-run --jobs 1
cargo +1.98.1 build --locked --offline -p df-types -p df-tools --lib --target wasm32-unknown-unknown --jobs 1
```

Run dependency-tree inspection for `df-types` and the WASM consumer closure as an additional source-bound gate, using `cargo +1.98.1 tree --locked --offline -p df-types --target wasm32-unknown-unknown` and `cargo +1.98.1 tree --locked --offline -p df-tools --target wasm32-unknown-unknown`; reject newly introduced native-only I/O or provider/database dependencies in the `df-types` closure. The schema ledger check is a regression of the linked canonical contract; no field allocation is proposed here. Exact future commands may need a reviewed adjustment if the current toolchain cannot compile the WASM test harness, but that failure must be recorded as unverified rather than silently replaced with the unrelated library build. No command in this section has run for this source-only resolution.

## I03-only prerequisite applicability proposal

The original I03 graph includes `B-C-df-types-D01` through `D04`, `B-G01-R04`, and `B-G03-R04`. D03's typed-unit decision and the done G01-R04 dependency/feature closure directly apply. G03-R04's original objective is the **broad** later-wave extension procedure, “no unversioned breaking change,” across unfinished G02, effect, service and other shared-owner prerequisites. I03's selected pure value addition makes no wire allocation or breaking schema change and has a named bounded external fixture. Requiring closure of those unrelated broad waves before this isolated primitive is inconsistent with `B-G03-D01`'s accepted “admit each later shared contract in a bounded wave” rule. This is an applicability argument for independent root review, not an automatic waiver or an assertion that G03-R04 has passed.

After this resolution receives independent resulting-source approval and integration, propose that root retain D01–D04 and G01-R04, replace the I03-only G03-R04 edge with `B-G03-D01`, and add accepted `MONEY-I03-CONTRACT-001` as the exact money contract prerequisite. The historical `CONTRACT-G03-001` is a mandatory accepted-source receipt outside the seed graph: root must reread its original task, attempt, independent review and integrated proof; compare complete objective/acceptance/verification/brief and canonical payload/proof hashes; confirm present source reuse separately; and retain a dated verifier result in the generated I03 brief. An unverifiable/mismatched/revoked receipt blocks I03 dispatch. Root must likewise verify the integrated money contract source/hash and D03/G03-D01 approvals. If independent review finds a wire change or broader G03 behavior within I03's actual implementation, retain G03-R04 and resolve the extra ownership first. This proposal does not mutate SQLite, the seed manifests or original I03 now.

`B-C-df-types-W01` and `B-C-df-types-A02` depend on I03, so they cannot be added as I03 prerequisites without a cycle. They remain downstream shared export/round-trip work. Full G01/G02/G03 qualification, production quote/admission/ledger/gateway behavior, wire evolution, browser/WASM execution and other feature/slice acceptance remain separately pending. I01 and I02's earlier applicability decisions show a review pattern, not authority to change I03. An accepted contract page alone cannot mark I03 done or turn its pending implementation into a tested result.

## Finite value-level executable example

This single standalone Rust literal mirrors the proposed public behavior and meaningful normal/refusal cases. It has private fields and no dependencies. It is extracted to an attempt-owned scratch path, formatted against repository `rustfmt.toml`, compiled with cached Rust 1.98.1 with warnings denied, and executed under `guarded-design-command-v4.py`. The artifact proves this literal only. Production I03 and its actual `df-tools` consumer must be independently implemented and executed later.

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

fn main() {
    let usd = Currency::parse("USD").unwrap();
    let eur = Currency::parse("EUR").unwrap();
    assert_eq!(usd.as_str(), "USD");
    for bad in ["", "US", "USDD", "Usd", " USD", "US ", "\u{00c9}UR"] {
        assert_eq!(Currency::parse(bad), Err(MoneyError::InvalidCurrencyTag));
    }

    let zero = Money::new(usd, 0);
    assert_eq!(zero.currency(), usd);
    assert_eq!(zero.micros(), 0);
    assert_eq!(
        Money::new(usd, 3).checked_add(Money::new(usd, 4)),
        Ok(Money::new(usd, 7))
    );
    assert_eq!(
        Money::new(usd, 7).checked_sub(Money::new(usd, 4)),
        Ok(Money::new(usd, 3))
    );
    assert_eq!(
        Money::new(usd, 1).checked_add(Money::new(eur, u128::MAX)),
        Err(MoneyError::CurrencyMismatch)
    );
    assert_eq!(
        Money::new(usd, 0).checked_sub(Money::new(eur, 1)),
        Err(MoneyError::CurrencyMismatch)
    );
    assert_eq!(
        Money::new(usd, u128::MAX).checked_add(Money::new(usd, 1)),
        Err(MoneyError::Overflow)
    );
    assert_eq!(
        Money::new(usd, 0).checked_sub(Money::new(usd, 1)),
        Err(MoneyError::Underflow)
    );

    let kinds = [
        UsageUnit::Token,
        UsageUnit::Character,
        UsageUnit::Byte,
        UsageUnit::AudioMillisecond,
        UsageUnit::VideoMillisecond,
        UsageUnit::Image,
    ];
    for unit in kinds {
        assert_eq!(Usage::new(0, unit).unit(), unit);
        assert_eq!(Usage::new(u128::MAX, unit).quantity(), u128::MAX);
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
        rate.liability(Usage::new(6, UsageUnit::Token)),
        Ok(Money::new(usd, 4))
    );
    assert_eq!(
        rate.liability(Usage::new(0, UsageUnit::Token)),
        Ok(Money::new(usd, 0))
    );
    assert_eq!(
        LiabilityRate::new(usd, UsageUnit::Token, 1, 2)
            .unwrap()
            .liability(Usage::new(1, UsageUnit::Token)),
        Ok(Money::new(usd, 1))
    );
    assert_eq!(
        LiabilityRate::new(usd, UsageUnit::Token, 0, 1)
            .unwrap()
            .liability(Usage::new(u128::MAX, UsageUnit::Token)),
        Ok(Money::new(usd, 0))
    );
    assert_eq!(
        LiabilityRate::new(usd, UsageUnit::Token, 1, 1)
            .unwrap()
            .liability(Usage::new(u128::MAX, UsageUnit::Token)),
        Ok(Money::new(usd, u128::MAX))
    );
    assert_eq!(
        LiabilityRate::new(usd, UsageUnit::Token, u128::MAX, 2)
            .unwrap()
            .liability(Usage::new(1, UsageUnit::Token)),
        Ok(Money::new(usd, u128::MAX / 2 + 1))
    );
    assert_eq!(
        rate.liability(Usage::new(1, UsageUnit::Byte)),
        Err(MoneyError::UnitMismatch)
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
}
```

## Source and evidence limits

The sources for this decision are `AGENTS.md`, ADR 0001–0005, `planning/coding-style.md`, `planning/typed-units-policy.md`, `planning/shared-contract-waves.md`, `planning/subsystem-architecture.md`, `planning/subsystem-interfaces.md`, `planning/commerce-service.md`, `planning/pricing-and-costs.md`, `planning/reproducible-command-registry.md`, `development/shared-contracts.md`, the accepted `CONTRACT-G03-001` brief/integration/review, current `crates/df-types` and `crates/df-tools` source, and the original I03 task. The issued brief and retained attempt evidence bind exact input hashes and execution results. In particular, a standalone literal, cross-compilation, or historical first-wave integration cannot establish a production quote, gateway conversion, consumer execution on WASM, a browser flow, provider bill, full G02/G03 or I03 completion. Unsupported and unperformed checks remain explicit in the handoff.
