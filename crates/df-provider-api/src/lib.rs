//! Checked provider input boundaries. Construction neither dispatches nor authorizes work.
//! Modality schemas, current access, spend admission and provider readiness have separate owners.

mod budget;
mod modality;
mod outcome;
mod request;

pub use budget::{BudgetAdmission, BudgetMutation, BudgetStore};

pub use modality::{CheckedModalityRequest, RequestBasis, RequestModality};

pub use outcome::{
    ProviderAttemptObservation, ProviderBillingClass, ProviderFailureClass,
    ProviderLiabilityDisposition, ProviderNextAction, ProviderOutcomeDecision,
    ProviderOutcomeError, ProviderResultClass, ProviderRetryPolicy, classify_provider_outcome,
};

pub use request::{
    CheckedRequest, InvalidRequestLimits, RequestBinding, RequestError, RequestIdentity,
    RequestIdentityField, RequestLimit, RequestLimits, RequestOwnerState, RequestUsage,
};
