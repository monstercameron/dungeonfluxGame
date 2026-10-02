use df_locale::{
    ArgumentKind, ArgumentValue, Catalog, FormatError, FormattedPart, MessageError, MessagePart,
    TextKey, lookup_chain, settle,
};
use df_types::{Currency, LocaleTag, Money, Usage, UsageUnit};
use std::collections::BTreeMap;

fn argument(name: &str, kind: ArgumentKind) -> MessagePart {
    MessagePart::Argument {
        name: name.to_owned(),
        kind,
    }
}

fn fixture() -> (
    Catalog,
    TextKey,
    Vec<LocaleTag>,
    BTreeMap<String, ArgumentValue>,
) {
    let key = TextKey::parse("display.usage.summary").unwrap();
    let en = LocaleTag::parse("en").unwrap();
    let fr = LocaleTag::parse("fr").unwrap();
    let usd = Currency::parse("USD").unwrap();
    let mut catalog = Catalog::default();
    catalog
        .declare_slots(
            key.clone(),
            &BTreeMap::from([
                ("note".to_owned(), ArgumentKind::Text),
                (
                    "count".to_owned(),
                    ArgumentKind::Quantity(UsageUnit::AudioMillisecond),
                ),
                ("cost".to_owned(), ArgumentKind::Money(usd)),
            ]),
        )
        .unwrap();
    catalog.insert(&en, key.clone(), "Usage summary");
    catalog.insert(&fr, key.clone(), "Résumé");
    catalog
        .insert_message(
            &en,
            &key,
            &[
                MessagePart::Literal("Usage: {literal}; ".to_owned()),
                argument("count", ArgumentKind::Quantity(UsageUnit::AudioMillisecond)),
                argument("note", ArgumentKind::Text),
                argument("cost", ArgumentKind::Money(usd)),
            ],
        )
        .unwrap();
    catalog
        .insert_message(
            &fr,
            &key,
            &[
                argument("cost", ArgumentKind::Money(usd)),
                argument("note", ArgumentKind::Text),
                argument("count", ArgumentKind::Quantity(UsageUnit::AudioMillisecond)),
                argument("note", ArgumentKind::Text),
            ],
        )
        .unwrap();
    let args = BTreeMap::from([
        (
            "count".to_owned(),
            ArgumentValue::Quantity(Usage::new(7, UsageUnit::AudioMillisecond)),
        ),
        ("note".to_owned(), ArgumentValue::Text("a note".to_owned())),
        ("cost".to_owned(), ArgumentValue::Money(Money::new(usd, 1))),
    ]);
    (catalog, key, vec![fr, en], args)
}

#[test]
fn negotiated_lookup_formats_reordered_repeated_slots_without_interpreting_text() {
    let (catalog, key, supported, mut args) = fixture();
    let default = &supported[1];
    let selection = settle(&supported[..1], &supported, default).unwrap();
    let chain = lookup_chain(selection.selected(), default);
    let source_outcome = ("source-revision-7", "action.attack");
    for text in [
        "<script>alert('x')</script>",
        "<a href=\"javascript:attack()\">click</a>",
        "$(rm -rf /) ; /attack @all",
        "{cost} {{note}} display.usage.summary",
        "攻撃 🐉 & <b>literal</b>\0\n",
    ] {
        args.insert("note".to_owned(), ArgumentValue::Text(text.to_owned()));
        let before = args.clone();
        let lookup = catalog.lookup(&key, &chain);
        let result = (source_outcome, catalog.format(&key, &chain, &args).unwrap());
        assert_eq!(result.0, source_outcome);
        assert_eq!(result.1.key, key);
        assert_eq!(Some(&result.1.source_locale), lookup.source_locale());
        assert_eq!(result.1.searched_locales, lookup.searched_locales());
        assert_eq!(lookup.label(), Some("Résumé"));
        assert_eq!(
            result.1.parts,
            vec![
                FormattedPart::Money(Money::new(Currency::parse("USD").unwrap(), 1)),
                FormattedPart::Text(text.to_owned()),
                FormattedPart::Quantity(Usage::new(7, UsageUnit::AudioMillisecond)),
                FormattedPart::Text(text.to_owned()),
            ]
        );
        assert_eq!(args, before);
    }
    let result = catalog.format(&key, &supported[1..], &args).unwrap();
    assert_eq!(
        result.parts.first(),
        Some(&FormattedPart::Literal("Usage: {literal}; ".to_owned()))
    );
}

#[test]
fn default_hits_missing_keys_and_raw_labels_preserve_exact_lookup_diagnostics() {
    let en = LocaleTag::parse("en").unwrap();
    let fr = LocaleTag::parse("fr").unwrap();
    let selected = settle(std::slice::from_ref(&fr), &[fr.clone(), en.clone()], &en).unwrap();
    let chain = lookup_chain(selected.selected(), &en);
    let key = TextKey::parse("display.literal").unwrap();
    let mut catalog = Catalog::default();
    catalog.insert(&en, key.clone(), "<b>{note}</b> /attack");
    let formatted = catalog.format(&key, &chain, &BTreeMap::new()).unwrap();
    let lookup = catalog.lookup(&key, &chain);
    assert_eq!(formatted.key, key);
    assert_eq!(Some(&formatted.source_locale), lookup.source_locale());
    assert_eq!(formatted.searched_locales, lookup.searched_locales());
    assert_eq!(formatted.searched_locales, chain);
    assert_eq!(
        formatted.parts,
        vec![FormattedPart::Literal("<b>{note}</b> /attack".to_owned())]
    );
    let missing = TextKey::parse("display.missing").unwrap();
    for searched in [chain.as_slice(), &[]] {
        assert_eq!(
            catalog.format(&missing, searched, &BTreeMap::new()),
            Err(FormatError::MissingKey {
                key: missing.clone(),
                searched_locales: searched.to_vec(),
            })
        );
    }
    assert!(matches!(
        catalog.format(&key, &[], &BTreeMap::new()),
        Err(FormatError::MissingKey { .. })
    ));
    catalog.insert(&en, key.clone(), "");
    assert_eq!(
        catalog
            .format(&key, &chain, &BTreeMap::new())
            .unwrap()
            .parts,
        vec![FormattedPart::Literal(String::new())]
    );
    catalog.insert_message(&en, &key, &[]).unwrap();
    assert!(
        catalog
            .format(&key, &chain, &BTreeMap::new())
            .unwrap()
            .parts
            .is_empty()
    );
    catalog
        .insert_message(
            &en,
            &key,
            &[MessagePart::Literal("{not_a_slot}".to_owned())],
        )
        .unwrap();
    assert_eq!(
        catalog
            .format(&key, &chain, &BTreeMap::new())
            .unwrap()
            .parts,
        vec![FormattedPart::Literal("{not_a_slot}".to_owned())]
    );
}

#[test]
fn argument_validation_refuses_extra_then_missing_then_wrong_kinds_without_values() {
    let (catalog, key, chain, args) = fixture();
    let mut invalid = args.clone();
    invalid.insert(
        "z_extra".to_owned(),
        ArgumentValue::Text("secret payload".to_owned()),
    );
    invalid.insert(
        "a_extra".to_owned(),
        ArgumentValue::Text("secret payload".to_owned()),
    );
    invalid.remove("cost");
    assert_eq!(
        catalog.format(&key, &chain, &invalid),
        Err(FormatError::ExtraArgument {
            name: "a_extra".to_owned()
        })
    );
    let mut invalid = args.clone();
    invalid.remove("note");
    invalid.insert(
        "cost".to_owned(),
        ArgumentValue::Text("secret payload".to_owned()),
    );
    assert_eq!(
        catalog.format(&key, &chain, &invalid),
        Err(FormatError::MissingArgument {
            name: "note".to_owned()
        })
    );
    assert_eq!(
        catalog.format(&key, &chain, &BTreeMap::new()),
        Err(FormatError::MissingArgument {
            name: "cost".to_owned()
        })
    );
    for (name, value) in [
        (
            "note",
            ArgumentValue::Quantity(Usage::new(1, UsageUnit::AudioMillisecond)),
        ),
        ("count", ArgumentValue::Text("7".to_owned())),
        ("cost", ArgumentValue::Text("USD 0.000001".to_owned())),
        (
            "count",
            ArgumentValue::Quantity(Usage::new(7, UsageUnit::VideoMillisecond)),
        ),
        (
            "cost",
            ArgumentValue::Money(Money::new(Currency::parse("EUR").unwrap(), 1)),
        ),
    ] {
        let mut invalid = args.clone();
        invalid.insert(name.to_owned(), value);
        let error = catalog.format(&key, &chain, &invalid);
        assert_eq!(
            error,
            Err(FormatError::WrongKind {
                name: name.to_owned()
            })
        );
        assert!(!format!("{error:?}").contains("secret payload"));
    }
}

#[test]
fn translation_admission_rejects_changed_slot_sets_atomically() {
    let (mut catalog, key, chain, args) = fixture();
    let expected = catalog.format(&key, &chain, &args).unwrap();
    let usd = Currency::parse("USD").unwrap();
    let valid = vec![
        argument("count", ArgumentKind::Quantity(UsageUnit::AudioMillisecond)),
        argument("note", ArgumentKind::Text),
        argument("cost", ArgumentKind::Money(usd)),
    ];
    let mut extra = valid.clone();
    extra.push(argument("extra", ArgumentKind::Text));
    let mut wrong_kind = valid.clone();
    wrong_kind[0] = argument("count", ArgumentKind::Quantity(UsageUnit::VideoMillisecond));
    let mut wrong_currency = valid.clone();
    wrong_currency[2] = argument("cost", ArgumentKind::Money(Currency::parse("EUR").unwrap()));
    let mut conflicting_repeat = valid.clone();
    conflicting_repeat.push(argument("note", ArgumentKind::Money(usd)));
    for parts in [
        vec![],
        valid[..2].to_vec(),
        extra,
        wrong_kind,
        wrong_currency,
        conflicting_repeat,
    ] {
        assert_eq!(
            catalog.insert_message(&chain[0], &key, &parts),
            Err(MessageError::SlotMismatch)
        );
        assert_eq!(catalog.format(&key, &chain, &args).unwrap(), expected);
        assert_eq!(catalog.lookup(&key, &chain).label(), Some("Résumé"));
    }
    for name in [
        "", "a.b", "Upper", "_bad", "1bad", "a-b", "a b", "é", "note\0", "note\n",
    ] {
        let mut invalid = valid.clone();
        invalid.push(argument(name, ArgumentKind::Text));
        assert_eq!(
            catalog.insert_message(&chain[0], &key, &invalid),
            Err(MessageError::InvalidArgumentName)
        );
        assert_eq!(catalog.format(&key, &chain, &args).unwrap(), expected);
        assert_eq!(
            catalog.declare_slots(
                TextKey::parse("display.other").unwrap(),
                &BTreeMap::from([(name.to_owned(), ArgumentKind::Text)])
            ),
            Err(MessageError::InvalidArgumentName)
        );
    }
    let missing_locale = LocaleTag::parse("de").unwrap();
    assert_eq!(
        catalog.insert_message(&missing_locale, &key, &valid),
        Err(MessageError::MissingEntry)
    );
    let undeclared = TextKey::parse("display.undeclared").unwrap();
    catalog.insert(&chain[0], undeclared.clone(), "raw");
    assert_eq!(
        catalog.insert_message(
            &chain[0],
            &undeclared,
            &[argument("note", ArgumentKind::Text)]
        ),
        Err(MessageError::SlotMismatch)
    );
}

#[test]
fn declarations_are_immutable_and_raw_replacement_cannot_bypass_first_hit_validation() {
    let (mut catalog, key, chain, args) = fixture();
    let usd = Currency::parse("USD").unwrap();
    let slots = BTreeMap::from([
        ("note".to_owned(), ArgumentKind::Text),
        (
            "count".to_owned(),
            ArgumentKind::Quantity(UsageUnit::AudioMillisecond),
        ),
        ("cost".to_owned(), ArgumentKind::Money(usd)),
    ]);
    catalog.declare_slots(key.clone(), &slots).unwrap();
    let expected = catalog.format(&key, &chain, &args).unwrap();
    for changed in [
        BTreeMap::new(),
        BTreeMap::from([("note".to_owned(), ArgumentKind::Money(usd))]),
    ] {
        assert_eq!(
            catalog.declare_slots(key.clone(), &changed),
            Err(MessageError::DeclarationMismatch)
        );
        assert_eq!(catalog.format(&key, &chain, &args).unwrap(), expected);
    }
    catalog.insert(&chain[0], key.clone(), "replacement {note}");
    assert_eq!(
        catalog.lookup(&key, &chain).label(),
        Some("replacement {note}")
    );
    assert_eq!(
        catalog.format(&key, &chain, &args),
        Err(FormatError::UnvalidatedMessage)
    );
    assert!(catalog.format(&key, &chain[1..], &args).is_ok());
    let late = TextKey::parse("display.late").unwrap();
    catalog.insert(&chain[0], late.clone(), "raw");
    catalog
        .insert_message(
            &chain[0],
            &late,
            &[MessagePart::Literal("literal".to_owned())],
        )
        .unwrap();
    catalog
        .declare_slots(
            late.clone(),
            &BTreeMap::from([("note".to_owned(), ArgumentKind::Text)]),
        )
        .unwrap();
    assert_eq!(
        catalog.format(
            &late,
            &chain,
            &BTreeMap::from([("note".to_owned(), ArgumentValue::Text("text".to_owned()))])
        ),
        Err(FormatError::UnvalidatedMessage)
    );
}

#[test]
fn canonical_usage_units_and_money_micro_units_survive_extreme_values_exactly() {
    let locale = LocaleTag::parse("en").unwrap();
    let key = TextKey::parse("display.exact").unwrap();
    for unit in [
        UsageUnit::Token,
        UsageUnit::Character,
        UsageUnit::Byte,
        UsageUnit::AudioMillisecond,
        UsageUnit::VideoMillisecond,
        UsageUnit::Image,
    ] {
        for tag in ["USD", "EUR", "ZZZ"] {
            let currency = Currency::parse(tag).unwrap();
            let mut catalog = Catalog::default();
            catalog.insert(&locale, key.clone(), "Exact");
            catalog
                .declare_slots(
                    key.clone(),
                    &BTreeMap::from([
                        ("quantity_1".to_owned(), ArgumentKind::Quantity(unit)),
                        ("money".to_owned(), ArgumentKind::Money(currency)),
                    ]),
                )
                .unwrap();
            catalog
                .insert_message(
                    &locale,
                    &key,
                    &[
                        argument("quantity_1", ArgumentKind::Quantity(unit)),
                        argument("money", ArgumentKind::Money(currency)),
                    ],
                )
                .unwrap();
            for value in [0, 1, 1_000_001, u128::MAX] {
                let quantity = Usage::new(value, unit);
                let money = Money::new(currency, value);
                let result = catalog
                    .format(
                        &key,
                        std::slice::from_ref(&locale),
                        &BTreeMap::from([
                            ("quantity_1".to_owned(), ArgumentValue::Quantity(quantity)),
                            ("money".to_owned(), ArgumentValue::Money(money)),
                        ]),
                    )
                    .unwrap();
                assert_eq!(
                    result.parts,
                    vec![
                        FormattedPart::Quantity(quantity),
                        FormattedPart::Money(money)
                    ]
                );
                let FormattedPart::Quantity(actual) = result.parts[0] else {
                    panic!("quantity part")
                };
                assert_eq!(actual.quantity(), value);
                assert_eq!(actual.unit(), unit);
                let FormattedPart::Money(actual) = result.parts[1] else {
                    panic!("money part")
                };
                assert_eq!(actual.micros(), value);
                assert_eq!(actual.currency().as_str(), tag);
            }
        }
    }
}
