from pathlib import Path
import json,hashlib,subprocess,time,datetime
r=Path('/Users/earlcameron/Desktop/dungeonflux');e=r/'development/evidence/srd-g07/a2/review-evidence';t=r/'artifacts/tmp/PIN-G07-SRD-001-a2-review';approved=json.loads((e/'identity.json').read_text());head='dd63d194a8e87ce575e861199abc28861ac2c28d'
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=r,text=True).strip()==head
identity={}
for path,prior in approved['files'].items():
 actual=(r/path).read_bytes();committed=subprocess.check_output(['git','show',head+':'+path],cwd=r)
 digest=hashlib.sha256(actual).hexdigest();assert digest==prior['sha256'];assert actual==committed
 identity[path]={'sha256':digest,'bytes':len(actual),'matches_approved_candidate':True,'matches_integrated_git_blob':True}
python='/Users/earlcameron/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/bin/python3';script=str(r/'development/check-rule-source.py');results=[]
def run(label,args,expected,cwd=r):
 start=time.monotonic();p=subprocess.run(args,cwd=cwd,capture_output=True,text=True,timeout=5)
 results.append({'label':label,'command':args,'cwd':str(cwd),'expected_exit':expected,'exit':p.returncode,'stdout':p.stdout,'stderr':p.stderr,'seconds':round(time.monotonic()-start,4),'external_bound_seconds':5});assert p.returncode==expected
run('root-default-partial',[python,script],0)
run('root-full-book-refusal',[python,script,'--require-full-book-sources'],1)
run('foreign-cwd-default-manifest',[python,script],0,t)
run('root-script-empty-attribution',[python,script,'--manifest',str(t/'empty-attribution/manifest.json')],1)
run('root-script-fifo-manifest',[python,script,'--manifest',str(t/'manifest-fifo')],1)
run('integrated-diff-check',['git','diff','ba0d3860fb27124be352b48766e99a955f746672..HEAD','--check'],0)
run('relevant-source-clean',['git','status','--porcelain','--','development/check-rule-source.py','development/rules-sources'],0)
assert results[-1]['stdout']==''
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=r,text=True).strip()==head
out={'timestamp_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'tested_revision':head,'approved_candidate':approved['tested_revision'],'source_equivalence':identity,'root_overall_status':'Runtime/evidence changes exist outside these three source paths; reviewed source paths are clean.','checks':results,'prior_19_candidate_checks_applicable':True,'prior_pdf_visual_and_publisher_proof_applicable':True}
(e/'integration-results.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps({'tested_revision':head,'checks':[(a['label'],a['exit']) for a in results],'source_hashes_match':True},indent=2))
