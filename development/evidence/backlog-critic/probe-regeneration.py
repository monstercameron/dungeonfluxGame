#!/usr/bin/env python3
"""Independent already-expanded input reproduction test, no authoritative writes."""
import argparse,pathlib,json,hashlib,importlib.util,collections
p=argparse.ArgumentParser();p.add_argument("root",type=pathlib.Path);p.add_argument("candidate",type=pathlib.Path);p.add_argument("output",type=pathlib.Path);a=p.parse_args();root=a.root.resolve();raw=a.candidate.read_bytes();before=json.loads(raw);catalog=json.loads((root/"development/backlog-catalog.json").read_text());script=root/"development/expand-backlog.py";script_sha=hashlib.sha256(script.read_bytes()).hexdigest()
spec=importlib.util.spec_from_file_location("critic_owned_expander",script);mod=importlib.util.module_from_spec(spec);spec.loader.exec_module(mod);after=mod.expand(before,root,catalog)
def strip(x):
 if isinstance(x,dict):return {k:strip(v) for k,v in x.items() if k not in {"generated_at","updated_at"}}
 if isinstance(x,list):return [strip(v) for v in x]
 return x
changes=[]
for table in ["features","tasks"]:
 old={r["id"]:r for r in before[table]};new={r["id"]:r for r in after[table]};assert old.keys()==new.keys(),(table,"IDs drifted")
 for tid in old:
  if strip(old[tid])!=strip(new[tid]):changes.append({"table":table,"id":tid,"changed_fields":[k for k in old[tid].keys()|new[tid].keys() if strip(old[tid].get(k))!=strip(new[tid].get(k))]})
oldedges={(e["task_id"],e["prerequisite_task_id"]) for e in before["dependencies"]};newedges={(e["task_id"],e["prerequisite_task_id"]) for e in after["dependencies"]}
assert oldedges==newedges,"Expanded-input dependencies changed"
result={"candidate_sha256":hashlib.sha256(raw).hexdigest(),"generator_sha256":script_sha,"exact_whole_manifest_parity":before==after,"logical_manifest_parity":strip(before)==strip(after),"changed_rows":changes,"ids":len(before["tasks"]),"dependencies":len(oldedges),"normalization":"Only generated_at/updated_at ignored; all task created timestamps, bodies, IDs, source identities and logical edges compared","verdict":"PASS" if not changes and strip(before)==strip(after) else "FAIL"};a.output.write_text(json.dumps(result,indent=2)+"\n");print(json.dumps(result));assert result["verdict"]=="PASS","Already-expanded input is not reproducible"
