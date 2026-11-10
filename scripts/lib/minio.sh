# shellcheck shell=bash
#
# MinIO in Docker, as every script here that needs real S3 runs it: the image,
# the way the container is started and waited on, and the way a bucket is made
# in it. Sourced, not run, by scripts/s3-store-it.sh, scripts/e2e-it.sh,
# scripts/e2e-electron.sh, scripts/local.sh and scripts/large.sh, so that the
# pins below are moved in one place and every one of them moves with it.
#
# Each function answers with its status and leaves what to say about a failure
# to the script, since what a failure means differs: a suite that never ran,
# or a local Library that is not up.

# MinIO itself no longer publishes an image: its Docker Hub repository is gone,
# its quay.io one answers every pull with `unauthorized`, and dl.min.io returns
# 410 for the release binaries. What is pinned here is the last upstream release
# (RELEASE.2025-10-15T17-29-55Z) as built from that source by the alpine-docker
# project, by digest rather than by tag so that a retagged or retracted image
# fails loudly instead of silently changing what runs against it. The image
# runs MinIO as an unprivileged user, so the data directory it is handed below
# is under that user's home rather than at the root of the filesystem.
MINIO_IMAGE="${COFFRET_MINIO_IMAGE:-docker.io/alpine/minio@sha256:cf23643a6cf9ce159c57643ceb88279e431262282428c9e0bf3a7ef1a97e84b4}"

# MinIO's own client is not in that image — only the image MinIO itself used to
# publish bundled it — so a bucket is made by the AWS CLI, in a container of its
# own. Pinned by digest for the reason the MinIO image is.
AWS_CLI_IMAGE="${COFFRET_AWS_CLI_IMAGE:-docker.io/amazon/aws-cli@sha256:83f8ffe939569070c5b66d22231862ab78718766d9d8e4c44ca84dd0be5569a5}"

# Where MinIO keeps its data inside the container. A script that keeps the data
# past the container mounts a directory of its own here.
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
