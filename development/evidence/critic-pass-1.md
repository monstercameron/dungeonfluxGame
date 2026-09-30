# Independent adversarial plan critic — pass 1

Verdict: **FAIL pending concrete database guard repairs and canonical blueprint expansion policy.** This is an internal executable artifact review. No game implementation exists and no game/native/WASM/browser/audio behavior was tested.

## Candidate and capabilities

Initial schema SHA256 `d569dddf28a0a9b6e497c8a31425767e26fadf5dbc4e137ed646a69d5feae88c`; initial database manifest read changed while pricing integration was ongoing. Independent probe capture later records manifest SHA256 `1751974b5c985688ca61fe2b1d1db3267038b67f6d812afcb835121c1d72c6b4`, schema-file SHA256 `82ba9103441f4f07c3940f4e4e77fccb5f1fa2f3539a385ec0e640adea824aa0`, source fingerprint `45910df8dd2d4c0728935e903b8ad2181cbb5133ead8348212b07d268047c1d6`. Candidate changes during review mean these are pass-1 findings, not a final immutable attestation. Final pass must freeze and reverify hashes/manifest/database parity.

Used independent Python/SQLite execution, read-only live database opened by URI and backed up to an in-memory database, document/model/API inspection, and dependency analysis. All negative mutation probes run in scratch memory with rollback. The inherited frontier runner exposes computer-use and image tools, but these were not needed for a nonvisual database boundary; no browser screenshot or playback capability was used or claimed. Critic does not change queue state, attempt records or devlog. A future independent running feature review must follow ADR0005.

## Executed checks and planning rubric

- **PASS** live SQLite integrity, foreign-key check, WAL mode; five intended tables. Mutation probes deny cycles, dangling prerequisites, direct-done tasks, unfinished-feature completion, duplicate live attempts, stale update submission and devlog update/delete.
- **PASS planning coverage** all 30 crate model ledger entries, F01–F34, R01–R17, G01–G09 and S00–S08 exist with distinct ownership. This conclusion includes review of inputs/types, public boundaries and full design documents rather than counts alone.
- **PASS substantive contract direction** session owner/fenced PostgreSQL commit, actual decision receipts and unknown-outcome lookup; durable effects independent of RPC cancellation; authorization derived separately from trace context; bounded revoked publications; private projections before serialization; deterministic rules and explicit ruling; partial/half-close/error RPC and stream sequence/epoch/cancel semantics; immutable complete asset publication; validated narration before speech; prepared/replay no-live modes; provider unknown spend and durable reservations; complete contract-keyed recordings; typed allowlisted presentation, keyed components and owned resources; OTEL ingest/recovery/read-only evidence; staged crate refinements and measured all-crate optimization.
- **PASS external prerequisite honesty** book access/catalog/errata denominator remains G07; browser transport/executor remains G02; devices/codecs G04/G08; native/WASM telemetry G06; concrete load/budget/vendor/host choices remain measured resolution gates. No source-backed standard-rules completion, compatible tonic browser runtime, unlimited capacity or provider access is claimed. Pricing research is a dated scenario, not a validated launch decision.
- **PASS routing honesty** economical model names are preferences conditional on availability; actual capability checks and frontier evidence review remain required; two unsuccessful Luna/Terra/Muse attempts escalate without resetting task identity. Missing device/audio tools keep future acceptance inconclusive. Four currently available concurrency slots do not pretend to provide hundreds.
- **PASS bounded detail** family/catalog blueprints do not explode into invented per-spell tasks. `dispatch_ready=false` differentiates coverage planning from a concrete executable brief. Generic crosscutting prose is acceptable only alongside owning model inventories and governing concrete contract sections; dispatch still must freeze exact scope, paths, checks, limits, hooks and capabilities.
- **INCONCLUSIVE final mutable-candidate consistency** pricing/quality amendments underway. Source hash check later found no mismatch on 25 source documents, but final regenerated verification and SQL/database parity must be rechecked after edits stop.

## Blocking findings

### B1: INSERT bypass of integration/lease fencing (initial schema)

`attempt_fenced_submission` guards UPDATE only. On an in-memory live backup, create a review task with lease generation 7; INSERT worker attempt already `integrated`, verdict approve, independent evaluator, matching tested/integrated revision, nonempty evidence, **NULL lease token and expired lease**, phase terminal; attach active attempt; update task done. Result: **done accepted**. Thus a routine invalid insertion bypasses the advertised pipeline. This does not rely on forged screenshot truth; it exploits omitted lifecycle guards.

Affected: `development/schema.sql`, `F33-DELIVER`, `df-workflow`, ADR0001 evidence/fencing. Correction: begin implementation attempts in the valid active phase with nonempty lease identity, prohibit direct submitted/approved/integrated insertion and test this transition. Parent's mid-review repair now rejects this probe; verify final schema from scratch and live DB, not just file text.

### B2: approval fields can invalidate independence after approval

Create an approved attempt then `UPDATE attempts SET evaluator_id=worker` without changing `verdict` or `status`. Result: stored self-approved attempt accepted. Current approval trigger watches UPDATE OF verdict,status only. Completion currently prevents the exact worker=evaluator case, but the stored approval invariant is already false and other evidence/capability fields can be changed outside the validation event.

Affected: same schema/workflow/evaluation plans. Correction: validate all updates to an approved record (or make relevant submitted/approved identity/evidence immutable with explicit replacement review); prohibit empty evaluator model/tested identity, require independent evaluator and appropriate capability evidence. Extend negative tests to change identity/model/tested/evidence/capabilities **after** approve. Future trusted runner must separately bind actual identity and evidence content.

### B3: stable primary-key IDs may be NULL

SQLite ordinary `TEXT PRIMARY KEY` does not imply NOT NULL. In-memory inserts of copied valid feature and task with `id=NULL` both succeeded. Stable IDs, deduplication and provenance must not permit these records. Correction: explicit NOT NULL plus nonempty validated ID constraints for all five primary IDs and checked dependency IDs; preserve existing history through schema migration. Negative tests must cover NULL and whitespace IDs on each table. Enforce manifest IDs separately as well.

### B4: feature/slice implementation blueprints may expand duplicate work

`S01-COMPOSE` depends on gates plus S00 acceptance, while `F01-DELIVER` independently specifies the same guest/session/member allocation, join and readiness implementation. Similar overlaps span F22/F30 and S00/S01. No canonical child mapping or explicit deduplication ownership says that the one implementation result is reused by both views. A coordinator expanding both blueprint families can create duplicate owners/tasks despite the single-message policy.

Correction: record a canonical bounded work identity/scope for expansion, look up/reuse existing child work before insertion, link that work into feature and slice acceptance; make COMPOSE wiring-only for already assigned behavior or explicitly own a selected child delivery with the feature blueprint referring to it. No extra table needed: existing task IDs, originating-task links, dependencies and brief mappings suffice. Stage required crate-boundary refinement before dependent consumers. Do not cure overlap by making S01 wait for full late feature coverage or all 30 crates. Add a concrete F01/S01 worked expansion example and prove the plan cannot dispatch duplicate same-scope children.

## Secondary corrections / integration checks

- Cross-task `tasks.active_attempt_id` link was accepted initially; parent repaired during review and reprobe rejects it. Retain same-task/current-generation guard plus final regression.
- Expired lease **after already completed integration** currently permits delayed task closure. This can be correct if integrated evidence, active attempt token/generation and ownership remain current: distinguish valid delayed bookkeeping from accepting expired new submissions/integrations. Do not require indefinite renewal of terminal evidence.
- Commercial X09 has quote/wallet/payment/webhook required work; review final amendment ensuring real `df-provider-api::BudgetStore`/authorization/API ownership and required implementation acceptance are assigned to F28 rather than just research-model existence. A quote model or profitable spreadsheet does not implement concurrent reservations, signature/replay-safe payment callbacks, refund policy or unknown supplier reconciliation. Final ownership must be frozen before billing consumers; no new billing crate required.
- Quality minute cadence is a separately provisioned capability. Check evidence for that operational result if claiming it completed; do not retroactively mark future Rust workflow F33 done.

## Reproduction

Run `python3 work/plan-database-refinement/critic-probes.py` from workspace root. It opens only read-only live workflow DB and all mutation probes execute in memory, writing `critic-probes.json` in scratch. Pass 1 script/results are an executable record; after repair, note that rejected newattempt setup can require adjusting the valid positive fixture before subsequent independent probes.

No remaining game runtime gates are waived by a successful planning review. Full implementation and source/catalog/device/provider acceptance remain unfinished.
