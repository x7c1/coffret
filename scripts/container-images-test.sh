#!/usr/bin/env bash
#
# Show that scripts/lib/minio.sh runs the images docker/minio/Dockerfile and
# docker/aws-cli/Dockerfile pin, and pins none of its own.
#
# Each image is pinned once, in the FROM line of its Dockerfile, where
# Dependabot can read it; the script reads it from there. So this checks that
# sourcing the script resolves each image to its file's reference, that the
# script holds no digest, that each pinned digest is written in no other file,
# and that COFFRET_MINIO_IMAGE and COFFRET_AWS_CLI_IMAGE still override the
# pins. The cases where a Dockerfile is missing or holds no usable FROM line run
# against a copy of the script in a throwaway tree, so the repository itself is
# never touched. Nothing here pulls or runs an image, so it needs no container
# runtime; it needs git and bash.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT
readonly LIB="scripts/lib/minio.sh"
readonly IMAGES=(minio aws-cli)

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

failures=0

fail() {
  echo "FAIL $1"
  failures=$((failures + 1))
}

# Sources the script at $1 in a fresh bash, with the overrides unset and any
# further arguments as NAME=value pairs in its environment, and prints the two
# images it resolved, one per line.
resolve() {
  local lib="$1"
  shift
  # shellcheck disable=SC2016 # expanded by the inner bash, not this one
  env -u COFFRET_MINIO_IMAGE -u COFFRET_AWS_CLI_IMAGE "$@" \
    bash -c 'set -euo pipefail; source "$1"; printf "%s\n%s\n" "$MINIO_IMAGE" "$AWS_CLI_IMAGE"' _ "$lib"
}

# The reference in the FROM line of the Dockerfile $1, read here independently
# of the script under test.
from_of() {
  sed -n 's/^FROM[[:space:]]\{1,\}\([^[:space:]]\{1,\}\)[[:space:]]*$/\1/p' "$1"
}

# The pins as they stand: one FROM line each, of the form the script asks for,
# and the script resolving to exactly that.
expected=()
for image in "${IMAGES[@]}"; do
  dockerfile="$ROOT/docker/$image/Dockerfile"
  if [ ! -f "$dockerfile" ]; then
    fail "pinned: docker/$image/Dockerfile is missing"
    expected+=("")
    continue
  fi
  ref="$(from_of "$dockerfile")"
  if [ "$(printf '%s' "$ref" | grep -c '')" -ne 1 ]; then
    fail "pinned: docker/$image/Dockerfile has to hold exactly one FROM line, and holds: ${ref:-none}"
  elif ! [[ "$ref" =~ ^[a-z0-9./_-]+:[A-Za-z0-9_.-]+@sha256:[0-9a-f]{64}$ ]]; then
    fail "pinned: docker/$image/Dockerfile pins $ref, which is not <image>:<tag>@sha256:<digest>"
  fi
  expected+=("$ref")
done

if ! got="$(resolve "$ROOT/$LIB" 2>"$work/pinned.log")"; then
  fail "pinned: sourcing $LIB failed"
  cat "$work/pinned.log"
elif [ "$got" != "$(printf '%s\n%s' "${expected[0]}" "${expected[1]}")" ]; then
  fail "pinned: $LIB resolved"
  echo "$got"
  echo "where the Dockerfiles pin"
  printf '%s\n' "${expected[@]}"
fi

# The script carries no digest of its own: no run of 64 hexadecimal digits.
if grep -nE '[0-9a-f]{64}' "$ROOT/$LIB"; then
  fail "no digest in the script: $LIB holds the line(s) above"
fi

# Each pinned digest is written down once, in its Dockerfile.
for i in "${!IMAGES[@]}"; do
  digest="${expected[$i]##*@sha256:}"
  if [ -z "$digest" ] || [ "$digest" = "${expected[$i]}" ]; then
    continue
  fi
  files="$(git -C "$ROOT" grep -l --untracked -F "$digest" || true)"
  if [ "$files" != "docker/${IMAGES[$i]}/Dockerfile" ]; then
    fail "pinned once: the ${IMAGES[$i]} digest has to appear in docker/${IMAGES[$i]}/Dockerfile alone, and appears in:"
    echo "${files:-no file}"
  fi
done

# The overrides win over the pins.
want="$(printf '%s\n%s' example.invalid/minio:override example.invalid/aws-cli:override)"
if ! got="$(resolve "$ROOT/$LIB" COFFRET_MINIO_IMAGE=example.invalid/minio:override \
  COFFRET_AWS_CLI_IMAGE=example.invalid/aws-cli:override 2>"$work/override.log")"; then
  fail "overrides: sourcing $LIB failed"
  cat "$work/override.log"
elif [ "$got" != "$want" ]; then
  fail "overrides: $LIB resolved"
  echo "$got"
fi

# A throwaway tree named $1, holding a copy of the script and the Dockerfiles
# that follow: each argument after it is `<image>=<file contents>`, and an image
# not named gets no Dockerfile.
tree() {
  local dir="$work/$1" pair
  shift
  mkdir -p "$dir/scripts/lib"
  cp "$ROOT/$LIB" "$dir/$LIB"
  for pair in "$@"; do
    mkdir -p "$dir/docker/${pair%%=*}"
    printf '%s\n' "${pair#*=}" >"$dir/docker/${pair%%=*}/Dockerfile"
  done
}

DIGEST_A="sha256:$(printf 'a%.0s' {1..64})"
DIGEST_B="sha256:$(printf 'b%.0s' {1..64})"
readonly DIGEST_A DIGEST_B

# The script follows the Dockerfiles: another pin there is another image here.
tree follows "minio=FROM example/minio:1@$DIGEST_A" "aws-cli=FROM example/aws-cli:2@$DIGEST_B"
if ! got="$(resolve "$work/follows/$LIB" 2>"$work/follows.log")"; then
  fail "follows: sourcing the copy failed"
  cat "$work/follows.log"
elif [ "$got" != "$(printf '%s\n%s' "example/minio:1@$DIGEST_A" "example/aws-cli:2@$DIGEST_B")" ]; then
  fail "follows: the copy resolved"
  echo "$got"
fi

# Sourcing the copy in tree $1 has to fail, saying why, for the reason after it.
expect_refused() {
  local name="$1" reason="$2"
  if resolve "$work/$name/$LIB" >"$work/$name.out" 2>"$work/$name.log"; then
    fail "$name: sourcing went through where $reason, resolving"
    cat "$work/$name.out"
  elif ! grep -q 'scripts/lib/minio.sh: ' "$work/$name.log"; then
    fail "$name: sourcing failed without saying why where $reason"
    cat "$work/$name.log"
  fi
}

tree missing "aws-cli=FROM example/aws-cli:2@$DIGEST_B"
expect_refused missing "docker/minio/Dockerfile is missing"

tree tag-only "minio=FROM example/minio:1" "aws-cli=FROM example/aws-cli:2@$DIGEST_B"
expect_refused tag-only "the MinIO pin has no digest"

tree digest-only "minio=FROM example/minio:1@$DIGEST_A" "aws-cli=FROM example/aws-cli@$DIGEST_B"
expect_refused digest-only "the AWS CLI pin has no tag"

tree two-froms "minio=FROM example/minio:1@$DIGEST_A
FROM example/minio:2@$DIGEST_B" "aws-cli=FROM example/aws-cli:2@$DIGEST_B"
expect_refused two-froms "the MinIO Dockerfile holds two FROM lines"

tree no-from "minio=# nothing" "aws-cli=FROM example/aws-cli:2@$DIGEST_B"
expect_refused no-from "the MinIO Dockerfile holds no FROM line"

if [ "$failures" -gt 0 ]; then
  echo "container-images-test: $failures failure(s)"
  exit 1
fi
echo "container-images-test: the scripts' images are the ones docker/ pins, and only there"
