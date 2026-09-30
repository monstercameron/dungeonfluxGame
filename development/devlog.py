#!/usr/bin/env python3
"""Append one scoped development observation. This local tool never mutates queue state."""
from pathlib import Path
import argparse,datetime,json,sqlite3,sys
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--database',type=Path,default=Path(__file__).resolve().parent/'workflow.sqlite3')
p.add_argument('--context',type=Path,required=True,help='Coordinator-supplied execution context: agent_id, role, task_id, attempt_id')
p.add_argument('--entry',type=Path,required=True,help='JSON with id, kind, summary, details, action, outcome and optional suggestion/related_entry_id/evidence_ref')
a=p.parse_args();context=json.loads(a.context.read_text());entry=json.loads(a.entry.read_text())
allowed={'id','kind','summary','details','action','outcome','suggestion','related_entry_id','evidence_ref'}
if set(entry)-allowed:raise SystemExit('Unsupported fields; identity/task references come from execution context')
for field in ['agent_id','role']:
 if not isinstance(context.get(field),str) or not context[field].strip():raise SystemExit('Missing context '+field)
if context['role'] not in ['worker','evaluator','critic','cleanup','coordinator']:raise SystemExit('Invalid role')
for field in ['id','kind','summary','details','action','outcome']:
 if not isinstance(entry.get(field),str) or not entry[field].strip():raise SystemExit('Missing entry '+field)
if entry['id']!=entry['id'].strip():raise SystemExit('Invalid stable ID')
if entry['kind'] not in ['confusion','error','defect','blocker','challenge','discovery','resolution']:raise SystemExit('Invalid observation kind')
for field,limit in [('summary',300),('details',16000),('action',8000),('outcome',8000),('evidence_ref',2000)]:
 if entry.get(field) and len(entry[field])>limit:raise SystemExit(field+' exceeds bound; retain large evidence separately')
con=sqlite3.connect(a.database.resolve().as_uri()+'?mode=rw',uri=True)
con.execute('PRAGMA foreign_keys=ON');con.execute('PRAGMA busy_timeout=3000')
task_id=context.get('task_id');attempt_id=context.get('attempt_id')
if attempt_id:
 attempt=con.execute('SELECT task_id,worker,evaluator_id FROM attempts WHERE id=?',(attempt_id,)).fetchone()
 if not attempt or attempt[0]!=task_id:raise SystemExit('Context task/attempt mismatch')
 if context['role']!='coordinator' and context['agent_id'] not in attempt[1:]:raise SystemExit('Agent not assigned to context attempt')
row={'id':entry['id'],'created_at':datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds'),'agent_id':context['agent_id'],'role':context['role'],'task_id':task_id,'attempt_id':attempt_id,'kind':entry['kind'],'summary':entry['summary'],'details':entry['details'],'action':entry['action'],'outcome':entry['outcome'],'suggestion':entry.get('suggestion'),'related_entry_id':entry.get('related_entry_id'),'evidence_ref':entry.get('evidence_ref')}
with con:
 old=con.execute('SELECT '+','.join(row)+' FROM devlog WHERE id=?',(row['id'],)).fetchone()
 if old:
  for field,value,previous in zip(row,row.values(),old):
   if field!='created_at' and value!=previous:raise SystemExit('ID already has different immutable observation; append linked correction')
  result='existing'
 else:
  con.execute('INSERT INTO devlog('+','.join(row)+') VALUES('+','.join('?' for _ in row)+')',list(row.values()));result='appended'
print(json.dumps({'id':row['id'],'result':result,'queue_state_changed':False}))
con.close()
