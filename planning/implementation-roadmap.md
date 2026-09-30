# Integrated implementation roadmap

Date: 2026-09-29
Status: Planning SQLite provisioned; game runtime/workspace implementation pending

## Dispatch and ownership

Deliver complete, observable slices through the crate boundaries in
[Subsystem architecture](subsystem-architecture.md),
[Subsystem interfaces](subsystem-interfaces.md) and [RPC API](rpc-api.md).
The 47 families in [Feature inventory](feature-inventory.md) remain a broad
backlog; this roadmap does not imply 47 dispatched tasks or 40 empty crates.
Required standard rules exceed the legacy demo; [Rules coverage](rules-coverage.md)
is the source gate for that work.

The coordinator delegates each user work message or scoped atomic work-item
message to one implementing subagent owning the whole result. It does not split
that single item into parallel implementation agents. Separate independent work
messages may run concurrently when dependencies/edit areas/resources allow.
Independent frontier evaluation is a separate completion gate, not a split of
implementation. The coordinator resolves shared gates and integrates sequentially.

Create only the next useful dependency-ready tasks, roughly two to three per
available worker. A task brief names governing sections, fixed input revision,
owning crate(s), permitted edits, hook owners, normal/failure criteria and evidence.
Unresolved prerequisites block affected dispatch rather than invite invented APIs.
Maintain one active owner for protobuf/types, migrations, manifests/composition,
shared UI foundations, source catalogs and build output registration. A slice may
have sequential atomic messages; consumers wait for shared-owner acceptance.

## Prerequisite decisions

| Gate | Owner / output required before dependent implementation |
| --- | --- |
| G01 Toolchain/targets | coordinator + build owner: stable toolchain/language edition/lockfile, native and WASM features, Rust framework candidates and concrete fmt/Clippy/compile commands |
| G02 Transport/runtime | df-rpc-bridge/df-protocol owner: real browser HTTP/2/gRPC driver and executor compatibility, binary WebSocket framing/bounds, all four modes/status/deadline/cancel/half-close; no unsafe browser Send workaround |
| G03 Shared contracts | df-types/df-model/df-protocol owner: concrete typed enums/IDs/errors/offer/view/stream structures, protobuf numbering/version fixtures, effect registration coverage and pure audience scope |
| G04 Identity/devices | df-auth + client owner: credential/bootstrap storage/admission/origin policy, guest/allocation/credential/binding dedupe, bounded stream revocation, host/operator capability rules, supported browsers/phones/tablets/display targets and codecs/capture constraints |
| G05 Durable state/assets | df-persistence/df-assets owner: PostgreSQL physical schema/access-pattern choice, fenced commit/allocation/idempotency retention, migrations/recovery/backups; initial durable byte root and publication/hash/access policy |
| G06 Telemetry | df-observe/df-telemetry owner: supported native/WASM SDK/export path, bounded upload/receipt and TelemetryIngress ports, OTLP/SQLite mapping, authenticated read-only queries, spool/retention/capture boundary and recovery envelope |
| G07 Rules/content | df-content/df-rules owner: source/catalog/errata manifest under rules-coverage.md, chosen launch campaign, legal early subset labeled as such, rules/content revision preservation |
| G08 Providers/presentation budgets | df-provider-api/df-media + client owner: vendors/models and bounded cost/deadline/concurrency policy, execution modes, recording identities, fallback assets, audio/device formats, initial capacity/latency/frame/memory targets |
| G09 Agent runner/evidence | coordinator/df-workflow owner: actual model/tool availability and routing preferences, independent frontier computer-use/vision evaluator and audio observation, tracked phase leases/recovery/edit/resource limits, durable evidence access |
| G10 Campaign authoring and replay | df-model/df-content/df-engine + storage owner: versioned director state/authoring schemas, beat/asset DAG and references/alternative reachability, logical-time/pause rules, ordered decision/semantic/draw provenance, compatible deterministic reducer/migration and replay/hash contract; unsupported versions explicit |
| G11 Causal interaction and bounded directors | df-world/df-knowledge/df-intent/df-interaction/df-narrative/df-encounter/df-combat owners: source-grounded social/physical rulings, epistemic privacy/witness/rumor/memory policy, actor decision/event/candidate/plan bounds, NPC perception/tactics, agency/refusal and compound-step cancellation/partial outcomes |
| G12 Experience, tempo and predictive assets | df-experience/df-tempo/df-presentation/df-media + device/cost owner: permitted pacing observations/consent, versioned intensity/cue/asset-demand APIs, accessibility/audio/UI budgets, hidden-state noninterference, inertia/fatigue/stem timing, fair prefetch/expiry/reservations/waste and measured session/device/cost calibration; stable-clause/barge-in safety |

Select concrete bounds from representative devices/load and verify them; proposed
frameworks/codecs/vendor names are not frozen just because they were researched.
G01/G03/G04/G08 also resolve the persistent Rust/WASM component implementation,
typed presentation variants/cue policies, actual device matrix and frame/input/
memory/audio-drift budgets in [Client presentation](client-presentation.md).
Do not select an unrestricted UI interpreter or a new RPC service to satisfy
ordinary reusable components/in-place updates. The newly requested server
`df-presentation` crate is the semantic director boundary in
[Runtime directors](runtime-directors.md); existing Rust UI crates retain mounted
component/render ownership. Ten runtime additions bring the plan to 40 crates.
G10/G11/G12 block only their affected work, not initial transport/identity delivery.
Initial hosting/auth provider, exact locales, renderer fidelity and advanced optional
rules remain decisions, with dependency gates limited to affected work. The legacy
Go bridge's compatibility is optional unless explicitly selected; preserve its MIT
attribution when translating/reusing code. No dependency installation is authorized
by writing this plan.

## Delivery slices

| Slice | Capability and prerequisites | Implementation/composition owners | Meaningful completion checks |
| --- | --- | --- | --- |
| S00 Contract and execution foundation | G01–G03; start F22/F23/F31/F32 | df-types, df-protocol, df-observe, df-rpc-bridge, minimal df-testkit/df-tools; df-server composition owner | real Rust/WASM browser demonstrates unary/server/client/bidi streaming with errors/trailers, half-close, deadlines and cancellation; bounded flow during simultaneous traffic; target/style gates and dependency checks |
| S01 First durable two-role slice | S00 + applicable G04–G07/G09; F01–F03/F07/F22–F25/F30/F31 | df-auth, df-model/df-engine, df-session, df-persistence, df-api, df-client/ui/player/display/web, df-telemetry; shared wiring by df-server | create/join/ready in display + player; initial idle and same-phase views; rejected action/draft; private projection/asset denial; duplicate/lost-response operation lookup; reconnect/rebind without another member; PostgreSQL restart/fence conflict; end-to-end OTEL query in SQLite |
| S02 Creation and first source-faithful rules | S01 + G07/G08 where portraits/flavor needed; F04/F05/F07/F10 | df-content, df-rules, df-engine; df-api projection, player/display, prepared df-assets/media | legal source-linked build/choices and complete sheet, deterministic check/save, explicit unsupported options, late/failing portrait fallback; accepted results/resources survive restart; rules-source and view correspondence |
| S03 Story, dialogue, voice and audio | S02 + G08/G10 and applicable G11/G12; F06/F08/F09/F15–F17/F25–F28/F35–F39/F42–F44 | df-engine/content, df-world/knowledge/intent/interaction/narrative/experience/tempo/presentation, df-provider-api/df-providers, df-ai, df-media, df-assets, df-api, df-audio/render/player/display; df-server registers every effect | complete scene/NPC flow, typed rejection and microphone fallback; validate before captions/TTS; Talk half-close/cancel; actual audible timed playback and old-buffer cancellation; matching fallback speech; prepared-only/replay make zero live calls on misses; exact usage/complete recording checks |
| S04 Combat, spells and tactical rendering | S02 rules foundations plus S03 for integrated narration, applicable G10/G11/G12; F07/F10–F13/F17/F40/F41 | df-rules/df-encounter/df-combat/df-engine/content, df-session/API, df-render/player/display/audio | initiative, movement/path/visibility, weapon/spell actions, reaction/concentration/condition/health flow, enemy legal inputs, encounter finish; relevant rules interactions and private info; real both-role flow without debug skip; narration cannot decide outcomes |
| S05 Campaign continuity and controls | S03/S04 + G10; F05/F18–F21/F24/F27–F29/F35/F36/F38/F39 | df-rules/engine/session/persistence, df-locale/content, df-ui/API/player/display, df-tools | advancement/rest/resources/rewards and durable journal; public/own knowledge filtering; host-authorized pause/resume/new run/timers/execution mode; reset cancels old owned jobs, request loss does not; locale captions/audio fallback; authorized checkpoints/debug commands/capture |
| S06 Complete required rules/catalog | S02–S05 + complete G07 manifest; expands F04/F05/F10–F12/F20 | df-rules/df-content, coordinated engine/protocol projections and source owner | every required R01–R17 family/catalog entry reaches source-linked implementation/interaction/integration gate; catalog denominator and remaining gaps visible; unsupported options cannot count as support; integrated frontier rules-flow review |
| S07 Conditional fidelity | after reliable flat/audio flow; selected F14/F15 scope + device/assets budgets | df-render/media/assets/tools/display/player | optional splat/3D/LOD/camera/loops/clips share committed scene/timeline; pre-first-frame flat/still fallback, identity consistency, late assets/deduping/cleanup, device frame/byte budgets; no authored JS renderer |
| S08 Delivery and developer operations | grows from S00 onward; final F29–F34 acceptance | df-tools/df-testkit/df-workflow/df-telemetry, df-server, docs/ and about panel | atomic build registration and stable previews; typed operator CLI/fault/capture/telemetry evidence; PostgreSQL migration/backup restore; five-table queue/lease/devlog/escalation/evaluator gates; resource-aware scheduling/cleanup; six-hour all-commit changelog; public docs/attribution |

S00 uses an isolated transport fixture; its success does not claim a production
game server is ready without PostgreSQL, authorized content and required executors.
Freeze only the shared definitions needed by the next slice; introduce `df-model`
when those domain definitions become prerequisites rather than empty scaffolding.

S08 is an incremental track, not a reason to delay observability, testability or
output review until the end. A manual coordinator follows the ADRs before the Rust
workflow runner exists. The public project site remains `docs/`, outside the game
crate graph. S07's selection is independent of complete standard rules support;
unselected fidelity ideas remain conditional and cannot conceal required gaps.

## Gap-refinement integration

[Gap analysis](gap-analysis.md) maps all 76 research backlog rows and corrects the
historical priority labels. X10 resolves private campaign package/template/import
production through df-content/df-tools; the bounded authoring path and real
activation/resume acceptance follow G07/G10, without blocking initial transport.
Hosted remote/mixed-room play is now required F45 core under the later feature
outline. X11 retains conditional async/community/extra-import/broader-homebrew
scope, while F47 plans explicitly disclosed custom-content opt-in. These plans
do not claim runtime implementation. Required progression/inventory
remains S05/S06. Existing F35/F36/F09/R09 families gain WORLD-TIME-ACCEPT,
MEMORY-LONGHORIZON-ACCEPT, SPEECH-ROOM-ACCEPT and RULE-EFFECT-CONTRACT refinement
plans; they exercise the same canonical implementations, not duplicate owners.
See [Campaign authoring](campaign-authoring.md), [Long-horizon state](long-horizon-state.md),
[Rules effects](rules-effect-model.md) and [Expansion boundaries](expansion-boundaries.md).

## New feature priority overlay

[Feature refinement](feature-refinement.md) maps every numbered idea, priority,
required table stake and product phase onto this single roadmap. F45 adds hosted
remote/mixed-room core: S01 remote join/snapshot/reconnect, S03 real private/public
capture/audio topology, S05 campaign resume and source-valid concurrent input.
G04/G08 freeze relevant admission/network/AFK/device/usage bounds in slice order;
full remote voice acceptance does not become a prerequisite cycle before S01.

F46 adds committed observer-safe recaps and clearly speculative spoiler-safe
trailers through journal/presentation/assets/audio after S05 continuity. F47 adds
source-compatible personalized templates and explicitly disclosed custom ruleset/
handler admission; standard 2024 mechanics/catalog completion remains mandatory.
Four targeted acceptance plans extend private phone cues, critical-event cosmetic
escalation, consented spotlight and committed world/item/media continuity.

P0 cinematic eligibility/identity/fallback contracts start in S03/S04. Paid live
video fidelity remains optional, budgeted and tested in S07 after a reliable legal
campaign and measured cost/latency. Cosmetic pauses never change mechanical
reaction/resource/timer authority. Async/public community/marketplace remain
conditional. No second phase roadmap or duplicate implementation owners are added.

## Runtime director delivery within existing slices

S03 first integrates a bounded authored arc with alternative disclosures, one
credible NPC/topic/secret/obligation, source-valid affordance/intent/compound plan,
world-time/threat/schedule, observer knowledge/provenance, macro pacing and
accessible anchored tempo/presentation. Preserve declined opportunities and
player agency. Expand catalogs/scenarios only after the full causal commit/view/
audio path passes; F35–F44 are broad plans, not a request for ten empty skeletons.

S04 adds source-legal encounter objectives/escape states and perception-limited
NPC tactics, reinforcement/hazard requests resolved by the rules owner. Test
reactions/resources and inaccessible hidden knowledge. S05 adds campaign-scale
bounded memory/rumor/thread continuity and snapshot/decision replay with source/
policy versions; replay makes zero provider calls and reports unsupported gaps.
S07 introduces optional higher-fidelity assets/cinematics only after cheap matching
fallbacks, canonical identity, fair bounded prefetch, reserved allowances and
measured waste. Music/SFX/stems and speech safety are core S03 work, not deferred
until 3D exists. S08 includes model/privacy/replay/performance/output acceptance
for all 41 crates. No new subsystem owns a parallel database/timer/provider loop.

## First-slice failure envelope

S01 proves the entire path before broad parallel delivery: RPC -> authorization
-> session owner -> PostgreSQL committed decision -> permitted projection -> real
player/display -> correlated OTEL -> separate SQLite query. Include actual
TelemetryIngress wiring, ingest watermarks/loss visibility and read-only operator
access. Inject temporary telemetry sink failure without freezing gameplay.

Test RPC cancellation/lost receipt after commit while an identified owned effect
continues and completes once. Also test unaccepted input/capture cancellation,
stale run/timer/job callbacks, owner fencing, duplicate allocation/action/credential/
binding writes, revocation during active subscriptions/transfers,
strict destructive preconditions and still-legal offers after unrelated updates.
No fake gameplay storage or no-op registered executor proves this slice. A small
faithful prepared effect is sufficient; paid providers are unnecessary here.
Retired allocation/operation keys must reject rather than create fresh work.
As tabletop adjudication enters scope, test authorized pending-resolution offers,
scoped ruling persistence and visible failure when no authorized ruling is available.

Inspect exact source/native/WASM/config/content/asset identity, screenshots and
interaction outcomes. A desktop viewport does not establish physical phone sleep,
microphone or audible audio compatibility; run the chosen target checks as their
features enter scope. Missing capabilities keep acceptance pending under
[ADR 0005](../ADR/0005-frontier-output-evaluation.md).

S01's browser evidence must also show a persistent mounted shell in both roles:
same-phase/phase updates, tabs/overlays and reconnect require no full-page reload,
valid focus/drafts survive reconciliation, and repeated updates release replaced
resources without duplicate listeners/layers. S03 adds measured and actually
observed audio/caption timing, late assets and interrupted cues. S04/S07 exercise
overlapping motion, reduced motion, visibility recovery and renderer fallback on
the selected device matrix. Carry these requirements into atomic task briefs;
visual smoothness claims need observed interaction and frame-time evidence.

## Optimization and release completion

Once S01 is accepted, create bounded measured optimization work for every integrated
crate under [Optimization](optimization.md). As additional crates/features integrate,
add their passes; retain one 41-crate checklist so tooling/shared libraries and both
clients are not omitted. Profile representative whole-game load before rewriting;
an evidence-backed within-budget result can close a pass without speculative changes.
Repeat affected source/behavior/privacy/telemetry checks after optimization.

Final acceptance maps F01–F47 plus R01–R17 to integrated evidence, explicit selected
scope and unresolved gaps. Preserve meaningful failure checks, measured p95/p99
action-to-view under concurrent audio/assets, provider cost and browser/server
memory/load evidence. Restore from existing PostgreSQL data, verify migration and
telemetry retention/recovery, and operate the complete intended game through both
roles. Code review, catalog counts or native tests alone cannot close the feature.
The coordinator alone records completion after independent evaluation and checked
integration; two unsuccessful economical attempts trigger the existing escalation.

## Service release stages and expensive prerequisites

The full target is now **41 crates, F01–F47, R01–R17, twelve services/41 RPCs**;
commercial authority does not shrink any game/director/remote/phone/cinema requirement.
`DesignReady` requires independent source-bound repair review and corpus consistency,
not production proof. `ExecutableReady` requires G01–G12 applicable library/source/
rights/type/transport choices and native/WASM fixtures before dispatch. `PrivatePilot`
requires source-valid integrated output, isolation, honest ruling/fallback, recovery
and scoped customer consent; supported subsets are labeled and cannot close full game.
`PaidRelease` additionally requires full required rights+coverage, X12 account/payment/
entitlement/ledger, ASR security/effect/deletion/restore/transport/speech/ruling evidence,
measured service bounds and ownership/on-call, funded worst-case supplier exposure,
price/cohort support/CAC/retention evidence. `IncomeValidated` requires actual recognized
revenue, invoices/COGS/support/acquisition/churn and operating cash against the selected
target, independent evidence and ongoing monitoring. Document scores cannot close it.

Next high-risk proof order: full-catalog commercial rights feasibility and browser
native-gRPC/WASM bound proof precede irreversible broad implementation; then shared
types/source fixtures and initial pure integration; accounts/admission and bounded
native effect execution integrate before live paid assets; all47 families/rules and
crate optimization follow existing slices, then full release evidence. Candidate
planning budgets: rights/source feasibility5person-days plus capped external counsel
quote; transport proof5person-days/$0paid calls; initial integration10person-days;
commerce/dispatch/isolation10person-days; recovery/real-device private pilot5person-days.
These are uncertainty estimates (roughly0.5–3x), not a delivery commitment or authority
to purchase counsel, call models or contact customers. At each failure reassess runway
and resolve mechanism/rights before next expensive tranche; don't redefine finalscope.

The twelve ASR review defects are mapped to concrete contracts/tasks in the service
refinement evidence. Independent review records weighted design separately from
observed evidence, source identity and finite remaining blockers. Full service design
can score highly while feasibility, rights, users and revenue remain unproven.
