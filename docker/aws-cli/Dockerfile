# The AWS CLI image that makes a bucket in the scripts' MinIO
# (scripts/lib/minio.sh reads the reference from the FROM line below; nothing
# builds this file). Kept in a Dockerfile of its own, as docker/minio/Dockerfile
# is, so that Dependabot's `docker` ecosystem proposes its updates; this line is
# the one place the image is pinned.
#
# MinIO's own client is not in the MinIO image — only the image MinIO itself
# used to publish bundled it — so a bucket is made by the AWS CLI, in a
# container of its own. Pinned by digest as well as by tag for the reason the
# MinIO image is.
FROM amazon/aws-cli:2.40.1@sha256:4ade0fe3e60e0b9e1fb0910f86c6d992baf4e12d3172c25a3c8c11f6c81cee08
