#!/usr/bin/env python3
"""Independent view guard probes using only a private in-memory DB backup."""
import argparse,json,pathlib,sqlite3,hashlib
p=argparse.ArgumentParser();p.add_argument("database",type=pathlib.Path);p.add_argument("output",type=pathlib.Path);a=p.parse_args();source=sqlite3.connect(a.database.resolve().as_uri()+"?mode=ro",uri=True);c=sqlite3.connect(":memory:");source.backup(c);source.close();c.row_factory=sqlite3.Row;c.execute("PRAGMA foreign_keys=ON")
view=c.execute("SELECT sql FROM sqlite_master WHERE name='dispatch_ready_tasks'").fetchone()[0];assert "record_role" in view and "scope_status" in view,view
# Isolate a previously pending, unclaimed child. Clear prerequisites so negative
# assertions cannot pass merely because dependency readiness or scope is false.
row=c.execute("SELECT * FROM tasks WHERE status='pending' AND json_extract(brief_json,'$.record_role')='atomic_blueprint' AND active_attempt_id IS NULL LIMIT1".replace("LIMIT1","LIMIT 1")).fetchone();assert row is not None
id=row["id"];original=json.loads(row["brief_json"]);c.execute("DELETE FROM dependencies WHERE task_id=?",(id,));results=[]
def attempt(role,scope,flag,want):
 b=dict(original);b.update(record_role=role,scope_status=scope,dispatch_ready=flag);c.execute("UPDATE tasks SET brief_json=? WHERE id=?",(json.dumps(b),id));actual=bool(c.execute("SELECT1 FROM dispatch_ready_tasks WHERE id=?".replace("SELECT1","SELECT 1"),(id,)).fetchone());assert actual==want,(role,scope,flag,actual,want);results.append({"role":role,"scope_status":scope,"dispatch_ready":flag,"dependencies":0,"appeared":actual,"expected":want})
for role in ["aggregate","reference","atomic_blueprint"]:attempt(role,"frozen",True,False)
attempt("atomic","planned",True,False);attempt("atomic","frozen",False,False);attempt("atomic","frozen",True,True);attempt("operational","frozen",True,True)
a.output.write_text(json.dumps({"verdict":"PASS","view_sql":view,"isolated_task":id,"probes":results,"scope":"Actual candidate view copied to memory. Source DB never mutated; positive control distinguishes safe rejection from vacuous readiness failure."},indent=2)+"\n");print(json.dumps({"verdict":"PASS","probes":len(results),"output":str(a.output)}))
