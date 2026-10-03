use df_content::catalog::{CatalogEntry, CatalogLimits, CatalogSnapshot};
use df_model::checkpoint::{
    CheckpointPins, ContentDigest, ContentPins, RuleReference, RulesMode, RulesPins,
};
use df_rules::{DispatchError, DispatchRegistry, HandlerRegistration, RegistryError};
use df_types::{BuildIdentity, RevisionLabel};

// Synthetic compiled values exercise selection, without claiming any D&D mechanic.
#[derive(Debug, Eq, PartialEq)]
enum FixtureHandler {
    First,
    Second,
}

fn label(text: &str) -> RevisionLabel {
    RevisionLabel::new(Some(text)).unwrap()
}

fn pins() -> CheckpointPins {
    CheckpointPins {
        rules: RulesPins {
            mode: RulesMode::Standard2024,
            ruleset: label("fixture-rules"),
            catalog: label("fixture-catalog"),
            catalog_digest: ContentDigest([1; 32]),
            source_manifest: label("fixture-sources"),
            source_manifest_digest: ContentDigest([2; 32]),
            handler: label("fixture-handler-revision"),
            handler_digest: ContentDigest([3; 32]),
        },
        content: ContentPins {
            content: label("fixture-content"),
            content_digest: ContentDigest([4; 32]),
            package: label("fixture-package"),
            package_digest: ContentDigest([5; 32]),
        },
        build: BuildIdentity::new(
            Some("fixture-source"),
            Some("fixture-native"),
            Some("fixture-wasm"),
            Some("fixture-config"),
            Some("fixture-content"),
        )
        .unwrap(),
    }
}

fn source() -> RuleReference {
    RuleReference {
        catalog: label("fixture-catalog"),
        source: label("fixture-source-document"),
        entry: label("fixture-entry"),
        clause: label("fixture-clause"),
    }
}

fn catalog<'a>(
    pins: &'a CheckpointPins,
    entries: &'a [CatalogEntry<'a, RuleReference>],
) -> CatalogSnapshot<'a, RevisionLabel, CheckpointPins, RuleReference> {
    CatalogSnapshot::from_published(
        &pins.rules.catalog,
        pins,
        b"synthetic-complete-publication",
        entries,
        CatalogLimits {
            max_complete_bytes: 64,
            max_entries: 4,
            max_item_bytes: 64,
            max_total_item_bytes: 128,
        },
    )
    .unwrap()
}

#[test]
fn exact_source_and_complete_pins_select_the_supplied_compiled_handler() {
    let pins = pins();
    let source = source();
    let selector = label("fixture-selector");
    let handler = FixtureHandler::First;
    let entries = [CatalogEntry::new(&source, b"opaque-source-parameters")];
    let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
    let registry =
        DispatchRegistry::from_catalog(catalog(&pins, &entries), &registrations, 1).unwrap();

    assert!(std::ptr::eq(
        registry.select(&pins, &selector, &source).unwrap(),
        &handler,
    ));
}

#[test]
fn unknown_handler_does_not_select_a_registered_fallback() {
    let pins = pins();
    let source = source();
    let selector = label("fixture-selector");
    let handler = FixtureHandler::First;
    let entries = [CatalogEntry::new(&source, b"source")];
    let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
    let registry =
        DispatchRegistry::from_catalog(catalog(&pins, &entries), &registrations, 1).unwrap();

    assert_eq!(
        registry.select(&pins, &label("unknown"), &source),
        Err(DispatchError::UnknownHandler),
    );
}

#[test]
fn known_handler_refuses_every_different_source_reference_component() {
    let pins = pins();
    let source = source();
    let selector = label("fixture-selector");
    let handler = FixtureHandler::First;
    let entries = [CatalogEntry::new(&source, b"source")];
    let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
    let registry =
        DispatchRegistry::from_catalog(catalog(&pins, &entries), &registrations, 1).unwrap();

    for component in 0..4 {
        let mut changed = source.clone();
        match component {
            0 => changed.catalog = label("other"),
            1 => changed.source = label("other"),
            2 => changed.entry = label("other"),
            _ => changed.clause = label("other"),
        }
        assert_eq!(
            registry.select(&pins, &selector, &changed),
            Err(DispatchError::UnsupportedSource),
        );
    }
}

#[test]
fn every_rules_content_and_build_pin_is_required_for_selection() {
    let pins = pins();
    let source = source();
    let selector = label("fixture-selector");
    let handler = FixtureHandler::First;
    let entries = [CatalogEntry::new(&source, b"source")];
    let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
    let registry =
        DispatchRegistry::from_catalog(catalog(&pins, &entries), &registrations, 1).unwrap();

    for component in 0..17 {
        let mut changed = pins.clone();
        match component {
            0 => changed.rules.mode = RulesMode::DisclosedCustom,
            1 => changed.rules.ruleset = label("other"),
            2 => changed.rules.catalog = label("other"),
            3 => changed.rules.catalog_digest = ContentDigest([9; 32]),
            4 => changed.rules.source_manifest = label("other"),
            5 => changed.rules.source_manifest_digest = ContentDigest([9; 32]),
            6 => changed.rules.handler = label("other"),
            7 => changed.rules.handler_digest = ContentDigest([9; 32]),
            8 => changed.content.content = label("other"),
            9 => changed.content.content_digest = ContentDigest([9; 32]),
            10 => changed.content.package = label("other"),
            11 => changed.content.package_digest = ContentDigest([9; 32]),
            _ => {
                let mut revisions = [
                    "fixture-source",
                    "fixture-native",
                    "fixture-wasm",
                    "fixture-config",
                    "fixture-content",
                ];
                revisions[component - 12] = "other";
                changed.build = BuildIdentity::new(
                    Some(revisions[0]),
                    Some(revisions[1]),
                    Some(revisions[2]),
                    Some(revisions[3]),
                    Some(revisions[4]),
                )
                .unwrap();
            }
        }
        assert_eq!(
            registry.select(&changed, &selector, &source),
            Err(DispatchError::PinsMismatch),
        );
    }
}

#[test]
fn conflicting_registration_is_refused_before_any_selection() {
    let pins = pins();
    let source = source();
    let selector = label("fixture-selector");
    let entries = [CatalogEntry::new(&source, b"source")];
    let registrations = [
        HandlerRegistration::new(&selector, &source, &FixtureHandler::First),
        HandlerRegistration::new(&selector, &source, &FixtureHandler::Second),
    ];

    assert!(matches!(
        DispatchRegistry::from_catalog(catalog(&pins, &entries), &registrations, 2),
        Err(RegistryError::DuplicateRegistration {
            first_index: 0,
            duplicate_index: 1,
        }),
    ));
}

#[test]
fn exact_duplicate_registration_is_also_refused() {
    let pins = pins();
    let source = source();
    let selector = label("fixture-selector");
    let handler = FixtureHandler::First;
    let entries = [CatalogEntry::new(&source, b"source")];
    let registrations = [
        HandlerRegistration::new(&selector, &source, &handler),
        HandlerRegistration::new(&selector, &source, &handler),
    ];

    assert!(matches!(
        DispatchRegistry::from_catalog(catalog(&pins, &entries), &registrations, 2),
        Err(RegistryError::DuplicateRegistration { .. }),
    ));
}

#[test]
fn registration_limit_is_enforced_and_zero_allows_only_an_empty_registry() {
    let pins = pins();
    let source = source();
    let selector = label("fixture-selector");
    let entries = [CatalogEntry::new(&source, b"source")];
    let registrations = [HandlerRegistration::new(
        &selector,
        &source,
        &FixtureHandler::First,
    )];

    assert!(matches!(
        DispatchRegistry::from_catalog(catalog(&pins, &entries), &registrations, 0),
        Err(RegistryError::RegistrationLimit {
            actual: 1,
            maximum: 0
        }),
    ));
    let empty: [HandlerRegistration<'_, FixtureHandler>; 0] = [];
    let registry = DispatchRegistry::from_catalog(catalog(&pins, &entries), &empty, 0).unwrap();
    assert_eq!(
        registry.select(&pins, &selector, &source),
        Err(DispatchError::UnknownHandler),
    );
}

#[test]
fn unindexed_source_is_refused_during_registry_admission() {
    let pins = pins();
    let source = source();
    let selector = label("fixture-selector");
    let registrations = [HandlerRegistration::new(
        &selector,
        &source,
        &FixtureHandler::First,
    )];
    let entries = [];

    assert!(matches!(
        DispatchRegistry::from_catalog(catalog(&pins, &entries), &registrations, 1),
        Err(RegistryError::MissingSource {
            registration_index: 0
        }),
    ));
}

#[test]
fn mismatched_source_catalog_is_refused_even_when_indexed() {
    let pins = pins();
    let mut source = source();
    source.catalog = label("other-catalog");
    let selector = label("fixture-selector");
    let entries = [CatalogEntry::new(&source, b"source")];
    let registrations = [HandlerRegistration::new(
        &selector,
        &source,
        &FixtureHandler::First,
    )];

    assert!(matches!(
        DispatchRegistry::from_catalog(catalog(&pins, &entries), &registrations, 1),
        Err(RegistryError::SourceCatalogMismatch {
            registration_index: 0
        }),
    ));
}

#[test]
fn publication_version_cannot_disagree_with_rules_catalog_pin() {
    let pins = pins();
    let version = label("other-catalog");
    let entries = [];
    let catalog = CatalogSnapshot::from_published(
        &version,
        &pins,
        b"synthetic",
        &entries,
        CatalogLimits {
            max_complete_bytes: 64,
            max_entries: 0,
            max_item_bytes: 0,
            max_total_item_bytes: 0,
        },
    )
    .unwrap();
    let registrations: [HandlerRegistration<'_, FixtureHandler>; 0] = [];

    assert!(matches!(
        DispatchRegistry::from_catalog(catalog, &registrations, 0),
        Err(RegistryError::CatalogVersionMismatch),
    ));
}

#[test]
fn selector_can_cover_multiple_admitted_clauses_without_clause_fallback() {
    let pins = pins();
    let first = source();
    let mut second = first.clone();
    second.clause = label("fixture-second-clause");
    let selector = label("fixture-selector");
    let entries = [
        CatalogEntry::new(&first, b"first"),
        CatalogEntry::new(&second, b"second"),
    ];
    let registrations = [
        HandlerRegistration::new(&selector, &first, &FixtureHandler::First),
        HandlerRegistration::new(&selector, &second, &FixtureHandler::Second),
    ];
    let registry =
        DispatchRegistry::from_catalog(catalog(&pins, &entries), &registrations, 2).unwrap();

    assert_eq!(
        registry.select(&pins, &selector, &first),
        Ok(&FixtureHandler::First)
    );
    assert_eq!(
        registry.select(&pins, &selector, &second),
        Ok(&FixtureHandler::Second)
    );
}

#[test]
fn indexed_clause_without_a_registration_remains_unsupported() {
    let pins = pins();
    let first = source();
    let mut second = first.clone();
    second.clause = label("fixture-unimplemented-clause");
    let selector = label("fixture-selector");
    let entries = [
        CatalogEntry::new(&first, b"first"),
        CatalogEntry::new(&second, b"second"),
    ];
    let registrations = [HandlerRegistration::new(
        &selector,
        &first,
        &FixtureHandler::First,
    )];
    let registry =
        DispatchRegistry::from_catalog(catalog(&pins, &entries), &registrations, 1).unwrap();

    assert_eq!(
        registry.select(&pins, &selector, &second),
        Err(DispatchError::UnsupportedSource),
    );
}

#[test]
fn two_selectors_for_one_source_select_their_own_handler() {
    let pins = pins();
    let source = source();
    let first_selector = label("first-selector");
    let second_selector = label("second-selector");
    let entries = [CatalogEntry::new(&source, b"source")];
    let registrations = [
        HandlerRegistration::new(&first_selector, &source, &FixtureHandler::First),
        HandlerRegistration::new(&second_selector, &source, &FixtureHandler::Second),
    ];
    let registry =
        DispatchRegistry::from_catalog(catalog(&pins, &entries), &registrations, 2).unwrap();

    assert_eq!(
        registry.select(&pins, &first_selector, &source),
        Ok(&FixtureHandler::First)
    );
    assert_eq!(
        registry.select(&pins, &second_selector, &source),
        Ok(&FixtureHandler::Second)
    );
}
