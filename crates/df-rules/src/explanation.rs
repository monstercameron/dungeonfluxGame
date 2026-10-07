//! Borrowed explanations from the canonical registry's exact compiled source selection.
use df_model::checkpoint::{CheckpointPins, ContentPins, RuleReference, RulesPins};
use df_types::{BuildIdentity, RevisionLabel};

/// The selected source, handler, content and supplied build basis for one registry operation.
///
/// Only the registry constructs this view. It does not qualify a rule, grant rights, establish
/// catalog completeness, or prove that a build/check ran. Rules identity is separate from
/// cosmetic content identity. There is deliberately no Debug or serialization implementation:
/// indexed source bytes may be private and must not enter default diagnostics or public views.
#[derive(Clone, Copy)]
pub struct RuleExplanation<'a> {
    pub(crate) pins: &'a CheckpointPins,
    pub(crate) selector: &'a RevisionLabel,
    pub(crate) source: &'a RuleReference,
    pub(crate) source_bytes: &'a [u8],
}

impl<'a> RuleExplanation<'a> {
    /// Exact selected mechanics mode, catalog/source manifest and compiled handler revisions.
    pub fn rules_pins(&self) -> &'a RulesPins {
        &self.pins.rules
    }

    /// Separate authored/cosmetic package basis; changing it cannot select another mechanic.
    pub fn content_pins(&self) -> &'a ContentPins {
        &self.pins.content
    }

    /// Supplied canonical labels only, never evidence that native/WASM checks passed.
    pub fn build(&self) -> &'a BuildIdentity {
        &self.pins.build
    }

    /// The admitted registration's exact clause locator, rather than caller-provided prose.
    pub fn source(&self) -> &'a RuleReference {
        self.source
    }

    /// The selected compiled registration's selector; catalog text cannot mint this value.
    pub fn selector(&self) -> &'a RevisionLabel {
        self.selector
    }

    /// Exact owner-supplied indexed bytes for the selected clause, borrowed without decoding.
    /// A native source owner may inspect/hash these separately from cosmetic bytes. This view
    /// neither verifies their digest nor authorizes their disclosure, delivery or execution.
    pub fn source_bytes(&self) -> &'a [u8] {
        self.source_bytes
    }
}
