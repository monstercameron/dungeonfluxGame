import json,subprocess,time,hashlib,shutil,os,ast,datetime,urllib.request
from pathlib import Path
R=Path('/Users/earlcameron/Desktop/dungeonflux');W=R/'artifacts/worktrees/pin-srd-g07';O=R/'development/evidence/srd-g07/review-evidence';T=R/'artifacts/tmp/PIN-G07-SRD-001-a1-review';PY='/Users/earlcameron/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/bin/python3';S=W/'development/check-rule-source.py';M=W/'development/rules-sources/manifest.json';PDF=M.parent/'srd-5.2.1-en.pdf';manifest=json.loads(M.read_text());records=[]
def run(label,args,limit=60):
 started=time.monotonic()
 try:
  x=subprocess.run(list(map(str,args)),capture_output=True,text=True,timeout=limit);r={'label':label,'argv':list(map(str,args)),'exit':x.returncode,'stdout':x.stdout,'stderr':x.stderr,'seconds':round(time.monotonic()-started,3)}
 except subprocess.TimeoutExpired as ex:r={'label':label,'argv':list(map(str,args)),'external_timeout':limit,'seconds':round(time.monotonic()-started,3)}
 records.append(r);(O/'verification-results.json').write_text(json.dumps(records,indent=2)+'\n');print(label,r.get('exit','TIMEOUT'),flush=True);return r
run('actual-source',[PY,S]);run('help',[PY,S,'--help'])
cases={
 'corrupted-bytes':lambda d:None,
 'malformed-language':lambda d:d.pop('language'),
 'mixed-edition':lambda d:d.update(version='5.1'),
 'omitted-required-book-record':lambda d:d['scope']['required_full_book_sources'].pop(),
 'required-books-recorded-but-unavailable':lambda d:None,
 'empty-attribution':lambda d:d['license'].update(attribution=''),
 'short-attribution':lambda d:d['license'].update(attribution='This work'),
 'malformed-attribution-type':lambda d:d['license'].update(attribution=[]),
 'false-full-coverage':lambda d:d['scope'].update(full_2024_rules_coverage=True),
 'false-production-rights':lambda d:d['scope'].update(production_rights_grant={'reviewed':True}),
 'wrong-section-page':lambda d:d['verified_locators'][4].update(pdf_page=6),
}
for name,mutate in cases.items():
 folder=T/name;folder.mkdir(exist_ok=True);d=json.loads(M.read_text());mutate(d);f=folder/'srd-5.2.1-en.pdf';shutil.copyfile(PDF,f)
 if name=='corrupted-bytes':
  raw=bytearray(f.read_bytes());raw[500]^=1;f.write_bytes(raw)
 (folder/'manifest.json').write_text(json.dumps(d,indent=2)+'\n');run(name,[PY,S,'--manifest',folder/'manifest.json'])
# Bound claims: a non-regular manifest must fail rather than indefinitely wait on a FIFO.
fifo=T/'manifest-fifo';os.mkfifo(fifo);run('fifo-manifest-must-not-block',[PY,S,'--manifest',fifo],2)
ast.parse(S.read_text());run('diff-check',['git','-C',W,'diff','f9343cb5ec17b33c76a03b20f6afec9168f9fb7a..HEAD','--check']);run('source-status',['git','-C',W,'status','--porcelain'])
identity={'timestamp_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'commit':subprocess.check_output(['git','-C',W,'rev-parse','HEAD'],text=True).strip(),'changed_paths':subprocess.check_output(['git','-C',W,'diff','--name-only','f9343cb5ec17b33c76a03b20f6afec9168f9fb7a..HEAD'],text=True).splitlines(),'files':{str(f.relative_to(W)):{'sha256':hashlib.sha256(f.read_bytes()).hexdigest(),'bytes':f.stat().st_size} for f in (S,M,PDF)},'syntax_ast':'pass','manifest_required_full_book_files':'No full-book bytes or pinned revisions provided; copied fixture contains only SRD PDF and manifest.'};(O/'identity.json').write_text(json.dumps(identity,indent=2)+'\n')
