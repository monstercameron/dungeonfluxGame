import pathlib, subprocess, json, hashlib, os, time, datetime
R=pathlib.Path('/Users/earlcameron/Desktop/dungeonflux'); W=R/'artifacts/worktrees/contracts-g03'; E=R/'development/evidence/contracts-g03/review'; S=R/'artifacts/tmp/CONTRACT-G03-001-review'; B=R/'artifacts/build/contracts-g03'; C='9898c70614a8555efa63d3cec47ba9adbc395bbb'
def sha(p): return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
def put(n,d): (E/n).write_text(json.dumps(d,indent=2)+'\n')
def run(n,a,expected=0,timeout=180):
 t=time.monotonic(); p=subprocess.run(list(map(str,a)),cwd=W,env=env,capture_output=True,timeout=timeout); (E/(n+'.stdout')).write_bytes(p.stdout); (E/(n+'.stderr')).write_bytes(p.stderr); x=dict(name=n,argv=list(map(str,a)),cwd=str(W),exit_code=p.returncode,expected=expected,seconds=round(time.monotonic()-t,3),passed=p.returncode==expected,stdout_sha256=sha(E/(n+'.stdout')),stderr_sha256=sha(E/(n+'.stderr'))); results.append(x); put('command-results.json',results); print(n,p.returncode,flush=True); assert p.returncode==expected,(n,p.stderr.decode()[:2000]); return p.stdout
art=json.loads((E.parent/'worker/artifact-identities.json').read_text()); src=json.loads((E.parent/'worker/source-identity.json').read_text()); brief=json.loads((E.parent/'brief.json').read_text()); env=os.environ.copy(); env.update(DUNGEONFLUX_ARTIFACT_ROOT=str(R/'artifacts'),DUNGEONFLUX_BUILD_ROOT=str(B),DUNGEONFLUX_TMP_ROOT=str(S),CARGO_NET_OFFLINE='true',CARGO_HOME=str(R/'artifacts/cache/cargo'),RUSTUP_HOME=str(R/'artifacts/cache/rustup'),TMPDIR=str(S)); env['PATH']=str(R/'artifacts/cache/cargo/bin')+':'+env['PATH']; results=[]
ident={'source':{p:sha(W/p)==h for p,h in src['source_sha256'].items()},'required_docs':{p:sha(W/p)==h for p,h in brief['source_hashes'].items()},'artifacts_before':{a['path']:sha(a['path'])==a['sha256'] for a in art['artifacts']+art['generated_descriptors']},'handoff_sha256':sha(E.parent/'worker/handoff.json')}; assert all(ident['source'].values()) and all(ident['required_docs'].values()) and all(ident['artifacts_before'].values()); put('identity.json',ident)
assert run('head',['git','rev-parse','HEAD']).decode().strip()==C
assert not run('clean',['git','status','--porcelain']).strip()
assert not run('protected-app-diff',['git','diff','f79bebf6f88f72f80bfb62d71798ebc56f64ca96','HEAD','--','crates/df-tools/src','crates/df-rpc-bridge','crates/df-observe','crates/df-protocol/proto/transport_fixture.proto']).strip()
# Independent actual executions of pinned test binaries.
for a in art['artifacts'][-2:]: run('frozen-'+pathlib.Path(a['path']).name,[a['path'],'--test-threads=1'])
# Resolve exact rlibs via Cargo's recorded little-endian fingerprints, not arbitrary filename selection.
f=json.loads((B/'debug/.fingerprint/df-tools-0be3eba5daacf3c5/test-integration-test-shared_contracts.json').read_text()); ex={}
for _,name,_,finger in f['deps']:
 if name not in ['df_protocol','df_types','prost','prost_types']:continue
 for p in (B/'debug/.fingerprint').glob('*/lib-'+name):
  if int.from_bytes(bytes.fromhex(p.read_text()),'little')==finger:
   rlib=B/'debug/deps'/('lib'+name+'-'+p.parent.name.rsplit('-',1)[1]+'.rlib')
   if rlib.exists(): ex[name]=rlib
assert len(ex)==4; ident['linked_rlibs']={n:{'path':str(p),'sha256':sha(p)} for n,p in ex.items()}; put('identity.json',ident)
for stem in ['shared_contracts','schema_ledger']:
 source=S/(stem+'_independent.rs'); source.write_text('include!("'+str(W/'crates/df-tools/tests'/ (stem+'.rs'))+'");\n'+(E/(stem+'-extra.rs')).read_text()); output=S/(stem+'_independent'); args=[R/'artifacts/cache/cargo/bin/rustc','--edition=2024','--test',source,'-L','dependency='+str(B/'debug/deps'),'-o',output];
 for n,p in ex.items(): args+=['--extern',n+'='+str(p)]
 run('compile-independent-'+stem,args); run('execute-independent-'+stem,[output,'--test-threads=1'])
# Repeat current source gates through pinned wrapper, serial/offline; no fixture server or preview replacement.
w=W/'development/build-fixture.sh'
commands=[('fmt',['fmt','--all','--','--check']),('clippy-native',['clippy','--locked','--offline','--jobs','1','--workspace','--all-targets','--','-D','warnings']),('test-native',['test','--locked','--offline','--jobs','1','--workspace']),('clippy-contract-wasm',['clippy','--locked','--offline','--jobs','1','-p','df-types','-p','df-protocol','--target','wasm32-unknown-unknown','--','-D','warnings']),('clippy-tools-wasm',['clippy','--locked','--offline','--jobs','1','-p','df-tools','--lib','--target','wasm32-unknown-unknown','--','-D','warnings']),('compile-consumer-wasm',['test','--locked','--offline','--jobs','1','-p','df-tools','--test','shared_contracts','--test','schema_ledger','--target','wasm32-unknown-unknown','--no-run'])]
for n,a in commands:run(n,[w,'cargo']+a)
for target in ['aarch64-apple-darwin','wasm32-unknown-unknown']:
 for crate in ['df-types','df-protocol']:run('tree-'+crate+'-'+target,[w,'cargo','tree','--locked','--offline','--target',target,'-p',crate,'--edges','normal'])
run('historical-audit-stale',[brief['bundled_python'],W/'development/dependency-audit.py'],expected=1)
ident['artifacts_after']={a['path']:sha(a['path'])==a['sha256'] for a in art['artifacts']+art['generated_descriptors']}; ident['source_after']={p:sha(W/p)==h for p,h in src['source_sha256'].items()}; assert all(ident['artifacts_after'].values()) and all(ident['source_after'].values()); put('identity.json',ident)
print('ALL PASS',flush=True)
