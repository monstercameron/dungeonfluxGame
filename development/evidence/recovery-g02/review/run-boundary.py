import json,hashlib,subprocess,urllib.request,datetime,plistlib,sqlite3
from pathlib import Path
R=Path('/Users/earlcameron/Desktop/dungeonflux');B=R/'development/evidence/recovery-g02';W=B/'worker';O=B/'review';I=R/'development/evidence/qualify-g02/integration';report=json.loads((W/'diagnostic-report.json').read_text());build=json.loads((I/'build-identity.json').read_text());brief=json.loads((B/'brief.json').read_text())
def command(args):
 p=subprocess.run(args,cwd=R,capture_output=True,text=True,timeout=15);return {'args':args,'exit_code':p.returncode,'stdout':p.stdout,'stderr':p.stderr}
artifacts=[]
for item in build['artifacts']:
 sha=hashlib.sha256(Path(item['path']).read_bytes()).hexdigest();artifacts.append({**item,'current_sha256':sha,'matches':sha==item['sha256']})
prior=[]
for item in json.loads((W/'prior-evidence-integrity.json').read_text())['files']:
 sha=hashlib.sha256((R/item['path']).read_bytes()).hexdigest();prior.append({'path':item['path'],'current_sha256':sha,'matches_worker_expected':sha==item['expected_sha256']})
http=[]
for endpoint in ['/','/fixture-health','/fixture-resources','/pkg/df_tools.js','/pkg/df_tools_bg.wasm']:
 with urllib.request.urlopen('http://127.0.0.1:43182'+endpoint,timeout=4) as response:
  data=response.read(8388609);assert len(data)<=8388608
  row={'url':response.url,'status':response.status,'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest(),'headers':dict(response.headers)}
  if endpoint in ['/','/fixture-health','/fixture-resources']:row['body']=data.decode()
  http.append(row)
commands={name:command(args) for name,args in [('head',['git','rev-parse','HEAD']),('app_diff',['git','diff','ba0d3860fb27124be352b48766e99a955f746672','--','crates','Cargo.toml','Cargo.lock','.cargo','rust-toolchain.toml','rustfmt.toml','development/build-fixture.sh']),('processes',['ps','-p','1222,18905,18907,25530,27721,80873','-o','pid,ppid,lstart,command']),('listener',['lsof','-nP','-a','-p','25530','-iTCP:43182','-sTCP:LISTEN']),('browser_loaded',['lsof','-a','-p','1222','-d','txt']),('host',['sw_vers']),('arch',['uname','-m'])]}
# Retain only relevant Chrome framework path from text mappings, not unrelated mappings.
commands['browser_loaded']['stdout']='\n'.join(line for line in commands['browser_loaded']['stdout'].splitlines() if 'Google Chrome Framework.framework/Versions/' in line)
installed=plistlib.loads(Path('/Applications/Google Chrome Beta.app/Contents/Info.plist').read_bytes()).get('CFBundleShortVersionString')
c=sqlite3.connect((R/'development/workflow.sqlite3').as_uri()+'?mode=ro',uri=True);c.row_factory=sqlite3.Row
result={'at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'report_sha256':hashlib.sha256((W/'diagnostic-report.json').read_bytes()).hexdigest(),'build':artifacts,'prior_integrity':prior,'http':http,'commands':commands,'installed_browser_version':installed,'worker_devlog':[dict(x) for x in c.execute("SELECT id,kind,summary,outcome FROM devlog WHERE attempt_id='DIAGNOSE-G02-RECOVERY-001-a1'")],'governing_sources':[{'path':p,'matches_brief':hashlib.sha256((R/p).read_bytes()).hexdigest()==h} for p,h in brief['source_hashes'].items()],'limits':'HTTP/native state and preserved prior screenshots are not current browser/DevTools recovery proof.'}
(O/'independent-boundary.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({'report_sha256':result['report_sha256'],'artifacts_match':all(x['matches'] for x in artifacts),'prior_files':len(prior),'prior_match':all(x['matches_worker_expected'] for x in prior),'http':[{k:v for k,v in x.items() if k not in ['body','headers']} for x in http],'health':http[1]['body'],'head':commands['head']['stdout'],'appdiff':commands['app_diff']['stdout'],'framework':commands['browser_loaded']['stdout'],'installed':installed,'sourcehashes':result['governing_sources']},indent=2))
