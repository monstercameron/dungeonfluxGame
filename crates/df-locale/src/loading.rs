use crate::{
    ArgumentKind, ArgumentValue, Catalog, FormatError, FormattedMessage, Lookup, MessageError,
    MessagePart, TextKey,
};
use df_types::{LocaleTag, RevisionLabel, RevisionLabelError};
use std::collections::{BTreeMap, BTreeSet};

/// One borrowed translation to admit against a source key's declared slots.
/// Empty labels and empty messages are explicit entries, not missing translations.
pub struct CatalogEntry<'a> {
    pub locale: &'a LocaleTag,
    pub key: &'a TextKey,
    pub text: &'a str,
    pub parts: &'a [MessagePart],
}

/// A complete, immutable catalog snapshot carrying its caller-owned revision.
///
/// The caller owns revision identity, required inventory, and source provenance.
/// Loading validates completeness and message declarations; it establishes no
/// external trust, language support, rights, or cryptographic identity.
pub struct VersionedCatalog<V> {
    version: V,
    catalog: Catalog,
}

/// The exact snapshot revision accompanying a hit or explicit missing-key result.
pub struct VersionedLookup<'a, V> {
    pub version: &'a V,
    pub lookup: Lookup<'a>,
}

/// The exact snapshot revision accompanying validated semantic formatted parts.
pub struct VersionedFormattedMessage<'a, V> {
    pub version: &'a V,
    pub message: FormattedMessage,
}

/// Atomic loading refusals identifying inventory or declarations, never supplied text.
#[derive(Debug, Eq, PartialEq)]
pub enum CatalogLoadError {
    InvalidRevision(RevisionLabelError),
    RevisionInventoryMismatch,
    MixedRevision {
        locale: LocaleTag,
        key: TextKey,
    },
    EmptyManifest,
    DuplicateLocale {
        locale: LocaleTag,
    },
    UnexpectedLocale {
        locale: LocaleTag,
    },
    UnexpectedKey {
        key: TextKey,
    },
    DuplicateEntry {
        locale: LocaleTag,
        key: TextKey,
    },
    MissingEntry {
        locale: LocaleTag,
        key: TextKey,
    },
    InvalidDeclaration {
        key: TextKey,
        error: MessageError,
    },
    InvalidMessage {
        locale: LocaleTag,
        key: TextKey,
        error: MessageError,
    },
}

impl VersionedCatalog<RevisionLabel> {
    /// Loads one source-labelled immutable catalog using the canonical revision parser.
    ///
    /// `entry_revisions` must have exactly one source revision per entry, in the same
    /// order. Every revision must match the source set before completeness or message
    /// admission can publish it. Caller-admitted source identity/rights remain caller-owned;
    /// revision spelling and equality do not authenticate an artifact or its issuer.
    /// Work and storage remain bounded by the supplied inventory, as for `load`.
    pub fn load_revision(
        revision: Option<&str>,
        locales: &[LocaleTag],
        declarations: &BTreeMap<TextKey, BTreeMap<String, ArgumentKind>>,
        entries: &[CatalogEntry<'_>],
        entry_revisions: &[RevisionLabel],
    ) -> Result<Self, CatalogLoadError> {
        let revision = RevisionLabel::new(revision).map_err(CatalogLoadError::InvalidRevision)?;
        if entries.len() != entry_revisions.len() {
            return Err(CatalogLoadError::RevisionInventoryMismatch);
        }
        for (entry, entry_revision) in entries.iter().zip(entry_revisions) {
            if entry_revision != &revision {
                return Err(CatalogLoadError::MixedRevision {
                    locale: entry.locale.clone(),
                    key: entry.key.clone(),
                });
            }
        }
        Self::load(revision, locales, declarations, entries)
    }

    /// Replaces revision, labels and declarations together only after full admission.
    /// Missing/invalid/mixed source revisions and all catalog refusals preserve the old set.
    pub fn reload_revision(
        &mut self,
        revision: Option<&str>,
        locales: &[LocaleTag],
        declarations: &BTreeMap<TextKey, BTreeMap<String, ArgumentKind>>,
        entries: &[CatalogEntry<'_>],
        entry_revisions: &[RevisionLabel],
    ) -> Result<(), CatalogLoadError> {
        let replacement =
            Self::load_revision(revision, locales, declarations, entries, entry_revisions)?;
        *self = replacement;
        Ok(())
    }
}

impl<V> VersionedCatalog<V> {
    /// Admits exactly one translation for each required locale/key pair.
    ///
    /// The finite caller-supplied inventory must contain at least one locale and key.
    /// No fallback can satisfy a missing required translation. Every message must
    /// match the source declaration through the same admission as `Catalog`.
    /// Rejections publish no version or partially loaded catalog.
    pub fn load(
        version: V,
        locales: &[LocaleTag],
        declarations: &BTreeMap<TextKey, BTreeMap<String, ArgumentKind>>,
        entries: &[CatalogEntry<'_>],
    ) -> Result<Self, CatalogLoadError> {
        if locales.is_empty() || declarations.is_empty() {
            return Err(CatalogLoadError::EmptyManifest);
        }
        let mut required_locales = BTreeSet::new();
        for locale in locales {
            if !required_locales.insert(locale.as_str()) {
                return Err(CatalogLoadError::DuplicateLocale {
                    locale: locale.clone(),
                });
            }
        }
        let mut catalog = Catalog::default();
        for (key, slots) in declarations {
            catalog.declare_slots(key.clone(), slots).map_err(|error| {
                CatalogLoadError::InvalidDeclaration {
                    key: key.clone(),
                    error,
                }
            })?;
        }
        let mut admitted_pairs = BTreeSet::new();
        for entry in entries {
            if !required_locales.contains(entry.locale.as_str()) {
                return Err(CatalogLoadError::UnexpectedLocale {
                    locale: entry.locale.clone(),
                });
            }
            if !declarations.contains_key(entry.key) {
                return Err(CatalogLoadError::UnexpectedKey {
                    key: entry.key.clone(),
                });
            }
            if !admitted_pairs.insert((entry.locale.as_str(), entry.key)) {
                return Err(CatalogLoadError::DuplicateEntry {
                    locale: entry.locale.clone(),
                    key: entry.key.clone(),
                });
            }
            catalog.insert(entry.locale, entry.key.clone(), entry.text);
            catalog
                .insert_message(entry.locale, entry.key, entry.parts)
                .map_err(|error| CatalogLoadError::InvalidMessage {
                    locale: entry.locale.clone(),
                    key: entry.key.clone(),
                    error,
                })?;
        }
        for locale in locales {
            for key in declarations.keys() {
                if !admitted_pairs.contains(&(locale.as_str(), key)) {
                    return Err(CatalogLoadError::MissingEntry {
                        locale: locale.clone(),
                        key: key.clone(),
                    });
                }
            }
        }
        Ok(Self { version, catalog })
    }

    /// Replaces the complete version and catalog only after all loading checks pass.
    /// Any refusal leaves existing labels, messages, declarations, and revision intact.
    pub fn reload(
        &mut self,
        version: V,
        locales: &[LocaleTag],
        declarations: &BTreeMap<TextKey, BTreeMap<String, ArgumentKind>>,
        entries: &[CatalogEntry<'_>],
    ) -> Result<(), CatalogLoadError> {
        let replacement = Self::load(version, locales, declarations, entries)?;
        *self = replacement;
        Ok(())
    }

    /// Returns the opaque revision supplied with this admitted snapshot.
    pub fn version(&self) -> &V {
        &self.version
    }

    /// Resolves the existing catalog lookup with its supplying snapshot revision.
    pub fn lookup<'a>(&'a self, key: &TextKey, chain: &[LocaleTag]) -> VersionedLookup<'a, V> {
        VersionedLookup {
            version: &self.version,
            lookup: self.catalog.lookup(key, chain),
        }
    }

    /// Formats through the existing catalog, keeping text plain and values exact.
    pub fn format<'a>(
        &'a self,
        key: &TextKey,
        chain: &[LocaleTag],
        args: &BTreeMap<String, ArgumentValue>,
    ) -> Result<VersionedFormattedMessage<'a, V>, FormatError> {
        Ok(VersionedFormattedMessage {
            version: &self.version,
            message: self.catalog.format(key, chain, args)?,
        })
    }
}
