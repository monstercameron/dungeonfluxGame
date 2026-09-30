import datetime,json,os,pathlib,signal,subprocess,sys,time
ROOT=pathlib.Path('/Users/earlcameron/Desktop/dungeonflux/artifacts/worktrees/contracts-g03')
EVIDENCE=pathlib.Path('/Users/earlcameron/Desktop/dungeonflux/development/evidence/contracts-g03/worker/commands')
name,limit,*args=sys.argv[1:]
limit=int(limit)
env=dict(os.environ,DUNGEONFLUX_ARTIFACT_ROOT='/Users/earlcameron/Desktop/dungeonflux/artifacts',DUNGEONFLUX_BUILD_ROOT='/Users/earlcameron/Desktop/dungeonflux/artifacts/build/contracts-g03',DUNGEONFLUX_TMP_ROOT='/Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/CONTRACT-G03-001-a1',CARGO_NET_OFFLINE='true')
argv=args[1:] if args and args[0]=='direct' else [str(ROOT/'development/build-fixture.sh'),*args]
commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
start=time.monotonic();peak=0;sample_count=0;timeout=False
pressure=subprocess.check_output(['memory_pressure'],text=True)
(EVIDENCE/(name+'.memory-before.txt')).write_text(pressure)
with (EVIDENCE/(name+'.txt')).open('w') as out:
 p=subprocess.Popen(argv,cwd=ROOT,env=env,stdout=out,stderr=subprocess.STDOUT,start_new_session=True)
 while p.poll() is None:
  lines=subprocess.check_output(['ps','-axo','pid,ppid,rss'],text=True).splitlines()[1:]
  rows=[tuple(map(int,line.split())) for line in lines if len(line.split())==3]
  owned={p.pid}
  for _ in range(8): owned.update(pid for pid,parent,rss in rows if parent in owned)
  peak=max(peak,sum(rss for pid,parent,rss in rows if pid in owned));sample_count+=1
  if time.monotonic()-start>limit:
   timeout=True;os.killpg(p.pid,signal.SIGTERM)
   try:p.wait(timeout=5)
   except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL)
   break
  time.sleep(.25)
 code=p.wait()
result=dict(name=name,argv=argv,environment={k:env[k] for k in ['DUNGEONFLUX_ARTIFACT_ROOT','DUNGEONFLUX_BUILD_ROOT','DUNGEONFLUX_TMP_ROOT','CARGO_NET_OFFLINE']},commit=commit,exit_code=code,timed_out=timeout,elapsed_seconds=round(time.monotonic()-start,3),observed_peak_process_tree_rss_kib=peak,rss_sample_count=sample_count,at=datetime.datetime.now(datetime.timezone.utc).isoformat())
(EVIDENCE/(name+'.json')).write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result));sys.exit(code if code>=0 else 1)
