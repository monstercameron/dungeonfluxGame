import pathlib,subprocess,time,json,os,hashlib
R=pathlib.Path('/Users/earlcameron/Desktop/dungeonflux');E=R/'e';O=R/'development/evidence/postgres-g05/a2/review-evidence';D=E/'development/runtime/postgres-g05';A=D/'data';B=D/'data-suffix-a2';K=D/'s2';BIN=E/'artifacts/build/postgres-g05/install/bin';S=E/'development/postgres.sh';out=[]
def run(label,argv,timeout=65):
 t=time.monotonic();p=subprocess.run(list(map(str,argv)),capture_output=True,text=True,timeout=timeout);out.append({'label':label,'argv':list(map(str,argv)),'exit':p.returncode,'stdout':p.stdout,'stderr':p.stderr,'seconds':round(time.monotonic()-t,3)});(O/'prefix-results.json').write_text(json.dumps(out,indent=2)+'\n');print(label,p.returncode,p.stdout[:160],p.stderr[:160],flush=True);return p
assert not (A/'postmaster.pid').exists(); assert not B.exists(); B.mkdir(mode=0o700);K.mkdir(mode=0o700)
(O/'prefix-fixture-owner.json').write_text(json.dumps({'owner':'/root/postgres_review_a2','main':str(A),'additional_owned_cluster':str(B),'purpose':'independent same-binary prefix-path process identity probe; preserve both clusters; only owned processes signaled'},indent=2)+'\n')
run('initdb-suffix',[BIN/'initdb','-D',B,'--encoding=UTF8','--locale=C','--auth-local=trust','--auth-host=reject'])
run('start-main',[S,'start']);main_pidfile=(A/'postmaster.pid').read_bytes(); main_pid=main_pidfile.decode().splitlines()[0]
run('start-suffix',[BIN/'pg_ctl','-D',B,'-l',D/'suffix-a2.log','-o',f"-c listen_addresses='' -c unix_socket_directories='{K}' -c port=55440 -c unix_socket_permissions=0700",'-w','-t','10','start'])
suffix_pid=(B/'postmaster.pid').read_text().splitlines()[0]
try:
 lines=main_pidfile.decode().splitlines();lines[0]=suffix_pid;(A/'postmaster.pid').write_text('\n'.join(lines)+'\n');(A/'postmaster.pid').chmod(0o600)
 (O/'forged-main-pidfile.txt').write_text((A/'postmaster.pid').read_text());(O/'real-main-pidfile.txt').write_bytes(main_pidfile)
 run('process-identity-before',['ps','-ww','-p',main_pid+','+suffix_pid,'-o','pid=,lstart=,command='])
 run('forged-prefix-status',[S,'status']);run('forged-prefix-start',[S,'start']);run('forged-prefix-stop',[S,'stop'],65)
 run('process-identity-after',['ps','-ww','-p',main_pid+','+suffix_pid,'-o','pid=,lstart=,command='])
 run('main-remains-ready',[BIN/'pg_isready','-h',D/'socket','-p','55439','-t','2'])
 run('suffix-no-longer-ready',[BIN/'pg_isready','-h',K,'-p','55440','-t','2'])
finally:
 (A/'postmaster.pid').write_bytes(main_pidfile);(A/'postmaster.pid').chmod(0o600)
 run('restore-main-stop',[S,'stop'])
 if (B/'postmaster.pid').exists():run('owned-suffix-cleanup',[BIN/'pg_ctl','-D',B,'-m','fast','-w','-t','10','stop'])
 (O/'suffix-server.log').write_bytes((D/'suffix-a2.log').read_bytes());(O/'evaluation-server.log').write_bytes((D/'server.log').read_bytes())
