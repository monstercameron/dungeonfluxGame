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
  for t in I:dep(t,D)
  for t in W:dep(t,D+I)
  for t in F+A:dep(t,I+W if I or W else R if R else D)
  for t in O:dep(t,F+A)
  for t in R:dep(t,D)
  if R:dep(R[-1],R[:-1])
  if fid.startswith('C-'):
   crate=fid[2:]
   for t in I+W:dep(t,[early('G01'),early('G03')]+['C-'+d+'-BOUNDARY' for d in family['plan_json'].get('direct_dependencies',[])]+[x for d in family['plan_json'].get('direct_dependencies',[]) for x in ids('C-'+d,'I') if not next(r for r in catalog['families']['C-'+d] if r['id']==x).get('implementation_delegate')])
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
