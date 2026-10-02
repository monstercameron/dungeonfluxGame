use df_locale::{
    ArgumentKind, ArgumentValue, CatalogEntry, CatalogLoadError, FormattedMessage, FormattedPart,
    Lookup, MessageError, MessagePart, TextKey, VersionedCatalog,
};
use df_types::{Currency, LocaleTag, Money, Usage, UsageUnit};
use std::collections::BTreeMap;

struct Translation {
    locale: LocaleTag,
    key: TextKey,
    text: String,
    parts: Vec<MessagePart>,
}

struct Fixture {
    locales: Vec<LocaleTag>,
    declarations: BTreeMap<TextKey, BTreeMap<String, ArgumentKind>>,
    translations: Vec<Translation>,
}

impl Fixture {
    fn entries(&self) -> Vec<CatalogEntry<'_>> {
        self.translations
            .iter()
            .map(|entry| CatalogEntry {
                locale: &entry.locale,
                key: &entry.key,
                text: &entry.text,
                parts: &entry.parts,
            })
            .collect()
    }

    fn load(&self, version: &str) -> VersionedCatalog<String> {
        VersionedCatalog::load(
            version.to_owned(),
            &self.locales,
            &self.declarations,
            &self.entries(),
        )
        .unwrap()
    }
}

fn key(value: &str) -> TextKey {
    TextKey::parse(value).unwrap()
}

fn locale(value: &str) -> LocaleTag {
    LocaleTag::parse(value).unwrap()
}

fn name_part() -> MessagePart {
    MessagePart::Argument {
        name: "name".to_owned(),
        kind: ArgumentKind::Text,
    }
}

fn fixture() -> Fixture {
    let greeting = key("ui.greeting");
    let empty = key("ui.empty");
    let locales = vec![locale("en"), locale("fr")];
    let declarations = BTreeMap::from([
        (
            greeting.clone(),
            BTreeMap::from([("name".to_owned(), ArgumentKind::Text)]),
        ),
        (empty.clone(), BTreeMap::new()),
    ]);
    let mut translations = Vec::new();
    for (language, text) in [("en", "Hello"), ("fr", "Bonjour")] {
        translations.push(Translation {
            locale: locale(language),
            key: greeting.clone(),
            text: text.to_owned(),
            parts: vec![MessagePart::Literal(text.to_owned()), name_part()],
        });
        translations.push(Translation {
            locale: locale(language),
            key: empty.clone(),
            text: String::new(),
            parts: vec![],
        });
    }
    Fixture {
        locales,
        declarations,
        translations,
    }
}

fn arguments() -> BTreeMap<String, ArgumentValue> {
    BTreeMap::from([(
        "name".to_owned(),
        ArgumentValue::Text("<script>{command}</script>".to_owned()),
    )])
}

fn snapshot(
    catalog: &VersionedCatalog<String>,
    fixture: &Fixture,
) -> (String, Vec<String>, Vec<FormattedMessage>) {
    let mut labels = Vec::new();
    let mut messages = Vec::new();
    for language in &fixture.locales {
        for text_key in fixture.declarations.keys() {
            let chain = [language.clone()];
            let found = catalog.lookup(text_key, &chain);
            assert_eq!(found.version, catalog.version());
            labels.push(found.lookup.label().unwrap().to_owned());
            let args = if text_key == &key("ui.greeting") {
                arguments()
            } else {
                BTreeMap::new()
            };
            let formatted = catalog.format(text_key, &chain, &args).unwrap();
            assert_eq!(formatted.version, catalog.version());
            messages.push(formatted.message);
        }
    }
    (catalog.version().clone(), labels, messages)
}

#[test]
fn complete_catalog_preserves_empty_entries_plain_text_and_revision() {
    let fixture = fixture();
    let catalog = fixture.load("revision-one");
    assert_eq!(catalog.version(), "revision-one");
    let formatted = catalog
        .format(&key("ui.greeting"), &[locale("fr")], &arguments())
        .unwrap();
    assert_eq!(formatted.version, "revision-one");
    assert_eq!(formatted.message.source_locale, locale("fr"));
    assert_eq!(
        formatted.message.parts,
        vec![
            FormattedPart::Literal("Bonjour".to_owned()),
            FormattedPart::Text("<script>{command}</script>".to_owned()),
        ]
    );
    let empty = catalog.lookup(&key("ui.empty"), &[locale("fr")]);
    assert_eq!(empty.lookup.label(), Some(""));
    assert!(
        catalog
            .format(&key("ui.empty"), &[locale("fr")], &BTreeMap::new())
            .unwrap()
            .message
            .parts
            .is_empty()
    );
}

#[test]
fn every_missing_pair_refuses_reload_without_mutating_prior_snapshot() {
    let fixture = fixture();
    let mut catalog = fixture.load("revision-one");
    let before = snapshot(&catalog, &fixture);
    for omitted in 0..fixture.translations.len() {
        let incomplete: Vec<_> = fixture
            .entries()
            .into_iter()
            .enumerate()
            .filter_map(|(index, entry)| (index != omitted).then_some(entry))
            .collect();
        let missing = &fixture.translations[omitted];
        assert_eq!(
            catalog.reload(
                "revision-two".to_owned(),
                &fixture.locales,
                &fixture.declarations,
                &incomplete
            ),
            Err(CatalogLoadError::MissingEntry {
                locale: missing.locale.clone(),
                key: missing.key.clone()
            })
        );
        assert_eq!(snapshot(&catalog, &fixture), before);
    }
    let failed_initial = VersionedCatalog::load(
        "never-published",
        &fixture.locales,
        &fixture.declarations,
        &[],
    );
    assert!(matches!(
        failed_initial,
        Err(CatalogLoadError::MissingEntry { .. })
    ));
}

#[test]
fn inventory_refusals_preserve_the_active_catalog() {
    let fixture = fixture();
    let mut catalog = fixture.load("revision-one");
    let before = snapshot(&catalog, &fixture);
    let entries = fixture.entries();
    for (locales, declarations) in [
        (&[][..], &fixture.declarations),
        (fixture.locales.as_slice(), &BTreeMap::new()),
    ] {
        assert_eq!(
            catalog.reload("rejected".to_owned(), locales, declarations, &entries),
            Err(CatalogLoadError::EmptyManifest)
        );
        assert_eq!(snapshot(&catalog, &fixture), before);
    }
    let duplicate_locales = [locale("en"), locale("en")];
    assert_eq!(
        catalog.reload(
            "rejected".to_owned(),
            &duplicate_locales,
            &fixture.declarations,
            &entries
        ),
        Err(CatalogLoadError::DuplicateLocale {
            locale: locale("en")
        })
    );
    let mut duplicate = fixture.entries();
    duplicate.push(CatalogEntry {
        locale: &fixture.translations[0].locale,
        key: &fixture.translations[0].key,
        text: "replacement",
        parts: &fixture.translations[0].parts,
    });
    assert_eq!(
        catalog.reload(
            "rejected".to_owned(),
            &fixture.locales,
            &fixture.declarations,
            &duplicate
        ),
        Err(CatalogLoadError::DuplicateEntry {
            locale: locale("en"),
            key: key("ui.greeting")
        })
    );
    let unsupported = locale("en-US");
    let unexpected = [CatalogEntry {
        locale: &unsupported,
        key: &fixture.translations[0].key,
        text: "extra",
        parts: &[],
    }];
    assert_eq!(
        catalog.reload(
            "rejected".to_owned(),
            &fixture.locales,
            &fixture.declarations,
            &unexpected
        ),
        Err(CatalogLoadError::UnexpectedLocale {
            locale: unsupported.clone()
        })
    );
    let extra_key = key("ui.extra");
    let unexpected = [CatalogEntry {
        locale: &fixture.locales[0],
        key: &extra_key,
        text: "extra",
        parts: &[],
    }];
    assert_eq!(
        catalog.reload(
            "rejected".to_owned(),
            &fixture.locales,
            &fixture.declarations,
            &unexpected
        ),
        Err(CatalogLoadError::UnexpectedKey { key: extra_key })
    );
    assert_eq!(snapshot(&catalog, &fixture), before);
}

#[test]
fn invalid_declaration_and_translation_slots_refuse_atomically() {
    let fixture = fixture();
    let mut catalog = fixture.load("revision-one");
    let before = snapshot(&catalog, &fixture);
    let mut invalid = fixture.declarations.clone();
    invalid.insert(
        key("ui.greeting"),
        BTreeMap::from([("Bad.Name".to_owned(), ArgumentKind::Text)]),
    );
    assert_eq!(
        catalog.reload(
            "rejected".to_owned(),
            &fixture.locales,
            &invalid,
            &fixture.entries()
        ),
        Err(CatalogLoadError::InvalidDeclaration {
            key: key("ui.greeting"),
            error: MessageError::InvalidArgumentName
        })
    );
    for invalid_parts in [
        vec![],
        vec![
            name_part(),
            MessagePart::Argument {
                name: "extra".to_owned(),
                kind: ArgumentKind::Text,
            },
        ],
        vec![MessagePart::Argument {
            name: "name".to_owned(),
            kind: ArgumentKind::Quantity(UsageUnit::AudioMillisecond),
        }],
        vec![
            name_part(),
            MessagePart::Argument {
                name: "name".to_owned(),
                kind: ArgumentKind::Quantity(UsageUnit::AudioMillisecond),
            },
        ],
    ] {
        let mut entries = fixture.entries();
        entries[2].parts = &invalid_parts;
        assert_eq!(
            catalog.reload(
                "rejected".to_owned(),
                &fixture.locales,
                &fixture.declarations,
                &entries
            ),
            Err(CatalogLoadError::InvalidMessage {
                locale: locale("fr"),
                key: key("ui.greeting"),
                error: MessageError::SlotMismatch
            })
        );
        assert_eq!(snapshot(&catalog, &fixture), before);
    }
}

#[test]
fn valid_reload_replaces_inventory_and_declarations_together() {
    let fixture = fixture();
    let mut catalog = fixture.load("revision-one");
    let new_locale = locale("de");
    let greeting = key("ui.greeting");
    let quantity_kind = ArgumentKind::Quantity(UsageUnit::AudioMillisecond);
    let declarations = BTreeMap::from([(
        greeting.clone(),
        BTreeMap::from([("name".to_owned(), quantity_kind)]),
    )]);
    let parts = [MessagePart::Argument {
        name: "name".to_owned(),
        kind: quantity_kind,
    }];
    let entries = [CatalogEntry {
        locale: &new_locale,
        key: &greeting,
        text: "Neu",
        parts: &parts,
    }];
    catalog
        .reload(
            "revision-two".to_owned(),
            std::slice::from_ref(&new_locale),
            &declarations,
            &entries,
        )
        .unwrap();
    assert_eq!(catalog.version(), "revision-two");
    assert!(matches!(
        catalog.lookup(&greeting, &[locale("fr")]).lookup,
        Lookup::Missing { .. }
    ));
    assert!(matches!(
        catalog
            .lookup(&key("ui.empty"), std::slice::from_ref(&new_locale))
            .lookup,
        Lookup::Missing { .. }
    ));
    let value = Usage::new(u128::MAX, UsageUnit::AudioMillisecond);
    let args = BTreeMap::from([("name".to_owned(), ArgumentValue::Quantity(value))]);
    let formatted = catalog.format(&greeting, &[new_locale], &args).unwrap();
    assert_eq!(formatted.version, "revision-two");
    assert_eq!(
        formatted.message.parts,
        vec![FormattedPart::Quantity(value)]
    );
}

#[test]
fn reordered_repeated_slots_and_exact_money_are_admitted() {
    let currency = Currency::parse("USD").unwrap();
    let text_key = key("ui.amount");
    let language = locale("fr");
    let declarations = BTreeMap::from([(
        text_key.clone(),
        BTreeMap::from([
            ("name".to_owned(), ArgumentKind::Text),
            ("amount".to_owned(), ArgumentKind::Money(currency)),
        ]),
    )]);
    let parts = [
        MessagePart::Argument {
            name: "amount".to_owned(),
            kind: ArgumentKind::Money(currency),
        },
        name_part(),
        name_part(),
    ];
    let entries = [CatalogEntry {
        locale: &language,
        key: &text_key,
        text: "Montant",
        parts: &parts,
    }];
    let catalog = VersionedCatalog::load(
        42_u64,
        std::slice::from_ref(&language),
        &declarations,
        &entries,
    )
    .unwrap();
    for micros in [0, 1, u128::MAX] {
        let amount = Money::new(currency, micros);
        let args = BTreeMap::from([
            (
                "name".to_owned(),
                ArgumentValue::Text("{not_a_template}".to_owned()),
            ),
            ("amount".to_owned(), ArgumentValue::Money(amount)),
        ]);
        let result = catalog
            .format(&text_key, std::slice::from_ref(&language), &args)
            .unwrap();
        assert_eq!(result.version, &42);
        assert_eq!(
            result.message.parts,
            vec![
                FormattedPart::Money(amount),
                FormattedPart::Text("{not_a_template}".to_owned()),
                FormattedPart::Text("{not_a_template}".to_owned())
            ]
        );
    }
}

#[test]
fn missing_and_fallback_lookup_report_the_exact_snapshot_revision() {
    let fixture = fixture();
    let catalog = fixture.load("revision-one");
    let chain = [locale("de"), locale("fr"), locale("en")];
    let found = catalog.lookup(&key("ui.greeting"), &chain);
    assert_eq!(found.version, "revision-one");
    assert_eq!(found.lookup.source_locale(), Some(&locale("fr")));
    assert_eq!(found.lookup.searched_locales(), &chain[..2]);
    let missing = catalog.lookup(&key("ui.missing"), &chain);
    assert_eq!(missing.version, "revision-one");
    assert_eq!(missing.lookup.key(), &key("ui.missing"));
    assert_eq!(missing.lookup.searched_locales(), &chain);
    assert_eq!(missing.lookup.label(), None);
}
