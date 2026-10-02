# Native origin, CSRF, and recovery controls

Task/attempt: `B-X01-D02/a1`  
Input revision: `308920e328ba6df85bea1e3d2abcbea5f1b796e6`  
Status: bounded source-backed policy decision; production auth, HTTP, journal, and restore implementations remain unqualified.

## Decision

Use [Bootstrap cookie and origin policy](bootstrap-origin-policy.md) as the governing browser-origin contract. Browser tunnel admission compares exactly one parsed `Origin` with the configured canonical external HTTPS origin and requires externally secure WSS; missing, null, duplicate, malformed, or mismatching origins fail closed. That check limits cross-site browser admission only. It never authenticates a principal or grants an RPC. Cookie-authenticated customer mutations additionally require same-origin validation and the route's explicit CSRF/state defense. Browser cookies stay host-only, Secure, HttpOnly, and SameSite-restricted; their concrete attributes remain a G04 decision.

Native clients use the separate native credential/admission mechanism selected by `df-auth` and G04. Do not require a browser `Origin` for native RPC, accept a caller-supplied `Origin` as native identity, or infer a native client from absent Origin. Native RPC requires authenticated current credentials over the configured secure native transport, then per-operation authorization against the current principal, tenant/audience, target and permission. A credential generation revoked or superseded by recovery rejects immediately; overlap for key rotation never revives revoked credentials. Native apps that embed a browser/webview and use cookie-authenticated HTTP routes are subject to the browser route's Origin and CSRF checks.

Credential recovery is a distinct `df-auth` operation. Require a short-lived, single-use opaque challenge sent only to an independently verified recovery channel, bounded attempts, and the same public response for unknown and known accounts. Successful completion atomically consumes the challenge, advances credential generation, revokes prior refresh/session/binding grants, and issues only the newly authorized credential. Do not link identities by email match, create another tenant/member, or change already committed game decisions. A consumed, expired, exhausted, wrong-purpose, or already-used challenge refuses without issuing a credential. If challenge consumption or credential-generation commit is ambiguous, report pending/unknown and require stable-ID lookup/reconciliation; never tell the caller recovery succeeded on a guessed write. Revoke previous credentials before acknowledging success.

For disaster recovery, keep the protected append-only recovery journal and its authenticated nonregressing head in `df-persistence` as the existing irreversible authority. Restore into quarantine, verify the latest protected head and its completeness, issue a strictly greater recovery epoch durably, replay revocations and privacy suppressions, retire old operation/allocation namespaces, and reconcile possible external sends by stable identity before serving affected scopes. Missing, stale, unverifiable, or ambiguous journal/head/key state holds affected native/private/commercial admissions closed. An old PostgreSQL snapshot's omission is not proof that a credential, deletion, payment, or external effect never existed. Preserve exact retained receipt lookup; otherwise return an explicit expired/indeterminate outcome. Do not regenerate deleted private payload, reset unknown liabilities, or blindly resend. Game state may use its separately declared RPO only with a visible lost-revision range; this does not weaken zero-regression requirements for irreversible suppressions and liabilities.

No new authority is introduced. `df-auth` owns credential issue/lookup/revoke, principal and permission decisions, recovery challenges and current credential generation. `df-api` plus the selected HTTP composition owner enforces route-level browser Origin/CSRF and calls `df-auth`; `df-rpc-bridge` preserves native/browser transport separation and passes transport facts but does not authorize. `df-server` supplies configured public origin, secure transport and trusted-proxy provenance. `df-persistence` owns durable credential/recovery state, challenge consumption, fences, protected journal/head, epoch issuance, namespace retirement and restore reconciliation. Session and RPC consumers revalidate current authorization at existing publication/delivery boundaries. The coordinator owns cross-system integration and acceptance. `df-types` owns only canonical primitives/revisions; this policy adds no duplicate credential, journal, tenant, or identity type.

## Alternatives and rationale

- **Apply browser Origin checks to native RPC:** rejected. Native transports may not send Origin, and any non-browser caller can forge it. Treating it as identity creates a bypass or excludes legitimate clients. Keep it a browser CSRF/admission signal, then authenticate credentials for both client classes.
- **Use a native credential as ambient browser cookie state without CSRF:** rejected. Browsers attach cookies cross-site; route Origin and explicit CSRF/state validation remain necessary for cookie-authenticated mutations.
- **Recover by email lookup, account creation, or replaying the old credential:** rejected. Account existence must not be disclosed; identity linkage needs independent proof; old grants must become unusable after recovery.
- **Treat restored PostgreSQL state as the only recovery authority, or replay an uncertain request:** rejected. It can omit post-backup revocations, suppression, or external liability. The existing protected journal/head must gate recovery and uncertain work is reconciled by stable identity.
- **Put a second local policy/journal in the bridge, client, or auth cache:** rejected. It duplicates authority and can diverge after revocation or restore. Adapters carry facts and enforce the single current decision at their boundary.

## Finite std-only contract example

This pure model exercises native admission, browser mutation CSRF, and credential-recovery refusal. Inputs are already parsed/configured facts; it does not parse HTTP, establish TLS, authenticate a real credential, persist a challenge, or implement production fencing. The recovery success branch means only that the supplied facts pass the policy decision; durable consume-and-revoke must be atomic in the real `df-auth`/`df-persistence` implementation.

```rust
#[derive(Debug, PartialEq, Eq)]
enum Refusal {
    InsecureTransport,
    MissingCredential,
    RevokedCredential,
    MissingOrigin,
    DuplicateOrigin,
    CrossOrigin,
    MissingCsrfProof,
    UnknownAccount,
    ExpiredChallenge,
    ReusedChallenge,
    AttemptsExhausted,
    UnverifiedRecoveryChannel,
}

#[derive(Clone, Copy)]
struct NativeRequest {
    secure_transport: bool,
    credential_present: bool,
    credential_generation: u64,
    current_generation: u64,
}

fn admit_native(request: NativeRequest) -> Result<(), Refusal> {
    if !request.secure_transport {
        return Err(Refusal::InsecureTransport);
    }
    if !request.credential_present {
        return Err(Refusal::MissingCredential);
    }
    if request.credential_generation != request.current_generation {
        return Err(Refusal::RevokedCredential);
    }
    Ok(())
}

fn admit_cookie_mutation(
    expected_origin: &str,
    origins: &[&str],
    csrf_proof_valid: bool,
) -> Result<(), Refusal> {
    match origins {
        [] => return Err(Refusal::MissingOrigin),
        [_first, _second, ..] => return Err(Refusal::DuplicateOrigin),
        [origin] if *origin != expected_origin => return Err(Refusal::CrossOrigin),
        [_] => {}
    }
    if !csrf_proof_valid {
        return Err(Refusal::MissingCsrfProof);
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct Recovery {
    account_known: bool,
    challenge_live: bool,
    challenge_unused: bool,
    attempts_left: u8,
    channel_verified: bool,
}

fn complete_recovery(recovery: Recovery) -> Result<(), Refusal> {
    if !recovery.account_known {
        return Err(Refusal::UnknownAccount);
    }
    if !recovery.channel_verified {
        return Err(Refusal::UnverifiedRecoveryChannel);
    }
    if !recovery.challenge_live {
        return Err(Refusal::ExpiredChallenge);
    }
    if !recovery.challenge_unused {
        return Err(Refusal::ReusedChallenge);
    }
    if recovery.attempts_left == 0 {
        return Err(Refusal::AttemptsExhausted);
    }
    Ok(())
}

fn main() {
    assert_eq!(
        admit_native(NativeRequest {
            secure_transport: true,
            credential_present: true,
            credential_generation: 4,
            current_generation: 4,
        }),
        Ok(())
    );
    assert_eq!(
        admit_native(NativeRequest {
            secure_transport: true,
            credential_present: true,
            credential_generation: 3,
            current_generation: 4,
        }),
        Err(Refusal::RevokedCredential)
    );
    assert_eq!(
        admit_native(NativeRequest {
            secure_transport: false,
            credential_present: true,
            credential_generation: 4,
            current_generation: 4,
        }),
        Err(Refusal::InsecureTransport)
    );
    assert_eq!(
        admit_cookie_mutation("https://play.example.test", &["https://evil.test"], true),
        Err(Refusal::CrossOrigin)
    );
    assert_eq!(
        admit_cookie_mutation(
            "https://play.example.test",
            &["https://play.example.test"],
            false
        ),
        Err(Refusal::MissingCsrfProof)
    );
    assert_eq!(
        complete_recovery(Recovery {
            account_known: true,
            challenge_live: true,
            challenge_unused: false,
            attempts_left: 2,
            channel_verified: true,
        }),
        Err(Refusal::ReusedChallenge)
    );
    assert_eq!(
        complete_recovery(Recovery {
            account_known: true,
            challenge_live: true,
            challenge_unused: true,
            attempts_left: 2,
            channel_verified: true,
        }),
        Ok(())
    );
    println!("origin, CSRF, and recovery policy: 7 assertions passed");
}
```

## Unresolved gates and evidence limits

G04 must select the concrete native credential mechanism, secure transport/listener policy, credential storage and rotation/revocation overlap; supported clients and any webview behavior; independently verified recovery channel and its enrollment/change flow; challenge identifier, purpose binding, expiry/attempt policy and durable idempotency; and HTTP mutation inventory plus CSRF proof mechanism. G02/G03 must select the parser/framework and ensure duplicate Origin values are observable and rejected; trusted proxy configuration must overwrite forwarding values. G05/X02 must qualify challenge atomicity, credential-generation fencing, journal object consistency/integrity/key custody/head nonregression, restore completeness, epoch persistence, namespace retirement, and fault recovery. Legal/compliance owners must approve account-recovery messaging and channel policy. Numerical challenge/rate/expiry values remain governed by existing proposed defaults and require measured/abuse review; this decision does not claim those values are tuned.

At the input revision there is no production `df-auth`, HTTP route, tunnel composition, credential store, or recovery-journal adapter in `crates/`. The S00 loopback fixture is synthetic only. Planning does not prove credential security, browser or native device behavior, CSRF enforcement, recovery, restore, journal durability, or service security. No live security or deployment claim is made. The Rust model's seven assertions, when run, prove only the decision branches encoded above. Browser and phone testing, native/WASM integration, database fault injection, independent frontier review, and coordinator integration review remain separate gates.

## Governing source fingerprints

Hashes were computed from the isolated input worktree at `308920e328ba6df85bea1e3d2abcbea5f1b796e6` before this document was added. The attempt's complete frozen governing input list, including selected section scope, is `worker-context.json`; principal source fingerprints are reproduced here for review:

- `planning/bootstrap-origin-policy.md`: `b289181f9cec413fa3d5410d0dee9d977992e78cdd213fca3a39595776ac76ee`
- `planning/service-operations.md`: `780360bcff243a96f5fc9a39a88b47efc1402e563623c134229f56579eb714df`
- `planning/implementation-roadmap.md`: `0160ad8e8ec38f768e2348209b9989e30e8f403d9b1a4ebf694f0801f7206932`
- `planning/commerce-service.md`: `bf56a48d8055639cb3634ba6fa32d72b19fb64ea3fc8df0cb649e5f7ad768b5e`
- `planning/recovery-revision-policy.md`: `45edf7f27cca903d6a5944e7fbb75a372a5e81ece50b554c80704a8dd2038e61`
- `planning/subsystem-architecture.md`: `cf549f4f046f76713c881228d82c6a7111d8ccc84db208cb9d9db67bb192d9f6`
- `planning/subsystem-interfaces.md`: `f26e1dca42e878f8a816c9f9aa37463cb8f061224598214ceee62632a161907a`
- `planning/rpc-api.md`: `a388e152c973efcd937b2725e3a4878699a22f5ea928ab55701d0ec06ade0432`
- `planning/rpc-transport.md`: `841ba2500a426e6bacf62017d60e106cae402eb62b0e171eccab26dfb9c31c79`
- `planning/erasure-retention-policy.md`: `8fd4ca161f2bfc9678a61b530b2de4143222984d8e11d7f1db9fdc962ecdda6e`
- `planning/typed-identity-policy.md`: `cf1648b4444b40f1c52571fdf7124e5ad1c1f41d544cc580807b2836e8ba22cf`
- `planning/storage-architecture.md`: `69c6ef1ec2ce02b0ecc33c361e83ef9a027f669df1abe49199605552466a1da6`
- `planning/shared-contract-waves.md`: `3498bda5d59aca8e4064cca74ce711e1a52a1b0eb71a3c7c240018b107b777c2`
- `development/backlog-catalog.json`: `039b0a03b4085b43ad32c4063e2cb8fc789fc55fff7552aec6e951ec3b4704c3`

Original acceptance criteria (preserved verbatim):

1. `concrete native policy`
2. `The named outcome has actual source/build-bound evidence; unsupported, pending, failed and unperformed checks remain explicit.`

Original verification procedures (preserved verbatim):

1. `Freeze the cited source decision and a bounded contract example; compare concrete native policy. Retain decision, alternatives and unresolved facts.`
2. `Exact executable commands: TBD at G01 and scoped prerequisite resolution; this planned procedure is not a claim that Rust/browser/provider checks ran.`
