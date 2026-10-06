//! Checked provider input boundaries. Construction neither dispatches nor authorizes work.
//! Modality schemas, current access, spend admission and provider readiness have separate owners.

mod request;

pub use request::{
    CheckedRequest, InvalidRequestLimits, RequestBinding, RequestError, RequestIdentity,
    RequestIdentityField, RequestLimit, RequestLimits, RequestOwnerState, RequestUsage,
};
