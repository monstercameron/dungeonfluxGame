#!/usr/bin/env python3
"""Project-only screenshot evidence; no desktop capture or gameplay implementation."""
import argparse,contextlib,datetime,fcntl,functools,hashlib,http.server,json,os
from pathlib import Path
import shutil,signal,stat,subprocess,tempfile,threading,time,uuid
from urllib.parse import urlsplit,unquote

PROJECT=Path(__file__).resolve().parents[1]
JOURNEY=PROJECT/'development/evidence/journey'
OUTPUTS=PROJECT.parent/'outputs'
DEFAULT_NODE=Path('/Users/earlcameron/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin/node')
DEFAULT_PLAYWRIGHT=Path('/Users/earlcameron/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright')
DEFAULT_BROWSER=Path('/Applications/Google Chrome Beta.app/Contents/MacOS/Google Chrome Beta')
CAPTURE=Path(__file__).with_name('capture_preview.mjs')
if not CAPTURE.exists():CAPTURE=Path(__file__).with_name('capture_preview.mjs')
CAP=512*1024*1024
MAX_SHOT=8*1024*1024

def utc():return datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds')
def write(path,data):
 path.parent.mkdir(parents=True,exist_ok=True)
 tmp=path.with_name(path.name+'.tmp');tmp.write_text(json.dumps(data,indent=2)+'\n');tmp.replace(path)
def load(path):return json.loads(path.read_text())
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def safe_doc_open(parts):
 fd=os.open(PROJECT/'docs',os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW)
 try:
  for part in parts[:-1]:
   nxt=os.open(part,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW,dir_fd=fd);os.close(fd);fd=nxt
  leaf=os.open(parts[-1],os.O_RDONLY|os.O_NOFOLLOW,dir_fd=fd)
  if not stat.S_ISREG(os.fstat(leaf).st_mode):os.close(leaf);raise ValueError('non_regular_preview_file')
  return os.fdopen(leaf,'rb')
 finally:os.close(fd)
def source_identity(config):
 paths=sorted((PROJECT/'docs').rglob('*')) if config['kind']=='static-site' else [PROJECT/p for p in config['source_paths']]
 hashes={}
 for p in paths:
  if p.is_symlink():raise ValueError('source_symlink_denied')
  if p.is_file():
   if config['kind']=='static-site':
    with safe_doc_open(p.relative_to(PROJECT/'docs').parts) as source:hashes[str(p.relative_to(PROJECT))]=hashlib.sha256(source.read()).hexdigest()
   else:hashes[str(p.relative_to(PROJECT))]=digest(p)
 return {'files':hashes,'sha256':hashlib.sha256(json.dumps(hashes,sort_keys=True).encode()).hexdigest()}
def admitted_url(url):
 p=urlsplit(url)
 if p.scheme!='http' or p.hostname not in ('127.0.0.1','localhost','::1') or p.username or p.password or p.query or p.fragment:
  raise ValueError('only_registered_loopback_preview_without_credentials_or_query_is_allowed')
 return url

def journal_size():return sum(p.stat().st_size for p in JOURNEY.rglob('*') if p.is_file() and not p.is_symlink())

def devlog_entry(record):
 source_ref=str((JOURNEY/'records'/f"{record['id']}.json").relative_to(PROJECT))
 def source_summary(source):
  if not isinstance(source,dict) or not isinstance(source.get('files'),dict):
   return None
  return {'sha256':source.get('sha256'),'file_count':len(source['files'])}
 details={
  'observed_at':record.get('observed_at'),
  'minute_bucket':record.get('minute_bucket'),
  'target':record.get('target'),
  'task_id':record.get('task_id'),
  'attempt_id':record.get('attempt_id'),
  'preview_kind':record.get('preview_kind'),
  'recorder_id':record.get('recorder_id'),
  'observed_work_context':record.get('observed_work_context'),
  'missed_minute_count':record.get('missed_minute_count'),
  'image':record.get('image'),
  'image_sha256':record.get('image_sha256'),
  'browser':record.get('browser'),
  'source_before':source_summary(record.get('source_before')),
  'source_after':source_summary(record.get('source_after')),
  'source_changed_during_capture':record.get('source_changed_during_capture'),
  'record_ref':source_ref,
  'reason':record.get('reason'),
 }
 return {'id':'JOURNEY-'+record['id'],
  'kind':'discovery' if record['outcome']=='captured' else 'challenge',
  'summary':'DungeonFlux visual journey '+record['outcome'],
  'details':json.dumps(details,sort_keys=True,separators=(',',':')),
  'action':'Capture only admitted public project preview in isolated headless browser; preserve actual evidence without queue mutation.',
  'outcome':record['outcome'],'evidence_ref':source_ref}

def devlog(record,context):
 entrypath=JOURNEY/'entries'/f"{record['id']}.json"
 entry=devlog_entry(record)
 if entrypath.exists():
  original=load(entrypath)
  if len(original.get('details',''))>16000:
   entrypath=JOURNEY/'entries'/f"{record['id']}.bounded.json"
   if entrypath.exists():
    if load(entrypath)!=entry:return 'pending_ingestion'
   else:write(entrypath,entry)
  elif original!=entry:
   # Accepted-size legacy entry bytes are immutable and remain the retry payload.
   entry=original
 else:
  write(entrypath,entry)
 if len(entry.get('details',''))>16000:return 'pending_ingestion'
 result=subprocess.run(['python3',str(PROJECT/'development/devlog.py'),'--context',str(context),'--entry',str(entrypath)],capture_output=True,text=True,timeout=15)
 return 'appended_or_existing' if result.returncode==0 else 'pending_ingestion'

def make_index():
 import html
 records=[load(p) for p in sorted((JOURNEY/'records').glob('*.json'))]
 records.sort(key=lambda record:(record['observed_at'],record['id']))
 cards=[]
 for r in records:
  image=r.get('image');src=''
  if image:src=f'<a href="../dungeonflux/{html.escape(image)}"><img loading="lazy" src="../dungeonflux/{html.escape(image)}" alt="Actual DungeonFlux preview at {html.escape(r["observed_at"])}"></a>'
  cards.append(f'<article><h2>{html.escape(r["observed_at"])} — {html.escape(r["outcome"])}</h2><p>{html.escape(r["preview_kind"])} · task {html.escape(str(r.get("task_id")))}</p>{src}<p>{html.escape(r.get("reason","Actual isolated browser capture"))}</p><a href="../dungeonflux/development/evidence/journey/records/{r["id"]}.json">Metadata/source identity</a></article>')
 OUTPUTS.mkdir(exist_ok=True)
 page='<!doctype html><meta charset="utf-8"><title>DungeonFlux visual development journey</title><style>body{font:16px system-ui;max-width:1100px;margin:32px auto;padding:0 20px;background:#131622;color:#eee}article{border-top:1px solid #48505e;padding:18px 0}img{max-width:100%;border:1px solid #555}a{color:#9ed8ff}h2{font-size:18px}</style><h1>DungeonFlux visual development journey</h1><p>Actual public project preview captures. The first static-site baseline is not a game implementation. One capture per active-development UTC minute; idle or expired leases do not capture. Scheduler timing is approximate. Evidence is retained, never deleted by age.</p>'+''.join(reversed(cards))
 (OUTPUTS/'dungeonflux-visual-journey.html').write_text(page)
 return records

class QuietHandler(http.server.SimpleHTTPRequestHandler):
 def log_message(self,*args):pass
 def send_head(self):
  parts=[p for p in unquote(urlsplit(self.path).path).split('/') if p]
  if any(p in ('.','..') for p in parts):self.send_error(403);return None
  if not parts or self.path.endswith('/'):parts.append('index.html')
  try:
   source=safe_doc_open(parts);size=os.fstat(source.fileno()).st_size
  except (OSError,ValueError):self.send_error(404);return None
  self.send_response(200);self.send_header('Content-Type',self.guess_type(parts[-1]));self.send_header('Content-Length',str(size));self.end_headers();return source

@contextlib.contextmanager
def preview(config):
 if config['kind']=='static-site':
  server=http.server.ThreadingHTTPServer(('127.0.0.1',0),functools.partial(QuietHandler,directory=str(PROJECT/'docs')))
  thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
  try:yield f'http://127.0.0.1:{server.server_port}/'
  finally:server.shutdown();server.server_close();thread.join(timeout=2)
 else:
  proof=load(Path(config['ownership_proof']))
  if proof.get('project_root')!=str(PROJECT) or proof.get('url')!=config['url'] or any(proof.get(k) is not True for k in ('public_safe','read_only','synthetic_public','no_load_effects')):
   raise ValueError('preview_ownership_proof_mismatch')
  os.kill(int(proof['owner_pid']),0)
  yield admitted_url(config['url'])

def cleanup_owned_profile(profile):
 if not profile.resolve().is_relative_to(PROJECT/'artifacts/tmp/journey'):
  raise ValueError('unowned_profile_cleanup_denied')
 processes=subprocess.run(['ps','-axo','pid=,args='],capture_output=True,text=True,timeout=3)
 marker='--user-data-dir='+str(profile)
 for line in processes.stdout.splitlines():
  fields=line.strip().split(None,1)
  if len(fields)==2 and marker in fields[1] and 'Google Chrome' in fields[1]:
   try:os.kill(int(fields[0]),signal.SIGTERM)
   except ProcessLookupError:pass

def flush_pending():
 for path in sorted((JOURNEY/'records').glob('*.json')):
  record=load(path)
  if record.get('devlog_status')!='pending_ingestion':continue
  context=Path(record.get('devlog_context',str(JOURNEY/'recorder-contexts'/(record['id'].split('-')[0]+'.json'))))
  if context.exists():record['devlog_status']=devlog(record,context);write(path,record)
  break

def capture(now=None,stage_id=None):
 now=time.time() if now is None else now
 statepath=JOURNEY/'active.json'
 if not statepath.exists():return {'result':'idle'}
 state=load(statepath)
 if not state.get('active') or state['expires_at_epoch']<=now:return {'result':'idle_or_expired'}
 flush_pending()
 registration_error=None
 try:config=load(JOURNEY/'target.json')
 except (FileNotFoundError,json.JSONDecodeError):
  config={'kind':'unavailable','label':'Missing or invalid registered project preview','disk_cap_bytes':CAP};registration_error='registered_preview_missing_or_invalid'
 context=Path(state['context']);ctx=load(context)
 bucket=int(now//60);rid=state['session_id']+'-'+str(bucket);recordpath=JOURNEY/'records'/f'{rid}.json'
 if recordpath.exists():
  r=load(recordpath)
  if r.get('devlog_status')=='pending_ingestion':r['devlog_status']=devlog(r,context);write(recordpath,r)
  return {'result':'coalesced','id':rid}
 if journal_size()+MAX_SHOT>config['disk_cap_bytes']:
  state['active']=False;state['stop_reason']='disk_cap';write(statepath,state)
  cap_record={'id':rid,'observed_at':utc(),'minute_bucket':bucket,'target':config['label'],'preview_kind':config['kind'],'task_id':ctx.get('task_id'),'attempt_id':ctx.get('attempt_id'),'recorder_id':ctx['agent_id'],'devlog_context':str(context),'outcome':'disk_cap_stopped','reason':'Retained evidence cap reached; no history deleted'}
  write(recordpath,cap_record);cap_record['devlog_status']=devlog(cap_record,context);write(recordpath,cap_record);make_index()
  return {'result':'disk_cap_stopped','action_required':True,'evidence_preserved':True}
 previous=[load(path)['minute_bucket'] for path in (JOURNEY/'records').glob(state['session_id']+'-*.json')]
 missed=max(0,bucket-max(previous)-1) if previous else 0
 r={'missed_minute_count':missed,'id':rid,'observed_at':utc(),'minute_bucket':bucket,'target':config['label'],'preview_kind':config['kind'],'task_id':ctx.get('task_id'),'attempt_id':ctx.get('attempt_id'),'outcome':'unavailable','recorder_id':ctx['agent_id'],'devlog_context':str(context),'observed_work_context':state.get('observed_work_context'),'capture_tool_identity':{'helper_sha256':digest(Path(__file__)), 'browser_helper_sha256':digest(CAPTURE)}}
 temporary=PROJECT/'artifacts/tmp/journey';temporary.mkdir(parents=True,exist_ok=True)
 try:
  if registration_error:raise RuntimeError(registration_error)
  if config.get('public_safe') is not True:raise ValueError('public_safe_scope_required')
  r['source_before']=source_identity(config)
  with preview(config) as url,tempfile.TemporaryDirectory(prefix=(stage_id or uuid.uuid4().hex)+'_',dir=temporary) as staging:
   png=Path(staging)/'capture.png';options=Path(staging)/'options.json'
   write(options,{'url':url,'output':str(png),'profile':str(Path(staging)/'owned-browser-profile'),'playwright_module':config['playwright_module'],'browser_executable':config['browser_executable']})
   try:
    result=subprocess.run([config['node_executable'],str(CAPTURE),str(options)],capture_output=True,text=True,timeout=35)
   finally:
    cleanup_owned_profile(Path(staging)/'owned-browser-profile')
   if result.returncode:raise RuntimeError('isolated_browser_capture_failed')
   observation=json.loads(result.stdout.strip().splitlines()[-1]);data=png.read_bytes()
   if data[:8]!=b'\x89PNG\r\n\x1a\n' or len(data)>MAX_SHOT:raise RuntimeError('invalid_or_oversized_capture')
   dest=JOURNEY/'shots'/f'{rid}.png';dest.parent.mkdir(exist_ok=True);shutil.copyfile(png,dest)
   r.update({'outcome':'captured','image':str(dest.relative_to(PROJECT)),'image_sha256':digest(dest),'image_bytes':dest.stat().st_size,'browser':observation})
   first=OUTPUTS/'dungeonflux-journey-first.png'
   if not first.exists():first.parent.mkdir(exist_ok=True);shutil.copyfile(dest,first)
  r['source_after']=source_identity(config);r['source_changed_during_capture']=r['source_before']!=r['source_after']
 except Exception as error:
  r['reason']=type(error).__name__+': '+str(error)[:160]
 write(recordpath,r)
 r['devlog_status']=devlog(r,context);write(recordpath,r);make_index()
 return {'result':r['outcome'],'id':rid,'image':r.get('image'),'devlog_status':r['devlog_status'],'action_required':r['outcome']!='captured' or r['devlog_status']=='pending_ingestion'}

def main():
 p=argparse.ArgumentParser(description=__doc__);s=p.add_subparsers(dest='command',required=True)
 reg=s.add_parser('register');reg.add_argument('--kind',choices=['static-site','owned-preview'],required=True);reg.add_argument('--label',required=True);reg.add_argument('--public-safe',action='store_true');reg.add_argument('--url');reg.add_argument('--ownership-proof',type=Path);reg.add_argument('--source-path',action='append',default=[])
 start=s.add_parser('start');start.add_argument('--context',type=Path,required=True);start.add_argument('--lease-seconds',type=int,default=600)
 s.add_parser('stop');cap_parser=s.add_parser('capture');cap_parser.add_argument('--stage-id');s.add_parser('status');s.add_parser('index');s.add_parser('run-loop');s.add_parser('recover')
 args=p.parse_args();JOURNEY.mkdir(parents=True,exist_ok=True)
 if args.command=='run-loop':
  return run_loop()
 with (JOURNEY/'journal.lock').open('a') as lock:
  fcntl.flock(lock,fcntl.LOCK_EX)
  if args.command=='register':
   if not args.public_safe:raise SystemExit('Explicit public-safe project preview scope required')
   if args.kind=='owned-preview':
    admitted_url(args.url or '')
    if not args.ownership_proof or not args.source_path:raise SystemExit('Owned preview proof and governing source paths required')
    for item in args.source_path:
     path=(PROJECT/item).resolve()
     if not path.is_relative_to(PROJECT) or not path.exists():raise SystemExit('Source path must exist inside project')
   write(JOURNEY/'target.json',{'kind':args.kind,'label':args.label,'url':args.url,'public_safe':True,'ownership_proof':str(args.ownership_proof.resolve()) if args.ownership_proof else None,'source_paths':args.source_path,'disk_cap_bytes':CAP,'node_executable':str(DEFAULT_NODE),'playwright_module':str(DEFAULT_PLAYWRIGHT),'browser_executable':str(DEFAULT_BROWSER)})
   result={'result':'registered','kind':args.kind}
  elif args.command=='start':
   if not 60<=args.lease_seconds<=1800:raise SystemExit('Active lease must be60–1800seconds; only coordinator work renews it')
   if not (JOURNEY/'target.json').exists():raise SystemExit('Register public-safe target first')
   context=args.context.resolve();incoming=load(context)
   old=load(JOURNEY/'active.json') if (JOURNEY/'active.json').exists() else {}
   observed_context={k:incoming.get(k) for k in ('agent_id','task_id','attempt_id')}
   sid=old.get('session_id') if old.get('active') and old.get('expires_at_epoch',0)>time.time() and old.get('observed_work_context')==observed_context else uuid.uuid4().hex[:16]
   recorder_context=JOURNEY/'recorder-contexts'/f'{sid}.json';incoming['agent_id']='screenshot-recorder:'+sid;incoming['role']='worker';incoming['attempt_id']=None
   if not recorder_context.exists():write(recorder_context,incoming)
   context=recorder_context
   write(JOURNEY/'active.json',{'active':True,'session_id':sid,'context':str(context),'observed_work_context':observed_context,'started_at':utc(),'expires_at_epoch':time.time()+args.lease_seconds,'stop_reason':None})
   result={'result':'active','lease_seconds':args.lease_seconds,'session_id':sid}
   result.update(ensure_loop())
  elif args.command=='stop':
   state=load(JOURNEY/'active.json') if (JOURNEY/'active.json').exists() else {};state['active']=False;state['stop_reason']='coordinator_work_ended';write(JOURNEY/'active.json',state);result={'result':'stopped'}
  elif args.command=='capture':result=capture(stage_id=args.stage_id)
  elif args.command=='index':result={'records':len(make_index())}
  elif args.command=='recover':result=ensure_loop()
  else:
   state=load(JOURNEY/'active.json') if (JOURNEY/'active.json').exists() else {};result={'active':state.get('active',False) and state.get('expires_at_epoch',0)>time.time(),'stop_reason':state.get('stop_reason'),'loop_alive':loop_alive(),'loop_health':load(JOURNEY/'loop-health.json') if (JOURNEY/'loop-health.json').exists() else None,'retained_bytes':journal_size(),'target':load(JOURNEY/'target.json') if (JOURNEY/'target.json').exists() else None}
  print(json.dumps(result))
def loop_alive():
 previous=load(JOURNEY/'loop.json') if (JOURNEY/'loop.json').exists() else {}
 pid=previous.get('pid')
 if not pid:return False
 result=subprocess.run(['ps','-p',str(pid),'-o','args='],capture_output=True,text=True,timeout=3)
 return result.returncode==0 and str(Path(__file__).resolve())+' run-loop' in result.stdout

def ensure_loop():
 state=load(JOURNEY/'active.json') if (JOURNEY/'active.json').exists() else {}
 if not state.get('active') or state.get('expires_at_epoch',0)<=time.time():return {'result':'inactive_no_restart','stop_reason':state.get('stop_reason')}
 if loop_alive():return {'result':'recorder_healthy'}
 logdir=PROJECT/'artifacts/tmp/journey';logdir.mkdir(parents=True,exist_ok=True)
 with (logdir/'recorder.log').open('a') as logfile:
  process=subprocess.Popen(['python3',str(Path(__file__).resolve()),'run-loop'],stdout=logfile,stderr=logfile,start_new_session=True)
 write(JOURNEY/'loop.json',{'pid':process.pid,'owner':'screenshot-recorder:'+state['session_id'],'started_at':utc()})
 return {'result':'recorder_started','recorder_pid':process.pid}

def run_loop():
 last_bucket=None
 while True:
  state=load(JOURNEY/'active.json') if (JOURNEY/'active.json').exists() else {}
  if not state.get('active') or state.get('expires_at_epoch',0)<=time.time():break
  bucket=int(time.time()//60)
  if bucket!=last_bucket:
   stage_id=uuid.uuid4().hex
   process=subprocess.Popen(['python3',str(Path(__file__).resolve()),'capture','--stage-id',stage_id],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True)
   try:output,error=process.communicate(timeout=90)
   except subprocess.TimeoutExpired:
    os.killpg(process.pid,signal.SIGTERM)
    try:output,error=process.communicate(timeout=5)
    except subprocess.TimeoutExpired:os.killpg(process.pid,signal.SIGKILL);output,error=process.communicate(timeout=5)
    for folder in (PROJECT/'artifacts/tmp/journey').glob(stage_id+'_*'):cleanup_owned_profile(folder/'owned-browser-profile')
    with (JOURNEY/'journal.lock').open('a') as lock:
     fcntl.flock(lock,fcntl.LOCK_EX)
     rid=state['session_id']+'-'+str(bucket);recordpath=JOURNEY/'records'/f'{rid}.json'
     if not recordpath.exists():
      context=Path(state['context']);ctx=load(context);config=load(JOURNEY/'target.json')
      record={'id':rid,'observed_at':utc(),'minute_bucket':bucket,'target':config['label'],'preview_kind':config['kind'],'task_id':ctx.get('task_id'),'attempt_id':ctx.get('attempt_id'),'recorder_id':ctx['agent_id'],'devlog_context':str(context),'outcome':'unavailable','reason':'owned_capture_owner_timeout'}
      write(recordpath,record);record['devlog_status']=devlog(record,context);write(recordpath,record);make_index()
    output=json.dumps({'result':'owned_capture_timeout','action_required':True})
   last_bucket=bucket
   write(JOURNEY/'loop-health.json',{'pid':os.getpid(),'observed_at':utc(),'minute_bucket':bucket,'result_code':process.returncode,'result':output.strip()[:600]})
  time.sleep(1)
 print(json.dumps({'result':'recorder_stopped','reason':'inactive_or_expired','observed_at':utc()}))

if __name__=='__main__':main()
