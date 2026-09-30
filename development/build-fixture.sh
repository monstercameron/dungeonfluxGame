#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
ARTIFACT_ROOT=${DUNGEONFLUX_ARTIFACT_ROOT:-$ROOT/artifacts}
export RUSTUP_HOME="$ARTIFACT_ROOT/cache/rustup"
export CARGO_HOME="$ARTIFACT_ROOT/cache/cargo"
export CARGO_TARGET_DIR=${DUNGEONFLUX_BUILD_ROOT:-$ARTIFACT_ROOT/build/start-s00}
export TMPDIR=${DUNGEONFLUX_TMP_ROOT:-$ARTIFACT_ROOT/tmp/start-s00}
export PATH="$CARGO_HOME/bin:$PATH"
export CARGO_BUILD_JOBS=2
DF_FIXTURE_BUILD=$(git -C "$ROOT" rev-parse HEAD)
if test -n "$(git -C "$ROOT" status --porcelain -- Cargo.toml Cargo.lock rust-toolchain.toml .cargo crates development/build-fixture.sh)"; then
  DF_FIXTURE_BUILD="$DF_FIXTURE_BUILD-dirty"
fi
export DF_FIXTURE_BUILD
mkdir -p "$TMPDIR"
cd "$ROOT"
case "${1:-build}" in
  cargo) shift; exec cargo "$@" ;;
  build)
    cargo build --locked -p df-tools --bin df-transport-fixture
    cargo build --locked -p df-tools --lib --target wasm32-unknown-unknown
    test "$(wasm-bindgen --version)" = "wasm-bindgen 0.2.129"
    STAGING="$TMPDIR/web-staging"
    mkdir -p "$STAGING"
    wasm-bindgen --target web --out-dir "$STAGING" "$CARGO_TARGET_DIR/wasm32-unknown-unknown/debug/df_tools.wasm"
    if test -d "$CARGO_TARGET_DIR/web"; then
      mv "$CARGO_TARGET_DIR/web" "$TMPDIR/web-previous-$(date +%s)"
    fi
    mv "$STAGING" "$CARGO_TARGET_DIR/web"
    printf '%s\n' "$DF_FIXTURE_BUILD" > "$CARGO_TARGET_DIR/web/source-revision.txt"
    ;;
  serve) exec "$CARGO_TARGET_DIR/debug/df-transport-fixture" "$CARGO_TARGET_DIR/web" "${DUNGEONFLUX_PREVIEW_PORT:-43180}" ;;
  *) echo 'usage: build-fixture.sh [build|serve|cargo arguments...]' >&2; exit 2 ;;
esac
