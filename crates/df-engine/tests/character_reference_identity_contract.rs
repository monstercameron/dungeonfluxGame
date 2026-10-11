#[expect(
    dead_code,
    reason = "This contract uses state constructors; checkpoint and accepted are shared fixture helpers for other targets."
)]
#[path = "support/fixture_model.rs"]
mod fixture_model;

#[path = "character_reference.rs"]
mod reference_identity_cases;
