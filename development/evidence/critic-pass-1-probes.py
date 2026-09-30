import sqlite3,json,pathlib,hashlib,datetime
root=pathlib.Path('dungeonflux'); m=json.loads((root/'development/plan-manifest.json').read_text()); schema=(root/'development/schema.sql').read_text()
src=sqlite3.connect('file:'+str((root/'development/workflow.sqlite3').resolve())+'?mode=ro',uri=True); db=sqlite3.connect(':memory:');src.backup(db);db.execute('pragma foreign_keys=on')
out={'manifest_sha256':hashlib.sha256((root/'development/plan-manifest.json').read_bytes()).hexdigest(),'schema_sha256':hashlib.sha256(schema.encode()).hexdigest(),'source_fingerprint':m['source_fingerprint'],'live_checks':{'integrity':src.execute('pragma integrity_check').fetchall(),'foreign_keys':src.execute('pragma foreign_key_check').fetchall(),'journal_mode':src.execute('pragma journal_mode').fetchone()[0]},'probes':[]}
def probe(name, fn):
 db.execute('savepoint p')
 try:
  value=fn();out['probes'].append({'name':name,'result':'ACCEPTED','observed':value})
 except sqlite3.Error as e:out['probes'].append({'name':name,'result':'REJECTED','observed':str(e)})
 finally:db.execute('rollback to p');db.execute('release p')
def copyrow(table,changes):
 columns=[r[1] for r in db.execute('pragma table_info('+table+')')]; row=list(db.execute('select * from '+table+' limit 1').fetchone())
 for k,v in changes.items():row[columns.index(k)]=v
 db.execute('insert into '+table+' ('+','.join(columns)+') values('+','.join('?' for x in columns)+')',row)
probe('null stable feature id',lambda:copyrow('features',{'id':None}))
probe('null stable task id',lambda:copyrow('tasks',{'id':None,'active_attempt_id':None}))
feature=db.execute('select id from features limit 1').fetchone()[0]
def newtask():
 copyrow('tasks',{'id':'critic_task','feature_id':feature,'status':'review','active_attempt_id':None,'lease_generation':7});db.execute('delete from dependencies where task_id=?',('critic_task',))
def newattempt(**kw):
 row={'id':'critic_attempt','task_id':'critic_task','worker':'worker','model':'luna','role':'worker','status':'active','phase':'implementation','lease_owner':'worker','lease_token':'critic-token','lease_generation':7,'brief_revision':'x','brief_json':'{}','lease_expires_at':'2999-01-01T00:00:00Z','tested_revision':'r','integrated_revision':'r','evaluator_id':'reviewer','evaluator_model':'frontier-model','capabilities_json':'{"frontier":true}','review_evidence_json':'["criterion"]','integration_evidence_json':'["integration"]','evidence_json':'[]','verdict':'approve','defects_json':'[]','started_at':'2026-01-01T00:00:00Z'};row.update(kw)
 db.execute('insert into attempts('+','.join(row)+')values('+','.join('?' for _ in row)+')',list(row.values()))
def bypass_insert():
 newtask();newattempt(status='integrated',phase='terminal',lease_token=None,lease_expires_at='2000-01-01T00:00:00Z');db.execute('update tasks set active_attempt_id=? where id=?',('critic_attempt','critic_task'));db.execute("update tasks set status='done' where id='critic_task'");return db.execute("select status from tasks where id='critic_task'").fetchone()[0]
probe('insert expired integrated attempt bypasses fencing and closes task',bypass_insert)
def selfmutate():
 newtask();newattempt();db.execute("update attempts set evaluator_id=worker where id='critic_attempt'");return db.execute("select worker,evaluator_id,verdict from attempts where id='critic_attempt'").fetchone()
probe('approved evidence fields permit self approval mutation',selfmutate)
def stale_fields():
 newtask();newattempt();db.execute("update tasks set active_attempt_id='critic_attempt' where id='critic_task'");db.execute("update attempts set status='integrated' where id='critic_attempt'");db.execute("update attempts set lease_expires_at='2000-01-01' where id='critic_attempt'");db.execute("update tasks set status='done' where id='critic_task'");return 'done'
probe('expired lease after integration closes task',stale_fields)
def mismatchlink():
 newtask();newattempt(verdict=None); another=db.execute("select id from tasks where id!='critic_task' limit 1").fetchone()[0];db.execute('update tasks set active_attempt_id=? where id=?',('critic_attempt',another));return 'cross-task active attempt accepted'
probe('active attempt belongs to another task',mismatchlink)
def feature_regress():
 db.execute("update features set status='active' where id=?",(feature,));db.execute("update features set status='done' where id=?",(feature,))
probe('unfinished feature done denied',feature_regress)
def cycle_probe():
 a,b=[r[0] for r in db.execute("select id from tasks limit 2")];db.execute('delete from dependencies where task_id in (?,?)',(a,b));db.execute('insert into dependencies values(?,?)',(a,b));db.execute('insert into dependencies values(?,?)',(b,a))
probe('cycle denied',cycle_probe)
probe('dangling prerequisite denied',lambda:db.execute('insert into dependencies values(?,?)',(db.execute('select id from tasks limit 1').fetchone()[0],'missing')))
probe('devlog update denied',lambda:db.execute("update devlog set summary='tamper' where id=(select id from devlog limit 1)"))
probe('devlog delete denied',lambda:db.execute('delete from devlog'))
probe('direct done task insert denied',lambda:copyrow('tasks',{'id':'critic_direct_done','status':'done','active_attempt_id':None}))
def live_two():
 newtask();newattempt(verdict=None);newattempt(id='critic_second',lease_token='second',verdict=None)
probe('duplicate live attempt denied',live_two)
def stale_submit():
 newtask();newattempt(verdict=None,lease_generation=6);db.execute("update tasks set active_attempt_id='critic_attempt' where id='critic_task'");db.execute("update attempts set status='submitted' where id='critic_attempt'")
probe('stale update submission denied',stale_submit)
# independent coverage/dependency and source parity
ids={t['id'] for t in m['tasks']}; fs={f['id'] for f in m['features']}; deps=m['dependencies'];out['manifest_checks']={'counts':{'features':len(fs),'tasks':len(ids),'dependencies':len(deps),'models':len(m['model_ledger'])},'task_id_unique':len(ids)==len(m['tasks']),'feature_id_unique':len(fs)==len(m['features']),'coverage':{},'missing_source_hashes':[]}
for category, expected in m['expected_coverage'].items():
 pref={'crates':'C-','features':'','rules':'','gates':'','slices':''}[category];out['manifest_checks']['coverage'][category]=[x for x in expected if pref+x not in fs]
for p,h in m['source_documents'].items():
 if hashlib.sha256((root/p).read_bytes()).hexdigest()!=h['sha256']:out['manifest_checks']['missing_source_hashes'].append(p)
pathlib.Path('work/plan-database-refinement/critic-probes.json').write_text(json.dumps(out,indent=2));print(json.dumps(out,indent=2))
