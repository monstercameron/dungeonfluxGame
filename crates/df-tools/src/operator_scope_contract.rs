//! Private, test-mounted policy example; no production authorization issuer.
//! A modeled server role is not a credential or method/tenant capability proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Operation {
    DebugInspect,
    TelemetryQuery,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ServerRole {
    Operator,
    Player,
    Unauthenticated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ResultKind {
    ForwardToOwningService,
    PermissionDenied,
}

pub(super) fn server_admit(operation: Operation, authenticated_role: ServerRole) -> ResultKind {
    match (operation, authenticated_role) {
        (Operation::DebugInspect | Operation::TelemetryQuery, ServerRole::Operator) => {
            ResultKind::ForwardToOwningService
        }
        (Operation::DebugInspect | Operation::TelemetryQuery, _) => ResultKind::PermissionDenied,
    }
}
