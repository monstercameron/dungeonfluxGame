import ast,hashlib,json,os,shutil,subprocess,time
from pathlib import Path
M=Path('/Users/earlcameron/Desktop/dungeonflux');R=M/'artifacts/worktrees/audit-g01';O=M/'development/evidence/dependency-g01/a2/review';T=M/'artifacts/tmp/AUDIT-G01-CLOSURE-001-a2-review'
PY='/Users/earlcameron/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/bin/python3'
REL='development/dependency-audit.py';REPORT='development/dependency-audit/report.json';SOURCE=(R/REL).read_text()
PINNED=next(ast.literal_eval(n.value) for n in ast.parse(SOURCE).body if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='PINNED_INPUTS' for t in n.targets))
rows=[]
def run(name,cmd,expected,cwd=T):
 start=time.monotonic()
 try:
  p=subprocess.run(cmd,cwd=cwd,capture_output=True,text=True,timeout=2)
  row={'name':name,'command':cmd,'cwd':str(cwd),'seconds':time.monotonic()-start,'exit_code':p.returncode,'stdout':p.stdout,'stderr':p.stderr,'expected_exit':expected,'pass':p.returncode==expected and not p.stderr}
 except subprocess.TimeoutExpired:row={'name':name,'command':cmd,'timeout':True,'pass':False}
 rows.append(row);return row
run('current-positive-foreign-cwd',[PY,str(R/REL)],0)
def fixture(name):
 d=T/name;d.mkdir()
 for rel in (*PINNED,REPORT,REL):
  p=d/rel;p.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(R/rel,p)
 return d
def case(name,mutator,expected=1):
 d=fixture(name);mutator(d);row=run(name,[PY,str(d/REL)],expected);row['fixture']=str(d);return d
def rebind(d,changed):
 r=json.loads((d/REPORT).read_text())
 for rel in changed:r['input_sha256'][rel]=hashlib.sha256((d/rel).read_bytes()).hexdigest()
 (d/REPORT).write_text(json.dumps(r,sort_keys=True));new=hashlib.sha256((d/REPORT).read_bytes()).hexdigest();old=hashlib.sha256((R/REPORT).read_bytes()).hexdigest();(d/REL).write_text(SOURCE.replace(old,new))
case('edited-report',lambda d:(d/REPORT).write_text('{}'))
case('stale-source',lambda d:(d/'Cargo.toml').write_text((d/'Cargo.toml').read_text()+'\n# negative\n'))
def omitted(d):
 rel='development/dependency-audit/packages.json';j=json.loads((d/rel).read_text());j['packages'].pop();(d/rel).write_text(json.dumps(j));rebind(d,[rel])
case('omitted-package-rebound-parser',omitted)
def malformed(d):
 rel='development/dependency-audit/metadata/native.json';(d/rel).write_text('{bad');rebind(d,[rel])
case('malformed-metadata-rebound-parser',malformed)
case('missing-source',lambda d:(d/'Cargo.toml').unlink())
case('missing-report',lambda d:(d/REPORT).unlink())
def leaf(d,rel,kind):
 p=d/rel;p.unlink()
 if kind=='symlink':p.symlink_to(R/rel)
 elif kind=='fifo':os.mkfifo(p)
 elif kind=='directory':p.mkdir()
 elif kind=='oversized':
  with p.open('wb') as f:f.truncate(32*1024*1024+1)
for rel,label in [('Cargo.toml','source'),(REPORT,'report'),('development/dependency-audit/metadata/native.json','metadata'),('development/dependency-audit/packages.json','packages')]:
 case(label+'-leaf-symlink',lambda d,rel=rel:leaf(d,rel,'symlink'))
 for kind in ['fifo','directory','oversized']:
  case(label+'-'+kind,lambda d,rel=rel,kind=kind:leaf(d,rel,kind))
def ancestor(d,rel):
 p=d/rel;outside=T/(d.name+'-outside');outside.mkdir();p.rename(outside/p.name);p.symlink_to(outside/p.name,target_is_directory=True)
ancestors={}
for rel,label in [('development/dependency-audit/metadata','original-metadata-parent'),('development/dependency-audit','report-parent'),('development','development-parent'),('crates','source-parent'),('crates/df-rpc-bridge/vendor','vendor-parent')]:
 ancestors[label]=case(label+'-symlink',lambda d,rel=rel:ancestor(d,rel))
d=fixture('root-symlink-base');link=T/'root-symlink';link.symlink_to(d,target_is_directory=True);run('checkout-root-symlink',[PY,str(link/REL)],1)
load="from pathlib import Path; import os; s=Path("+repr(str(R/REL))+"); ns={'__file__':str(s),'__name__':'audit_test'};exec(compile(s.read_text(),str(s),'exec'),ns);"
run('unsupported-dir-fd',[PY,'-c',load+"os.supports_dir_fd=set();raise SystemExit(ns['main']())"],1)
for flag in ['O_NOFOLLOW','O_DIRECTORY','O_NONBLOCK','O_CLOEXEC']:
 run('unsupported-'+flag,[PY,'-c',load+"delattr(os,"+repr(flag)+");raise SystemExit(ns['main']())"],1)
fdcode='''
import os,json

def open_fds():
 result=[]
 for fd in range(512):
  try:os.fstat(fd);result.append(fd)
  except OSError:pass
 return result
before=open_fds(); outcomes=[]
for i in range(20):
 try:ns['verify']();outcomes.append('success')
 except (OSError,ValueError):outcomes.append('refusal')
after=open_fds();print(json.dumps({'before':before,'after':after,'outcomes':outcomes}));assert before==after
'''
run('descriptor-cleanup-success',[PY,'-c',load+fdcode],0)
for key,path in [('ancestor',ancestors['original-metadata-parent']),('leaf',T/'source-leaf-symlink'),('fifo',T/'source-fifo'),('missing',T/'missing-source')]:
 run('descriptor-cleanup-'+key,[PY,'-c',load+"ns['ROOT']="+repr(str(path))+";"+fdcode],0)
run('syntax',[PY,'-c',"from pathlib import Path;p=Path("+repr(str(R/REL))+");compile(p.read_text(),str(p),'exec')"],0)
(O/'boundary-results.json').write_text(json.dumps(rows,indent=2)+'\n')
print(json.dumps([{'name':r['name'],'pass':r['pass'],'seconds':r.get('seconds'),'stdout':r.get('stdout'),'stderr':r.get('stderr')} for r in rows],indent=2))
assert all(r['pass'] for r in rows)
