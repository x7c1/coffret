#!/usr/bin/env bash
#
# A Library to work on the explorer against, on this machine, from one command.
#
# Working on the explorer's interface needs a Library to look at, and every
# other way to get one costs something: `make dev` serves a Library somebody
# first created on Google Drive or S3 and filled by hand, and `make e2e-it`
# builds one on MinIO and tears it down again. `make local` is a Library on a
# MinIO container on this machine, created and filled with a varied set of
# books and albums the first time it starts, and kept across restarts until it
# is reset on purpose.
#
#   scripts/local.sh up <port> <web-port> <minio-port>
#   scripts/local.sh down
#   scripts/local.sh reset
#
# `up` starts MinIO, seeds the Library where there is none yet, and then starts
# the server, the explorer's build in watch mode and the development host over
# it, in the background, exactly as `make dev` does — by scripts/dev.sh, which
# prints the launch file to open. `down` stops the three and MinIO, and keeps
# everything else; `down` then `up` is the restart, which is also what a server
# that has locked itself after idling needs. `reset` is `down`, then the whole
# Library thrown away, so that the next `up` seeds a fresh one.
#
# Everything lives under .tmp/local/, and nothing of it anywhere else but in
# the container, which keeps nothing of its own:
#
#   minio/     MinIO's data, bind-mounted into the container, so that it
#              outlives the container being removed and made again
#   state/     the device's state directory: the Library's catalog, its keys
#   library/   the folders the Library is mapped to on this device, one per
#              top-level part
#   logs/      the binaries' logs, and seed.log, what the seed's commands said
#   run/       the pid files and the output of the three processes
#   minio-port the port MinIO was published on when the Library was created,
#              which its settings name as the endpoint
#   freeze-units  the folders the seed froze into Packs, one per line
#   seeded     there once the seed has finished, and what says it has
#
# A seed that was cut short leaves no `seeded` behind, and the next `up` throws
# the half-made Library away and seeds it again; there is nothing in one a
# person could have changed, since the explorer is started only after it.
#
# What the seed holds is laid out in one place: the `seed` scenario of
# `coffret-fixtures` (backend/crates/apps/coffret-fixtures/src/seed.rs), which
# also says which folders are frozen into Packs. Books are carried in by
# `freeze`, as a book dropped in the explorer is, and the albums by `sync`.
#
# The Passphrase is written below, in the clear, and printed by `up`, and the
# server is handed it on its standard input instead of asking on the terminal.
# Nowhere else is that acceptable — `make dev`'s Passphrase is never seen by a
# script — and it is here because of what this Library is: generated pictures
# of digits, in a bucket on a container bound to the loopback address, under
# a directory that is thrown away by `reset`. A Library like this one that
# asked for its Passphrase on every restart would ask a person to type a string
# that protects nothing. Nothing you would keep belongs in it.
#
# The ports are three arguments rather than fixed, and the Makefile's defaults
# are apart from `make dev`'s and `make e2e-it`'s, so that all three can run
# side by side. The container's name is apart from theirs too, and it is one
# per machine: a second checkout's `make local` finds this one's container and
# refuses rather than serving another checkout's data.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT

# The MinIO image and the AWS CLI image are pinned in scripts/lib/minio.sh,
# with the way the container is started and the bucket made.
# shellcheck source=scripts/lib/minio.sh
source "$ROOT/scripts/lib/minio.sh"

usage() {
  echo "usage: scripts/local.sh up <port> <web-port> <minio-port> | scripts/local.sh down | scripts/local.sh reset" >&2
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
  up)
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

readonly WORK="$ROOT/.tmp/local"
readonly DATA="$WORK/minio"
readonly STATE="$WORK/state"
readonly FOLDERS="$WORK/library"
readonly LOG_DIR="$WORK/logs"
readonly RUN_DIR="$WORK/run"
readonly SEEDED="$WORK/seeded"
readonly SEED_LOG="$LOG_DIR/seed.log"
readonly SEEDED_PORT="$WORK/minio-port"
readonly COFFRET="$ROOT/backend/target/release/coffret"
readonly FIXTURES="$ROOT/backend/target/release/coffret-fixtures"

CONTAINER="${COFFRET_LOCAL_MINIO_CONTAINER:-coffret-minio-local}"
readonly CONTAINER

# The Library's name on this device, which is also what scripts/dev.sh starts
# the server and the host for.
readonly LIBRARY="local"

# Credentials that reach nothing but the container this script starts. MinIO
# refuses a root password shorter than eight characters.
readonly ACCESS_KEY="coffret-local"
readonly SECRET_KEY="coffret-local-secret"
readonly BUCKET="coffret-local"

# The Passphrase, fixed and in the clear: the header says why that is
# acceptable for this Library and for no other.
readonly PASSPHRASE="a coffret Library of generated files"

readonly STARTUP_TIMEOUT_SECONDS=60

# This script's own directories, whatever the Makefile or the environment says:
# the Library it serves lives here and nowhere else, and the development state
# directory is never reached.
export COFFRET_STATE_DIR="$STATE"
export COFFRET_LOG_DIR="$LOG_DIR"

# What the Library signs with. A Library's settings say where its bucket is and
# never how to sign for it, so the command line and the server take the
# credentials from the SDK's own resolution, which reads the environment first.
export AWS_ACCESS_KEY_ID="$ACCESS_KEY"
export AWS_SECRET_ACCESS_KEY="$SECRET_KEY"
export AWS_REGION="us-east-1"

# What `make local` and `make local-down` hand to scripts/dev.sh: where its pid
# files and logs go, and the command that stops what it started.
readonly DEV_OPTIONS=(--run-dir "$RUN_DIR" --down "make local-down")

# The container's state as docker reports it — `running`, `exited` and the
# rest — or nothing where there is no container by that name.
container_state() {
  docker inspect --format '{{.State.Status}}' "$CONTAINER" 2>/dev/null || true
}

# Whether the container by that name keeps its data in this checkout's
# directory: one from another checkout's `make local` has the same name.
container_is_ours() {
  local mounted
  mounted="$(docker inspect --format \
    '{{range .Mounts}}{{if eq .Destination "'"$MINIO_DATA_IN_CONTAINER"'"}}{{.Source}}{{end}}{{end}}' \
    "$CONTAINER" 2>/dev/null || true)"
  [ "$mounted" = "$DATA" ]
}

# The host port the container publishes MinIO's on, or nothing.
container_port() {
  docker inspect --format '{{range $p, $b := .NetworkSettings.Ports}}{{if eq $p "9000/tcp"}}{{range $b}}{{.HostPort}}{{end}}{{end}}{{end}}' \
    "$CONTAINER" 2>/dev/null || true
}

refuse_a_container_not_ours() {
  [ -z "$(container_state)" ] || container_is_ours ||
    fail "a container named $CONTAINER is there, and it keeps its data somewhere other than
$DATA — most likely another checkout's \`make local\`. Stop it from there with
\`make local-down\`, or set COFFRET_LOCAL_MINIO_CONTAINER to another name."
}

# The Library's settings name MinIO's endpoint, port and all, as `init` was
# given it, so a seeded Library reaches MinIO on that port and on no other.
refuse_another_minio_port() {
  local seeded_port
  seeded_port="$(cat "$SEEDED_PORT" 2>/dev/null || true)"
  [ -z "$seeded_port" ] || [ "$seeded_port" = "$MINIO_PORT" ] ||
    fail "the Library under $WORK was created against MinIO on port $seeded_port, and its settings
name that port. Pass LOCAL_MINIO_PORT=$seeded_port; or \`make local-reset\` to seed a fresh
Library on port $MINIO_PORT."
}

# Starts MinIO over $DATA, or finds it already running.
start_minio() {
  refuse_a_container_not_ours
  case "$(container_state)" in
    running)
      [ "$(container_port)" = "$MINIO_PORT" ] ||
        fail "MinIO is running in $CONTAINER on port $(container_port), not $MINIO_PORT.
\`make local-down\` stops it; or pass LOCAL_MINIO_PORT=$(container_port)."
      echo "MinIO is already running in $CONTAINER on port $MINIO_PORT."
      return 0
      ;;
    '') ;;
    *)
      # Stopped some other way than `down`, which removes it: what it held is
      # all in $DATA, so it is made again rather than restarted.
      docker rm --force "$CONTAINER" >/dev/null
      ;;
  esac

  mkdir -p "$DATA"
  echo "Starting MinIO in $CONTAINER on port $MINIO_PORT, over $DATA."
  # As this account rather than as the image's own user, so that what MinIO
  # writes into the mounted directory is this account's to read and to remove;
  # with a home MinIO can write its own small configuration under, since an
  # account the image does not know has none.
  minio_run "$CONTAINER" "$MINIO_PORT" "$ACCESS_KEY" "$SECRET_KEY" \
    --user "$(id -u):$(id -g)" \
    --env "HOME=/tmp" \
    --volume "$DATA:$MINIO_DATA_IN_CONTAINER" ||
    fail "MinIO could not be started, and the line above is docker's own account of why."
  minio_await "$CONTAINER" "$MINIO_PORT" "$STARTUP_TIMEOUT_SECONDS" ||
    fail "MinIO did not become healthy within ${STARTUP_TIMEOUT_SECONDS}s, and its own output is above."
}

# Removes the container, where it is this checkout's; its data stays in $DATA.
stop_minio() {
  [ -n "$(container_state)" ] || return 0
  if ! container_is_ours; then
    echo "Left $CONTAINER alone: it keeps its data somewhere other than $DATA."
    return 0
  fi
  docker rm --force "$CONTAINER" >/dev/null
  echo "Stopped MinIO ($CONTAINER); its data is kept in $DATA."
}

# Runs the command line for the seed, with the Passphrase on its standard input
# for the commands that read it. What it says is kept in the seed's log rather
# than shown: `init`'s advice to write the Recovery Code down, above all, is for
# a Library somebody keeps, and is not true of this one. A failure quotes the
# end of it.
run_cli() {
  printf '%s\n' "$PASSPHRASE" |
    "$COFFRET" --json "$@" >>"$SEED_LOG" 2>&1 ||
    fail "\`coffret $1\` failed while seeding. It said:
$(tail -n 15 "$SEED_LOG")
The rest is in $SEED_LOG; the next \`make local\` starts the seed again."
}

# Creates the Library and fills it. Runs only over an empty $WORK.
seed() {
  local started dir unit units=0
  started="$(date +%s)"
  echo
  echo "--- seeding a fresh Library under $WORK ---"
  (cd "$ROOT/backend" && cargo build --release -p coffret-cli -p coffret-fixtures)

  # `init` checks that the bucket answers and never makes one, so it is made
  # here. MinIO's data was empty a moment ago, so it is not there already.
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
  # The folders to freeze come out on standard output, one per line.
  "$FIXTURES" --scenario seed --out "$FOLDERS" >"$WORK/freeze-units"

  # Every folder the generator wrote at the top is a top-level part of the
  # Library, mapped under its own name.
  for dir in "$FOLDERS"/*/; do
    dir="${dir%/}"
    run_cli map --library "$LIBRARY" --prefix "$(basename "$dir")" "$dir"
  done

  # Read on a descriptor of its own, so that nothing in the loop reads a unit
  # off the list instead of its own input.
  while IFS= read -r unit <&3; do
    [ -n "$unit" ] || continue
    run_cli freeze --library "$LIBRARY" --under "$unit" --passphrase-stdin
    units=$((units + 1))
    printf '\rFroze %d book folder(s) into Packs.' "$units"
  done 3<"$WORK/freeze-units"
  echo

  echo "Syncing the rest: the albums."
  run_cli sync --library "$LIBRARY" --passphrase-stdin

  touch "$SEEDED"
  echo "Seeded in $(($(date +%s) - started))s."
}

up() {
  for tool in docker curl cargo node; do
    command -v "$tool" >/dev/null 2>&1 ||
      fail "$tool is needed and was not found on PATH."
  done
  docker info >/dev/null 2>&1 ||
    fail "docker is on PATH but does not answer; start Docker first."

  if [ ! -e "$SEEDED" ]; then
    refuse_a_container_not_ours
    if [ -e "$WORK" ]; then
      echo "The Library under $WORK was never finished seeding; starting it again."
      "$ROOT/scripts/dev.sh" down "$LIBRARY" "${DEV_OPTIONS[@]}" >/dev/null
      stop_minio >/dev/null
      rm -rf "$WORK"
    fi
    mkdir -p "$WORK" "$LOG_DIR" "$STATE" "$FOLDERS"
    start_minio
    seed
  else
    refuse_another_minio_port
    start_minio
  fi

  echo
  echo "The Passphrase is \"$PASSPHRASE\": fixed, for a Library of generated files."
  "$ROOT/scripts/dev.sh" up "$LIBRARY" "$PORT" "$WEB_PORT" "${DEV_OPTIONS[@]}" --passphrase-stdin \
    <<<"$PASSPHRASE"
}

down() {
  "$ROOT/scripts/dev.sh" down "$LIBRARY" "${DEV_OPTIONS[@]}"
  if command -v docker >/dev/null 2>&1; then
    stop_minio
  fi
}

reset() {
  down
  rm -rf "$WORK"
  echo "Removed $WORK; the next \`make local\` seeds a fresh Library."
}

case "$COMMAND" in
  up) up ;;
  down) down ;;
  reset) reset ;;
esac
