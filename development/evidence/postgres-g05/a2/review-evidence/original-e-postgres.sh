#!/bin/bash
set -euo pipefail

VERSION=18.6
ARCHIVE_SHA256=983ee554ec53dbeb9b70797bef9fcf4e67e117e7e48ca1463cc80b3ff8e8ff3f
ARCHIVE_URL="https://ftp.postgresql.org/pub/source/v${VERSION}/postgresql-${VERSION}.tar.gz"
SOCKET_NAME=.s.PGSQL.55439
MAX_CONNECTIONS=16

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
WORKTREE_ROOT=$(cd -- "$SCRIPT_DIR/.." && pwd -P)
COMMON_DIR=$(git -C "$WORKTREE_ROOT" rev-parse --path-format=absolute --git-common-dir)
REPO_ROOT=$(cd -- "$(dirname -- "$COMMON_DIR")" && pwd -P)
ARTIFACT_ROOT="$REPO_ROOT/artifacts"
CACHE="$ARTIFACT_ROOT/cache/postgres-g05"
BUILD="$ARTIFACT_ROOT/build/postgres-g05"
SCRATCH="$ARTIFACT_ROOT/tmp/SETUP-G05-001-a1"
DURABLE="$REPO_ROOT/development/runtime/postgres-g05"
OWNER="$DURABLE/owner.json"
DATA="$DURABLE/data"
SOCKET_DIR="$DURABLE/socket"
LOG="$DURABLE/server.log"
PREFIX="$BUILD/install"
BIN="$PREFIX/bin"

fail() { printf 'postgres-g05: %s\n' "$*" >&2; exit 1; }

no_symlink_path() {
  local path=$1 cursor=/ part
  IFS=/ read -r -a parts <<< "$path"
  for part in "${parts[@]}"; do
    [[ -z "$part" ]] && continue
    cursor="${cursor%/}/$part"
    [[ ! -L "$cursor" ]] || fail "refusing symlink path: $cursor"
  done
}

make_owned_dir() {
  local path=$1
  no_symlink_path "$path"
  mkdir -p "$path"
  [[ -d "$path" && ! -L "$path" ]] || fail "not a real directory: $path"
  chmod 700 "$path"
}

check_owner() {
  [[ -f "$OWNER" && ! -L "$OWNER" ]] || fail "cluster has no regular owner manifest: $DURABLE"
  python3 - "$OWNER" "$DURABLE" "$VERSION" <<'PY'
import json, os, sys
manifest, root, version = sys.argv[1:]
try:
    with open(manifest, encoding="utf-8") as f:
        owner = json.load(f)
    st = os.stat(root, follow_symlinks=False)
    if owner.get("uid") != os.getuid() or st.st_uid != os.getuid():
        raise ValueError("cluster owner does not match current user")
    if owner.get("version") != version or owner.get("data_dir") != os.path.join(root, "data"):
        raise ValueError("owner manifest does not match this runtime")
    for path in (root, os.path.join(root, "data"), os.path.join(root, "socket")):
        item = os.stat(path, follow_symlinks=False)
        if not os.path.isdir(path) or item.st_uid != os.getuid() or item.st_mode & 0o777 != 0o700:
            raise ValueError(f"directory ownership or permissions are unsafe: {path}")
    manifest_stat = os.stat(manifest, follow_symlinks=False)
    if not os.path.isfile(manifest) or manifest_stat.st_uid != os.getuid() or manifest_stat.st_mode & 0o777 != 0o600:
        raise ValueError("owner manifest ownership or permissions are unsafe")
except Exception as exc:
    print(f"postgres-g05: ownership check failed: {exc}", file=sys.stderr)
    sys.exit(1)
PY
  [[ -d "$DATA" && ! -L "$DATA" ]] || fail "data directory missing or unsafe"
  [[ -d "$SOCKET_DIR" && ! -L "$SOCKET_DIR" ]] || fail "socket directory missing or unsafe"
  [[ $(stat -f '%u' "$DATA") == $(id -u) ]] || fail "data directory is owned by another user"
  [[ $(stat -f '%Lp' "$DATA") == 700 ]] || fail "data directory permissions are not 700"
}

server_pid() {
  [[ -S "$SOCKET_DIR/$SOCKET_NAME" ]] || return 1
  "$BIN/pg_ctl" -D "$DATA" -o "-c unix_socket_directories='$SOCKET_DIR' -c unix_socket_permissions=0700" status >/dev/null 2>&1 || return 1
  "$BIN/pg_ctl" -D "$DATA" -o "-c unix_socket_directories='$SOCKET_DIR' -c unix_socket_permissions=0700" status 2>&1 | sed -n 's/.*server is running (PID: \([0-9][0-9]*\)).*/\1/p'
}

setup() {
  local archive="$CACHE/postgresql-${VERSION}.tar.gz" src="$SCRATCH/postgresql-${VERSION}"
  no_symlink_path "$CACHE"
  no_symlink_path "$BUILD"
  no_symlink_path "$SCRATCH"
  no_symlink_path "$PREFIX"
  no_symlink_path "$archive"
  mkdir -p "$CACHE" "$BUILD" "$SCRATCH"
  for d in "$CACHE" "$BUILD" "$SCRATCH"; do [[ ! -L "$d" ]] || fail "refusing symlink: $d"; done
  if [[ ! -f "$archive" ]]; then
    curl --proto '=https' --tlsv1.2 -fsS --max-time 90 "$ARCHIVE_URL" -o "$archive.part"
    printf '%s  %s\n' "$ARCHIVE_SHA256" "$archive.part" | shasum -a 256 -c -
    mv "$archive.part" "$archive"
  else
    printf '%s  %s\n' "$ARCHIVE_SHA256" "$archive" | shasum -a 256 -c -
  fi
  if [[ ! -x "$BIN/pg_ctl" ]]; then
    [[ ! -e "$src" ]] || fail "source extraction already exists without installed runtime; inspect $src"
    tar -xzf "$archive" -C "$SCRATCH"
    (cd "$src" && ./configure --prefix="$PREFIX" --without-readline --without-icu --without-llvm --without-lz4 --without-zstd --without-openssl --without-pam --without-systemd --without-bonjour --without-gssapi --without-ldap --without-perl --without-python --without-tcl --without-libxml --without-libxslt)
    make -C "$src" -j1
    make -C "$src" install
  fi
  [[ -x "$BIN/initdb" && -x "$BIN/pg_ctl" && -x "$BIN/psql" ]] || fail "installed PostgreSQL tools are incomplete"
  "$BIN/psql" --version | grep -F "${VERSION}" >/dev/null || fail "installed psql version mismatch"
  if [[ -e "$DURABLE" ]]; then
    check_owner
    [[ -f "$DATA/PG_VERSION" ]] || fail "existing owner data has no PostgreSQL cluster; refusing replacement"
    [[ $(cat "$DATA/PG_VERSION") == 18 ]] || fail "existing cluster major version is not 18"
    printf 'Existing owned PostgreSQL %s cluster preserved at %s\n' "$VERSION" "$DATA"
    return
  fi
  no_symlink_path "$REPO_ROOT/development/runtime"
  mkdir -p "$REPO_ROOT/development/runtime"
  make_owned_dir "$DURABLE"
  make_owned_dir "$DATA"
  make_owned_dir "$SOCKET_DIR"
  "$BIN/initdb" --encoding=UTF8 --locale=C -D "$DATA" --auth-local=trust --auth-host=reject
  cat >> "$DATA/postgresql.conf" <<EOF_CONF
listen_addresses = ''
unix_socket_directories = '$SOCKET_DIR'
unix_socket_permissions = 0700
port = 55439
max_connections = $MAX_CONNECTIONS
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
  if "$BIN/pg_ctl" -D "$DATA" status >/dev/null 2>&1; then
    printf 'already running: '
    "$BIN/pg_ctl" -D "$DATA" status
    return
  fi
  [[ ! -e "$DATA/postmaster.pid" ]] || fail "stale or foreign postmaster.pid exists; inspect manually"
  "$BIN/pg_ctl" -D "$DATA" -l "$LOG" -o "-c listen_addresses='' -c unix_socket_directories='$SOCKET_DIR' -c unix_socket_permissions=0700 -c port=55439 -c max_connections=$MAX_CONNECTIONS -c fsync=on -c full_page_writes=on -c synchronous_commit=on" -w -t 30 start
  local pid
  pid=$(server_pid) || fail "server did not become ready on owned Unix socket"
  printf 'started PostgreSQL %s pid=%s socket=%s\n' "$VERSION" "$pid" "$SOCKET_DIR/$SOCKET_NAME"
}

status() {
  [[ -d "$DURABLE" ]] || { printf 'not initialized: %s\n' "$DURABLE"; return 3; }
  check_owner
  if "$BIN/pg_ctl" -D "$DATA" status >/dev/null 2>&1; then
    "$BIN/pg_ctl" -D "$DATA" status
    "$BIN/psql" -XAtq -h "$SOCKET_DIR" -p 55439 -U "$(id -un)" -d postgres -c "select 'ready', version(), current_setting('listen_addresses'), current_setting('max_connections'), current_setting('fsync'), current_setting('full_page_writes'), current_setting('synchronous_commit')"
  else
    printf 'stopped: owned cluster at %s\n' "$DATA"
  fi
}

stop() {
  check_owner
  if ! "$BIN/pg_ctl" -D "$DATA" status >/dev/null 2>&1; then
    [[ ! -e "$DATA/postmaster.pid" ]] || fail "server status failed while postmaster.pid exists; refusing to signal any process"
    printf 'already stopped\n'
    return
  fi
  local pid
  pid=$(server_pid) || fail "unable to verify owned server PID; refusing stop"
  "$BIN/pg_ctl" -D "$DATA" -m fast -w -t 30 stop
  [[ ! -e "$DATA/postmaster.pid" ]] || fail "server did not remove its PID file"
  printf 'stopped owned PostgreSQL pid=%s\n' "$pid"
}

case "${1:-}" in
  setup) setup ;;
  start) start ;;
  status) status ;;
  stop) stop ;;
  *) printf 'usage: %s {setup|start|status|stop}\n' "$0" >&2; exit 2 ;;
esac
