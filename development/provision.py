#!/usr/bin/env python3
"""Coordinator-owned planning-data provisioning tool, not the application runner."""
from pathlib import Path
import argparse,hashlib,json,sqlite3,os
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--directory',type=Path,default=Path(__file__).resolve().parent)
p.add_argument('--check-only',action='store_true')
a=p.parse_args();base=a.directory.resolve()
manifest=json.loads((base/'plan-manifest.json').read_text())
operational_path=base/'operational-tasks.json'
if operational_path.exists():
 operational=json.loads(operational_path.read_text())
 manifest['tasks']+=operational.get('tasks',[])
 manifest['dependencies']+=operational.get('dependencies',[])
 if len({t['id'] for t in manifest['tasks']})!=len(manifest['tasks']):raise SystemExit('Duplicate task across main and operational manifests')
path=base/'workflow.sqlite3'
if path.exists() and not a.check_only:
 old=sqlite3.connect(path);version=old.execute('PRAGMA user_version').fetchone()[0]
 if version==1:
  # Initial unreleased bootstrap schema only: preserve every row and a durable backup.
  if old.execute('SELECT count(*) FROM attempts').fetchone()[0] or old.execute("SELECT count(*) FROM tasks WHERE status IN ('running','review','done')").fetchone()[0]:
   raise SystemExit('Active/history-bearing schema migration requires a reviewed migration; refusing bootstrap rebuild')
  backup=sqlite3.connect(base/'workflow.pre-schema-v2.sqlite3');old.backup(backup);backup.close()
  old.execute('PRAGMA wal_checkpoint(TRUNCATE)')
  migration=base/'workflow.schema-migration.sqlite3'
  if migration.exists():raise SystemExit('Prior migration file exists; coordinator recovery required')
  new=sqlite3.connect(migration);new.execute('PRAGMA foreign_keys=ON');new.executescript((base/'schema.sql').read_text())
  with new:
   for table in ['features','tasks','dependencies','attempts','devlog']:
    columns=[r[1] for r in old.execute(f'PRAGMA table_info({table})')]
    for row in old.execute(f'SELECT {",".join(columns)} FROM {table}'):
     new.execute(f'INSERT INTO {table} ({",".join(columns)}) VALUES ({",".join("?" for _ in columns)})',row)
  assert new.execute('PRAGMA integrity_check').fetchone()==('ok',)
  assert not new.execute('PRAGMA foreign_key_check').fetchall()
  new.close();old.close();os.replace(migration,path)
 else:
  old.close()
  if version!=2:raise SystemExit(f'Unsupported schema version {version}; reviewed migration required')
if a.check_only:
 con=sqlite3.connect(path.as_uri()+'?mode=ro',uri=True)
else:
 con=sqlite3.connect(path)
 con.execute('PRAGMA journal_mode=WAL');con.execute('PRAGMA synchronous=FULL');con.execute('PRAGMA foreign_keys=ON');con.execute('PRAGMA busy_timeout=3000')
 con.executescript((base/'schema.sql').read_text())
 for table in ['features','tasks']:
  for row in manifest[table]:
   existing=con.execute(f'SELECT status,source_fingerprint FROM {table} WHERE id=?',(row['id'],)).fetchone()
   if existing and existing[0] in ['active','running','review'] and existing[1]!=row['source_fingerprint']:
    raise SystemExit(f'Refusing to change active plan {row["id"]}; coordinator must resolve ownership first')
 with con:
  for table in ['features','tasks']:
   for row in manifest[table]:
    values={k:json.dumps(v,ensure_ascii=False,sort_keys=True) if isinstance(v,(dict,list)) else v for k,v in row.items()}
    cols=list(values);keep={'status','created_at','blocking_reason','originating_task_id'}
    updates=','.join(f'{c}=excluded.{c}' for c in cols if c not in keep and c!='id')
    con.execute(f'INSERT INTO {table} ({",".join(cols)}) VALUES ({",".join("?" for _ in cols)}) ON CONFLICT(id) DO UPDATE SET {updates}',[values[c] for c in cols])
  expected={(d['task_id'],d['prerequisite_task_id']) for d in manifest['dependencies']}
  seeded={t['id'] for t in manifest['tasks']}
  actual=set(con.execute('SELECT task_id,prerequisite_task_id FROM dependencies'))
  for t,prereq in actual-expected:
   if t in seeded:con.execute('DELETE FROM dependencies WHERE task_id=? AND prerequisite_task_id=?',(t,prereq))
  for t,prereq in expected:con.execute('INSERT OR IGNORE INTO dependencies VALUES (?,?)',(t,prereq))
# Read-only status. Extra coordinator-intake rows are allowed; seeded rows must match design payloads.
con.execute('PRAGMA foreign_keys=ON')
assert con.execute('PRAGMA integrity_check').fetchone()==('ok',)
assert con.execute('PRAGMA foreign_key_check').fetchall()==[]
for table in ['features','tasks']:
 for row in manifest[table]:
  found=con.execute(f'SELECT source_fingerprint FROM {table} WHERE id=?',(row['id'],)).fetchone()
  assert found==(row['source_fingerprint'],),(table,row['id'],'seed parity')
for group,ids in manifest['expected_coverage'].items():
 required={'crates':['C-'+i for i in ids],'features':ids,'rules':ids,'gates':ids,'slices':ids}[group]
 for fid in required:assert con.execute('SELECT 1 FROM features WHERE id=?',(fid,)).fetchone(),fid
print(json.dumps({'database':str(path),'features':con.execute('SELECT count(*) FROM features').fetchone()[0],'tasks':con.execute('SELECT count(*) FROM tasks').fetchone()[0],'dependencies':con.execute('SELECT count(*) FROM dependencies').fetchone()[0],'attempts':con.execute('SELECT count(*) FROM attempts').fetchone()[0],'devlog':con.execute('SELECT count(*) FROM devlog').fetchone()[0],'dispatch_ready':con.execute('SELECT count(*) FROM dispatch_ready_tasks').fetchone()[0],'integrity':'ok','foreign_key_violations':0,'source_fingerprint':manifest['source_fingerprint']}))
if not a.check_only:con.execute('PRAGMA wal_checkpoint(TRUNCATE)')
con.close()
