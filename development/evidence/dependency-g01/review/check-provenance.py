import hashlib,json,stat,tomllib,subprocess,datetime
from pathlib import Path
M=Path('/Users/earlcameron/Desktop/dungeonflux');R=M/'artifacts/worktrees/audit-g01';O=M/'development/evidence/dependency-g01/review';W=M/'development/evidence/dependency-g01/worker'
report=json.loads((R/'development/dependency-audit/report.json').read_text());pkgs=json.loads((R/'development/dependency-audit/packages.json').read_text())['packages'];lock=tomllib.loads((R/'Cargo.lock').read_text());errors=[];checks=[]
allowed=[R.resolve(),(M/'artifacts/cache/cargo/registry/src').resolve()]
def safe_hash(path):
 p=Path(path);resolved=p.resolve()
 assert any(resolved.is_relative_to(a) for a in allowed),str(p)
 assert stat.S_ISREG(p.lstat().st_mode),str(p)
 assert p.stat().st_size<=1048576,str(p)
 return hashlib.sha256(p.read_bytes()).hexdigest()
metas={target:json.loads((O/('metadata-native.json' if target=='aarch64-apple-darwin' else 'metadata-wasm.json')).read_text()) for target in report['targets']}
byid={target:{p['id']:p for p in meta['packages']} for target,meta in metas.items()}
nodes={target:{n['id']:n for n in meta['resolve']['nodes']} for target,meta in metas.items()}
for p in pkgs:
 prov=p['provenance'];good=safe_hash(prov['manifest_path'])==prov['manifest_sha256'];checks.append({'id':p['id'],'manifest_matches':good,'notices':len(prov['license_files'])})
 if not good:errors.append([p['id'],'manifest'])
 for f in prov['license_files']:
  if safe_hash(f['path'])!=f['sha256']:errors.append([p['id'],'notice',f['path']])
 for target in p['targets']:
  actual=byid[target][p['id']]
  for key in ['name','version','license','repository','source']:
   if p[key]!=actual[key]:errors.append([p['id'],target,key])
  if p['license_file_declared']!=actual['license_file']:errors.append([p['id'],target,'license_file'])
  if p['features_by_target'][target]!=nodes[target][p['id']]['features']:errors.append([p['id'],target,'features'])
 matching=[n for n in lock['package'] if n['name']==p['name'] and n['version']==p['version'] and n.get('source')==p['source']]
 if len(matching)!=1 or matching[0].get('checksum')!=prov['cargo_lock_checksum']:errors.append([p['id'],'lock_checksum'])
commands=[]
for name,expected in report['command_evidence'].items():
 path=W/'commands'/name;actual=hashlib.sha256(path.read_bytes()).hexdigest();commands.append({'file':name,'matches':actual==expected['sha256']})
 if actual!=expected['sha256']:errors.append(['command',name])
summary={'at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'handoff_sha256':hashlib.sha256((W/'handoff.json').read_bytes()).hexdigest(),'report_sha256':hashlib.sha256((R/'development/dependency-audit/report.json').read_bytes()).hexdigest(),'packages':len(pkgs),'package_checks':checks,'notice_files_hashed':sum(len(p['provenance']['license_files']) for p in pkgs),'packages_no_notices':sum(not p['provenance']['license_files'] for p in pkgs),'packages_no_lock_checksum':sum(not p['provenance']['cargo_lock_checksum'] for p in pkgs),'command_evidence':commands,'errors':errors,'h2':next(p for p in pkgs if p['name']=='h2'),'graph_scope':'Metadata ids opaque; per-target features match actual resolve nodes. Cargo tree all/build/dev outputs distinguish proc-macro/host-build/development; not compiled-runtime proof.','config':(R/'.cargo/config.toml').read_text(),'config_sha256':hashlib.sha256((R/'.cargo/config.toml').read_bytes()).hexdigest(),'config_impact':'jobs2 and target-dir only; wrapper pins target-dir and fresh target metadata/tree bytes exactly match captured output. No target/feature changing config omission found.','limits':['Source archives and complete extracted source trees not independently verified.','Missing notice/license coverage and legal admission/fulfillment unqualified.','Native I/O exclusion and full G01 unqualified.']}
(O/'provenance-boundary.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps({k:v for k,v in summary.items() if k not in ['package_checks','h2']},indent=2))
