from pathlib import Path
import subprocess,json,hashlib,os,copy,time,ast,platform,sys,datetime
r=Path('/Users/earlcameron/Desktop/dungeonflux');w=r/'artifacts/worktrees/pin-srd-g07';e=r/'development/evidence/srd-g07/a2/review-evidence';t=r/'artifacts/tmp/PIN-G07-SRD-001-a2-review'
python='/Users/earlcameron/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/bin/python3'
script=w/'development/check-rule-source.py';source=w/'development/rules-sources';manifest=json.loads((source/'manifest.json').read_text());pdf=(source/'srd-5.2.1-en.pdf').read_bytes()
expected=sys.argv[1];head=subprocess.check_output(['git','-C',str(w),'rev-parse','HEAD'],text=True).strip();assert head==expected,(head,expected)
results=[]
def run(label,args,expect):
 start=time.monotonic();record={'label':label,'command':args,'expected_exit':expect,'external_bound_seconds':5}
 try:
  p=subprocess.run(args,capture_output=True,text=True,timeout=5);record.update(exit=p.returncode,stdout=p.stdout,stderr=p.stderr,pass_expected=p.returncode==expect)
 except subprocess.TimeoutExpired as ex:record.update(external_timeout=True,pass_expected=False)
 record['seconds']=round(time.monotonic()-start,4);results.append(record)
def fixture(label,mutate=None,contents=None,corrupt=False,extra=None):
 d=t/label;d.mkdir(exist_ok=True);m=copy.deepcopy(manifest)
 if mutate:mutate(m)
 (d/'manifest.json').write_text(contents if contents is not None else json.dumps(m));(d/'srd-5.2.1-en.pdf').write_bytes((b'X'+pdf[1:]) if corrupt else pdf)
 run(label,[python,str(script),'--manifest',str(d/'manifest.json')]+(extra or []),1)
run('actual-partial-source',[python,str(script)],0)
run('actual-missing-required-full-book-bytes',[python,str(script),'--require-full-book-sources'],1)
fixture('empty-attribution',lambda m:m['license'].update(attribution=''))
fixture('short-attribution',lambda m:m['license'].update(attribution='This work'))
fixture('malformed-attribution-type',lambda m:m['license'].update(attribution=[]))
fixture('corrupted-bytes',corrupt=True)
fixture('missing-language',lambda m:m.pop('language'))
fixture('mixed-edition',lambda m:m.update(version='5.1'))
fixture('omitted-required-book-record',lambda m:m['scope']['required_full_book_sources'].pop())
fixture('malformed-json',contents='{')
fixture('oversized-manifest',contents=' '*(1024*1024+1))
fixture('false-full-coverage',lambda m:m['scope'].update(full_2024_rules_coverage=True))
fixture('fabricated-ruleset',lambda m:m['scope'].update(production_ruleset_id='invented'))
def missing_files(m):
 for b in m['scope']['required_full_book_sources']:b.update(status='acquired',revision='fixture-only-absent-file',file='does-not-exist.pdf',size_bytes=1,sha256='0'*64)
fixture('claimed-acquired-but-no-book-bytes',missing_files,extra=['--require-full-book-sources'])
fifo=t/'manifest-fifo'
if not fifo.exists():os.mkfifo(fifo)
run('fifo-manifest',[python,str(script),'--manifest',str(fifo)],1)
run('directory-manifest',[python,str(script),'--manifest',str(t)],1)
run('help',[python,str(script),'--help'],0)
run('diff-check',['git','-C',str(w),'diff','0df797256ae35350521fa9dc0d4ece8d5b85b4a0..HEAD','--check'],0)
run('clean-status',['git','-C',str(w),'status','--porcelain'],0)
assert subprocess.check_output(['git','-C',str(w),'rev-parse','HEAD'],text=True).strip()==head
ast.parse(script.read_text())
files={str(p.relative_to(w)):{'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'bytes':p.stat().st_size} for p in [script,source/'manifest.json',source/'srd-5.2.1-en.pdf']}
prior=json.loads((r/'development/evidence/srd-g07/review-evidence/identity.json').read_text())
for p in ['development/rules-sources/manifest.json','development/rules-sources/srd-5.2.1-en.pdf']:assert files[p]==prior['files'][p]
ident={'tested_revision':head,'timestamp_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'files':files,'asset_hashes_match_prior_independent_review':True,'ast_parse':'pass','python_version':sys.version,'host':platform.platform(),'changed_paths':subprocess.check_output(['git','-C',str(w),'diff','--name-only','0df797256ae35350521fa9dc0d4ece8d5b85b4a0..HEAD'],text=True).splitlines(),'all_expected_exits':all(x['pass_expected'] for x in results)}
(e/'verification-results.json').write_text(json.dumps(results,indent=2)+'\n');(e/'identity.json').write_text(json.dumps(ident,indent=2)+'\n')
print(json.dumps(ident,indent=2));print([(x['label'],x.get('exit'),x['seconds']) for x in results])
