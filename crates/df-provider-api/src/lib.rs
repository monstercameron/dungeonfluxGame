//! Checked provider input boundaries. Construction neither dispatches nor authorizes work.
//! Modality schemas, current access, spend admission and provider readiness have separate owners.

mod budget;
mod modality;
mod outcome;
mod provider;
mod recording;
mod request;

pub use budget::{BudgetAdmission, BudgetMutation, BudgetStore};

pub use modality::{
    CheckedModalityRequest, ImageRequest, ImageSemantics, ModalityRequestError, RequestBasis,
    RequestModality, SoundRequest, SoundSemantics, SttRequest, SttSemantics, TextRequest,
    TextSemantics, TtsRequest, TtsSemantics, VideoRequest, VideoSemantics,
};

pub use outcome::{
    ProviderAttemptObservation, ProviderBillingClass, ProviderFailureClass,
    ProviderLiabilityDisposition, ProviderNextAction, ProviderOutcomeDecision,
    ProviderOutcomeError, ProviderResultClass, ProviderRetryPolicy, classify_provider_outcome,
};

pub use provider::{
    ImageEvent, ImageEventKind, InvalidProviderProgress, ProviderCloseReason, ProviderEvent,
    ProviderProgress, ProviderRequestId, ProviderStreamLease, ProviderStreamOwner, SoundEvent,
    SoundEventKind, SttEvent, SttEventKind, TextEvent, TextEventKind, TtsEvent, TtsEventKind,
    VideoEvent, VideoEventKind,
};

pub use recording::{
    RecordingArtifact, RecordingError, RecordingManifest, RecordingPage, RecordingPageError,
    RecordingStore,
};

pub use request::{
    CheckedRequest, InvalidRequestLimits, RequestBinding, RequestError, RequestIdentity,
    RequestIdentityField, RequestLimit, RequestLimits, RequestOwnerState, RequestUsage,
};
