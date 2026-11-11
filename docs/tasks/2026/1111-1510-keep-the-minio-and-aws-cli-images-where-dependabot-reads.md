---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it'
assignee: null
branch: task/1111-1510-keep-the-minio-and-aws-cli-images-where-dependabot-reads
created_at: 2026-11-11T06:10:00Z
updated_at: 2026-11-11T07:05:00Z
---

# build: keep the MinIO and AWS CLI images where Dependabot can read them

## Overview

`scripts/lib/minio.sh` pins the two container images the scripts run —
`MINIO_IMAGE` (`docker.io/alpine/minio@sha256:…`) and `AWS_CLI_IMAGE`
(`docker.io/amazon/aws-cli@sha256:…`) — as digests inside shell variables.
Dependabot's `docker` ecosystem reads image references only from
`Dockerfile`s and Compose files, so these pins never receive an update pull
request.

Move both references, each as `<image>:<tag>@sha256:<digest>`, into files
Dependabot's `docker` ecosystem reads — for example one `FROM`-only
`Dockerfile` per image under a directory of their own (such as
`docker/minio/Dockerfile` and `docker/aws-cli/Dockerfile`) — and have
`scripts/lib/minio.sh` read the reference from there, so that the file is
the single place each image is pinned. Choose the tags that the current
digests carry (MinIO's is the `RELEASE.2025-10-15T17-29-55Z` build named in
the comment), so that what runs does not change. Keep the comments' reasons
(why digests, why that MinIO build) next to the pins they explain. Keep the
`COFFRET_MINIO_IMAGE` / `COFFRET_AWS_CLI_IMAGE` overrides working, and keep
every caller of `scripts/lib/minio.sh` (`make e2e-it`, `make local`, the
`large-*` targets, `e2e-electron`, `s3-store-it`) working as it does.

Do not change `.github/`. Adding the `docker` ecosystem entry for the new
directories to `.github/dependabot.yml` is done separately; say in the PR
body which directories that entry has to name.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `scripts/lib/minio.sh` holds no image digest; each image's reference,
      tag and digest appear in exactly one file of a kind Dependabot's
      `docker` ecosystem reads, and the script reads them from there (a test
      run by `make check`, e.g. in the shell-script tests, fails when the
      script and the file disagree or the script carries its own digest).
- [x] The resolved references equal the digests pinned before this change.
- [x] `COFFRET_MINIO_IMAGE` and `COFFRET_AWS_CLI_IMAGE` still override them.
- [x] `make check` and `make e2e-it` pass.
