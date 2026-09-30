"""Read-only frozen gap-brief and durable-plan audit; evidence is written here only."""
import collections
import hashlib
import json
import sqlite3
from pathlib import Path

ROOT = Path('/Users/earlcameron/Documents/Codex/2026-09-29/cr/dungeonflux')
OUT = ROOT.parent / 'work/gap-refinement-critic'
ATTACHMENT = Path('/Users/earlcameron/.codex/attachments/b0b3eca5-7201-4e49-acd6-67f1c9082e26/Pasted text.txt')
def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
paths = [ROOT/'AGENTS.md', ROOT/'README.md', *sorted((ROOT/'ADR').glob('*.md')),
         *sorted(path for path in (ROOT/'planning').glob('*.md') if path.name != 'engine-overview.md'),
         *[ROOT/'development'/name for name in ['schema.sql', 'plan-manifest.json', 'operational-tasks.json', 'provision.py', 'verify-plans.py']]]
before = {str(path.relative_to(ROOT)): sha(path) for path in paths}
manifest = json.loads((ROOT/'development/plan-manifest.json').read_text())
mapping = json.loads((ROOT/'development/evidence/gap-brief-mapping.json').read_text())
brief = ATTACHMENT.read_text()
headings = ['Narrative engine', 'NPC & interaction engine', 'Rules & combat', 'World simulation',
            'Tempo engine', 'Asset orchestration', 'Speech', 'Images / scenes', 'Live cutscenes',
            'Persistence & memory', 'Creator / worldbuilding tools', 'Production systems']
section = brief.split('4. Research backlog by subsystem', 1)[1].split('5. Questions', 1)[0]
expected = [line.strip() for line in section.splitlines() if line.strip() and line.strip() not in headings]
assert len(expected) == 76
assert mapping['source_sha256'] == sha(ATTACHMENT)
assert [item['research_item'] for item in mapping['items']] == expected
assert [item['id'] for item in mapping['items']] == [f'B{index:02d}' for index in range(1,77)]
for item in mapping['items']:
    assert (ROOT/item['design']).exists(), item['id']
    assert item['status'] and item['plans'], item['id']

db = sqlite3.connect((ROOT/'development/workflow.sqlite3').as_uri()+'?mode=ro', uri=True)
db.row_factory = sqlite3.Row
baseline = json.loads((OUT/'baseline.json').read_text())
actual_logs = {row['id']:dict(row) for row in db.execute('SELECT * FROM devlog')}
for ident, row in baseline['devlogs'].items():
    assert actual_logs[ident] == row, ('historical devlog changed', ident)
for row in baseline['attempts']:
    assert dict(db.execute('SELECT * FROM attempts WHERE id=?', (row['id'],)).fetchone()) == row
for row in baseline['task_states']:
    assert dict(db.execute('SELECT id,status,blocking_reason FROM tasks WHERE id=?', (row['id'],)).fetchone()) == row
for row in baseline['features_states']:
    assert dict(db.execute('SELECT id,status FROM features WHERE id=?', (row['id'],)).fetchone()) == row
assert before['development/schema.sql'] == baseline['schema_sha256']
new_ids = ['X10-RESOLVE', 'X10-AUTHOR', 'X10-ACCEPT', 'X11-RESOLVE', 'X11-ACCEPT',
           'RULE-EFFECT-CONTRACT', 'MEMORY-LONGHORIZON-ACCEPT', 'WORLD-TIME-ACCEPT', 'SPEECH-ROOM-ACCEPT']
new_tasks = {}
for ident in new_ids:
    row = db.execute('SELECT * FROM tasks WHERE id=?', (ident,)).fetchone()
    assert row, ident
    row = dict(row)
    plan = json.loads(row['brief_json'])
    assert plan['dispatch_ready'] is False and row['status'] == 'pending', ident
    assert json.loads(row['acceptance_json']) and json.loads(row['verification_json']), ident
    assert plan['owners'] and plan['integration_hooks'] and plan['governing_sources'], ident
    new_tasks[ident] = {'feature':row['feature_id'], 'stage':row['stage'], 'owners':plan['owners'],
                        'acceptance':json.loads(row['acceptance_json']),
                        'prerequisites':[r[0] for r in db.execute('SELECT prerequisite_task_id FROM dependencies WHERE task_id=?', (ident,))]}
dns = dict(db.execute("SELECT * FROM tasks WHERE id='SITE-GODADDY-DNS'").fetchone())
assert dns['status'] == 'blocked' and json.loads(dns['brief_json'])['dispatch_ready'] is False
assert db.execute('PRAGMA integrity_check').fetchone()[0] == 'ok'
assert not db.execute('PRAGMA foreign_key_check').fetchall()
assert db.execute('SELECT count(*) FROM dispatch_ready_tasks').fetchone()[0] == 0
assert db.execute("SELECT count(*) FROM features WHERE kind!='planning' AND status='done'").fetchone()[0] == 0
assert len(manifest['model_ledger']) == 40
assert len(manifest['api_services']) == 11
assert sum(len(service['methods']) for service in manifest['api_services']) == 37
after = {str(path.relative_to(ROOT)): sha(path) for path in paths}
assert before == after, 'Source changed during independent audit'
result = {'source_hashes':before, 'manifest_sha256':before['development/plan-manifest.json'],
          'attachment_sha256':sha(ATTACHMENT), 'source_fingerprint':manifest['source_fingerprint'],
          'mapping_rows':len(expected), 'mapping_subsystems':dict(collections.Counter(item['subsystem'] for item in mapping['items'])),
          'historical_devlogs_preserved':len(baseline['devlogs']), 'historical_attempts_preserved':len(baseline['attempts']),
          'historical_task_statuses_preserved':len(baseline['task_states']),
          'schema_unchanged':True, 'new_tasks':new_tasks,
          'counts':{table:db.execute('SELECT count(*) FROM '+table).fetchone()[0] for table in ['features','tasks','dependencies','attempts','devlog']},
          'rpc_services':11, 'rpc_methods':37, 'dispatch_ready':0, 'source_unchanged_during_checks':True,
          'excluded_pending_derived_artifacts':['planning/engine-overview.md; independently reviewed after its own source freeze'],
          'limits':['Planning and executable SQLite/tool boundaries only; no game application exists.', 'No Rust/native/WASM/browser/device/audio runtime acceptance performed.']}
(OUT/'audit.json').write_text(json.dumps(result, indent=2)+'\n')
print(json.dumps({key:result[key] for key in ['manifest_sha256','source_fingerprint','mapping_rows','counts','schema_unchanged','dispatch_ready']}))
