#[path = "fixtures/locale_transition_catalog.rs"]
mod fixture;

use std::collections::BTreeMap;

use df_locale::{ArgumentValue, CatalogLoadError, FormatError};
use df_types::LocaleTag;
use df_ui::{SceneTransitionValidationError, SceneTransitionView};
use fixture::CatalogFixture;

#[test]
fn admitted_catalog_negotiates_the_same_stable_keys_for_native_and_browser_text() {
    let fixture = CatalogFixture::new();
    let catalog = fixture.load(1).unwrap();
    for (requested, selected, expected_action) in [
        ("fr", "fr", "Entrer dans les salles"),
        ("en-US", "en", "Enter the halls"),
    ] {
        let request = [LocaleTag::parse(requested).unwrap()];
        let selection = df_locale::settle(&request, &fixture.locales, &fixture.locales[0]).unwrap();
        assert_eq!(selection.used_fallback(), requested != selected);
        let messages = fixture.messages(&catalog, &request, 1).unwrap();
        let text = SceneTransitionView::catalog_text(&1, messages.each_ref()).unwrap();
        assert_eq!(text[3], expected_action);
        assert!(text[1].starts_with("<b>"));
        assert!(text[2].contains("{name}<script>literal</script>"));
        assert!(text[2].ends_with("340282366920938463463374607431768211455 byte / USD 0.000001"));
        for (key, message) in fixture.keys.iter().zip(&messages) {
            assert_eq!(&message.message.key, key);
            assert_eq!(message.version, &1);
            assert_eq!(
                message.message.source_locale,
                LocaleTag::parse(selected).unwrap()
            );
            assert_eq!(
                message.message.searched_locales,
                [LocaleTag::parse(selected).unwrap()]
            );
        }
    }
}

#[test]
fn refused_reload_keeps_every_connected_label_and_revision_then_replaces_together() {
    let mut fixture = CatalogFixture::new();
    let mut catalog = fixture.load(1).unwrap();
    let request = [LocaleTag::parse("fr").unwrap()];
    let before = SceneTransitionView::catalog_text(
        &1,
        fixture.messages(&catalog, &request, 1).unwrap().each_ref(),
    )
    .unwrap();
    fixture.parts[1][0] = vec![df_locale::MessagePart::Literal(
        "Changed location".to_owned(),
    )];
    assert_eq!(
        catalog.reload(
            2,
            &fixture.locales,
            &fixture.declarations,
            &fixture.entries(Some((1, 3)))
        ),
        Err(CatalogLoadError::MissingEntry {
            locale: fixture.locales[1].clone(),
            key: fixture.keys[3].clone()
        })
    );
    assert_eq!(
        SceneTransitionView::catalog_text(
            &1,
            fixture.messages(&catalog, &request, 1).unwrap().each_ref()
        )
        .unwrap(),
        before
    );
    let previous_action = fixture.parts[1][3].clone();
    fixture.parts[1][3] = vec![
        df_locale::MessagePart::Literal("synthetic-private-catalog-literal".to_owned()),
        df_locale::MessagePart::Argument {
            name: "undeclared".to_owned(),
            kind: df_locale::ArgumentKind::Text,
        },
    ];
    let error = catalog
        .reload(
            2,
            &fixture.locales,
            &fixture.declarations,
            &fixture.entries(None),
        )
        .unwrap_err();
    assert!(matches!(error, CatalogLoadError::InvalidMessage { .. }));
    assert!(!format!("{error:?}").contains("synthetic-private-catalog-literal"));
    assert_eq!(
        SceneTransitionView::catalog_text(
            &1,
            fixture.messages(&catalog, &request, 1).unwrap().each_ref()
        )
        .unwrap(),
        before
    );
    fixture.parts[1][3] = previous_action;
    catalog
        .reload(
            2,
            &fixture.locales,
            &fixture.declarations,
            &fixture.entries(None),
        )
        .unwrap();
    let messages = fixture.messages(&catalog, &request, 1).unwrap();
    assert!(messages.iter().all(|message| message.version == &2));
    let after = SceneTransitionView::catalog_text(&2, messages.each_ref()).unwrap();
    assert_eq!(after[0], "Changed location");
    assert_eq!(after[1..], before[1..]);
    assert_eq!(
        SceneTransitionView::catalog_text(&1, messages.each_ref()),
        Err(SceneTransitionValidationError::CatalogVersionMismatch)
    );
}

#[test]
fn mixed_snapshot_messages_and_missing_keys_return_no_partial_transition_text() {
    let fixture = CatalogFixture::new();
    let old = fixture.load(1).unwrap();
    let new = fixture.load(2).unwrap();
    let request = [LocaleTag::parse("fr").unwrap()];
    let old_messages = fixture.messages(&old, &request, 1).unwrap();
    let new_messages = fixture.messages(&new, &request, 1).unwrap();
    let mixed = [
        &old_messages[0],
        &new_messages[1],
        &old_messages[2],
        &old_messages[3],
    ];
    let error = SceneTransitionView::catalog_text(&1, mixed).unwrap_err();
    assert_eq!(
        error,
        SceneTransitionValidationError::CatalogVersionMismatch
    );
    assert_eq!(
        error.to_string(),
        "scene transition catalog revision does not match"
    );
    let missing = df_locale::TextKey::parse("transition.missing").unwrap();
    assert!(
        matches!(old.format(&missing, &request, &BTreeMap::new()), Err(FormatError::MissingKey { key, searched_locales }) if key == missing && searched_locales == request)
    );
    let invalid = BTreeMap::from([(
        "private".to_owned(),
        ArgumentValue::Text("synthetic-private-payload".to_owned()),
    )]);
    let error = old
        .format(&fixture.keys[0], &request, &invalid)
        .err()
        .unwrap();
    assert_eq!(
        error,
        FormatError::ExtraArgument {
            name: "private".to_owned()
        }
    );
    assert!(!format!("{error:?}").contains("synthetic-private-payload"));
}
