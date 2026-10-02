# F45-D01: one authority for remote mixed-room play

Task/attempt: `B-F45-D01-a1`  
Decision input: `308920e328ba6df85bea1e3d2abcbea5f1b796e6`  
Owner: `df-session` owns the per-session actor and fenced decision path; `df-auth` authorizes principals and audiences; `df-api` projects authorized views; `df-client` reconnects and reconciles snapshots; `df-audio` enforces output leases. `df-engine`/`df-rules` retain game and source legality. `df-persistence` persists decisions through the existing owner fence. This is a design contract, not an implementation claim.

Governing sources: [remote-play](remote-play.md) (Model, permissions and audiovisual topology; Native ownership, failure and recovery; Gates, observability and acceptance), [feature refinement](feature-refinement.md) (remote table stakes and delivery), [subsystem interfaces](subsystem-interfaces.md) (Identity and session ownership; Projection, RPC and transport; feature candidates), [subsystem architecture](subsystem-architecture.md) (server and browser crate ownership), [runtime reliability](runtime-reliability.md), [storage architecture](storage-architecture.md), [observability](observability.md), [client architecture](client-architecture.md), [typed identity policy](typed-identity-policy.md), and [shared contract waves](shared-contract-waves.md). Workflow, source hashes, toolchain and guard are frozen in the attempt brief.

## Decision

A campaign/session has one authoritative `df-session` actor/fenced writer for both remote inputs and local TV/phone inputs. The directory routes to an actor per session; it does not serialize all sessions globally. The actor serializes membership-linked commands, accepted timers and job results, and invokes the existing engine/rules authority for legal outcomes. Remote transport changes reachability and latency, not rules authority, turn ownership or state replication. No peer authority, remote-only engine, shadow seat registry or alternate input protocol is added.

Every command is authenticated as a trusted principal, checked against current membership and the relevant input lease/binding at admission, and committed under the actor's current owner fence. An operation retry first looks up its operation identity; an ambiguous commit remains uncertain until lookup resolves it. A lost connection or revoked lease blocks new submissions and closes affected streams, but does not undo an already committed decision or detach its run-owned work. A takeover is explicit and fences the old binding. Disconnect/presence is observational: it does not spend a turn, grant another member input, or advance campaign time. Pause/AFK/expiry behavior remains an explicit separately disclosed policy.

The actor creates an authorized snapshot/read model tagged with session/run/revision and presentation epoch/sequence. `df-api` obtains `AuthorizedAudience` through `df-auth` and filters private data before serialization or asset demand. Shared displays receive only their authorized audience; an individual member's private view/audio is delivered to that member's authorized binding. Revalidation at publish/delivery boundaries fences revocation; unsent unauthorized data is discarded and obsolete client buffers are cleared. Already received data cannot be recalled. Trace IDs and client-reported role labels never grant permission.

For mixed-room media, `df-audio` routes public tracks to a designated room output under an explicit output lease, avoiding duplicate local public playback. Private tracks go only to approved individual listener bindings with current audio leases. Private content never appears in shared streams, manifests, captions, prefetch cues, or tempo changes. A missing/denied private route falls back only to that audience's authorized text/caption; never reroute private audio to the public speaker. Browser capture/playback reports are observations, not authority or proof of audible output.

## Alternatives and rationale

Peer-to-peer or one-writer-per-device gameplay would create competing state authorities, make simultaneous legal actions and durable retries ambiguous, and risk exposing private state. A globally serialized actor would needlessly couple unrelated sessions. API-owned mutable state or a parallel membership/seat registry would split the existing session boundary. A single shared audience/stream would leak private views. All are rejected in favor of the already planned per-session actor, trusted audience projection and scoped leases.

The selected design reuses `SessionHandle`, `AuthorizedAudience`, client bindings, operation lookup, fenced `SessionRepository` commit and existing BindClient/WatchControl/Talk/Listen/Report families. These names describe current planned boundaries; F45 additions remain candidates until G03 freezes generated fields, numbering and compatibility fixtures. This policy does not define production Rust structs or protobuf fields.

## Finite executable contract illustration

The literal std-only Rust program below models only the selected boundary. It accepts a command only through the single actor sequence and current input lease, and exposes private data only to its permitted audience. Its counters stand in for committed revisions; it does not implement persistence, auth, RPC, encryption or real audio. Run this exact literal under pinned Rust 1.98.1, edition 2024, repository rustfmt configuration, `rustfmt --check`, `rustc -D warnings`, then execute it. Passing proves only these finite assertions.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Audience {
    Shared,
    Member(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refusal {
    StaleBinding,
    DuplicateOperation,
    UnauthorizedAudience,
}

#[derive(Debug)]
struct SessionActor {
    revision: u64,
    binding: u8,
    seen_operation: Option<u8>,
    private_fact: &'static str,
}

impl SessionActor {
    fn submit(&mut self, binding: u8, operation: u8) -> Result<u64, Refusal> {
        if binding != self.binding {
            return Err(Refusal::StaleBinding);
        }
        if self.seen_operation == Some(operation) {
            return Err(Refusal::DuplicateOperation);
        }
        self.seen_operation = Some(operation);
        self.revision += 1;
        Ok(self.revision)
    }

    fn project(&self, audience: Audience) -> Result<&'static str, Refusal> {
        match audience {
            Audience::Member(7) => Ok(self.private_fact),
            Audience::Member(_) => Err(Refusal::UnauthorizedAudience),
            Audience::Shared => Ok("public scene"),
        }
    }
}

fn main() {
    let mut actor = SessionActor {
        revision: 0,
        binding: 9,
        seen_operation: None,
        private_fact: "member 7 alone hears the whisper",
    };
    assert_eq!(actor.submit(9, 1), Ok(1));
    assert_eq!(actor.revision, 1);
    assert_eq!(actor.submit(9, 1), Err(Refusal::DuplicateOperation));
    assert_eq!(actor.submit(8, 2), Err(Refusal::StaleBinding));
    assert_eq!(actor.revision, 1);
    assert_eq!(
        actor.project(Audience::Member(7)),
        Ok("member 7 alone hears the whisper")
    );
    assert_eq!(
        actor.project(Audience::Member(8)),
        Err(Refusal::UnauthorizedAudience)
    );
    assert_eq!(actor.project(Audience::Shared), Ok("public scene"));
}
```

## Failure semantics and production gates

Admission/authentication failure is a typed refusal and cannot reach the actor. Stale/revoked binding rejects new input and terminates the affected sensitive stream; an already committed operation remains committed. Duplicate operation identity resolves to the prior receipt, never a second rules execution. Storage/fence failure prevents authoritative commit; after uncertain transport, lookup is required before retry. Slow consumers are bounded and resynchronize from a current authorized snapshot without blocking other sessions. Private-route failure remains private and uses authorized text/captions. No disconnect-driven NPC takeover, reroll, duplicate reward, fabricated success, or wall-clock turn deadline is permitted.

Still unresolved before production: G03 typed/wire fields, protobuf numbering and compatibility fixtures; G04 trusted invite/credential storage, abuse bounds, principal/binding lease and active-stream revocation behavior; G08 selected network/device/capture/audio matrix and measured admission/resource limits; measured campaign capacity, tail latency, resource use and relay/usage cost; validated private/public audio routing and denied-mic fallback on supported clients; fixed-build independently networked browser acceptance for simultaneous legal input, reconnect mid-cue, revocation, slow consumers, packet impairment and paired-state noninterference. No arbitrary player/device cap is selected here. Physical phone and TV tests, browser smoke, audio playback observation, PostgreSQL/fence integration and native/WASM workspace checks are unperformed. A local illustrative fixture cannot establish product behavior or independent frontier approval.

## Integration hooks

- `df-auth`: validate invite/admission and current principal, membership, audience and binding leases.
- `df-session`: route to the per-session actor; serialize local/remote commands; operation lookup and fenced decision ownership.
- `df-engine`/`df-rules`: determine legal decision and pending source choices; do not infer source deadlines from network presence.
- `df-api`: map and filter authorized views before serialization and asset demand.
- `df-client`: bounded reconnect/resubscribe, operation lookup before uncertain replay, clear obsolete view/audio buffers.
- `df-audio`/`df-media`: enforce capture/output/listener leases, supported codecs, and safe private-route fallback.
- `df-persistence`: persist membership, policy, binding/operation decisions and pending outcomes under the existing owner fence.

The required remote/mixed-room end-to-end integration and independent frontier review remain separate tasks/gates.
