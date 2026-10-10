use df_locale::{
    CatalogEntry, CatalogLoadError, MessagePart, TextKey, VersionedCatalog,
    VersionedFormattedMessage,
};
use df_types::{LocaleTag, RevisionLabel};
use df_ui::{SceneTransitionValidationError, SceneTransitionView};
use std::collections::BTreeMap;

fn keys() -> [TextKey; 4] {
    [
        "scene.location",
        "scene.title",
        "scene.narration",
        "scene.continue",
    ]
    .map(|key| TextKey::parse(key).unwrap())
}

fn catalog(version: &str, title: &str) -> VersionedCatalog<RevisionLabel> {
    let locales = [
        LocaleTag::parse("en").unwrap(),
        LocaleTag::parse("fr").unwrap(),
    ];
    let keys = keys();
    let declarations = keys
        .iter()
        .map(|key| (key.clone(), BTreeMap::new()))
        .collect();
    let english = [
        "Below Greyhaven",
        title,
        "Torchlight catches the water.",
        "Enter the halls",
    ];
    let french = [
        "Sous Greyhaven",
        title,
        "La torche éclaire l'eau.",
        "Entrer dans les salles",
    ];
    let mut parts = Vec::new();
    for text in english.into_iter().chain(french) {
        parts.push(vec![MessagePart::Literal(text.to_owned())]);
    }
    let mut entries = Vec::new();
    for (index, language) in locales.iter().enumerate() {
        let labels = if index == 0 { english } else { french };
        for (key_index, key) in keys.iter().enumerate() {
            entries.push(CatalogEntry {
                locale: language,
                key,
                text: labels[key_index],
                parts: &parts[index * keys.len() + key_index],
            });
        }
    }
    let revisions = vec![RevisionLabel::new(Some(version)).unwrap(); entries.len()];
    VersionedCatalog::load_revision(Some(version), &locales, &declarations, &entries, &revisions)
        .unwrap()
}

fn messages<'a>(
    catalog: &'a VersionedCatalog<RevisionLabel>,
    language: &LocaleTag,
) -> [VersionedFormattedMessage<'a, RevisionLabel>; 4] {
    keys().map(|key| {
        catalog
            .format(&key, std::slice::from_ref(language), &BTreeMap::new())
            .unwrap()
    })
}

#[test]
fn actual_transition_consumer_accepts_only_the_admitted_source_revision() {
    let catalog = catalog("text:1", "The sunken halls");
    for (language, expected_action) in [("en", "Enter the halls"), ("fr", "Entrer dans les salles")]
    {
        let language = LocaleTag::parse(language).unwrap();
        let messages = messages(&catalog, &language);
        let text =
            SceneTransitionView::catalog_text(catalog.version(), messages.each_ref()).unwrap();
        assert_eq!(text[1], "The sunken halls");
        assert_eq!(text[3], expected_action);
        assert!(
            messages
                .iter()
                .all(|message| message.version == catalog.version())
        );
        assert_eq!(messages[3].message.key, keys()[3]);
        assert_eq!(messages[3].message.source_locale, language);
        assert_eq!(
            SceneTransitionView::catalog_text(
                &RevisionLabel::new(Some("text:2")).unwrap(),
                messages.each_ref()
            ),
            Err(SceneTransitionValidationError::CatalogVersionMismatch),
        );
    }
}

#[test]
fn failed_source_reload_preserves_consumer_text_and_mixed_messages_publish_nothing() {
    let mut old = catalog("text:1", "The sunken halls");
    let french = LocaleTag::parse("fr").unwrap();
    let before =
        SceneTransitionView::catalog_text(old.version(), messages(&old, &french).each_ref())
            .unwrap();
    let declarations = keys()
        .into_iter()
        .map(|key| (key, BTreeMap::new()))
        .collect();
    assert!(matches!(
        old.reload_revision(
            Some("text:2"),
            std::slice::from_ref(&french),
            &declarations,
            &[],
            &[]
        ),
        Err(CatalogLoadError::MissingEntry { .. })
    ));
    assert_eq!(
        SceneTransitionView::catalog_text(old.version(), messages(&old, &french).each_ref())
            .unwrap(),
        before
    );

    let new = catalog("text:2", "synthetic-private-title");
    let old_messages = messages(&old, &french);
    let new_messages = messages(&new, &french);
    let mixed = [
        &old_messages[0],
        &new_messages[1],
        &old_messages[2],
        &old_messages[3],
    ];
    let error = SceneTransitionView::catalog_text(old.version(), mixed).unwrap_err();
    assert_eq!(
        error,
        SceneTransitionValidationError::CatalogVersionMismatch
    );
    assert!(!error.to_string().contains("synthetic-private-title"));
    assert_eq!(
        SceneTransitionView::catalog_text(old.version(), old_messages.each_ref()).unwrap(),
        before
    );
    let replaced =
        SceneTransitionView::catalog_text(new.version(), new_messages.each_ref()).unwrap();
    assert_eq!(replaced[1], "synthetic-private-title");
}
