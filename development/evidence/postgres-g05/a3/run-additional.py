from pathlib import Path
import hashlib
import json
import os
import subprocess
import time

ROOT = Path('/Users/earlcameron/Desktop/dungeonflux')
FIXTURE = ROOT / 's'
OUTPUT = ROOT / 'development/evidence/postgres-g05/a3'
RUNTIME = FIXTURE / 'development/runtime/postgres-g05'
DATA = RUNTIME / 'data'
BIN = FIXTURE / 'artifacts/build/postgres-g05/install/bin'
SCRIPT = FIXTURE / 'development/postgres.sh'
records = []

def run(label, argv, expected=0):
    argv = list(map(str, argv))
    start = time.monotonic()
    result = subprocess.run(argv, capture_output=True, text=True, timeout=65)
    records.append({'label': label, 'argv': argv, 'exit': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr, 'seconds': time.monotonic() - start})
    (OUTPUT / 'additional-results.json').write_text(json.dumps(records, indent=2) + '\n')
    print(label, result.returncode, flush=True)
    assert result.returncode == expected, records[-1]
    return result

owner_path = RUNTIME / 'owner.json'
owner_original = owner_path.read_bytes()
owner = json.loads(owner_original)
owner['uid'] += 1
owner_path.write_text(json.dumps(owner))
try:
    for action in ['setup', 'start', 'status', 'stop']:
        run('foreign-owner-' + action, [SCRIPT, action], 1)
finally:
    owner_path.write_bytes(owner_original)
DATA.chmod(0o755)
try:
    for action in ['setup', 'start', 'status', 'stop']:
        run('unsafe-data-permissions-' + action, [SCRIPT, action], 1)
finally:
    DATA.chmod(0o700)

run('live-log-guard-start', [SCRIPT, 'start'])
main_pid = (DATA / 'postmaster.pid').read_text().splitlines()[0]
log = RUNTIME / 'server.log'
saved_log = RUNTIME / 'server-log-a3-live-preserved'
target = OUTPUT / 'live-symlink-target.log'
target.write_bytes(b'SOL_A3_LIVE_SENTINEL\n')
log.rename(saved_log)
log.symlink_to(target)
try:
    for action in ['setup', 'start', 'status', 'stop']:
        run('live-log-symlink-' + action, [SCRIPT, action], 1)
    assert target.read_bytes() == b'SOL_A3_LIVE_SENTINEL\n'
    run('main-still-ready-after-live-log-stop-refusal', [BIN / 'pg_isready', '-h', RUNTIME / 'socket', '-p', '55439', '-t', '2'])
finally:
    log.unlink()
    saved_log.rename(log)

suffix = RUNTIME / 'data-suffix-a3'
suffix_socket = RUNTIME / 's3'
run('pid-file-race-suffix-start', [BIN / 'pg_ctl', '-D', suffix, '-l', RUNTIME / 'suffix-a3.log', '-o', f"-c listen_addresses='' -c unix_socket_directories='{suffix_socket}' -c port=55440 -c unix_socket_permissions=0700", '-w', '-t', '10', 'start'])
suffix_pid = (suffix / 'postmaster.pid').read_text().splitlines()[0]
pg_ctl = BIN / 'pg_ctl'
saved_pg_ctl = BIN / 'pg_ctl-a3-race-preserved'
pg_ctl.rename(saved_pg_ctl)
receipt = OUTPUT / 'pid-file-race.json'
pg_ctl.write_text('#!/usr/bin/env python3\nimport json,os,pathlib,sys\np=pathlib.Path(' + repr(str(DATA / 'postmaster.pid')) + ')\nlines=p.read_text().splitlines();before=lines[0];lines[0]=' + repr(suffix_pid) + ';p.write_text("\\n".join(lines)+"\\n");p.chmod(0o600)\npathlib.Path(' + repr(str(receipt)) + ').write_text(json.dumps({"argv":sys.argv,"verified_pid":before,"replaced_pid":lines[0]}))\nos.execv(' + repr(str(saved_pg_ctl)) + ',[' + repr(str(saved_pg_ctl)) + ']+sys.argv[1:])\n')
pg_ctl.chmod(0o755)
try:
    run('stop-pins-verified-pid-despite-file-replacement', [SCRIPT, 'stop'])
finally:
    pg_ctl.unlink()
    saved_pg_ctl.rename(pg_ctl)
race = json.loads(receipt.read_text())
assert race['verified_pid'] == main_pid and race['replaced_pid'] == suffix_pid
assert race['argv'][1:] == ['kill', 'INT', main_pid]
run('verified-main-pid-gone-after-pinned-stop', ['ps', '-p', main_pid, '-o', 'pid=,command='], 1)
run('suffix-still-ready-after-pid-file-race', [BIN / 'pg_isready', '-h', suffix_socket, '-p', '55440', '-t', '2'])
run('stop-owned-race-suffix', [BIN / 'pg_ctl', '-D', suffix, '-m', 'fast', '-w', '-t', '10', 'stop'])
run('final-stopped', [SCRIPT, 'status'])
(OUTPUT / 'evaluation-server.log').write_bytes(log.read_bytes())
(OUTPUT / 'suffix-server.log').write_bytes((RUNTIME / 'suffix-a3.log').read_bytes())
print('ALL ADDITIONAL CHECKS PASSED', len(records), flush=True)
