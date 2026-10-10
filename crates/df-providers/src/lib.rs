//! Dated supplier route evidence for native provider adapters.
//!
//! Published rates and model pages are research evidence. They do not establish
//! account access, current capacity, rights, or permission to dispatch provider work.

mod registry;

pub use registry::{
    BillingUnit, CandidateRoute, CandidateRouteId, Capability, DispatchBlock, EvidenceReference,
    Gate, GateStatus, LicenseEvidence, PublishedRate, RoundingRule, UnsupportedCapability,
    assess_dispatch_gates, candidate_for, candidate_routes,
};
