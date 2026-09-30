#!/usr/bin/env python3
"""Export portable operational history after writers stop; never changes source DB or queue."""
import argparse, datetime, hashlib, json, sqlite3
from pathlib import Path

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--database', type=Path, default=Path(__file__).with_name('workflow.sqlite3'))
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    connection = sqlite3.connect(args.database.resolve().as_uri() + '?mode=ro', uri=True)
    connection.row_factory = sqlite3.Row
    connection.execute('BEGIN')
    assert tuple(connection.execute('PRAGMA integrity_check').fetchone()) == ('ok',)
    assert not connection.execute('PRAGMA foreign_key_check').fetchall()
    data = {
        'format_version': 1,
        'exported_at': datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds'),
        'scope': 'Exact append-only attempts/devlog plus execution fields; source plan content is in canonical manifest. No binary DB/WAL/browser files.',
        'schema_version': connection.execute('PRAGMA user_version').fetchone()[0],
        'schema_sha256': hashlib.sha256(args.database.with_name('schema.sql').read_bytes()).hexdigest(),
        'features': [dict(row) for row in connection.execute('SELECT id,status,created_at,updated_at,source_fingerprint FROM features ORDER BY id')],
        'tasks': [dict(row) for row in connection.execute('SELECT id,feature_id,status,created_at,updated_at,blocking_reason,originating_task_id,source_fingerprint FROM tasks ORDER BY id')],
        'attempts': [dict(row) for row in connection.execute('SELECT * FROM attempts ORDER BY id')],
        'devlog': [dict(row) for row in connection.execute('SELECT * FROM devlog ORDER BY id')],
        'dependencies': [dict(row) for row in connection.execute('SELECT * FROM dependencies ORDER BY task_id,prerequisite_task_id')],
    }
    connection.close()
    data['counts'] = {key: len(data[key]) for key in ['features', 'tasks', 'dependencies', 'attempts', 'devlog']}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(data, indent=2, ensure_ascii=False) + '\n')
    print(json.dumps({'output': str(args.output), 'counts': data['counts'], 'queue_state_changed': False}))

if __name__ == '__main__':
    main()
