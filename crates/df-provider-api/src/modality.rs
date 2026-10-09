//! Consumer-owned modality metadata around the common checked request boundary.
//!
//! Modality names do not define payload schemas, transport headers, provider routes,
//! billing meters, rights qualification, or permission to dispatch. Source and rights
//! revisions are equality fences supplied by the owning consumer; their presence does
//! not establish source rights, authorization, quote approval, or egress permission.

use std::fmt;

use df_model::checkpoint::ContentDigest;
use df_types::RevisionLabel;

use crate::{
    CheckedRequest, RequestBinding, RequestError, RequestLimits, RequestOwnerState, RequestUsage,
};

/// One of the six provider-work families currently named by the subsystem plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestModality {
    Text,
    Stt,
    Tts,
    Image,
    Video,
    Sound,
}

/// Explicit source, opaque rights/version marker, output format, and caller context.
///
/// Equality participates in the existing request's current-binding check. The opaque
/// markers are not themselves rights evidence, access credentials, or format schemas.
#[derive(Clone, Eq, PartialEq)]
pub struct RequestBasis<Semantic> {
    pub modality: RequestModality,
    pub source: ContentDigest,
    pub rights_revision: RevisionLabel,
    pub output_format_revision: RevisionLabel,
    pub semantic: Semantic,
}

impl<Semantic> fmt::Debug for RequestBasis<Semantic> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RequestBasis")
            .finish_non_exhaustive()
    }
}

/// A common checked request tagged with its consumer-selected modality basis.
///
/// Construction and current validation use the existing `CheckedRequest` policy for
/// job identity, execution mode, deadline, payload bytes, declared usage, and limits.
/// This type does not qualify modality payloads or initiate provider work.
pub struct CheckedModalityRequest<Semantic> {
    checked: CheckedRequest<RequestBasis<Semantic>>,
}

impl<Semantic> fmt::Debug for CheckedModalityRequest<Semantic> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CheckedModalityRequest")
            .finish_non_exhaustive()
    }
}

impl<Semantic: Eq> CheckedModalityRequest<Semantic> {
    /// Constructs the envelope through the canonical checked request boundary.
    pub fn new(
        binding: RequestBinding<RequestBasis<Semantic>>,
        payload: &[u8],
        usage: RequestUsage,
        limits: RequestLimits,
        owner: RequestOwnerState<'_, RequestBasis<Semantic>>,
    ) -> Result<Self, RequestError> {
        let checked = CheckedRequest::new(binding, payload, usage, limits, owner)?;
        Ok(Self { checked })
    }

    /// Rechecks the unchanged canonical binding and the consumer's current basis.
    pub fn validate_current(
        &self,
        owner: RequestOwnerState<'_, RequestBasis<Semantic>>,
    ) -> Result<(), RequestError> {
        self.checked.validate_current(owner)
    }
}

impl<Semantic> CheckedModalityRequest<Semantic> {
    pub fn binding(&self) -> &RequestBinding<RequestBasis<Semantic>> {
        self.checked.binding()
    }

    /// Returns the unchanged payload bytes for an authorized consumer.
    pub fn payload(&self) -> &[u8] {
        self.checked.payload()
    }

    pub fn usage(&self) -> RequestUsage {
        self.checked.usage()
    }

    pub fn limits(&self) -> RequestLimits {
        self.checked.limits()
    }

    pub fn into_parts(
        self,
    ) -> (
        RequestBinding<RequestBasis<Semantic>>,
        Vec<u8>,
        RequestUsage,
    ) {
        self.checked.into_parts()
    }
}
