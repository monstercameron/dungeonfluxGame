import datetime,hashlib,json,pathlib,re,shutil,subprocess
root=pathlib.Path('/Users/earlcameron/Desktop/dungeonflux/artifacts/worktrees/contracts-g03')
main=pathlib.Path('/Users/earlcameron/Desktop/dungeonflux')
evidence=main/'development/evidence/contracts-g03/worker'
build=main/'artifacts/build/contracts-g03'
commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
inspection={'commit':commit,'targets':{},'limitation':'Target-filtered locked Cargo metadata reflects workspace feature resolution; separate df-protocol normal-edge trees show its actual selected package closure. Descriptive graph inspection does not extend the historical fixed-four-crate audit or approve legal fulfillment.'}
for label,target in [('native','aarch64-apple-darwin'),('wasm','wasm32-unknown-unknown')]:
 p=evidence/'commands'/f'metadata-{label}-current.txt'
 data=json.loads(p.read_text()); packages={p['id']:p for p in data['packages']}; nodes={n['id']:n for n in data['resolve']['nodes']}
 workspace=set(data['workspace_members']); own={packages[id]['name']:id for id in workspace}
 assert set(own)=={'df-types','df-protocol','df-rpc-bridge','df-observe','df-tools'}
 assert not packages[own['df-types']]['dependencies']
 def closure(id):
  visited=set(); todo=[id]
  while todo:
   id=todo.pop()
   if id in visited:continue
   visited.add(id)
   todo.extend(d['pkg'] for d in nodes[id]['deps'] if any(k['kind'] is None for k in d['dep_kinds']))
  return visited
 protocol_projects=sorted(packages[id]['name'] for id in closure(own['df-protocol']) if id in workspace)
 assert protocol_projects==['df-protocol']
 assert 'df-types' not in [d['name'] for d in packages[own['df-protocol']]['dependencies']]
 graph={name:[{'name':dep['name'],'kind':dep['kind'],'target':dep['target'],'path':dep.get('path')} for dep in packages[id]['dependencies'] if dep['name'].startswith('df-')] for name,id in own.items()}
 inspection['targets'][label]={'target':target,'metadata_sha256':digest(p),'workspace_crates':sorted(own),'df_types_dependency_count':0,'df_protocol_production_project_closure':protocol_projects,'direct_project_edges':graph,'resolved_package_count':len(nodes),'full_metadata_evidence':str(p.relative_to(main))}
(evidence/'dependency-inspection.json').write_text(json.dumps(inspection,indent=2)+'\n')
artifacts=[]
paths=[build/'debug/df-transport-fixture',build/'wasm32-unknown-unknown/debug/df_tools.wasm']
paths+=sorted((build/'wasm32-unknown-unknown/debug/deps').glob('shared_contracts-*.wasm'))
paths+=sorted((build/'wasm32-unknown-unknown/debug/deps').glob('schema_ledger-*.wasm'))
paths+=sorted((build/'debug/deps').glob('shared_contracts-*'))
paths+=sorted((build/'debug/deps').glob('schema_ledger-*'))
for p in paths:
 if not p.is_file() or (p.parent == build/'debug/deps' and p.suffix):continue
 artifacts.append({'path':str(p),'sha256':digest(p),'bytes':p.stat().st_size,'embedded_clean_source_revision':commit.encode() in p.read_bytes()})
assert all(item['embedded_clean_source_revision'] for item in artifacts[:2])
schema=evidence/'schema';schema.mkdir(exist_ok=True)
for file in ['common.proto','contract_fixture.proto','transport_fixture.proto','field-ledger.txt']:
 shutil.copyfile(root/'crates/df-protocol/proto'/file,schema/file)
descriptors=sorted(build.glob('debug/build/df-protocol-*/out/contracts.bin'))
assert descriptors
hashes={digest(p) for p in descriptors};assert len(hashes)==1
shutil.copyfile(descriptors[0],schema/'contracts.bin')
artifact_identity={'commit':commit,'artifact_root':str(build),'purpose':'Retained current native fixture and WASM library plus consuming native/WASM test binaries for independent review; no preview replacement','artifacts':artifacts,'generated_descriptors':[{'path':str(p),'sha256':digest(p)} for p in descriptors],'retained_descriptor_sha256':digest(schema/'contracts.bin'),'wasm_execution_performed':False,'native_contract_execution_performed':True}
(evidence/'artifact-identities.json').write_text(json.dumps(artifact_identity,indent=2)+'\n')
(build/'ownership.json').write_text(json.dumps({'task_id':'CONTRACT-G03-001','attempt_id':'CONTRACT-G03-001-a1','owner':'/root/contracts_g03_sol','commit':commit,'purpose':'Current native/WASM artifacts retained for independent contract review','active_review_use':True,'evidence_ref':'development/evidence/contracts-g03/worker/artifact-identities.json'},indent=2)+'\n')
checks=[json.loads(p.read_text()) for p in (evidence/'commands').glob('*current.json')]
assert all(c['exit_code']==0 and not c['timed_out'] and c['commit']==commit for c in checks)
stale=json.loads((evidence/'commands/historical-audit-stale-refusal.json').read_text());assert stale['exit_code']==1
assert json.loads((evidence/'commands/historical-audit-stale-refusal.txt').read_text())=={'ok':False,'reason':'stale or changed pinned input: Cargo.toml'}
pressure=subprocess.check_output(['memory_pressure'],text=True);(evidence/'memory-after.txt').write_text(pressure)
allchecks=[json.loads(p.read_text()) for p in (evidence/'commands').glob('*.json')]
resources={'serial_current_commands':True,'cargo_jobs':1,'brief_max_jobs':2,'rss_sample_interval_seconds':0.25,'current_sampled_peak_process_tree_rss_kib':max(c['observed_peak_process_tree_rss_kib'] for c in checks),'all_runner_sampled_peak_process_tree_rss_kib':max(c['observed_peak_process_tree_rss_kib'] for c in allchecks),'limitation':'Sampled process-tree RSS is not an absolute peak or whole-system/heap/device qualification. Initial small check ran with wrapper jobs2 before coordinator jobs1 follow-up; all subsequent checks/builds use jobs1. Existing previews/processes untouched.','memory_pressure_before_samples':[{'command':p.stem,'free_percentage':re.search(r'System-wide memory free percentage: (\d+)%',p.read_text()).group(1)} for p in (evidence/'commands').glob('*.memory-before.txt')]}
(evidence/'resources.json').write_text(json.dumps(resources,indent=2)+'\n')
print(json.dumps({'commit':commit,'checks':len(checks),'artifact_count':len(artifacts),'descriptor_sha256':next(iter(hashes)),'sampled_peak_rss_kib':resources['all_runner_sampled_peak_process_tree_rss_kib'],'dependency_targets':list(inspection['targets'])}))
