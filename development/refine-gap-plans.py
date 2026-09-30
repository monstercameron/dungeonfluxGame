#!/usr/bin/env python3
"""Coordinator-only deterministic source/brief refinement; writes an explicit output manifest, never queue state."""
import argparse,copy,datetime,hashlib,json,re
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--input',type=Path,default=Path(__file__).resolve().parent/'plan-manifest.json')
p.add_argument('--output',type=Path,required=True)
p.add_argument('--operational-output',type=Path,help='Explicit optional output for separately preserved operational manifest source hashes')
a=p.parse_args();root=Path(__file__).resolve().parent.parent
m=json.loads(a.input.read_text());now=datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds')
def digest(b):return hashlib.sha256(b).hexdigest()
docs={str(v.relative_to(root)):v.read_text() for v in sorted(root.rglob('*.md')) if v.name!='engine-overview.md' and ((v.parent==root and v.name=='AGENTS.md') or v.parent.name in ['planning','ADR'])}
hashes={v:digest(t.encode()) for v,t in docs.items()};fingerprint=digest(json.dumps(hashes,sort_keys=True).encode())
def ref(path):return {'path':path,'sha256':hashes[path],'sections':re.findall(r'^## (.+)$',docs[path],re.M)}
m['source_documents']={v:{'sha256':h} for v,h in hashes.items()};m['source_sections']={}
for path,text in docs.items():
 parts=re.split(r'(?m)^## ',text)
 m['source_sections'][path]={'preamble':parts[0],'sections':{v.split('\n',1)[0]:v.split('\n',1)[1] if '\n' in v else '' for v in parts[1:]}}
feature_map={v['id']:v for v in m['features']};task_map={v['id']:v for v in m['tasks']}
common=copy.deepcopy(feature_map['X09']['plan_json'])
base_task=copy.deepcopy(task_map['X09-RESOLVE'])
new_families=[
 ('X10','Private campaign authoring and content lifecycle',['df-content','df-tools','df-persistence','df-engine','df-session'],'planning/campaign-authoring.md',
  'Produce authorized validated immutable campaign packages/templates with bounded imports, source/rights provenance, graph closure and safe activation/migration.',
  ['Draft/import diagnostics preserve provenance, access and bounded input; extracted claims require confirmation and cannot run code or change canonical facts.',
   'Published complete pack versions are immutable and source-compatible; duplicate/cancel/lost receipts, stale edits and unsupported live migrations have explicit outcomes.',
   'A creator can build and activate a complete private campaign with alternative disclosures, templates, secrets and fallback media; public community remains conditional.']),
 ('X11','Conditional expansion decisions and honest degraded play',['coordinator','df-session','df-auth','df-api'],'planning/expansion-boundaries.md',
  'Record required F45 hosted remote and bounded disclosed F47 custom-content opt-in; own selected/deferred/rejected async, notification, community/remix/moderation/marketplace, outside-room mobile, broader rules/import expansion without inventing implementation.',
  ['Every expansion has a recorded scope owner, rationale and privacy/rights/cost/evidence prerequisite; unselected work never becomes dispatched implementation or launch support.',
   'Core offline/reconnect keeps permitted stale views and uncommitted drafts with operation lookup; no local rules authority, automatic expired input or world-time advancement.',
   'Any selected async/notification/community design uses the same durable actor and scoped outbox/access boundaries, bounded failure/recovery and explicit consent.'])]
for fid,title,owners,doc,goal,criteria in new_families:
 row=copy.deepcopy(feature_map.get(fid,feature_map['X09']));row.update(id=fid,kind='crosscutting',title=title,goal=goal,acceptance_json=criteria,priority=70,status='planned',source_fingerprint=fingerprint,updated_at=now)
 row['created_at']=feature_map.get(fid,{}).get('created_at',now)
 row['plan_json']=copy.deepcopy(common)
 row['plan_json'].update(owners=owners,governing_sources=[ref(doc),ref('planning/gap-analysis.md')],models=goal,outputs=goal,definition_of_complete=criteria,feature_ids=['F24','F35','F36','F39'] if fid=='X10' else ['F02','F09','F24'],status_meaning='Planned decisions/contracts only; no authoring/runtime/expansion implementation exists.')
 feature_map[fid]=row
 if fid not in {v['id'] for v in m['features']}:m['features'].append(row)
 else:m['features']=[row if v['id']==fid else v for v in m['features']]
specs=[
 ('X10-RESOLVE','X10','resolution','Freeze private campaign package and import contract',['df-content','df-tools'],'planning/campaign-authoring.md',
  ['Pin minimum package/template/schema/import format, source/use-rights checks, namespace and byte/item/depth limits, draft expected version and diagnostic rules.', 'Freeze authorized publication/activation consumer ports and admin CLI versus RPC choice under G03/G05/G10; no invented public protobuf methods.'],['G03-RESOLVE','G07-RESOLVE','G10-RESOLVE']),
 ('X10-AUTHOR','X10','implementation','Build the bounded private authoring path',['df-content','df-tools','df-persistence','df-session'],'planning/campaign-authoring.md',
  ['A bounded allowlist import/author/validate/package flow produces complete immutable source-compatible packs with retained provenance and template instance IDs.', 'Reject malformed/traversal/cyclic/private/rights-conflicting content; canceled/lost publication and stale activation use existing ownership and idempotency contracts.'],['X10-RESOLVE','C-df-content-BOUNDARY','C-df-tools-BOUNDARY','G05-RESOLVE']),
 ('X10-ACCEPT','X10','acceptance','Operate private authoring and campaign activation',['df-tools','df-session','coordinator'],'planning/campaign-authoring.md',
  ['Author from templates and activate/resume a complete fixed campaign with alternate beats, private clues and prepared fallbacks through the actual tools and both browser roles.', 'Frontier computer-use/vision and audible observation verify output; invalid source, conflict, cancel, unpublished access and unsupported migration checks retained.'],['X10-AUTHOR','S05-COMPOSE']),
 ('X11-RESOLVE','X11','resolution','Decide expansion scope and core degradation',['coordinator'],'planning/expansion-boundaries.md',
  ['Record selected required F45 remote and bounded disclosed F47 custom content with owners; resolve conditional async/notifications/publishing/discovery/remix/moderation/marketplace/broader imports/rules/outside-room UX/economy.', 'Selected work has actor/access/cost/rights/consent prerequisites; no unselected feature implementation is dispatched and no disconnected client commits offline game outcomes.'],['G04-RESOLVE']),
 ('X11-ACCEPT','X11','acceptance','Verify conditional scope and authority consistency',['coordinator'],'planning/expansion-boundaries.md',
  ['Check every recorded decision against launch inventory, API/model/privacy/cost boundaries and explicit open gates; exclusions cannot be counted as implemented support.', 'Selected designs use same actor/durable outbox and operation lookup; notification duplicates/revocation, AFK/reaction/expiry and malicious rights/import risks have testable acceptance.'],['X11-RESOLVE']),
 ('MEMORY-LONGHORIZON-ACCEPT','F36','acceptance','Verify multi-session source-grounded memory',['df-knowledge','df-interaction','df-persistence','coordinator'],'planning/long-horizon-state.md',
  ['Versioned multi-session fixture retains secret, false rumor, obligation, contradictory belief and relationship across decay/summary/restart with exact canonical provenance.', 'Bounded authorized retrieval, stale/quarantined summary/index, missing episodes and consolidation basis tested; paired private-state queries cannot leak; replay has zero provider calls.'],['F36-DELIVER','F38-DELIVER','S05-COMPOSE']),
 ('WORLD-TIME-ACCEPT','F35','acceptance','Verify pause-aware bounded world catch-up',['df-world','df-session','coordinator'],'planning/long-horizon-state.md',
  ['Pause overnight and resume source-valid travel/threat/faction schedules under controlled clocks; world time never advances silently from wall time.', 'Bounded deterministic continuation resumes exactly once after restart/fence/duplicate events; private consequences and free resources never appear.'],['F35-DELIVER','S05-COMPOSE']),
 ('RULE-EFFECT-CONTRACT','R09','design','Freeze source-grounded effect and trigger representation',['df-rules','df-content','df-model','df-testkit'],'planning/rules-effect-model.md',
  ['Freeze closed source-linked effect/condition/trigger/pending shapes, exceptional stacking/order/duration and compatible migration/replay; no downloaded rules code or invented D&D bonus.', 'Goldens and legal-state properties link exact source/handler versions and draws; interaction denominator/gaps and shrinking retained failures are explicit.'],['G03-RESOLVE','G07-RESOLVE','R09-MODEL','R05-MODEL']),
 ('SPEECH-ROOM-ACCEPT','F09','acceptance','Verify noisy-room speech, intent and interruptions',['df-audio','df-media','df-intent','coordinator'],'planning/gap-analysis.md',
  ['Exercise crosstalk/background TV/names/numbers/negation/accents, denied mic/sleep/network and explicit barge-in using actual authorized capture leases and both browser roles.', 'Final validated input alone proposes action; ambiguity/jokes/questions clarify, private/current utterance cache scope is exact, obsolete buffers stop and captions match actual sound.', 'Retain final-input-to-intent/commit/approved-audio p50/p95/p99 with samples, noise/device/build/provider identity and actual cost; no raw private speech in default logs or guessed latency.'],['F09-DELIVER','F16-DELIVER','S03-COMPOSE','G12-RESOLVE'])]
edges={(v['task_id'],v['prerequisite_task_id']) for v in m['dependencies']}
for tid,fid,stage,title,owners,doc,criteria,prereqs in specs:
 row=copy.deepcopy(task_map.get(tid,base_task));row.update(id=tid,feature_id=fid,title=title,objective=title+' under the governing design, preserving existing authoritative ownership.',stage=stage,edit_areas_json=owners,acceptance_json=criteria,verification_json=['Run the source-linked normal/boundary/failure procedures in '+doc+' on fixed source/build/content/device identities; record actual commands/outcomes and unperformed checks.'],priority=80 if stage in ['resolution','design'] else 60,source_fingerprint=fingerprint,updated_at=now)
 row['created_at']=task_map.get(tid,{}).get('created_at',now);row['status']=task_map.get(tid,{}).get('status','pending');row['blocking_reason']=task_map.get(tid,{}).get('blocking_reason')
 row['brief_json'].update(objective=row['objective'],why=feature_map[fid]['goal'],owners=owners,governing_sources=[ref(doc),ref('planning/gap-analysis.md')],input_revision=fingerprint,inputs='Pinned source/schema and current canonical implementation family; exact scope/limits frozen before dispatch.',outputs=criteria,granularity='bounded decision/contract work' if stage in ['design','resolution'] else 'future bounded expansion blueprint',dispatch_ready=False,canonical_implementation_policy='Extends the existing family implementation and acceptance; no duplicate owner or independently recreated feature implementation.',required_capabilities={'computer_use':stage=='acceptance' and fid!='X11','vision':stage=='acceptance' and fid!='X11','audio':tid in ['X10-ACCEPT','MEMORY-LONGHORIZON-ACCEPT','SPEECH-ROOM-ACCEPT']},known_pitfalls=['Historical one-shot strength and prior planning PASS do not prove current game behavior. Source gaps stay explicit, derived summaries cannot become truth, and conditional features remain unselected.'],dispatch_guard='Preserve bounded scope, exact edit paths/contracts/limits/commands and required capability evidence before claim; unresolved gates block affected implementation. No paid calls, user contact or deployment authorized by this plan.')
 if tid not in task_map:m['tasks'].append(row)
 else:m['tasks']=[row if v['id']==tid else v for v in m['tasks']]
 task_map[tid]=row
 for prerequisite in prereqs:edges.add((tid,prerequisite))
for edge in [('R09-DELIVER','RULE-EFFECT-CONTRACT'),('S03-ACCEPT','SPEECH-ROOM-ACCEPT'),('S05-ACCEPT','MEMORY-LONGHORIZON-ACCEPT'),('S05-ACCEPT','WORLD-TIME-ACCEPT'),('P01-ACCEPT','X10-ACCEPT'),('P01-ACCEPT','X11-ACCEPT')]:edges.add(edge)
# Attach new actual model/acceptance semantics to existing canonical family/crate briefs.
updates={
 'planning/campaign-authoring.md':['G10','F06','F24','F39','C-df-content','C-df-tools','C-df-persistence','C-df-session'],
 'planning/long-horizon-state.md':['G05','G10','G11','F24','F27','F35','F36','F38','F39','C-df-world','C-df-knowledge','C-df-model','C-df-persistence'],
 'planning/rules-effect-model.md':['G03','G07','F05','F10','F11','F12','R02','R05','R06','R09','R10','R11','R12','R13','R16','C-df-rules','C-df-content','C-df-testkit'],
 'planning/expansion-boundaries.md':['G04','X01','X02','F02','F09','F24','C-df-auth','C-df-session','C-df-api'],
 'planning/gap-analysis.md':['P00','P01','X09','F09','F16','F42','F43','F44']}
for path,fids in updates.items():
 for fid in fids:
  feature_map[fid]['plan_json'].setdefault('gap_refinement_sources',[])
  if path not in feature_map[fid]['plan_json']['gap_refinement_sources']:feature_map[fid]['plan_json']['gap_refinement_sources'].append(path)
  feature_map[fid]['plan_json']['governing_sources']=[r for r in feature_map[fid]['plan_json']['governing_sources'] if r['path']!=path]+[ref(path)]
  for task in m['tasks']:
   if task['feature_id']==fid:task['brief_json']['governing_sources']=[r for r in task['brief_json']['governing_sources'] if r['path']!=path]+[ref(path)]
model_updates={
 'df-model':'MemoryEpisode/MemorySummary/RetrievalRequest/RetrievedMemory/ConsolidationCandidate and admitted MemoryCandidatesReady GameInput bind source/access/index generation; CampaignPackage references and SimulationTier/CatchUpCursor preserve canonical authority. Closed EffectDefinition/ActiveEffect/TriggerWindow/PendingResolution remain source-grounded.',
 'df-content':'CampaignPackage/DraftRevision/ImportSpec/ImportReport/ExtractionCandidate/ValidatedPack, stable template versus runtime instance IDs, source/use-rights and immutable dependency manifest; planning/campaign-authoring.md.',
 'df-tools':'Scoped private import/author/validate/package flow and diagnostics; exact admin publication/activation port selected by X10; no untrusted rules code or assumed public editor.',
 'df-knowledge':'Pure rank_candidates over bounded supplied MemoryCandidateBatch, observer/source/access revalidation and source-grounded MemoryEpisode/MemorySummary/RetrievalRequest/RetrievedMemory/ConsolidationCandidate; no DB/provider I/O. Native df-session-owned MemoryCandidateStore implemented by df-persistence supplies candidates; planning/long-horizon-state.md.',
 'df-session':'Consumer-owned MemoryCandidateStore::load native port and admitted MemoryCandidatesReady staged job; current basis/source/access generation validation before pure knowledge ranking and commit/view.',
 'df-world':'SimulationTier Active/Scheduled/Dormant and CatchUpCursor use deterministic bounded due events/logical time; pause never advances from wall time and optional market economy is unselected.',
 'df-rules':'Source-linked EffectDefinition/ActiveEffect/TriggerWindow/PendingResolution and exceptional precedence, stacking/removal and legal-state source fixtures; closed typed handlers rather than executable imported DSL; planning/rules-effect-model.md.',
 'df-testkit':'RuleFixture exact source/handler/draw provenance; reviewed golden and constrained property/generative interaction tests with retained shrink seed and actual state hash; noisy-room/continuity fixtures.',
 'df-persistence':'MemoryCandidateStore::load adapter supplies bounded access-filtered MemoryCandidateBatch/Incomplete with source/access/index generation; index publication fences source/access basis. Immutable campaign package/draft/version/rights references plus attributed episodes, source-bound summaries and derived index generation; canonical snapshots/decisions retain authority; conditional expansion tables unselected.'}
for crate,value in model_updates.items():
 marker=' Gap refinement: '
 m['model_ledger'][crate]=m['model_ledger'][crate].split(marker,1)[0]+marker+value
 f=feature_map['C-'+crate];f['plan_json']['models']=m['model_ledger'][crate]
 for task in m['tasks']:
  if task['feature_id']=='C-'+crate:task['brief_json']['inputs']=m['model_ledger'][crate]
# Latest user feature outline: reuse existing source/brief refresh and provisioning.
if 'planning/feature-refinement.md' in docs:
 core=[
  ('F45','Hosted remote and mixed-room campaigns',['df-auth','df-session','df-api','df-client','df-audio'],'planning/remote-play.md',
   'Required core remote/mixed-room play uses the same fenced actor/RPC/private views with RemotePlayPolicy, ParticipantPresence, AudioTopology and bounded admission/network/capture/audio leases.',
   ['Remote/local simultaneous input preserves source legality, operation dedupe, pending reactions and current audience/binding generations.', 'Private whispers/captions/manifests stay private, explicit public output leases avoid local duplicates; AFK/disconnect cannot spend actions or advance paused world time.', 'Real selected networked devices prove join/resume/revocation/capture/audio/fallback and measured p50/p95/p99 capacity/relay-cost limits, not local-tab assumptions.']),
  ('F46','Audience-safe campaign recaps and speculative trailers',['df-knowledge','df-narrative','df-presentation','df-media','df-assets','df-session'],'planning/campaign-cinematics.md',
   'BookendSpec/FactSelection/BookendPlan select authorized committed history for recaps and explicitly speculative known-thread-only trailers with bounded optional video, prepared fallback and rights-scoped ExportGrant.',
   ['Private/public/member variants use exact current source/access versions; no false belief canonization, hidden branch spoiler, future fact or player commitment.', 'Cue/jobs are bounded/deduplicated/fenced; failed/stale/canceled/missing video uses permitted prepared stills/narration without blocking entry/play or changing due outcomes.', 'Exports require explicit recipient/fact/media rights and consent; no automatic public sharing or guarantee to retract downloaded media; actual browser/audio and cost review retained.']),
  ('F47','Source-safe generated content and approved custom opt-in',['df-content','df-rules','df-ai','df-engine','df-session','df-persistence'],'planning/generated-content.md',
   'ContentCandidate/Validation/Admission creates source-compatible personalized item/enemy/spell templates and explicitly disclosed distinct custom RulesetId/catalog/approved deterministic handlers; no generated runtime code or standard-mode retcon.',
   ['Standard mode admits only source-compatible legal templates; optional custom definitions require approved implemented handlers, exact versions, participant disclosure and explicit campaign opt-in.', 'Boss phases/objectives/terrain/effects follow approved source handlers/current world facts; unsupported mechanics/rulings are explicit, no invented HP/bonuses/action economy.', 'Duplicate/stale/migrated content/award and private assets are safe; actual source-required progression/rewards remain due despite provider failure, paid/unknown waste reconciled and replay makes zero provider calls.'])]
 for fid,title,owners,doc,goal,criteria in core:
  row=copy.deepcopy(feature_map.get(fid,feature_map['F44']))
  row.update(id=fid,kind='capability',title=title,goal=goal,acceptance_json=criteria,priority=85 if fid=='F45' else 65 if fid=='F46' else 50,source_fingerprint=fingerprint,updated_at=now)
  row['created_at']=feature_map.get(fid,{}).get('created_at',now)
  row['plan_json'].update(owners=owners,governing_sources=[ref(doc),ref('planning/feature-refinement.md')],models=goal,outputs=criteria,definition_of_complete=criteria,status_meaning='Planned/pending capability; no game/native/browser/runtime implementation or current output evidence exists.')
  if fid not in feature_map:m['features'].append(row)
  else:m['features']=[row if v['id']==fid else v for v in m['features']]
  feature_map[fid]=row
  for suffix,stage in [('MODEL','design'),('DELIVER','implementation'),('ACCEPT','acceptance')]:
   tid=fid+'-'+suffix;old=task_map.get(tid);task=copy.deepcopy(old or base_task)
   criteria_stage=['Freeze candidate models/ports, source/access/rules versions, exact limits and current owned decision semantics before dependent implementation.']+criteria if suffix=='MODEL' else criteria
   task.update(id=tid,feature_id=fid,title=('Freeze ' if suffix=='MODEL' else 'Implement ' if suffix=='DELIVER' else 'Accept ')+title,objective=goal,stage=stage,edit_areas_json=owners,acceptance_json=criteria_stage,verification_json=['Execute governing normal/boundary/failure procedures from '+doc+' against a fixed source/build/content/rules/device/provider revision; distinguish actual outcome from planned checks.'],priority=row['priority'],source_fingerprint=fingerprint,updated_at=now)
   task['created_at']=old.get('created_at') if old else now;task['status']=old.get('status') if old else 'pending';task['blocking_reason']=old.get('blocking_reason') if old else None
   task['brief_json'].update(objective=goal,why=goal,owners=owners,governing_sources=row['plan_json']['governing_sources'],inputs=goal,outputs=criteria_stage,input_revision=fingerprint,dispatch_ready=False,granularity='bounded decision/contract work' if suffix=='MODEL' else 'future bounded expansion blueprint',canonical_implementation_policy='One canonical bounded implementation child shared by feature and slice; existing dependencies are reused, never duplicate remote/session/media/rules owners.',required_capabilities={'computer_use':suffix!='MODEL','vision':suffix!='MODEL','audio':suffix!='MODEL'},integration_hooks=[{'hook':'Candidate shared models/generated views and source/access/rules versions','owner':'df-model/df-protocol shared owner under G03'}, {'hook':'Native authorized actor/provider/storage effects and permitted browser rendering','owner':', '.join(owners)}],known_pitfalls=['No old competitor claim, high priority or prepared fixture proves implemented product behavior; source and budgets remain genuine gates.','Remote is core; trailer is speculative; paid video optional; standard-source rewards cannot be withheld after generation failure; novel mechanics require disclosed distinct RulesetId and approved deterministic handlers.'],dispatch_guard='Preserve exact bounded scope, edit paths/contracts/limits/commands and required frontier capabilities; resolve applicable gates before claim. This pending plan authorizes no app scaffolding, paid call, contact or deployment.')
   if not old:m['tasks'].append(task)
   else:m['tasks']=[task if v['id']==tid else v for v in m['tasks']]
   task_map[tid]=task
  for gate in ['G03-RESOLVE','G04-RESOLVE','G08-RESOLVE'] if fid=='F45' else ['G03-RESOLVE','G04-RESOLVE','G08-RESOLVE','G10-RESOLVE'] if fid=='F46' else ['G03-RESOLVE','G07-RESOLVE','G10-RESOLVE','G11-RESOLVE']:edges.add((fid+'-MODEL',gate))
  edges.add((fid+'-DELIVER',fid+'-MODEL'));edges.add((fid+'-ACCEPT',fid+'-DELIVER'));edges.add(('P01-ACCEPT',fid+'-ACCEPT'))
 for tid,prereqs in {
  'F45-DELIVER':['F01-DELIVER','F02-DELIVER','F03-DELIVER','F09-DELIVER','F16-DELIVER','F22-DELIVER','F24-DELIVER'],
  'F45-ACCEPT':['S03-COMPOSE','S05-COMPOSE'],
  'F46-DELIVER':['F15-DELIVER','F16-DELIVER','F20-DELIVER','F24-DELIVER','F44-DELIVER'],
  'F46-ACCEPT':['S05-COMPOSE'],
  'F47-DELIVER':['F05-DELIVER','F40-DELIVER','F41-DELIVER','C-df-content-BOUNDARY','C-df-rules-BOUNDARY'],
  'F47-ACCEPT':['S05-COMPOSE']}.items():
  for prerequisite in prereqs:edges.add((tid,prerequisite))
 checks=[
  ('F03-PRIVATE-ACCEPT','F03','Private contextual phone knowledge and disclosure',['df-api','df-player','df-display','df-knowledge'],'planning/feature-refinement.md',['Paired secret/false-belief states and explicit RevealToParty preserve scoped recipients/current source/access versions without TV/audio/cue/prefetch leaks.','Stable focus/legal options, private whisper fallback, stale/revoked offers and consent checks operate through existing views and typed actions.'],['F03-DELIVER','F07-DELIVER','F36-DELIVER','F16-DELIVER','S03-COMPOSE']),
  ('F15-CRITICAL-ACCEPT','F15','Committed critical-event cinematic eligibility',['df-presentation','df-media','df-render','df-audio'],'planning/campaign-cinematics.md',['Natural20 obeys its exact source outcome; cinematic eligibility uses committed permitted facts, ready references/fatigue and admitted budget, never retcon success.','Skip/reduced-motion/late/missing video and duplicate/reconnect cue preserve legal actions and reaction/resource/timer authority; any actual pause uses authorized existing policy.'],['F15-DELIVER','F44-DELIVER','S04-COMPOSE']),
  ('F42-SPOTLIGHT-ACCEPT','F42','Consented credible player spotlight',['df-experience','df-narrative','df-interaction','df-player'],'planning/feature-refinement.md',['Bounded accepted activity windows and explicit preferences create voluntary credible opportunities, not emotion inference, forced equal speaking shares or turn penalties.','Quiet/dominant/AFK/spamming/declining/revoked-consent/stale-hook cases preserve private backstory and exactly-once accepted/refused opportunity identity.'],['F42-DELIVER','F39-DELIVER','F38-DELIVER','S03-COMPOSE']),
  ('F35-CONTINUITY-ACCEPT','F35','Committed world item and media identity continuity',['df-world','df-assets','df-media','df-render'],'planning/campaign-cinematics.md',['Revisit weather/damage/barricades/NPC/item origin under committed source world time and stable canonical identity; geometry/visibility changes require rules validation.','Late/stale/versioned assets, style invalidation, personal lore and item upgrades cannot revert truth, leak hidden changes or spend/withhold required awards; actual both-role media continuity observed.'],['F35-DELIVER','F15-DELIVER','F25-DELIVER','S05-COMPOSE'])]
 for tid,fid,title,owners,doc,criteria,prereqs in checks:
  old=task_map.get(tid);row=copy.deepcopy(old or base_task)
  row.update(id=tid,feature_id=fid,title=title,objective=title+' using the existing canonical feature implementation.',stage='acceptance',edit_areas_json=owners,acceptance_json=criteria,verification_json=['Run fixed-build source-linked normal/boundary/failure browser and appropriate audible checks from '+doc+'; retain exact observed criterion evidence and unperformed checks.'],priority=70,source_fingerprint=fingerprint,updated_at=now)
  row['created_at']=old.get('created_at') if old else now;row['status']=old.get('status') if old else 'pending';row['blocking_reason']=old.get('blocking_reason') if old else None
  row['brief_json'].update(objective=row['objective'],why=feature_map[fid]['goal'],owners=owners,governing_sources=[ref(doc),ref('planning/feature-refinement.md')],inputs='Existing canonical family delivery, current authorized audience/source/rules/identity basis and permitted event/cue fixtures.',outputs=criteria,input_revision=fingerprint,dispatch_ready=False,granularity='future bounded expansion blueprint',required_capabilities={'computer_use':True,'vision':True,'audio':True},canonical_implementation_policy='Acceptance extension of existing family, not a duplicate implementing task.',integration_hooks=[{'hook':'Observe native committed source/outcome and permitted mounted player/display/audio projection','owner':', '.join(owners)}],known_pitfalls=criteria)
  if not old:m['tasks'].append(row)
  else:m['tasks']=[row if v['id']==tid else v for v in m['tasks']]
  task_map[tid]=row
  for prerequisite in prereqs:edges.add((tid,prerequisite))
  edges.add((fid+'-ACCEPT',tid))
 feature_sources={
  'planning/remote-play.md':['F01','F02','F03','F09','F16','F21','F22','F24','G03','G04','G08','S01','S03','S05','X11','C-df-auth','C-df-session','C-df-api','C-df-client','C-df-audio'],
  'planning/campaign-cinematics.md':['F15','F20','F25','F28','F35','F44','G03','G04','G08','G10','S04','S05','S07','C-df-model','C-df-narrative','C-df-presentation','C-df-media','C-df-assets','C-df-persistence'],
  'planning/generated-content.md':['F05','F11','F15','F35','F40','F41','R02','R12','R15','R16','G03','G07','G10','G11','S04','S05','S06','X11','C-df-content','C-df-rules','C-df-ai','C-df-engine','C-df-session'],
  'planning/feature-refinement.md':['F03','F07','F36','F38','F39','F42','F43','F44','P00','P01','X09','C-df-experience','C-df-player','C-df-display']}
 for path,fids in feature_sources.items():
  for fid in fids:
   f=feature_map[fid];f['plan_json']['governing_sources']=[r for r in f['plan_json']['governing_sources'] if r['path']!=path]+[ref(path)]
   for row in m['tasks']:
    if row['feature_id']==fid:row['brief_json']['governing_sources']=[r for r in row['brief_json']['governing_sources'] if r['path']!=path]+[ref(path)]
 selected='Hosted remote/mixed-room F45 is required core; bounded explicitly disclosed custom-content F47 is planned. Async/community/marketplace and broader homebrew remain conditional; no unselected scope is dispatched.'
 feature_map['X11']['goal']=selected;feature_map['X11']['plan_json'].update(models=selected,outputs=selected,selected_core_features=['F45'],selected_opt_in_features=['F47'])
 for row in m['tasks']:
  if row['feature_id']=='X11':row['brief_json']['why']=selected;row['brief_json']['inputs']=selected
 model_additions={
  'df-model':'RemotePlayPolicy/ParticipantPresence/AudioTopology; ContextualPrivateOffer/KnowledgeCue; BookendSpec/FactSelection/BookendPlan/ExportGrant with recipient/source/access/rights and download-retraction limitation; CriticalCueEligibility; ParticipationWindow/SpotlightPreference; SceneIdentityRevision/ItemOrigin; ContentCandidate/Validation/Admission and approved distinct RulesetId/handler provenance.',
  'df-protocol':'Candidate typed remote presence/topology/private contextual views and existing operation selections; bookend playback and approved custom catalog disclosure fields frozen under G03 numbering/compatibility. No new service/RPC or script injection.',
  'df-auth':'Invite/admission/AFK/takeover and private output/capture authorization, scoped ExportGrant recipient/fact/media rights, source/access versions and prospective revocation; remote core.',
  'df-session':'One actor for remote/local simultaneous input, explicit AudioTopology/output/capture leases, admitted campaign bookend jobs, source/award/content admission dedupe and current access revalidation.',
  'df-presentation':'Pure plan_bookend over supplied permitted committed facts, speculative known-thread trailers and committed CriticalCueEligibility; rights/epoch/fatigue/budget/skippable fallback, never mechanical pause/outcome retcon.',
  'df-media':'Native budgeted bookend/critical/identity jobs use existing AssetDemand/BudgetStore/AssetStore; exact scope/expiry/unknown spend, no due reward withheld on failure.',
  'df-experience':'Bounded accepted ParticipationWindow and explicit SpotlightPreference, credible consented SpotlightOpportunity with refusal/expiry/basis; no ambient emotion/boredom inference or compelled interaction.',
  'df-world':'Committed SceneIdentityRevision/world layer diff and geometry basis; rumors/offscreen events advance only accepted campaign time, not wall time while paused.',
  'df-content':'Approved source-compatible generation templates and immutable ContentCandidate/Validation/Admission/ItemOrigin definitions; selected opt-in custom catalog/handler allowlist and exact rules/source/rights provenance.',
  'df-rules':'Standard-compatible generated templates and independently reviewed deterministic custom handlers gated by disclosed distinct RulesetId, source timing/geometry/resources/stacking; no creator script or novel standard bonuses.',
  'df-persistence':'Versioned remote/bookend/export access/rights/identity/content/award metadata through existing fenced decisions and immutable publication; snapshots/replay keep source/handler versions, no separate database.'}
 for crate,value in model_additions.items():
  marker=' Feature refinement: ';m['model_ledger'][crate]=m['model_ledger'][crate].split(marker,1)[0]+marker+value
  feature_map['C-'+crate]['plan_json']['models']=m['model_ledger'][crate]
  for row in m['tasks']:
   if row['feature_id']=='C-'+crate:row['brief_json']['inputs']=m['model_ledger'][crate]
 priority_map={'F03':'P0','F43':'P0','F44':'P0','F15':'P0','F36':'P0','F38':'P0','F39':'P0','F24':'P0','F45':'P0','F35':'P1','F42':'P1','F40':'P1','F41':'P1','F46':'P1','F47':'P2'}
 for fid,priority in priority_map.items():
  feature_map[fid]['plan_json']['requested_priority']=priority
  feature_map[fid]['plan_json']['priority_semantics']='User importance overlay; source/authority/dependency gates and optional paid-fidelity budgets still apply.'
  for row in m['tasks']:
   if row['feature_id']==fid:row['brief_json']['requested_priority']=priority
 m['expected_coverage']['features']=['F'+str(n).zfill(2) for n in range(1,48)]
 feature_note='Latest feature outline adds F45 required hosted remote, F46 audience-safe recap/speculative trailer, F47 source-compatible generation and approved disclosed custom opt-in; all15 ideas/15 priorities/10stakes/3phases mapped, same40crates/11services37RPC/schema.'
 if feature_note not in m['notes']:m['notes'].append(feature_note)

# Scoped service review extension preserves prior corpus and execution state.
if 'planning/commerce-service.md' in docs:
 from importlib.util import spec_from_file_location,module_from_spec
 spec=spec_from_file_location('service_refinement',root/'development/refine-service-plans.py');module=module_from_spec(spec);spec.loader.exec_module(module)
 module.refine(m,docs,ref,fingerprint,now,edges)

# Recursively refresh every exact source reference; historic attempt briefs stay in SQLite/evidence untouched.
def refresh(value):
 if isinstance(value,dict):
  if 'path' in value and 'sha256' in value and value['path'] in docs:value.update(ref(value['path']))
  for v in value.values():refresh(v)
 elif isinstance(value,list):
  for v in value:refresh(v)
for collection,key in [('features','plan_json'),('tasks','brief_json')]:
 for row in m[collection]:
  row['source_fingerprint']=fingerprint;row['updated_at']=now;refresh(row[key])
  if key=='brief_json':row[key]['input_revision']=fingerprint
m['dependencies']=[{'task_id':t,'prerequisite_task_id':v} for t,v in sorted(edges)]
m['source_fingerprint']=fingerprint;m['generated_at']=now
note='Gap brief refinement maps all 76 research rows; no new crates/API methods/schema, X10 private authoring and X11 conditional expansion plus four targeted existing-family refinement plans. Historical critics remain scoped to their original fingerprints.'
if note not in m['notes']:m['notes'].append(note)
if (root/'development/backlog-catalog.json').exists():
 from importlib.util import spec_from_file_location,module_from_spec
 spec=spec_from_file_location('backlog_refinement',root/'development/expand-backlog.py');module=module_from_spec(spec);spec.loader.exec_module(module)
 m=module.expand(m,root,json.loads((root/'development/backlog-catalog.json').read_text()));fingerprint=m['source_fingerprint'];hashes={path:value['sha256'] for path,value in m['source_documents'].items()}
a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(m,indent=2,ensure_ascii=False,sort_keys=True)+'\n')
if a.operational_output:
 operational_path=root/'development/operational-tasks.json'
 operational=json.loads(operational_path.read_text())
 refresh(operational)
 for row in operational.get('tasks',[]):
  row['source_fingerprint']=fingerprint;row['updated_at']=now
  row['brief_json']['input_revision']=fingerprint
  row['brief_json'].update(record_role='operational',scope_status='planned')
 a.operational_output.parent.mkdir(parents=True,exist_ok=True)
 a.operational_output.write_text(json.dumps(operational,indent=2,ensure_ascii=False)+'\n')
print(json.dumps({'families':len(m['features']),'tasks':len(m['tasks']),'dependencies':len(m['dependencies']),'sources':len(m['source_documents']),'source_fingerprint':fingerprint}))
