//! Dated supplier route evidence for native provider adapters.
//!
//! Published rates and model pages are research evidence. They do not establish
//! account access, current capacity, rights, or permission to dispatch provider work.

mod http_contract;
mod reconciliation;
mod registry;

pub use http_contract::{
    ElevenLabsAudioObservation, FalQueueObservation, FalSubmitObservation, HttpMethod, HttpRequest,
    HttpRequestError, ProviderImageMetadata, ProviderRequestId, classify_fal_status,
    classify_fal_status_response, classify_fal_submit_response, elevenlabs_stream_request,
    fal_result_request, fal_status_request, fal_submit_request, validate_elevenlabs_audio,
    validate_fal_images,
};
pub use reconciliation::{
    ReconciliationCapability, ReconciliationInput, ReconciliationLookup,
    reconcile_provider_attempt, reconciliation_capability,
};
pub use registry::{
    BillingUnit, CandidateRoute, CandidateRouteId, Capability, DispatchBlock, EvidenceReference,
    Gate, GateStatus, LicenseEvidence, PublishedRate, RoundingRule, UnsupportedCapability,
    assess_dispatch_gates, candidate_for, candidate_routes,
};
