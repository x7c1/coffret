# The MinIO image every script that needs real S3 runs (scripts/lib/minio.sh
# reads the reference from the FROM line below; nothing builds this file). It is
# kept in a Dockerfile, alone, so that Dependabot's `docker` ecosystem, which
# reads image references only from Dockerfiles and Compose files, proposes its
# updates; this line is the one place the image is pinned.
#
# MinIO itself no longer publishes an image: its Docker Hub repository is gone,
# its quay.io one answers every pull with `unauthorized`, and dl.min.io returns
# 410 for the release binaries. What is pinned here is the last upstream release
# (RELEASE.2025-10-15T17-29-55Z) as built from that source by the alpine-docker
# project, by digest as well as by tag so that a retagged or retracted image
# fails loudly instead of silently changing what runs against it; the tag is
# there for Dependabot and for the reader, and the digest is what is pulled.
FROM alpine/minio:RELEASE.2025-10-15T17-29-55Z@sha256:cf23643a6cf9ce159c57643ceb88279e431262282428c9e0bf3a7ef1a97e84b4
