#!/usr/bin/env bash
#
# Run the ObjectStore, commit, sync, freeze, and fetch conformance suites, and
# the device-layer cases that open a Library, against a real S3 implementation.
#
# The suites need a server that actually evaluates `If-None-Match: *`, keeps
# continuation tokens, and reports ETags — none of which a mock proves. MinIO
# is that server, started here and torn down on the way out, so the target is
# self-contained: nothing is left running and no state carries between runs.

set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Overridable so a second run, or a developer whose 19000 is taken, does not
# collide with the first.
CONTAINER="${COFFRET_MINIO_CONTAINER:-coffret-minio-it}"
PORT="${COFFRET_MINIO_PORT:-19000}"
IMAGE="${COFFRET_MINIO_IMAGE:-quay.io/minio/minio:latest}"

# MinIO refuses a root password shorter than eight characters. These credentials
# only ever reach a container this script starts and stops.
readonly ACCESS_KEY="coffret-it"
readonly SECRET_KEY="coffret-it-secret"
readonly BUCKET="coffret-conformance"

# How long to wait for MinIO to answer its health check before giving up.
readonly STARTUP_TIMEOUT_SECONDS=60

teardown() {
  docker rm --force "$CONTAINER" >/dev/null 2>&1 || true
}

# Also clears a container left behind by a run that was killed outright.
trap teardown EXIT
teardown

echo "starting MinIO in $CONTAINER on port $PORT"
docker run --detach \
  --name "$CONTAINER" \
  --publish "127.0.0.1:${PORT}:9000" \
  --env "MINIO_ROOT_USER=${ACCESS_KEY}" \
  --env "MINIO_ROOT_PASSWORD=${SECRET_KEY}" \
  "$IMAGE" server /data >/dev/null

for _ in $(seq "$STARTUP_TIMEOUT_SECONDS"); do
  if curl --fail --silent --show-error "http://127.0.0.1:${PORT}/minio/health/live" >/dev/null 2>&1; then
    ready=1
    break
  fi
  sleep 1
done

if [ "${ready:-}" != 1 ]; then
  echo "MinIO did not become healthy within ${STARTUP_TIMEOUT_SECONDS}s" >&2
  docker logs "$CONTAINER" >&2 || true
  exit 1
fi

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
