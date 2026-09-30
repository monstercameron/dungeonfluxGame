from pathlib import Path
import os,sys,subprocess,json,time,shutil,signal,socket,hashlib
R=Path('/Users/earlcameron/Desktop/dungeonflux');E=R/'e';O=R/'development/evidence/postgres-g05/a3/review-evidence';D=E/'development/runtime/postgres-g05';A=D/'data';K=D/'socket';B=E/'artifacts/build/postgres-g05/install/bin';S=E/'development/postgres.sh';P=A/'postmaster.pid';records=[]
def run(label,args,want=0,timeout=65,env=None):
 args=list(map(str,args));t=time.monotonic();r=subprocess.run(args,text=True,capture_output=True,timeout=timeout,env=env);rec={'label':label,'argv':args,'exit':r.returncode,'stdout':r.stdout,'stderr':r.stderr,'seconds':round(time.monotonic()-t,3)};records.append(rec);(O/'independent-boundaries.json').write_text(json.dumps(records,indent=2)+'\n');print(label,r.returncode,flush=True);assert r.returncode==want,rec;return r
def act(action,want=0,label=None):return run(label or action,[S,action],want)
def sql(label,q):return run(label,[B/'psql','-XAtq','-v','ON_ERROR_STOP=1','-w','-h',K,'-p','55439','-d','postgres','-c',q])
def refuse(label,actions=('start','status','stop'),text=None):
 for a in actions:
  r=act(a,1,label+'-'+a)
  if text:assert text in r.stderr,r.stderr

def writepid(contents):P.write_bytes(contents);P.chmod(0o600)
run('syntax',['bash','-n',S]);act('status');act('setup');act('start');act('status');act('start',label='start-idempotent')
mainpid=P.read_text().splitlines()[0]
settings=sql('settings',"select version(),current_setting('listen_addresses'),current_setting('max_connections'),current_setting('max_worker_processes'),current_setting('max_parallel_workers'),current_setting('max_parallel_workers_per_gather'),current_setting('autovacuum_max_workers'),current_setting('fsync'),current_setting('full_page_writes'),current_setting('synchronous_commit'),inet_server_addr() is null;")
assert '|16|8|8|2|3|on|on|on|t' in settings.stdout
run('no-owned-tcp',['lsof','-nP','-a','-p',mainpid,'-iTCP','-sTCP:LISTEN'],1)
for f in (D,A,K):assert (f.stat().st_mode&0o777)==0o700
q="BEGIN;CREATE TABLE evaluator_a3_rollback(v int);INSERT INTO evaluator_a3_rollback VALUES(186);ROLLBACK;SELECT to_regclass('public.evaluator_a3_rollback') IS NULL;BEGIN;CREATE TABLE evaluator_a3_commit(v int);INSERT INTO evaluator_a3_commit VALUES(186);COMMIT;SELECT v FROM evaluator_a3_commit;"
r=sql('real-transactions',q);assert r.stdout.strip()=='t\n186',r.stdout
act('stop');act('start');assert sql('restart-persistence','SELECT v FROM evaluator_a3_commit;').stdout.strip()=='186';act('stop')
# Bounded stopped-state refusal tests; retain all original files/directories.
saved=D.with_name('postgres-g05-evaluator-a3-preserved');D.rename(saved)
try:act('status',3,'absent-status')
finally:saved.rename(D)
for content,name in [(b'not-a-pid\n','malformed-pid'),(b'999999999\n','incomplete-dead-pid')]:
 writepid(content)
 try:refuse(name)
 finally:P.unlink()
# Dummy can catch an erroneous signal, but only this owned process is ever exposed.
dummy_code="import os,signal,time,pathlib; p=pathlib.Path("+repr(str(O/'dummy-signals.txt'))+"); signal.signal(signal.SIGINT,lambda s,f:p.write_text('SIGINT')); signal.signal(signal.SIGTERM,lambda s,f:exit(0)); pathlib.Path("+repr(str(O/'dummy-ready.txt'))+").write_text(str(os.getpid())); time.sleep(120)"
dummy=subprocess.Popen([sys.executable,'-c',dummy_code],start_new_session=True)
try:
 for _ in range(100):
  if (O/'dummy-ready.txt').exists():break
  time.sleep(.01)
 writepid(f'{dummy.pid}\n{A}\n{int(time.time())}\n55439\n{K}\n\n'.encode());refuse('foreign-dummy',text='PID executable does not match');assert not (O/'dummy-signals.txt').exists()
finally:
 if P.exists():P.unlink()
 dummy.terminate();dummy.wait(timeout=3)
# Stale socket inode is not readiness.
so=socket.socket(socket.AF_UNIX);so.bind(str(K/'.s.PGSQL.55439'));so.close()
try:refuse('stale-socket')
finally:(K/'.s.PGSQL.55439').unlink()
# Every lifecycle command refuses a symlinked existing runtime ancestor and log.
for target,label in [(E/'development/runtime','ancestor'),(D/'server.log','log')]:
 backup=target.with_name(target.name+'-evaluator-a3-preserved');target.rename(backup)
 sentinel=O/'symlink-sentinel.log'
 if label=='log':sentinel.write_bytes(b'EVALUATOR_A3_SENTINEL\n');target.symlink_to(sentinel)
 else:target.symlink_to(backup)
 try:
  refuse('symlink-'+label,('setup','start','status','stop'),'refusing symlink')
  if label=='log':assert sentinel.read_bytes()==b'EVALUATOR_A3_SENTINEL\n'
 finally:target.unlink();backup.rename(target)
owner=D/'owner.json';owner_bytes=owner.read_bytes();j=json.loads(owner_bytes);j['uid']+=1;owner.write_text(json.dumps(j))
try:refuse('foreign-owner',('setup','start','status','stop'))
finally:owner.write_bytes(owner_bytes)
A.chmod(0o755)
try:refuse('unsafe-data-mode',('setup','start','status','stop'))
finally:A.chmod(0o700)
# Fresh suffix cluster provides exact original prefix regression without resetting retained data.
SU=D/'data-suffix-eval3';SK=D/'ev3';assert not SU.exists();SU.mkdir(mode=0o700);SK.mkdir(mode=0o700)
run('suffix-initdb',[B/'initdb','-D',SU,'--encoding=UTF8','--locale=C','--auth-local=trust','--auth-host=reject'])
common=['-c','listen_addresses=','-c','unix_socket_permissions=0700','-c','max_connections=16','-c','max_worker_processes=8','-c','max_parallel_workers=8','-c','max_parallel_workers_per_gather=2','-c','autovacuum_max_workers=3','-c','fsync=on','-c','full_page_writes=on','-c','synchronous_commit=on']
# Launch near start of same second and verify actual postmaster epochs.
time.sleep((1-time.time()%1)+.03)
mainlog=open(D/'server.log','ab');suflog=open(D/'suffix-eval3.log','ab')
main=subprocess.Popen([str(B/'postgres'),'-D',str(A),'-c','unix_socket_directories='+str(K),'-c','port=55439']+common,stdout=mainlog,stderr=mainlog,start_new_session=True)
suffix=subprocess.Popen([str(B/'postgres'),'-D',str(SU),'-c','unix_socket_directories='+str(SK),'-c','port=55440']+common,stdout=suflog,stderr=suflog,start_new_session=True)
try:
 for _ in range(200):
  if P.exists() and (SU/'postmaster.pid').exists() and len(P.read_text().splitlines())>=8 and len((SU/'postmaster.pid').read_text().splitlines())>=8:break
  time.sleep(.02)
 real=P.read_bytes();other=(SU/'postmaster.pid').read_bytes();m=real.decode().splitlines();s=other.decode().splitlines();assert m[2]==s[2],(m,s);assert int(m[0])==main.pid and int(s[0])==suffix.pid
 (O/'same-second-identities.json').write_text(json.dumps({'main_pid':main.pid,'suffix_pid':suffix.pid,'main_epoch':m[2],'suffix_epoch':s[2],'main_data':str(A),'suffix_data':str(SU)},indent=2)+'\n')
 act('status',label='same-second-main-valid');m[0]=str(suffix.pid);writepid(('\n'.join(m)+'\n').encode())
 try:refuse('same-second-prefix-pid',text='PID command line does not name')
 finally:writepid(real)
 assert main.poll() is None and suffix.poll() is None
 # The right PID with a socket answering from the wrong server must still be refused.
 tool=B/'psql';keep=B/'psql-eval3-preserved';tool.rename(keep)
 wrapper="#!/usr/bin/env python3\nimport os,sys\na=sys.argv[1:];a[a.index('-h')+1]="+repr(str(SK))+";a[a.index('-p')+1]='55440';os.execv("+repr(str(keep))+",["+repr(str(keep))+"]+a)\n"
 tool.write_text(wrapper);tool.chmod(0o755)
 try:refuse('wrong-answering-backend',text='SQL backend does not belong')
 finally:tool.unlink();keep.rename(tool)
 assert main.poll() is None and suffix.poll() is None
 # Stop must use the verified PID even when PID-file changes at the signal boundary.
 tool=B/'pg_ctl';keep=B/'pg_ctl-eval3-preserved';tool.rename(keep);receipt=O/'pidfile-race.json'
 wrapper="#!/usr/bin/env python3\nimport pathlib,os,json,sys\np=pathlib.Path("+repr(str(P))+");lines=p.read_text().splitlines();before=lines[0];lines[0]="+repr(str(suffix.pid))+";p.write_text('\\n'.join(lines)+'\\n');p.chmod(0o600);pathlib.Path("+repr(str(receipt))+").write_text(json.dumps({'argv':sys.argv,'before':before,'after':lines[0]}));os.execv("+repr(str(keep))+",["+repr(str(keep))+"]+sys.argv[1:])\n"
 tool.write_text(wrapper);tool.chmod(0o755)
 try:act('stop',label='captured-pid-stop')
 finally:tool.unlink();keep.rename(tool)
 main.wait(timeout=5);assert suffix.poll() is None;assert json.loads(receipt.read_text())['argv'][1:]==['kill','INT',str(main.pid)]
 run('suffix-survives-captured-stop',[B/'pg_isready','-h',SK,'-p','55440','-t','2'])
finally:
 if main.poll() is None:
  if 'real' in locals():writepid(real)
  main.send_signal(signal.SIGINT);main.wait(timeout=10)
 if suffix.poll() is None:suffix.send_signal(signal.SIGINT);suffix.wait(timeout=10)
 mainlog.close();suflog.close()
act('status',label='final-stopped-phase1');(O/'evaluation-server.log').write_bytes((D/'server.log').read_bytes());(O/'suffix-server.log').write_bytes((D/'suffix-eval3.log').read_bytes());print('PHASE1 PASS',len(records),flush=True)
