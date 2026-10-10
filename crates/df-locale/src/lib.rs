//! Pure locale negotiation and checked display-text lookup.
//!
//! Callers supply each output's ordered preferences, supported locales, and default.
//! Parsing belongs to `df_types::LocaleTag`; formatting preserves plain text and exact values.
//! Source revision admission uses `VersionedCatalog<df_types::RevisionLabel>::load_revision`.

mod catalog;
mod format;
mod loading;
mod negotiation;
mod text_key;

pub use catalog::{Catalog, Lookup, lookup_chain};
pub use format::{
    ArgumentKind, ArgumentValue, FormatError, FormattedMessage, FormattedPart, MessageError,
    MessagePart,
};
pub use loading::{
    CatalogEntry, CatalogLoadError, VersionedCatalog, VersionedFormattedMessage, VersionedLookup,
};
pub use negotiation::{LocaleConfigurationError, LocaleSelection, settle};
pub use text_key::{TextKey, TextKeyError};
