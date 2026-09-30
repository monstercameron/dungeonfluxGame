#!/usr/bin/env python3
"""Verify this planning snapshot read-only; negative SQL probes use an in-memory backup."""
from pathlib import Path
import sqlite3,json,hashlib,re,collections,tempfile,datetime
ROOT=Path('/Users/earlcameron/Documents/Codex/2026-09-29/cr/dungeonflux');BASE=ROOT/'development'
m=json.loads((BASE/'plan-manifest.json').read_text());checks={};errors=[]
if (BASE/'operational-tasks.json').exists():
 operations=json.loads((BASE/'operational-tasks.json').read_text())
 m['tasks']+=operations.get('tasks',[])
 m['dependencies']+=operations.get('dependencies',[])
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
c=sqlite3.connect((BASE/'workflow.sqlite3').as_uri()+'?mode=ro',uri=True)
checks['integrity']=c.execute('PRAGMA integrity_check').fetchone()[0]
checks['foreign_keys']=c.execute('PRAGMA foreign_key_check').fetchall()
checks['journal_mode']=c.execute('PRAGMA journal_mode').fetchone()[0]
checks['five_tables']=[r[0] for r in c.execute("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")]
assert checks['five_tables']==sorted(['features','tasks','dependencies','attempts','devlog'])
# Export parity checks actual seed JSON contents, not only row counts or stored fingerprint labels.
for table in ['features','tasks']:
 for row in m[table]:
  names=list(row);actual=c.execute(f'SELECT {",".join(names)} FROM {table} WHERE id=?',(row['id'],)).fetchone();assert actual,row['id']
  for key,want,have in zip(names,row.values(),actual):
   if key in ['status','blocking_reason','originating_task_id','created_at','updated_at']:continue # execution state/history may evolve independently.
   if isinstance(want,(dict,list)):have=json.loads(have)
   assert want==have,(table,row['id'],key)
checks['full_manifest_db_plan_parity']=True
seed_ids={t['id'] for t in m['tasks']}
actual_deps={(t,p) for t,p in c.execute('SELECT task_id,prerequisite_task_id FROM dependencies') if t in seed_ids}
assert actual_deps=={(d['task_id'],d['prerequisite_task_id']) for d in m['dependencies']}
checks['dependency_parity']=True
graph=collections.defaultdict(list)
for t,p in c.execute('SELECT task_id,prerequisite_task_id FROM dependencies'):graph[t].append(p)
seen=set();active=set()
def dfs(t):
 assert t not in active,('cycle',t)
 if t in seen:return
 active.add(t)
 for p in graph[t]:dfs(p)
 active.remove(t);seen.add(t)
for (t,) in c.execute('SELECT id FROM tasks'):dfs(t)
checks['acyclic_tasks']=len(seen)
for table in ['features','tasks']:
 for ident,stored in c.execute(f'SELECT id,{"plan_json" if table=="features" else "brief_json"} FROM {table}'):
  data=json.loads(stored)
  for ref in data['governing_sources']+data.get('mandatory_workflow_sources',[]):assert digest(ROOT/ref['path'])==ref['sha256'],(ident,ref['path'],'changed source')
  if table=='tasks':
   for key in ['objective','why','owners','governing_sources','input_revision','inputs','outputs','semantics','integration_hooks','verification_environment','known_pitfalls','handoff','routing','dispatch_guard','granularity','required_output_review']:assert data.get(key),(ident,key)
   assert data.get('dispatch_ready') is not True,('unexpected runnable',ident)
checks['required_briefs_and_source_hashes']=True
for p,metadata in m['source_documents'].items():assert digest(ROOT/p)==metadata['sha256'],p
checks['manifest_input_fingerprints']=len(m['source_documents'])
for group,ids in m['expected_coverage'].items():
 for fid in (['C-'+i for i in ids] if group=='crates' else ids):assert c.execute('SELECT 1 FROM features WHERE id=?',(fid,)).fetchone(),fid
 checks['coverage_'+group]=len(ids)
assert len(m['model_ledger'])==40 and all(m['model_ledger'].values());checks['crate_models']=40
checks['rpc_services']=len(m['api_services']);checks['rpc_methods']=sum(len(s['methods']) for s in m['api_services'])
# Crate DAG and browser transitive closure.
crategraph={f['id'][2:]:f['plan_json']['direct_dependencies'] for f in m['features'] if f['kind']=='crate'}
visited=set();visiting=set()
def cratewalk(t):
 assert t not in visiting,('crate cycle',t)
 if t in visited:return
 visiting.add(t)
 for p in crategraph[t]:assert p in crategraph,p;cratewalk(p)
 visiting.remove(t);visited.add(t)
for name in crategraph:cratewalk(name)
def closure(t):
 found=set();todo=[t]
 while todo:
  name=todo.pop()
  for p in crategraph[name]:
   if p not in found:found.add(p);todo.append(p)
 return found
server={'df-model','df-content','df-rules','df-engine','df-session','df-auth','df-persistence','df-providers','df-provider-api','df-ai','df-media','df-server'}
for browser in ['df-client','df-ui','df-render','df-audio','df-player','df-display','df-web']:assert not closure(browser)&server,browser
checks['crate_dag_and_server_free_browser']=True
# Markdown local file links and cited archived evidence.
links=0
for p in [ROOT/'AGENTS.md',*sorted((ROOT/'ADR').glob('*.md')),*sorted((ROOT/'planning').glob('*.md'))]:
 for link in re.findall(r'\]\(([^)]+)\)',p.read_text()):
  link=link.strip('<>').split('#',1)[0]
  if not link or ':' in link:continue
  assert (p.parent/link).exists(),(p,link);links+=1
checks['local_markdown_links']=links
archive=ROOT.parent/'dungeonflux.old';devlog=(archive/'docs/devlog.html').read_text()
ids=set();paths=set()
for p in [*sorted((ROOT/'ADR').glob('*.md')), ROOT/'planning/runtime-reliability.md',ROOT/'planning/feature-inventory.md']:
 text=p.read_text();ids.update(re.findall(r'`(e-[^`]+)`',text))
 paths.update(re.findall(r'`((?:internal|web|proto|scripts|notes|cmd)/[^`]+)`',text))
for ident in ids:assert ident in devlog,ident
for path in paths:
 if not (archive/path).exists():
  moved=ROOT/'development/evidence/website-migration-provenance.json'
  assert moved.exists(),path
  provenance=json.loads(moved.read_text())
  assert any(path in str(item) for item in provenance.get('files',[])),('missing archive evidence without retained relocation',path)
checks['archive_devlog_ids']=len(ids);checks['archive_evidence_paths']=len(paths)
checks['dispatch_ready']=c.execute('SELECT count(*) FROM dispatch_ready_tasks').fetchone()[0]
checks['game_features_done']=c.execute("SELECT count(*) FROM features WHERE kind!='planning' AND status='done'").fetchone()[0]
# Negative schema checks on disposable in-memory copy; they cannot mutate durable plans/evidence.
s=sqlite3.connect(':memory:');c.backup(s);s.execute('PRAGMA foreign_keys=ON')
negative=[]
def rejects(label,sql,args=()):
 s.execute('SAVEPOINT negative')
 try:
  s.execute(sql,args)
 except sqlite3.IntegrityError as e:negative.append({'case':label,'result':'rejected','reason':str(e)})
 else:raise AssertionError('Invalid schema transition accepted: '+label)
 finally:s.execute('ROLLBACK TO negative');s.execute('RELEASE negative')
rejects('dependency cycle',"INSERT INTO dependencies VALUES ('G01-RESOLVE','G02-RESOLVE')")
rejects('dangling prerequisite',"INSERT INTO dependencies VALUES ('G01-RESOLVE','not-a-task')")
rejects('blocked task lacks reason',"UPDATE tasks SET status='blocked',blocking_reason=NULL WHERE id='G01-RESOLVE'")
rejects('task done without independent integrated evidence',"UPDATE tasks SET status='done' WHERE id='G01-RESOLVE'")
rejects('feature done while required tasks unfinished',"UPDATE features SET status='done' WHERE id='G01'")
now=datetime.datetime.now(datetime.timezone.utc);past=(now-datetime.timedelta(hours=1)).isoformat();future=(now+datetime.timedelta(hours=1)).isoformat()
columns='id,task_id,worker,model,role,status,phase,brief_revision,brief_json,source_revision,lease_owner,lease_token,lease_generation,lease_expires_at,evidence_json,defects_json,started_at'
base=('test-a','G01-RESOLVE','worker','model','worker','active','implementation','r','{}','source','worker','token',1,future,'[]','[]',now.isoformat())
s.execute(f'INSERT INTO attempts ({columns}) VALUES ({",".join("?" for _ in base)})',base)
rejects('one live attempt per task',f'INSERT INTO attempts ({columns}) VALUES ({",".join("?" for _ in base)})',('test-b',*base[1:11],'other-token',*base[12:]))
rejects('unclaimed attempt submission',"UPDATE attempts SET status='submitted',phase='review' WHERE id='test-a'")
s.execute("UPDATE tasks SET active_attempt_id='test-a',lease_generation=1 WHERE id='G01-RESOLVE'")
rejects('malformed lease expiry',"UPDATE attempts SET lease_expires_at='not-a-date' WHERE id='test-a'")
rejects('blank lease token',"UPDATE attempts SET lease_token='' WHERE id='test-a'")
rejects('NULL lease owner',"UPDATE attempts SET lease_owner=NULL WHERE id='test-a'")
rejects('active terminal phase cannot hide live worker',"UPDATE attempts SET phase='terminal' WHERE id='test-a'")
rejects('claimed acceptance cannot change',"UPDATE tasks SET acceptance_json='[\"different requirement\"]' WHERE id='G01-RESOLVE'")
s.execute("UPDATE attempts SET lease_expires_at=? WHERE id='test-a'",(past,))
rejects('expired attempt submission',"UPDATE attempts SET status='submitted',phase='review' WHERE id='test-a'")
s.execute("UPDATE attempts SET lease_expires_at=? WHERE id='test-a'",(future,))
s.execute("UPDATE attempts SET lease_generation=2 WHERE id='test-a'")
rejects('superseded generation submission',"UPDATE attempts SET status='submitted',phase='review' WHERE id='test-a'")
s.execute("UPDATE attempts SET lease_generation=1 WHERE id='test-a'")
s.execute("UPDATE attempts SET status='submitted',phase='review',lease_owner='other' WHERE id='test-a'")
rejects('self approval',"UPDATE attempts SET status='approved',verdict='approve',evaluator_id='worker',evaluator_model='frontier',tested_revision='source',capabilities_json='{\"frontier\":true}',review_evidence_json='[\"claim\"]' WHERE id='test-a'")
rejects('approval lacks criterion evidence',"UPDATE attempts SET status='approved',verdict='approve',evaluator_id='other',evaluator_model='frontier',tested_revision='source',capabilities_json='{\"frontier\":true}' WHERE id='test-a'")
rejects('NULL stable feature identity',"INSERT INTO features SELECT NULL,kind,title,goal,acceptance_json,priority,status,plan_json,source_fingerprint,created_at,updated_at FROM features WHERE id='G01'")
rejects('blank stable feature identity',"INSERT INTO features SELECT ' ',kind,title,goal,acceptance_json,priority,status,plan_json,source_fingerprint,created_at,updated_at FROM features WHERE id='G01'")
rejects('direct terminal-integrated attempt INSERT',f'INSERT INTO attempts ({columns}) VALUES ({",".join("?" for _ in base)})',('test-direct',*base[1:5],'integrated','terminal',*base[7:11],'direct-token',*base[12:]))
rejects('cross-task active attempt link',"UPDATE tasks SET active_attempt_id='test-a',lease_generation=1 WHERE id='G02-RESOLVE'")
# Positive evidence transition and changed integration invalidation.
criteria=json.loads(s.execute("SELECT acceptance_json FROM tasks WHERE id='G01-RESOLVE'").fetchone()[0])
s.execute("UPDATE attempts SET status='approved',phase='review',verdict='approve',evaluator_id='other',evaluator_model='frontier',tested_revision='source',capabilities_json='{\"frontier\":true}',review_evidence_json=? WHERE id='test-a'",(json.dumps([{'criterion':v,'result':'pass','evidence_ref':'isolated actual check'} for v in criteria]),))
rejects('incomplete criterion coverage',"UPDATE attempts SET review_evidence_json='[{\"criterion\":\"unrelated\",\"result\":\"pass\",\"evidence_ref\":\"claim\"}]' WHERE id='test-a'")
rejects('blank evaluator model',"UPDATE attempts SET evaluator_model=' ' WHERE id='test-a'")
rejects('frontier capability removed after approval',"UPDATE attempts SET capabilities_json='{}' WHERE id='test-a'")
rejects('post-approval self-evaluator mutation',"UPDATE attempts SET evaluator_id='worker' WHERE id='test-a'")
rejects('attempt worker identity mutation',"UPDATE attempts SET worker='other' WHERE id='test-a'")
s.execute("UPDATE tasks SET status='review' WHERE id='G01-RESOLVE'")
rejects('integration cannot skip tracked phase',"UPDATE attempts SET status='integrated',phase='terminal',integrated_revision='source',integration_evidence_json='[\"check\"]' WHERE id='test-a'")
s.execute("UPDATE attempts SET phase='integration',lease_owner='coordinator' WHERE id='test-a'")
s.execute("UPDATE attempts SET status='integrated',phase='terminal',integrated_revision='changed',integration_evidence_json='[\"integration check\"]' WHERE id='test-a'")
rejects('changed integrated revision cannot close reviewed task',"UPDATE tasks SET status='done' WHERE id='G01-RESOLVE'")
s.execute("UPDATE attempts SET integrated_revision='source' WHERE id='test-a'")
s.execute("UPDATE tasks SET status='done' WHERE id='G01-RESOLVE'")
checks['positive_independent_review_integration_transition']=True
log=('test-log',now.isoformat(),'worker','worker','G01-RESOLVE','test-a','discovery','schema probe','Observed test fixture','probe','recorded',None,None,None)
s.execute('INSERT INTO devlog VALUES ('+','.join('?' for _ in log)+')',log)
rejects('devlog update',"UPDATE devlog SET outcome='overwrite' WHERE id='test-log'")
rejects('devlog delete',"DELETE FROM devlog WHERE id='test-log'")
rejects('devlog task attempt mismatch','INSERT INTO devlog VALUES ('+','.join('?' for _ in log)+')',('other-log',*log[1:4],'G02-RESOLVE',*log[5:]))
checks['negative_schema_checks']=negative
result={'date':now.isoformat(),'candidate_manifest_sha256':digest(BASE/'plan-manifest.json'),'schema_sha256':digest(BASE/'schema.sql'),'source_fingerprint':m['source_fingerprint'],'checks':checks,'unperformed':['Game/native/WASM execution, rendered browser and physical-device/audio acceptance; no game implementation exists.','Actual source/book access/catalog freeze, vendor/tool/device/budget decisions remain G01–G12 work.']}
(ROOT.parent/'work/gap-refinement-critic/independent-verification.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'integrity':checks['integrity'],'task_dag':checks['acyclic_tasks'],'negative_checks':len(negative),'source_docs':checks['manifest_input_fingerprints'],'links':links,'archive_ids':len(ids),'archive_paths':len(paths),'dispatch_ready':checks['dispatch_ready']}))
