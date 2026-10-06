//! Bounded, pure commercial policy candidates using caller-supplied exact values.
//!
//! These functions do not verify payment observations, mint grants, reserve money,
//! authenticate tenants, commit a ledger, or authorize dispatch. Native owners must
//! bind the inputs to current trusted records and commit candidates atomically with
//! uniqueness, ledger, grant, accepted-intent and protected recovery-journal changes.
//! Subscription and spend inputs have separate revisions and policy assumptions;
//! neither candidate alone constitutes durable paid-work admission.

mod spend;
mod subscription;

pub use spend::{
    SpendConsent, SpendCounter, SpendOperationObservation, SpendOperationStatus, SpendProposal,
    SpendRefusal, SpendRequest, SpendScope, SpendSnapshot, propose_spend,
};
pub use subscription::{
    Access, ObservationAuthority, PaidInvoice, PaymentOutcome, SubscriptionObservation,
    SubscriptionRefusal, SubscriptionState, SubscriptionTransition, effective_access,
    observe_subscription,
};
