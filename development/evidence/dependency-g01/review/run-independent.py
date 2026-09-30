import ast,hashlib,json,os,shutil,stat,subprocess,time
from pathlib import Path
MAIN=Path('/Users/earlcameron/Desktop/dungeonflux')
ROOT=MAIN/'artifacts/worktrees/audit-g01'
OUT=MAIN/'development/evidence/dependency-g01/review'
TMP=MAIN/'artifacts/tmp/AUDIT-G01-CLOSURE-001-review'
PYTHON='/Users/earlcameron/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/bin/python3'
source=(ROOT/'development/dependency-audit.py').read_text()
module=ast.parse(source)
pinned=next(ast.literal_eval(n.value) for n in module.body if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='PINNED_INPUTS' for t in n.targets))
report_rel='development/dependency-audit/report.json'
results=[]
def run(name,cmd,cwd=ROOT,env=None,timeout=15):
 start=time.monotonic()
 try:
  p=subprocess.run(cmd,cwd=cwd,env=env,capture_output=True,timeout=timeout)
  row={'name':name,'command':cmd,'cwd':str(cwd),'exit_code':p.returncode,'seconds':time.monotonic()-start,'stdout':p.stdout.decode(errors='replace'),'stderr':p.stderr.decode(errors='replace')}
 except subprocess.TimeoutExpired as e:
  row={'name':name,'command':cmd,'cwd':str(cwd),'timeout':True,'seconds':time.monotonic()-start}
 results.append(row);return row
run('actual-success',[PYTHON,'development/dependency-audit.py'])
run('python-version',[PYTHON,'--version'])
run('syntax',[PYTHON,'-c',"from pathlib import Path; compile(Path('development/dependency-audit.py').read_text(), 'development/dependency-audit.py', 'exec')"])
run('source-diff-check',['git','diff','--check'])
run('commit-diff-check',['git','diff','822ffc4992b34ab548a47cf91efad845ca50c716','HEAD','--check'])
run('source-status',['git','status','--porcelain'])
run('changed-paths',['git','diff','--name-only','822ffc4992b34ab548a47cf91efad845ca50c716','HEAD'])
run('head',['git','rev-parse','HEAD'])
run('app-input-diff',['git','diff','822ffc4992b34ab548a47cf91efad845ca50c716','HEAD','--','Cargo.toml','Cargo.lock','rust-toolchain.toml','rustfmt.toml','.cargo','crates','development/build-fixture.sh'])
def fixture(name):
 d=TMP/name;d.mkdir()
 for rel in (*pinned,report_rel,'development/dependency-audit.py'):
  target=d/rel;target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(ROOT/rel,target)
 return d
def rebind(d,changed):
 r=json.loads((d/report_rel).read_text())
 for rel in changed:r['input_sha256'][rel]=hashlib.sha256((d/rel).read_bytes()).hexdigest()
 (d/report_rel).write_text(json.dumps(r,sort_keys=True))
 new_hash=hashlib.sha256((d/report_rel).read_bytes()).hexdigest()
 old_hash=hashlib.sha256((ROOT/report_rel).read_bytes()).hexdigest()
 (d/'development/dependency-audit.py').write_text(source.replace(old_hash,new_hash))
def check(name,mutator):
 d=fixture(name);mutator(d);row=run(name,[PYTHON,str(d/'development/dependency-audit.py')],d,timeout=5);row['fixture']=str(d)
check('edited-report',lambda d:(d/report_rel).write_text('{}'))
check('stale-source',lambda d:(d/'Cargo.toml').write_text((d/'Cargo.toml').read_text()+'\n# negative\n'))
def omit(d):
 rel='development/dependency-audit/packages.json';j=json.loads((d/rel).read_text());j['packages'].pop();(d/rel).write_text(json.dumps(j));rebind(d,[rel])
check('omitted-package-rebound-parser',omit)
def malformed(d):
 rel='development/dependency-audit/metadata/native.json';(d/rel).write_text('{bad');rebind(d,[rel])
check('malformed-metadata-rebound-parser',malformed)
check('missing-input',lambda d:(d/'Cargo.toml').unlink())
def special(d,kind):
 p=d/'Cargo.toml';p.unlink()
 if kind=='fifo':os.mkfifo(p)
 elif kind=='directory':p.mkdir()
 elif kind=='symlink':p.symlink_to(ROOT/'Cargo.toml')
check('fifo-input',lambda d:special(d,'fifo'))
check('directory-input',lambda d:special(d,'directory'))
check('symlink-input',lambda d:special(d,'symlink'))
def oversized(d):
 with (d/'Cargo.toml').open('wb') as f:f.truncate(32*1024*1024+1)
check('oversized-input',oversized)
def parent_symlink(d):
 inside=d/'development/dependency-audit/metadata';outside=TMP/'controlled-outside-parent';outside.mkdir();inside.rename(outside/'metadata');inside.symlink_to(outside/'metadata',target_is_directory=True)
check('symlink-parent-outside-root',parent_symlink)
# Return no secret data: outside-root negative above references only evaluator-owned copied fixture metadata.
(OUT/'independent-verifier.json').write_text(json.dumps(results,indent=2)+'\n')
print(json.dumps([{k:r[k] for k in ['name','exit_code','timeout','stdout','stderr'] if k in r} for r in results],indent=2))
