# shellcheck shell=bash
#
# MinIO in Docker, as every script here that needs real S3 runs it: the image,
# the way the container is started and waited on, and the way a bucket is made
# in it. Sourced, not run, by scripts/s3-store-it.sh, scripts/e2e-it.sh,
# scripts/e2e-electron.sh, scripts/local.sh and scripts/large.sh, so that every
# one of them runs the same images, pinned under docker/ (see below).
#
# Each function answers with its status and leaves what to say about a failure
# to the script, since what a failure means differs: a suite that never ran,
# or a local Library that is not up.

# The images are pinned each in the FROM line of a Dockerfile of its own,
# docker/minio/Dockerfile and docker/aws-cli/Dockerfile, which say why each is
# the image it is and why it is pinned by digest. They are kept there rather
# than here because Dependabot reads image references only from Dockerfiles and
# Compose files; this reads them back, so that each file is the one place its
# image is pinned. Nothing builds either file.
#
# Prints the image reference in the one FROM line of docker/$1/Dockerfile,
# which has to be `<image>:<tag>@sha256:<digest>`. Where the file is missing,
# or holds no such line or more than one, says so on standard error and fails,
# so that a script sourcing this stops rather than running some other image.
pinned_image() {
  local dockerfile line ref="" count=0
  dockerfile="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/docker/$1/Dockerfile"
  if [ ! -f "$dockerfile" ]; then
    echo "scripts/lib/minio.sh: $dockerfile, which pins the $1 image, is missing." >&2
    return 1
  fi
  while IFS= read -r line || [ -n "$line" ]; do
    if [[ "$line" =~ ^FROM[[:space:]]+([^[:space:]]+)[[:space:]]*$ ]]; then
      ref="${BASH_REMATCH[1]}"
      count=$((count + 1))
    fi
  done <"$dockerfile"
  if [ "$count" -ne 1 ] || ! [[ "$ref" =~ ^[a-z0-9./_-]+:[A-Za-z0-9_.-]+@sha256:[0-9a-f]{64}$ ]]; then
    echo "scripts/lib/minio.sh: $dockerfile has to hold exactly one line \`FROM <image>:<tag>@sha256:<digest>\`, and holds $count FROM line(s)${ref:+, the last naming $ref}." >&2
    return 1
  fi
  printf '%s\n' "$ref"
}

# COFFRET_MINIO_IMAGE and COFFRET_AWS_CLI_IMAGE, where set, name another image
# to run in place of the pinned one, and the Dockerfile is then not read.
MINIO_IMAGE="${COFFRET_MINIO_IMAGE:-$(pinned_image minio)}"
AWS_CLI_IMAGE="${COFFRET_AWS_CLI_IMAGE:-$(pinned_image aws-cli)}"

# Where MinIO keeps its data inside the container. The image runs MinIO as an
# unprivileged user, so this is under that user's home rather than at the root
# of the filesystem. A script that keeps the data past the container mounts a
# directory of its own here.
readonly MINIO_DATA_IN_CONTAINER="/home/minio/data"

# Starts MinIO in the background, in a container named $1, published on
# 127.0.0.1:$2, with $3 and $4 as its root user and password. MinIO refuses a
# password shorter than eight characters. Any further arguments go to
# `docker run` ahead of the image, a `--volume` for the data directory, say.
# Says nothing of its own where it succeeds; where it fails, docker's own
# account of why is what is on standard error.
minio_run() {
  local container="$1" port="$2" access_key="$3" secret_key="$4"
  shift 4
  docker run --detach \
    --name "$container" \
    --publish "127.0.0.1:${port}:9000" \
    --env "MINIO_ROOT_USER=${access_key}" \
    --env "MINIO_ROOT_PASSWORD=${secret_key}" \
    "$@" \
    "$MINIO_IMAGE" server "$MINIO_DATA_IN_CONTAINER" >/dev/null
}

# minio_run, in the host's own network namespace: MinIO in a container named
# $1 listening on 127.0.0.1:$2 itself, with $3 and $4 as its root user and
# password, its web console off, and any further arguments going to
# `docker run` ahead of the image.
#
# For a MinIO that is stopped and started while a browser is open, as the
# e2e journeys do to take Storage away and give it back. A container on
# Docker's bridge network brings a veth interface onto the host each time it
# starts, and takes it away each time it stops; the browser sees the host's
# interfaces change — the new interface's IPv6 link-local address arrives a
# second or two after the start — and aborts every request it has in flight
# with `net::ERR_NETWORK_CHANGED`, the explorer's own loopback requests
# included. A container in the host's namespace adds and removes no
# interface. Listening on the loopback address keeps it off every other
# interface, as the bridge's `--publish 127.0.0.1:…` did, and the console is
# turned off rather than left to listen on a port of its choosing on all of
# them.
minio_run_on_host() {
  local container="$1" port="$2" access_key="$3" secret_key="$4"
  shift 4
  docker run --detach \
    --name "$container" \
    --network host \
    --env "MINIO_ROOT_USER=${access_key}" \
    --env "MINIO_ROOT_PASSWORD=${secret_key}" \
    --env "MINIO_BROWSER=off" \
    "$@" \
    "$MINIO_IMAGE" server --address "127.0.0.1:${port}" "$MINIO_DATA_IN_CONTAINER" >/dev/null
}

# Waits up to $3 seconds for the MinIO on 127.0.0.1:$2 to answer its
# health check. Where it does not, the container $1's own output is put on
# standard error before the failure is answered, since that output is what
# says why.
minio_await() {
  local container="$1" port="$2" seconds="$3" _
  for _ in $(seq "$seconds"); do
    if curl --fail --silent "http://127.0.0.1:${port}/minio/health/live" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  docker logs "$container" >&2 || true
  return 1
}

# Makes the bucket $4 in the MinIO in container $1, signing as $2 with $3.
# The CLI's container joins MinIO's network namespace, so that the loopback
# address MinIO listens on is the one the CLI reaches: port 9000 inside a
# container minio_run started, or the port $5 names for one minio_run_on_host
# started, whose namespace is the host's. A bucket that is there already is a
# failure, as it is to the CLI.
minio_create_bucket() {
  local container="$1" access_key="$2" secret_key="$3" bucket="$4" port="${5:-9000}"
  docker run --rm \
    --network "container:${container}" \
    --env "AWS_ACCESS_KEY_ID=${access_key}" \
    --env "AWS_SECRET_ACCESS_KEY=${secret_key}" \
    --env "AWS_REGION=us-east-1" \
    "$AWS_CLI_IMAGE" s3api create-bucket --bucket "$bucket" --endpoint-url "http://127.0.0.1:${port}" >/dev/null
}
