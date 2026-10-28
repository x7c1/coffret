#!/usr/bin/env bash
#
# Run the ObjectStore, commit, sync, freeze, and fetch conformance suites, the
# device-layer cases that open a Library, and the command line's own round trip
# — `init`, `map`, `sync`, `freeze`, `join`, `fetch` over three Libraries in one
# bucket — against a real S3 implementation, with scripts/rename-smoke.sh run
# against Libraries in the same bucket.
#
# The suites need a server that actually evaluates `If-None-Match: *`, keeps
# continuation tokens, and reports ETags — none of which a mock proves. MinIO
# is that server, started here and torn down on the way out, so the target is
# self-contained: nothing is left running and no state carries between runs.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT

# Overridable so a second run, or a developer whose 19000 is taken, does not
# collide with the first.
CONTAINER="${COFFRET_MINIO_CONTAINER:-coffret-minio-it}"
PORT="${COFFRET_MINIO_PORT:-19000}"
# The MinIO image and the AWS CLI image are pinned in scripts/lib/minio.sh,
# with the way the container is started and the bucket made.
# shellcheck source=scripts/lib/minio.sh
source "$ROOT/scripts/lib/minio.sh"

# MinIO refuses a root password shorter than eight characters. These credentials
# only ever reach a container this script starts and stops.
readonly ACCESS_KEY="coffret-it"
readonly SECRET_KEY="coffret-it-secret"
readonly BUCKET="coffret-conformance"

# How long to wait for MinIO to answer its health check before giving up.
readonly STARTUP_TIMEOUT_SECONDS=60

# Says what stopped the run and exits non-zero, which is the only honest answer
# a suite that never ran can give. Every step that needs MinIO checks its own
# status and comes through here rather than leaving it to `set -e`: a shell that
# exits on an unchecked failure says nothing about what failed, and the last
# word of the run would be a line printed by docker about a socket.
fail() {
  echo "$*" >&2
  exit 1
}

command -v docker >/dev/null 2>&1 ||
  fail "docker is needed to run these suites against MinIO and is not on this PATH."

teardown() {
  docker rm --force --volumes "$CONTAINER" >/dev/null 2>&1 || true
}

# Also clears a container left behind by a run that was killed outright.
trap teardown EXIT
teardown

echo "starting MinIO in $CONTAINER on port $PORT"
minio_run "$CONTAINER" "$PORT" "$ACCESS_KEY" "$SECRET_KEY" ||
  fail "MinIO could not be started, and the line above is docker's own account of why.
None of the suites ran: they are here for what a real implementation decides about a
conditional create, a continuation token and an ETag, so a run without one is a
failure and never a pass."

minio_await "$CONTAINER" "$PORT" "$STARTUP_TIMEOUT_SECONDS" ||
  fail "MinIO did not become healthy within ${STARTUP_TIMEOUT_SECONDS}s, and its own output
is above. None of the suites ran."

# The COFFRET_S3_IT_* variables are what a test harness builds its own client
# from. The AWS_* ones are for what is under test: a Library's settings say
# where its bucket is and never how to sign for it, so opening one takes its
# credentials from the SDK's own resolution — which is the environment first,
# and this is that environment.
cd "$ROOT/backend"
COFFRET_S3_IT_ENDPOINT="http://127.0.0.1:${PORT}" \
  COFFRET_S3_IT_BUCKET="$BUCKET" \
  COFFRET_S3_IT_ACCESS_KEY="$ACCESS_KEY" \
  COFFRET_S3_IT_SECRET_KEY="$SECRET_KEY" \
  AWS_ACCESS_KEY_ID="$ACCESS_KEY" \
  AWS_SECRET_ACCESS_KEY="$SECRET_KEY" \
  AWS_REGION="us-east-1" \
  cargo test -p s3-store -p coffret-device -p coffret-cli
