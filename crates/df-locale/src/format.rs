use crate::TextKey;
use df_types::{Currency, LocaleTag, Money, Usage, UsageUnit};
use std::collections::BTreeMap;

/// A source-declared slot kind, including its exact unit or currency discriminator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArgumentKind {
    Text,
    Quantity(UsageUnit),
    Money(Currency),
}

/// Runtime data; text is never interpreted as a slot, key, markup, or command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArgumentValue {
    Text(String),
    Quantity(Usage),
    Money(Money),
}

/// Structured catalog data, without a string template or executable language.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MessagePart {
    Literal(String),
    Argument { name: String, kind: ArgumentKind },
}

/// Semantic output; both literal and inserted text need encoding at the output sink.
/// Numeric parts preserve exact quantities, units, money micro-units, and currency tags.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormattedPart {
    Literal(String),
    Text(String),
    Quantity(Usage),
    Money(Money),
}

/// A formatted catalog hit with the original key and actual lookup diagnostics.
#[derive(Debug, Eq, PartialEq)]
pub struct FormattedMessage {
    pub key: TextKey,
    pub source_locale: LocaleTag,
    pub searched_locales: Vec<LocaleTag>,
    pub parts: Vec<FormattedPart>,
}

impl FormattedMessage {
    /// Renders semantic parts as plain data for native labels or a text-only DOM sink.
    ///
    /// Literals and inserted text are copied verbatim, without interpreting templates
    /// or markup. Quantities use integer decimal and an explicit canonical unit tag;
    /// money uses its currency tag and exactly six fractional digits. These canonical
    /// forms preserve all values and do not claim locale-specific number typography.
    /// The caller must encode the returned string for its output sink.
    pub fn plain_text(&self) -> String {
        let mut text = String::new();
        for part in &self.parts {
            match part {
                FormattedPart::Literal(value) | FormattedPart::Text(value) => text.push_str(value),
                FormattedPart::Quantity(value) => {
                    let unit = match value.unit() {
                        UsageUnit::Token => "token",
                        UsageUnit::Character => "character",
                        UsageUnit::Byte => "byte",
                        UsageUnit::AudioMillisecond => "audio_ms",
                        UsageUnit::VideoMillisecond => "video_ms",
                        UsageUnit::Image => "image",
                    };
                    text.push_str(&format!("{} {unit}", value.quantity()));
                }
                FormattedPart::Money(value) => {
                    text.push_str(&format!(
                        "{} {}.{:06}",
                        value.currency().as_str(),
                        value.micros() / 1_000_000,
                        value.micros() % 1_000_000,
                    ));
                }
            }
        }
        text
    }
}

/// Input-free rejection facts from source declaration or message admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageError {
    InvalidArgumentName,
    DeclarationMismatch,
    MissingEntry,
    SlotMismatch,
}

/// Formatting refusals; names identify slots, never include supplied argument values.
#[derive(Debug, Eq, PartialEq)]
pub enum FormatError {
    MissingKey {
        key: TextKey,
        searched_locales: Vec<LocaleTag>,
    },
    MissingArgument {
        name: String,
    },
    ExtraArgument {
        name: String,
    },
    WrongKind {
        name: String,
    },
    UnvalidatedMessage,
}

fn validate_name(name: &str) -> Result<(), MessageError> {
    if !name.contains('.') && TextKey::parse(name).is_ok() {
        Ok(())
    } else {
        Err(MessageError::InvalidArgumentName)
    }
}

pub(crate) fn validate_slot_names(
    slots: &BTreeMap<String, ArgumentKind>,
) -> Result<(), MessageError> {
    for name in slots.keys() {
        validate_name(name)?;
    }
    Ok(())
}

pub(crate) fn message_slots(
    parts: &[MessagePart],
) -> Result<BTreeMap<String, ArgumentKind>, MessageError> {
    let mut slots = BTreeMap::new();
    for part in parts {
        if let MessagePart::Argument { name, kind } = part {
            validate_name(name)?;
            if let Some(previous) = slots.insert(name.clone(), *kind)
                && previous != *kind
            {
                return Err(MessageError::SlotMismatch);
            }
        }
    }
    Ok(slots)
}

pub(crate) fn validate_arguments(
    slots: &BTreeMap<String, ArgumentKind>,
    args: &BTreeMap<String, ArgumentValue>,
) -> Result<(), FormatError> {
    for name in args.keys() {
        if !slots.contains_key(name) {
            return Err(FormatError::ExtraArgument { name: name.clone() });
        }
    }
    for name in slots.keys() {
        if !args.contains_key(name) {
            return Err(FormatError::MissingArgument { name: name.clone() });
        }
    }
    for (name, kind) in slots {
        let value = args
            .get(name)
            .ok_or_else(|| FormatError::MissingArgument { name: name.clone() })?;
        let matches = match (kind, value) {
            (ArgumentKind::Text, ArgumentValue::Text(_)) => true,
            (ArgumentKind::Quantity(unit), ArgumentValue::Quantity(value)) => *unit == value.unit(),
            (ArgumentKind::Money(currency), ArgumentValue::Money(value)) => {
                *currency == value.currency()
            }
            _ => false,
        };
        if !matches {
            return Err(FormatError::WrongKind { name: name.clone() });
        }
    }
    Ok(())
}

pub(crate) fn substitute_parts(
    parts: &[MessagePart],
    args: &BTreeMap<String, ArgumentValue>,
) -> Result<Vec<FormattedPart>, FormatError> {
    parts
        .iter()
        .map(|part| match part {
            MessagePart::Literal(text) => Ok(FormattedPart::Literal(text.clone())),
            MessagePart::Argument { name, .. } => match args.get(name) {
                Some(ArgumentValue::Text(text)) => Ok(FormattedPart::Text(text.clone())),
                Some(ArgumentValue::Quantity(value)) => Ok(FormattedPart::Quantity(*value)),
                Some(ArgumentValue::Money(value)) => Ok(FormattedPart::Money(*value)),
                None => Err(FormatError::MissingArgument { name: name.clone() }),
            },
        })
        .collect()
}
