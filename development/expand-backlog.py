#!/usr/bin/env python3
"""Deterministic final planning refinement. Writes a candidate, never the SQLite queue."""
import argparse,copy,hashlib,json,collections,re
from pathlib import Path

def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
PHASE={'D':'design','R':'resolution','I':'implementation','W':'implementation','F':'acceptance','A':'acceptance','O':'optimization'}
PURE={'df-types','df-protocol','df-model','df-content','df-rules','df-world','df-knowledge','df-intent','df-interaction','df-narrative','df-encounter','df-combat','df-experience','df-tempo','df-presentation','df-engine','df-commerce'}
GATE_COMPONENTS={'G04':['df-auth','df-client'],'G05':['df-persistence','df-assets'],'G06':['df-observe','df-telemetry'],'G08':['df-provider-api','df-providers','df-media'],'G09':['df-workflow'],'G10':['df-content','df-tools'],'G11':['df-world','df-knowledge','df-intent','df-interaction','df-narrative'],'G12':['df-experience','df-tempo','df-presentation','df-media']}
RULE_USE={'F04':['R01','R02','R03'],'F05':['R02','R12','R16'],'F07':['R04','R05'],'F10':['R04'],'F11':['R05','R07','R08','R09','R10','R11'],'F12':['R06'],'F40':['R14'],'F41':['R15'],'F47':['R11','R12','R17']}
RULE_DEPS={'R02':['R01'],'R03':['R01'],'R05':['R04'],'R06':['R05'],'R07':['R04','R05','R06'],'R08':['R04','R05'],'R09':['R05'],'R10':['R04','R05','R06','R09'],'R11':['R05','R09','R10'],'R12':['R01'],'R13':['R02','R08'],'R14':['R04','R06'],'R15':['R05','R06','R07','R10','R11'],'R16':['R02','R12'],'R17':['R14']}
SLICE_FEATURES={'S00':['F22','F23','F31','F32'],'S01':['F01','F02','F03','F07','F24','F25','F30'],'S02':['F04','F05','F10'],'S03':['F06','F08','F09','F16','F17','F35','F36','F37','F38','F39','F43','F44'],'S04':['F11','F12','F13','F40','F41'],'S05':['F18','F20','F21'],'S06':[],'S07':['F14','F15'],'S08':['F29','F30','F31','F32','F33','F34']}
X_REUSE={'X01':['df-auth','df-persistence','df-assets','df-providers'],'X02':['df-server','df-persistence','df-telemetry'],'X03':['df-workflow'],'X04':['df-content','df-rules'],'X05':['df-tools','df-observe'],'X06':['df-workflow','df-tools'],'X07':['df-ui','df-client','df-audio'],'X08':['df-workflow'],'X09':['df-commerce','df-persistence'],'X10':['df-content','df-tools'],'X11':['df-client','df-world'],'X12':['df-commerce','df-auth','df-api','df-persistence','df-client','df-ui','df-web']}

# Historical accepted source is a required I01 dispatch input, not a seed task.
I01_ACCEPTED_SOURCE_RECEIPT = json.loads(r'''
{
  "accepted_source": {
    "integrated_commit": "9898c70614a8555efa63d3cec47ba9adbc395bbb",
    "integrated_revision": "9898c70614a8555efa63d3cec47ba9adbc395bbb",
    "missing_receipt_files": [],
    "original_acceptance": [
      "Distinct pure validated first-wave identity/provenance/recovery-revision contracts have bounded values, checked arithmetic and documented non-authority semantics without forbidden runtime or credential dependencies.",
      "Generated additive common protobuf messages have explicit presence and a durable field-number/name ledger; actual malformed/presence/unknown-required cases reject safely, optional additive cases remain compatible, and field reuse is rejected by a real schema boundary check.",
      "Actual consuming encode/decode mapping fixtures preserve identity and lexicographic recovery revision ordering, including newer epoch with lower sequence, zero/missing epochs, max sequence overflow and old scalar ambiguity rejection.",
      "Current source native/WASM format/lint/compile and focused contract tests pass; native/WASM artifact identities and real executed boundary results are retained with an independent frontier evaluator verdict for every criterion.",
      "Preserve experimental all-four-mode fixture and explain remaining fullG03 models/effects/audience/authorization/service and G02 physical/resource gates; no prototype claims production or entire project completion."
    ],
    "original_brief": {
      "acceptance": [
        "Distinct pure validated first-wave identity/provenance/recovery-revision contracts have bounded values, checked arithmetic and documented non-authority semantics without forbidden runtime or credential dependencies.",
        "Generated additive common protobuf messages have explicit presence and a durable field-number/name ledger; actual malformed/presence/unknown-required cases reject safely, optional additive cases remain compatible, and field reuse is rejected by a real schema boundary check.",
        "Actual consuming encode/decode mapping fixtures preserve identity and lexicographic recovery revision ordering, including newer epoch with lower sequence, zero/missing epochs, max sequence overflow and old scalar ambiguity rejection.",
        "Current source native/WASM format/lint/compile and focused contract tests pass; native/WASM artifact identities and real executed boundary results are retained with an independent frontier evaluator verdict for every criterion.",
        "Preserve experimental all-four-mode fixture and explain remaining fullG03 models/effects/audience/authorization/service and G02 physical/resource gates; no prototype claims production or entire project completion."
      ],
      "accepted_transport_atomic": {
        "completion": "development/evidence/qualify-g02/integration/completion.json",
        "full_G02": "INCONCLUSIVE; retained-tab recovery and required device/memory/network/audio/production bounds remainpending.",
        "task": "QUALIFY-G02-001",
        "tested_integrated_revision": "ba0d3860fb27124be352b48766e99a955f746672"
      },
      "actual_existing_schema": {
        "build": "crates/df-protocol/build.rs",
        "crate": "crates/df-protocol",
        "module": "transport_fixture",
        "next_wave": "Add common namespace/contracts without altering experimental transport schema or browser Rust source; actual mapping consumer limited to df-tools tests.",
        "package": "dungeonflux.experimental.transport.v1",
        "production_project_crate_dependencies": [],
        "proto": "crates/df-protocol/proto/transport_fixture.proto"
      },
      "attempt_id": "CONTRACT-G03-001-a1",
      "bounds": {
        "build_timeout_seconds": 600,
        "cargo_jobs": 2,
        "command_timeout_seconds": 180,
        "estimated_additional_peak_gib": 2,
        "isolation": "fresh source worktree from accepted integration; independent artifacts/build/contracts-g03 and tmp/attempt; preserve previews",
        "lease_hours": 3,
        "no_recursive_delegation": true,
        "owned_build": "/Users/earlcameron/Desktop/dungeonflux/artifacts/build/contracts-g03",
        "paid_calls": 0,
        "review_reserve_gib": 2,
        "scratch": "/Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/CONTRACT-G03-001-a1",
        "shared_cache": "/Users/earlcameron/Desktop/dungeonflux/artifacts/cache",
        "system_headroom_gib": 3.2
      },
      "bundled_python": "/Users/earlcameron/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/bin/python3",
      "candidate_input": "822ffc4992b34ab548a47cf91efad845ca50c716",
      "canonical_reuse": {
        "B-G03-D01": "first contract wave evidence",
        "B-G03-D02": "numbering ledger evidence",
        "B-G03-D03": "composite recovery encoding evidence",
        "B-G03-F01": "actual schema reuse rejection",
        "B-G03-F02": "older restore epoch ordering",
        "B-G03-F03": "pure dependency/credential exclusion",
        "B-G03-R01": "contract-wave evidence aggregation remains independent"
      },
      "coordinator_decisions": {
        "build_identity": "The first wave describes a paired native/WASM fixture build: source_revision, native_revision, wasm_revision, configuration_revision and content_revision are each required bounded provenance labels. Missing/empty values reject; labels cannot mint tested-build approval, credentials, content rights or a real game catalog. No optional fields concealed as default strings. Later runtime-specific identity shapes require explicit consuming-contract refinement.",
        "epoch_revision": "RecoveryEpoch is nonzero u64; SessionRevision is epoch plus u64 sequence, sequence0 valid. Checked next sequence errors at max; no epoch issuance/recovery authority. Required protobuf presence distinct from zero.",
        "ids": "Distinct opaque 16-byte nonzero values accepted as caller-supplied identity only. No random generation/authentication/public issuance mechanism in this wave.",
        "protocol": "public.v1 common messages additive; protocol revision1 supported, zero/missing/unsupported rejected by actual consuming boundary fixture. Capability requiredness explicit; unknown optional skipped, unknown required rejected, zero/unknown enum and missing oneof never manufacture required behavior. Preserve all allocated names/numbers with actual descriptor/schema boundary check.",
        "revision_label": "ASCII letters/digits and . _ - : / only, 1..128 bytes. Use a typed error for missing/empty/too-long/invalid-character cases; supplied labels confer no authority and must contain no credentials.",
        "units": "No speculative time/distance/money quantities in this first-wave schema. Numeric fields explicitly name dimensionless protocol revision and in-epoch sequence; local elapsed durations remain Rust Duration. Later consuming quantities require distinct units when first needed."
      },
      "dispatch_ready": true,
      "edit_paths": [
        "crates/df-types/",
        "crates/df-protocol/",
        "crates/df-tools/Cargo.toml",
        "crates/df-tools/tests/",
        "Cargo.toml",
        "Cargo.lock",
        "development/shared-contracts.md"
      ],
      "environment": {
        "DUNGEONFLUX_ARTIFACT_ROOT": "/Users/earlcameron/Desktop/dungeonflux/artifacts",
        "DUNGEONFLUX_BUILD_ROOT": "/Users/earlcameron/Desktop/dungeonflux/artifacts/build/contracts-g03",
        "DUNGEONFLUX_TMP_ROOT": "/Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/CONTRACT-G03-001-a1"
      },
      "evidence_root": "/Users/earlcameron/Desktop/dungeonflux/development/evidence/contracts-g03",
      "gate_policy": "This pure first-wave contract task neither completes fullG03 nor waives fullG02 physical/memory/production-client feasibility. Catalog/source/rights and later model/API waves remain required.",
      "governing_sections": {
        "planning/coding-style.md": [
          "all"
        ],
        "planning/implementation-roadmap.md": [
          "Prerequisite decisions G03",
          "S00"
        ],
        "planning/rpc-api.md": [
          "Protocol and access"
        ],
        "planning/service-operations.md": [
          "Recovery and restore"
        ],
        "planning/subsystem-architecture.md": [
          "Shared and transport crates",
          "Tooling and integration"
        ],
        "planning/subsystem-interfaces.md": [
          "Common contract rules",
          "Pure domain and content",
          "Persistence and assets"
        ]
      },
      "input_policy": "Re-read actual independently approved integration before fenced claim. G02 atomic source integrated and completion approved; retain fixed preview builds and full G02 gaps. Wait for current fixed-four-crate G01 audit integration before changing Cargo workspace inputs.",
      "input_revision": "f79bebf6f88f72f80bfb62d71798ebc56f64ca96",
      "integration_hooks": {
        "production auth/model/session consumers": "later explicitly claimed owned waves",
        "types/proto/manifest/contract fixtures": "one implementing owner",
        "workflow/changelog/sequential integration": "coordinator"
      },
      "objective": "Implement and independently exercise the first required common identity/recovery revision/protocol compatibility contracts for S00/S01. Full project/all TODO objective remains unchanged.",
      "prepared_at": "2026-09-30T20:48:30.201010+00:00",
      "prerequisite_receipts": [
        "development/evidence/qualify-g02/integration/completion.json",
        "development/evidence/dependency-g01/a2/integration.json"
      ],
      "protection": [
        "No queue/changelog/rootREADME/other helper/audit files or another worktree edits; sourcechanges only frozenedit_paths.",
        "Preserve verifiedpreviews18905/43180,18907/43181,25530/43182 and their build/tmp; PostgreSQL80873 and all durable state/evidence/cache/Git/e/p/s protected.",
        "No new screenshot timer, network/provider/account/payment/privatebook/contact activity, global/homecache changes or unrelated dependency upgrades. Prefer locked/offline cache; missing dependency is precise prerequisite before any download/installation.",
        "No recursive delegation/selfapproval or speculative game/model/auth/service skeleton; no fullG02/G03/project completion claims.",
        "Stage only ownsourcepaths, commit before completehandoff, disclose untracked retained evidence, freeze finalsource aftersubmission."
      ],
      "record_role": "atomic",
      "required_capabilities": {
        "audio": false,
        "computer_use": false,
        "vision": false
      },
      "required_sources": [
        "AGENTS.md",
        "planning/coding-style.md",
        "planning/subsystem-architecture.md",
        "planning/subsystem-interfaces.md",
        "planning/rpc-api.md",
        "planning/service-operations.md",
        "planning/implementation-roadmap.md",
        "planning/plan-database.md",
        "ADR/0001-sqlite-agent-workflow.md",
        "ADR/0004-development-reliability.md",
        "ADR/0005-frontier-output-evaluation.md",
        "planning/runtime-reliability.md",
        "ADR/0002-resource-scheduling-and-cleanup.md",
        "ADR/0003-agent-devlog.md"
      ],
      "scope": [
        "One implementing owner freezes field numbering and first-wave Rust types and fixtures. No game/auth/service handler, public registration, state schema or full41-method skeleton.",
        "Distinct SessionId, MemberId, ClientBindingId, RunId and OperationId. Coordinated representation: opaque nonzero128-bit values as16 canonical bytes; validated supplied values confer no permission or credential and this library does not generate identities.",
        "RecoveryEpoch validated nonzero u64; SessionRevision contains epoch plus u64 in-epoch sequence, sequence zero is valid initial state. Lexicographic ordering and checked next-sequence arithmetic; no protected-head issuance/recovery authority in pure types.",
        "BuildIdentity bounded nonsecret revision labels for source/native/WASM/config/content, explicit presence and typed constructor errors. Revision labels identify supplied provenance; construction never asserts a tested or approved build. Resolve required/optional distinctions in documented contracts before consumer fixtures.",
        "Only actually required first-wave explicit wire units with checked conversions. Use Rust Duration for local elapsed durations; no generic unit framework, global clock/auth/engine/SQL/serde/network runtime dependency in df-types.",
        "Generated common messages in approved public.v1 namespace with ledger covering all allocated numbers/names; optional presence where missing differs from zero. Unknown optional fields remain additive; unknown required capability or unsupported protocol revision returns typed incompatibility. Enum zero unspecified; unknown enum/oneof is explicit, never default behavior.",
        "No project-crate dependencies in df-protocol production import closure. No domain-to-network import in df-types. Native/WASM-compatible actual consuming test fixture may compose types and generated messages at df-tools boundary with explicit mapping rather than prohibited crate edges.",
        "Test actual encode/decode current/older-compatible fixtures, required missing fields, malformed IDs/revisions, unknown optional versus required enum/capability/oneof, and ledger reused/removed-number rejection. No test-generated production service stub.",
        "Any wrapper needed solely to exercise unknown enum/oneof compatibility belongs to an explicitly experimental contract fixture namespace. Do not introduce a speculative production handshake/game operation envelope or register a service. Common first-wave schemas use dungeonflux.public.v1.",
        "Field ledger covers actual allocated message/field/enum names and numbers and reserves retired numbers/names. Check against actual generated descriptor/schema bytes, not a test-only hardcoded clone. Negative mutation fixtures must prove reused/removed numbering is refused.",
        "Consumer mapping is exercised at df-tools/tests with explicit checked conversion; no browser.rs/qualification.rs/fixture.rs production behavior edits, no df-types dependency from df-protocol, no serde framework in df-types.",
        "Current fixed-four-crate audit is historical source822/f79 evidence. Future Cargo changes must make its verifier refuse stale inputs; do not repin that audit or claim its graph covers the fifth crate. Independently inspect current task target/dependency closure as part of contract gates."
      ],
      "scope_status": "frozen",
      "scratch": "/Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/CONTRACT-G03-001-a1",
      "source_hashes": {
        "ADR/0001-sqlite-agent-workflow.md": "acfe32a8d5e846aa4d3b2981529533b9f53cdcc11088424128e60c20b722adc8",
        "ADR/0002-resource-scheduling-and-cleanup.md": "a6476cfef449e089639109cc6d3cf5f0800b792b57a32696258d5ac0b9511966",
        "ADR/0003-agent-devlog.md": "ee293673b01391c3a39577bd116a60abb3edfa662a7a8d1315556d6fe537d912",
        "ADR/0004-development-reliability.md": "8f03d02d478086fd1f50d7aa10e8e7e766e3b15ae02f7f96efa43757c98a4dae",
        "ADR/0005-frontier-output-evaluation.md": "25ab35a57ee8516a272b1ff3d04bba4def91319255d89158c9be55283ca6c35c",
        "AGENTS.md": "55578ed92c477dfe3c306db38c6ad20390bfe8208ca998e6f3bf6f6bebbec182",
        "planning/coding-style.md": "2d8e327e4172643544bd80b591f38226c25b83940a10f1d9e18ada9044faeabb",
        "planning/implementation-roadmap.md": "0160ad8e8ec38f768e2348209b9989e30e8f403d9b1a4ebf694f0801f7206932",
        "planning/plan-database.md": "96e3d6c791e7e7cb26e5831d9fde720f69d17301cb067d106966455870f21898",
        "planning/rpc-api.md": "b1606a657a81248b460c2b3f5c12dd8a40725aaa35b5010ecb4cbf02ffe86f1e",
        "planning/runtime-reliability.md": "176e5a8fb04e1d9cb2402225c9eb1bb4ffd110abf353f226a302e10e780acf05",
        "planning/service-operations.md": "780360bcff243a96f5fc9a39a88b47efc1402e563623c134229f56579eb714df",
        "planning/subsystem-architecture.md": "cf549f4f046f76713c881228d82c6a7111d8ccc84db208cb9d9db67bb192d9f6",
        "planning/subsystem-interfaces.md": "f26e1dca42e878f8a816c9f9aa37463cb8f061224598214ceee62632a161907a"
      },
      "task_id": "CONTRACT-G03-001",
      "verification": [
        "Repository wrapper cargo fmt --all -- --check",
        "Repository wrapper cargo clippy --locked --workspace --all-targets -- -D warnings",
        "Repository wrapper cargo test --locked --workspace",
        "Repository wrapper cargo clippy --locked -p df-types -p df-protocol --target wasm32-unknown-unknown -- -D warnings",
        "Repository wrapper cargo clippy --locked -p df-tools --lib --target wasm32-unknown-unknown -- -D warnings",
        "Cross-compile actual consuming contract tests for wasm32-unknown-unknown; execute actual native generated encode/decode/schema ledger boundaries and typed malformed/compatibility cases.",
        "Build current source native fixture and WASM lib into owned distinct build root without replacing live previews; retain exact source/artifact identities and distinguish cross-compilation from WASM execution.",
        "Independent frontier actual internal contract/schema boundary evaluation; no current browser/audio approval inferred from old fixed preview."
      ],
      "worker": "/root/contracts_g03_sol",
      "worktree": "/Users/earlcameron/Desktop/dungeonflux/artifacts/worktrees/contracts-g03"
    },
    "original_objective": "Implement and independently exercise the first required common identity/recovery revision/protocol compatibility contracts for S00/S01. Full project/all TODO objective remains unchanged.",
    "original_verification": [
      "Repository wrapper cargo fmt --all -- --check",
      "Repository wrapper cargo clippy --locked --workspace --all-targets -- -D warnings",
      "Repository wrapper cargo test --locked --workspace",
      "Repository wrapper cargo clippy --locked -p df-types -p df-protocol --target wasm32-unknown-unknown -- -D warnings",
      "Repository wrapper cargo clippy --locked -p df-tools --lib --target wasm32-unknown-unknown -- -D warnings",
      "Cross-compile actual consuming contract tests for wasm32-unknown-unknown; execute actual native generated encode/decode/schema ledger boundaries and typed malformed/compatibility cases.",
      "Build current source native fixture and WASM lib into owned distinct build root without replacing live previews; retain exact source/artifact identities and distinguish cross-compilation from WASM execution.",
      "Independent frontier actual internal contract/schema boundary evaluation; no current browser/audio approval inferred from old fixed preview."
    ],
    "proof_sha256": {
      "crates/df-protocol/proto/common.proto": "6cddc5db2102911b714ff707a7edc66ed137012e44dc58875e69756fc3ff27bc",
      "crates/df-protocol/proto/field-ledger.txt": "ceb064777c63f90c3e19dcf0f7c530697769bce571735a5fca79e79d80c82c7b",
      "crates/df-tools/tests/schema_ledger.rs": "bd916a9a97b499f4c8fddc7209138c4eaab41ca0dede114114c7e5864ef63640",
      "crates/df-tools/tests/shared_contracts.rs": "0f080afb60c0326fc11b4acc4744dcf7a0da0ad5247b32330c7c59b830c45fa8",
      "crates/df-types/Cargo.toml": "adeea17f31b369f16b22ffe2029fa33e6f74379d0688da9359ae738a34402dad",
      "crates/df-types/src/identity.rs": "3eabdba5e02139c334c14767278e3ea89307bbe33d632c03f2dbe509f83f5cef",
      "crates/df-types/src/lib.rs": "3aa843f870a7536152ceaf4f14801e5c2a2f6cc2cb4553632aaee1e00c2815c4",
      "crates/df-types/src/provenance.rs": "c91492bfe232ac0933534575f164b27ca7dd7940505e8d5ce3c97e90a9bb5102",
      "crates/df-types/src/revision.rs": "17d7ff394b33f231a9493d3026691934252989bf84a3bf756c3e2be5bc8ae71e",
      "development/evidence/contracts-g03/integration-review.json": "e346d4e8515ff633d4e2a5794df684438fa45cd106eecae848dec264c9586496",
      "development/evidence/contracts-g03/integration/commands.json": "70363991ae0d28f09c3b0023b778e1537f7ee5561b9149f343a4a7e6e95e3bec",
      "development/evidence/contracts-g03/integration/identity.json": "a738079a82cd0241e0c83fd738d5b5821168afdd5a8d26394919283340aa1ecc",
      "development/evidence/contracts-g03/integration/main-contract-tests.stdout": "ed3197d66d1975135b4c3caf9e20aa2ebc05cce4ad1700ecdcc45e135d3761d7",
      "development/evidence/contracts-g03/review/identity.json": "b7396fa7914be7d70136ba5f1efb7295d4bc85757fe602e560a2e6cce7e23b47",
      "development/evidence/contracts-g03/review/review.json": "34bb02ac36d8c49cdc379dd5938e008a3d7c9ae451cf9df0eb5c7f5af6f8afbc",
      "development/shared-contracts.md": "e026c0dc924e133f0a864b2eb59b8db543f6ce6d41e042d4e50d52ce0870184c",
      "planning/shared-contract-waves.md": "474ed0061989c6117d851a48e07c8cb2a6386335c6ff58dfc8eb3ebbee4aa565"
    },
    "required_attempt_id": "CONTRACT-G03-001-a1",
    "required_attempt_status": "integrated",
    "required_task_id": "CONTRACT-G03-001",
    "required_task_status": "done",
    "required_verdict": "approve",
    "reuse_limit": "Accepted first-wave source and evidence only. Current candidate source/descriptor/consumer compatibility must be reviewed; historic compilation is not new I01 compilation.",
    "source_revision": "9898c70614a8555efa63d3cec47ba9adbc395bbb",
    "task_payload_canonical_sha256": "6b0ac8671ee8556f651e38de039a32bf8d5e930596ee3c81a58385ef8242973c",
    "tested_revision": "9898c70614a8555efa63d3cec47ba9adbc395bbb"
  },
  "approved_alternative_ref": "development/evidence/native-readiness/a2/reviewer/manifest-reference-alternative.json",
  "approved_alternative_sha256": "cc708c1f64a3baaef7e2d3a464cf4592a1c1cee268a351f3436dbbce1d016d3a",
  "mandatory": true,
  "on_missing_or_mismatch": "block I01 dispatch; preserve dispatch_ready=false",
  "root_verification": "Before dispatch reread canonical SQLite task/attempt/review and retained files, compare every receipt field and hash, verify current-source reuse, then retain a dated verifier receipt. Historical tests do not prove candidate-specific checks."
}
''')

def expand(m,root,catalog):
 m=copy.deepcopy(m);families={f['id']:f for f in m['features']};baseline={t['id']:t for t in m['tasks'] if t['brief_json'].get('backlog_generated') is not True}
 assert set(catalog['families'])==set(families),'catalog family coverage'
 original_edges=m.get('backlog_expansion',{}).get('original_dependencies',m['dependencies'])
 oldedges={(e['task_id'],e['prerequisite_task_id']) for e in original_edges if e['task_id'] in baseline and e['prerequisite_task_id'] in baseline}
 paths=set(m['source_documents'])|{'development/backlog-catalog.json'}
 sources={p:{'sha256':digest(root/p)} for p in sorted(paths)}
 fingerprint=hashlib.sha256(json.dumps({p:v['sha256'] for p,v in sources.items()},sort_keys=True).encode()).hexdigest()
 now=catalog['created_at'];m.update(source_documents=sources,source_fingerprint=fingerprint,generated_at=now)
 m['source_sections']={}
 for path in sorted(paths):
  if path.endswith('.md'):
   parts=re.split(r'(?m)^## ',(root/path).read_text());m['source_sections'][path]={'preamble':parts[0],'sections':{part.split('\n',1)[0]:part.split('\n',1)[1] if '\n'in part else '' for part in parts[1:]}}
  else:m['source_sections'][path]={'families':sorted(catalog['families'])}
 def refresh(value):
  if isinstance(value,dict):
   if value.get('path') in sources and 'sha256'in value:value['sha256']=sources[value['path']]['sha256']
   for v in value.values():refresh(v)
  elif isinstance(value,list):
   for v in value:refresh(v)
 for f in m['features']:refresh(f);f.update(source_fingerprint=fingerprint,updated_at=now)
 for t in baseline.values():refresh(t);t.update(source_fingerprint=fingerprint,updated_at=now);t['brief_json']['input_revision']=fingerprint
 phases={fid:collections.defaultdict(list) for fid in families}
 for fid,rows in catalog['families'].items():
  for row in rows:phases[fid][row['phase']].append(row['id'])
 def ids(fid,*ps):return [t for p in ps for t in phases[fid][p]]
 def last(fid,p):return ids(fid,p)[-1]
 def early(g):return last(g,'R') if g in {'G01','G02','G03','G07'} else last(g,'D')
 edges=set();new={}
 def dep(t,ps):
  for p in ps:
   if p!=t:edges.add((t,p))
 # Baseline requirements retained, gate prerequisites now identify early decision/component evidence.
 for t,p in oldedges:
  if re.fullmatch(r'G\d\d-RESOLVE',p) and not t.startswith('P01'):p=early(p[:3])
  edges.add((t,p))
 for fid,rows in catalog['families'].items():
  family=families[fid];original=[t for t in baseline.values() if t['feature_id']==fid];base=copy.deepcopy(min(original,key=lambda t:t['id']));owners=family['plan_json'].get('owners',base['brief_json']['owners'])
  for row in rows:
   phase=row['phase'];tid=row['id'];stage=PHASE[phase];item=copy.deepcopy(base)
   selected=row['owners']
   capabilities=row['required_capabilities'];user=capabilities['computer_use'];audio=capabilities['audio']
   procedure=('Freeze the cited source decision and a bounded contract example; compare '+row['expected']+'. Retain decision, alternatives and unresolved facts.' if phase=='D' else 'Execute the named qualification against pinned tool/source/provider/device identities; measure '+row['expected']+'; classify PASS/FAIL/INCONCLUSIVE and retain actual output.' if phase=='R' else 'Create the smallest owned implementation or use-case wiring for '+row['action']+'; execute its deterministic normal and failure fixture; require '+row['expected']+'.' if phase in {'I','W'} else 'At the dependency-pinned implementation build, exercise '+row['action']+'; observe '+row['expected']+'; retain fixture input, actual receipt/output and failed-path cleanup.' if phase in {'F','A'} else 'Use the accepted integrated baseline and named reference workload; measure '+row['action']+' before/after; verify '+row['expected']+' and equivalent source receipts/privacy/cancellation.')
   areas=['planning/'] if phase=='D' else ['artifacts/tmp/qualification/'+fid+'/ (planned isolated fixture; exact path frozen by coordinator)'] if phase=='R' else [f'crates/{o}/src/ (planned owner boundary; exact module paths frozen at dispatch)' for o in selected if o.startswith('df-')]
   if phase in {'F','A','O'}:areas=[f'crates/{o}/tests/ or benches/ (planned case evidence; exact path frozen by G01/scoped dispatch)' for o in selected if o.startswith('df-')]
   areas += [o+' (planned project-owned surface; exact files frozen at dispatch)' for o in selected if o.endswith('/') and not o.startswith('df-')]
   if not areas:areas=['development/ or planning/ (exact evidence path frozen at scoped dispatch)']
   criteria=[row['expected'], 'The named outcome has actual source/build-bound evidence; unsupported, pending, failed and unperformed checks remain explicit.']
   item.update(id=tid,feature_id=fid,title=row['action'],objective=row['action']+'; expected: '+row['expected']+'.',stage=stage,edit_areas_json=areas,acceptance_json=criteria,verification_json=[procedure,'Exact executable commands: TBD at G01 and scoped prerequisite resolution; this planned procedure is not a claim that Rust/browser/provider checks ran.'],priority=base['priority'],status='pending',blocking_reason=None,originating_task_id=None,source_fingerprint=fingerprint,created_at=now,updated_at=now)
   b=item['brief_json']
   for key in ['backlog_children','aggregation_requires','canonical_implementation_refs','backlog_role_reason']:b.pop(key,None)
   b.update(objective=item['objective'],why=family['goal'],owners=selected,input_revision=fingerprint,inputs={'family_models':family['plan_json'].get('models',[]),'case':row['action'],'prerequisite_contracts':'Exact reviewed children named in dependencies; later production types remain G03 wave-specific.'},outputs=criteria,dispatch_ready=False,record_role='atomic_blueprint',scope_status='planned',backlog_generated=True,canonical_key=row['canonical_key'],case_phase=phase,granularity='one independently reviewable explicit outcome; exact paths/limits frozen before execution',dispatch_guard='Planning only. Coordinator freezes exact source/tool/types/paths/limits and applicable rights/phase approvals before changing record_role to atomic, scope_status to frozen and dispatch_ready to true.',required_capabilities=capabilities,verification_environment='Planned authorized native/WASM environment; commands TBD by G01, production contracts by G03. No executable application currently exists.',required_output_review='Independent frontier review of this exact boundary; actual computer use/vision for user-facing cases, actual audible observation where audio is required. Logs alone do not prove output.',canonical_implementation_policy='Crate child owns the primitive; rules child owns source behavior; feature child owns this named use-case adapter; slice child owns this specific cross-system connection and end-to-end proof. Reuse canonical_task_refs, never create a second implementation.',completion_policy='Existing task/attempt independent review and integrated revision triggers; no automatic completion from dependencies or child counts.')
   if tid=='B-C-df-types-I01':b['required_accepted_source_receipt']=copy.deepcopy(I01_ACCEPTED_SOURCE_RECEIPT)
   b['governing_sources'].append({'path':'development/backlog-catalog.json','sha256':sources['development/backlog-catalog.json']['sha256'],'sections':[fid,row['id']]})
   b['integration_hooks']=[{'hook':row['action']+' -> '+row['expected'],'owner':', '.join(selected)},{'hook':row['consumer_hook']['boundary'],'owner':', '.join(row['consumer_hook']['owner'])}]
   if any(o in PURE for o in selected):b['known_pitfalls'].append('Pure shared/domain owner has no SDK clock DB provider socket I/O; diagnostic facts handed to native consumer. Domain wire/serialization codecs belong to df-api/df-client/df-persistence consumers.')
   if fid in {'F14','S07'}:b['scope_selection']='Conditional fidelity. Explicit independently reviewed selection/rescope receipt required; mandatory flat fallback stays required. This seed does not mark unselected optional children done or satisfy cancelled dependencies.'
   if fid=='F47':b['scope_selection']='Source-compatible standard route required; novel custom mechanics only explicitly consented versioned opt-in. No runtime creator code.'
   if fid=='X11':b['scope_selection']='Hosted remote required core; async/public community/marketplace only explicit future selection. Degraded cache is not authority.'
   if row.get('implementation_delegate'):
    b.update(record_role='reference',implementation_delegate=row['implementation_delegate'],scope_status='planned')
   new[tid]=item
  D,I,W,F,A,O,R=[ids(fid,p) for p in ['D','I','W','F','A','O','R']]
  # Contract/design outcomes are separately reviewable; only the last explicit design milestone rolls up the preceding design decisions.
  if D:dep(D[-1],D[:-1])
  # I01 consumes only the identity decision and the admitted first G03 wave.
  for t in I:dep(t,[D[0]] if t=='B-C-df-types-I01' else D)
  for t in W:dep(t,D+I)
  for t in F+A:dep(t,I+W if I or W else R if R else D)
  for t in O:dep(t,F+A)
  for t in R:dep(t,D)
  if R:dep(R[-1],R[:-1])
  if fid.startswith('C-'):
   crate=fid[2:]
   for t in I+W:dep(t,(['B-G03-D01'] if t=='B-C-df-types-I01' else [early('G01'),early('G03')])+['C-'+d+'-BOUNDARY' for d in family['plan_json'].get('direct_dependencies',[])]+[x for d in family['plan_json'].get('direct_dependencies',[]) for x in ids('C-'+d,'I') if not next(r for r in catalog['families']['C-'+d] if r['id']==x).get('implementation_delegate')])
   for t in W:
    for consumer in new[t]['brief_json']['owners']:
     if consumer!=crate and 'C-'+consumer in families:dep(t,ids('C-'+consumer,'I'))
   for t in I:
    if crate in {'df-content','df-rules'}:dep(t,[early('G07')])
   for t in O:
    for t0,p in oldedges:
     if t0==fid+'-OPT' and p.startswith('S'):dep(t,[p])
  elif fid.startswith('R'):
   for t in D+I+W:dep(t,[early('G07')])
   for t in I+W:dep(t,ids('C-df-rules','I')+[early('G03')])
   for t in I:
    for used in RULE_DEPS.get(fid,[]):dep(t,ids(used,'I'))
   for t in O:dep(t,['S06-ACCEPT'])
  elif fid.startswith('F'):
   canonical=[x for o in owners if 'C-'+o in families for x in ids('C-'+o,'I') if not next(r for r in catalog['families']['C-'+o] if r['id']==x).get('implementation_delegate')]
   for t in I+W:dep(t,canonical+[early('G03')])
   for used in RULE_USE.get(fid,[]):
    for t in I+W:dep(t,ids(used,'I'))
   for t in O:
    sliceid=next((s for s,fs in SLICE_FEATURES.items() if fid in fs and s!='S00'),'S05')
    dep(t,[sliceid+'-ACCEPT'])
  elif fid.startswith('G'):
   gate=fid
   if gate=='G02':
    for t in R:dep(t,[early('G01')])
   elif gate=='G03':
    for t in R:dep(t,[early('G02')]+[last(c,'D') for c in ['C-df-types','C-df-model','C-df-protocol','C-df-rpc-bridge','C-df-observe','C-df-testkit','C-df-tools']])
   elif gate in GATE_COMPONENTS:
    for t in R:dep(t,[early('G03')]+[x for c in GATE_COMPONENTS[gate] for x in ids('C-'+c,'I')])
   for t in F+A:
    integrated={'G06':'S01-ACCEPT','G10':'X10-ACCEPT','G11':'S03-ACCEPT','G12':'S03-ACCEPT'}.get(gate)
    if integrated:dep(t,[integrated])
   for t in O:dep(t,['S00-ACCEPT' if gate in {'G01','G02','G03'} else 'S03-ACCEPT' if gate in {'G08','G11','G12'} else 'S01-ACCEPT' if gate in {'G04','G05','G06'} else 'S08-ACCEPT' if gate=='G09' else 'S06-ACCEPT' if gate=='G07' else 'X10-ACCEPT'])
  elif fid.startswith('S'):
   for t in W:
    for f in SLICE_FEATURES[fid] if fid!='S00' else []:dep(t,ids(f,'I','W'))
    if fid=='S00':
     for c in ['df-types','df-protocol','df-observe','df-rpc-bridge']:dep(t,[x for x in ids('C-'+c,'I') if not next(r for r in catalog['families']['C-'+c] if r['id']==x).get('implementation_delegate')])
    if fid=='S06':
     for r in ['R'+str(i).zfill(2) for i in range(1,18)]:dep(t,ids(r,'I','W','F','A'))
   for t in W:dep(t,[early('G01'),early('G02'),early('G03')])
   for t in O:dep(t,[fid+'-ACCEPT'])
  elif fid.startswith('X'):
   for t in I+W:
    for c in X_REUSE[fid]:dep(t,ids('C-'+c,'I'))
    dep(t,[early('G03')])
   for t in O:dep(t,['X12-ACCEPT' if fid in {'X09','X12'} else 'S08-ACCEPT' if fid in {'X02','X03','X05','X06','X08'} else 'S05-ACCEPT'])
  elif fid=='P01':
   for t in W+F+A:dep(t,['S06-ACCEPT','X12-ACCEPT','X09-ACCEPT'])
  # All original IDs become coordinator-only evidence aggregations, not duplicate runnable producers.
  for parent in original:
   suffix=parent['id'].split('-')[-1];pstage=parent['stage'];b=parent['brief_json'];b.update(record_role='aggregate',scope_status='planned',dispatch_ready=False,backlog_children=D+R+I+W+F+A+O,completion_policy='Coordinator-only evidence aggregation under existing claimed attempt lifecycle; independent reviewer covers every original criterion plus selected children and actual integrated outcome. Never automatic child-count completion.',backlog_role_reason='Preserved coarse planning ID; canonical implementation is owned by explicit children below.')
   chosen=D if suffix in {'BOUNDARY','MODEL'} or pstage=='design' else I+W if suffix in {'DELIVER','COMPOSE','AUTHOR'} else O if suffix=='OPT' else D+R+F+A+O if suffix=='RESOLVE' and fid.startswith('G') else D if suffix=='RESOLVE' else D+W+F+A if fid=='P00' else F+A if pstage=='acceptance' else I+W+F+A
   b['aggregation_requires']=chosen;b['canonical_implementation_refs']=I+W
   dep(parent['id'],chosen)
  family['plan_json']['backlog_arc']={p:ids(fid,p) for p in ['D','R','I','W','F','A','O']}
  family['plan_json']['aggregate_policy']='Preserved coarse records require reviewed selected-child receipts and original integrated criteria; implementation dispatched only through atomic children.'
 # Consumer-owned adapters require compiled consumer primitives, not merely abstract producer contracts.
 for fid,rows in catalog['families'].items():
  for row in rows:
   tid=row['id']
   if row.get('implementation_delegate'):dep(tid,[row['implementation_delegate']])
   if row['phase'] in {'I','W'} or (row['phase'] in {'F','A'} and not fid.startswith('G')):
    for owner in row['owners']:
     if 'C-'+owner in families and not (fid=='C-'+owner):dep(tid,[x for x in ids('C-'+owner,'I') if not next(r for r in catalog['families']['C-'+owner] if r['id']==x).get('implementation_delegate')])
 # New acceptance evidence must also wait for original inherited feature/slice prerequisite scope, without depending on its own aggregate.
 for tid,item in new.items():
  fid=item['feature_id'];phase=item['brief_json']['case_phase']
  if phase in {'I','W'} and not fid.startswith(('G','C-','R')):
   for t,p in oldedges:
    if t.startswith(fid+'-') and t.endswith(('DELIVER','COMPOSE','AUTHOR')) and not p.startswith(fid+'-'):
     dep(tid,[early(p[:3]) if re.fullmatch(r'G\d\d-RESOLVE',p) else p])
  item['brief_json']['canonical_task_refs']=sorted(p for t,p in edges if t==tid and p in new and new[p]['stage']=='implementation')
  item['brief_json']['matching_prerequisites']=sorted(p for t,p in edges if t==tid)
 m['tasks']=sorted([*baseline.values(),*new.values()],key=lambda t:t['id']);m['dependencies']=[{'task_id':t,'prerequisite_task_id':p} for t,p in sorted(edges)]
 m['backlog_expansion']={'original_dependencies':[{'task_id':t,'prerequisite_task_id':p} for t,p in sorted(oldedges)],'catalog_sha256':sources['development/backlog-catalog.json']['sha256'],'generated_children':len(new),'preserved_coarse_tasks':len(baseline),'selection_policy':'Only atomic or operational frozen scopes enter dispatch view; aggregates require coordinator evidence operation.','baseline_execution_state':'Never overwritten by provisioning; authoritative coordinator applies reviewed candidate only.'}
 m['notes']=[n for n in m['notes'] if not n.startswith('Detailed backlog:')]+['Detailed backlog: explicit whole-project outcomes; broad plans authorized, dispatch remains just-in-time. No source catalogs, runtime success or optional selection fabricated.']
 return m

if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--root',type=Path,default=Path(__file__).resolve().parent.parent);p.add_argument('--input',type=Path);p.add_argument('--output',type=Path,required=True);a=p.parse_args();root=a.root.resolve()
 m=json.loads((a.input or root/'development/plan-manifest.json').read_text());catalog=json.loads((root/'development/backlog-catalog.json').read_text());out=expand(m,root,catalog);a.output.write_text(json.dumps(out,indent=2,ensure_ascii=False,sort_keys=True)+'\n');print(json.dumps({'families':len(out['features']),'manifest_tasks':len(out['tasks']),'dependencies':len(out['dependencies']),'source_fingerprint':out['source_fingerprint']}))
