import concurrent.futures
import copy
import importlib.util
import hashlib
import json
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile
from datetime import datetime, timedelta, timezone

HELPER = Path('/Users/earlcameron/.codex/skills/dungeonflux-quality-review/scripts/quality_cache.py')
SCHEMA_PATH = Path('/Users/earlcameron/Documents/Codex/2026-09-29/cr/dungeonflux/development/schema.sql')
SCHEMA_BYTES = SCHEMA_PATH.read_bytes()
spec = importlib.util.spec_from_file_location('quality', HELPER)
q = importlib.util.module_from_spec(spec)
spec.loader.exec_module(q)
results = []

def cli(project, command='scan'):
    return json.loads(subprocess.check_output([sys.executable, str(HELPER), command,
                      '--project', str(project)], text=True))

def rejects(call):
    try:
        call()
    except ValueError:
        return
    raise AssertionError('Expected rejection')


def complete_fixture_task(workflow, task_id):
    """Exercise actual schema lifecycle in isolated synthetic fixtures only."""
    acceptance, brief_json = workflow.execute('SELECT acceptance_json,brief_json FROM tasks WHERE id=?',
                                              (task_id,)).fetchone()
    brief = json.loads(brief_json)
    brief['dispatch_ready'] = True
    brief_json = json.dumps(brief)
    workflow.execute('UPDATE tasks SET brief_json=? WHERE id=?', (brief_json, task_id))
    attempt_id = 'fixture-attempt-' + task_id
    expires = (datetime.now(timezone.utc) + timedelta(minutes=15)).isoformat()
    workflow.execute('''INSERT INTO attempts(id,task_id,worker,model,role,status,phase,
      lease_owner,lease_token,lease_generation,brief_revision,brief_json,lease_expires_at,
      source_revision,evidence_json,defects_json,started_at)
      VALUES(?,?,?,?,?,'active','implementation',?,?,1,?,?,?,?,?,? ,?)''',
      (attempt_id, task_id, 'fixture-worker', 'fixture-worker-model', 'worker',
       'fixture-worker', 'fixture-token-' + task_id, 'fixture-brief-1', brief_json,
       expires, 'fixture-source-1', '[]', '[]', q.utc()))
    workflow.execute('UPDATE tasks SET status="running",active_attempt_id=?,lease_generation=1 WHERE id=?',
                     (attempt_id, task_id))
    workflow.execute('UPDATE attempts SET status="submitted",phase="review",submitted_commit=? WHERE id=?',
                     ('fixture-source-1', attempt_id))
    workflow.execute('UPDATE tasks SET status="review" WHERE id=?', (task_id,))
    evidence = [{'criterion': criterion, 'result': 'pass',
                 'evidence_ref': 'fixture://isolated-lifecycle/criterion'}
                for criterion in json.loads(acceptance)]
    workflow.execute('''UPDATE attempts SET status='approved',verdict='approve',
      evaluator_id=?,evaluator_model=?,tested_revision=?,capabilities_json=?,review_evidence_json=?
      WHERE id=?''', ('fixture-independent-evaluator', 'fixture-frontier-model',
       'fixture-source-1', '{"frontier":true}', json.dumps(evidence), attempt_id))
    workflow.execute('UPDATE attempts SET phase="integration" WHERE id=?', (attempt_id,))
    workflow.execute('''UPDATE attempts SET status='integrated',phase='terminal',
      integrated_revision=?,integrated_commit=?,integration_evidence_json=?,finished_at=? WHERE id=?''',
      ('fixture-source-1', 'fixture-source-1', '[{"result":"pass","evidence_ref":"fixture://integration"}]',
       q.utc(), attempt_id))
    workflow.execute('UPDATE tasks SET status="done" WHERE id=?', (task_id,))
    workflow.commit()

with tempfile.TemporaryDirectory(prefix='quality-fixture-') as temporary:
    project = Path(temporary).resolve()
    (project / 'AGENTS.md').write_text('Fixture guidance')
    (project / 'planning').mkdir()
    (project / 'planning/coding-style.md').write_text('Explicit errors required')
    (project / 'src').mkdir()
    code = project / 'src/main.rs'
    code.write_text('fn main() { let _n: u32 = "x".parse().unwrap(); }')
    for directory in ['artifacts', 'target', 'generated', 'dungeonflux.old']:
        (project / directory).mkdir()
        (project / directory / 'ignored.rs').write_text('bad ignored code')
    (project / 'src/generated.rs').write_text('// @generated\nfn generated() {}')
    (project / 'src/symlink.rs').symlink_to(code)
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as executor:
        scans = list(executor.map(lambda _: cli(project), range(2)))
    candidates = [c for r in scans for c in r['candidates']]
    assert len(candidates) == 1 and candidates[0]['path'] == 'src/main.rs'
    assert all(r['authored_rust_files'] == 1 for r in scans)
    results.append('Two scanner processes claim one authored file once; ignored/generated/symlink files excluded')
    claim = candidates[0]
    report = {'outcome': 'reviewed', 'summary': 'Actual fixture source inspection',
              'checks': {c: {'status': 'checked', 'evidence': 'Fixture review evidence'} for c in q.CATEGORIES},
              'findings': [{'key': 'parse-input-panic', 'category': 'bugs', 'severity': 'high',
                 'title': 'Parse errors panic', 'evidence': 'src/main.rs:1 parses x as u32 then unwraps its Err',
                 'acceptance': 'Invalid input returns a typed error without panicking',
                 'design_refs': ['planning/coding-style.md']}]}
    db = q.connect(project)
    saved = q.finish(db, project, claim['path'], claim['token'], report)
    fid = saved['finding_ids'][0]
    assert cli(project)['candidates'] == []
    row = db.execute('SELECT * FROM file_reviews').fetchone()
    assert row['reviewed_md5'] == q.md5(code) and row['last_reviewed_at']
    results.append('Successful review caches actual MD5/date; unchanged source skipped')
    code.write_text(code.read_text() + '\n// revision 2')
    claim = cli(project)['candidates'][0]
    code.write_text(code.read_text() + '\n// changed while reviewing')
    rejects(lambda: q.finish(db, project, claim['path'], claim['token'], report))
    claim = cli(project)['candidates'][0]
    (project / 'planning/coding-style.md').write_text('Policy changed')
    rejects(lambda: q.finish(db, project, claim['path'], claim['token'], report))
    results.append('Changed bytes and changed design policy refuse stale review completion')
    claim = cli(project)['candidates'][0]
    q.finish(db, project, claim['path'], claim['token'], {'outcome': 'inconclusive', 'summary': 'Missing required test evidence'})
    assert db.execute('SELECT reviewed_md5 FROM file_reviews').fetchone()[0] != q.md5(code)
    assert cli(project)['candidates'] == []
    db.execute('UPDATE file_reviews SET retry_after=0')
    db.commit()
    claim = cli(project)['candidates'][0]
    q.finish(db, project, claim['path'], claim['token'], {'outcome': 'failed', 'summary': 'Review tool unavailable'})
    db.execute('UPDATE file_reviews SET retry_after=0')
    db.commit()
    assert cli(project)['candidates'] == []
    results.append('Failed/inconclusive reviews preserve prior successful hash; retries back off and stop after two attempts')
    code.write_text(code.read_text() + '\n// new bytes permit retry')
    claim = cli(project)['candidates'][0]
    q.finish(db, project, claim['path'], claim['token'], report)
    assert db.execute('SELECT count(*) FROM findings').fetchone()[0] == 1
    results.append('Same defect across source revisions keeps one finding identity')
    workflow = project / 'development/workflow.sqlite3'
    wf = sqlite3.connect(workflow)
    wf.executescript(SCHEMA_BYTES.decode())
    wf.execute('INSERT INTO features VALUES(?,?,?,?,?,?,?,?,?,?,?)',
               ('X08', 'crosscutting', 'Quality', 'Quality review', '["reviewed"]', 50,
                'planned', '{}', 'fixture', q.utc(), q.utc()))
    wf.commit()
    first = q.intake(db, project, fid, 'fixture-coordinator')
    second = q.intake(db, project, fid, 'fixture-coordinator')
    assert first['task_id'] == second['task_id'] and first['devlog_id'] == second['devlog_id']
    assert second['acknowledged_existing']
    assert wf.execute('SELECT count(*) FROM tasks').fetchone()[0] == 1
    assert wf.execute('SELECT count(*) FROM devlog').fetchone()[0] == 1
    assert wf.execute('SELECT count(*) FROM dispatch_ready_tasks').fetchone()[0] == 0
    results.append('Real five-table schema intake twice produces one pending task, one devlog, no dispatch-ready task')
    # Isolated recurrence setup follows every actual phase/fence/evidence gate.
    complete_fixture_task(wf, first['task_id'])
    code.write_text(code.read_text() + '\n// defect recurrence on new revision')
    claim = cli(project)['candidates'][0]
    q.finish(db, project, claim['path'], claim['token'], report)
    assert db.execute('SELECT status FROM findings').fetchone()[0] == 'pending'
    recurrence = q.intake(db, project, fid, 'fixture-coordinator')
    assert recurrence['task_id'] != first['task_id']
    assert q.intake(db, project, fid, 'fixture-coordinator')['task_id'] == recurrence['task_id']
    assert wf.execute('SELECT count(*) FROM tasks').fetchone()[0] == 2
    assert wf.execute('SELECT originating_task_id FROM tasks WHERE id=?', (recurrence['task_id'],)).fetchone()[0] == first['task_id']
    results.append('New defective revision after completed work creates exactly one linked regression task')
    # A relevant dependency change requeues only files that recorded it.
    dependency = project / 'src/dependency.rs'
    dependency.write_text('pub fn dependency() {}')
    claims = cli(project)['candidates']
    assert len(claims) == 1 and claims[0]['path'] == 'src/dependency.rs'
    clean = copy.deepcopy(report)
    clean['findings'] = []
    q.finish(db, project, claims[0]['path'], claims[0]['token'], clean)
    code.write_text(code.read_text() + '\n// dependency tracking')
    claim = cli(project)['candidates'][0]
    tracked = copy.deepcopy(report)
    tracked['context_paths'] = {'src/dependency.rs': q.md5(dependency)}
    q.finish(db, project, claim['path'], claim['token'], tracked)
    dependency.write_text(dependency.read_text() + '\n// changed contract')
    assert {c['path'] for c in cli(project)['candidates']} == {'src/main.rs', 'src/dependency.rs'}
    results.append('Relevant tracked dependency changes invalidate dependent review; unrelated new file did not')
    code.rename(project / 'src/renamed.rs')
    cli(project)
    assert db.execute('SELECT deleted FROM file_reviews WHERE path="src/main.rs"').fetchone()[0] == 1
    assert db.execute('SELECT count(*) FROM review_history').fetchone()[0] >= 6
    assert db.execute('PRAGMA integrity_check').fetchone()[0] == 'ok'
    assert wf.execute('PRAGMA foreign_key_check').fetchall() == []
    results.append('Rename records old path deleted, retains review history; both databases pass integrity/FK checks')
    db.close()
    wf.close()

for case in ['source_changed', 'source_deleted', 'policy_changed', 'committed_ack_retry',
             'unrelated_research_cached', 'referenced_design_changed']:
    with tempfile.TemporaryDirectory(prefix='quality-intake-stale-') as temporary:
        project = Path(temporary).resolve()
        (project / 'AGENTS.md').write_text('Fixture guidance')
        (project / 'planning').mkdir()
        (project / 'planning/coding-style.md').write_text('Explicit errors required')
        (project / 'src').mkdir()
        code = project / 'src/main.rs'
        code.write_text('fn main() { let _n: u32 = "x".parse().unwrap(); }')
        design = project / 'planning/rules.md'
        design.write_text('Current governing rule')
        research = project / 'planning/pricing-research.md'
        research.write_text('Unrelated model prices')
        claim = cli(project)['candidates'][0]
        db = q.connect(project)
        local_report = copy.deepcopy(report)
        local_report['context_paths'] = {'planning/rules.md': q.md5(design)}
        local_report['findings'][0]['design_refs'].append('planning/rules.md')
        fid = q.finish(db, project, claim['path'], claim['token'], local_report)['finding_ids'][0]
        wf = sqlite3.connect(project / 'development/workflow.sqlite3')
        wf.executescript(SCHEMA_BYTES.decode())
        wf.execute('INSERT INTO features VALUES(?,?,?,?,?,?,?,?,?,?,?)',
                   ('X08', 'crosscutting', 'Quality', 'Quality review', '["reviewed"]', 50,
                    'planned', '{}', 'fixture', q.utc(), q.utc()))
        wf.commit()
        if case == 'unrelated_research_cached':
            research.write_text('Changed unrelated research')
            assert cli(project)['candidates'] == []
            q.intake(db, project, fid, 'fixture-coordinator')
            assert wf.execute('SELECT count(*) FROM tasks').fetchone()[0] == 1
        elif case == 'committed_ack_retry':
            first = q.intake(db, project, fid, 'fixture-coordinator')
            historical_task = wf.execute('SELECT * FROM tasks').fetchall()
            historical_log = wf.execute('SELECT * FROM devlog').fetchall()
            db.execute('UPDATE findings SET status="pending",workflow_task_id=NULL')
            db.commit()
            code.write_text('fn main() {} // fixed after durable workflow commit')
            recovered = q.intake(db, project, fid, 'fixture-coordinator')
            assert recovered['acknowledged_existing'] and recovered['task_id'] == first['task_id']
            assert recovered['requires_current_triage']
            assert wf.execute('SELECT * FROM tasks').fetchall() == historical_task
            assert wf.execute('SELECT * FROM devlog').fetchall() == historical_log
        else:
            if case == 'source_changed':
                code.write_text('fn main() {} // fixed before intake')
            elif case == 'source_deleted':
                code.unlink()
            elif case == 'referenced_design_changed':
                design.write_text('Changed governing rule')
            else:
                (project / 'planning/coding-style.md').write_text('New design policy')
            rejects(lambda: q.intake(db, project, fid, 'fixture-coordinator'))
            assert wf.execute('SELECT count(*) FROM tasks').fetchone()[0] == 0
            assert wf.execute('SELECT count(*) FROM devlog').fetchone()[0] == 0
            assert db.execute('SELECT status FROM findings').fetchone()[0] == 'stale'
            assert db.execute('SELECT count(*) FROM review_history').fetchone()[0] == 1
            if case == 'referenced_design_changed':
                assert [c['path'] for c in cli(project)['candidates']] == ['src/main.rs']
        db.close()
        wf.close()
        results.append('Stale intake guard: ' + case)

assert SCHEMA_PATH.read_bytes() == SCHEMA_BYTES, 'Schema changed during compatibility run'
print(json.dumps({'passed': len(results), 'schema_path': str(SCHEMA_PATH),
                  'schema_sha256': hashlib.sha256(SCHEMA_BYTES).hexdigest(),
                  'checks': results}, indent=2))
