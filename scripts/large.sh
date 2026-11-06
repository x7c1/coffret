#!/usr/bin/env bash
#
# A Library of about a hundred thousand files, and the measurements a filter
# for the explorer would rest on, taken against it.
#
#   scripts/large.sh seed <minio-port>
#   scripts/large.sh measure <port> <web-port> <minio-port>
#   scripts/large.sh down
#   scripts/large.sh reset
#
# `seed` creates the Library the way `make local` creates its own — MinIO in a
# container, a device state under .tmp/, a fixed Passphrase — and fills it
# with the `large` scenario of `coffret-fixtures`
# (backend/crates/apps/coffret-fixtures/src/large.rs): about a hundred thousand
# tiny pages in books and series, every one frozen into Packs and committed,
# so that the Index and the listing are what a device holds. The freezes run
# one series, author, magazine year or archive collection at a time (the loose
# scans in one), in an order mixing the parts, and each commits at its own
# moment, which is what spreads the Library's deposit times. Where the Library
# is seeded already, `seed` says so and does nothing.
#
# `measure` takes the three measurements and prints them as one report:
#
#   1. the full-scan match and the n most recent books, timed by
#      `coffret-measure` over the Index file read-only, before anything serves
#      the Library;
#   2. the folder listing the explorer uses (`GET /api/list`), at the root and
#      at the folder with the most rows, asked from the explorer's own tab;
#   3. the tree's `↓` with about 5,000 and then about 20,000 rows open, from
#      the key press to the frame that paints the new focus, in Playwright's
#      Chromium, headless.
#
# For 2 and 3 it starts MinIO, the server, the explorer's build and the
# development host, as `make local` does (by scripts/dev.sh), and stops all of
# them again when it is done, however it ends. The report is also left in
# .tmp/large/report.txt.
#
# `down` stops whatever an interrupted `seed` or `measure` left running and
# keeps the Library, and `reset` is `down`, then .tmp/large/ removed.
#
# Everything lives under .tmp/large/, apart from `make local`'s .tmp/local/:
#
#   minio/        MinIO's data, bind-mounted into the container
#   state/        the device's state directory: the Library's catalog, its keys
#   library/      the folders the Library is mapped to, one per top-level part
#   logs/         the binaries' logs, and seed.log, what the seed's commands said
#   run/          the pid files and the output of the server, build and host
#   minio-port    the port MinIO was published on when the Library was created,
#                 which its settings name as the endpoint
#   freeze-units  the folders the seed freezes, one per line, in order
#   seeded        there once the seed has finished; it holds how many seconds
#                 the seed took
#   largest-folder  the folder with the most rows, as `coffret-measure` found
#                 it, where `measure` times the listing besides the root
#   playwright/   Playwright's own traces and failure shots from `measure`
#   report.txt    the last report `measure` printed
#
# The Passphrase is fixed and in the clear, for the reason scripts/local.sh
# gives for its own: the Library holds nothing but generated pictures, in a
# container bound to the loopback address, under a directory `reset` throws
# away.

set -euo pipefail

# Physical, as in scripts/local.sh: scripts/dev.sh recognises its processes by
# this path.
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
readonly ROOT

# shellcheck source=scripts/lib/minio.sh
source "$ROOT/scripts/lib/minio.sh"

usage() {
  echo "usage: scripts/large.sh seed <minio-port>
       scripts/large.sh measure <port> <web-port> <minio-port>
       scripts/large.sh down | scripts/large.sh reset" >&2
  exit 2
}

fail() {
  echo "error: $*" >&2
  exit 1
}

[ $# -ge 1 ] || usage
readonly COMMAND="$1"
PORT=""
WEB_PORT=""
MINIO_PORT=""
case "$COMMAND" in
  seed)
    [ $# -eq 2 ] || usage
    MINIO_PORT="$2"
    ;;
  measure)
    [ $# -eq 4 ] || usage
    PORT="$2"
    WEB_PORT="$3"
    MINIO_PORT="$4"
    ;;
  down | reset)
    [ $# -eq 1 ] || usage
    ;;
  *)
    usage
    ;;
esac
readonly PORT WEB_PORT MINIO_PORT

readonly WORK="$ROOT/.tmp/large"
readonly DATA="$WORK/minio"
readonly STATE="$WORK/state"
readonly FOLDERS="$WORK/library"
readonly LOG_DIR="$WORK/logs"
readonly RUN_DIR="$WORK/run"
readonly SEEDED="$WORK/seeded"
readonly SEED_LOG="$LOG_DIR/seed.log"
readonly SEEDED_PORT="$WORK/minio-port"
readonly UNITS="$WORK/freeze-units"
readonly REPORT="$WORK/report.txt"
readonly LARGEST="$WORK/largest-folder"
readonly TARGET="$ROOT/backend/target/release"
readonly COFFRET="$TARGET/coffret"
readonly FIXTURES="$TARGET/coffret-fixtures"
readonly MEASURE="$TARGET/coffret-measure"

CONTAINER="${COFFRET_LARGE_MINIO_CONTAINER:-coffret-minio-large}"
readonly CONTAINER

readonly LIBRARY="large"
readonly ACCESS_KEY="coffret-large"
readonly SECRET_KEY="coffret-large-secret"
readonly BUCKET="coffret-large"
readonly PASSPHRASE="a coffret Library of generated files"
readonly STARTUP_TIMEOUT_SECONDS=60

# The names measured: one in a great many Entries' paths (every comic's
# volume folder), one in a few (the one series whose name is written in
# full-width letters, typed here in half-width ones), and one in none.
readonly QUERIES=(--query "vol" --query "star lantern" --query "no such title")

# The rows the tree is opened to before `↓` is timed, and how many presses
# each count is timed over.
readonly ROWS="${COFFRET_LARGE_ROWS:-5000,20000}"
readonly PRESSES="${COFFRET_LARGE_PRESSES:-15}"

export COFFRET_STATE_DIR="$STATE"
export COFFRET_LOG_DIR="$LOG_DIR"
export AWS_ACCESS_KEY_ID="$ACCESS_KEY"
export AWS_SECRET_ACCESS_KEY="$SECRET_KEY"
export AWS_REGION="us-east-1"

readonly DEV_OPTIONS=(--run-dir "$RUN_DIR" --down "make large-down")

container_state() {
  docker inspect --format '{{.State.Status}}' "$CONTAINER" 2>/dev/null || true
}

# Whether the container by that name keeps its data in this checkout's
# directory, rather than another checkout's.
container_is_ours() {
  local mounted
  mounted="$(docker inspect --format \
    '{{range .Mounts}}{{if eq .Destination "'"$MINIO_DATA_IN_CONTAINER"'"}}{{.Source}}{{end}}{{end}}' \
    "$CONTAINER" 2>/dev/null || true)"
  [ "$mounted" = "$DATA" ]
}

start_minio() {
  if [ -n "$(container_state)" ]; then
    container_is_ours ||
      fail "a container named $CONTAINER is there, and it keeps its data somewhere other than
$DATA — most likely another checkout's. Stop it from there, or set
COFFRET_LARGE_MINIO_CONTAINER to another name."
    # Left by a run that was cut short: what it held is all in $DATA.
    docker rm --force "$CONTAINER" >/dev/null
  fi
  mkdir -p "$DATA"
  echo "Starting MinIO in $CONTAINER on port $MINIO_PORT, over $DATA."
  minio_run "$CONTAINER" "$MINIO_PORT" "$ACCESS_KEY" "$SECRET_KEY" \
    --user "$(id -u):$(id -g)" \
    --env "HOME=/tmp" \
    --volume "$DATA:$MINIO_DATA_IN_CONTAINER" ||
    fail "MinIO could not be started, and the line above is docker's own account of why."
  minio_await "$CONTAINER" "$MINIO_PORT" "$STARTUP_TIMEOUT_SECONDS" ||
    fail "MinIO did not become healthy within ${STARTUP_TIMEOUT_SECONDS}s, and its own output is above."
}

stop_minio() {
  command -v docker >/dev/null 2>&1 || return 0
  [ -n "$(container_state)" ] || return 0
  if ! container_is_ours; then
    echo "Left $CONTAINER alone: it keeps its data somewhere other than $DATA."
    return 0
  fi
  docker rm --force "$CONTAINER" >/dev/null
  echo "Stopped MinIO ($CONTAINER); its data is kept in $DATA."
}

down() {
  "$ROOT/scripts/dev.sh" down "$LIBRARY" "${DEV_OPTIONS[@]}"
  stop_minio
}

need() {
  local tool
  for tool in "$@"; do
    command -v "$tool" >/dev/null 2>&1 ||
      fail "$tool is needed and was not found on PATH."
  done
  docker info >/dev/null 2>&1 ||
    fail "docker is on PATH but does not answer; start Docker first."
}

# The command line, for the seed: the Passphrase on its standard input, and
# what it says kept in the seed's log.
run_cli() {
  printf '%s\n' "$PASSPHRASE" |
    "$COFFRET" --json "$@" >>"$SEED_LOG" 2>&1 ||
    fail "\`coffret $1\` failed while seeding. It said:
$(tail -n 15 "$SEED_LOG")
The rest is in $SEED_LOG; the next \`make large-seed\` starts the seed again."
}

seed() {
  local started dir unit units=0 total
  need docker curl cargo
  if [ -e "$SEEDED" ]; then
    echo "The large Library under $WORK is seeded already ($(cat "$SEEDED")s, when it was)."
    echo "\`make large-measure\` measures it; \`make large-reset\` removes it."
    return 0
  fi
  if [ -e "$WORK" ]; then
    echo "The Library under $WORK was never finished seeding; starting it again."
    down >/dev/null
    rm -rf "$WORK"
  fi
  mkdir -p "$WORK" "$LOG_DIR" "$STATE" "$FOLDERS"
  trap stop_minio EXIT

  echo "--- building the command line and the fixture generator ---"
  (cd "$ROOT/backend" && cargo build --release -p coffret-cli -p coffret-fixtures)

  # Timed from here, so that what the seed is said to take is the seed's own,
  # and not however much of the build was left to do.
  started="$(date +%s)"

  start_minio
  minio_create_bucket "$CONTAINER" "$ACCESS_KEY" "$SECRET_KEY" "$BUCKET" ||
    fail "the bucket $BUCKET could not be made in MinIO, and the line above is the AWS CLI's own account of why."

  echo "Creating the Library \"$LIBRARY\"."
  run_cli init \
    --name "$LIBRARY" \
    --s3 \
    --bucket "$BUCKET" \
    --endpoint "http://127.0.0.1:$MINIO_PORT" \
    --path-style \
    --passphrase-stdin
  printf '%s\n' "$MINIO_PORT" >"$SEEDED_PORT"

  echo "Generating what goes in it."
  "$FIXTURES" --scenario large --out "$FOLDERS" >"$UNITS"

  for dir in "$FOLDERS"/*/; do
    dir="${dir%/}"
    run_cli map --library "$LIBRARY" --prefix "$(basename "$dir")" "$dir"
  done

  total="$(grep -c . "$UNITS")"
  while IFS= read -r unit <&3; do
    [ -n "$unit" ] || continue
    run_cli freeze --library "$LIBRARY" --under "$unit" --passphrase-stdin
    units=$((units + 1))
    printf '\rFroze %d of %d folders into Packs (%ds so far).' \
      "$units" "$total" "$(($(date +%s) - started))"
  done 3<"$UNITS"
  echo

  echo "$(($(date +%s) - started))" >"$SEEDED"
  echo "Seeded in $(cat "$SEEDED")s, not counting the build."
}

# The launch file the development host writes for the Library, which a tab
# opens to hold a session.
launch_dir() {
  local found
  found="$(find "$STATE/libraries" -mindepth 2 -maxdepth 2 -type d -name explorer-launch | head -n 1)"
  [ -n "$found" ] || fail "no explorer-launch directory under $STATE/libraries."
  echo "$found"
}

measure() {
  local index seeded_port
  need docker curl cargo node pnpm
  [ -e "$SEEDED" ] || fail "there is no seeded Library under $WORK; \`make large-seed\` makes one."
  seeded_port="$(cat "$SEEDED_PORT")"
  [ "$seeded_port" = "$MINIO_PORT" ] ||
    fail "the Library under $WORK was created against MinIO on port $seeded_port, and its settings
name that port. Pass LARGE_MINIO_PORT=$seeded_port."

  # The server and the host are built by scripts/dev.sh as it starts them, each
  # on its own, as `make local` builds them: built here together, they would
  # be built a second time there with the features of each alone.
  echo "--- building the harness ---"
  (cd "$ROOT/backend" && cargo build --release -p coffret-measure)
  if [ -z "${COFFRET_CHROMIUM:-}" ]; then
    (cd "$ROOT/frontend" && pnpm --filter @coffret/e2e exec playwright install chromium)
  fi

  index="$(find "$STATE/libraries" -mindepth 2 -maxdepth 2 -name index.sqlite | head -n 1)"
  [ -n "$index" ] || fail "no Index under $STATE/libraries."

  trap down EXIT
  {
    echo "=== the explorer against a Library of a hundred thousand files ==="
    echo
    echo "Machine: $(uname -sm), $(nproc 2>/dev/null || sysctl -n hw.ncpu) CPUs"
    echo "Seeded in $(cat "$SEEDED")s, not counting the build."
    echo
  } >"$REPORT"

  echo
  echo "--- 1. the full-scan match, over the Index alone ---"
  "$MEASURE" --index "$index" "${QUERIES[@]}" --recent 5 --recent 50 \
    --largest-folder-out "$LARGEST" | tee -a "$REPORT"

  echo
  echo "--- 2 and 3. the listing and the tree, in the explorer ---"
  start_minio
  "$ROOT/scripts/dev.sh" up "$LIBRARY" "$PORT" "$WEB_PORT" "${DEV_OPTIONS[@]}" --passphrase-stdin \
    <<<"$PASSPHRASE"

  (
    cd "$ROOT/frontend"
    COFFRET_LARGE_WEB_URL="http://127.0.0.1:$WEB_PORT/" \
      COFFRET_LARGE_LAUNCH_DIR="$(launch_dir)" \
      COFFRET_LARGE_FOLDER="$(cat "$LARGEST")" \
      COFFRET_LARGE_ROWS="$ROWS" \
      COFFRET_LARGE_PRESSES="$PRESSES" \
      COFFRET_LARGE_REPORT="$REPORT" \
      COFFRET_LARGE_ARTIFACTS="$WORK/playwright" \
      pnpm --filter @coffret/e2e exec playwright test --config scale/playwright.config.ts
  )

  echo
  echo "--- the report, also in $REPORT ---"
  cat "$REPORT"
}

reset() {
  down
  rm -rf "$WORK"
  echo "Removed $WORK; the next \`make large-seed\` seeds a fresh Library."
}

case "$COMMAND" in
  seed) seed ;;
  measure) measure ;;
  down) down ;;
  reset) reset ;;
esac
