from pathlib import Path
import contextlib,hashlib,http.server,importlib.util,json,sqlite3,subprocess,sys,tempfile,threading,time
sys.dont_write_bytecode=True
ROOT=Path(__file__).resolve().parents[2];PROJECT=ROOT/'dungeonflux'
spec=importlib.util.spec_from_file_location('journey',PROJECT/'development/screenshot_journey.py');j=importlib.util.module_from_spec(spec);spec.loader.exec_module(j)
results=[]
def assert_equal(actual,expected):assert actual==expected,(actual,expected)
def check(name,fn):
 fn();results.append({'case':name,'result':'pass'})
def denied(url):
 try:j.admitted_url(url)
 except ValueError:return
 raise AssertionError('unsafe URL admitted')
for name,url in [('external_url','https://example.com/'),('credentials','http://u:p@127.0.0.1:8000/'),('query','http://127.0.0.1:8000/?token=x'),('fragment','http://127.0.0.1:8000/#private')]:check(name,lambda url=url:denied(url))
with tempfile.TemporaryDirectory(dir=ROOT/'work/screenshot-journey') as tmp:
 p=Path(tmp);fake=p/'dungeonflux';dev=fake/'development';dev.mkdir(parents=True)
 (dev/'devlog.py').write_bytes((PROJECT/'development/devlog.py').read_bytes())
 conn=sqlite3.connect(dev/'workflow.sqlite3');conn.executescript((PROJECT/'development/schema.sql').read_text());conn.close()
 original=(j.PROJECT,j.JOURNEY,j.OUTPUTS);j.PROJECT=fake;j.JOURNEY=dev/'evidence/journey';j.OUTPUTS=p/'outputs';j.JOURNEY.mkdir(parents=True)
 ctx=p/'context.json';j.write(ctx,{'agent_id':'screenshot-recorder:fixture','role':'worker','task_id':None,'attempt_id':None})
 def state(active=True,expiry=None):j.write(j.JOURNEY/'active.json',{'active':active,'expires_at_epoch':expiry or time.time()+120,'session_id':'fixture','context':str(ctx)})
 check('inactive_has_no_capture',lambda: (state(False),assert_equal(j.capture()['result'],'idle_or_expired')))
 check('expired_has_no_capture',lambda: (state(True,time.time()-1),assert_equal(j.capture()['result'],'idle_or_expired')))
 check('inactive_recovery_does_not_restart',lambda: (state(False),assert_equal(j.ensure_loop()['result'],'inactive_no_restart')))
 proof=p/'proof.json';j.write(proof,{'project_root':str(fake),'url':'http://127.0.0.1:8000/','public_safe':True,'owner_pid':1})
 def private_denied():
  try:
   with j.preview({'kind':'owned-preview','ownership_proof':str(proof),'url':'http://127.0.0.1:8000/'}):pass
  except ValueError:return
  raise AssertionError('unverified passive/private preview admitted')
 check('passive_synthetic_proof_required',private_denied)
 def cleanup_denied():
  try:j.cleanup_owned_profile(p/'unowned-user-profile')
  except ValueError:return
  raise AssertionError('unowned profile cleanup admitted')
 check('unowned_profile_cleanup_denied',cleanup_denied)
 state();j.write(j.JOURNEY/'target.json',{'kind':'static-site','label':'fixture','disk_cap_bytes':1})
 existing=j.JOURNEY/'retained.txt';existing.write_text('keep original evidence')
 def cap():
  assert_equal(j.capture()['result'],'disk_cap_stopped')
  assert_equal(existing.read_text(),'keep original evidence')
  assert_equal(j.load(j.JOURNEY/'active.json')['active'],False)
  c=sqlite3.connect(dev/'workflow.sqlite3');assert_equal(c.execute('select count(*) from devlog').fetchone()[0],1);assert_equal(c.execute('select count(*) from tasks').fetchone()[0],0);c.close()
 check('cap_stops_preserves_history_appends_only_devlog',cap)
 state()
 check('same_minute_capture_coalesces',lambda:assert_equal(j.capture()['result'],'coalesced'))
 record=j.JOURNEY/'records'/f'fixture-{int(time.time()//60)}.json';r=j.load(record);r['devlog_status']='pending_ingestion';j.write(record,r)
 def retry():
  assert_equal(j.capture()['result'],'coalesced')
  c=sqlite3.connect(dev/'workflow.sqlite3');assert_equal(c.execute('select count(*) from devlog').fetchone()[0],1);c.close()
 check('devlog_retry_idempotent_no_second_capture',retry)
 def missing_target():
  state(True,time.time()+600);(j.JOURNEY/'target.json').unlink()
  result=j.capture(now=time.time()+60);assert_equal(result['result'],'unavailable')
  assert_equal(result.get('image'),None)
 check('missing_target_records_failure_without_image',missing_target)
 def unsafe_scope():
  state(True,time.time()+600);j.write(j.JOURNEY/'target.json',{'kind':'static-site','label':'unsafefixture','disk_cap_bytes':j.CAP,'public_safe':False})
  result=j.capture(now=time.time()+120);assert_equal(result['result'],'unavailable');assert_equal(result.get('image'),None)
 check('unapproved_public_scope_records_failure_without_image',unsafe_scope)
 def old_pending():
  state(True,time.time()+600);current=j.load(j.JOURNEY/'records'/f'fixture-{int(time.time()//60)}.json')
  old=dict(current);old['id']='fixture-'+str(int(time.time()//60)-1);old['devlog_status']='pending_ingestion';old['devlog_context']=str(ctx)
  path=j.JOURNEY/'records'/f"{old['id']}.json";j.write(path,old);j.capture()
  assert_equal(j.load(path)['devlog_status'],'appended_or_existing')
 check('older_pending_ingestion_drains_without_recapture',old_pending)
 docs=fake/'docs';docs.mkdir();secret=p/'outside-secret.txt';secret.write_text('must not serve outside data');(docs/'outside.txt').symlink_to(secret)
 outside=p/'outside-directory';outside.mkdir();(outside/'secret.txt').write_text('must not serve directory');(docs/'linked').symlink_to(outside,target_is_directory=True)
 def no_symlink(parts):
  try:
   with j.safe_doc_open(parts):pass
  except OSError:return
  raise AssertionError('symlink escaped admitted publicroot')
 check('file_symlink_cannot_open_private_target',lambda:no_symlink(['outside.txt']))
 check('directory_symlink_cannot_open_private_target',lambda:no_symlink(['linked','secret.txt']))
 def immutable_recorder_context():
  original_ensure=j.ensure_loop;original_args=sys.argv[:];j.ensure_loop=lambda:{'result':'fixture_no_process'}
  try:
   incoming=p/'assigned-context.json';j.write(incoming,{'agent_id':'assigned-worker','role':'worker','task_id':'T-1','attempt_id':'A-1'})
   sys.argv=[str(PROJECT/'development/screenshot_journey.py'),'start','--context',str(incoming),'--lease-seconds','600'];j.main()
   first=j.load(j.JOURNEY/'active.json');firstpath=Path(first['context']);before=firstpath.read_bytes()
   assert_equal(j.load(firstpath)['attempt_id'],None);assert_equal(first['observed_work_context']['attempt_id'],'A-1')
   j.write(incoming,{'agent_id':'another-worker','role':'worker','task_id':'T-2','attempt_id':'A-2'});j.main()
   second=j.load(j.JOURNEY/'active.json');assert first['session_id']!=second['session_id'];assert_equal(firstpath.read_bytes(),before)
  finally:j.ensure_loop=original_ensure;sys.argv=original_args
 check('assigned_attempt_observed_not_impersonated_and_context_switch_immutable',immutable_recorder_context)
 j.PROJECT,j.JOURNEY,j.OUTPUTS=original

# Actual isolated browser verifies same-origin POST and cross-origin resource attempts are blocked.
class Receiver(http.server.BaseHTTPRequestHandler):
 requests=0
 def do_GET(self):type(self).requests+=1;self.send_response(200);self.end_headers();self.wfile.write(b'blocked target')
 def log_message(self,*args):pass
class Source(http.server.BaseHTTPRequestHandler):
 posts=0
 def do_POST(self):type(self).posts+=1;self.send_response(200);self.end_headers()
 def do_GET(self):
  self.send_response(200);self.send_header('Content-Type','text/html');self.end_headers()
  self.wfile.write((f'<h1>Isolated screenshot privacy fixture</h1><img src="http://127.0.0.1:{receiver.server_port}/outside"><script>fetch("/write",{{method:"POST"}}).catch(()=>{{}});new WebSocket("ws://127.0.0.1:{receiver.server_port}/socket");</script>').encode())
 def log_message(self,*args):pass
receiver=http.server.ThreadingHTTPServer(('127.0.0.1',0),Receiver);source=http.server.ThreadingHTTPServer(('127.0.0.1',0),Source)
threads=[]
for server in (receiver,source):
 thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start();threads.append(thread)
try:
 staging=PROJECT/'artifacts/tmp/journey/privacy-fixture';staging.mkdir(parents=True,exist_ok=True)
 config=j.load(PROJECT/'development/evidence/journey/target.json');options=staging/'options.json'
 j.write(options,{'url':f'http://127.0.0.1:{source.server_port}/','output':str(staging/'fixture.png'),'profile':str(staging/'owned-browser-profile'),'playwright_module':config['playwright_module'],'browser_executable':config['browser_executable']})
 r=subprocess.run([config['node_executable'],str(PROJECT/'development/capture_preview.mjs'),str(options)],capture_output=True,text=True,timeout=35)
 assert r.returncode==0,r.stderr[-600:]
 actual=json.loads(r.stdout.strip().splitlines()[-1]);assert Receiver.requests==0 and Source.posts==0
 assert actual['network_blocks']['external_origin']>=1 and actual['network_blocks']['write_method']>=1 and actual['network_blocks']['websocket']>=1
 results.append({'case':'actual_headless_external_POST_WebSocket_blocking','result':'pass','observed_network_blocks':actual['network_blocks'],'receiver_requests':Receiver.requests,'same_origin_posts':Source.posts})
finally:
 j.cleanup_owned_profile(staging/'owned-browser-profile')
 for server in (receiver,source):server.shutdown();server.server_close()
 for thread in threads:thread.join(timeout=2)
report={'schema_sha256':hashlib.sha256((PROJECT/'development/schema.sql').read_bytes()).hexdigest(),'helper_sha256':hashlib.sha256((PROJECT/'development/screenshot_journey.py').read_bytes()).hexdigest(),'browser_helper_sha256':hashlib.sha256((PROJECT/'development/capture_preview.mjs').read_bytes()).hexdigest(),'cases':results,'passed':len(results),'scope':'Isolated guard/devlog fixture and actual isolated browser network boundaries; no fake live journal captures or queue mutations.'}
(ROOT/'work/screenshot-journey/verification.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report))
