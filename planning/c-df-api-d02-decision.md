# Command receipts and RPC transport status

Date: 2026-10-01

Task/attempt: `B-C-df-api-D02/a1`

Input revision: `3676e59e6316d9e4d77eab1f9dd812776041c2c9eb70f5dc2e396a13e50bf6b`

Status: source-backed API decision; production types, schema, runtime and integration remain gated

## Decision

Treat a command's **receipt** and the RPC's **transport/framework status** as two independent facts. A successfully completed RPC can carry a typed accepted or rejected `DecisionReceipt`; a transport status only reports what happened to the RPC exchange. Neither `OK` nor an error status proves whether a command committed. The existing `DecisionReceipt` and `OperationLookup` shapes in [RPC API](rpc-api.md) already establish this boundary and are the canonical contract; this decision adds no production type, wire field, enum allocation or second authority.

For a client presenting its current knowledge of one stable operation ID, use four explicit cases:

| Client knowledge | Evidence | Required behavior |
| --- | --- | --- |
| `Accepted` | A committed accepted `DecisionReceipt` or a lookup returning that receipt | Show the confirmed decision. Any returned pending job/resolution remains separately pending; acceptance does not mean its effect completed. |
| `Pending` | `GetOperation` explicitly reports `in_progress` | Keep the same operation ID and fingerprint; wait or query that operation again. Do not issue a replacement mutation. |
| `Rejected` | A committed receipt carries a closed, applicable typed domain rejection | Apply that rejection's documented correction. A corrected intent is a new operation under its method's policy. |
| `Unknown` | The RPC failed after possible submission, lookup says `not_recorded`, or lookup says `expired/indeterminate` | Preserve uncertainty, retain the same operation identity, resync/query or offer explicit resolution. Never infer success/failure or automatically replay under a new key. |

`Pending` is a confirmed lookup state, not an RPC status and not proof of a durable accepted decision. In particular, a durable accepted game receipt may itself identify pending session-owned jobs; those jobs do not make the command receipt pending. `Unknown` is epistemic uncertainty: it includes an ambiguous disconnect/deadline/cancellation, and the lookup's `not_recorded`/expired states where absence cannot authorize a fresh execution. A transport failure known to happen before submission may be retried only where the method contract proves no mutation could have been admitted; this task does not define such a transport phase guarantee.

The server/session owner decides and durably commits game mutations. `df-api` authenticates and maps requests, returns/project receipts, and maps safe framework/auth/capacity/storage failures to gRPC status/trailers. `df-persistence` implements the consumer-owned repository and operation lookup/deduplication port; PostgreSQL commit/fence is authoritative. `df-protocol` owns generated receipt/lookup wire schemas and compatibility. `df-rpc-bridge` preserves RPC status, trailers, deadlines and cancellation while remaining unaware of command meaning. `df-auth` supplies principal and audience authorization; trace context never grants access. `df-observe` records classified checkpoints but is not a decision store. Integration for this feature belongs to the `df-api` owner, coordinated with these existing owners; no implementation is authorized by this policy.

## Failure and cancellation semantics

1. The client creates one stable operation ID and canonical request fingerprint per intent. Every retry or lookup uses that identity. Reusing its key with a different fingerprint is an operation conflict, not a second command.
2. The session owner validates the authorized binding/current state and atomically commits the decision, operation result, and required effect intents behind the current owner fence/revision. A success receipt exists only after this durable commit. A valid domain rejection is a typed receipt outcome; malformed input, authorization, capacity, deadline, unavailable infrastructure and safe internal failures remain RPC/framework failures.
3. If a response is lost or an RPC ends ambiguously after submission, the client classifies its command knowledge as `Unknown`, then calls the method's `GetOperation` using the same operation ID/fingerprint. A committed receipt resolves it to `Accepted` or `Rejected`; `in_progress` resolves it to `Pending`; `not_recorded` and `expired/indeterminate` remain `Unknown` / require explicit resolution. Do not parse status strings into domain codes.
4. Caller cancellation or deadline ends the caller's wait and transport work. It cannot undo a durable decision or cancel accepted session/run-owned jobs. Before durable acceptance, cancellation may stop admission or discard unaccepted request work as allowed by the method. Only an authorized owner cancellation, reset or run termination controls accepted jobs; late results must pass operation, generation and binding checks.
5. Operation-result retention is finite only when the storage gate chooses and proves a policy. Tombstones or retired namespaces must prevent an expired operation ID from becoming executable again. A missing old result is not evidence of rejection and never licenses automatic resubmission.

## Alternatives considered

- **Treat `gRPC OK` as accepted:** rejected because the status describes RPC handling, while only the session owner's durable commit can establish a command decision. An adapter can return successfully after enqueue or lose a reply after commit.
- **Treat every non-OK status as rejection:** rejected because deadline, cancellation, unavailable service, or disconnect can occur after commit; converting these into rejection risks duplicate decisions and false UI state.
- **Collapse `in_progress`, `not_recorded`, and expired lookup into one pending state:** rejected because an explicit in-flight operation is different from absence/retention uncertainty. Only `in_progress` proves pending work is known; the latter cases remain unknown.
- **Add a universal command-result envelope/status enum:** rejected because existing `DecisionReceipt`, `OperationLookup`, service-specific results, and gRPC status/trailers already cover the distinction. A parallel universal envelope would duplicate authority and would incorrectly force receipts onto reads, streams, uploads, or non-game service results.
- **Use a new operation ID when retrying after ambiguity:** rejected because that bypasses deduplication and may apply the mutation twice. Corrected intent after a known rejection is distinct and follows its method policy.

## Bounded contract example

The following standalone Rust example models a client's classification of existing receipt/lookup observations. `RpcStatus` is carried independently and never drives domain acceptance. It is a finite policy fixture, not a production API, protobuf declaration, transport simulator, or proof of a running service.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RpcStatus {
    Ok,
    Unavailable,
    DeadlineExceeded,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Receipt {
    Accepted,
    Rejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lookup {
    Receipt(Receipt),
    InProgress,
    NotRecorded,
    ExpiredOrIndeterminate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Knowledge {
    Accepted,
    Pending,
    Rejected,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Next {
    ShowConfirmed,
    WaitSameOperation,
    CorrectKnownRejection,
    LookupSameOperation,
    ResolveWithoutReplay,
}

fn next_lookup(lookup: Lookup) -> Next {
    match lookup {
        Lookup::Receipt(Receipt::Accepted) => Next::ShowConfirmed,
        Lookup::Receipt(Receipt::Rejected) => Next::CorrectKnownRejection,
        Lookup::InProgress => Next::WaitSameOperation,
        Lookup::NotRecorded | Lookup::ExpiredOrIndeterminate => Next::ResolveWithoutReplay,
    }
}

fn from_rpc(status: RpcStatus, receipt: Option<Receipt>) -> Knowledge {
    match (status, receipt) {
        (RpcStatus::Ok, Some(Receipt::Accepted)) => Knowledge::Accepted,
        (RpcStatus::Ok, Some(Receipt::Rejected)) => Knowledge::Rejected,
        (RpcStatus::Ok, None)
        | (RpcStatus::Unavailable, _)
        | (RpcStatus::DeadlineExceeded, _)
        | (RpcStatus::Cancelled, _) => Knowledge::Unknown,
    }
}

fn next(knowledge: Knowledge) -> Next {
    match knowledge {
        Knowledge::Accepted => Next::ShowConfirmed,
        Knowledge::Pending => Next::WaitSameOperation,
        Knowledge::Rejected => Next::CorrectKnownRejection,
        Knowledge::Unknown => Next::LookupSameOperation,
    }
}

fn main() {
    let cases = [
        (
            "OK plus accepted receipt",
            from_rpc(RpcStatus::Ok, Some(Receipt::Accepted)),
            Next::ShowConfirmed,
        ),
        (
            "OK plus typed rejection",
            from_rpc(RpcStatus::Ok, Some(Receipt::Rejected)),
            Next::CorrectKnownRejection,
        ),
        (
            "in progress lookup",
            Knowledge::Pending,
            Next::WaitSameOperation,
        ),
        (
            "unavailable after possible submit",
            from_rpc(RpcStatus::Unavailable, None),
            Next::LookupSameOperation,
        ),
        (
            "deadline after possible submit",
            from_rpc(RpcStatus::DeadlineExceeded, None),
            Next::LookupSameOperation,
        ),
        (
            "cancelled receipt wait",
            from_rpc(RpcStatus::Cancelled, None),
            Next::LookupSameOperation,
        ),
    ];

    for (label, knowledge, expected) in cases {
        assert_eq!(next(knowledge), expected, "{label}");
        println!("{label}: {knowledge:?} -> {expected:?}");
    }

    assert_eq!(from_rpc(RpcStatus::Ok, None), Knowledge::Unknown);
    assert_eq!(next(Knowledge::Unknown), Next::LookupSameOperation);
    assert_eq!(next(Knowledge::Pending), Next::WaitSameOperation);
    assert_ne!(Knowledge::Pending, Knowledge::Unknown);
    assert_eq!(
        next_lookup(Lookup::Receipt(Receipt::Accepted)),
        Next::ShowConfirmed
    );
    assert_eq!(
        next_lookup(Lookup::Receipt(Receipt::Rejected)),
        Next::CorrectKnownRejection
    );
    assert_eq!(next_lookup(Lookup::InProgress), Next::WaitSameOperation);
    assert_eq!(next_lookup(Lookup::NotRecorded), Next::ResolveWithoutReplay);
    assert_eq!(
        next_lookup(Lookup::ExpiredOrIndeterminate),
        Next::ResolveWithoutReplay
    );
}
```

## Unresolved production gates and evidence limits

- The RPC API's concrete `DecisionReceipt`, `OperationLookup`, rejection code fields/presence and protobuf allocation/generation are still G03/`df-protocol` owner work. This document freezes no field numbers or Rust public types.
- Physical PostgreSQL constraints, fencing, canonical request fingerprint representation, concurrency behavior, finite retention/tombstones, migration/restore behavior and ambiguous-commit lookup need the storage gate and `df-persistence` integration evidence.
- The session actor's real validation/commit/effect path and end-to-end API mapping must be implemented and exercised by `df-session`, `df-persistence`, `df-api`, and `df-server`; this example does not prove exactly-once effects, authorization, cancellation under load, recovery or restart behavior.
- RPC/WASM bridge feasibility and its actual status/trailer/deadline/cancel behavior remain open under [RPC transport](rpc-transport.md). No browser or physical-device check was run; phone testing remains deferred.
- The operation-ID scope and whether a method can prove a failure occurred before any possible submission are method-specific gates; absent a proven pre-submission guarantee, this decision treats status-only failures as ambiguous.
- Native/WASM workspace builds, Cargo checks, browser execution, transport interoperability, provider calls, audio/device observation and independent frontier review were not performed for this planning-only change.

## Verification and provenance

The governing inputs and SHA-256 values are recorded in `input-hashes.json` under this task's retained evidence root. Earlier v3 evidence includes a 39% free-memory proxy hold against its 42% minimum; that hold and the preceding failed checks remain unchanged. Under a later one-shot v4 admission, extraction passed, `rustfmt --check` failed because two assertions needed repository wrapping, and `rustc --edition 2024 -D warnings` passed. Execution then exposed a fixture mismatch: the generic `Unknown` follow-up correctly chose `LookupSameOperation`, while two rows incorrectly expected the separate post-lookup `ResolveWithoutReplay` action. The source was corrected to test lookup results through `next_lookup` and to format the assertions, then committed as a source-only follow-up. This final correction has not been re-extracted, formatted, compiled, or executed; fresh verification admission is required. All prior receipts/stdout/stderr remain retained. No production behavior or integrated game acceptance is established.

Integration hook: `Define command receipt versus transport status -> accepted pending rejected and unknown are distinct`, owner `df-api`, coordinated with the existing `df-session`, `df-persistence`, `df-protocol`, `df-rpc-bridge`, `df-auth`, and `df-observe` contracts above. Independent review and root integration remain pending.
