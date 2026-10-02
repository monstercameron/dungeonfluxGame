/// Caller-selected limits for one already-published catalog snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogLimits {
    pub max_complete_bytes: usize,
    pub max_entries: usize,
    pub max_item_bytes: usize,
    pub max_total_item_bytes: usize,
}

/// A read-only item view supplied by the published representation's owner.
#[derive(Debug)]
pub struct CatalogEntry<'a, ItemId> {
    item_id: &'a ItemId,
    bytes: &'a [u8],
}

impl<'a, ItemId> CatalogEntry<'a, ItemId> {
    pub fn new(item_id: &'a ItemId, bytes: &'a [u8]) -> Self {
        Self { item_id, bytes }
    }

    pub fn item_id(&self) -> &'a ItemId {
        self.item_id
    }

    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }
}

/// Safe structural refusals; no failure retains content or rights evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogError {
    CompleteBytesLimit {
        actual: usize,
        maximum: usize,
    },
    EntryCountLimit {
        actual: usize,
        maximum: usize,
    },
    ItemBytesLimit {
        entry_index: usize,
        actual: usize,
        maximum: usize,
    },
    TotalItemBytesLimit {
        maximum: usize,
    },
    DuplicateItem {
        first_index: usize,
        duplicate_index: usize,
    },
    VersionMismatch,
}

/// A bounded, immutable view of one already-published version.
///
/// The owner supplies the exact version, complete bytes, pinned manifest/rights
/// facts, and decoded entries from that publication. This view does not publish,
/// decode, authorize delivery, validate rights, or mint a version. Version, pin,
/// and item identity equality must remain stable while borrowed. Item lookup
/// requires the pinned version; there is no mutable or "latest" interface.
///
/// Complete publication bytes cannot be edited through a snapshot:
/// ```compile_fail
/// use df_content::catalog::CatalogSnapshot;
/// fn edit(snapshot: &mut CatalogSnapshot<'_, u64, (), u64>) {
///     snapshot.complete_bytes()[0] = 0;
/// }
/// ```
/// The borrowed version cannot be replaced through a snapshot:
/// ```compile_fail
/// use df_content::catalog::CatalogSnapshot;
/// fn replace(snapshot: &mut CatalogSnapshot<'_, u64, (), u64>) {
///     *snapshot.version() = 9;
/// }
/// ```
pub struct CatalogSnapshot<'a, Version, Pins, ItemId> {
    version: &'a Version,
    pins: &'a Pins,
    complete_bytes: &'a [u8],
    entries: &'a [CatalogEntry<'a, ItemId>],
}

impl<'a, Version: Eq, Pins, ItemId: Eq> CatalogSnapshot<'a, Version, Pins, ItemId> {
    /// Checks the whole supplied view before returning any usable snapshot.
    /// No content is copied or allocated. Identity comparisons are bounded by
    /// the caller's entry-count limit; duplicate checking is quadratic in that
    /// bounded count and requires no invented ordering or hash identity.
    pub fn from_published(
        version: &'a Version,
        pins: &'a Pins,
        complete_bytes: &'a [u8],
        entries: &'a [CatalogEntry<'a, ItemId>],
        limits: CatalogLimits,
    ) -> Result<Self, CatalogError> {
        if complete_bytes.len() > limits.max_complete_bytes {
            return Err(CatalogError::CompleteBytesLimit {
                actual: complete_bytes.len(),
                maximum: limits.max_complete_bytes,
            });
        }
        if entries.len() > limits.max_entries {
            return Err(CatalogError::EntryCountLimit {
                actual: entries.len(),
                maximum: limits.max_entries,
            });
        }
        let mut total_item_bytes = 0_usize;
        for (entry_index, entry) in entries.iter().enumerate() {
            if entry.bytes.len() > limits.max_item_bytes {
                return Err(CatalogError::ItemBytesLimit {
                    entry_index,
                    actual: entry.bytes.len(),
                    maximum: limits.max_item_bytes,
                });
            }
            total_item_bytes = total_item_bytes
                .checked_add(entry.bytes.len())
                .filter(|total| *total <= limits.max_total_item_bytes)
                .ok_or(CatalogError::TotalItemBytesLimit {
                    maximum: limits.max_total_item_bytes,
                })?;
            for (first_index, prior) in entries.iter().take(entry_index).enumerate() {
                if prior.item_id == entry.item_id {
                    return Err(CatalogError::DuplicateItem {
                        first_index,
                        duplicate_index: entry_index,
                    });
                }
            }
        }
        Ok(Self {
            version,
            pins,
            complete_bytes,
            entries,
        })
    }

    pub fn version(&self) -> &'a Version {
        self.version
    }

    pub fn pins(&self) -> &'a Pins {
        self.pins
    }

    /// Exact owner-supplied complete representation, including its manifest.
    pub fn complete_bytes(&self) -> &'a [u8] {
        self.complete_bytes
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns exact item bytes only for the caller's expected published version.
    /// A missing item is distinct from an attempt to use another version.
    pub fn get(
        &self,
        expected_version: &Version,
        item_id: &ItemId,
    ) -> Result<Option<&'a [u8]>, CatalogError> {
        if expected_version != self.version {
            return Err(CatalogError::VersionMismatch);
        }
        Ok(self
            .entries
            .iter()
            .find(|entry| entry.item_id == item_id)
            .map(|entry| entry.bytes))
    }
}
