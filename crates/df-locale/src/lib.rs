//! Pure locale negotiation over caller-admitted usable catalog locales.
//!
//! Callers supply each output's ordered preferences, supported locales, and default.
//! Parsing belongs to `df_types::LocaleTag`; catalog lookup and formatting are separate.

mod negotiation;

pub use negotiation::{LocaleConfigurationError, LocaleSelection, settle};
