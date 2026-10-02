use crate::format::{message_slots, substitute_parts, validate_arguments, validate_slot_names};
use crate::{
    ArgumentKind, ArgumentValue, FormatError, FormattedMessage, FormattedPart, MessageError,
    MessagePart, TextKey,
};
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
    entries: BTreeMap<(String, TextKey), Entry>,
    declarations: BTreeMap<TextKey, BTreeMap<String, ArgumentKind>>,
}

struct Entry {
    text: String,
    message: Option<Vec<MessagePart>>,
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
    /// separate boundaries owned by their callers. Replacement clears any attached
    /// structured message for this exact pair; it does not change the key's declaration.
    pub fn insert(&mut self, locale: &LocaleTag, key: TextKey, text: &str) {
        self.entries.insert(
            (locale.as_str().to_owned(), key),
            Entry {
                text: text.to_owned(),
                message: None,
            },
        );
    }

    /// Admits the immutable source slot kinds for a key, independently of locale.
    ///
    /// Names use one `TextKey` component. An identical declaration is idempotent;
    /// invalid names or a changed declaration reject without mutation.
    pub fn declare_slots(
        &mut self,
        key: TextKey,
        slots: &BTreeMap<String, ArgumentKind>,
    ) -> Result<(), MessageError> {
        validate_slot_names(slots)?;
        if let Some(current) = self.declarations.get(&key) {
            return if current == slots {
                Ok(())
            } else {
                Err(MessageError::DeclarationMismatch)
            };
        }
        self.declarations.insert(key, slots.clone());
        Ok(())
    }

    /// Attaches owned semantic parts to an existing exact locale/key entry.
    ///
    /// The unique argument names and kinds must equal the source declaration; absent
    /// declaration means no slots. Reordering and repeating the same kind are allowed.
    /// Any rejection preserves the entry's label and previous validated parts.
    pub fn insert_message(
        &mut self,
        locale: &LocaleTag,
        key: &TextKey,
        parts: &[MessagePart],
    ) -> Result<(), MessageError> {
        let pair = (locale.as_str().to_owned(), key.clone());
        if !self.entries.contains_key(&pair) {
            return Err(MessageError::MissingEntry);
        }
        let slots = message_slots(parts)?;
        let empty = BTreeMap::new();
        if &slots != self.declarations.get(key).unwrap_or(&empty) {
            return Err(MessageError::SlotMismatch);
        }
        let entry = self
            .entries
            .get_mut(&pair)
            .ok_or(MessageError::MissingEntry)?;
        entry.message = Some(parts.to_vec());
        Ok(())
    }

    /// Formats the same first hit as `lookup`, retaining its key and search diagnostics.
    ///
    /// Extra, missing, then wrong-kind arguments reject before producing any parts.
    /// Raw entries require an empty source declaration. Literal and inserted text are
    /// plain data requiring sink encoding; neither is markup or an executable command.
    /// Quantity units and money micro-units/currencies remain exact canonical values.
    pub fn format(
        &self,
        key: &TextKey,
        chain: &[LocaleTag],
        args: &BTreeMap<String, ArgumentValue>,
    ) -> Result<FormattedMessage, FormatError> {
        let (key, source_locale, searched_locales) = match self.lookup(key, chain) {
            Lookup::Found {
                key,
                source_locale,
                searched_locales,
                ..
            } => (key, source_locale, searched_locales),
            Lookup::Missing {
                key,
                searched_locales,
            } => {
                return Err(FormatError::MissingKey {
                    key,
                    searched_locales,
                });
            }
        };
        let entry = self
            .entries
            .get(&(source_locale.as_str().to_owned(), key.clone()))
            .ok_or(FormatError::UnvalidatedMessage)?;
        let empty = BTreeMap::new();
        let declaration = self.declarations.get(&key).unwrap_or(&empty);
        // A declaration may be admitted after a literal-only message was attached.
        // Recheck that earlier attachment instead of treating it as new authority.
        if let Some(parts) = &entry.message {
            if message_slots(parts).map_err(|_| FormatError::UnvalidatedMessage)? != *declaration {
                return Err(FormatError::UnvalidatedMessage);
            }
        } else if !declaration.is_empty() {
            return Err(FormatError::UnvalidatedMessage);
        }
        validate_arguments(declaration, args)?;
        let parts = match &entry.message {
            Some(parts) => substitute_parts(parts, args)?,
            None => vec![FormattedPart::Literal(entry.text.clone())],
        };
        Ok(FormattedMessage {
            key,
            source_locale,
            searched_locales,
            parts,
        })
    }

    /// Searches the same key in caller-supplied chain order until the first hit.
    ///
    /// A hit retains the searched prefix and supplying locale; a miss retains the full
    /// chain, including an empty chain. The requested key is preserved in either outcome.
    pub fn lookup<'a>(&'a self, key: &TextKey, chain: &[LocaleTag]) -> Lookup<'a> {
        let mut searched_locales = Vec::new();
        for locale in chain {
            searched_locales.push(locale.clone());
            if let Some(entry) = self.entries.get(&(locale.as_str().to_owned(), key.clone())) {
                return Lookup::Found {
                    key: key.clone(),
                    text: entry.text.as_str(),
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
