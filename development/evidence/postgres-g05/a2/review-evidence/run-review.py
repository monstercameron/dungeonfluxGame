import os, sys, json, time, pathlib, subprocess, hashlib, socket, signal, shutil
R=pathlib.Path('/Users/earlcameron/Desktop/dungeonflux'); E=R/'e'; O=R/'development/evidence/postgres-g05/a2/review-evidence'; D=E/'development/runtime/postgres-g05'; DATA=D/'data'; BIN=E/'artifacts/build/postgres-g05/install/bin'; S=E/'development/postgres.sh'; records=[]
def run(label,argv,timeout=65,env=None):
 t=time.monotonic(); p=subprocess.run(list(map(str,argv)),capture_output=True,text=True,timeout=timeout,env=env); a={'label':label,'argv':list(map(str,argv)),'exit':p.returncode,'stdout':p.stdout,'stderr':p.stderr,'seconds':round(time.monotonic()-t,3)};records.append(a);(O/'native-results.json').write_text(json.dumps(records,indent=2)+'\n');print(label,p.returncode,a['seconds'],p.stdout.strip()[:130],p.stderr.strip()[:150],flush=True);return p
def act(label,action):return run(label,[S,action])
def sql(label,q):return run(label,[BIN/'psql','-XAt','-v','ON_ERROR_STOP=1','-h',D/'socket','-p','55439','-d','postgres','-c',q])
def writepid(text):
 p=DATA/'postmaster.pid';p.write_text(text);p.chmod(0o600)
def fake(pid):return f'{pid}\n{DATA}\n{int(time.time())}\n55439\n{D}/socket\n\n0\nready\n'
run('shell-syntax',['bash','-n',S]);act('stopped-before','status');act('setup-preserves-existing','setup');act('start-normal','start');act('ready-normal','status');act('start-idempotent','start');sql('settings',"select version(),current_setting('data_directory'),current_setting('listen_addresses'),current_setting('max_connections'),current_setting('max_worker_processes'),current_setting('max_parallel_workers'),current_setting('max_parallel_workers_per_gather'),current_setting('autovacuum_max_workers'),current_setting('fsync'),current_setting('full_page_writes'),current_setting('synchronous_commit'),inet_server_addr() is null")
sql('rollback','BEGIN; CREATE TABLE eval_a2_rolledback(n int); INSERT INTO eval_a2_rolledback VALUES (186); ROLLBACK; SELECT to_regclass(\'public.eval_a2_rolledback\') IS NULL;');sql('commit','BEGIN; CREATE TABLE eval_a2_committed(n int); INSERT INTO eval_a2_committed VALUES (186); COMMIT; SELECT n FROM eval_a2_committed;');run('no-tcp-listener',['lsof','-nP','-a','-p',(DATA/'postmaster.pid').read_text().splitlines()[0],'-iTCP','-sTCP:LISTEN']);act('stop-normal','stop');act('restart','start');sql('restart-persistence','SELECT n FROM eval_a2_committed;');act('stop-after-restart','stop')
# Invalid or reused PID files and stale Unix socket. Only the dummy is evaluator-owned.
dummy_code="import signal,time,pathlib; p=pathlib.Path("+repr(str(O/'dummy-signals.txt'))+"); signal.signal(signal.SIGINT,lambda s,f:p.write_text(str(s))); signal.signal(signal.SIGTERM,lambda s,f:exit(0)); time.sleep(180)"
dummy=subprocess.Popen([sys.executable,'-c',dummy_code]);(O/'dummy-owner.json').write_text(json.dumps({'pid':dummy.pid,'creator':'/root/postgres_review_a2','command':dummy_code}))
try:
 for label,text in [('malformed','x\n'),('dead',fake(99999999)),('foreign-executable',fake(dummy.pid))]:
  writepid(text)
  for action in ['start','status','stop']: act(label+'-'+action,action)
  (DATA/'postmaster.pid').unlink()
 stale=D/'socket/.s.PGSQL.55439';sock=socket.socket(socket.AF_UNIX);sock.bind(str(stale));sock.close()
 writepid(fake(dummy.pid))
 for action in ['start','status','stop']:act('stale-socket-dummy-'+action,action)
 (DATA/'postmaster.pid').unlink()
 for action in ['start','status','stop']:act('orphan-socket-'+action,action)
 stale.unlink()
 records.append({'label':'dummy-not-signaled','pid':dummy.pid,'signal_record_exists':(O/'dummy-signals.txt').exists(),'still_alive':dummy.poll() is None})
finally:
 dummy.terminate();dummy.wait(timeout=5)
# Runtime ancestor symlink, every lifecycle action, restored even if probe fails.
runtime=E/'development/runtime';saved=E/'development/runtime-a2-evaluator-preserved';runtime.rename(saved);runtime.symlink_to(saved,target_is_directory=True)
try:
 for action in ['setup','start','status','stop']:act('ancestor-symlink-'+action,action)
finally: runtime.unlink();saved.rename(runtime)
act('orphan-fixture-start','start');pidfile=DATA/'postmaster.pid';original=pidfile.read_bytes();pidfile.unlink()
try:
 for action in ['start','status','stop']:act('live-server-no-pid-'+action,action)
finally: pidfile.write_bytes(original);pidfile.chmod(0o600)
# Deliberately stalled readiness tool: record its child and verify deadline cleanup.
ready=BIN/'pg_isready';backup=BIN/'pg_isready-a2-original';ready.rename(backup)
ready.write_text('#!/usr/bin/env python3\nimport os,subprocess,time,pathlib\np=subprocess.Popen(["/bin/sleep","120"])\npathlib.Path('+repr(str(O/'stalled-child.json'))+').write_text(__import__("json").dumps({"parent":os.getpid(),"child":p.pid}))\ntime.sleep(120)\n');ready.chmod(0o755)
try:act('stalled-readiness-status','status')
finally:ready.unlink();backup.rename(ready)
child=json.loads((O/'stalled-child.json').read_text());run('stalled-child-cleanup',['ps','-ww','-p',','.join(map(str,child.values())),'-o','pid=,stat=,command=']);act('ready-after-timeout','status');act('final-stop-phase1','stop')
(O/'native-results.json').write_text(json.dumps(records,indent=2)+'\n')
