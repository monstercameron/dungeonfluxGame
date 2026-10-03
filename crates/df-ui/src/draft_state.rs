use std::fmt;

/// Presentation storage bound, in UTF-16 code units as used by native maxlength.
/// This cap does not decide whether a server-authorized action is legal.
pub const MAX_DRAFT_UTF16_UNITS: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DraftError {
    TooLong,
    Unavailable,
    Disposed,
}

impl fmt::Display for DraftError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TooLong => "draft exceeds presentation length limit",
            Self::Unavailable => "control is unavailable",
            Self::Disposed => "control is disposed",
        })
    }
}

impl std::error::Error for DraftError {}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) struct DraftState {
    value: String,
    enabled: bool,
    pending: bool,
    disposed: bool,
}

#[cfg(any(target_arch = "wasm32", test))]
impl DraftState {
    pub(crate) fn create(value: &str) -> Result<Self, DraftError> {
        validate_value(value)?;
        Ok(Self {
            value: value.to_owned(),
            enabled: true,
            pending: false,
            disposed: false,
        })
    }

    pub(crate) fn value(&self) -> &str {
        &self.value
    }

    pub(crate) fn is_disposed(&self) -> bool {
        self.disposed
    }

    pub(crate) fn reconcile(
        &mut self,
        enabled: bool,
        pending: bool,
        replacement: Option<&str>,
    ) -> Result<(), DraftError> {
        self.ensure_live()?;
        if let Some(value) = replacement {
            validate_value(value)?;
            self.value.clear();
            self.value.push_str(value);
        }
        self.enabled = enabled;
        self.pending = pending;
        Ok(())
    }

    pub(crate) fn change(&mut self, value: &str) -> Result<bool, DraftError> {
        self.ensure_live()?;
        if !self.enabled || self.pending {
            return Err(DraftError::Unavailable);
        }
        validate_value(value)?;
        if self.value == value {
            return Ok(false);
        }
        self.value.clear();
        self.value.push_str(value);
        Ok(true)
    }

    pub(crate) fn dispose(&mut self) {
        self.disposed = true;
        self.value.clear();
        self.value.shrink_to_fit();
    }

    fn ensure_live(&self) -> Result<(), DraftError> {
        if self.disposed {
            return Err(DraftError::Disposed);
        }
        Ok(())
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn validate_value(value: &str) -> Result<(), DraftError> {
    if value.encode_utf16().take(MAX_DRAFT_UTF16_UNITS + 1).count() > MAX_DRAFT_UTF16_UNITS {
        return Err(DraftError::TooLong);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{DraftError, DraftState, MAX_DRAFT_UTF16_UNITS};

    #[test]
    fn length_limit_matches_native_utf16_units() {
        let at_limit = "😀".repeat(MAX_DRAFT_UTF16_UNITS / 2);
        assert!(DraftState::create(&at_limit).is_ok());
        let over_limit = format!("{at_limit}x");
        assert!(matches!(
            DraftState::create(&over_limit),
            Err(DraftError::TooLong)
        ));
    }

    #[test]
    fn preserved_updates_and_rejection_keep_local_edits() {
        let mut state = DraftState::create("initial").expect("valid fixture");
        assert_eq!(state.change("local draft"), Ok(true));
        for _ in 0..100 {
            state.reconcile(true, false, None).expect("live fixture");
        }
        state.reconcile(true, true, None).expect("pending fixture");
        assert_eq!(state.change("late input"), Err(DraftError::Unavailable));
        assert_eq!(state.value(), "local draft");
        state
            .reconcile(true, false, None)
            .expect("rejected fixture");
        assert_eq!(state.change("corrected draft"), Ok(true));
        assert_eq!(state.value(), "corrected draft");
    }

    #[test]
    fn invalid_replacement_leaves_prior_draft_and_editability_unchanged() {
        let mut state = DraftState::create("retained").expect("valid fixture");
        let excessive = "x".repeat(MAX_DRAFT_UTF16_UNITS + 1);
        assert_eq!(
            state.reconcile(false, true, Some(&excessive)),
            Err(DraftError::TooLong)
        );
        assert_eq!(state.value(), "retained");
        assert_eq!(state.change("still editable"), Ok(true));
    }

    #[test]
    fn ownership_invalidation_clears_draft_and_disposal_is_terminal() {
        let mut state = DraftState::create("obsolete private draft").expect("valid fixture");
        state.reconcile(true, false, Some("")).expect("replacement");
        assert_eq!(state.value(), "");
        state.dispose();
        state.dispose();
        assert!(state.is_disposed());
        assert_eq!(state.value(), "");
        assert_eq!(state.change("revive"), Err(DraftError::Disposed));
        assert_eq!(
            state.reconcile(true, false, Some("revive")),
            Err(DraftError::Disposed)
        );
    }
}
