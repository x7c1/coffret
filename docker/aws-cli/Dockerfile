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
FROM amazon/aws-cli:2.41.1@sha256:538a9a80bc7c78e3228bc65100404a5dbd89ad812a547004c05ea20109543432
