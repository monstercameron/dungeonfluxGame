#!/bin/bash
set -euo pipefail

VERSION=18.6
ARCHIVE_SHA256=983ee554ec53dbeb9b70797bef9fcf4e67e117e7e48ca1463cc80b3ff8e8ff3f
ARCHIVE_URL="https://ftp.postgresql.org/pub/source/v${VERSION}/postgresql-${VERSION}.tar.gz"
PORT=55439
SOCKET_NAME=".s.PGSQL.${PORT}"
MAX_CONNECTIONS=16
MAX_WORKER_PROCESSES=8
MAX_PARALLEL_WORKERS=8
MAX_PARALLEL_WORKERS_PER_GATHER=2
MAX_AUTOVACUUM_WORKERS=3

no_symlink_path() {
  python3 - "$1" <<'PY'
import os, stat, sys
path = os.path.abspath(sys.argv[1])
current = os.path.sep
for part in path.split(os.path.sep):
    if not part:
        continue
    current = os.path.join(current, part)
    try:
        info = os.lstat(current)
    except FileNotFoundError:
        continue
    if stat.S_ISLNK(info.st_mode):
        print(f"postgres-g05: refusing symlink component: {current}", file=sys.stderr)
        sys.exit(1)
PY
}

SCRIPT_INPUT=${BASH_SOURCE[0]}
[[ "$SCRIPT_INPUT" = /* ]] || SCRIPT_INPUT="$PWD/$SCRIPT_INPUT"
no_symlink_path "$SCRIPT_INPUT"
SCRIPT_DIR=$(cd -- "$(dirname -- "$SCRIPT_INPUT")" && pwd -P)
WORKTREE_ROOT=$(cd -- "$SCRIPT_DIR/.." && pwd -P)
no_symlink_path "$WORKTREE_ROOT"
COMMON_DIR=$(git -C "$WORKTREE_ROOT" rev-parse --path-format=absolute --git-common-dir)
no_symlink_path "$COMMON_DIR"
REPO_ROOT=$(cd -- "$(dirname -- "$COMMON_DIR")" && pwd -P)
no_symlink_path "$REPO_ROOT"

ARTIFACT_ROOT="$REPO_ROOT/artifacts"
CACHE="$ARTIFACT_ROOT/cache/postgres-g05"
BUILD="$ARTIFACT_ROOT/build/postgres-g05"
SCRATCH="$ARTIFACT_ROOT/tmp/SETUP-G05-001-a2"
DURABLE="$REPO_ROOT/development/runtime/postgres-g05"
OWNER="$DURABLE/owner.json"
DATA="$DURABLE/data"
SOCKET_DIR="$DURABLE/socket"
SOCKET="$SOCKET_DIR/$SOCKET_NAME"
LOG="$DURABLE/server.log"
PREFIX="$BUILD/install"
BIN="$PREFIX/bin"

fail() { printf 'postgres-g05: %s\n' "$*" >&2; exit 1; }

guard_runtime_path() {
  no_symlink_path "$REPO_ROOT/development/runtime"
  no_symlink_path "$DURABLE"
}

run_bounded() {
  local limit=$1
  shift
  python3 -c '
import os, signal, subprocess, sys, time
limit = float(sys.argv[1])
argv = sys.argv[2:]
if not argv:
    raise SystemExit("empty bounded command")
child = None
def stop_child():
    if child is None:
        return
    try:
        os.killpg(child.pid, signal.SIGTERM)
    except ProcessLookupError:
        return
    try:
        child.wait(timeout=1)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(child.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        child.wait()
def interrupted(signum, frame):
    stop_child()
    raise SystemExit(128 + signum)
signal.signal(signal.SIGINT, interrupted)
signal.signal(signal.SIGTERM, interrupted)
child = subprocess.Popen(argv, stdin=None, stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
try:
    stdout, stderr = child.communicate(timeout=limit)
except subprocess.TimeoutExpired:
    stop_child()
    stdout, stderr = child.communicate()
    sys.stdout.buffer.write(stdout)
    sys.stderr.buffer.write(stderr)
    print(f"postgres-g05: command exceeded {limit:g}s deadline: {argv[0]}", file=sys.stderr)
    raise SystemExit(124)
sys.stdout.buffer.write(stdout)
sys.stderr.buffer.write(stderr)
raise SystemExit(child.returncode)
' "$limit" "$@"
}

setup_started=0
setup_bounded() {
  local command_limit=$1
  shift
  local elapsed=$((SECONDS - setup_started)) remaining
  remaining=$((1200 - elapsed))
  (( remaining > 0 )) || fail 'setup exceeded its 1200s total deadline'
  (( command_limit <= remaining )) || command_limit=$remaining
  run_bounded "$command_limit" "$@"
}

make_owned_dir() {
  local path=$1
  no_symlink_path "$path"
  mkdir -p "$path"
  [[ -d "$path" && ! -L "$path" ]] || fail "not a real directory: $path"
  chmod 700 "$path"
}

check_owner() {
  guard_runtime_path
  no_symlink_path "$BIN"
  for tool in postgres pg_ctl pg_isready psql initdb; do no_symlink_path "$BIN/$tool"; done
  no_symlink_path "$OWNER"
  [[ -f "$OWNER" && ! -L "$OWNER" ]] || fail "cluster has no regular owner manifest: $DURABLE"
  python3 - "$OWNER" "$DURABLE" "$DATA" "$SOCKET_DIR" "$VERSION" <<'PY'
import json, os, stat, sys
manifest, root, data, sockets, version = sys.argv[1:]
try:
    with open(manifest, encoding="utf-8") as f:
        owner = json.load(f)
    if owner.get("uid") != os.getuid() or owner.get("version") != version:
        raise ValueError("cluster owner/version does not match this runtime")
    if owner.get("data_dir") != data or owner.get("socket_dir") != sockets:
        raise ValueError("owner manifest paths do not match this runtime")
    for path in (root, data, sockets):
        item = os.lstat(path)
        if not stat.S_ISDIR(item.st_mode) or item.st_uid != os.getuid() or stat.S_IMODE(item.st_mode) != 0o700:
            raise ValueError(f"directory ownership or permissions are unsafe: {path}")
    item = os.lstat(manifest)
    if not stat.S_ISREG(item.st_mode) or item.st_uid != os.getuid() or stat.S_IMODE(item.st_mode) != 0o600:
        raise ValueError("owner manifest ownership or permissions are unsafe")
except Exception as exc:
    print(f"postgres-g05: ownership check failed: {exc}", file=sys.stderr)
    sys.exit(1)
PY
  [[ -f "$DATA/PG_VERSION" && ! -L "$DATA/PG_VERSION" ]] || fail 'data directory is not an initialized cluster'
  [[ $(cat "$DATA/PG_VERSION") == 18 ]] || fail 'existing cluster major version is not 18'
}

verify_running() {
  local identity
  identity=$(run_bounded 10 python3 - "$DATA" "$SOCKET_DIR" "$SOCKET" "$BIN/postgres" "$BIN/pg_isready" "$BIN/psql" "$PORT" "$MAX_CONNECTIONS" "$MAX_WORKER_PROCESSES" "$MAX_PARALLEL_WORKERS" "$MAX_PARALLEL_WORKERS_PER_GATHER" "$MAX_AUTOVACUUM_WORKERS" <<'PY'
import ctypes, os, pwd, signal, stat, subprocess, sys, time
from pathlib import Path

data, socket_dir, socket_path, postgres_bin, ready_bin, psql_bin, port, max_connections, max_workers, parallel_workers, parallel_gather, autovacuum_workers = sys.argv[1:]
active_children = []
def stop_group(child):
    try:
        os.killpg(child.pid, signal.SIGTERM)
    except ProcessLookupError:
        return
    try:
        child.communicate(timeout=1)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(child.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        child.communicate()
def on_signal(signum, frame):
    for child in list(active_children):
        stop_group(child)
    raise SystemExit(128 + signum)
signal.signal(signal.SIGINT, on_signal)
signal.signal(signal.SIGTERM, on_signal)
def capture(argv, timeout, env=None):
    child = subprocess.Popen(argv, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=env, start_new_session=True)
    active_children.append(child)
    try:
        stdout, stderr = child.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        stop_group(child)
        raise
    finally:
        if child in active_children:
            active_children.remove(child)
    return subprocess.CompletedProcess(argv, child.returncode, stdout, stderr)
port, max_connections, max_workers, parallel_workers, parallel_gather, autovacuum_workers = map(int, (port, max_connections, max_workers, parallel_workers, parallel_gather, autovacuum_workers))
pidfile = Path(data) / "postmaster.pid"
def reject(message):
    print(f"postgres-g05: running-server identity check failed: {message}", file=sys.stderr)
    raise SystemExit(1)
try:
    st = os.lstat(pidfile)
    if not stat.S_ISREG(st.st_mode) or st.st_uid != os.getuid() or stat.S_IMODE(st.st_mode) & 0o077:
        reject("postmaster.pid is not a private regular file owned by this user")
    lines = pidfile.read_text(encoding="utf-8").splitlines()
    if len(lines) < 6:
        reject("postmaster.pid is incomplete")
    pid = int(lines[0])
    start_epoch = int(lines[2])
    if pid <= 1 or lines[1] != data or int(lines[3]) != port or lines[4] != socket_dir or lines[5] != "":
        reject("postmaster.pid fields do not match the configured cluster")
    expected = os.path.realpath(postgres_bin)
    if sys.platform == "darwin":
        lib = ctypes.CDLL("/usr/lib/libproc.dylib")
        buf = ctypes.create_string_buffer(4096)
        if lib.proc_pidpath(pid, buf, len(buf)) <= 0:
            reject("PID has no readable executable path")
        executable = os.path.realpath(os.fsdecode(buf.value))
    elif sys.platform.startswith("linux"):
        executable = os.path.realpath(os.readlink(f"/proc/{pid}/exe"))
    else:
        reject("process identity checks are unsupported on this operating system")
    if executable != expected:
        reject("PID executable does not match the owned PostgreSQL binary")
    ps = subprocess.run(["ps", "-ww", "-p", str(pid), "-o", "command="], capture_output=True, text=True, timeout=2, check=True)
    if f"-D {data}" not in ps.stdout.strip():
        reject("PID command line does not name the owned data directory")
    deadline = time.monotonic() + 9.0
    ready = capture([ready_bin, "-h", socket_dir, "-p", str(port), "-t", "8"], max(.1, deadline-time.monotonic()))
    if ready.returncode != 0:
        reject("private Unix socket did not become ready within 10 seconds")
    query = "SELECT current_setting('data_directory'), floor(extract(epoch FROM pg_postmaster_start_time()))::bigint, current_setting('port'), current_setting('unix_socket_directories'), current_setting('listen_addresses'), current_setting('max_connections'), current_setting('max_worker_processes'), current_setting('max_parallel_workers'), current_setting('max_parallel_workers_per_gather'), current_setting('autovacuum_max_workers'), current_setting('fsync'), current_setting('full_page_writes'), current_setting('synchronous_commit'), (inet_server_addr() IS NULL), current_setting('server_version_num')"
    env = os.environ.copy()
    env["PGOPTIONS"] = "-c statement_timeout=4000"
    env.pop("PGPASSWORD", None)
    remaining = deadline - time.monotonic()
    if remaining <= 0:
        reject("readiness deadline expired before diagnostic query")
    sql = capture([psql_bin, "-XAtq", "-F", "|", "-v", "ON_ERROR_STOP=1", "-w", "-h", socket_dir, "-p", str(port), "-U", pwd.getpwuid(os.getuid()).pw_name, "-d", "postgres", "-c", query], remaining, env=env)
    if sql.returncode != 0:
        reject(f"owned psql readiness query failed with exit {sql.returncode}")
    fields = sql.stdout.strip().split("|")
    if len(fields) != 15:
        reject("diagnostic query returned an incomplete identity")
    real_data = os.path.realpath(data)
    expected_values = [real_data, str(port), socket_dir, "", str(max_connections), str(max_workers), str(parallel_workers), str(parallel_gather), str(autovacuum_workers), "on", "on", "on", "t"]
    if fields[0:1] + fields[2:14] != expected_values or abs(int(fields[1]) - start_epoch) > 1 or not fields[14].startswith("18000"):
        reject("SQL server identity/configuration does not match the owned cluster")
    if sys.platform == "darwin":
        buf2 = ctypes.create_string_buffer(4096)
        if lib.proc_pidpath(pid, buf2, len(buf2)) <= 0 or os.path.realpath(os.fsdecode(buf2.value)) != expected:
            reject("PID identity changed during readiness verification")
    else:
        if os.path.realpath(os.readlink(f"/proc/{pid}/exe")) != expected:
            reject("PID identity changed during readiness verification")
    print(f"pid={pid} version=18.6 data_directory={real_data} max_connections={max_connections} max_worker_processes={max_workers} max_parallel_workers={parallel_workers} max_parallel_workers_per_gather={parallel_gather} autovacuum_max_workers={autovacuum_workers} fsync=on full_page_writes=on synchronous_commit=on listen_addresses='' tcp=false")
except subprocess.TimeoutExpired:
    reject("readiness probe exceeded its 10 second deadline")
except subprocess.CalledProcessError as exc:
    reject(f"owned psql readiness query failed with exit {exc.returncode}")
except (OSError, ValueError, IndexError) as exc:
    reject(str(exc))
PY
  ) || return 1
  printf '%s\n' "$identity"
}

RUNTIME_DETAILS=
find_orphan_data_process() {
  run_bounded 5 python3 - "$DATA" <<'PY'
import subprocess, sys
data = sys.argv[1]
try:
    listing = subprocess.run(["ps", "-Aww", "-o", "pid=,command="], capture_output=True, text=True, timeout=3, check=True).stdout
    for line in listing.splitlines():
        parts = line.strip().split(None, 1)
        if len(parts) != 2 or f"-D {data}" not in parts[1]:
            continue
        print(parts[0])
except Exception as exc:
    print(f"postgres-g05: orphan-process scan failed: {exc}", file=sys.stderr)
    sys.exit(1)
PY
}

runtime_state() {
  RUNTIME_DETAILS=
  local pidfile="$DATA/postmaster.pid"
  no_symlink_path "$SOCKET"
  no_symlink_path "$pidfile"
  if [[ -L "$pidfile" ]]; then fail 'refusing symlinked postmaster.pid'; fi
  if [[ -e "$pidfile" ]]; then
    local pid
    pid=$(head -n 1 "$pidfile")
    [[ "$pid" =~ ^[0-9]+$ ]] || fail 'postmaster.pid is malformed; refusing lifecycle action'
    RUNTIME_DETAILS=$(verify_running) || fail 'postmaster.pid/process/socket ownership is ambiguous; refusing lifecycle action'
    return 0
  fi
  local orphan_pid
  orphan_pid=$(find_orphan_data_process) || fail 'could not verify absence of a PostgreSQL process; refusing lifecycle action'
  [[ -z "$orphan_pid" ]] || fail "process PID $orphan_pid references the owned data directory without its PID file; refusing lifecycle action"
  if [[ -e "$SOCKET" ]]; then fail 'socket exists without postmaster.pid; refusing lifecycle action'; fi
  return 1
}

setup() {
  setup_started=$SECONDS
  guard_runtime_path
  no_symlink_path "$CACHE"
  no_symlink_path "$BUILD"
  no_symlink_path "$SCRATCH"
  no_symlink_path "$PREFIX"
  for tool in postgres pg_ctl pg_isready psql initdb; do no_symlink_path "$BIN/$tool"; done
  local archive="$CACHE/postgresql-${VERSION}.tar.gz" src="$SCRATCH/postgresql-${VERSION}"
  no_symlink_path "$archive"
  mkdir -p "$CACHE" "$BUILD" "$SCRATCH"
  if [[ ! -f "$archive" ]]; then
    no_symlink_path "$archive.part"
    setup_bounded 95 curl --proto '=https' --tlsv1.2 -fsS --max-time 90 "$ARCHIVE_URL" -o "$archive.part"
    no_symlink_path "$archive.part"
    setup_bounded 60 bash -c 'printf "%s  %s\n" "$1" "$2" | shasum -a 256 -c -' _ "$ARCHIVE_SHA256" "$archive.part"
    mv "$archive.part" "$archive"
  else
    setup_bounded 60 bash -c 'printf "%s  %s\n" "$1" "$2" | shasum -a 256 -c -' _ "$ARCHIVE_SHA256" "$archive"
  fi
  if [[ ! -x "$BIN/pg_ctl" ]]; then
    no_symlink_path "$src"
    [[ ! -e "$src" ]] || fail "source extraction already exists without installed runtime; inspect $src"
    setup_bounded 60 tar -xzf "$archive" -C "$SCRATCH"
    setup_bounded 900 bash -c 'cd "$1" && ./configure --prefix="$2" --without-readline --without-icu --without-llvm --without-lz4 --without-zstd --without-openssl --without-pam --without-systemd --without-bonjour --without-gssapi --without-ldap --without-perl --without-python --without-tcl --without-libxml --without-libxslt' _ "$src" "$PREFIX"
    setup_bounded 900 make -C "$src" -j1
    setup_bounded 900 make -C "$src" install
  fi
  [[ -x "$BIN/initdb" && -x "$BIN/pg_ctl" && -x "$BIN/pg_isready" && -x "$BIN/psql" && -x "$BIN/postgres" ]] || fail 'installed PostgreSQL tools are incomplete'
  local psql_version
  psql_version=$(setup_bounded 60 "$BIN/psql" --version) || fail 'could not query installed psql version'
  [[ "$psql_version" == *"PostgreSQL) $VERSION"* ]] || fail "installed psql version mismatch: $psql_version"
  if [[ -e "$DURABLE" ]]; then
    check_owner
    printf 'Existing owned PostgreSQL %s cluster preserved at %s\n' "$VERSION" "$DATA"
    return
  fi
  no_symlink_path "$REPO_ROOT/development/runtime"
  mkdir -p "$REPO_ROOT/development/runtime"
  make_owned_dir "$DURABLE"
  make_owned_dir "$DATA"
  make_owned_dir "$SOCKET_DIR"
  setup_bounded 60 "$BIN/initdb" --encoding=UTF8 --locale=C -D "$DATA" --auth-local=trust --auth-host=reject
  cat >> "$DATA/postgresql.conf" <<EOF_CONF
listen_addresses = ''
unix_socket_directories = '$SOCKET_DIR'
unix_socket_permissions = 0700
port = $PORT
max_connections = $MAX_CONNECTIONS
max_worker_processes = $MAX_WORKER_PROCESSES
max_parallel_workers = $MAX_PARALLEL_WORKERS
max_parallel_workers_per_gather = $MAX_PARALLEL_WORKERS_PER_GATHER
autovacuum_max_workers = $MAX_AUTOVACUUM_WORKERS
fsync = on
full_page_writes = on
synchronous_commit = on
logging_collector = off
log_destination = 'stderr'
EOF_CONF
  python3 - "$OWNER" <<'PY'
import json, os, sys
p=sys.argv[1]
with open(p,"x",encoding="utf-8") as f:
    json.dump({"uid":os.getuid(),"version":"18.6","data_dir":os.path.dirname(p)+"/data","socket_dir":os.path.dirname(p)+"/socket","port":55439,"created_by":"development/postgres.sh"},f,sort_keys=True,indent=2)
    f.write("\n")
os.chmod(p,0o600)
PY
  printf 'Initialized owned PostgreSQL %s cluster at %s\n' "$VERSION" "$DATA"
}

start() {
  check_owner
  if runtime_state; then
    printf 'already running: verified owned PostgreSQL %s %s\n' "$VERSION" "$RUNTIME_DETAILS"
    return
  fi
  run_bounded 60 "$BIN/pg_ctl" -D "$DATA" -l "$LOG" -o "-c listen_addresses='' -c unix_socket_directories='$SOCKET_DIR' -c unix_socket_permissions=0700 -c port=$PORT -c max_connections=$MAX_CONNECTIONS -c max_worker_processes=$MAX_WORKER_PROCESSES -c max_parallel_workers=$MAX_PARALLEL_WORKERS -c max_parallel_workers_per_gather=$MAX_PARALLEL_WORKERS_PER_GATHER -c autovacuum_max_workers=$MAX_AUTOVACUUM_WORKERS -c fsync=on -c full_page_writes=on -c synchronous_commit=on" -w -t 10 start
  local details
  details=$(verify_running) || fail 'server command completed but PostgreSQL process/readiness identity could not be verified'
  printf 'started PostgreSQL %s %s socket=%s\n' "$VERSION" "$details" "$SOCKET"
}

status() {
  guard_runtime_path
  if [[ ! -e "$DURABLE" ]]; then
    printf 'not initialized: %s\n' "$DURABLE"
    return 3
  fi
  check_owner
  if runtime_state; then
    printf 'ready %s\n' "$RUNTIME_DETAILS"
  else
    printf 'stopped: owned cluster at %s\n' "$DATA"
  fi
}

stop() {
  check_owner
  local details pid
  runtime_state || { printf 'already stopped\n'; return; }
  details=$RUNTIME_DETAILS
  pid=${details#pid=}; pid=${pid%% *}
  run_bounded 60 "$BIN/pg_ctl" -D "$DATA" -m fast -w -t 30 stop
  [[ ! -e "$DATA/postmaster.pid" && ! -e "$SOCKET" ]] || fail 'owned server did not remove its PID file/socket after graceful stop'
  printf 'stopped owned PostgreSQL pid=%s\n' "$pid"
}

if [[ "${POSTGRES_G05_BOUNDED_CHILD:-0}" != 1 ]]; then
  case "${1:-}" in
    setup) action_timeout=1200 ;;
    start|status|stop) action_timeout=60 ;;
    *) printf 'usage: %s {setup|start|status|stop}\n' "$0" >&2; exit 2 ;;
  esac
  export POSTGRES_G05_BOUNDED_CHILD=1
  run_bounded "$action_timeout" "$SCRIPT_INPUT" "$1"
  exit $?
fi

case "${1:-}" in
  setup) setup ;;
  start) start ;;
  status) status ;;
  stop) stop ;;
  *) printf 'usage: %s {setup|start|status|stop}\n' "$0" >&2; exit 2 ;;
esac
