use df_content::catalog::{CatalogEntry, CatalogError, CatalogLimits, CatalogSnapshot};
use df_content::graph::{GraphBeat, GraphLimits, GraphView};
use df_content::import::{
    DecodedUpload, ImportAdmission, ImportCatalog, ImportCode, ImportEntrySource, ImportLimits,
    ImportLocation, ImportSource, ImportSpec, validate_import,
};

// These are private structural identities, never evidence of a reviewed G07 mechanic.
const CORPUS: &[u8] = include_bytes!("golden_corpus.json");
const VERSION: u64 = 1;
const PINS: [&str; 3] = [
    "synthetic-corpus-v1",
    "G07-unqualified",
    "no-mechanics-handler",
];
const FORMAT: &str = "private-corpus-ledger";
const ITEM: u8 = 1;
const BEAT: u8 = 1;
const BEATS: &[GraphBeat<'_, u8>] = &[GraphBeat {
    id: &BEAT,
    next: &[],
    terminal: true,
}];

fn limits() -> ImportLimits {
    ImportLimits {
        catalog: CatalogLimits {
            max_complete_bytes: 16_384,
            max_entries: 1,
            max_item_bytes: 16_384,
            max_total_item_bytes: 16_384,
        },
        graph: GraphLimits {
            max_beats: 1,
            max_edges: 0,
            max_facts: 0,
            max_alternatives: 0,
            max_work: 16,
        },
        max_path_bytes: 64,
        max_path_components: 2,
        max_total_path_bytes: 64,
    }
}

fn upload<'a>() -> DecodedUpload<'a, u64, [&'static str; 3], &'static str> {
    DecodedUpload {
        source: ImportSource::Uploaded(CORPUS),
        version: &VERSION,
        pins: &PINS,
        format: &FORMAT,
    }
}

fn spec<'a>(
    admission: ImportAdmission<'a, [&'static str; 3]>,
) -> ImportSpec<'a, u64, [&'static str; 3], &'static str> {
    ImportSpec {
        version: &VERSION,
        admission,
        format: &FORMAT,
        namespace: "corpus",
        limits: limits(),
    }
}

fn graph() -> GraphView<'static, u8, u8, u8> {
    GraphView {
        start: &BEAT,
        beats: BEATS,
        facts: &[],
        alternatives: &[],
    }
}

#[test]
fn missing_g07_source_admission_cannot_produce_a_corpus_candidate() {
    let entries = [CatalogEntry::new(&ITEM, CORPUS)];
    let sources = [ImportEntrySource {
        path: "corpus/identity",
        location: ImportLocation {
            start: 0,
            end: CORPUS.len(),
        },
    }];
    let catalog = ImportCatalog {
        entries: &entries,
        sources: &sources,
    };
    let refusal = validate_import(
        &upload(),
        &spec(ImportAdmission::SourceGap),
        &catalog,
        &graph(),
    )
    .err()
    .unwrap();
    assert_eq!(refusal.code, ImportCode::SourceGap);
    assert_eq!(refusal.entry_index, None);
    assert_eq!(refusal.location, None);
}

#[test]
fn absent_rights_and_unsupported_rules_remain_distinct_source_owner_refusals() {
    let catalog = ImportCatalog::<u8> {
        entries: &[],
        sources: &[],
    };
    for (admission, expected) in [
        (ImportAdmission::RightsGap, ImportCode::RightsGap),
        (
            ImportAdmission::UnsupportedRules,
            ImportCode::UnsupportedRules,
        ),
    ] {
        let refusal = validate_import(&upload(), &spec(admission), &catalog, &graph())
            .err()
            .unwrap();
        assert_eq!(refusal.code, expected);
        assert_eq!(refusal.entry_index, None);
        assert_eq!(refusal.location, None);
    }
}

#[test]
fn stale_corpus_version_or_source_identity_is_refused_before_record_processing() {
    let catalog = ImportCatalog::<u8> {
        entries: &[],
        sources: &[],
    };
    let other_version = VERSION + 1;
    let other_pins = [
        "synthetic-corpus-v2",
        "G07-unqualified",
        "no-mechanics-handler",
    ];
    let mut stale_version = upload();
    stale_version.version = &other_version;
    let mut stale_pins = upload();
    stale_pins.pins = &other_pins;
    for stale in [&stale_version, &stale_pins] {
        let refusal = validate_import(
            stale,
            &spec(ImportAdmission::Admitted(&PINS)),
            &catalog,
            &graph(),
        )
        .err()
        .unwrap();
        assert_eq!(refusal.code, ImportCode::SourceGap);
        assert_eq!(refusal.entry_index, None);
    }
}

#[test]
fn admitted_synthetic_identity_retains_exact_bytes_without_qualifying_mechanics() {
    let entries = [CatalogEntry::new(&ITEM, CORPUS)];
    let sources = [ImportEntrySource {
        path: "corpus/identity",
        location: ImportLocation {
            start: 0,
            end: CORPUS.len(),
        },
    }];
    let catalog = ImportCatalog {
        entries: &entries,
        sources: &sources,
    };
    let current = spec(ImportAdmission::Admitted(&PINS));
    let candidate = validate_import(&upload(), &current, &catalog, &graph()).unwrap();
    assert!(std::ptr::eq(candidate.bytes(), CORPUS));
    assert!(std::ptr::eq(candidate.version(), &VERSION));
    assert!(std::ptr::eq(candidate.pins(), &PINS));
    assert!(std::ptr::eq(candidate.format(), &FORMAT));
    assert_eq!(candidate.item_count(), 1);
}

#[test]
fn immutable_corpus_lookup_requires_the_exact_published_version() {
    let entries = [CatalogEntry::new(&ITEM, CORPUS)];
    let snapshot =
        CatalogSnapshot::from_published(&VERSION, &PINS, CORPUS, &entries, limits().catalog)
            .unwrap();
    let exact = snapshot.get(&VERSION, &ITEM).unwrap().unwrap();
    assert!(std::ptr::eq(exact, CORPUS));
    assert_eq!(
        snapshot.get(&(VERSION + 1), &ITEM),
        Err(CatalogError::VersionMismatch)
    );
    assert_eq!(snapshot.get(&VERSION, &(ITEM + 1)), Ok(None));
    assert!(std::ptr::eq(snapshot.complete_bytes(), CORPUS));
    assert!(std::ptr::eq(snapshot.pins(), &PINS));
}

#[test]
fn a_complete_corpus_is_bounded_without_truncating_or_rewriting_fixture_bytes() {
    let entries = [CatalogEntry::new(&ITEM, CORPUS)];
    let mut bounded = limits().catalog;
    bounded.max_complete_bytes = CORPUS.len() - 1;
    let refusal = CatalogSnapshot::from_published(&VERSION, &PINS, CORPUS, &entries, bounded)
        .err()
        .unwrap();
    assert_eq!(
        refusal,
        CatalogError::CompleteBytesLimit {
            actual: CORPUS.len(),
            maximum: CORPUS.len() - 1
        }
    );
    bounded.max_complete_bytes = CORPUS.len();
    let exact =
        CatalogSnapshot::from_published(&VERSION, &PINS, CORPUS, &entries, bounded).unwrap();
    assert!(std::ptr::eq(exact.complete_bytes(), CORPUS));
}
