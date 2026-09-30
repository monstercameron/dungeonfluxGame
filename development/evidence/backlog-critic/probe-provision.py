#!/usr/bin/env python3
"""Independent boundary probes; writes only owned fixture copies."""
import argparse, pathlib, json, shutil, sqlite3, subprocess, hashlib
p=argparse.ArgumentParser();p.add_argument("development",type=pathlib.Path);p.add_argument("output",type=pathlib.Path);p.add_argument("--root",required=True,type=pathlib.Path);a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
base=a.development.resolve();manifest=json.loads((base/"plan-manifest.json").read_text());results=[]
def create(name):
 d=a.output/name
 if d.exists():shutil.rmtree(d)
 d.mkdir()
 for f in ["provision.py","validate-plan-seed.py","schema.sql","plan-manifest.json","operational-tasks.json"]:shutil.copy2((base/f if (base/f).exists() else a.root.resolve()/"development"/f),d/f)
 origin=sqlite3.connect((base/"workflow.sqlite3").as_uri()+"?mode=ro",uri=True);target=sqlite3.connect(d/"workflow.sqlite3");origin.backup(target);target.close();origin.close();return d

def rows(d):
 c=sqlite3.connect(d/"workflow.sqlite3");c.row_factory=sqlite3.Row
 v={t:[dict(r) for r in c.execute("SELECT * FROM "+t+" ORDER BY "+("task_id,prerequisite_task_id" if t=="dependencies" else "id"))] for t in ["features","tasks","dependencies","attempts","devlog"]};c.close();return v

def run(d):
 return subprocess.run(["python3",str(d/"provision.py"),"--directory",str(d),"--root",str(a.root.resolve())],capture_output=True,text=True,timeout=120)

def mutate(d,fn):
 q=d/"plan-manifest.json";m=json.loads(q.read_text());fn(m);q.write_text(json.dumps(m))

def check(name,fn):
 d=create(name);fn(d)

# Reprovision exact seed twice, compare all persisted data excluding no columns.
d=create("idempotent");before=rows(d);one=run(d);two=run(d);after=rows(d)
assert one.returncode==two.returncode==0,(one.stderr,two.stderr)
assert before==after,"Repeated seed changed execution or history/payload rows"
results.append({"probe":"exact_seed_twice","outcome":"PASS","rows_unchanged":True})

# Existing v2 deployment must actually replace its old permissive view.
d=create("legacy_view_migration")
c=sqlite3.connect(d/"workflow.sqlite3");c.executescript("DROP VIEW dispatch_ready_tasks; CREATE VIEW dispatch_ready_tasks AS SELECT t.id,t.feature_id,t.title,t.stage,t.priority FROM tasks t WHERE t.status='pending' AND json_extract(t.brief_json,'$.dispatch_ready')=1 AND NOT EXISTS(SELECT 1 FROM dependencies d JOIN tasks p ON p.id=d.prerequisite_task_id WHERE d.task_id=t.id AND p.status!='done');");c.close()
r=run(d);assert r.returncode==0,r.stderr
c=sqlite3.connect(d/"workflow.sqlite3");view=c.execute("SELECT sql FROM sqlite_master WHERE name='dispatch_ready_tasks'").fetchone()[0];c.close()
assert "record_kind" in view or "aggregate" in view or "atomic" in view,"Existing DB retained old permissive dispatch view"
results.append({"probe":"legacy_v2_view_replaced","outcome":"PASS","view_sql":view})

# Fail closed and rollback plan mutations for malformed input. The documented
# view DDL migration precedes plan transaction and is not claimed to rollback.
mutations={
 "duplicate_id":lambda m:m["tasks"].append(dict(m["tasks"][0])),
 "dangling_dependency":lambda m:m["dependencies"].append({"task_id":m["tasks"][0]["id"],"prerequisite_task_id":"CRITIC-MISSING"}),
 "self_cycle":lambda m:m["dependencies"].append({"task_id":m["tasks"][0]["id"],"prerequisite_task_id":m["tasks"][0]["id"]}),
 "reverse_existing_edge":lambda m:m["dependencies"].append({"task_id":m["dependencies"][0]["prerequisite_task_id"],"prerequisite_task_id":m["dependencies"][0]["task_id"]}),
 "invalid_stage":lambda m:m["tasks"][0].update(stage="unknown-stage"),
 "empty_acceptance":lambda m:m["tasks"][0].update(acceptance_json=[]),
 "missing_feature":lambda m:m["tasks"][0].update(feature_id="CRITIC-MISSING")}
for name,fn in mutations.items():
 d=create(name);before=rows(d);mutate(d,fn);r=run(d);assert r.returncode!=0,(name,"bad input admitted");assert rows(d)==before,(name,"partial plan mutation")
 results.append({"probe":name,"outcome":"PASS","negative_rejected":True,"plan_rows_preserved":True,"error":r.stderr[-300:]})
# Existing active ownership cannot be overwritten by a new source revision.
d=create("active_plan");c=sqlite3.connect(d/"workflow.sqlite3");tid=manifest["tasks"][0]["id"];c.execute("UPDATE tasks SET status='running' WHERE id=?",(tid,));c.commit();c.close();before=rows(d)
mutate(d,lambda m:m["tasks"][0].update(source_fingerprint="critic-different-source"));r=run(d);assert r.returncode!=0;assert before==rows(d)
results.append({"probe":"active_plan_revision_refusal","outcome":"PASS","negative_rejected":True,"plan_rows_preserved":True})
# Identical governing source fingerprint does not permit changing an owned brief.
for name,change in {
 "same_source_active_edit_scope":lambda t:t["edit_areas_json"].append("crates/df-ai/tests/new_owned_scope.rs"),
 "same_source_active_criteria":lambda t:t["acceptance_json"].append("Additional changed contract criterion"),
 "same_source_active_brief":lambda t:t["brief_json"].update(known_pitfalls=["Changed owned contract without source fingerprint change"])
}.items():
 d=create(name);c=sqlite3.connect(d/"workflow.sqlite3");tid=manifest["tasks"][0]["id"];c.execute("UPDATE tasks SET status='running' WHERE id=?",(tid,));c.commit();c.close();before=rows(d)
 mutate(d,lambda m:change(m["tasks"][0]));r=run(d);assert r.returncode!=0,(name,"Owned same-source payload overwritten");assert before==rows(d),(name,"Owned row changed")
 results.append({"probe":name,"outcome":"PASS","negative_rejected":True,"plan_rows_preserved":True,"error":r.stderr[-300:]})
# Frozen prerequisite sets are part of ownership even when source hash is unchanged.
for name,mode in [("same_source_active_add_dependency","add"),("same_source_active_remove_dependency","remove")]:
 d=create(name);c=sqlite3.connect(d/"workflow.sqlite3");tid=manifest["tasks"][0]["id"];c.execute("UPDATE tasks SET status='running' WHERE id=?",(tid,));c.commit();c.close();before=rows(d)
 def change(m):
  if mode=="add":m["dependencies"].append({"task_id":tid,"prerequisite_task_id":"B-P01-D02"})
  else:
   edge=next(e for e in m["dependencies"] if e["task_id"]==tid);m["dependencies"].remove(edge)
 mutate(d,change);r=run(d);assert r.returncode!=0,(name,"Owned prerequisite set changed");assert before==rows(d),(name,"Owned graph changed")
 results.append({"probe":name,"outcome":"PASS","negative_rejected":True,"plan_rows_preserved":True,"error":r.stderr[-300:]})
# Active family scope is frozen as well as task scope.
d=create("same_source_active_family");c=sqlite3.connect(d/"workflow.sqlite3");fid=manifest["features"][0]["id"];c.execute("UPDATE features SET status='active' WHERE id=?",(fid,));c.commit();c.close();before=rows(d)
mutate(d,lambda m:m["features"][0].update(goal=m["features"][0]["goal"]+" Changed active scope."));r=run(d);assert r.returncode!=0,"Owned family payload changed";assert before==rows(d)
results.append({"probe":"same_source_active_family","outcome":"PASS","negative_rejected":True,"plan_rows_preserved":True,"error":r.stderr[-300:]})
(a.output/"provision-probes.json").write_text(json.dumps({"source_manifest_sha256":hashlib.sha256((base/"plan-manifest.json").read_bytes()).hexdigest(),"checks":results},indent=2)+"\n")
print(json.dumps({"checks":len(results),"verdict":"PASS","output":str(a.output/"provision-probes.json")}))
