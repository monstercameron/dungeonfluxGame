from pathlib import Path
import hashlib
import json
import os
import shutil
import socket
import subprocess
import sys
import time

ROOT = Path('/Users/earlcameron/Desktop/dungeonflux')
FIXTURE = ROOT / 's'
OUTPUT = ROOT / 'development/evidence/postgres-g05/a3'
RUNTIME = FIXTURE / 'development/runtime/postgres-g05'
DATA = RUNTIME / 'data'
BIN = FIXTURE / 'artifacts/build/postgres-g05/install/bin'
SCRIPT = FIXTURE / 'development/postgres.sh'
SOCKET = RUNTIME / 'socket/.s.PGSQL.55439'
PIDFILE = DATA / 'postmaster.pid'
records = []

def save():
    (OUTPUT / 'boundary-results.json').write_text(json.dumps(records, indent=2) + '\n')

def run(label, argv, expected=0, timeout=65):
    argv = list(map(str, argv))
    started = time.monotonic()
    result = subprocess.run(argv, capture_output=True, text=True, timeout=timeout)
    records.append({'label': label, 'argv': argv, 'exit': result.returncode,
                    'stdout': result.stdout, 'stderr': result.stderr,
                    'seconds': round(time.monotonic() - started, 3)})
    save()
    print(label, result.returncode, flush=True)
    assert result.returncode == expected, records[-1]
    return result

def action(label, command, expected=0):
    return run(label, [SCRIPT, command], expected)

def sql(label, query):
    return run(label, [BIN / 'psql', '-XAtq', '-v', 'ON_ERROR_STOP=1', '-w',
                       '-h', RUNTIME / 'socket', '-p', '55439', '-d', 'postgres', '-c', query])

def write_pid(content):
    PIDFILE.write_bytes(content)
    PIDFILE.chmod(0o600)

def refuse(label, actions=('start', 'status', 'stop'), message=None):
    for command in actions:
        result = action(label + '-' + command, command, 1)
        if message:
            assert message in result.stderr, records[-1]

action('stopped-before', 'status')
action('start-native', 'start')
action('start-idempotent', 'start')
real_pidfile = PIDFILE.read_bytes()
sql('native-settings', "select version(),current_setting('data_directory'),current_setting('listen_addresses'),current_setting('max_connections'),current_setting('max_worker_processes'),current_setting('max_parallel_workers'),current_setting('max_parallel_workers_per_gather'),current_setting('autovacuum_max_workers'),current_setting('fsync'),current_setting('full_page_writes'),current_setting('synchronous_commit'),inet_server_addr() is null")
rollback = sql('rollback', "BEGIN; CREATE TABLE sol_a3_final_rollback(n int); INSERT INTO sol_a3_final_rollback VALUES (186); ROLLBACK; SELECT to_regclass('public.sol_a3_final_rollback') IS NULL;")
assert rollback.stdout.strip() == 't'
commit = sql('commit', 'BEGIN; CREATE TABLE sol_a3_final_committed(n int); INSERT INTO sol_a3_final_committed VALUES (186); COMMIT; SELECT n FROM sol_a3_final_committed;')
assert commit.stdout.strip() == '186'
identities = {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in [RUNTIME / 'owner.json', DATA / 'postgresql.conf']}
action('setup-preserves-existing', 'setup')
assert identities == {p: hashlib.sha256(Path(p).read_bytes()).hexdigest() for p in identities}
run('no-owned-tcp-listener', ['lsof', '-nP', '-a', '-p', real_pidfile.decode().splitlines()[0], '-iTCP', '-sTCP:LISTEN'], 1)
assert all((p.stat().st_mode & 0o777) == 0o700 for p in [RUNTIME, DATA, RUNTIME / 'socket'])
action('stop-native', 'stop')
action('restart', 'start')
assert sql('restart-persistence', 'SELECT n FROM sol_a3_final_committed;').stdout.strip() == '186'
action('stop-restart', 'stop')

suffix = RUNTIME / 'data-suffix-a3'
suffix_socket = RUNTIME / 's3'
suffix_socket.mkdir(mode=0o700, exist_ok=True)
assert suffix.exists() and not (suffix / 'postmaster.pid').exists()
records.append({'label': 'reuse-owned-synced-suffix', 'data': str(suffix), 'initdb_evidence': 'boundary-initial-results.json initdb-suffix-synced'})
save()
suffix_start = [BIN / 'pg_ctl', '-D', suffix, '-l', RUNTIME / 'suffix-a3.log', '-o', f"-c listen_addresses='' -c unix_socket_directories='{suffix_socket}' -c port=55440 -c unix_socket_permissions=0700", '-w', '-t', '10', 'start']
main_start = [BIN / 'pg_ctl', '-D', DATA, '-l', RUNTIME / 'server.log', '-w', '-t', '10', 'start']
time.sleep((int(time.time()) + 1.05) - time.time())
children = []
for label, argv in [('same-second-main-start', main_start), ('same-second-suffix-start', suffix_start)]:
    children.append((label, list(map(str, argv)), subprocess.Popen(list(map(str, argv)), stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)))
for label, argv, child in children:
    stdout, stderr = child.communicate(timeout=15)
    records.append({'label': label, 'argv': argv, 'exit': child.returncode, 'stdout': stdout, 'stderr': stderr})
    save()
    assert child.returncode == 0
main_bytes = PIDFILE.read_bytes()
suffix_bytes = (suffix / 'postmaster.pid').read_bytes()
main_lines = main_bytes.decode().splitlines()
suffix_lines = suffix_bytes.decode().splitlines()
assert main_lines[2] == suffix_lines[2], (main_lines, suffix_lines)
run('same-second-native-identities', ['ps', '-ww', '-p', main_lines[0] + ',' + suffix_lines[0], '-o', 'pid=,ppid=,lstart=,command='])
(OUTPUT / 'real-main-pidfile.txt').write_bytes(main_bytes)
(OUTPUT / 'real-suffix-pidfile.txt').write_bytes(suffix_bytes)
try:
    forged = list(main_lines)
    forged[0] = suffix_lines[0]
    write_pid(('\n'.join(forged) + '\n').encode())
    (OUTPUT / 'forged-main-pidfile.txt').write_bytes(PIDFILE.read_bytes())
    refuse('same-binary-prefix-forged-pid', message='PID command line does not name the owned data directory')
finally:
    write_pid(main_bytes)
run('main-still-ready-after-forged-stop', [BIN / 'pg_isready', '-h', RUNTIME / 'socket', '-p', '55439', '-t', '2'])
run('suffix-still-ready-after-forged-stop', [BIN / 'pg_isready', '-h', suffix_socket, '-p', '55440', '-t', '2'])

# Redirect the actual socket to another real answering server, while PID/argv remain valid.
suffix_node = suffix_socket / '.s.PGSQL.55440'
main_node_saved = RUNTIME / 'socket/main-a3-preserved'
SOCKET.rename(main_node_saved)
suffix_node.rename(SOCKET)
try:
    refuse('wrong-answering-server', message='SQL backend does not belong to the verified postmaster PID')
finally:
    SOCKET.rename(suffix_node)
    main_node_saved.rename(SOCKET)
action('main-ready-after-socket-restore', 'status')
run('suffix-ready-after-socket-restore', [BIN / 'pg_isready', '-h', suffix_socket, '-p', '55440', '-t', '2'])
action('stop-main-prefix-fixture', 'stop')
action('stopped-main-with-live-prefix', 'status')
action('start-main-with-live-prefix', 'start')
action('stop-main-with-live-prefix', 'stop')
run('stop-owned-suffix', [BIN / 'pg_ctl', '-D', suffix, '-m', 'fast', '-w', '-t', '10', 'stop'])

signal_record = OUTPUT / 'dummy-signals.txt'
dummy_code = "import json,os,pathlib,signal,time; p=pathlib.Path(" + repr(str(signal_record)) + "); signal.signal(signal.SIGINT,lambda s,f:p.write_text(str(s))); signal.signal(signal.SIGTERM,lambda s,f:p.write_text(str(s))); pathlib.Path(" + repr(str(OUTPUT / 'dummy-ready.json')) + ").write_text(json.dumps({'pid':os.getpid()})); time.sleep(240)"
dummy = subprocess.Popen([sys.executable, '-c', dummy_code])
while not (OUTPUT / 'dummy-ready.json').exists():
    assert dummy.poll() is None
    time.sleep(.01)
fake = lambda pid: f'{pid}\n{DATA}\n{int(time.time())}\n55439\n{RUNTIME}/socket\n\n0\nready\n'.encode()
try:
    for label, content in [('malformed', b'x\n'), ('dead', fake(99999999)), ('foreign-executable', fake(dummy.pid)), ('stale-real-server', real_pidfile)]:
        write_pid(content)
        try:
            refuse(label)
        finally:
            PIDFILE.unlink()
    inode = socket.socket(socket.AF_UNIX)
    inode.bind(str(SOCKET))
    inode.close()
    write_pid(fake(dummy.pid))
    try:
        refuse('stale-socket-foreign-pid')
    finally:
        PIDFILE.unlink()
    refuse('stale-socket-no-pid')
    SOCKET.unlink()
    assert dummy.poll() is None and not signal_record.exists()
    records.append({'label': 'foreign-dummy-never-signaled', 'pid': dummy.pid, 'alive': True, 'signal_record_exists': False})
    save()
finally:
    # SIGKILL is only worker cleanup of this explicitly created signal catcher.
    dummy.kill()
    dummy.wait(timeout=5)

action('start-orphan-fixture', 'start')
orphan_bytes = PIDFILE.read_bytes()
PIDFILE.unlink()
try:
    refuse('live-server-no-pid')
finally:
    write_pid(orphan_bytes)
action('ready-after-orphan-restore', 'status')
action('stop-orphan-fixture', 'stop')

guarded = [FIXTURE / 'development/runtime', RUNTIME, DATA, RUNTIME / 'socket', RUNTIME / 'server.log', RUNTIME / 'owner.json', DATA / 'PG_VERSION', DATA / 'postmaster.pid', DATA / 'postmaster.opts', DATA / 'postgresql.conf', DATA / 'postgresql.auto.conf', DATA / 'pg_hba.conf', DATA / 'pg_ident.conf', DATA / 'pg_wal', SOCKET, Path(str(SOCKET) + '.lock'), BIN]
guarded += [BIN / tool for tool in ['postgres', 'pg_ctl', 'pg_isready', 'psql', 'initdb']]
sentinel = OUTPUT / 'symlink-target.log'
sentinel.write_bytes(b'SOL_A3_SENTINEL\n')
for index, path in enumerate(guarded):
    present = path.exists()
    saved = path.with_name(path.name + '-a3-preserved')
    if present:
        path.rename(saved)
    path.symlink_to(saved if present else sentinel, target_is_directory=present and saved.is_dir())
    try:
        refuse(f'symlink-{index}-{path.name}', ('setup', 'start', 'status', 'stop'), message='refusing symlink component')
        assert sentinel.read_bytes() == b'SOL_A3_SENTINEL\n'
        assert not PIDFILE.exists() if path not in [PIDFILE, DATA, RUNTIME, FIXTURE / 'development/runtime'] else True
    finally:
        path.unlink()
        if present:
            saved.rename(path)
records.append({'label': 'all-symlink-targets-preserved', 'path_count': len(guarded), 'actions': 4, 'sentinel_sha256': hashlib.sha256(sentinel.read_bytes()).hexdigest()})
save()

action('start-timeout-fixture', 'start')
for tool in ['pg_isready', 'psql']:
    path = BIN / tool
    saved = path.with_name(tool + '-a3-preserved')
    path.rename(saved)
    receipt = OUTPUT / (tool + '-stalled-child.json')
    path.write_text('#!/usr/bin/env python3\nimport json,os,pathlib,subprocess,time\np=subprocess.Popen(["/bin/sleep","120"])\npathlib.Path(' + repr(str(receipt)) + ').write_text(json.dumps({"parent":os.getpid(),"child":p.pid}))\ntime.sleep(120)\n')
    path.chmod(0o755)
    try:
        result = action('stalled-' + tool + '-status', 'status', 1)
        assert records[-1]['seconds'] < 12
    finally:
        path.unlink()
        saved.rename(path)
    owned = json.loads(receipt.read_text())
    run('stalled-' + tool + '-children-gone', ['ps', '-ww', '-p', ','.join(map(str, owned.values())), '-o', 'pid=,stat=,command='], 1)
    action('ready-after-' + tool + '-timeout', 'status')
action('final-stop', 'stop')
action('final-stopped-status', 'status')
assert not PIDFILE.exists() and not SOCKET.exists() and not (suffix / 'postmaster.pid').exists()
(OUTPUT / 'evaluation-server.log').write_bytes((RUNTIME / 'server.log').read_bytes())
(OUTPUT / 'suffix-server.log').write_bytes((RUNTIME / 'suffix-a3.log').read_bytes())
(OUTPUT / 'fixture-owner.json').write_text(json.dumps({'owner': '/root/postgres_g05_sol', 'task_id': 'SETUP-G05-001', 'attempt_id': 'SETUP-G05-001-a3', 'root': str(FIXTURE), 'main_data': str(DATA), 'suffix_data': str(suffix), 'main_socket': str(RUNTIME / 'socket'), 'suffix_socket': str(suffix_socket), 'native_tools': str(BIN), 'archive': str(FIXTURE / 'artifacts/cache/postgres-g05/postgresql-18.6.tar.gz'), 'scratch': str(FIXTURE / 'artifacts/tmp/SETUP-G05-001-a3'), 'state': 'both native clusters stopped, durable data retained, all symlink/wrapper fixtures restored'}, indent=2) + '\n')
save()
print('ALL BOUNDARIES PASSED', len(records), flush=True)
