use df_content::catalog::{CatalogEntry, CatalogError, CatalogLimits, CatalogSnapshot};

#[derive(Debug, PartialEq, Eq)]
struct PublishedVersion(u64);

#[derive(Debug, PartialEq, Eq)]
struct ItemId(u64);

#[derive(Debug, PartialEq, Eq)]
struct PublicationPins {
    source_revision: u64,
    namespace_revision: u64,
    rights_revision: u64,
}

fn limits() -> CatalogLimits {
    CatalogLimits {
        max_complete_bytes: 64,
        max_entries: 4,
        max_item_bytes: 16,
        max_total_item_bytes: 32,
    }
}

#[test]
fn lookup_retains_exact_published_version_complete_bytes_and_pins() {
    let version = PublishedVersion(3);
    let pins = PublicationPins {
        source_revision: 2,
        namespace_revision: 4,
        rights_revision: 6,
    };
    let first = ItemId(1);
    let second = ItemId(2);
    let entries = [
        CatalogEntry::new(&first, b"first"),
        CatalogEntry::new(&second, b"second"),
    ];
    let complete = b"complete bytes including manifest";
    let catalog =
        CatalogSnapshot::from_published(&version, &pins, complete, &entries, limits()).unwrap();

    assert!(std::ptr::eq(catalog.version(), &version));
    assert!(std::ptr::eq(catalog.pins(), &pins));
    assert_eq!(catalog.complete_bytes(), complete);
    assert_eq!(catalog.len(), 2);
    assert!(!catalog.is_empty());
    assert_eq!(catalog.get(&version, &first), Ok(Some(b"first".as_slice())));
    assert_eq!(
        catalog.get(&version, &second),
        Ok(Some(b"second".as_slice()))
    );
    assert_eq!(catalog.get(&version, &ItemId(99)), Ok(None));
    assert_eq!(
        catalog.get(&PublishedVersion(4), &first),
        Err(CatalogError::VersionMismatch)
    );
    assert!(std::ptr::eq(entries[0].item_id(), &first));
    assert_eq!(entries[0].bytes(), b"first");
}

#[test]
fn changed_publication_has_a_separate_snapshot_and_old_lookup_stays_unchanged() {
    let first_version = PublishedVersion(1);
    let second_version = PublishedVersion(2);
    let first_pins = PublicationPins {
        source_revision: 1,
        namespace_revision: 1,
        rights_revision: 1,
    };
    let second_pins = PublicationPins {
        source_revision: 2,
        namespace_revision: 1,
        rights_revision: 2,
    };
    let item = ItemId(8);
    let first_entries = [CatalogEntry::new(&item, b"original")];
    let second_entries = [CatalogEntry::new(&item, b"replacement")];
    let first = CatalogSnapshot::from_published(
        &first_version,
        &first_pins,
        b"published first",
        &first_entries,
        limits(),
    )
    .unwrap();
    let second = CatalogSnapshot::from_published(
        &second_version,
        &second_pins,
        b"published second",
        &second_entries,
        limits(),
    )
    .unwrap();

    assert_eq!(
        second.get(&second_version, &item),
        Ok(Some(b"replacement".as_slice()))
    );
    assert_eq!(
        first.get(&first_version, &item),
        Ok(Some(b"original".as_slice()))
    );
    assert_eq!(first.complete_bytes(), b"published first");
    assert_eq!(first.pins().rights_revision, 1);
    assert_eq!(
        first.get(&second_version, &item),
        Err(CatalogError::VersionMismatch)
    );
}

#[test]
fn duplicate_item_ids_refuse_the_whole_snapshot_even_when_bytes_match() {
    let version = PublishedVersion(1);
    let item = ItemId(1);
    let same_id = ItemId(1);
    for replacement in [b"same".as_slice(), b"different".as_slice()] {
        let entries = [
            CatalogEntry::new(&item, b"same"),
            CatalogEntry::new(&same_id, replacement),
        ];
        let result = CatalogSnapshot::from_published(&version, &(), b"pack", &entries, limits());
        assert!(matches!(
            result,
            Err(CatalogError::DuplicateItem {
                first_index: 0,
                duplicate_index: 1
            })
        ));
    }
}

#[test]
fn every_independent_size_limit_is_checked_before_snapshot_success() {
    let version = PublishedVersion(1);
    let first = ItemId(1);
    let second = ItemId(2);
    let entries = [
        CatalogEntry::new(&first, b"1234"),
        CatalogEntry::new(&second, b"5678"),
    ];
    let boundary = CatalogLimits {
        max_complete_bytes: 4,
        max_entries: 2,
        max_item_bytes: 4,
        max_total_item_bytes: 8,
    };
    assert!(CatalogSnapshot::from_published(&version, &(), b"pack", &entries, boundary).is_ok());
    assert!(matches!(
        CatalogSnapshot::from_published(&version, &(), b"packs", &entries, boundary),
        Err(CatalogError::CompleteBytesLimit {
            actual: 5,
            maximum: 4
        })
    ));
    assert!(matches!(
        CatalogSnapshot::from_published(
            &version,
            &(),
            b"pack",
            &entries,
            CatalogLimits {
                max_entries: 1,
                ..boundary
            }
        ),
        Err(CatalogError::EntryCountLimit {
            actual: 2,
            maximum: 1
        })
    ));
    assert!(matches!(
        CatalogSnapshot::from_published(
            &version,
            &(),
            b"pack",
            &entries,
            CatalogLimits {
                max_item_bytes: 3,
                ..boundary
            }
        ),
        Err(CatalogError::ItemBytesLimit {
            entry_index: 0,
            actual: 4,
            maximum: 3
        })
    ));
    assert!(matches!(
        CatalogSnapshot::from_published(
            &version,
            &(),
            b"pack",
            &entries,
            CatalogLimits {
                max_total_item_bytes: 7,
                ..boundary
            }
        ),
        Err(CatalogError::TotalItemBytesLimit { maximum: 7 })
    ));
}

#[test]
fn empty_published_view_and_empty_item_bytes_have_no_fabricated_lookup() {
    let version = PublishedVersion(1);
    let entries: [CatalogEntry<'_, ItemId>; 0] = [];
    let zero = CatalogLimits {
        max_complete_bytes: 0,
        max_entries: 0,
        max_item_bytes: 0,
        max_total_item_bytes: 0,
    };
    let empty = CatalogSnapshot::from_published(&version, &(), b"", &entries, zero).unwrap();
    assert!(empty.is_empty());
    assert_eq!(empty.len(), 0);
    assert_eq!(empty.get(&version, &ItemId(1)), Ok(None));

    let item = ItemId(1);
    let entries = [CatalogEntry::new(&item, b"")];
    let present =
        CatalogSnapshot::from_published(&version, &(), b"pack", &entries, limits()).unwrap();
    assert_eq!(present.get(&version, &item), Ok(Some(b"".as_slice())));
}

#[test]
fn binary_item_lookup_preserves_exact_bytes_without_decoding_or_normalizing() {
    let version = PublishedVersion(1);
    let item = ItemId(1);
    let bytes = [0, 255, 128, b'<', b'>', b'{', b'}'];
    let entries = [CatalogEntry::new(&item, &bytes)];
    let snapshot =
        CatalogSnapshot::from_published(&version, &(), &bytes, &entries, limits()).unwrap();
    let found = snapshot.get(&version, &item).unwrap().unwrap();
    assert_eq!(found, bytes);
    assert!(std::ptr::eq(found.as_ptr(), bytes.as_ptr()));
    assert_eq!(snapshot.complete_bytes(), bytes);
}
