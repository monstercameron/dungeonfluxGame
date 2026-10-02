# C-df-locale-D03: typed placeholder and unit-formatting contract

Task/attempt: `B-C-df-locale-D03-a1`  
Input revision: `4c0e050c2d30e632bd3b32c51a92cfda52479e6e`  
Status: bounded `df-locale` contract decision. This is not production formatter code or integrated locale acceptance.

## Decision

`Catalog::format(TextKey, args)` uses named argument slots declared with each catalog message. A slot has one declared kind: plain `Text`, `Quantity(UsageUnit)`, or `Money(Currency)`. Catalog translations may reorder or repeat named slots, but the set of unique slot names and kinds must equal the source message's declaration. Catalog loading/validation rejects an entry whose slot set differs; formatting independently checks the supplied argument map.

Formatting returns a typed error for a missing slot, an undeclared extra argument, or a value whose kind does not match the slot. It does not stringify arbitrary input and parse that result back into a type. Repeated use of one slot reads the same single typed argument. Argument names are identifiers owned by the catalog, not values interpolated into output.

Text arguments remain text segments all the way through formatting. Formatting does not parse a text argument as markup, a template, a command, or a localization key. Any downstream rich-text renderer is responsible for encoding plain text for its output sink; no value is implicitly trusted markup.

Quantity arguments carry the existing exact `Usage` value, retaining both its unsigned integer quantity and closed `UsageUnit`. A localized display can change digit grouping and the translated unit label, but it cannot scale the quantity or change the unit. Money arguments carry existing `Money` plus its `Currency`; the tag stays explicit in the display. The value is an integer count of micro-units, with 1,000,000 micro-units per whole tagged currency unit. A decimal display therefore preserves all six fractional digits and does not round or infer a gateway minor-unit scale. Locale affects presentation only: formatted values are never fed back into rules, pricing, comparison, or settlement arithmetic.

This boundary deliberately exposes semantic formatted parts (`Text`, `Quantity`, `Money`) to the presentation caller. `df-locale` owns locale-sensitive digits, separators, and unit labels when that formatting policy is implemented; a caller owns sink-specific escaping. No plural-rule language, custom markup, conditionals, scripts, or general template engine is introduced here.

## Bounded contract example

This exact Rust literal is a standalone decision check for typed slot validation and semantic value preservation. It intentionally omits catalog lookup, locale negotiation, localized separators, markup rendering, and production `df-types` imports. It is not a substitute production implementation.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Unit {
    AudioMillisecond,
    VideoMillisecond,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Currency([u8; 3]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Usage {
    quantity: u128,
    unit: Unit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Money {
    currency: Currency,
    micros: u128,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Text,
    Quantity(Unit),
    Money(Currency),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Value {
    Text(String),
    Quantity(Usage),
    Money(Money),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Slot {
    Literal(String),
    Argument { name: String, kind: Kind },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Part {
    Literal(String),
    Text(String),
    Quantity(Usage),
    Money(Money),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FormatError {
    MissingArgument,
    ExtraArgument,
    WrongKind,
}

fn value_matches(kind: Kind, value: &Value) -> bool {
    match (kind, value) {
        (Kind::Text, Value::Text(_)) => true,
        (Kind::Quantity(expected), Value::Quantity(actual)) => expected == actual.unit,
        (Kind::Money(expected), Value::Money(actual)) => expected == actual.currency,
        _ => false,
    }
}

fn format(
    slots: &[Slot],
    arguments: &std::collections::BTreeMap<String, Value>,
) -> Result<Vec<Part>, FormatError> {
    for name in arguments.keys() {
        if !slots
            .iter()
            .any(|slot| matches!(slot, Slot::Argument { name: slot_name, .. } if slot_name == name))
        {
            return Err(FormatError::ExtraArgument);
        }
    }

    let mut output = Vec::new();
    for slot in slots {
        match slot {
            Slot::Literal(text) => output.push(Part::Literal(text.clone())),
            Slot::Argument { name, kind } => {
                let value = arguments.get(name).ok_or(FormatError::MissingArgument)?;
                if !value_matches(*kind, value) {
                    return Err(FormatError::WrongKind);
                }
                let part = match value {
                    Value::Text(text) => Part::Text(text.clone()),
                    Value::Quantity(quantity) => Part::Quantity(*quantity),
                    Value::Money(money) => Part::Money(*money),
                };
                output.push(part);
            }
        }
    }
    Ok(output)
}

fn main() {
    use std::collections::BTreeMap;

    let usd = Currency(*b"USD");
    let template = [
        Slot::Literal("Usage: ".to_owned()),
        Slot::Argument {
            name: "count".to_owned(),
            kind: Kind::Quantity(Unit::AudioMillisecond),
        },
        Slot::Literal("; note: ".to_owned()),
        Slot::Argument {
            name: "note".to_owned(),
            kind: Kind::Text,
        },
        Slot::Literal("; cost: ".to_owned()),
        Slot::Argument {
            name: "cost".to_owned(),
            kind: Kind::Money(usd),
        },
    ];
    let mut arguments = BTreeMap::new();
    arguments.insert(
        "count".to_owned(),
        Value::Quantity(Usage {
            quantity: 7,
            unit: Unit::AudioMillisecond,
        }),
    );
    arguments.insert(
        "note".to_owned(),
        Value::Text("<script>literal text</script>".to_owned()),
    );
    arguments.insert(
        "cost".to_owned(),
        Value::Money(Money {
            currency: usd,
            micros: 1,
        }),
    );

    let parts = format(&template, &arguments).unwrap();
    assert_eq!(
        parts,
        vec![
            Part::Literal("Usage: ".to_owned()),
            Part::Quantity(Usage {
                quantity: 7,
                unit: Unit::AudioMillisecond,
            }),
            Part::Literal("; note: ".to_owned()),
            Part::Text("<script>literal text</script>".to_owned()),
            Part::Literal("; cost: ".to_owned()),
            Part::Money(Money {
                currency: usd,
                micros: 1,
            }),
        ]
    );

    assert_eq!(
        format(&template, &BTreeMap::new()),
        Err(FormatError::MissingArgument)
    );

    let mut extra = arguments.clone();
    extra.insert("surprise".to_owned(), Value::Text("x".to_owned()));
    assert_eq!(format(&template, &extra), Err(FormatError::ExtraArgument));

    let mut wrong_kind = arguments;
    wrong_kind.insert(
        "count".to_owned(),
        Value::Quantity(Usage {
            quantity: 7,
            unit: Unit::VideoMillisecond,
        }),
    );
    assert_eq!(format(&template, &wrong_kind), Err(FormatError::WrongKind));
}
```

## Alternatives and remaining decisions

Positional arguments were rejected because translated word order must be free to change without changing argument meaning. Generic string values were rejected because callers could pass a quantity with the wrong unit or parse untrusted text as a number. Treating inserted text as markup was rejected because catalog values and runtime text have separate trust boundaries. Floating-point quantities, currency conversion, and localized display values as arithmetic inputs are excluded because they can lose exact values or units.

The concrete localized digit/grouping algorithm, locale-specific unit-label inventory, plural behavior, and rich-text sink encoding require their consumer/output-specific decisions. `df-types` currently supplies the exact `Usage` and `Money` values, but no production `df-locale::Catalog::format` exists yet. The following implementation should consume this contract inside `df-locale` and coordinate stable `TextKey`/catalog ownership with D01 and negotiation/fallback ownership with D02; this decision does not invent either sibling's details.

## Evidence and scope

The cited existing boundaries are `planning/subsystem-architecture.md` (`df-locale` owns catalogs/negotiation), `planning/subsystem-interfaces.md` (the `Catalog::format` contract and explicit missing-key diagnostics), `planning/client-presentation.md` (presentation remains separate from source rules), `crates/df-types/src/money.rs` (`Usage`, `Money`, exact units), and `planning/typed-units-policy.md` (preserve explicit closed units). The source state has typed locale and exact money/usage primitives, but not a catalog formatter or integrated caller.

The executable decision literal is evidence only for the bounded typed contract above. The required checks and their receipts are recorded in the attempt evidence handoff; no Cargo, native/WASM, catalog-loading, browser, or user-facing rendering check is implied by this decision.
