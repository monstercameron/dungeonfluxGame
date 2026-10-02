# Player, campaign, display, and speech locale precedence

Task: `B-C-df-locale-D02`, attempt `B-C-df-locale-D02-a1`  
Decision owner: `df-locale`  
Source revision: `4c0e050c2d30e632bd3b32c51a92cfda52479e6e`

## Decision

Locale is a presentation choice made for an identified audience and output. It
does not select, translate, or revise campaign rules. Rule and choice identity,
numeric values, validation, and outcomes continue to come from the pinned source
rules/content revisions. Changing a preference cannot change an accepted offer,
pending action, or committed result.

Resolve preferences separately for each destination:

| Output destination | Requested locale precedence | Fallback |
| --- | --- | --- |
| A player's private UI, captions, or narrative text | That player's explicit preference, then campaign presentation locale | Configured supported default |
| Shared-display text and captions | That display's explicit preference, then campaign presentation locale | Configured supported default |
| Speech routed to one player | That player's explicit speech preference, then that player's locale preference, then campaign presentation locale | Configured supported default |
| Speech routed to a shared display or audience | That route's explicit speech preference, then the route's display preference, then campaign presentation locale | Configured supported default |

The route/audience is determined by the owning API or media boundary. A private
player preference cannot change shared-display text; a display preference cannot
change a player's private text. When one utterance is delivered to destinations
with different resolved locales, each destination resolves independently. A
single delivery must use one resolved locale for its text and speech metadata.

`df-locale::settle(requested, supported, default)` considers requested tags in
the order above and selects the first tag that is an exact member of the
catalog-supported set. It does not infer support from a language prefix, likely
script, registry alias, device locale, or provider capability. `default` must
also be a member of `supported`; an invalid configuration has no selected locale
and must be surfaced as a configuration error. An unsupported request therefore
resolves to the campaign locale only when that exact tag is supported, otherwise
to the supported default. The outcome records the selected locale and whether
fallback occurred so the UI can disclose it.

`df-types::LocaleTag` is the existing syntactic value for all these tags. Its
parser normalizes ASCII case only and intentionally does not establish registry
membership or support. This decision adds no alias matching or broader
canonicalization. `supported` means locales with a usable catalog for the
relevant output, not locales presumed available from a speech provider. Speech
provider language capability remains a separate admission/result fact owned by
the provider/media boundary. A provider must not be assumed to support a
selected language; any actual mismatch or fallback must remain explicit, and
captions must describe the speech actually delivered. This document does not
qualify audible output or select a speech provider.

Campaign presentation locale identifies the authored presentation fallback;
it is not an instruction to translate source mechanics. Text keys, rule IDs,
choice IDs, source references, typed arguments, and authoritative values remain
stable across locales. Missing translations remain explicit catalog misses,
not grounds to substitute a different rule or to claim that the text is
translated.

## Bounded contract example

The selector consumes parsed `LocaleTag` values. The implementation must validate
that `default` belongs to `supported` before using it. Exact membership makes an
unsupported preference visibly settle only on a configured catalog locale.

```rust
fn settle(
    requested: &[LocaleTag],
    supported: &[LocaleTag],
    default: &LocaleTag,
) -> Option<LocaleTag> {
    if !supported.contains(default) {
        return None;
    }

    requested
        .iter()
        .find(|candidate| supported.contains(candidate))
        .or_else(|| supported.iter().find(|candidate| *candidate == default))
        .cloned()
}

fn main() {
    let en = LocaleTag::parse("en").unwrap();
    let fr = LocaleTag::parse("fr").unwrap();
    let de = LocaleTag::parse("de").unwrap();
    let supported = vec![en.clone(), fr.clone()];

    assert_eq!(fr.as_str(), "fr");
    assert_eq!(settle(&[de.clone()], &supported, &en), Some(en.clone()));
    assert_eq!(settle(&[fr.clone(), en.clone()], &supported, &en), Some(fr));
    assert_eq!(settle(&[], &supported, &de), None);
}
```

The example's `LocaleTag` name is the existing `df-types::LocaleTag`; its
standalone check supplies that source module directly. `None` represents invalid
default configuration. A production implementation may use a typed result and
record whether the selected tag was a fallback.

## Alternatives and unresolved facts

- **Use a device/browser language implicitly.** Rejected because it bypasses
  explicit player/display preferences and makes behavior vary with ambient
  client state. A client may propose a preference through its authorized
  boundary; that does not grant authority or alter rules.
- **Use campaign locale for every recipient.** Rejected because player, display,
  and speech destinations can have distinct needs. The campaign locale remains
  the common fallback.
- **Use best-effort language-prefix, alias, or script matching.** Rejected for
  this contract because `LocaleTag` does not canonicalize tags and inferred
  matches can select a catalog that was never declared. Any future matching
  policy requires an explicit supported-set contract and fixtures.
- **Assume speech providers support the catalog-selected language.** Rejected;
  actual provider availability is not established here. Provider capability and
  delivered speech language must be reported by the owning boundary.

Still unresolved: the concrete campaign preference field and its provenance;
the concrete authenticated request/configuration fields for player and display
preferences; catalog inventory and supported default for each deployment;
provider-specific speech capability and fallback behavior; and wire-level
disclosure shape. Those are prerequisites or follow-up decisions for their
owning implementation boundaries. D01 owns stable text keys/catalog fallback
ownership; D03 owns formatting contracts. No mechanics translation is authorized.

## Scope and verification

The next consumer is `df-locale` locale negotiation, with consumers supplying
the per-output ordered requests and output-specific supported catalog set. The
permitted implementation boundary remains `df-locale`; shared `LocaleTag`
already exists in `df-types`. This decision does not modify source rules,
catalog contents, provider adapters, client UI, audio routing, or persistence.

The exact Rust code fence is extracted byte-for-byte and compared with the
retained submitted source. The checked-in `rustfmt.toml` and pinned Rust 1.98.1
tools apply to that literal. Finite cases cover preference order, unsupported
preference fallback, and refusal when the configured default is unsupported.
These checks establish only the decision example. No Cargo workspace, native or
WASM production build, catalog integration, browser behavior, provider
availability, or audible output is verified by this planning task.
