//! Pure locale negotiation and checked display-text lookup.
//!
//! Callers supply each output's ordered preferences, supported locales, and default.
//! Parsing belongs to `df_types::LocaleTag`; formatting is a separate boundary.

mod catalog;
mod negotiation;
mod text_key;

pub use catalog::{Catalog, Lookup, lookup_chain};
pub use negotiation::{LocaleConfigurationError, LocaleSelection, settle};
pub use text_key::{TextKey, TextKeyError};
