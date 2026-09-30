"""Independent readonly coverage/history check for the subsequent feature outline."""
import hashlib
import json
import re
import sqlite3
from pathlib import Path

ROOT = Path('/Users/earlcameron/Documents/Codex/2026-09-29/cr/dungeonflux')
OUT = ROOT.parent/'work/gap-refinement-critic'
ATTACHMENT = Path('/Users/earlcameron/.codex/attachments/f428f549-c65c-48d2-ab64-ed92ac4c3891/Pasted text.txt')
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
manifest_path = ROOT/'development/plan-manifest.json'
manifest = json.loads(manifest_path.read_text())
mapping = json.loads((ROOT/'development/evidence/feature-brief-mapping.json').read_text())
brief = ATTACHMENT.read_text()
assert mapping['source_sha256'] == sha(ATTACHMENT)
ideas = [(int(ident), title) for ident,title in re.findall(r'^# (\d+)\. (.+)$',brief,re.M)]
assert [(row['id'],row['title']) for row in mapping['numbered_ideas']] == ideas
assert len(ideas) == 15
priorities = [(name,priority) for name,priority in re.findall(r'^\| \*\*(.+?)\*\* \| .*? \| .*? \| \*\*(P\d)\*\* \|$',brief,re.M)]
assert [(row['feature'],row['priority']) for row in mapping['priority_rows']] == priorities
assert len(priorities) == 15
stakes_source = brief.split('### You need:',1)[1].split("These aren't killer features.",1)[0]
stakes = [value.rstrip('.') for value in re.findall(r'^\*\*(.+?)\*\*',stakes_source,re.M)]
assert [row['feature'] for row in mapping['table_stakes']] == stakes
assert len(stakes) == 10
phase_block = next(block for block in re.findall(r'```text\n(.*?)\n```',brief,re.S) if 'PHASE 1' in block)
phase_parts = re.split(r'PHASE\s+\d',phase_block)[1:]
phase_items = []
for part in phase_parts:
    lines = [line.strip() for line in part.splitlines() if line.strip()]
    phase_items.append([line for line in lines[1:] if line not in ['│','▼']])
assert [row['items'] for row in mapping['phases']] == phase_items

db = sqlite3.connect((ROOT/'development/workflow.sqlite3').as_uri()+'?mode=ro',uri=True)
db.row_factory = sqlite3.Row
baseline = json.loads((OUT/'feature-baseline.json').read_text())
logs = {row['id']:dict(row) for row in db.execute('SELECT * FROM devlog')}
for ident,row in baseline['devlogs'].items(): assert logs[ident] == row, ident
for row in baseline['attempts']: assert dict(db.execute('SELECT * FROM attempts WHERE id=?',(row['id'],)).fetchone()) == row
for row in baseline['task_states']: assert dict(db.execute('SELECT id,status,blocking_reason FROM tasks WHERE id=?',(row['id'],)).fetchone()) == row
for row in baseline['feature_states']: assert dict(db.execute('SELECT id,status FROM features WHERE id=?',(row['id'],)).fetchone()) == row
assert sha(ROOT/'development/schema.sql') == baseline['schema_sha256']
new_ids = [f'F{ident}-{phase}' for ident in [45,46,47] for phase in ['MODEL','DELIVER','ACCEPT']]
new_ids += ['F03-PRIVATE-ACCEPT','F15-CRITICAL-ACCEPT','F42-SPOTLIGHT-ACCEPT','F35-CONTINUITY-ACCEPT']
new_tasks = {}
for ident in new_ids:
    row = dict(db.execute('SELECT * FROM tasks WHERE id=?',(ident,)).fetchone())
    plan = json.loads(row['brief_json'])
    assert row['status'] == 'pending' and plan['dispatch_ready'] is False, ident
    assert plan['owners'] and plan['integration_hooks'] and plan['governing_sources'], ident
    assert json.loads(row['acceptance_json']) and json.loads(row['verification_json']), ident
    new_tasks[ident] = {'feature':row['feature_id'],'owners':plan['owners'],
                        'acceptance':json.loads(row['acceptance_json']),
                        'prerequisites':[value[0] for value in db.execute('SELECT prerequisite_task_id FROM dependencies WHERE task_id=?',(ident,))]}
assert len(manifest['expected_coverage']['features']) == 47
assert len(manifest['model_ledger']) == 40
assert len(manifest['api_services']) == 11
assert sum(len(row['methods']) for row in manifest['api_services']) == 37
assert db.execute('PRAGMA integrity_check').fetchone()[0] == 'ok'
assert not db.execute('PRAGMA foreign_key_check').fetchall()
assert db.execute('SELECT count(*) FROM dispatch_ready_tasks').fetchone()[0] == 0
assert db.execute("SELECT count(*) FROM features WHERE kind!='planning' AND status='done'").fetchone()[0] == 0
dns = db.execute("SELECT status,brief_json FROM tasks WHERE id='SITE-GODADDY-DNS'").fetchone()
assert dns['status'] == 'blocked' and json.loads(dns['brief_json'])['dispatch_ready'] is False
source_hashes = {path:sha(ROOT/path) for path in manifest['source_documents']}
assert all(source_hashes[path] == record['sha256'] for path,record in manifest['source_documents'].items())
result = {'source_fingerprint':manifest['source_fingerprint'],'manifest_sha256':sha(manifest_path),
          'attachment_sha256':sha(ATTACHMENT),'source_hashes':source_hashes,
          'coverage':{'numbered_ideas':15,'priority_rows':15,'table_stakes':10,'phases':3,'phase_item_counts':list(map(len,phase_items))},
          'preserved_history':{'devlogs':len(baseline['devlogs']),'attempts':len(baseline['attempts']),'task_statuses':len(baseline['task_states'])},
          'new_tasks':new_tasks,'counts':{name:db.execute('SELECT count(*) FROM '+name).fetchone()[0] for name in ['features','tasks','dependencies','attempts','devlog']},
          'schema_unchanged':True,'dispatch_ready':0,'crates':40,'features':47,'rpc_services':11,'rpc_methods':37,
          'excluded_derived_artifact':'planning/engine-overview.md has a separate frozen artifact review',
          'limits':['Planning and actual SQLite/tool boundary verification only; no game/runtime/native/WASM/browser/device/audio execution.']}
(OUT/'feature-audit.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({key:result[key] for key in ['source_fingerprint','manifest_sha256','coverage','counts','schema_unchanged','dispatch_ready']}))
