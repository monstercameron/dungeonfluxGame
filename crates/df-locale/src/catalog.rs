use crate::TextKey;
use df_types::LocaleTag;
use std::collections::BTreeMap;

/// Forms a selected-first lookup chain with the caller-admitted default second.
///
/// Equal inputs appear once. Locale support and preference selection belong to `settle`;
/// this function does not infer language prefixes or validate catalog admission.
pub fn lookup_chain(selected: &LocaleTag, admitted_default: &LocaleTag) -> Vec<LocaleTag> {
    let mut chain = vec![selected.clone()];
    if selected != admitted_default {
        chain.push(admitted_default.clone());
    }
    chain
}

/// Caller-inserted display text indexed by exact locale spelling and stable key.
#[derive(Default)]
pub struct Catalog {
    entries: BTreeMap<(String, TextKey), String>,
}

/// A catalog hit or explicit missing key with the locales actually searched in order.
#[derive(Debug, Eq, PartialEq)]
pub enum Lookup<'a> {
    /// The first matching entry, borrowing text from the catalog.
    Found {
        key: TextKey,
        text: &'a str,
        source_locale: LocaleTag,
        searched_locales: Vec<LocaleTag>,
    },
    /// No entry matched; retains the requested key and full supplied search chain.
    Missing {
        key: TextKey,
        searched_locales: Vec<LocaleTag>,
    },
}

impl Catalog {
    /// Copies an entry into the catalog, replacing an existing entry for the same pair.
    ///
    /// Empty text is an entry. Loading, formatting, and catalog revision admission are
    /// separate boundaries owned by their callers.
    pub fn insert(&mut self, locale: &LocaleTag, key: TextKey, text: &str) {
        self.entries
            .insert((locale.as_str().to_owned(), key), text.to_owned());
    }

    /// Searches the same key in caller-supplied chain order until the first hit.
    ///
    /// A hit retains the searched prefix and supplying locale; a miss retains the full
    /// chain, including an empty chain. The requested key is preserved in either outcome.
    pub fn lookup<'a>(&'a self, key: &TextKey, chain: &[LocaleTag]) -> Lookup<'a> {
        let mut searched_locales = Vec::new();
        for locale in chain {
            searched_locales.push(locale.clone());
            if let Some(text) = self.entries.get(&(locale.as_str().to_owned(), key.clone())) {
                return Lookup::Found {
                    key: key.clone(),
                    text: text.as_str(),
                    source_locale: locale.clone(),
                    searched_locales,
                };
            }
        }
        Lookup::Missing {
            key: key.clone(),
            searched_locales,
        }
    }
}

impl Lookup<'_> {
    /// Returns the original requested display-text identifier.
    pub fn key(&self) -> &TextKey {
        match self {
            Self::Found { key, .. } | Self::Missing { key, .. } => key,
        }
    }

    /// Returns actual catalog text on a hit, or no label on a miss.
    pub fn label(&self) -> Option<&str> {
        match self {
            Self::Found { text, .. } => Some(text),
            Self::Missing { .. } => None,
        }
    }

    /// Returns the locale that supplied a matching entry.
    pub fn source_locale(&self) -> Option<&LocaleTag> {
        match self {
            Self::Found { source_locale, .. } => Some(source_locale),
            Self::Missing { .. } => None,
        }
    }

    /// Returns the ordered locales visited before the outcome was known.
    pub fn searched_locales(&self) -> &[LocaleTag] {
        match self {
            Self::Found {
                searched_locales, ..
            }
            | Self::Missing {
                searched_locales, ..
            } => searched_locales,
        }
    }
}
