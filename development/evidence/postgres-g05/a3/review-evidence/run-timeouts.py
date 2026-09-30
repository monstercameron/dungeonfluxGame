from pathlib import Path
import subprocess,json,os,time,sys,shutil,hashlib
R=Path('/Users/earlcameron/Desktop/dungeonflux');E=R/'e';O=R/'development/evidence/postgres-g05/a3/review-evidence';D=E/'development/runtime/postgres-g05';P=D/'data/postmaster.pid';B=E/'artifacts/build/postgres-g05/install/bin';S=E/'development/postgres.sh';records=[]
def run(label,args,want=0,env=None):
 t=time.monotonic();r=subprocess.run(list(map(str,args)),capture_output=True,text=True,timeout=65,env=env);rec={'label':label,'argv':list(map(str,args)),'exit':r.returncode,'stdout':r.stdout,'stderr':r.stderr,'seconds':round(time.monotonic()-t,3)};records.append(rec);(O/'independent-timeouts.json').write_text(json.dumps(records,indent=2)+'\n');print(label,r.returncode,flush=True);assert r.returncode==want,rec;return r
assert not P.exists();run('timeout-fixture-start',[S,'start']);real=P.read_bytes()
try:
 # Missing PID of a still-running owned server is not treated as safely stopped.
 P.unlink()
 try:
  for a in ('start','status','stop'):
   r=run('live-orphan-'+a,[S,a],1);assert 'without its PID file' in r.stderr
 finally:P.write_bytes(real);P.chmod(0o600)
 # Each injected executable owns its sleep child; timeout must clear both.
 for name in ('pg_isready','psql'):
  tool=B/name;backup=B/(name+'-eval3-timeout-preserved');tool.rename(backup);receipt=O/(name+'-stall-pids.json')
  tool.write_text('#!/usr/bin/env python3\nimport subprocess,os,time,json,pathlib\np=subprocess.Popen(["/bin/sleep","120"]);pathlib.Path('+repr(str(receipt))+').write_text(json.dumps({"parent":os.getpid(),"child":p.pid}));time.sleep(120)\n');tool.chmod(0o755)
  try:
   r=run(name+'-bounded-timeout',[S,'status'],1);assert r.stdout=='' and 'deadline' in r.stderr and r.returncode!=0;assert records[-1]['seconds']<15
   ids=json.loads(receipt.read_text())
   for role,pid in ids.items():
    r=run(name+'-'+role+'-gone',['ps','-p',pid,'-o','pid=,stat=,command='],1)
  finally:tool.unlink();backup.rename(tool)
 # Parameter/credential-looking environment value must not leak into diagnostics.
 env=os.environ.copy();env['PGPASSWORD']='EVAL_SECRET_SENTINEL_NOT_A_CREDENTIAL';env['PGOPTIONS']='-c statement_timeout=1';r=run('safe-diagnostic-output',[S,'status'],env=env);assert env['PGPASSWORD'] not in r.stdout+r.stderr
 run('readiness-after-timeouts',[B/'pg_isready','-h',D/'socket','-p','55439','-t','2'])
finally:
 run('timeout-fixture-stop',[S,'stop'])
run('final-stopped',[S,'status']);(O/'evaluation-server.log').write_bytes((D/'server.log').read_bytes());print('PHASE2 PASS',len(records),flush=True)
