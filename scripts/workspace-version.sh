#!/usr/bin/env bash
#
# Print the workspace version: [workspace.package].version of the cargo
# workspace in the given directory (backend/ of this repository by default).
#
#   scripts/workspace-version.sh [<workspace directory>]
#
# It is read through `cargo metadata` rather than by grepping TOML, so that
# cargo itself honours the section boundary; cargo metadata reads Cargo.toml
# and Cargo.lock only, and needs no network. cargo metadata reports members'
# versions rather than the workspace's own, so this reads the version of
# coffret-desktop-core, which takes its version from the workspace and is part
# of the desktop app every bundle carries. It fails where that crate is not in
# the workspace, rather than print nothing.
#
# The release workflow (scripts/check-version-change.sh) and the desktop
# bundles (scripts/desktop.sh) both read the version through this.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT
readonly CRATE=coffret-desktop-core

[ $# -le 1 ] || {
  echo "usage: scripts/workspace-version.sh [<workspace directory>]" >&2
  exit 2
}
workspace="${1:-$ROOT/backend}"

command -v jq >/dev/null 2>&1 || {
  echo "error: jq is missing; it reads cargo metadata's output" >&2
  exit 1
}

metadata="$(cd "$workspace" && cargo metadata --no-deps --format-version 1)"
version="$(jq -r --arg crate "$CRATE" '.packages[] | select(.name == $crate) | .version' <<<"$metadata")"
if [ -z "$version" ]; then
  echo "error: the workspace in $workspace has no $CRATE, whose version is the workspace's" >&2
  exit 1
fi
echo "$version"
