# Bootstrap credential and binding lifecycle

Task/attempt: `B-C-df-auth-D02/a1`

Owner: `df-auth`; persistence adapters remain `df-persistence` and public RPC
mapping remains `df-api`.

## Decision

`Principal` is the stable authenticated subject. A credential is a revocable proof
that resolves to one `Principal`; its opaque identity is neither the principal ID,
a permission, nor a trace identifier. Credential bytes and hashes stay inside
`df-auth` and its approved persistence boundary. They never appear in default
telemetry. `OperationContext` carries correlation, deadline, and cancellation
metadata; trace context never authenticates a caller or grants an audience.

`BeginGuest` uses its bounded bootstrap-request namespace and canonical request
fingerprint before a principal exists. A committed bootstrap result binds that key
to one principal and credential identity. A retry with the same key and fingerprint
returns the same identity/result; a different fingerprint conflicts, and a retired
key is refused rather than treated as fresh. Identity, credential grant, and
deduplication result commit atomically. Losing or cancelling the response wait
after commit cannot undo that result or authorize another allocation.

Credential issue, renew, and revoke are principal-scoped, operation-idempotent
transitions. Authentication resolves an active credential at its current
generation. Rotation fences the prior credential generation and its dependent
bindings before a new credential is accepted; revocation immediately denies new
authentication and binding checks. An in-flight result must recheck credential
and binding generations at its commit/delivery boundary. A stale completion is
refused. Cancellation of an RPC ends its wait or unaccepted admission; it does not
roll back a committed grant or cancel owner-owned work.

A client binding is a separate, revocable relationship among an authorized
principal, session, client binding identity, and explicitly granted capabilities or
leases. Stable membership does not identify a connection or tab. Binding/rebind
operations use principal/session/operation identity, and takeover is explicit.
Every action, private projection, audio/capture operation, and delivery checks the
current binding and relevant lease. Replacing or revoking a binding fences earlier
generations and closes affected streams/discards unsent unauthorized data. Already
received data cannot be recalled. Guest classification, selected client role,
tenant/payer authority, and a valid credential alone do not create campaign
membership or private audience access.

## Rationale and unresolved facts

Opaque credential identity supports lookup, deduplication, rotation, and revocation
without making bearer bytes an application identifier. Separate trace context
preserves observability without creating an authorization input. Generation checks
reject work that finishes after a credential or client binding has changed.
Stable binding identity is distinct from ephemeral connection/tab identity so a
reconnect does not silently create membership or take control.

The credential provider, secret format/hash, token lifetime, storage schema,
bootstrap transport mechanism, and concrete production types remain open gates.
This decision defines neither those mechanisms nor cryptographic strength. The
finite proof below uses synthetic integer identities and in-memory state only; it
does not authenticate real credentials, contact a provider, or qualify persistence,
RPC, browser, or running service behavior. It does not add a public competing
production type.

## Bounded executable proof

This standalone Rust 2024 example models only the selected lifecycle rules with
finite synthetic identities. In particular, the `TraceContext` and
`CredentialIdentity` types are distinct, and authentication accepts only the
credential identity. A committed bootstrap result is retrieved after a simulated
cancelled response wait; replacing a binding makes its earlier completion stale;
revoking a credential denies its current binding. No real credential value is
present.

```rust
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct BootstrapRequestId(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CredentialIdentity(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct PrincipalId(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct SessionId(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ClientBindingId(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TraceContext {
    trace_id: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BootstrapGrant {
    principal: PrincipalId,
    credential: CredentialIdentity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BindingTicket {
    principal: PrincipalId,
    credential: CredentialIdentity,
    credential_generation: u64,
    session: SessionId,
    binding: ClientBindingId,
    binding_generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refusal {
    UnknownCredential,
    CredentialRevoked,
    StaleBinding,
}

#[derive(Clone, Copy, Debug)]
struct CredentialRecord {
    principal: PrincipalId,
    generation: u64,
    active: bool,
}

#[derive(Clone, Copy, Debug)]
struct BindingRecord {
    principal: PrincipalId,
    credential: CredentialIdentity,
    session: SessionId,
    generation: u64,
    active: bool,
}

#[derive(Debug, Default)]
struct AuthState {
    bootstrap_results: BTreeMap<BootstrapRequestId, BootstrapGrant>,
    credentials: BTreeMap<CredentialIdentity, CredentialRecord>,
    bindings: BTreeMap<ClientBindingId, BindingRecord>,
    next_principal: u64,
    next_credential: u64,
}

impl AuthState {
    fn begin_guest(&mut self, request: BootstrapRequestId, _trace: TraceContext) -> BootstrapGrant {
        if let Some(grant) = self.bootstrap_results.get(&request) {
            return *grant;
        }

        self.next_principal += 1;
        self.next_credential += 1;
        let grant = BootstrapGrant {
            principal: PrincipalId(self.next_principal),
            credential: CredentialIdentity(self.next_credential),
        };
        self.credentials.insert(
            grant.credential,
            CredentialRecord {
                principal: grant.principal,
                generation: 1,
                active: true,
            },
        );
        self.bootstrap_results.insert(request, grant);
        grant
    }

    fn authenticate(&self, credential: CredentialIdentity) -> Result<(PrincipalId, u64), Refusal> {
        let record = self
            .credentials
            .get(&credential)
            .ok_or(Refusal::UnknownCredential)?;
        if !record.active {
            return Err(Refusal::CredentialRevoked);
        }
        Ok((record.principal, record.generation))
    }

    fn bind(
        &mut self,
        credential: CredentialIdentity,
        session: SessionId,
        binding: ClientBindingId,
    ) -> Result<BindingTicket, Refusal> {
        let (principal, credential_generation) = self.authenticate(credential)?;
        let binding_generation = self
            .bindings
            .get(&binding)
            .map_or(1, |current| current.generation + 1);
        self.bindings.insert(
            binding,
            BindingRecord {
                principal,
                credential,
                session,
                generation: binding_generation,
                active: true,
            },
        );
        Ok(BindingTicket {
            principal,
            credential,
            credential_generation,
            session,
            binding,
            binding_generation,
        })
    }

    fn validate_binding(&self, ticket: BindingTicket) -> Result<(), Refusal> {
        let (principal, credential_generation) = self.authenticate(ticket.credential)?;
        if principal != ticket.principal || credential_generation != ticket.credential_generation {
            return Err(Refusal::StaleBinding);
        }
        let current = self
            .bindings
            .get(&ticket.binding)
            .ok_or(Refusal::StaleBinding)?;
        if !current.active
            || current.principal != ticket.principal
            || current.credential != ticket.credential
            || current.session != ticket.session
            || current.generation != ticket.binding_generation
        {
            return Err(Refusal::StaleBinding);
        }
        Ok(())
    }

    fn revoke(&mut self, credential: CredentialIdentity) -> Result<(), Refusal> {
        let record = self
            .credentials
            .get_mut(&credential)
            .ok_or(Refusal::UnknownCredential)?;
        record.generation += 1;
        record.active = false;
        for binding in self.bindings.values_mut() {
            if binding.credential == credential {
                binding.active = false;
                binding.generation += 1;
            }
        }
        Ok(())
    }
}

fn main() {
    let mut state = AuthState::default();
    let first_trace = TraceContext { trace_id: 7 };
    let first = state.begin_guest(BootstrapRequestId(11), first_trace);

    // The response wait is cancelled after the grant is committed. A retry with a
    // different trace still finds the same principal and credential identity.
    let retry_trace = TraceContext { trace_id: 99 };
    let retry = state.begin_guest(BootstrapRequestId(11), retry_trace);
    assert_ne!(first_trace, retry_trace);
    assert_eq!(first, retry);
    assert_eq!(
        state.authenticate(first.credential),
        Ok((first.principal, 1))
    );

    let second = state.begin_guest(BootstrapRequestId(12), first_trace);
    assert_ne!(first.credential, second.credential);
    assert_ne!(first.principal, second.principal);

    let old_binding = state
        .bind(first.credential, SessionId(4), ClientBindingId(21))
        .expect("active credential can bind");
    assert_eq!(state.validate_binding(old_binding), Ok(()));

    let replacement = state
        .bind(first.credential, SessionId(4), ClientBindingId(21))
        .expect("authorized takeover replaces the binding generation");
    assert_eq!(
        replacement.binding_generation,
        old_binding.binding_generation + 1
    );
    assert_eq!(
        state.validate_binding(old_binding),
        Err(Refusal::StaleBinding)
    );
    assert_eq!(state.validate_binding(replacement), Ok(()));

    state
        .revoke(first.credential)
        .expect("known credential revokes");
    assert_eq!(
        state.authenticate(first.credential),
        Err(Refusal::CredentialRevoked)
    );
    assert_eq!(
        state.validate_binding(replacement),
        Err(Refusal::CredentialRevoked)
    );
    assert_eq!(
        state.authenticate(CredentialIdentity(999)),
        Err(Refusal::UnknownCredential)
    );
    println!("PASS: idempotent bootstrap, separate trace identity, stale binding and refusal");
}
```

## Governing contracts and consumer handoff

- [Subsystem architecture](subsystem-architecture.md) assigns identity, membership
  access, `Authenticator`, `CredentialStore`, `Principal`, and
  `AuthorizedAudience` to `df-auth`.
- [Subsystem interfaces](subsystem-interfaces.md#identity-and-session-ownership)
  requires scoped bootstrap deduplication, atomic identity/grant/result allocation,
  stable membership separate from transient connection, and checks at delivery.
- [RPC API](rpc-api.md#protocol-and-access) says trace headers do not grant identity;
  its allocation and cancellation rules require stable idempotency and committed
  work to survive cancellation of the caller's wait.
- [Runtime reliability](runtime-reliability.md) assigns timers/jobs/results to an
  owner and requires generation and operation identity checks for stale results.
- [Commerce service](commerce-service.md) keeps credentials in `df-auth` and
  financial payer/entitlement state in commerce; neither substitutes for campaign
  audience authorization.

The next implementation consumer is `df-auth`, with `df-persistence` implementing
its credential-store boundary and `df-api` mapping the approved bootstrap/renew/
revoke/bind contract. Concrete public types, cryptographic tokens, store schema,
provider selection, and endpoint registration require their separately frozen
implementation contracts. This proof is a finite decision fixture only.
