#[path = "../src/operator_scope_contract.rs"]
mod operator_scope_contract;

use operator_scope_contract::{Operation, ResultKind, ServerRole, server_admit};

// Preserve the original literal assertions without changing their expected outcomes.
#[test]
fn retained_operator_player_and_unauthenticated_cases() {
    assert_eq!(
        server_admit(Operation::DebugInspect, ServerRole::Operator),
        ResultKind::ForwardToOwningService,
    );
    assert_eq!(
        server_admit(Operation::TelemetryQuery, ServerRole::Player),
        ResultKind::PermissionDenied,
    );
    assert_eq!(
        server_admit(Operation::DebugInspect, ServerRole::Unauthenticated),
        ResultKind::PermissionDenied,
    );
}

#[test]
fn complementary_operation_requests_cannot_grant_an_operator_role() {
    for (operation, role, expected) in [
        (
            Operation::TelemetryQuery,
            ServerRole::Operator,
            ResultKind::ForwardToOwningService,
        ),
        (
            Operation::DebugInspect,
            ServerRole::Player,
            ResultKind::PermissionDenied,
        ),
        (
            Operation::TelemetryQuery,
            ServerRole::Unauthenticated,
            ResultKind::PermissionDenied,
        ),
    ] {
        assert_eq!(server_admit(operation, role), expected);
    }
}
