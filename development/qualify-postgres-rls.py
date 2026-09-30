#!/usr/bin/env python3
"""Finite synthetic native SQL qualification. Never adopts or deletes a cluster."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import selectors
import signal
import stat
import subprocess
import sys
import time


MAIN = Path('/Users/earlcameron/Desktop/dungeonflux')
RUNTIME_ROOT = MAIN / 'development/runtime/rls-g05'
BIN = MAIN / 'artifacts/build/postgres-g05/install/bin'
BINARY_HASHES = {
    'postgres': 'fdb263f367f30e248021c9f74441c9120c2091aceaed0957bdb7a13efca7f8d9',
    'initdb': 'b9dc5c551ca309f8d2500e78d815e1aae3f7745c92d22c180345cd7283b0f0f9',
    'psql': '572ac7789c613a4c066cdb4deaf1b4d5c18d1975000cb097c1ce764f828432a7',
    'pg_ctl': '401d19cff0e8c98a76bb0965cb7ca0c7a66c27b3480796fcd03ddbfe4b24e0b4',
    'pg_isready': '1d851244735169d4d0ceef5d472af935fac397bfb6cc90297b8391e5b623b994',
}
EXPECTED_STATES = {
    'failed_statement': '22012', 'aborted_transaction': '25P02',
    'unique_own': '23505', 'composite_fk_missing': '23503',
    'hidden_unique_channel': '23505',
    **{name: '42501' for name in (
        'foreign_insert', 'tenant_move', 'foreign_child', 'disable_rls', 'unforce_rls',
        'drop_policy', 'assume_owner', 'assume_admin', 'grant_bypass', 'create_admin',
        'truncate', 'references', 'row_security_off')},
}
PROTECTED_CORE = {18905, 18907, 25530, 80873}
MAX_OUTPUT = 1048576


class QualificationError(Exception):
    pass


def require(condition, message):
    if not condition:
        raise QualificationError(message)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def no_symlinks(path):
    require(path.is_absolute(), 'absolute path required')
    for component in [*reversed(path.parents), path]:
        if component.is_symlink():
            raise QualificationError('symlink refused: ' + str(component))


def owned_directory(path):
    no_symlinks(path)
    info = path.stat()
    require(stat.S_ISDIR(info.st_mode) and info.st_uid == os.getuid(),
            'foreign directory ownership: ' + str(path))


def protected_identity(pids):
    require(re.fullmatch(r'[1-9][0-9]*(,[1-9][0-9]*){4,7}', pids) is not None,
            'five to eight explicit protected process IDs required')
    ids = [int(value) for value in pids.split(',')]
    require(len(ids) == len(set(ids)) and PROTECTED_CORE.issubset(ids)
            and all(value < 2147483648 for value in ids), 'protected core process IDs required')
    result = subprocess.run(['/bin/ps', '-p', pids, '-o', 'pid=,ppid=,lstart=,args='],
                            capture_output=True, text=True, timeout=5, check=True)
    require(len(result.stdout.splitlines()) == len(ids), 'protected process missing')
    return result.stdout


class Fixture:
    def __init__(self, fixture_id, runtime_root):
        require(runtime_root == RUNTIME_ROOT, 'foreign runtime root refused')
        require(re.fullmatch(r'(worker|reviewer|integration)-a1-[a-z0-9][a-z0-9-]{0,31}',
                             fixture_id) is not None, 'invalid fixture ID')
        no_symlinks(runtime_root)
        owned_directory(runtime_root.parent)
        if not runtime_root.exists():
            runtime_root.mkdir(mode=0o700)
        owned_directory(runtime_root)
        require(stat.S_IMODE(runtime_root.stat().st_mode) == 0o700, 'root must be private0700')
        self.root = runtime_root / fixture_id
        no_symlinks(self.root)
        require(not self.root.exists(), 'existing fixture refused; never reset or adopt data')
        require(len(os.fsencode(self.root / 's/.s.PGSQL.55441')) < 104,
                'fixture ID exceeds native Unix socket path bound')
        self.root.mkdir(mode=0o700)
        self.socket = self.root / 's'
        self.socket.mkdir(mode=0o700)
        self.data = self.root / 'data'
        self.env = {'PATH': '/usr/bin:/bin', 'HOME': str(self.root), 'LC_ALL': 'C',
                    'PGCONNECT_TIMEOUT': '2', 'PGPASSFILE': str(self.root / 'no-password'),
                    'PGSYSCONFDIR': str(self.root), 'TMPDIR': str(self.root),
                    'PGOPTIONS': '-c statement_timeout=2000 -c lock_timeout=500'}
        self.started = time.monotonic()
        self.server = None
        self.sessions = []
        self.operations = []
        self.records = []
        self.states = {}
        self.resource_samples = []
        self.serial = 0
        self.result = {'fixture_root': str(self.root), 'scope': 'finite native SQL only',
                       'full_g05': 'pending', 'passed': False}

    def remaining(self):
        value = 120 - (time.monotonic() - self.started)
        require(value > 0, 'full fixture120second deadline exceeded')
        if self.server is not None:
            require((self.root / 'server.log').stat().st_size <= MAX_OUTPUT, 'server log1MiB bound')
        return min(30, value)

    def sample(self):
        output = subprocess.run(['/bin/ps', '-axo', 'pid=,ppid=,rss='],
                                capture_output=True, text=True, timeout=5, check=True).stdout
        rows = [tuple(map(int, line.split())) for line in output.splitlines()]
        ids = {os.getpid()}
        changed = True
        while changed:
            additions = {pid for pid, parent, _ in rows if parent in ids}
            changed = not additions.issubset(ids)
            ids.update(additions)
        sample = {'elapsed_seconds': round(time.monotonic() - self.started, 3),
                  'rss_kib': sum(rss for pid, _, rss in rows if pid in ids),
                  'processes': [{'pid': pid, 'ppid': parent, 'rss_kib': rss}
                                for pid, parent, rss in rows if pid in ids]}
        self.resource_samples.append(sample)

    def stop_child(self, child):
        # Only direct Popen children in a session created here may be signaled.
        if child.poll() is None:
            os.killpg(child.pid, signal.SIGTERM)
            try:
                child.wait(timeout=1)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait(timeout=1)

    def command(self, name, argv, sql=None, allow_failure=False):
        self.serial += 1
        prefix = self.root / ('%02d-%s' % (self.serial, name))
        if sql is not None:
            require(len(sql.encode()) <= 65536, 'SQL input exceeds64KiB')
            prefix.with_suffix('.sql').write_text(sql)
        deadline = time.monotonic() + self.remaining()
        input_path = prefix.with_suffix('.sql') if sql is not None else Path('/dev/null')
        with prefix.with_suffix('.stdout').open('xb') as stdout, \
                prefix.with_suffix('.stderr').open('xb') as stderr, input_path.open('rb') as stdin:
            child = subprocess.Popen(argv, stdin=stdin,
                                     stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=self.env,
                                     start_new_session=True)
            selector = selectors.DefaultSelector()
            selector.register(child.stdout, selectors.EVENT_READ, stdout)
            selector.register(child.stderr, selectors.EVENT_READ, stderr)
            total = 0
            sampled_at = 0
            try:
                while selector.get_map():
                    self.remaining()
                    require(time.monotonic() < deadline, 'child30second deadline: ' + name)
                    if time.monotonic() - sampled_at >= 0.1:
                        self.sample()
                        sampled_at = time.monotonic()
                    for key, _ in selector.select(timeout=0.05):
                        chunk = os.read(key.fileobj.fileno(), 8192)
                        if not chunk:
                            selector.unregister(key.fileobj)
                            continue
                        available = MAX_OUTPUT - total
                        key.data.write(chunk[:available])
                        total += len(chunk)
                        require(total <= MAX_OUTPUT, 'output bound: ' + name)
                child.wait(timeout=max(0.01, deadline - time.monotonic()))
                self.sample()
            finally:
                self.stop_child(child)
                selector.close()
                child.stdout.close()
                child.stderr.close()
                self.operations.append({'name': name, 'argv': argv, 'pid': child.pid,
                                        'exit_code': child.returncode,
                                        'stdout': str(prefix.with_suffix('.stdout')),
                                        'stderr': str(prefix.with_suffix('.stderr'))})
        out = prefix.with_suffix('.stdout').read_text()
        err = prefix.with_suffix('.stderr').read_text()
        require(len(out.encode()) + len(err.encode()) <= MAX_OUTPUT, 'output bound: ' + name)
        require(allow_failure or child.returncode == 0, name + ' failed: ' + err[-2000:])
        return child.returncode, out

    def psql_args(self, role):
        return [str(BIN / 'psql'), '-X', '-w', '-qAt', '-v', 'ON_ERROR_STOP=1', '-v', 'VERBOSITY=verbose',
                '-h', str(self.socket), '-p', '55441', '-U', role, '-d', 'postgres']

    def sql(self, name, sql, role='fixture_admin', allow_failure=False):
        code, output = self.command(name, self.psql_args(role), sql, allow_failure)
        if code == 0:
            self.observe(output)
        return code, output

    def observe(self, output):
        lines = output.splitlines()
        require(len(lines) <= 100, 'query row bound exceeded')
        for line in lines:
            if line.startswith('{'):
                record = json.loads(line)
                require(record.get('pass') is True, 'SQL assertion failed: ' + line)
                self.records.append(record)
            elif line.startswith('EXPECTED '):
                _, name, state = line.split()
                require(EXPECTED_STATES.get(name) == state, 'unexpected SQLSTATE: ' + line)
                self.states[name] = state
            elif line.strip():
                raise QualificationError('unexpected SQL output: ' + line)

    def start(self):
        binaries = {}
        for name, expected in BINARY_HASHES.items():
            path = BIN / name
            no_symlinks(path)
            require(path.stat().st_uid == os.getuid() and digest(path) == expected,
                    'approved binary mismatch: ' + name)
            binaries[name] = {'path': str(path), 'sha256': expected}
        self.result['binaries'] = binaries
        self.command('initdb', [str(BIN / 'initdb'), '-D', str(self.data), '-U', 'fixture_admin',
                               '--auth-local=trust', '--auth-host=reject', '--no-locale', '-E', 'UTF8'])
        config = """listen_addresses = ''
unix_socket_permissions = 0700
port = 55441
max_connections = 4
reserved_connections = 0
superuser_reserved_connections = 0
shared_buffers = '16MB'
fsync = on
full_page_writes = on
synchronous_commit = on
statement_timeout = '2s'
lock_timeout = '500ms'
max_worker_processes = 0
max_parallel_workers = 0
autovacuum = off
logging_collector = off
""" + "unix_socket_directories = '" + str(self.socket) + "'\n"
        (self.data / 'postgresql.conf').write_text(config)
        (self.root / 'configuration.txt').write_text(config)
        self.result['configuration_sha256'] = digest(self.root / 'configuration.txt')
        self.server_log = (self.root / 'server.log').open('xb')
        self.server = subprocess.Popen([str(BIN / 'postgres'), '-D', str(self.data)],
                                       stdin=subprocess.DEVNULL, stdout=self.server_log,
                                       stderr=subprocess.STDOUT, env=self.env, start_new_session=True)
        self.result['owned_postmaster_pid'] = self.server.pid
        owner = {'helper_pid': os.getpid(), 'postmaster_pid': self.server.pid,
                 'session_id': os.getsid(self.server.pid), 'data': str(self.data),
                 'created_unix_seconds': time.time(), 'binary_sha256': BINARY_HASHES['postgres']}
        (self.root / 'owner.json').write_text(json.dumps(owner, indent=2) + '\n')
        readiness_end = time.monotonic() + 10
        while True:
            require(self.server.poll() is None, 'owned server exited before readiness')
            require(time.monotonic() < readiness_end, 'readiness10second deadline')
            code, out = self.sql('readiness', "SELECT json_build_object('case','identity','pass',true," +
                                 "'data',current_setting('data_directory'),'backend',pg_backend_pid()," +
                                 "'version',current_setting('server_version'),'listen',current_setting('listen_addresses')," +
                                 "'connections',current_setting('max_connections'),'fsync',current_setting('fsync')," +
                                 "'full_page_writes',current_setting('full_page_writes')," +
                                 "'synchronous_commit',current_setting('synchronous_commit'));\n", allow_failure=True)
            if code == 0:
                identity = json.loads(out)
                require(identity['data'] == str(self.data) and identity['version'] == '18.6'
                        and identity['listen'] == '' and identity['connections'] == '4'
                        and all(identity[key] == 'on' for key in ('fsync','full_page_writes','synchronous_commit')),
                        'exact runtime/configuration identity mismatch')
                # Readiness connection must still exist while its OS parent is probed.
                session = Session(self, 'identity', 'fixture_admin')
                identity_out = session.exchange("SELECT pg_backend_pid();\n")
                backend = int(identity_out.strip())
                parent = subprocess.run(['/bin/ps', '-p', str(backend), '-o', 'ppid='],
                                        capture_output=True, text=True, timeout=5, check=True).stdout
                require(int(parent.strip()) == self.server.pid, 'backend parent not owned postmaster')
                self.result['readiness_identity'] = {**identity, 'live_backend': backend,
                                                     'backend_parent': self.server.pid}
                session.close()
                return

    def concurrency(self):
        controller = Session(self, 'controller', 'fixture_admin')
        a = Session(self, 'concurrent-a', 'fixture_runtime')
        b = Session(self, 'concurrent-b', 'fixture_runtime')
        controller.exchange('SELECT pg_advisory_lock(55441);\n')
        backend_ids = []
        for session, tenant in ((a, '1'), (b, '2')):
            pid = int(session.exchange("SET dungeonflux.fixture_tenant = ''; BEGIN;\n" +
                                      "SET LOCAL dungeonflux.fixture_tenant = '" + tenant * 32 + "';\n" +
                                      'SELECT pg_backend_pid();\n').strip())
            backend_ids.append(pid)
            session.send("WITH changed AS (UPDATE fixture.parent SET value = value + 1\n" +
                         "WHERE tenant = fixture.tenant() AND id = 2 RETURNING tenant, id, value),\n" +
                         "barrier AS MATERIALIZED (SELECT tenant,id,value,\n" +
                         "pg_advisory_xact_lock_shared(55441) FROM changed)\n" +
                         "SELECT json_build_object('case','concurrent_" + tenant + "','pass',\n" +
                         "count(*) = 1 AND bool_and(tenant = fixture.tenant()) AND max(value) = 1,\n" +
                         "'backend',pg_backend_pid(),'rows',count(*)) FROM barrier;\n")
        require(len(set(backend_ids)) == 2, 'concurrency requires distinct backends')
        barrier_end = time.monotonic() + 1.5
        while True:
            # Fixed numeric backend IDs came from these two directly opened sessions.
            state = controller.exchange('SELECT count(*) FROM pg_locks WHERE locktype = ' +
                                        "'advisory' AND NOT granted AND pid IN (" +
                                        ','.join(map(str, backend_ids)) + ');\n')
            if state.strip() == '2':
                break
            require(time.monotonic() < barrier_end, 'two-backend controlled barrier not reached')
        self.result['concurrent_barrier'] = {'backend_ids': backend_ids,
                                            'simultaneously_waiting_advisory_locks': 2}
        self.observe(controller.exchange("SELECT json_build_object('case','concurrent_uncommitted'," +
                    "'pass',(SELECT count(DISTINCT pid)=2 FROM pg_locks WHERE pid IN (" +
                    ','.join(map(str, backend_ids)) + ") AND relation='fixture.parent'::regclass " +
                    "AND mode='RowExclusiveLock' AND granted) AND " +
                    "(SELECT count(*)=2 AND bool_and(value=0) FROM fixture.parent WHERE id=2));\n"))
        self.sample()
        controller.exchange('SELECT pg_advisory_unlock(55441);\n')
        for session in (a, b):
            self.observe(session.receive())
            self.observe(session.exchange("COMMIT; SELECT json_build_object('case','concurrent_reset'," +
                                          "'pass',count(*)=0,'rows',count(*),'backend',pg_backend_pid()) " +
                                          'FROM fixture.parent;\n'))
            session.close()
        controller.close()

    def finish(self):
        for session in self.sessions:
            session.close()
        if self.server is not None:
            # Fast PostgreSQL shutdown of exactly the retained direct child; no pid file lookup.
            if self.server.poll() is None:
                os.kill(self.server.pid, signal.SIGINT)
                try:
                    self.server.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    self.stop_child(self.server)
            self.result['server_exit_code'] = self.server.returncode
            self.result['owned_server_reaped'] = self.server.poll() is not None
            self.server_log.close()
        self.result.update({'elapsed_seconds': round(time.monotonic() - self.started, 3),
                            'operations': self.operations, 'observations': self.records,
                            'sqlstates': self.states, 'resource_samples': self.resource_samples,
                            'sampled_peak_rss_kib': max((s['rss_kib'] for s in self.resource_samples), default=0)})
        self.result['retained_files'] = {
            str(path.relative_to(self.root)): digest(path)
            for path in self.root.iterdir() if path.is_file() and path.name != 'result.json'}
        self.result['runtime_identity_files'] = {
            name: digest(self.data / name) for name in
            ('PG_VERSION', 'postgresql.conf', 'postgresql.auto.conf', 'pg_hba.conf', 'pg_ident.conf')
            if (self.data / name).is_file()}
        (self.root / 'result.json').write_text(json.dumps(self.result, indent=2) + '\n')


class Session:
    def __init__(self, fixture, name, role):
        require(sum(s.child.poll() is None for s in fixture.sessions) < 3,
                'at most three fixture client sessions')
        self.fixture = fixture
        self.name = name
        self.role = role
        self.deadline = time.monotonic() + fixture.remaining()
        self.transcript = (fixture.root / (name + '.stdout')).open('xb')
        self.errors = (fixture.root / (name + '.stderr')).open('xb')
        self.inputs = (fixture.root / (name + '.sql')).open('x')
        self.child = subprocess.Popen(fixture.psql_args(role), stdin=subprocess.PIPE,
                                      stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                      env=fixture.env, start_new_session=True)
        self.selector = selectors.DefaultSelector()
        self.selector.register(self.child.stdout, selectors.EVENT_READ, self.transcript)
        self.selector.register(self.child.stderr, selectors.EVENT_READ, self.errors)
        self.buffer = b''
        self.total = 0
        self.sequence = 0
        self.pending = None
        fixture.sessions.append(self)

    def send(self, sql):
        require(self.pending is None, 'pending session command')
        require(len(sql.encode()) <= 65536, 'session SQL64KiB bound')
        self.sequence += 1
        self.pending = ('DONE_%s_%d' % (self.name, self.sequence)).encode()
        command = sql + '\\echo ' + self.pending.decode() + '\n'
        self.inputs.write(command)
        self.inputs.flush()
        self.child.stdin.write(command.encode())
        self.child.stdin.flush()

    def receive(self):
        require(self.pending is not None, 'no pending session command')
        result = []
        while True:
            while b'\n' in self.buffer:
                line, self.buffer = self.buffer.split(b'\n', 1)
                if line == self.pending:
                    self.pending = None
                    return '\n'.join(result) + '\n'
                result.append(line.decode())
                require(len(result) <= 100, 'session query100row bound')
            self.fixture.remaining()
            require(time.monotonic() < self.deadline, 'session30second deadline: ' + self.name)
            require(self.child.poll() is None, 'psql session exited: ' + self.name)
            events = self.selector.select(timeout=0.05)
            for key, _ in events:
                data = os.read(key.fileobj.fileno(), 8192)
                if not data:
                    self.selector.unregister(key.fileobj)
                    continue
                available = MAX_OUTPUT - self.total
                key.data.write(data[:available])
                key.data.flush()
                self.total += len(data)
                require(self.total <= MAX_OUTPUT, 'session1MiB output bound')
                if key.fileobj is self.child.stdout:
                    self.buffer += data

    def exchange(self, sql):
        self.send(sql)
        return self.receive()

    def close(self):
        if self.inputs.closed:
            return
        if self.child.poll() is None:
            self.child.stdin.close()
            try:
                self.child.wait(timeout=1)
            except subprocess.TimeoutExpired:
                self.fixture.stop_child(self.child)
        self.fixture.operations.append({'name': self.name, 'role': self.role,
                                        'pid': self.child.pid, 'exit_code': self.child.returncode})
        self.selector.close()
        self.child.stdout.close()
        self.child.stderr.close()
        self.inputs.close()
        self.transcript.close()
        self.errors.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fixture-id', required=True)
    parser.add_argument('--runtime-root', type=Path, default=RUNTIME_ROOT)
    parser.add_argument('--protected-pids', required=True,
                        help='Coordinator-current previews, original PostgreSQL and recorder PIDs')
    args = parser.parse_args()
    def interrupted(signum, frame):
        raise QualificationError('bounded fixture interrupted by signal ' + str(signum))
    for signum in (signal.SIGINT, signal.SIGTERM, signal.SIGALRM):
        signal.signal(signum, interrupted)
    # Reserve twenty seconds inside the120second total for owned-child shutdown.
    signal.alarm(100)
    fixture = None
    before = None
    try:
        before = protected_identity(args.protected_pids)
        fixture = Fixture(args.fixture_id, args.runtime_root)
        source_dir = Path(__file__).absolute().parent
        paths = [source_dir / 'qualify-postgres-rls.py', source_dir / 'postgres-rls-g05.md',
                 *sorted((source_dir / 'fixtures/postgres-rls').glob('*.sql'))]
        for path in paths:
            no_symlinks(path)
            require(path.stat().st_size <= 65536, 'source64KiB bound: ' + str(path))
        fixture.result['source_hashes'] = {str(p): digest(p) for p in paths}
        fixture.result['source_commit'] = subprocess.run(
            ['/usr/bin/git', '-C', str(source_dir), 'rev-parse', 'HEAD'],
            capture_output=True, text=True, timeout=5, check=True).stdout.strip()
        fixture.result['source_status'] = subprocess.run(
            ['/usr/bin/git', '-C', str(source_dir), 'status', '--porcelain'],
            capture_output=True, text=True, timeout=5, check=True).stdout
        fixture.start()
        for name, role in (('setup', 'fixture_admin'), ('runtime', 'fixture_runtime'),
                           ('denials', 'fixture_runtime')):
            fixture.sql(name, (source_dir / ('fixtures/postgres-rls/' + name + '.sql')).read_text(), role)
        require(fixture.states == EXPECTED_STATES, 'missing expected SQLSTATE evidence')
        reuse = [r['backend'] for r in fixture.records if r['case'] in
                 ('direct_login','commit_reset','tenant_b','rollback_reset','sanitized_reuse','error_reset')]
        require(len(reuse) == 6 and len(set(reuse)) == 1, 'same-backend reuse evidence incomplete')
        fixture.result['reused_backend'] = reuse[0]
        fixture.sql('owner_force', "SET ROLE fixture_owner; SELECT json_build_object('case','owner_force'," +
                    "'pass',count(*)=0,'rows',count(*)) FROM fixture.parent; RESET ROLE;\n")
        fixture.concurrency()
        fixture.sql('final_counts', "SELECT json_build_object('case','final_counts','pass'," +
                    "(SELECT count(*)=64 FROM fixture.parent) AND (SELECT count(*)=64 FROM fixture.child)" +
                    " AND (SELECT count(*)=2 FROM fixture.parent WHERE id=2 AND value=1)," +
                    "'parents',(SELECT count(*) FROM fixture.parent),'children',(SELECT count(*) FROM fixture.child));\n")
        fixture.result['passed'] = True
    except (QualificationError, OSError, ValueError, subprocess.SubprocessError) as error:
        if fixture is not None:
            fixture.result['error'] = str(error)
        else:
            print(json.dumps({'passed': False, 'error': str(error)}))
    finally:
        signal.alarm(0)
        if fixture is not None:
            fixture.result['protected_before'] = before
            try:
                fixture.finish()
                after = protected_identity(args.protected_pids)
                fixture.result['protected_after'] = after
                fixture.result['protected_identity_preserved'] = after == before
                require(after == before, 'protected process identity changed')
                require(fixture.server is None or fixture.server.returncode == 0,
                        'owned server shutdown was not clean')
                require(all(digest(BIN / name) == value for name, value in BINARY_HASHES.items()),
                        'approved binary changed during run')
            except (QualificationError, OSError, subprocess.SubprocessError) as error:
                fixture.result.update({'passed': False, 'shutdown_error': str(error)})
            (fixture.root / 'result.json').write_text(json.dumps(fixture.result, indent=2) + '\n')
            print(json.dumps({'passed': fixture.result['passed'], 'result': str(fixture.root / 'result.json'),
                              'error': fixture.result.get('error'), 'shutdown_error': fixture.result.get('shutdown_error')}))
    return 0 if fixture is not None and fixture.result['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
