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

The caller supplies a `TextKey` and D02's resolved supported locale. For this
bounded decision example, it also supplies D02's already-admitted supported
default. `df-locale` forms a lookup chain from the selected locale followed by
that default, omitting a duplicate when they are equal. This is catalog-key
fallback after locale selection: it does not choose preferences, validate the
default, parse or normalize tags, or infer language-prefix matches. Those
remain with D02 and `df-types::LocaleTag`.

`df-locale` tries the catalogs in that chain for the same key and owns the
observable result. A successful lookup preserves the requested `TextKey`, the
chain searched, and the locale/catalog entry that supplied the text. If every
catalog in the chain lacks the key, return an explicit missing-key outcome
preserving the requested `TextKey` and full search chain. Do not silently emit
the key, an English string, or an unrelated default as if lookup succeeded. A
UI may render a diagnostic marker from that typed outcome; policy for that
marker is a presentation concern.

This is key lookup only. Catalog loading and atomic completeness, language
preference precedence, format argument validation, rich text, and mechanic
rule semantics remain outside D01.

## Bounded contract example

This standalone literal uses the actual `df-types::LocaleTag` source module and
small local catalog/lookup fixtures. It demonstrates same-key translations,
mechanical identity preservation, a default-catalog hit, an all-catalogs-missing
outcome, and malformed-key refusals. The fixture is evidence for this decision,
not a production `df-locale` API or catalog implementation:

```rust
#[path = "../../../crates/df-types/src/locale.rs"]
mod df_types_locale;

use df_types_locale::LocaleTag;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct TextKey(String);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TextKeyError {
    InvalidSyntax,
}

impl TextKey {
    fn parse(value: &str) -> Result<Self, TextKeyError> {
        let valid = !value.is_empty()
            && value.is_ascii()
            && value.split('.').all(|segment| {
                let mut bytes = segment.bytes();
                matches!(bytes.next(), Some(b'a'..=b'z'))
                    && bytes.all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'_'))
            });

        if valid {
            Ok(Self(value.to_owned()))
        } else {
            Err(TextKeyError::InvalidSyntax)
        }
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

fn lookup_chain(selected: &LocaleTag, admitted_default: &LocaleTag) -> Vec<LocaleTag> {
    let mut chain = vec![selected.clone()];
    if selected != admitted_default {
        chain.push(admitted_default.clone());
    }
    chain
}

#[derive(Default)]
struct Catalog {
    entries: BTreeMap<(String, TextKey), String>,
}

#[derive(Debug, Eq, PartialEq)]
enum Lookup<'a> {
    Found {
        key: TextKey,
        text: &'a str,
        source_locale: LocaleTag,
        searched_locales: Vec<LocaleTag>,
    },
    Missing {
        key: TextKey,
        searched_locales: Vec<LocaleTag>,
    },
}

impl Catalog {
    fn insert(&mut self, locale: &LocaleTag, key: TextKey, text: &str) {
        self.entries
            .insert((locale.as_str().to_owned(), key), text.to_owned());
    }

    fn lookup<'a>(&'a self, key: &TextKey, chain: &[LocaleTag]) -> Lookup<'a> {
        let mut searched_locales = Vec::new();
        for locale in chain {
            searched_locales.push(locale.clone());
            if let Some(text) = self.entries.get(&(locale.as_str().to_owned(), key.clone())) {
                return Lookup::Found {
                    key: key.clone(),
                    text: text.as_str(),
                    source_locale: locale.clone(),
                    searched_locales,
                };
            }
        }
        Lookup::Missing {
            key: key.clone(),
            searched_locales,
        }
    }
}

impl Lookup<'_> {
    fn key(&self) -> &TextKey {
        match self {
            Self::Found { key, .. } | Self::Missing { key, .. } => key,
        }
    }

    fn label(&self) -> Option<&str> {
        match self {
            Self::Found { text, .. } => Some(text),
            Self::Missing { .. } => None,
        }
    }

    fn source_locale(&self) -> Option<&LocaleTag> {
        match self {
            Self::Found { source_locale, .. } => Some(source_locale),
            Self::Missing { .. } => None,
        }
    }

    fn searched_locales(&self) -> &[LocaleTag] {
        match self {
            Self::Found {
                searched_locales, ..
            }
            | Self::Missing {
                searched_locales, ..
            } => searched_locales,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MechanicalIdentity(u16);

struct Presentation<'a> {
    mechanic: MechanicalIdentity,
    lookup: Lookup<'a>,
}

fn present(mechanic: MechanicalIdentity, lookup: Lookup<'_>) -> Presentation<'_> {
    Presentation { mechanic, lookup }
}

fn main() {
    let key = TextKey::parse("mechanics.action.attack").unwrap();
    let en = LocaleTag::parse("en").unwrap();
    let fr = LocaleTag::parse("fr").unwrap();
    let english_chain = lookup_chain(&en, &en);
    let french_chain = lookup_chain(&fr, &en);
    assert_eq!(english_chain, vec![en.clone()]);
    assert_eq!(french_chain, vec![fr.clone(), en.clone()]);

    let mut translated = Catalog::default();
    translated.insert(&en, key.clone(), "Attack");
    translated.insert(&fr, key.clone(), "Attaque");
    let mechanic = MechanicalIdentity(42);
    let english = present(mechanic, translated.lookup(&key, &english_chain));
    let french = present(mechanic, translated.lookup(&key, &french_chain));
    assert_eq!(english.mechanic, mechanic);
    assert_eq!(french.mechanic, mechanic);
    assert_eq!(english.lookup.key(), &key);
    assert_eq!(french.lookup.key(), &key);
    assert_eq!(english.lookup.key().as_str(), "mechanics.action.attack");
    assert_eq!(french.lookup.key().as_str(), "mechanics.action.attack");
    assert_eq!(english.lookup.label(), Some("Attack"));
    assert_eq!(french.lookup.label(), Some("Attaque"));
    assert_ne!(english.lookup.label(), french.lookup.label());

    let mut default_only = Catalog::default();
    default_only.insert(&en, key.clone(), "Attack");
    let fallback = present(mechanic, default_only.lookup(&key, &french_chain));
    assert_eq!(fallback.mechanic, mechanic);
    assert_eq!(fallback.lookup.key(), &key);
    assert_eq!(fallback.lookup.label(), Some("Attack"));
    assert_eq!(fallback.lookup.source_locale(), Some(&en));
    assert_eq!(fallback.lookup.searched_locales(), &french_chain);

    let empty = Catalog::default();
    let missing = present(mechanic, empty.lookup(&key, &french_chain));
    assert_eq!(missing.mechanic, mechanic);
    assert_eq!(missing.lookup.key(), &key);
    assert_eq!(missing.lookup.label(), None);
    assert_eq!(missing.lookup.source_locale(), None);
    assert_eq!(missing.lookup.searched_locales(), &french_chain);

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

For the same opaque mechanical identity, the fixture resolves one key to
different English/French labels without changing the identity or key. The
selected French catalog falls through to the admitted English default when
needed. Both that fallback hit and the all-catalogs-missing result retain the
original key and complete ordered search chain; the hit also names its source
locale. The fixture does not introduce production mechanic IDs or real catalog
content.

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
implementation. The D02 candidate at `4eda2a7` supplies a selected locale and
requires its configured default to be supported; this D01 example treats both
as already-resolved inputs and forms the deduplicated lookup chain without
reimplementing preference selection. D03's `Catalog::format(TextKey, args)`
candidate owns placeholder validation and formatted parts; composition of
format errors with lookup provenance remains open until those candidates are
reviewed. These are pending comparison inputs, not accepted contracts. D04
catalog provenance also remains separate. This document and its bounded
example do not establish integrated runtime behavior or complete the original
criterion by themselves.
