use df_locale::{LocaleConfigurationError, settle};
use df_types::LocaleTag;

fn tags(spellings: &[&str]) -> Vec<LocaleTag> {
    spellings
        .iter()
        .map(|spelling| LocaleTag::parse(spelling).unwrap())
        .collect()
}

#[test]
fn caller_discloses_first_later_default_and_empty_request_outcomes() {
    let supported = tags(&["en", "fr"]);
    let default = LocaleTag::parse("en").unwrap();
    for (requested, selected, fallback) in [
        (vec!["fr", "en"], "fr", false),
        (vec!["de", "fr", "en"], "fr", true),
        (vec!["de", "ja"], "en", true),
        (vec![], "en", true),
        (vec!["en"], "en", false),
    ] {
        let selection = settle(&tags(&requested), &supported, &default).unwrap();
        assert_eq!(selection.selected().as_str(), selected);
        assert_eq!(selection.used_fallback(), fallback);
    }
}

#[test]
fn unsupported_default_refuses_before_a_supported_request_or_empty_inputs() {
    let requested = tags(&["fr"]);
    let supported = tags(&["fr"]);
    let default = LocaleTag::parse("en").unwrap();
    for (requested, supported) in [
        (requested.as_slice(), supported.as_slice()),
        (&[], supported.as_slice()),
        (requested.as_slice(), &[]),
        (&[], &[]),
    ] {
        assert_eq!(
            settle(requested, supported, &default),
            Err(LocaleConfigurationError::UnsupportedDefault)
        );
    }
}

#[test]
fn only_actual_parsed_supported_values_match() {
    let default = LocaleTag::parse("en").unwrap();
    for (requested, admitted) in [
        ("en", "en-US"),
        ("iw", "he"),
        ("zh-Hant", "zh-Hans"),
        ("x-private", "x-other"),
        ("i-klingon", "tlh"),
        ("en-a-aa-b-bb", "en-b-bb-a-aa"),
    ] {
        let supported = tags(&[admitted, "en"]);
        let requested = tags(&[requested]);
        let selection = settle(&requested, &supported, &default).unwrap();
        assert_eq!(selection.selected(), &default);
        // The exact supported default is still an ordinary first request match.
        assert_eq!(selection.used_fallback(), requested[0] != default);
    }

    let regional_only = tags(&["en-US"]);
    let regional_default = LocaleTag::parse("en-US").unwrap();
    let selection = settle(&tags(&["en"]), &regional_only, &regional_default).unwrap();
    assert_eq!(selection.selected().as_str(), "en-us");
    assert!(selection.used_fallback());

    for spelling in ["EN-us", "IW", "ZH-hANT", "X-private", "I-KLINGON"] {
        let supported = tags(&[spelling, "en"]);
        let requested = tags(&[&spelling.to_ascii_lowercase()]);
        let selection = settle(&requested, &supported, &default).unwrap();
        assert_eq!(selection.selected(), &requested[0]);
        assert!(!selection.used_fallback());
    }
}

#[test]
fn duplicate_inventory_does_not_override_requested_priority() {
    let default = LocaleTag::parse("en").unwrap();
    for supported in [tags(&["en", "fr", "fr", "en"]), tags(&["fr", "en"])] {
        let first = settle(&tags(&["fr", "fr", "en"]), &supported, &default).unwrap();
        assert_eq!(first.selected().as_str(), "fr");
        assert!(!first.used_fallback());
        let later = settle(&tags(&["de", "de", "fr", "en"]), &supported, &default).unwrap();
        assert_eq!(later.selected().as_str(), "fr");
        assert!(later.used_fallback());
    }
}

#[test]
fn caller_resolves_destinations_independently_and_preserves_borrowed_inputs() {
    let supported = tags(&["en", "fr", "de"]);
    let default = LocaleTag::parse("en").unwrap();
    let private_requests = tags(&["fr", "de"]);
    let display_requests = tags(&["ja", "de"]);
    let speech_requests = tags(&["x-voice", "fr", "de"]);
    let before = (
        supported.clone(),
        default.clone(),
        private_requests.clone(),
        display_requests.clone(),
        speech_requests.clone(),
    );
    let source_rule_identity = ("source-revision-7", "action.attack");

    let private = settle(&private_requests, &supported, &default).unwrap();
    let display = settle(&display_requests, &supported, &default).unwrap();
    let speech = settle(&speech_requests, &supported, &default).unwrap();
    assert_eq!(private.selected().as_str(), "fr");
    assert!(!private.used_fallback());
    assert_eq!(display.selected().as_str(), "de");
    assert!(display.used_fallback());
    assert_eq!(speech.selected().as_str(), "fr");
    assert!(speech.used_fallback());
    assert_eq!(source_rule_identity, ("source-revision-7", "action.attack"));
    assert_eq!(
        (
            supported,
            default,
            private_requests,
            display_requests,
            speech_requests
        ),
        before
    );
}

#[test]
fn result_owns_a_bounded_tag_and_error_exposes_only_configuration_facts() {
    let selection = {
        let maximum = format!("x{}-abcde", "-abcdefg".repeat(31));
        let supported = tags(&[&maximum]);
        settle(&[], &supported, &supported[0]).unwrap()
    };
    assert_eq!(selection.selected().as_str().len(), LocaleTag::MAX_BYTES);
    assert!(selection.used_fallback());
    assert_eq!(selection, selection.clone());

    let error = LocaleConfigurationError::UnsupportedDefault;
    let standard_error: &dyn std::error::Error = &error;
    assert_eq!(
        standard_error.to_string(),
        "configured default locale is unsupported"
    );
    assert!(standard_error.source().is_none());
    assert_eq!(format!("{error:?}"), "UnsupportedDefault");
}
