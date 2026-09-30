from pathlib import Path
import os,subprocess,json,socket,sys,threading,hashlib,datetime
root=Path('/Users/earlcameron/Desktop/dungeonflux');e=root/'e';out=root/'development/evidence/postgres-g05/review-evidence';runtime=e/'development/runtime/postgres-g05';data=runtime/'data';sockdir=runtime/'socket';bin=e/'artifacts/build/postgres-g05/install/bin';results=[]
def run(name,args,timeout=45):
 p=subprocess.run(args,cwd=e,text=True,capture_output=True,timeout=timeout)
 r={'name':name,'command':args,'exit_code':p.returncode,'stdout':p.stdout,'stderr':p.stderr};results.append(r);return p
script=['./development/postgres.sh']
psql=[str(bin/'psql'),'-XAtq','-v','ON_ERROR_STOP=1','-h',str(sockdir),'-p','55439','-d','postgres']
transaction="BEGIN; CREATE TABLE evaluation_rollback(value integer); INSERT INTO evaluation_rollback VALUES (99); ROLLBACK; SELECT CASE WHEN to_regclass('public.evaluation_rollback') IS NULL THEN 'rollback-pass' ELSE 'rollback-fail' END; BEGIN; CREATE TABLE evaluation_committed(value integer primary key); INSERT INTO evaluation_committed VALUES (186); COMMIT; SELECT 'commit='||value FROM evaluation_committed;"
p=run('real-transactions',psql+['-c',transaction]);assert p.returncode==0 and 'rollback-pass' in p.stdout and 'commit=186' in p.stdout
before={str(p.relative_to(runtime)):hashlib.sha256(p.read_bytes()).hexdigest() for p in [runtime/'owner.json',data/'postgresql.conf',data/'global/pg_control']}
assert run('existing-data-preserved',script+['setup']).returncode==0
assert run('graceful-stop',script+['stop']).returncode==0
assert run('restart',script+['start']).returncode==0
p=run('restart-row-preserved',psql+['-c',"SELECT 'persisted='||value FROM evaluation_committed;"]);assert p.returncode==0 and 'persisted=186' in p.stdout
assert run('stop-before-safe-refusal-probes',script+['stop']).returncode==0
owner=runtime/'owner.json';original=owner.read_bytes();modified=json.loads(original);modified['uid']+=1
try:
 owner.write_text(json.dumps(modified));assert run('wrong-owner-refusal',script+['status']).returncode!=0
finally:owner.write_bytes(original)
try:
 sockdir.chmod(0o755);assert run('unsafe-permissions-refusal',script+['status']).returncode!=0
finally:sockdir.chmod(0o700)
# Owned ancestor symlink: stopped data moved intact, and restored after read-only probe.
ancestor=e/'development/runtime';preserved=e/'development/runtime-evaluator-preserved'
assert not preserved.exists();ancestor.rename(preserved)
try:
 ancestor.symlink_to(preserved,target_is_directory=True)
 run('ancestor-symlink-refusal-expected',script+['status'])
finally:
 ancestor.unlink();preserved.rename(ancestor)
# Recycled PID test uses ONLY an evaluator-owned signal-catching Python child.
signal_record=out/'dummy-signal.json';pidfile=data/'postmaster.pid';assert not pidfile.exists()
dummy_code='''import signal,time,sys,json,os\nfrom pathlib import Path\ndef received(sig,frame):\n Path(sys.argv[1]).write_text(json.dumps({"pid":os.getpid(),"signal":sig,"owned_evaluator_dummy":True}))\n sys.exit(0)\nsignal.signal(signal.SIGINT,received)\nprint("ready",flush=True)\nwhile True:time.sleep(1)\n'''
dummy=subprocess.Popen([sys.executable,'-c',dummy_code,str(signal_record)],stdout=subprocess.PIPE,text=True)
assert dummy.stdout.readline().strip()=='ready';thread=threading.Thread(target=dummy.wait,daemon=True);thread.start()
sockpath=sockdir/'.s.PGSQL.55439';fake=None
try:
 pidfile.write_text(str(dummy.pid)+'\n'+str(data)+'\n')
 run('foreign-live-pid-start-must-refuse',script+['start'])
 fake=socket.socket(socket.AF_UNIX,socket.SOCK_STREAM);fake.bind(str(sockpath));fake.close();fake=None
 run('foreign-live-pid-stop-must-not-signal',script+['stop'])
finally:
 if dummy.poll() is None:dummy.terminate();dummy.wait(timeout=5)
 if fake:fake.close()
 if pidfile.exists():pidfile.rename(out/'probe-postmaster.pid')
 if sockpath.exists():sockpath.unlink()
# Final evaluator cluster is stopped and retained; main cluster untouched.
assert run('final-evaluation-status',script+['status']).returncode==0
record={'timestamp':datetime.datetime.now(datetime.timezone.utc).isoformat(),'evaluation_repo':str(e),'results':results,'before_existing_setup_hashes':before,'dummy_signal':json.loads(signal_record.read_text()) if signal_record.exists() else None,'main_cluster_untouched':True,'evaluator_cluster_retained_stopped':True}
(out/'native-boundaries.json').write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(record,indent=2))
