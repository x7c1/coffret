#!/usr/bin/env bash
#
# The desktop app's journey, end to end: the Electron app itself, unpackaged,
# over a Library on MinIO.
#
# The browser journeys (`make e2e-it`, scripts/e2e-it.sh) walk the explorer in
# Chromium behind the development host. What only the desktop app does — the
# Passphrase window its door serves, the explorer in a window of its own, a
# drop taken by path, the idle lock and the unlock in place, and a quit that
# leaves nothing running — is walked here, by Playwright's Electron driver,
# in `frontend/packages/apps/e2e/electron/`.
#
# This builds what the app runs from a checkout (`make desktop-dev-build`: the
# explorer, the Node-API addon with the explorer compiled in, and the app's
# code), starts MinIO in Docker as scripts/e2e-it.sh does, creates a Library on
# it with the command line under a state directory of its own, and hands the
# suite the rest. The app is started with that state directory, a browser-data
# directory of its own and a one-minute idle interval, so it touches neither
# the development state directory nor an installed app's data, and runs beside
# either.
#
# On Linux the app is started with `--no-sandbox`, for this run alone: the
# Electron binary of the npm package carries a `chrome-sandbox` that is not
# setuid root, and Ubuntu 24.04 refuses the unprivileged user namespaces that
# would stand in for it. Nothing in the app or its bundle turns the sandbox off.
# On a Linux machine with no display, run this under `xvfb-run`, as CI does.
#
# Nothing is left behind but the pictures under .tmp/e2e-electron/screenshots/.
# MinIO is removed on the way out, and .tmp/e2e-electron/ is made again on
# every run.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT

readonly WORK="$ROOT/.tmp/e2e-electron"
readonly SCREENSHOTS="$WORK/screenshots"
readonly LOG_DIR="$WORK/logs"
readonly ARTIFACTS="$WORK/playwright"
readonly APP_DIR="$ROOT/frontend/packages/apps/desktop"

# A container and a port of its own, apart from scripts/e2e-it.sh's, so that
# the two targets can run one after the other without either finding the
# other's container in its way.
CONTAINER="${COFFRET_E2E_ELECTRON_MINIO_CONTAINER:-coffret-minio-e2e-electron}"
MINIO_PORT="${COFFRET_E2E_ELECTRON_MINIO_PORT:-19011}"
# The image scripts/e2e-it.sh and scripts/s3-store-it.sh pin, for the reason
# given there: MinIO no longer publishes one, and this is its last release as
# the alpine-docker project builds it, by digest.
IMAGE="${COFFRET_E2E_MINIO_IMAGE:-docker.io/alpine/minio@sha256:cf23643a6cf9ce159c57643ceb88279e431262282428c9e0bf3a7ef1a97e84b4}"
# The AWS CLI the bucket is made with, pinned by digest as in scripts/e2e-it.sh.
AWS_CLI_IMAGE="${COFFRET_E2E_AWS_CLI_IMAGE:-docker.io/amazon/aws-cli@sha256:83f8ffe939569070c5b66d22231862ab78718766d9d8e4c44ca84dd0be5569a5}"

# Credentials that only ever reach a container this script starts and stops.
readonly ACCESS_KEY="coffret-e2e"
readonly SECRET_KEY="coffret-e2e-secret"
readonly BUCKET="coffret-e2e-electron"

# A fixed test string, protecting a Library of generated JPEGs that is thrown
# away with the rest of .tmp/e2e-electron/.
readonly PASSPHRASE="a coffret desktop journey against MinIO"

readonly LIBRARY="main"
readonly PREFIX="runs"
readonly ALBUM="$PREFIX/album-000"
readonly FIRST_PHOTO="img-00000.jpg"

# The shortest idle interval the app takes: a whole number of minutes, at
# least one.
readonly IDLE_MINUTES=1

readonly STARTUP_TIMEOUT_SECONDS=60

fail() {
  echo "$*" >&2
  exit 1
}

for tool in docker curl jq pnpm cargo node; do
  command -v "$tool" >/dev/null 2>&1 ||
    fail "$tool is needed to run the desktop app's journey and is not on this PATH."
done

teardown() {
  docker rm --force --volumes "$CONTAINER" >/dev/null 2>&1 || true
}

# Also clears a container left behind by a run that was killed outright.
trap teardown EXIT
teardown

rm -rf "$WORK"
readonly STATE_DIR="$WORK/state"
readonly USER_DATA_DIR="$WORK/user-data"
readonly MAPPED_ROOT="$WORK/$PREFIX"
readonly SPARE="$WORK/spare"
readonly DROP_FILE="$WORK/dropped/dropped.jpg"
mkdir -p "$WORK" "$LOG_DIR" "$SCREENSHOTS" "$STATE_DIR" "$USER_DATA_DIR" \
  "$MAPPED_ROOT" "$SPARE" "$(dirname "$DROP_FILE")"

echo "=== the desktop app's journey, against MinIO ==="
echo
echo "Working directory:  $WORK"
echo "Screenshots:        $SCREENSHOTS"
echo "Logs:               $LOG_DIR"
echo

# ---------------------------------------------------------------------------
# What the run needs, built once and up front.
# ---------------------------------------------------------------------------

echo "--- building the command line, the fixtures and the desktop app ---"
(cd "$ROOT/backend" && cargo build --release -p coffret-cli -p coffret-fixtures)
readonly COFFRET="$ROOT/backend/target/release/coffret"
readonly FIXTURES="$ROOT/backend/target/release/coffret-fixtures"
make --no-print-directory -C "$ROOT" desktop-dev-build

# The Electron binary the desktop package runs on, fetched the first time it is
# asked for: the package resolves to its path, downloading it where it is not
# there yet. A download announces itself on standard output, so the first
# resolution, which may download, goes to standard error, and only the second,
# which cannot, is read as the path.
(cd "$APP_DIR" && node -e "require('electron')") >&2 ||
  fail "the desktop package's Electron binary could not be found or fetched."
ELECTRON_BIN="$(cd "$APP_DIR" && node -p "require('electron')")" ||
  fail "the desktop package's Electron binary could not be found or fetched."
[ -x "$ELECTRON_BIN" ] || fail "the Electron binary is not at $ELECTRON_BIN."
readonly ELECTRON_BIN

ELECTRON_SWITCHES=""
if [ "$(uname -s)" = Linux ]; then
  ELECTRON_SWITCHES="--no-sandbox"
fi
readonly ELECTRON_SWITCHES

# ---------------------------------------------------------------------------
# MinIO, as scripts/e2e-it.sh starts it.
# ---------------------------------------------------------------------------

echo
echo "--- starting MinIO in $CONTAINER on port $MINIO_PORT ---"
docker run --detach \
  --name "$CONTAINER" \
  --publish "127.0.0.1:${MINIO_PORT}:9000" \
  --env "MINIO_ROOT_USER=${ACCESS_KEY}" \
  --env "MINIO_ROOT_PASSWORD=${SECRET_KEY}" \
  "$IMAGE" server /home/minio/data >/dev/null ||
  fail "MinIO could not be started, and the line above is docker's own account of why."

for _ in $(seq "$STARTUP_TIMEOUT_SECONDS"); do
  if curl --fail --silent --show-error "http://127.0.0.1:${MINIO_PORT}/minio/health/live" >/dev/null 2>&1; then
    ready=1
    break
  fi
  sleep 1
done
[ "${ready:-}" = 1 ] || {
  docker logs "$CONTAINER" >&2 || true
  fail "MinIO did not become healthy within ${STARTUP_TIMEOUT_SECONDS}s"
}

docker run --rm \
  --network "container:${CONTAINER}" \
  --env "AWS_ACCESS_KEY_ID=${ACCESS_KEY}" \
  --env "AWS_SECRET_ACCESS_KEY=${SECRET_KEY}" \
  --env "AWS_REGION=us-east-1" \
  "$AWS_CLI_IMAGE" s3api create-bucket --bucket "$BUCKET" --endpoint-url "http://127.0.0.1:9000" >/dev/null ||
  fail "the bucket ${BUCKET} could not be created in MinIO."

# What the Library signs with, the command line's and the app's alike: the
# SDK's own resolution reads the environment first.
export AWS_ACCESS_KEY_ID="$ACCESS_KEY"
export AWS_SECRET_ACCESS_KEY="$SECRET_KEY"
export AWS_REGION="us-east-1"
readonly ENDPOINT="http://127.0.0.1:${MINIO_PORT}"

# ---------------------------------------------------------------------------
# One Library, on one device.
# ---------------------------------------------------------------------------

# The command line, under the run's state directory, with the Passphrase on
# its standard input and the answer kept as JSON.
run_cli() {
  printf '%s\n' "$PASSPHRASE" |
    COFFRET_STATE_DIR="$STATE_DIR" COFFRET_LOG_DIR="$LOG_DIR" "$COFFRET" --json "$@" \
      >"$WORK/last-answer.json"
}

echo
echo "--- creating the Library $LIBRARY ---"
run_cli init --name "$LIBRARY" --s3 --bucket "$BUCKET" --endpoint "$ENDPOINT" \
  --path-style --passphrase-stdin || fail "the Library was not created."
echo "(The Recovery Code init printed is for a Library thrown away with this run.)"

"$FIXTURES" --out "$MAPPED_ROOT" --photos 3 --pages 0
"$FIXTURES" --out "$SPARE" --photos 1 --pages 0
cp "$SPARE/album-000/img-00000.jpg" "$DROP_FILE"

run_cli map --library "$LIBRARY" --prefix "$PREFIX" "$MAPPED_ROOT" ||
  fail "$PREFIX could not be mapped."
run_cli sync --library "$LIBRARY" --passphrase-stdin || fail "the first sync failed."
jq --exit-status '.answer.committed_head' "$WORK/last-answer.json" >/dev/null ||
  fail "the first sync committed nothing."

# ---------------------------------------------------------------------------
# The journey, in the app.
# ---------------------------------------------------------------------------

echo
echo "--- the desktop app, over $LIBRARY ---"
echo "It waits out a ${IDLE_MINUTES}-minute idle lock on the way."
echo
cd "$ROOT/frontend"
COFFRET_E2E_ELECTRON_BIN="$ELECTRON_BIN" \
  COFFRET_E2E_APP_DIR="$APP_DIR" \
  COFFRET_E2E_ELECTRON_SWITCHES="$ELECTRON_SWITCHES" \
  COFFRET_E2E_STATE_DIR="$STATE_DIR" \
  COFFRET_E2E_LOG_DIR="$LOG_DIR" \
  COFFRET_E2E_USER_DATA_DIR="$USER_DATA_DIR" \
  COFFRET_E2E_LIBRARY="$LIBRARY" \
  COFFRET_E2E_PASSPHRASE="$PASSPHRASE" \
  COFFRET_E2E_MAPPED="$PREFIX" \
  COFFRET_E2E_ALBUM="$ALBUM" \
  COFFRET_E2E_FIRST_PHOTO="$FIRST_PHOTO" \
  COFFRET_E2E_DROP_FILE="$DROP_FILE" \
  COFFRET_E2E_IDLE_MINUTES="$IDLE_MINUTES" \
  COFFRET_E2E_ARTIFACTS="$ARTIFACTS" \
  COFFRET_E2E_SCREENSHOTS="$SCREENSHOTS" \
  pnpm --filter @coffret/e2e run test:electron

echo
echo "=== the desktop app's journey held ==="
echo
echo "Screenshots:  $SCREENSHOTS"
echo "Logs:         $LOG_DIR"
