import pathlib,subprocess,time,json
R=pathlib.Path('/Users/earlcameron/Desktop/dungeonflux');E=R/'e';O=R/'development/evidence/postgres-g05/a2/review-evidence';D=E/'development/runtime/postgres-g05';S=E/'development/postgres.sh';out=[]
def run(label,action):
 t=time.monotonic();p=subprocess.run([str(S),action],capture_output=True,text=True,timeout=65);out.append({'label':label,'argv':[str(S),action],'exit':p.returncode,'stdout':p.stdout,'stderr':p.stderr,'seconds':time.monotonic()-t});(O/'extra-boundaries.json').write_text(json.dumps(out,indent=2)+'\n');print(label,p.returncode,p.stdout[:140],p.stderr[:140],flush=True)
assert not (D/'data/postmaster.pid').exists()
# Absent runtime is tested by reversible rename of evaluator's stopped fixture only.
saved=D.with_name('postgres-g05-a2-preserved');D.rename(saved)
try:run('absent-runtime-status','status')
finally:saved.rename(D)
# Log path is an owned sentinel outside runtime, never any user file.
log=D/'server.log';savedlog=D/'server-log-a2-preserved';target=O/'symlink-target.log';target.write_text('EVALUATOR_SENTINEL\n');log.rename(savedlog);log.symlink_to(target)
try:
 run('log-symlink-start','start');run('log-symlink-status','status');run('log-symlink-stop','stop')
finally:
 log.unlink();savedlog.rename(log)
out.append({'label':'log-symlink-target-result','expected':'refuse symlink before appending','target':str(target),'bytes':target.stat().st_size,'sentinel_preserved':target.read_text().startswith('EVALUATOR_SENTINEL\n'),'server_messages_appended':'database system is ready to accept connections' in target.read_text()});(O/'extra-boundaries.json').write_text(json.dumps(out,indent=2)+'\n')
