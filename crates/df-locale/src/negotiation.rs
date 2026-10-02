use df_types::LocaleTag;
use std::fmt;

/// An owned selected locale and the facts needed to disclose preference fallback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocaleSelection {
    selected: LocaleTag,
    used_fallback: bool,
}

impl LocaleSelection {
    /// Returns the exact supported locale selected for this output.
    pub fn selected(&self) -> &LocaleTag {
        &self.selected
    }

    /// Returns false only when the first requested preference was selected.
    ///
    /// Later requested alternatives and the default, including an empty request list,
    /// are disclosed as fallback.
    pub fn used_fallback(&self) -> bool {
        self.used_fallback
    }
}

/// Input-free facts about a configuration that cannot select a locale.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocaleConfigurationError {
    /// The configured default is not an exact member of the supported locales.
    UnsupportedDefault,
}

impl fmt::Display for LocaleConfigurationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedDefault => {
                formatter.write_str("configured default locale is unsupported")
            }
        }
    }
}

impl std::error::Error for LocaleConfigurationError {}

/// Selects the first exact supported request in the caller's precedence order.
///
/// Validates the default before considering requests; an unsupported default returns
/// [`LocaleConfigurationError::UnsupportedDefault`] even when a request is supported.
/// If no request matches, selects the supported default and discloses fallback.
/// Duplicate tags do not change request priority. Inputs are borrowed and unchanged;
/// only the selected, already bounded `LocaleTag` is cloned into the result.
///
/// The caller admits usable catalogs for this output. This function infers no language
/// prefixes, aliases, scripts, provider capabilities, or ambient device preferences.
pub fn settle(
    requested: &[LocaleTag],
    supported: &[LocaleTag],
    default: &LocaleTag,
) -> Result<LocaleSelection, LocaleConfigurationError> {
    if !supported.contains(default) {
        return Err(LocaleConfigurationError::UnsupportedDefault);
    }

    for (index, candidate) in requested.iter().enumerate() {
        if supported.contains(candidate) {
            return Ok(LocaleSelection {
                selected: candidate.clone(),
                used_fallback: index != 0,
            });
        }
    }

    Ok(LocaleSelection {
        selected: default.clone(),
        used_fallback: true,
    })
}
