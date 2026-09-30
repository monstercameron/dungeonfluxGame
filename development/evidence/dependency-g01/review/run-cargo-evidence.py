import hashlib,json,os,subprocess,time
from pathlib import Path
MAIN=Path('/Users/earlcameron/Desktop/dungeonflux');ROOT=MAIN/'artifacts/worktrees/audit-g01';OUT=MAIN/'development/evidence/dependency-g01/review';TMP=MAIN/'artifacts/tmp/AUDIT-G01-CLOSURE-001-review'
env=dict(os.environ,DUNGEONFLUX_ARTIFACT_ROOT=str(MAIN/'artifacts'),DUNGEONFLUX_BUILD_ROOT=str(MAIN/'artifacts/build/audit-g01'),DUNGEONFLUX_TMP_ROOT=str(TMP),CARGO_NET_OFFLINE='true')
cmds=[('metadata-native',['metadata','--locked','--offline','--format-version','1','--filter-platform','aarch64-apple-darwin']),('metadata-wasm',['metadata','--locked','--offline','--format-version','1','--filter-platform','wasm32-unknown-unknown']),('tree-native',['tree','--locked','--offline','--target','aarch64-apple-darwin','--edges','normal,build,features','-p','df-tools']),('tree-wasm',['tree','--locked','--offline','--target','wasm32-unknown-unknown','--edges','normal,build,features','-p','df-tools']),('tree-all-native',['tree','--locked','--offline','--target','aarch64-apple-darwin','--edges','all','--prefix','depth','-p','df-tools']),('tree-all-wasm',['tree','--locked','--offline','--target','wasm32-unknown-unknown','--edges','all','--prefix','depth','-p','df-tools']),('tree-rpc-dev-native',['tree','--locked','--offline','--target','aarch64-apple-darwin','-e','dev','-p','df-rpc-bridge']),('tree-protocol-build',['tree','--locked','--offline','--target','aarch64-apple-darwin','-e','build','-p','df-protocol']),('cargo-version',['--version','--verbose'])]
rows=[];started=time.monotonic()
for name,args in cmds:
 start=time.monotonic();p=subprocess.run(['./development/build-fixture.sh','cargo']+args,cwd=ROOT,env=env,capture_output=True,timeout=min(60,300-(start-started)))
 suffix='.json' if name.startswith('metadata-') else '.txt';path=OUT/(name+suffix);path.write_bytes(p.stdout);(OUT/(name+'.stderr.txt')).write_bytes(p.stderr)
 row={'name':name,'command':['./development/build-fixture.sh','cargo']+args,'exit_code':p.returncode,'seconds':time.monotonic()-start,'sha256':hashlib.sha256(p.stdout).hexdigest(),'stderr':p.stderr.decode(),'output':str(path)}
 target=ROOT/'development/dependency-audit/metadata'/('native.json' if name.endswith('native') else 'wasm.json') if name.startswith('metadata-') else MAIN/'development/evidence/dependency-g01/worker/commands'/(name+'.txt')
 if target.exists():row['matches_captured_bytes']=p.stdout==target.read_bytes()
 rows.append(row)
(OUT/'cargo-boundary.json').write_text(json.dumps({'environment':{k:env[k] for k in env if k.startswith('DUNGEONFLUX') or k=='CARGO_NET_OFFLINE'},'results':rows,'total_seconds':time.monotonic()-started},indent=2)+'\n')
print(json.dumps(rows,indent=2))
