import os,stat,json,hashlib,datetime,subprocess,sqlite3,collections
from pathlib import Path
R=Path('/Users/earlcameron/Desktop/dungeonflux');B=R/'development/evidence/cleanup-008';report=json.loads((B/'current-interval-inventory.json').read_text());old={};current={};errors=[];sumerrors=[]
def flatten(n):
 old[n['path']]={k:n[k] for k in ['kind','bytes'] if k in n}
 if n['kind']=='directory':
  for c in n.get('children',[]):flatten(c)
  if sum(c.get('bytes',0) for c in n.get('children',[]) if c['kind'] in ['directory','regular_file'])!=n['bytes']:sumerrors.append(n['path'])
for n in report['candidates']:flatten(n)
def scan(p):
 rel=str(p.relative_to(R))
 try:s=p.lstat()
 except OSError as e:errors.append([rel,str(e)]);return 0
 kind='symlink' if stat.S_ISLNK(s.st_mode) else 'directory' if stat.S_ISDIR(s.st_mode) else 'regular_file' if stat.S_ISREG(s.st_mode) else 'special';n={'kind':kind,'bytes':s.st_size if kind=='regular_file' else 0};current[rel]=n
 if kind=='directory':
  for c in sorted(p.iterdir(),key=lambda x:x.name):n['bytes']+=scan(c)
 return n['bytes'] if kind in ['regular_file','directory'] else 0
for rel in report['scope']:
 p=R/rel;assert stat.S_ISDIR(p.lstat().st_mode)
 for child in sorted(p.iterdir(),key=lambda x:x.name):scan(child)
def counts(entries):
 n=collections.Counter(x['kind'] for x in entries.values());return {'regular_files':n['regular_file'],'regular_file_bytes':sum(x['bytes'] for x in entries.values() if x['kind']=='regular_file'),'directories':n['directory'],'symlinks_not_followed':n['symlink'],'special_entries':n['special']}
prior=counts(old);assert all(report['totals'][k]==v for k,v in prior.items()) and not sumerrors
changes=[{'path':p,'submitted':old.get(p),'current':current.get(p)} for p in sorted(old.keys()|current.keys()) if old.get(p)!=current.get(p)]
def run(args):
 p=subprocess.run(args,cwd=R,capture_output=True,text=True,timeout=5);return {'command':args,'exit_code':p.returncode,'stdout':p.stdout,'stderr':p.stderr}
identities=[]
for name,item in report['runtime_build_identity'].items():
 if not isinstance(item,dict):continue
 p=R/item['path'];assert stat.S_ISREG(p.lstat().st_mode);h=hashlib.sha256(p.read_bytes()).hexdigest();identities.append({'name':name,'path':item['path'],'sha256':h,'matches':h==item['sha256']})
c=sqlite3.connect((R/'development/workflow.sqlite3').as_uri()+'?mode=ro',uri=True);c.row_factory=sqlite3.Row
j={'at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'method':'Independent lstat eachentry, recurse directoryonly, neveropen symlinktarget orspecial/FIFO; logicalregularbytes.','report_sha256':hashlib.sha256((B/'current-interval-inventory.json').read_bytes()).hexdigest(),'script_sha256':hashlib.sha256((R/report['inventory_script']).read_bytes()).hexdigest(),'submitted_recomputed':prior,'current':counts(current),'sum_errors':sumerrors,'current_errors':errors,'dynamic_changes':changes,'missing_submitted_paths':sorted(old.keys()-current.keys()),'protected_binary_hashes':identities,'processes':run(['ps','-p','18905,18907,25530,27721,80873','-o','pid,ppid,lstart,command']),'listeners':run(['lsof','-nP','-iTCP:43180','-iTCP:43181','-iTCP:43182','-sTCP:LISTEN']),'current_head':run(['git','rev-parse','HEAD'])['stdout'].strip(),'claims':[dict(r) for r in c.execute("SELECT id,status,phase,worker,evaluator_id FROM attempts WHERE id IN ('ACTIVE-CLEANUP-008-a1','AUDIT-G01-CLOSURE-001-a2','DIAGNOSE-G02-RECOVERY-001-a1')")],'worker_devlog':[dict(r) for r in c.execute("SELECT id,kind,summary,outcome FROM devlog WHERE attempt_id='ACTIVE-CLEANUP-008-a1'")],'all_retained':all(n['classification']=='retained' and not n['eligible'] and not n['deleted'] for n in report['candidates'])}
(B/'independent-boundary.json').write_text(json.dumps(j,indent=2)+'\n');print(json.dumps({k:v for k,v in j.items() if k not in ['dynamic_changes','processes','listeners']},indent=2));print('changed roots',sorted(set('/'.join(x['path'].split('/')[:3]) for x in changes)))
