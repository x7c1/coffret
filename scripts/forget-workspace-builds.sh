#!/usr/bin/env bash
#
# Make cargo rebuild this workspace's own crates in a restored target directory.
#
# CI caches backend/target between runs, and cargo decides whether a path
# crate needs rebuilding by comparing modification times: a crate is fresh
# when its last build is newer than every source file it read. A checkout
# stamps the sources with the time of the checkout, so a cache another job
# saved later — main's run of the same Cargo.lock, finishing after this job's
# checkout — holds builds newer than sources they were never built from, and
# cargo links them as they are. That is how a pull request once failed to
# compile against a method its own branch adds: the crate defining it was
# taken from main's build and not rebuilt.
#
# Dependencies come from crates.io at the versions Cargo.lock pins, so their
# cached builds are what the cache is for. Only the workspace's own packages
# change between commits, and dropping their fingerprints makes cargo rebuild
# them whatever the timestamps say.
#
# Usage: scripts/forget-workspace-builds.sh [backend directory]

set -euo pipefail

backend=${1:-backend}
target="$backend/target"
[ -d "$target" ] || exit 0

cargo metadata --manifest-path "$backend/Cargo.toml" --no-deps --format-version 1 |
  jq -r '.packages[].name' |
  while read -r name; do
    # A fingerprint directory is named after the package with its hash after
    # the last hyphen, so `coffret-` alone would also catch `coffret-cli-…`;
    # that is a workspace package too, so nothing outside the workspace goes.
    find "$target" -type d -path "*/.fingerprint/${name}-*" -prune -exec rm -rf {} +
  done
