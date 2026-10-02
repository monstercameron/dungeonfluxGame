# F45-D01: one authority for remote mixed-room play

Task/attempt: `B-F45-D01-a2` (repair of `B-F45-D01-a1`)

Repair input: `750e24ef5d6a7cc386f9650a628c626c48c73b4d`; original frozen design inputs: `308920e328ba6df85bea1e3d2abcbea5f1b796e6`

Owner: `df-session` owns the per-session actor and fenced decision path; `df-auth` authorizes principals and audiences; `df-api` projects authorized views; `df-client` reconnects and reconciles snapshots; `df-audio` enforces output leases. `df-engine`/`df-rules` retain game and source legality. `df-persistence` persists decisions through the existing owner fence. This is a design contract, not an implementation claim.

Governing sources: [remote-play](remote-play.md) (Model, permissions and audiovisual topology; Native ownership, failure and recovery; Gates, observability and acceptance), [feature refinement](feature-refinement.md) (remote table stakes and delivery), [subsystem interfaces](subsystem-interfaces.md) (Identity and session ownership; Projection, RPC and transport; feature candidates), [subsystem architecture](subsystem-architecture.md) (server and browser crate ownership), [runtime reliability](runtime-reliability.md), [storage architecture](storage-architecture.md), [observability](observability.md), [client architecture](client-architecture.md), [typed identity policy](typed-identity-policy.md), and [shared contract waves](shared-contract-waves.md). Workflow, source hashes, toolchain and guard are frozen in the attempt brief.

## Decision

A campaign/session has one authoritative `df-session` actor/fenced writer for both remote inputs and local TV/phone inputs. The directory routes to an actor per session; it does not serialize all sessions globally. The actor serializes membership-linked commands, accepted timers and job results, and invokes the existing engine/rules authority for legal outcomes. Remote transport changes reachability and latency, not rules authority, turn ownership or state replication. No peer authority, remote-only engine, shadow seat registry or alternate input protocol is added.

Every command is authenticated as a trusted principal, checked against current membership and the relevant input lease/binding at admission, and committed under the actor's current owner fence. An operation retry first looks up its durable `(session, principal, operation)` identity before current-offer validation. A matching request fingerprint returns the original committed receipt even after intervening operations; a mismatched fingerprint is rejected. Retention expiry never makes an accepted key fresh: keep a tombstone/retired namespace or refuse new work while its receipt cannot be retained. An ambiguous commit remains uncertain until durable lookup resolves it. A lost connection or revoked lease blocks new submissions and closes affected streams, but does not undo an already committed decision or detach its run-owned work. A takeover is explicit and fences the old binding. Disconnect/presence is observational: it does not spend a turn, grant another member input, or advance campaign time. Pause/AFK/expiry behavior remains an explicit separately disclosed policy.

The actor creates an authorized snapshot/read model tagged with session/run/revision and presentation epoch/sequence. `df-api` obtains `AuthorizedAudience` through `df-auth` and filters private data before serialization or asset demand. Shared displays receive only their authorized audience; an individual member's private view/audio is delivered to that member's authorized binding. Revalidation at publish/delivery boundaries fences revocation; unsent unauthorized data is discarded and obsolete client buffers are cleared. Already received data cannot be recalled. Trace IDs and client-reported role labels never grant permission.

For mixed-room media, `df-audio` routes public tracks to a designated room output under an explicit output lease, avoiding duplicate local public playback. Lease validity is checked against the current output/listener binding and generation at delivery; replacement or revocation makes an old lease unusable. Private tracks go only to approved individual listener bindings with current audio leases. Private content never appears in shared streams, manifests, captions, prefetch cues, or tempo changes. A missing/denied private route falls back only to that audience's authorized text/caption; never reroute private audio to the public speaker. Browser capture/playback reports are observations, not authority or proof of audible output.

## Alternatives and rationale

Peer-to-peer or one-writer-per-device gameplay would create competing state authorities, make simultaneous legal actions and durable retries ambiguous, and risk exposing private state. A globally serialized actor would needlessly couple unrelated sessions. API-owned mutable state or a parallel membership/seat registry would split the existing session boundary. A single shared audience/stream would leak private views. All are rejected in favor of the already planned per-session actor, trusted audience projection and scoped leases.

The selected design reuses `SessionHandle`, `AuthorizedAudience`, client bindings, operation lookup, fenced `SessionRepository` commit and existing BindClient/WatchControl/Talk/Listen/Report families. These names describe current planned boundaries; F45 additions remain candidates until G03 freezes generated fields, numbering and compatibility fixtures. This policy does not define production Rust structs or protobuf fields.

## Finite executable contract illustration

The literal std-only Rust program below models only the selected boundary. It accepts commands through one actor with the current input binding/generation, returns the prior receipt for a nonadjacent matching operation retry, rejects fingerprint conflicts, and refuses new commands rather than forgetting accepted identities when its illustrative finite receipt slots fill. It also exercises symbolic public/private routes, listener/output lease generations, revocation and audience refusal. The tiny fixed receipt array is a finite fixture bound only, not a measured or proposed production capacity. The model does not implement durable PostgreSQL, trusted auth, RPC, encryption, audio playback or actual lease enforcement. Run this exact literal under pinned Rust 1.98.1, edition 2024, repository rustfmt configuration, `rustfmt --check`, `rustc -D warnings`, then execute it. Passing proves only these finite assertions.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Audience {
    Shared,
    Member(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refusal {
    StaleInputLease,
    OperationFingerprintConflict,
    ReceiptLedgerFull,
    RevisionExhausted,
    StaleOutputLease,
    StaleListenerLease,
    LockedListener,
    PrivateRouteDenied,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct InputLease {
    principal: u8,
    member: u8,
    binding: u8,
    generation: u8,
    active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Receipt {
    principal: u8,
    operation: u8,
    fingerprint: u8,
    revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Submit {
    Committed(u64),
    PreviouslyCommitted(u64),
}

#[derive(Debug)]
struct SessionActor {
    revision: u64,
    input_lease: InputLease,
    receipts: [Option<Receipt>; 2],
    private_fact: &'static str,
}

impl SessionActor {
    fn submit(
        &mut self,
        lease: InputLease,
        operation: u8,
        fingerprint: u8,
    ) -> Result<Submit, Refusal> {
        if !lease.active || lease != self.input_lease {
            return Err(Refusal::StaleInputLease);
        }
        if let Some(receipt) =
            self.receipts.iter().flatten().find(|receipt| {
                receipt.principal == lease.principal && receipt.operation == operation
            })
        {
            return if receipt.fingerprint == fingerprint {
                Ok(Submit::PreviouslyCommitted(receipt.revision))
            } else {
                Err(Refusal::OperationFingerprintConflict)
            };
        }
        let slot = self
            .receipts
            .iter_mut()
            .find(|receipt| receipt.is_none())
            .ok_or(Refusal::ReceiptLedgerFull)?;
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(Refusal::RevisionExhausted)?;
        *slot = Some(Receipt {
            principal: lease.principal,
            operation,
            fingerprint,
            revision,
        });
        self.revision = revision;
        Ok(Submit::Committed(revision))
    }

    fn project(&self, audience: Audience) -> Result<&'static str, Refusal> {
        match audience {
            Audience::Member(7) => Ok(self.private_fact),
            Audience::Member(_) | Audience::Shared => Err(Refusal::PrivateRouteDenied),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct OutputLease {
    output: u8,
    generation: u8,
    active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ListenerLease {
    member: u8,
    generation: u8,
    active: bool,
    unlocked: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Track {
    Public,
    Private(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Route {
    PublicRoomOutput { output: u8 },
    PublicListener { output: u8, member: u8 },
    PrivateListener { member: u8 },
}

#[derive(Debug)]
struct AudioTopology {
    generation: u8,
    output: OutputLease,
}

impl AudioTopology {
    fn route(
        &self,
        track: Track,
        audience: Audience,
        output: OutputLease,
        listener: Option<ListenerLease>,
    ) -> Result<Route, Refusal> {
        match track {
            Track::Public => {
                if !output.active || output != self.output || output.generation != self.generation {
                    return Err(Refusal::StaleOutputLease);
                }
                match audience {
                    Audience::Shared => Ok(Route::PublicRoomOutput {
                        output: output.output,
                    }),
                    Audience::Member(member) => {
                        let listener = listener.ok_or(Refusal::StaleListenerLease)?;
                        if !listener.active
                            || listener.generation != self.generation
                            || listener.member != member
                        {
                            return Err(Refusal::StaleListenerLease);
                        }
                        Ok(Route::PublicListener {
                            output: output.output,
                            member,
                        })
                    }
                }
            }
            Track::Private(owner) => {
                let Audience::Member(member) = audience else {
                    return Err(Refusal::PrivateRouteDenied);
                };
                let listener = listener.ok_or(Refusal::StaleListenerLease)?;
                if !listener.active
                    || listener.generation != self.generation
                    || listener.member != member
                {
                    return Err(Refusal::StaleListenerLease);
                }
                if member != owner {
                    return Err(Refusal::PrivateRouteDenied);
                }
                if !listener.unlocked {
                    return Err(Refusal::LockedListener);
                }
                Ok(Route::PrivateListener { member })
            }
        }
    }
}

fn main() {
    let current = InputLease {
        principal: 88,
        member: 7,
        binding: 9,
        generation: 3,
        active: true,
    };
    let mut actor = SessionActor {
        revision: 0,
        input_lease: current,
        receipts: [None, None],
        private_fact: "member 7 alone hears the whisper",
    };
    assert_eq!(actor.submit(current, 1, 41), Ok(Submit::Committed(1)));
    assert_eq!(actor.submit(current, 2, 42), Ok(Submit::Committed(2)));
    assert_eq!(
        actor.submit(current, 1, 41),
        Ok(Submit::PreviouslyCommitted(1))
    );
    assert_eq!(actor.revision, 2);
    assert_eq!(
        actor.submit(current, 1, 99),
        Err(Refusal::OperationFingerprintConflict)
    );
    let stale = InputLease {
        generation: 2,
        ..current
    };
    assert_eq!(actor.submit(stale, 3, 43), Err(Refusal::StaleInputLease));
    assert_eq!(
        actor.project(Audience::Member(7)),
        Ok("member 7 alone hears the whisper")
    );
    assert_eq!(
        actor.project(Audience::Member(8)),
        Err(Refusal::PrivateRouteDenied)
    );
    assert_eq!(
        actor.project(Audience::Shared),
        Err(Refusal::PrivateRouteDenied)
    );

    actor.input_lease = InputLease {
        binding: 10,
        generation: 4,
        ..current
    };
    assert_eq!(actor.submit(current, 5, 45), Err(Refusal::StaleInputLease));
    let replacement = actor.input_lease;
    assert_eq!(
        actor.submit(replacement, 1, 41),
        Ok(Submit::PreviouslyCommitted(1))
    );

    let old_output = OutputLease {
        output: 5,
        generation: 3,
        active: true,
    };
    let topology = AudioTopology {
        generation: 3,
        output: old_output,
    };
    let listener = ListenerLease {
        member: 7,
        generation: 3,
        active: true,
        unlocked: true,
    };
    assert_eq!(
        topology.route(
            Track::Public,
            Audience::Member(7),
            old_output,
            Some(listener)
        ),
        Ok(Route::PublicListener {
            output: 5,
            member: 7
        })
    );
    assert_eq!(
        topology.route(
            Track::Private(7),
            Audience::Member(7),
            old_output,
            Some(listener)
        ),
        Ok(Route::PrivateListener { member: 7 })
    );
    assert_eq!(
        topology.route(Track::Private(7), Audience::Shared, old_output, None),
        Err(Refusal::PrivateRouteDenied)
    );
    assert_eq!(
        topology.route(
            Track::Private(7),
            Audience::Member(8),
            old_output,
            Some(ListenerLease {
                member: 8,
                unlocked: false,
                ..listener
            })
        ),
        Err(Refusal::PrivateRouteDenied)
    );
    assert_eq!(
        topology.route(
            Track::Private(7),
            Audience::Member(7),
            old_output,
            Some(ListenerLease {
                unlocked: false,
                ..listener
            })
        ),
        Err(Refusal::LockedListener)
    );
    assert_eq!(
        topology.route(
            Track::Private(7),
            Audience::Member(7),
            old_output,
            Some(ListenerLease {
                active: false,
                ..listener
            })
        ),
        Err(Refusal::StaleListenerLease)
    );
    assert_eq!(
        topology.route(
            Track::Private(7),
            Audience::Member(7),
            old_output,
            Some(ListenerLease {
                generation: 2,
                ..listener
            })
        ),
        Err(Refusal::StaleListenerLease)
    );
    assert_eq!(
        topology.route(Track::Public, Audience::Shared, old_output, None),
        Ok(Route::PublicRoomOutput { output: 5 })
    );
    assert_eq!(
        topology.route(
            Track::Public,
            Audience::Shared,
            OutputLease {
                active: false,
                ..old_output
            },
            None
        ),
        Err(Refusal::StaleOutputLease)
    );
    let replaced = AudioTopology {
        generation: 4,
        output: OutputLease {
            generation: 4,
            ..old_output
        },
    };
    assert_eq!(
        replaced.route(Track::Public, Audience::Shared, old_output, None),
        Err(Refusal::StaleOutputLease)
    );

    let mut full = SessionActor {
        revision: 0,
        input_lease: current,
        receipts: [None, None],
        private_fact: "private",
    };
    assert_eq!(full.submit(current, 1, 1), Ok(Submit::Committed(1)));
    assert_eq!(full.submit(current, 2, 2), Ok(Submit::Committed(2)));
    assert_eq!(full.submit(current, 3, 3), Err(Refusal::ReceiptLedgerFull));
    assert_eq!(
        full.submit(current, 1, 1),
        Ok(Submit::PreviouslyCommitted(1))
    );
    assert_eq!(full.revision, 2);
}
```

## Failure semantics and production gates

Admission/authentication failure is a typed refusal and cannot reach the actor. Stale/revoked binding rejects new input and terminates the affected sensitive stream; an already committed operation remains committed. A matching operation identity resolves to the prior committed receipt even after intervening operations; a different fingerprint for the same identity is rejected. The durable store must not evict an accepted receipt and then accept that identity as new. If the configured durable retention/tombstone mechanism cannot safely preserve that fact, admission fails closed. Storage/fence failure prevents authoritative commit; after uncertain transport, lookup is required before retry. Slow consumers are bounded and resynchronize from a current authorized snapshot without blocking other sessions. Private-route failure remains private and uses authorized text/captions. No disconnect-driven NPC takeover, reroll, duplicate reward, fabricated success, or wall-clock turn deadline is permitted.

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
