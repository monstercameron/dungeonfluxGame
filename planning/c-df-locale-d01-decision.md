# D01: Stable display text keys and missing-key ownership

Status: Planning decision; implementation pending

## Scope

`df-locale` owns display-text keys and lookup of those keys in a selected catalog
with an explicit locale fallback chain. A text key names presentation content;
it does not replace a rules, action, content, or other mechanic identity. The
source decision for a mechanic stays the same when its label is translated.

`df-types::LocaleTag` remains the canonical parsed locale value. This decision
does not add another locale type, change its syntax, or decide which audience
preference wins. Locale selection and the ordered supported-locale chain belong
to D02. Placeholder types and formatting belong to D03. Catalog revision and
translation provenance belong to D04.

## Decision

Use a `df-locale::TextKey` value as the stable identifier shared by catalog
entries and callers. The minimal key spelling is an ASCII, lowercase,
dot-separated identifier. Each non-empty segment starts with `a`–`z` and
continues with `a`–`z`, `0`–`9`, or `_`; for example,
`mechanics.action.attack`. Reject empty keys, empty segments, uppercase,
whitespace, and non-ASCII characters. Keep the key independent of locale and
display text. Renaming or translating a label does not rename its key; changing
the meaning or ownership of an identifier requires an explicit source change.

The caller supplies a `TextKey` and the ordered locale fallback chain selected
by locale negotiation. `df-locale` owns trying the catalogs in that order for
the same key and owns the observable result. A successful lookup identifies
the locale/catalog entry that supplied the text. If every catalog in the chain
lacks the key, return an explicit missing-key outcome that preserves the
requested `TextKey` and the catalogs/chain considered. Do not silently emit the
key, an English string, or an unrelated default as if lookup succeeded. A UI
may render a diagnostic marker from that typed outcome; policy for that marker
is a presentation concern.

This is key lookup only. Catalog loading and atomic completeness, language
preference precedence, format argument validation, rich text, and mechanic
rule semantics remain outside D01.

## Bounded contract example

This standalone literal exercises only the proposed `TextKey` spelling rule.
It does not pretend that the not-yet-implemented `df-locale` catalog API exists:

```rust
#[derive(Debug, Eq, PartialEq)]
struct TextKey(String);

impl TextKey {
    fn parse(value: &str) -> Result<Self, ()> {
        let valid = !value.is_empty()
            && value.is_ascii()
            && value.split('.').all(|segment| {
                let mut bytes = segment.bytes();
                matches!(bytes.next(), Some(b'a'..=b'z'))
                    && bytes.all(|byte| {
                        matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'_')
                    })
            });

        if valid {
            Ok(Self(value.to_owned()))
        } else {
            Err(())
        }
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

fn main() {
    let key = TextKey::parse("mechanics.action.attack").unwrap();
    assert_eq!(key.as_str(), "mechanics.action.attack");

    for malformed in [
        "",
        ".attack",
        "mechanics..attack",
        "mechanics.attack.",
        "Mechanics.attack",
        "mechanics.ünknown",
        "mechanics.action attack",
    ] {
        assert!(TextKey::parse(malformed).is_err(), "{malformed:?}");
    }
}
```

For any locale chain, every lookup uses the same key. Translating the catalog
value changes the returned text, not the key; missing all entries produces a
typed missing outcome with the original key instead of a fabricated success.

## Alternatives and unresolved facts

- Using translated strings or array positions as identifiers was rejected:
  either can change across language/catalog edits and therefore cannot serve as
  the stable cross-language reference.
- Embedding a locale in each key was rejected because locale is a separate
  lookup input and would duplicate D02 negotiation in mechanic/presentation
  callers.
- Falling directly to a hard-coded default string was rejected because it
  hides catalog defects and fails to identify the missing entry.
- Exact production names, storage representation, catalog serialization,
  catalog revision type, and user-facing diagnostic rendering are unresolved
  implementation choices. D01 does not claim that catalogs or runtime lookup
  exist.

## Evidence and next consumer

The governing architecture assigns `Catalog` and `TextKey` to `df-locale`
(`planning/subsystem-architecture.md`, Design table); the interface plan assigns
`Catalog::format(key, args)` and requires explicit fallback/catalog revision
and missing-key diagnostics without rule translation
(`planning/subsystem-interfaces.md`, Browser interfaces). The shared `df-types`
source already provides `LocaleTag::parse` and `LocaleTag::as_str`; its module
comment explicitly reserves support, negotiation, and fallback to the locale
subsystem (`crates/df-types/src/locale.rs`).

The next consumer is the actual `df-locale` checked catalog lookup/negotiation
implementation. It must reconcile this decision with D02's preference/fallback
selection, D03's formatting contract, and D04's catalog provenance before
freezing concrete shared types. The later implementation must test a valid key,
refusal of malformed keys, same-key lookup across locales, fallback hit, and
explicit all-catalogs-missing behavior against its built source. This document
and its bounded example do not establish runtime behavior or complete the
original acceptance criterion by themselves.
