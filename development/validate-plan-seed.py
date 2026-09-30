"""Pure preflight for coordinator planning seed; no queue or filesystem writes."""
import hashlib,json,sqlite3,collections

def validate(manifest,root,schema):
 for group in ['features','tasks']:
  rows=manifest[group];assert len({r['id'] for r in rows})==len(rows),('duplicate id',group)
  assert all(isinstance(r['id'],str) and r['id'].strip() for r in rows),('empty identity',group)
 features={r['id'] for r in manifest['features']};tasks={r['id']:r for r in manifest['tasks']}
 roles={'aggregate','reference','atomic_blueprint','atomic','operational'}
 for t in tasks.values():
  b=t['brief_json'];assert t['status'] in {'pending','blocked'},(t['id'],'seed cannot assign execution status')
  assert t.get('active_attempt_id') is None and t.get('lease_generation',0)==0,(t['id'],'seed cannot claim lease')
  assert t['feature_id'] in features,(t['id'],'missing feature')
  assert b.get('objective') and b.get('owners') and b.get('integration_hooks'),(t['id'],'incomplete brief')
  assert t['acceptance_json'] and t['verification_json'],(t['id'],'empty acceptance')
  if 'record_role'in b:assert b['record_role'] in roles,(t['id'],'invalid role')
  if b.get('dispatch_ready'):
   assert b.get('record_role') in {'atomic','operational'} and b.get('scope_status')=='frozen',(t['id'],'unsafe dispatch classification')
  assert b.get('dispatch_ready') is not True,(t['id'],'planning seed must not enable dispatch')
  for name in ['canonical_task_refs','matching_prerequisites','backlog_children','aggregation_requires']:
   assert all(v in tasks for v in b.get(name,[])),(t['id'],name,'dangling reference')
  refs=b.get('governing_sources',[])+b.get('mandatory_workflow_sources',[])
  assert refs,(t['id'],'no source refs')
  for r in refs:
   p=(root/r['path']).resolve();assert p.is_relative_to(root.resolve()),(t['id'],'source escape')
   assert hashlib.sha256(p.read_bytes()).hexdigest()==r['sha256'],(t['id'],r['path'],'source changed')
 for p,r in manifest['source_documents'].items():
  target=(root/p).resolve();assert target.is_relative_to(root.resolve()),('source escape',p)
  assert hashlib.sha256(target.read_bytes()).hexdigest()==r['sha256'],('source digest',p)
 hashes={p:r['sha256'] for p,r in manifest['source_documents'].items()}
 assert hashlib.sha256(json.dumps(hashes,sort_keys=True).encode()).hexdigest()==manifest['source_fingerprint'],'fingerprint mismatch'
 graph=collections.defaultdict(list);edges=set()
 for e in manifest['dependencies']:
  pair=(e['task_id'],e['prerequisite_task_id']);assert pair not in edges,('duplicate edge',pair);edges.add(pair)
  t,p=pair;assert t in tasks and p in tasks and t!=p,('invalid dependency',pair);graph[t].append(p)
 active=set();visited=set()
 def walk(t):
  assert t not in active,('dependency cycle',t)
  if t in visited:return
  active.add(t)
  for p in graph[t]:walk(p)
  active.remove(t);visited.add(t)
 for t in tasks:walk(t)
 # SQL CHECK validation in an isolated fixture before any destination/schema mutation.
 c=sqlite3.connect(':memory:');c.executescript(schema)
 for table in ['features','tasks']:
  for row in manifest[table]:
   values={k:json.dumps(v,sort_keys=True) if isinstance(v,(dict,list)) else v for k,v in row.items()}
   keys=list(values);c.execute(f'INSERT INTO {table} ({",".join(keys)}) VALUES ({",".join("?" for _ in keys)})',[values[k] for k in keys])
 c.close()
 return {'tasks':len(tasks),'families':len(features),'dependencies':len(edges),'preflight':'pass'}
