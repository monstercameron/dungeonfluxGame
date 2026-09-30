import ast,hashlib,json,os,shutil,subprocess,time
from pathlib import Path
M=Path('/Users/earlcameron/Desktop/dungeonflux');O=M/'development/evidence/dependency-g01/a2/integration';T=M/'artifacts/tmp/AUDIT-G01-CLOSURE-001-a2-integration-review';PY='/Users/earlcameron/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/bin/python3'
S='development/dependency-audit.py';REP='development/dependency-audit/report.json';source=(M/S).read_text();pin=next(ast.literal_eval(n.value) for n in ast.parse(source).body if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='PINNED_INPUTS' for t in n.targets));rows=[]
def run(name,cmd,expected):
 start=time.monotonic();p=subprocess.run(cmd,cwd=T,capture_output=True,text=True,timeout=2);row={'name':name,'command':cmd,'cwd':str(T),'exit_code':p.returncode,'stdout':p.stdout,'stderr':p.stderr,'seconds':time.monotonic()-start,'pass':p.returncode==expected and not p.stderr};rows.append(row)
run('main-positive-foreign-cwd',[PY,str(M/S)],0)
def fixture(name):
 d=T/name;d.mkdir()
 for rel in (*pin,REP,S):
  path=d/rel;path.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(M/rel,path)
 return d
for rel,name in [('development/dependency-audit/metadata','metadata-parent'),('development/dependency-audit','report-parent'),('crates','source-parent')]:
 d=fixture(name);p=d/rel;outside=T/(name+'-outside');outside.mkdir();p.rename(outside/p.name);p.symlink_to(outside/p.name,target_is_directory=True);run(name,[PY,str(d/S)],1)
for rel,name in [('Cargo.toml','source-leaf-fifo'),(REP,'report-leaf-fifo'),('development/dependency-audit/metadata/native.json','metadata-leaf-symlink')]:
 d=fixture(name);p=d/rel;p.unlink()
 if name.endswith('fifo'):os.mkfifo(p)
 else:p.symlink_to(M/rel)
 run(name,[PY,str(d/S)],1)
load="from pathlib import Path;import os,json;s=Path("+repr(str(M/S))+");ns={'__file__':str(s),'__name__':'audit_test'};exec(compile(s.read_text(),str(s),'exec'),ns);"
fdcode='''
def fds():
 a=[]
 for i in range(512):
  try:os.fstat(i);a.append(i)
  except OSError:pass
 return a
before=fds();outcomes=[]
for i in range(20):
 try:ns['verify']();outcomes.append('success')
 except (ValueError,OSError):outcomes.append('refusal')
after=fds();print(json.dumps({'before':before,'after':after,'outcomes':outcomes}));assert before==after
'''
run('main-descriptor-success',[PY,'-c',load+fdcode],0)
run('merged-descriptor-refusal',[PY,'-c',load+"ns['ROOT']="+repr(str(T/'metadata-parent'))+";"+fdcode],0)
paths=['development/dependency-audit.md',S,'development/dependency-audit/metadata/native.json','development/dependency-audit/metadata/wasm.json','development/dependency-audit/packages.json',REP];identities=[]
for rel in paths:
 blob=subprocess.check_output(['git','show','f79bebf6f88f72f80bfb62d71798ebc56f64ca96:'+rel],cwd=M);actual=(M/rel).read_bytes();identities.append({'path':rel,'sha256':hashlib.sha256(actual).hexdigest(),'matches_reviewed_git_blob':blob==actual})
cmds={}
for name,args in [('head',['git','rev-parse','HEAD']),('app_diff822',['git','diff','822ffc4992b34ab548a47cf91efad845ca50c716','--','Cargo.toml','Cargo.lock','.cargo','rust-toolchain.toml','rustfmt.toml','crates','development/build-fixture.sh']),('app_diffba0',['git','diff','ba0d3860fb27124be352b48766e99a955f746672','--','Cargo.toml','Cargo.lock','.cargo','rust-toolchain.toml','rustfmt.toml','crates','development/build-fixture.sh']),('source_clean',['git','status','--porcelain','--']+paths)]:
 p=subprocess.run(args,cwd=M,capture_output=True,text=True,timeout=5);cmds[name]={'args':args,'exit_code':p.returncode,'stdout':p.stdout,'stderr':p.stderr}
proof={'rows':rows,'identity':identities,'commands':cmds,'build_scope':'Actualmergedsourcef79 tested. Runningapplication remains separatelypinnedba0; no appbuild or revisedappbuild identity claimed.'};(O/'boundary-results.json').write_text(json.dumps(proof,indent=2)+'\n');assert all(r['pass'] for r in rows) and all(i['matches_reviewed_git_blob'] for i in identities);print(json.dumps(proof,indent=2))
