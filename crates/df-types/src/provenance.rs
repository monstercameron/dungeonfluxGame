/// Invalid supplied nonsecret revision label. No error retains its input text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevisionLabelError {
    Missing,
    Empty,
    TooLong { actual: usize },
    InvalidCharacter { byte_index: usize },
}

/// A 1..=128-byte ASCII revision label, never a credential or approval assertion.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RevisionLabel(String);

impl RevisionLabel {
    /// Accepts letters, digits, and `. _ - : /` only. Callers must supply nonsecret labels.
    pub fn new(value: Option<&str>) -> Result<Self, RevisionLabelError> {
        let value = value.ok_or(RevisionLabelError::Missing)?;
        if value.is_empty() {
            return Err(RevisionLabelError::Empty);
        }
        if value.len() > 128 {
            return Err(RevisionLabelError::TooLong {
                actual: value.len(),
            });
        }
        for (byte_index, byte) in value.bytes().enumerate() {
            if !byte.is_ascii_alphanumeric() && !matches!(byte, b'.' | b'_' | b'-' | b':' | b'/') {
                return Err(RevisionLabelError::InvalidCharacter { byte_index });
            }
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Retained label allocation for owners enforcing a total memory bound.
    pub fn retained_heap_bytes(&self) -> usize {
        self.0.capacity()
    }
}

/// Required provenance component of the paired native/WASM fixture build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildRevision {
    Source,
    Native,
    Wasm,
    Configuration,
    Content,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildIdentityError {
    pub revision: BuildRevision,
    pub cause: RevisionLabelError,
}

/// Five required bounded provenance labels. These describe supplied revisions only;
/// they do not prove testing, approval, content rights, or catalog validity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildIdentity {
    source: RevisionLabel,
    native: RevisionLabel,
    wasm: RevisionLabel,
    configuration: RevisionLabel,
    content: RevisionLabel,
}

impl BuildIdentity {
    pub fn new(
        source: Option<&str>,
        native: Option<&str>,
        wasm: Option<&str>,
        configuration: Option<&str>,
        content: Option<&str>,
    ) -> Result<Self, BuildIdentityError> {
        fn label(
            revision: BuildRevision,
            value: Option<&str>,
        ) -> Result<RevisionLabel, BuildIdentityError> {
            RevisionLabel::new(value).map_err(|cause| BuildIdentityError { revision, cause })
        }
        Ok(Self {
            source: label(BuildRevision::Source, source)?,
            native: label(BuildRevision::Native, native)?,
            wasm: label(BuildRevision::Wasm, wasm)?,
            configuration: label(BuildRevision::Configuration, configuration)?,
            content: label(BuildRevision::Content, content)?,
        })
    }

    pub fn revision(&self, revision: BuildRevision) -> &RevisionLabel {
        match revision {
            BuildRevision::Source => &self.source,
            BuildRevision::Native => &self.native,
            BuildRevision::Wasm => &self.wasm,
            BuildRevision::Configuration => &self.configuration,
            BuildRevision::Content => &self.content,
        }
    }
}
