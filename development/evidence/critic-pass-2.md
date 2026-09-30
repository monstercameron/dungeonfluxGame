# Independent adversarial refinement — pass 2

**Verdict: FAIL on three reproducible database guard bundles.** Planning coverage and substantive architecture pass the scoped rubric. No game executable exists; no native/WASM/game/browser/audio acceptance was performed or implied.

## Frozen candidate reviewed

- Manifest SHA256: `4dfe2801f284a225d521234b5712cf07ef458430cc8367ee3064cd9d3f77b630`
- Schema SHA256: `8796fa9be1ea05fbee7de486cd0cd71bb4b240cbb34bc477b385460f8a5e98c5`
- Source fingerprint: `75e2bb7f366bca756a9e46faef82f6174918cb16ab83a1fa1e580c98be06241e`
- Actual DB schema version 2, integrity `ok`, foreign-key check empty, WAL.
- Independent executable probe source/results: `work/critic-pass-2/probes.py` and `probes.json`.
- Independent parity/model/DAG results: `work/critic-pass-2/parity.json`.

Computer-use/vision tools are available to this independent frontier runner. Actual capabilities used were read-only source/database inspection, Python/SQLite executable fixtures and graph/model/contract analysis. This is an internal boundary, with no user-facing application to operate. Browser, physical-device or audio playback checks remain future feature gates.

## Executed result

23 independently authored SQLite probes ran against an in-memory backup of the actual live database, never mutating live workflow state. 17 gave the expected result, including one valid reviewed/integrated completion; **six invalid mutations were accepted**. Earlier B1 terminal insertion, B2 self-approval mutation, B3 nullable IDs and cross-task current-attempt linking are repaired. Cycle, dangling-edge, append-only history, independent identity/capability/evidence and build-mismatch denial work in their tested paths.

### B5 — lease values can bypass fencing

Reproduction (each on its own valid active/current implementation attempt): set `lease_expires_at='not-a-date'`, or `lease_token=''`, or `lease_owner=NULL`; update status submitted. All three succeeded. `julianday('not-a-date')` returns NULL, and the existing <=now disjunction does not reject it. The empty token and missing owner do not represent tracked phase ownership.

Affected: schema `attempt_fenced_submission`/`verdict_current_phase_fence`, F33, df-workflow. Required correction: require nonempty owner/token and parseable positive future expiry on new submissions/reviews/integration; reject NULL date conversions explicitly. Preserve documented valid delayed bookkeeping after completed integration without indefinitely renewing a terminal lease. Trusted runner identity binding remains separately required.

### B6 — phase/status inconsistency bypasses one-live ownership and integration phase

Two distinct probes succeeded:

1. An approved attempt still in `phase='review'` was updated directly to integrated with matched build/evidence, then its task done; no integration phase/owner transition was required.
2. An active implementation attempt was changed to `phase='terminal'` while status stayed active; a second active implementation row for the same task then inserted successfully because the unique live index excludes terminal phase.

Affected: schema attempt lifecycle/live index and ADR0001 tracked phase requirements. Correction: enforce coherent allowed phase/status combinations and valid transition ownership; never make a nonterminal status invisible to one-live uniqueness merely by changing phase. New integrated status must traverse actual integration ownership. Retain a valid submitted candidate during evaluator replacement rather than duplicating its implementing attempt.

### B7 — current acceptance can change after review and close without corresponding evidence

A valid independently reviewed/integrated attempt proves criterion `c1`. Update its task acceptance array from `["c1"]` to `["c1","new-required"]`, then mark done. It succeeds even though the new required criterion has no evidence. Approval validation only runs when changing an attempt; the final task transition does not revalidate current criteria/capabilities or establish applicability to the preserved reviewed contract.

Affected: schema completion, F33, task/attempt preservation and ADR0005 evidence. Correction: freeze criterion/capability/contract identity during a claimed attempt (explicit rescope requires a fresh attempt/review), or independently revalidate complete current criterion/capability coverage at final done and bind the attempt's frozen brief to that contract. Add post-review task-contract change regression; changing a current contract cannot reuse stale approval.

These are routine invalid-state guard gaps, not claims that SQL can authenticate true screenshots, model capability or executable test content. Scoped trusted coordinator commands and actual independent evaluation still must establish those facts.

## Planning rubric: PASS

- All 40 distinct crate model inventories, F01–F44, R01–R17, G01–G12, S00–S08 and commercial/competitive/provider/quality crosscuts are represented. DB matches manifest fields and dependencies; all 32 source-document hashes match. Manifest has 274 blueprint tasks; actual DB has one additional separately authorized `SITE-GODADDY-DNS` operational task, which is not a missing system or duplicate plan. Manifest graph is acyclic; 40-crate graph is acyclic; all seven browser crates have no prohibited server dependency transitively.
- New runtime models are meaningful, rather than empty count entries: world/time/schedules/threats; scoped observations/beliefs/memories/rumors; intent disposition/compound plan progress; NPC relationships/topics/secrets/obligations; authored beats/alternative reachability/threads/hooks; encounter objectives/escape/challenge budget; rules-limited tactical proposals; consented macro spotlight; bounded tempo inertia/fatigue/curves; permitted presentation and forecast demand. Their records are defined once in df-model, policies in df-content, typed pure boundaries in runtime-directors, source fidelity in df-rules, single staged transition in df-engine and sole authoritative fenced Postgres commit in df-session.
- Recovery preserves chosen semantic decisions, ordered draws, policy/source versions and compatible snapshot/decision reducers; unsupported replay versions explicitly remain gaps. No LLM/paid generation is used to recreate accepted past outcomes. Directors have no separate DB writers, real-time loop or recursive cross-calls. Bounded continuations and explicit pending choices/reactions/rulings avoid unlimited policy iteration.
- Privacy reaches deeper than view DTO filtering: NPC perception scopes, secret topic policies, private character hooks, rumor evidence and hidden-state noninterference extend to tempo/music/camera/profile/assets/provider context. Freeform intent, jokes/questions/ASR partials cannot authorize actions. Compound steps revalidate and can commit partial outcomes; social example DC/modifiers do not become invented standard D&D rules.
- AssetEngine remains df-media with durable complete publication owned by df-assets, generation through existing provider ports and immutable canonical identity revisions. Demand/queue/priority/fairness/dependency invalidation/spend/unknown outcomes are explicit. Prepared/replay misses do not go live; optional speculative cinema cannot consume unapproved allowance or block action/speech. Stable-clause safety and barge-in require their actual acceptance gate, otherwise complete validated text is buffered.
- F/S overlap is now explicitly resolved through one canonical bounded child, wiring-only COMPOSE, a join worked example and a required frozen canonical_child_map before dispatch. Slice prerequisites include applicable crate boundaries; S01 does not wait for the entire late catalog/feature set. All blueprint dispatch flags remain false; broad family rows must become exact scope/path/hook/check/budget/capability briefs before execution.
- G10–G12 own concrete authoring/replay, causal/social/knowledge bounds and tempo/device/privacy/cost calibration. G07 still requires actual book/catalog access, pinned sources/rights and denominator; source-linked fixtures and full integrated standard-rules coverage are unfinished. Optional fidelity/module cancellation is explicit and does not automatically satisfy dependencies.
- Cost/provider/market research stays qualification rather than a claimed moat: G08 depends PROVIDER-QUALIFY; F28 requires X09 price/wallet/API/authorization ownership; PROVIDER-BENCHMARK depends qualified routes, pricing resolution and accepted S03; COMP-PLAYTEST depends S03, dated competitor verification and pricing; COMP-OFFER waits playtest and accepted-output COGS benchmark. Research distinguishes vendor-reported throughput/first byte from first validated audible/visible output, minimums/currency/units, rights and account access, trial/retry/unknown spend and cache/relay economics. Product/competitor claims are not functional tests or product-market-fit proof. This review does not independently reprice all vendor pages; final model selection and commercial decisions explicitly require requalification.
- Model routing and tools remain actual-availability gated; frontier independent output evidence, two-failure escalation, resource/reserve/cleanup rules and exact build identity remain mandatory. Quality's personal helper is a separately tested result rather than completion of the future Rust workflow.

## Review limits

This pass validates the planning and actual SQL boundary and identifies the exact remaining blockers. Concrete protobuf tags/Rust compilation/OTLP-WASM integration, PostgreSQL physical layout/restores, provider quality/cost, standard catalog completeness, physical device/audio playtests and commercial demand are owned future work. The plan may be complete as a coverage blueprint while these outputs remain deliberately unfinished. No confidence score or count can waive them.
