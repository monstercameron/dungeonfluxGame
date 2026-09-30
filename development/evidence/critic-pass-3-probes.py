import sqlite3,json,pathlib,hashlib
root=pathlib.Path('dungeonflux'); manifest=json.loads((root/'development/plan-manifest.json').read_text()); live=sqlite3.connect('file:'+str((root/'development/workflow.sqlite3').resolve())+'?mode=ro',uri=True);db=sqlite3.connect(':memory:');live.backup(db);db.execute('pragma foreign_keys=on');rows=[]
cols=lambda table:[x[1] for x in db.execute('pragma table_info('+table+')')]
def copy(table,changes):
 c=cols(table);v=list(db.execute('select * from '+table+' limit 1').fetchone())
 for k,x in changes.items():v[c.index(k)]=x
 db.execute('insert into '+table+'('+','.join(c)+')values('+','.join('?'for _ in c)+')',v)
def setup():
 copy('features',{'id':'criticF','status':'planned'});copy('tasks',{'id':'criticT','feature_id':'criticF','status':'review','active_attempt_id':None,'lease_generation':1,'acceptance_json':'["c1"]','brief_json':'{"required_capabilities":{"computer_use":true,"vision":true}}'})
def attempt(**kwargs):
 x={'id':'criticA','task_id':'criticT','worker':'worker','model':'luna','role':'worker','status':'active','phase':'implementation','lease_owner':'worker','lease_token':'token','lease_generation':1,'brief_revision':'b','brief_json':'{}','lease_expires_at':'2999-01-01T00:00:00Z','evidence_json':'[]','defects_json':'[]','started_at':'2026-01-01T00:00:00Z'};x.update(kwargs);db.execute('insert into attempts('+','.join(x)+') values('+','.join('?'for _ in x)+')',list(x.values()));db.execute("update tasks set active_attempt_id='criticA' where id='criticT'")
def review():
 attempt();db.execute("update attempts set status='submitted',phase='review' where id='criticA'");db.execute("update attempts set phase='review',lease_owner='reviewer' where id='criticA'");db.execute('update attempts set verdict=?,status=?,evaluator_id=?,evaluator_model=?,tested_revision=?,capabilities_json=?,review_evidence_json=? where id=?',('approve','approved','reviewer','frontier','r','{"frontier":true,"computer_use_enabled":true,"vision_enabled":true}','[{"criterion":"c1","result":"pass","evidence_ref":"proof"}]','criticA'))
def integrated():
 review();db.execute("update attempts set phase='integration',lease_owner='coordinator' where id='criticA'");db.execute("update attempts set status='integrated', integrated_revision='r',integration_evidence_json='[\"verified\"]',phase='terminal' where id='criticA'")
def done():db.execute("update tasks set status='done' where id='criticT'")
def probe(name, fn, expected='REJECTED'):
 db.execute('savepoint p')
 try:setup();value=fn();result='ACCEPTED';detail='operation completed' if value is not None else None
 except sqlite3.Error as e:result='REJECTED';detail=str(e)
 finally:db.execute('rollback to p');db.execute('release p')
 rows.append({'case':name,'expected':expected,'result':result,'pass':result==expected,'detail':detail})
def action(*cmds):
 for c in cmds:db.execute(c)
probe('positive reviewed integrated task',lambda:(integrated(),done()),'ACCEPTED')
probe('terminal integrated insertion',lambda:attempt(status='integrated',phase='terminal',lease_token=None))
probe('NULL feature id',lambda:copy('features',{'id':None}))
probe('empty task id',lambda:copy('tasks',{'id':'  ','active_attempt_id':None}))
probe('postapproval evaluator worker mutation',lambda:(review(),db.execute("update attempts set evaluator_id=worker where id='criticA'")))
probe('postapproval worker mutation',lambda:(review(),db.execute("update attempts set worker='another' where id='criticA'")))
probe('postapproval criterion removal',lambda:(review(),db.execute("update attempts set review_evidence_json='[]' where id='criticA'")))
probe('postapproval required capability removal',lambda:(review(),db.execute("update attempts set capabilities_json='{\"frontier\":true}' where id='criticA'")))
probe('expired new submission',lambda:(attempt(lease_expires_at='2000-01-01'),db.execute("update attempts set status='submitted',phase='review' where id='criticA'")))
probe('malformed expiry submission',lambda:(attempt(lease_expires_at='not-a-date'),db.execute("update attempts set status='submitted',phase='review' where id='criticA'")))
probe('empty token submission',lambda:(attempt(lease_token=''),db.execute("update attempts set status='submitted',phase='review' where id='criticA'")))
probe('NULL lease owner submission',lambda:(attempt(lease_owner=None),db.execute("update attempts set status='submitted',phase='review' where id='criticA'")))
probe('review verdict outside phase',lambda:(attempt(),db.execute("update attempts set verdict='reject',evaluator_id='reviewer' where id='criticA'")))
probe('integrating directly from approved review phase',lambda:(review(),db.execute("update attempts set status='integrated',integrated_revision='r',integration_evidence_json='[\"ok\"]' where id='criticA'"),done()))
probe('terminal active allows second live worker',lambda:(attempt(),db.execute("update attempts set phase='terminal' where id='criticA'"),db.execute("insert into attempts(id,task_id,worker,model,role,status,phase,brief_revision,brief_json,evidence_json,defects_json,started_at)values('criticB','criticT','second','luna','worker','active','implementation','b','{}','[]','[]','now')")))
probe('criteria changed after review then done',lambda:(integrated(),db.execute("update tasks set acceptance_json='[\"c1\",\"new-required\"]' where id='criticT'"),done()))
probe('build mismatch done',lambda:(integrated(),db.execute("update attempts set integrated_revision='other' where id='criticA'"),done()))
probe('selfapproval evidence done',lambda:(integrated(),db.execute("update attempts set evaluator_id='worker' where id='criticA'"),done()))
probe('no independent evidence taskdone',done)
probe('cycle edge',lambda:(db.execute("insert into dependencies values('criticT','G01-RESOLVE')"),db.execute("insert into dependencies values('G01-RESOLVE','criticT')")))
probe('dangling edge',lambda:db.execute("insert into dependencies values('criticT','missing')"))
probe('appendonly update',lambda:db.execute("update devlog set summary='tampered'"))
probe('appendonly delete',lambda:db.execute('delete from devlog'))
probe('zero lease generation',lambda:attempt(lease_generation=0))
probe('NULL attempt id',lambda:attempt(id=None))
probe('missing frontier evidence',lambda:(review(),db.execute("update attempts set capabilities_json='{\"computer_use_enabled\":true,\"vision_enabled\":true}' where id='criticA'")))
probe('stale generation after submission',lambda:(review(),db.execute("update attempts set lease_generation=2,status='approved' where id='criticA'")))
probe('terminal cannot return active',lambda:(integrated(),db.execute("update attempts set status='active',phase='implementation' where id='criticA'")))
probe('current task capability contract changes',lambda:(integrated(),db.execute("update tasks set brief_json='{\"required_capabilities\":{\"audio\":true}}' where id='criticT'")))
probe('terminal INSERT with otherwise valid lease',lambda:attempt(status='integrated',phase='terminal'))
probe('expiry becomes invalid after valid claim',lambda:(attempt(),db.execute("update attempts set lease_expires_at='not-a-date' where id='criticA'")))
probe('expiry expires at submission transition',lambda:(attempt(),db.execute("update attempts set status='submitted',phase='review',lease_expires_at='2000-01-01' where id='criticA'")))
probe('token erased after valid claim',lambda:(attempt(),db.execute("update attempts set lease_token='' where id='criticA'")))
probe('second coherent live attempt denied',lambda:(attempt(),db.execute("insert into attempts(id,task_id,worker,model,role,status,phase,lease_owner,lease_token,lease_generation,lease_expires_at,brief_revision,brief_json,evidence_json,defects_json,started_at)values('criticB','criticT','second','luna','worker','active','implementation','second','secondtoken',1,'2999-01-01','b','{}','[]','[]','now')")))
probe('delayed done after valid terminal integration',lambda:(integrated(),db.execute("update attempts set lease_expires_at='2000-01-01' where id='criticA'"),done()),'ACCEPTED')
result={'manifest_sha256'  :hashlib.sha256((root/'development/plan-manifest.json').read_bytes()).hexdigest(),'schema_sha256':hashlib.sha256((root/'development/schema.sql').read_bytes()).hexdigest(),'source_fingerprint':manifest['source_fingerprint'],'db_version':live.execute('pragma user_version').fetchone()[0],'integrity':live.execute('pragma integrity_check').fetchone()[0],'fk':live.execute('pragma foreign_key_check').fetchall(),'journal':live.execute('pragma journal_mode').fetchone()[0],'probes':rows}
pathlib.Path('work/critic-pass-3/probes.json').write_text(json.dumps(result,indent=2));print(json.dumps(result,indent=2))
