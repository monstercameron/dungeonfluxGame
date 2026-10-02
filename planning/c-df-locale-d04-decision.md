# Translation catalog revision provenance

Task/attempt: `B-C-df-locale-D04-a1`
Input revision: `d31d2828402ee930865f3cb4c44a234afc96954c`
Status: bounded `df-locale` contract decision. This is not production catalog
loading or integrated locale acceptance.

## Decision

A usable translation catalog is an immutable, versioned set of the locale
catalogs available to a given output. It has one text version represented by
the existing `df_types::RevisionLabel`, supplied by the content/catalog source
when that set is loaded. Every locale entry in that set belongs to that same
version. The version is not generated per lookup, inferred from a locale, or
substituted with the executable build version. `RevisionLabel` validates only
the bounded nonsecret spelling; it does not prove who supplied a catalog, its
contents, completeness, approval, or rights.

Loading or replacing a catalog set is atomic. A load either admits a complete
immutable set with its text version or returns a typed failure and leaves the
previous usable set in place. A lookup never combines entries from different
text versions. The loader and its authority still have to bind the supplied
version to the immutable source artifact; this decision does not invent a
catalog identifier, source registry, catalog contents, or provider locale
support.

Each successful localized label carries the text version of the catalog set
that supplied it, together with the existing stable `TextKey` and D01's
`source_locale`. D03's formatted message preserves the same version from the
catalog entry used to format it. Locale fallback may change `source_locale`
within the set, but does not change the set's text version. A missing-key,
invalid-argument, or invalid-catalog result emits no localized label and
therefore has no text version to claim; its typed failure and D01 diagnostic
identity remain visible. Do not put a key, English default, or fabricated
label in that result as if it were translated.

The text version identifies catalog text, not mechanics. Rules, choice IDs,
source rules/content revisions, numeric values, and committed outcomes remain
independent of locale and catalog updates. Output/session provenance that also
needs the build or authored content revision continues to use the existing
`BuildIdentity`; the catalog text version is a separate value and does not
overwrite that provenance.

## Bounded contract example

This exact Rust literal checks the admitted `LocaleTag` and `RevisionLabel`
types. It exercises propagation of a valid version onto an emitted label and
refusal of absent or malformed version metadata. It uses no catalog fixture,
translation text, formatter, or claim of supported locales; catalog loading
and lookup remain the next implementation boundary.

```rust
use df_types::{LocaleTag, RevisionLabel, RevisionLabelError};

#[derive(Debug, Eq, PartialEq)]
struct CatalogSet {
    text_version: RevisionLabel,
}

fn load_catalog_set(text_version: Option<&str>) -> Result<CatalogSet, RevisionLabelError> {
    Ok(CatalogSet {
        text_version: RevisionLabel::new(text_version)?,
    })
}

#[derive(Debug, Eq, PartialEq)]
struct EmittedLabel<'a> {
    text: &'a str,
    source_locale: LocaleTag,
    text_version: RevisionLabel,
}

fn emit_label<'a>(
    text: &'a str,
    source_locale: LocaleTag,
    catalog: &CatalogSet,
) -> EmittedLabel<'a> {
    EmittedLabel {
        text,
        source_locale,
        text_version: catalog.text_version.clone(),
    }
}

fn main() {
    let locale = LocaleTag::parse("fr").unwrap();
    let catalog = load_catalog_set(Some("text-catalog:42")).unwrap();
    let label = emit_label(
        "label supplied by a successful catalog lookup",
        locale.clone(),
        &catalog,
    );
    assert_eq!(label.text, "label supplied by a successful catalog lookup");
    assert_eq!(label.source_locale, locale);
    assert_eq!(label.text_version, catalog.text_version);

    assert_eq!(load_catalog_set(None), Err(RevisionLabelError::Missing));
    assert_eq!(
        load_catalog_set(Some("text catalog 42")),
        Err(RevisionLabelError::InvalidCharacter { byte_index: 4 })
    );
}
```

The small `CatalogSet` carries only the admitted revision type; it has no
entries and does not model or assert that any translation exists. The label
string is only a stand-in for the output of a successful lookup. Production
loading must reject absent/invalid revision metadata before publishing a
catalog set, then every successful lookup/format result takes its version from
that set.

## Alternatives and remaining facts

- **Version each emitted label independently.** Rejected because labels from a
  single loaded catalog set could then appear to have unrelated revisions, and
  per-request version creation would not identify immutable catalog data.
- **Version each locale separately inside one set.** Rejected for this
  contract because D01 fallback could combine different revisions in one
  lookup chain. Locale catalogs are admitted and replaced as one set under one
  version.
- **Reuse the executable or campaign content version as the text version.**
  Rejected because text can be revised independently and that substitution
  would hide which catalog text supplied a label. Existing build/content
  provenance remains separately available through `BuildIdentity`.
- **Treat a valid `RevisionLabel` as proof of catalog provenance.** Rejected;
  the type checks syntax only. The owning immutable content/catalog loader must
  bind it to its source artifact and enforce atomic completeness.

The identifier format and issuing authority for catalog source revisions,
retention and rollback policy, deployment support inventory, cross-build
compatibility, and wire disclosure of `text_version` remain with the catalog
loader and its consumers. No production catalog, supported-locale inventory,
provider capability, or revision authority exists in this decision.

## Next consumer and scope

The exact next implementation consumer is `B-C-df-locale-I04`, “Implement
catalog version loading,” owned by `df-locale`. It must bind a source-supplied
`RevisionLabel` to one complete immutable catalog set, reject invalid or mixed
revisions atomically, and retain the previous usable set on failure. `I02`
lookup must attach that set's version to a successful D01 result, and `I03`
formatting must preserve it on D03's formatted message. Those implementation
consumers remain pending; this decision adds no source code.

The cited boundaries are `planning/subsystem-architecture.md` (`df-locale`
owns catalogs/locale negotiation; `df-types` owns revisions/provenance),
`planning/subsystem-interfaces.md` (explicit catalog revision and missing-key
diagnostics), `planning/c-df-locale-d01-decision.md` (stable key, source locale,
and missing outcome), `planning/c-df-locale-d02-decision.md` (catalog-supported
fallback), and `planning/c-df-locale-d03-decision.md` (formatted semantic
parts). The existing admitted values are `df_types::LocaleTag` and
`df_types::RevisionLabel`.

The bounded verification checks the exact literal against this file, formats
it with the repository `rustfmt.toml`, compiles with the pinned standalone
Rust toolchain, and runs the valid/refusal cases under the frozen v6 guard.
Those checks establish only this contract example. No Cargo build, native or
WASM production implementation, catalog loading, browser behavior, provider
availability, or audible output is established here.
