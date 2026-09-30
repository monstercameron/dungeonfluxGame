from pathlib import Path
import os,stat,json,hashlib,subprocess,sqlite3,datetime
r=Path('/Users/earlcameron/Desktop/dungeonflux');e=r/'development/evidence/cleanup-006';report=e/'current-interval-inventory.json';j=json.loads(report.read_text());brief=json.loads((e/'brief.json').read_text())
plan={'criteria':brief['acceptance'],'procedure':['Verify exact immutable report/script SHA256 and recompute historical nested counts independently.','Walk only main artifacts/build and artifacts/tmp with lstat, classifying symlinks/FIFOs without opening or following them; retain all roots.','Compare historical/current leaves and account for dynamic differences.','Read current task phase/ownership and worker devlog from read-only workflow SQLite; inspect targeted ps/listeners/open files and hash retained runtime binaries.','Confirm empty allowlist and zero eligible/deleted/reclaimed; preserve all evidence and append scoped evaluator finding.']}
(e/'independent-check-plan.json').write_text(json.dumps(plan,indent=2)+'\n')
assert hashlib.sha256(report.read_bytes()).hexdigest()=='1434894af7a3ee2173454aa6633ddf00959ce11430e80e26e139fc6d0b30da8d'
script=r/j['inventory_script'];assert hashlib.sha256(script.read_bytes()).hexdigest()=='016de305699481a6c96dd65644a1f1e2b05797ea63043d33d77a3742fb636592'
def count(entries):
 out={'regular_files':0,'regular_file_bytes':0,'directories':0,'symlinks_not_followed':0,'special_entries':0,'errors':0}
 for v in entries.values():
  k=v['kind'];key={'regular_file':'regular_files','directory':'directories','symlink':'symlinks_not_followed','special':'special_entries','error':'errors'}[k];out[key]+=1
  if k=='regular_file':out['regular_file_bytes']+=v['bytes']
 return out
def flatten(node,d):
 d[node['path']]={k:node[k] for k in ['kind','bytes','symlink_target'] if k in node}
 for c in node.get('children',[]):flatten(c,d)
historical={}
for x in j['candidates']:flatten(x,historical)
hcounts=count(historical)
for k,v in hcounts.items():assert j['totals'][k]==v
current={};roots=[];errors=[]
for scope in ('artifacts/build','artifacts/tmp'):
 parent=r/scope;assert stat.S_ISDIR(parent.lstat().st_mode)
 roots.extend(sorted(parent.iterdir()))
for root in roots:
 stack=[root]
 while stack:
  p=stack.pop();rel=str(p.relative_to(r))
  try:
   s=p.lstat()
   if stat.S_ISLNK(s.st_mode):v={'kind':'symlink','bytes':0,'symlink_target':os.readlink(p)}
   elif stat.S_ISDIR(s.st_mode):v={'kind':'directory'};stack.extend(sorted(p.iterdir(),reverse=True))
   elif stat.S_ISREG(s.st_mode):v={'kind':'regular_file','bytes':s.st_size}
   else:v={'kind':'special','bytes':0,'type':'fifo' if stat.S_ISFIFO(s.st_mode) else 'other'}
   current[rel]=v
  except OSError as ex:current[rel]={'kind':'error','error':str(ex)};errors.append(rel)
missing=sorted(set(historical)-set(current));added=sorted(set(current)-set(historical));changes=[]
for path in sorted(set(historical)&set(current)):
 a=historical[path];b=current[path]
 if a['kind']!=b['kind'] or (a['kind']!='directory' and any(a.get(k)!=b.get(k) for k in ('bytes','symlink_target'))):changes.append({'path':path,'historical':a,'current':b})
commands=[]
def run(label,args):
 p=subprocess.run(args,capture_output=True,text=True,timeout=10);commands.append({'label':label,'command':args,'exit':p.returncode,'stdout':p.stdout,'stderr':p.stderr});return p.stdout
run('processes',['ps','-p','18905,18907,25530,80873,69190,26743','-o','pid=,ppid=,lstart=,stat=,command='])
run('preview-listeners',['lsof','-nP','-iTCP:43180','-iTCP:43181','-iTCP:43182','-sTCP:LISTEN'])
run('postgres-unix',['lsof','-nP','-a','-p','80873','-U'])
run('recorder-open-file',['lsof','-nP','-a','-p','26743',str(r/'artifacts/tmp/journey/recorder.log')])
run('old-recorder',['ps','-p','69190','-o','pid=,command='])
head=run('current-head',['git','-C',str(r),'rev-parse','HEAD']).strip()
binaries={}
for k,v in j['runtime_build_identity'].items():
 p=r/v['path'];d=hashlib.sha256(p.read_bytes()).hexdigest();binaries[k]={'path':v['path'],'sha256':d,'matches_report':d==v['sha256']}
c=sqlite3.connect((r/'development/workflow.sqlite3').resolve().as_uri()+'?mode=ro',uri=True);c.row_factory=sqlite3.Row
attempts=[dict(x) for x in c.execute("select id,task_id,worker,status,phase,lease_owner,lease_generation,lease_expires_at,submitted_commit,integrated_commit,evaluator_id,verdict from attempts where status not in ('integrated','rejected','abandoned','cancelled') or id in ('SETUP-G05-001-a3','PIN-G07-SRD-001-a1','PIN-G07-SRD-001-a2')")]
devlog=[dict(x) for x in c.execute('select id,agent_id,role,task_id,attempt_id,kind,summary,evidence_ref from devlog where attempt_id=?',('ACTIVE-CLEANUP-006-a1',))];c.close()
active=json.loads((r/'development/evidence/journey/active.json').read_text());target=json.loads((r/'development/evidence/journey/target.json').read_text())
protected={str(p.relative_to(r)):{'kind':'directory' if p.is_dir() else 'file','exists':p.exists()} for p in [r/'e',r/'p',r/'s',r/'artifacts/cache',r/'artifacts/worktrees',r/'development/evidence',r/'development/runtime',r/'development/workflow.sqlite3']}
out={'timestamp_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'report_sha256':hashlib.sha256(report.read_bytes()).hexdigest(),'script_sha256':hashlib.sha256(script.read_bytes()).hexdigest(),'source_input_revision':j['input_revision'],'observed_current_head':head,'scope':['artifacts/build','artifacts/tmp'],'method':'Independent iterative lstat walk; descend only directory entries, never open symlinks/FIFOs. Logical regular-file bytes.','historical_recomputed_totals':hcounts,'current_totals':dict(candidates=len(roots),**count(current)),'root_summaries':[{'path':str(p.relative_to(r)),**count({k:v for k,v in current.items() if k==str(p.relative_to(r)) or k.startswith(str(p.relative_to(r))+'/')})} for p in roots],'missing_historical_paths':missing,'added_paths':added,'changed_non_directory_entries':changes,'scan_errors':errors,'retention':{'delete_allowlist':[],'eligible':0,'eligible_bytes':0,'deleted':0,'reclaimed_bytes':0,'all_candidates_retained':True},'commands':commands,'runtime_build_identity':binaries,'current_attempts':attempts,'worker_devlog':devlog,'recorder_active':active,'recorder_target':target,'protected_outside_scan':protected}
(e/'independent-inventory.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps({k:out[k] for k in ('historical_recomputed_totals','current_totals','missing_historical_paths','added_paths','changed_non_directory_entries','runtime_build_identity')},indent=2))
