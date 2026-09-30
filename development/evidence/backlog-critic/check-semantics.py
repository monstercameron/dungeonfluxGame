import json,pathlib,hashlib,collections
base=pathlib.Path.cwd();root=base/'dungeonflux';m=json.load(open('work/backlog-expansion/candidate-manifest.json'));cat=json.load(open(root/'development/backlog-catalog.json'));t={r['id']:r for r in m['tasks']};e=collections.defaultdict(set)
for row in m['dependencies']:e[row['task_id']].add(row['prerequisite_task_id'])
checks={};counts=collections.Counter();semantic=[]
for fid,rs in cat['families'].items():
 assert all(r['id'] in t for r in rs);arc=collections.defaultdict(list)
 for r in rs:arc[r['phase']].append(r['id'])
 assert all(arc[p] for p in ['D','F','A','O'])
 for r in rs:
  row=t[r['id']];b=row['brief_json'];counts[r['phase']]+=1
  assert row['objective']==r['action']+'; expected: '+r['expected']+'.';assert r['expected']==row['acceptance_json'][0]
  assert b['owners']==r['owners'];assert b['required_capabilities']==r['required_capabilities'];assert set(b['matching_prerequisites'])==e[r['id']]
  assert b['record_role'] in ['atomic_blueprint','reference'] and b['dispatch_ready'] is False and b['scope_status']=='planned'
  if r['phase'] not in ['D','R']:assert all(any(owner in a for a in row['edit_areas_json']) for owner in b['owners'] if owner.startswith('df-') or owner.endswith('/')),r['id']
  if r['phase'] in ['I','W']:assert set(arc['D'])<=e[r['id']]
  if r['phase'] in ['F','A']:assert set(arc['I']+arc['W'] if arc['I'] or arc['W'] else arc['R'] if arc['R'] else arc['D'])<=e[r['id']]
  if r['phase']=='O':assert set(arc['F']+arc['A'])<=e[r['id']]
  semantic.append({'id':r['id'],'action':r['action'],'expected':r['expected'],'owners':b['owners'],'consumer':r['consumer_hook'],'capabilities':b['required_capabilities']})
 assert len(set((r['phase'],r['action'],r['expected']) for r in rs))==len(rs)
checks['all_2838_exact_outcomes_owners_edit_areas_flags_and_prerequisites']=True;checks['all_140_family_design_fixture_acceptance_optimization_arcs']=True
assert e['RULE-EFFECT-CONTRACT']>=set('B-R09-D0'+str(i) for i in range(1,5));assert t['RULE-EFFECT-CONTRACT']['brief_json']['aggregation_requires']==['B-R09-D01','B-R09-D02','B-R09-D03','B-R09-D04'];checks['special_design_contract_waits_design']=True
for row in m['tasks']:
 b=row['brief_json']
 if b['record_role']=='aggregate':assert set(b['aggregation_requires'])<=e[row['id']]
 if b['record_role']=='reference':assert b['implementation_delegate'] in e[row['id']]
checks['aggregation_and_canonical_reference_edges']=True
for fid,rs in cat['families'].items():
 if fid.startswith('C-'):
  f=next(x for x in m['features'] if x['id']==fid)
  required={r['id'] for dep in f['plan_json']['direct_dependencies'] for r in cat['families']['C-'+dep] if r['phase']=='I' and not r.get('implementation_delegate')}
  for r in rs:
   if r['phase'] in ['I','W']:assert required<=e[r['id']],(r['id'],required-e[r['id']])
checks['crate_consumer_compiled_dependency_prerequisites']=True
for fid in ['R05','R07','R08','R09','R10','R11']:
 ids={r['id'] for r in cat['families'][fid] if r['phase']=='I'}
 for r in cat['families']['F11']:
  if r['phase'] in ['I','W']:assert ids<=e[r['id']]
checks['combat_usecase_source_handlers_including_casting']=True
for id in ['S00-ACCEPT','B-G03-R04','B-S00-W01']:
 seen=set();todo=[id]
 while todo:
  x=todo.pop()
  for p in e[x]:
   if p not in seen:seen.add(p);todo.append(p)
 assert not any(p.startswith(('G07','B-G07','G10','B-G10','G12','B-G12')) for p in seen),(id,seen)
checks['foundation_not_blocked_by_full_game_gates']=True
for id in ['B-C-df-player-A01','B-C-df-audio-A01','B-G12-A01']:
 assert t[id]['brief_json']['required_capabilities']['computer_use'] and t[id]['brief_json']['required_capabilities']['vision']
for id in ['B-C-df-audio-A01','B-G12-A01']:assert t[id]['brief_json']['required_capabilities']['audio']
checks['real_output_and_audible_evaluator_requirements']=True
freeze=json.load(open('work/backlog-expansion/freeze.json'));hashes={}
for p,h in freeze['files'].items():
 f=(root/p) if p.startswith(('development/','planning/','ADR/')) else base/p
 actual=hashlib.sha256(f.read_bytes()).hexdigest();assert actual==h,(p,h,actual);hashes[p]=actual
assert hashlib.sha256(json.dumps({p:v['sha256'] for p,v in m['source_documents'].items()},sort_keys=True).encode()).hexdigest()==m['source_fingerprint']==freeze['source_fingerprint']
checks['frozen_all_artifact_hashes_and_combined_source_identity']=True
out=base/'work/backlog-expansion/critic';(out/'semantic-frozen.json').write_text(json.dumps({'verdict':'PASS','source':m['source_fingerprint'],'manifest_sha256':hashes['work/backlog-expansion/candidate-manifest.json'],'checks':checks,'phase_counts':counts,'family_count':len(cat['families']),'roles':dict(collections.Counter(x['brief_json']['record_role'] for x in m['tasks'])),'hashes':hashes},indent=2));(out/'frozen-case-ledger.json').write_text(json.dumps(semantic,indent=2));print(json.dumps(checks))
