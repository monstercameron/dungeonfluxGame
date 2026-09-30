"""Deterministic scoped service-planning extension; invoked by refine-gap-plans.py."""
import copy

def refine(m, docs, ref, fingerprint, now, edges):
    features={v['id']:v for v in m['features']};tasks={v['id']:v for v in m['tasks']}
    commerce='CustomerAccount/Tenant/Payer identities, subscription PriceVersion/EntitlementSnapshot/AllowanceGrantId, exact LedgerEntry/SpendAdmission/Reservation, PaymentObservation and DataLifecycleRequest; pure CommerceTransition, consumer-owned CommerceRepository/PaymentGateway, native supplied CommerceService facade.'
    if 'df-commerce' not in m['expected_coverage']['crates']:m['expected_coverage']['crates'].append('df-commerce')
    m['model_ledger']['df-commerce']=commerce
    def family(fid,template,title,goal,owners,source,kind):
        row=copy.deepcopy(features.get(fid,features[template]));row.update(id=fid,kind=kind,title=title,goal=goal,source_fingerprint=fingerprint,updated_at=now)
        row['created_at']=features.get(fid,{}).get('created_at',now);row['status']=features.get(fid,{}).get('status','planned')
        row['acceptance_json']=['Owned source-bound state/ports, privacy, exact limits, cancellation, version/recovery and failure fixtures frozen.','Required independent executable evidence remains pending; no planning PASS closes release.']
        row['plan_json'].update(owners=owners,models=goal,outputs=goal,governing_sources=[ref(source)],definition_of_complete=row['acceptance_json'],status_meaning='Planning only; no service/game/payment implementation or customers exists.')
        if fid=='C-df-commerce':row['plan_json'].update(public_boundary='CommerceTransition, CommerceRepository, PaymentGateway, supplied native CommerceService',target_and_owner='Native pure customer/tenant, entitlement and spend-ledger policy',direct_dependencies=['df-types'])
        features[fid]=row
    family('C-df-commerce','C-df-auth','df-commerce public contract and data model',commerce,['df-commerce'],'planning/commerce-service.md','crate')
    family('X12','X09','Recoverable customer commerce and entitlement lifecycle',commerce,['df-commerce','df-auth','df-api','df-persistence','df-providers','df-server','df-client','df-ui','df-web'],'planning/commerce-service.md','crosscutting')
    for crate in ['df-client','df-ui','df-web']:
        marker=' Customer service: ';m['model_ledger'][crate]=m['model_ledger'][crate].split(marker,1)[0]+marker+'Existing generated CustomerService adapter or mounted Rust account/recovery/checkout/export/delete panel; server-current grant/operation receipt, payment Unknown/read-only, secure external redirect, no browser card or native policy dependency.'
        features['C-'+crate]['plan_json']['models']=m['model_ledger'][crate]
        for row in tasks.values():
            if row['feature_id']=='C-'+crate:row['brief_json']['inputs']=m['model_ledger'][crate]
    if not any(v['service']=='CustomerService' for v in m['api_services']):
        m['api_services'].insert(1,{'service':'CustomerService','methods':[{'name':name,'request':req,'response':resp,'mode':mode,'owner':'df-api','access':'closed variant current account/tenant permission; only rate-limited scoped identity challenge variants bootstrap'} for name,req,resp,mode in [('Command','CustomerCommandRequest','CustomerReceipt','unary'),('Inspect','CustomerInspectRequest','CustomerView','unary'),('GetOperation','CustomerOperationRequest','CustomerOperationLookup','unary'),('Export','CustomerExportRequest','CustomerExportPart','server_streaming')]]})
    specs=[
      ('C-df-commerce-BOUNDARY','C-df-commerce','design','Freeze one pure commercial policy and native ports','planning/commerce-service.md',['G03-RESOLVE','G04-RESOLVE'],['Freeze exact shared IDs/currencies, account/tenant/payer permissions, pure transition and native facade/repository/gateway consumers; no duplicate auth/wallet registry.','Contract fixtures cover source/grant/cancel/unknown/recovery; Rust-native consumer graph excludes game/WASM dependencies.']),
      ('C-df-commerce-OPT','C-df-commerce','optimization','Profile commercial admission and ledger after integration','planning/commerce-service.md',['S00-ACCEPT','X12-DELIVER'],['Measure representative concurrent exact ledger/entitlement paths, lock contention and hierarchy caps; preserve money/tenant/dispatch semantics before optimizing.']),
      ('X12-RESOLVE','X12','resolution','Freeze account payment entitlement lifecycle','planning/commerce-service.md',['C-df-commerce-BOUNDARY','G05-RESOLVE'],['Freeze hosted gateway/credential provider/jurisdiction selection, customer protobuf numbers and versioned terms/allowances; primary rates are dated inputs.','Pin recoverable identity, payer transfer, renewal/proration/cancel/grace/downgrade/refund/dispute/reconciliation state and read/export permissions.']),
      ('X12-DELIVER','X12','implementation','Implement canonical commerce lifecycle adapters','planning/commerce-service.md',['X12-RESOLVE','C-df-auth-BOUNDARY','C-df-api-BOUNDARY','C-df-persistence-BOUNDARY','C-df-providers-BOUNDARY','G02-RESOLVE','C-df-client-BOUNDARY','C-df-ui-BOUNDARY','C-df-web-BOUNDARY'],['Implement bounded policy/native facade/repository/gateway/inbox/outbox, same exact ledger BudgetStore adapter and current EntitlementReader projection.','No checkout redirect proof, card payload, duplicate wallet, stale grant admission or unconsented spender; customer app source Rust only.']),
      ('X12-ACCEPT','X12','acceptance','Observe customer recovery and payment lifecycle','planning/commerce-service.md',['X12-DELIVER','SPEND-HIERARCHY-ACCEPT','TENANT-LIFECYCLE-ACCEPT'],['Run raw-signature/duplicate/out-of-order/reconciliation fixtures, unknown payments, expiry/cancel/downgrade/refund, lost-storage guest link and payer-host transfer.','Independent computer-use/vision observes honest receipt/read-only/checkout/account recovery UI; no current paid checkout authorized.']),
      ('SPEND-HIERARCHY-ACCEPT','X09','acceptance','Race global supplier payer campaign admission','planning/commerce-service.md',['X12-DELIVER','F28-DELIVER','G08-RESOLVE'],['Multiple campaigns/nodes race last currency/concurrency/rate units; only atomic platform/supplier/tenant/campaign/job permitted jobs dispatch.','Unsent known cancellation releases once; expired Unknown retains liability; price/quote/rights/current entitlement revocation and billed overage stop unsafe admission.']),
      ('EFFECT-FENCE-ACCEPT','F24','acceptance','Observe fenced dispatch takeover and ambiguous send','planning/service-operations.md',['F24-DELIVER','X12-DELIVER','G05-RESOLVE'],['Pause two owners at claim/dispatch/socket/response/settlement; DB lease/fence clock and revocation prevent old game publication.','Unknown send remains discoverable/worst-case reserved; no-idempotency supplier is never automatically resent, stale responses may settle accounting but cannot commit wrong run.']),
      ('TENANT-LIFECYCLE-ACCEPT','X01','acceptance','Observe tenant isolation abuse and deletion restore','planning/service-operations.md',['X12-DELIVER','X01-RESOLVE','X02-RESOLVE','F24-DELIVER'],['Cross-tenant ID/cache/asset/wallet/cursor/history negatives and pooled RLS/role reset reject disclosure/mutation/spend.','Fetch redirects/DNS/IP/bytes/codec bombs/floods bounded; deletion invalidates summaries/indexes/private payload and restored backup reapplies tombstones before serve.']),
      ('SPEECH-GROUNDING-ACCEPT','F38','acceptance','Qualify listener safe creative speech and claims','planning/interaction-engine.md',['F38-DELIVER','F36-DELIVER','F16-DELIVER','G11-RESOLVE','G12-RESOLVE'],['Multilingual names/numbers/negation/rumor/deception/injection/hidden-secret pairs test ExpressionContext/SpeechEnvelope slot claims and exact source/access version.','No raw tokens or secret context exposure; rich flavor qualification reports denominator/errors and residual semantic risk, matching prepared caption/audio fallback.']),
      ('RULING-NOVICE-ACCEPT','R14','acceptance','Observe source guided novice adjudication','planning/interaction-engine.md',['R14-DELIVER','F07-DELIVER','F38-DELIVER','S05-COMPOSE'],['Inexperienced/absent/unauthorized host sees source explanation, permitted decline/wait/alternative/transfer/recovery; no default fabricated DC/timing/resources.','Actual both-role/audio behavior resumes once; record discretionary intervention counts/minutes before autonomous-GM claim.'])]
    base=tasks['X09-RESOLVE']
    for tid,fid,stage,title,doc,prereqs,criteria in specs:
        old=tasks.get(tid);row=copy.deepcopy(old or base)
        owners=features[fid]['plan_json']['owners'];row.update(id=tid,feature_id=fid,title=title,objective=title,stage=stage,edit_areas_json=owners,acceptance_json=criteria,verification_json=['Execute exact normal/boundary/failure procedures in '+doc+' under fixed source/build/tenant/provider/device identities; retain actual outcomes and unperformed gates.'],source_fingerprint=fingerprint,updated_at=now,priority=85)
        row['created_at']=old.get('created_at') if old else now;row['status']=old.get('status','pending') if old else 'pending';row['blocking_reason']=old.get('blocking_reason') if old else None
        row['brief_json'].update(objective=title,why=features[fid]['goal'],owners=owners,inputs=features[fid]['plan_json']['models'],outputs=criteria,input_revision=fingerprint,governing_sources=[ref(doc),ref('planning/implementation-roadmap.md')],dispatch_ready=False,granularity='bounded service contract blueprint',canonical_implementation_policy='One canonical existing family/shared consumer implementation; this is specific repair acceptance, not a duplicate game implementation.',required_capabilities={'computer_use':stage=='acceptance','vision':stage=='acceptance','audio':tid in ['SPEECH-GROUNDING-ACCEPT','RULING-NOVICE-ACCEPT']},known_pitfalls=['Planning/source parity does not prove game/payment/provider/browser/customer behavior.','Unknown liabilities and deleted private payload are never silently reset or regenerated.'],dispatch_guard='Freeze bounded exact edit paths/types/limits and resolve prerequisites before claim. This plan does not authorize app scaffolding, paid calls, contact or deployment.')
        if fid=='X12':
            row['brief_json']['integration_hooks']=[{'hook':'Pure commerce/native facade and exact current grant/ledger','owner':'df-commerce, df-server, df-persistence, df-auth'}, {'hook':'Generated CustomerService current operation/view adapter','owner':'df-client, df-api, df-protocol under G03'}, {'hook':'Persistent mounted account/recovery/checkout/export/delete panel; no card handling or ongoing-game reload','owner':'df-ui, df-web'}]
        tasks[tid]=row
        for prereq in prereqs:edges.add((tid,prereq))
    for tid,prereq in [('P01-ACCEPT','X12-ACCEPT'),('P01-ACCEPT','SPEECH-GROUNDING-ACCEPT'),('P01-ACCEPT','RULING-NOVICE-ACCEPT'),('P01-ACCEPT','EFFECT-FENCE-ACCEPT'),('X09-ACCEPT','SPEND-HIERARCHY-ACCEPT'),('X01-ACCEPT','TENANT-LIFECYCLE-ACCEPT')]:edges.add((tid,prereq))
    updates={
      'planning/commerce-service.md':['X12','X09','G03','G04','G05','G08','F26','F28','C-df-commerce','C-df-auth','C-df-provider-api','C-df-providers','C-df-persistence','C-df-api','C-df-server','C-df-client','C-df-ui','C-df-web'],
      'planning/service-operations.md':['X01','X02','G02','G04','G05','G06','G08','F22','F24','F25','F27','F28','C-df-session','C-df-assets','C-df-persistence','C-df-server','C-df-telemetry','C-df-rpc-bridge'],
      'planning/commercial-validation.md':['P00','P01','X09','X12','G07','G08','C-df-content','C-df-rules','C-df-tools','C-df-observe'],
      'planning/interaction-engine.md':['F08','F16','F36','F37','F38','R14','G11','G12','C-df-ai','C-df-interaction','C-df-intent','C-df-knowledge','C-df-media','C-df-rules']}
    for path,fids in updates.items():
        for fid in fids:
            f=features[fid];f['plan_json']['governing_sources']=[v for v in f['plan_json']['governing_sources'] if v['path']!=path]+[ref(path)]
            f['plan_json'].setdefault('service_refinement_sources',[])
            if path not in f['plan_json']['service_refinement_sources']:f['plan_json']['service_refinement_sources'].append(path)
            f['acceptance_json']=list(dict.fromkeys(f['acceptance_json']+['Apply concrete service repair contract, limits and future evidence from '+path+'; no planning-only launch proof.']))
            for row in tasks.values():
                if row['feature_id']==fid:
                    row['brief_json']['governing_sources']=[v for v in row['brief_json']['governing_sources'] if v['path']!=path]+[ref(path)]
                    row['acceptance_json']=list(dict.fromkeys(row['acceptance_json']+['Apply governing service normal/failure/rights/privacy/recovery criteria from '+path+'.']))
                    row['brief_json']['outputs']=row['acceptance_json']
    model_updates={
      'df-auth':'Trusted TenantScope, EntitlementReader/versioned campaign capabilities and recoverable account/guest-link/ownership permission without duplicate commerce state.',
      'df-provider-api':'Existing BudgetStore consumer port delegates one exact commerce SpendAdmission/Reservation authority; supplier/platform/payer/campaign/job spend/concurrency/rate counters atomic, UnknownLiability retained.',
      'df-session':'Consumer EffectRepository::claim_dispatch owns persisted attempt/fence/reservation, one native egress dispatcher and ambiguous sent status; current actor alone publishes game results.',
      'df-ai':'Listener-safe ExpressionContext and SpeechEnvelope claim/outcome/name/number slots plus qualified noncanonical flavor; no private reasoning context in public provider input or raw token publication.',
      'df-interaction':'SpeechActPlan includes audience-permitted claim/deceit attribution; RulingRequest/Response source-guided novice/absent host with typed wait/decline/alternative.',
      'df-persistence':'CommerceRepository, EntitlementReader and effect claimed-dispatch adapters; trusted tenant scope/RLS pooled reset; deletion/suppression/backup tombstones; single ledger state and current grant at paid admission.',
      'df-api':'CustomerService four closed authorized RPCs with own operation namespace, scoped account recovery and audience-safe lifecycle/export; signed native HTTPS webhook ingress separate from game.',
      'df-server':'Native CommerceService facade/dispatcher/gateway/budget wiring; no bypass provider sockets; explicit single/multi-instance routing and finite recovery/bounds/readiness.',
      'df-telemetry':'Per-instance protected local SQLite segment/spool, bounded federation and suppression/retention/pinned evidence scope; visible loss/gaps no shared writable network SQLite.'}
    for crate,text in model_updates.items():
        marker=' Service refinement: ';m['model_ledger'][crate]=m['model_ledger'][crate].split(marker,1)[0]+marker+text
        features['C-'+crate]['plan_json']['models']=m['model_ledger'][crate]
        for row in tasks.values():
            if row['feature_id']=='C-'+crate:row['brief_json']['inputs']=m['model_ledger'][crate]
    for crate in ['df-providers','df-persistence','df-api','df-server']:
        f=features['C-'+crate]
        if 'df-commerce' not in f['plan_json']['direct_dependencies']:f['plan_json']['direct_dependencies'].append('df-commerce')
    for crate in ['df-commerce','df-providers','df-persistence','df-api','df-server']:
        f=features['C-'+crate];deps=f['plan_json']['direct_dependencies']
        criteria=[v for v in f['acceptance_json'] if not v.startswith('Allowed direct dependencies only:')]+['Allowed direct dependencies only: '+', '.join(deps)+'; preserve consumer ownership and browser closure.']
        f['acceptance_json']=criteria;f['plan_json']['definition_of_complete']=criteria
        for row in tasks.values():
            if row['feature_id']=='C-'+crate:
                row['brief_json']['inputs']=m['model_ledger'][crate]+' Allowed dependencies: '+', '.join(deps)
    for tid in ['G02-RESOLVE','S00-ACCEPT','C-df-rpc-bridge-BOUNDARY']:
        row=tasks[tid];path='planning/rpc-transport.md'
        row['brief_json']['governing_sources']=[v for v in row['brief_json']['governing_sources'] if v['path']!=path]+[ref(path)]
        criterion='Qualify exact HTTP2 DATA/window/header/control/message bounds and browser native allocation/receive observations under slow/suspended/malicious mixed traffic; record PASS/FAIL/INCONCLUSIVE with device/build/driver identity, blocking dependent admissions on FAIL or unresolved critical receive bound, no grpc-web/unsafe Send substitute.'
        row['acceptance_json']=list(dict.fromkeys(row['acceptance_json']+[criterion]));row['brief_json']['outputs']=row['acceptance_json']
        feature=features[row['feature_id']];feature['plan_json']['governing_sources']=[v for v in feature['plan_json']['governing_sources'] if v['path']!=path]+[ref(path)]
    for tid in ['G03-RESOLVE','G05-RESOLVE','X02-ACCEPT']:
        row=tasks[tid];path='planning/service-operations.md'
        row['brief_json']['governing_sources']=[v for v in row['brief_json']['governing_sources'] if v['path']!=path]+[ref(path)]
        criterion='Freeze and test lexicographic SessionRevision=(verified monotonic RecoveryEpoch,in_epoch_sequence); old Pg restore issues protected strictly newer epoch/full snapshot with declared lost game range, retires old revision/operation/allocation namespaces, missing head fails closed, no per-game journal or gameRPO0 claim.'
        row['acceptance_json']=list(dict.fromkeys(row['acceptance_json']+[criterion]));row['brief_json']['outputs']=row['acceptance_json']
    for crate in ['df-types','df-model','df-protocol','df-session','df-client','df-persistence']:
        marker=' Recovery revision: ';m['model_ledger'][crate]=m['model_ledger'][crate].split(marker,1)[0]+marker+'SessionRevision uses lexicographic verified monotonic RecoveryEpoch and in_epoch_sequence; typed/wire comparison frozen G03, lost-game-range snapshot and retired old namespaces on disaster restore, no false gameRPO0.'
        features['C-'+crate]['plan_json']['models']=m['model_ledger'][crate]
        for row in tasks.values():
            if row['feature_id']=='C-'+crate:row['brief_json']['inputs']=m['model_ledger'][crate]
    m['features']=list(features.values());m['tasks']=list(tasks.values())
    note='Service refinement: ASR-01..12 concrete contracts, one df-commerce crate/X12 family, four CustomerService RPCs; full F01-F47/R01-R17 preserved. Independent design and actual validation/revenue remain separate.'
    if note not in m['notes']:m['notes'].append(note)
