#!/usr/bin/env bash
#
# Set the workspace version: [workspace.package].version of the cargo
# workspace in the given directory (backend/ of this repository by default),
# and bring that workspace's Cargo.lock in line with it.
#
#   scripts/set-workspace-version.sh <version> [<workspace directory>]
#
# The release PR workflow (.github/workflows/release-pr.yml) runs this as
# colophon's `command` version writer, with the version the release PR's title
# names; scripts/set-workspace-version-test.sh runs it against throwaway
# workspaces. The version is X.Y.Z, which is what colophon's semver scheme
# produces.
#
# It runs where colophon's job runs it: a fresh ubuntu-latest runner with this
# repository checked out and nothing installed beyond the runner image. So:
#
#   - The toolchain. rustup does not install the toolchain rust-toolchain.toml
#     names on first use, so `rustup toolchain install` (rustup 1.28 or later)
#     is run in the workspace directory first. With no toolchain named it
#     installs the one active there — the pinned one in this repository, the
#     default one in a throwaway workspace outside it — and does nothing when
#     it is already installed. `--profile minimal` leaves out the
#     documentation, which nothing here reads and which is most of the
#     download.
#   - Cargo.toml. The quoted value of the one `version = "..."` line of the
#     [workspace.package] table is rewritten in place with awk, rather than
#     through cargo-edit's `cargo set-version`, which the runner does not carry
#     and which would take minutes to build. The line is looked for within that
#     table only, and the script fails unless exactly one was rewritten; it
#     then reads the version back through cargo (scripts/workspace-version.sh,
#     which needs jq, as the runner image carries) and fails unless cargo sees
#     the new one, so a manifest the rewrite did not understand stops here
#     rather than in the commit.
#   - Cargo.lock. `cargo update --workspace` rewrites the entries of the
#     workspace's own crates and leaves every other locked dependency where it
#     is. Cargo itself writing the lock file, rather than a text edit of it,
#     keeps the file exactly as cargo would have it, so a later `--locked` build
#     accepts it. It builds nothing, and takes a second or two even with an
#     empty cargo home.
#
# Nothing is written outside the workspace's Cargo.toml and Cargo.lock, since
# colophon commits whatever the command leaves in the working tree.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT

[ $# -ge 1 ] && [ $# -le 2 ] || {
  echo "usage: scripts/set-workspace-version.sh <version> [<workspace directory>]" >&2
  exit 2
}
version="$1"
workspace="${2:-$ROOT/backend}"

if [[ ! "$version" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]]; then
  echo "error: '$version' is not a version of the form X.Y.Z" >&2
  exit 1
fi

manifest="$workspace/Cargo.toml"
[ -f "$manifest" ] || {
  echo "error: there is no Cargo.toml in $workspace" >&2
  exit 1
}

if command -v rustup >/dev/null 2>&1; then
  (cd "$workspace" && rustup toolchain install --profile minimal)
fi

# Prints the manifest with the version line of [workspace.package] replaced,
# and exits 3 unless exactly one such line was found. A table header is a line
# starting with `[`; an array of tables (`[[...]]`) ends the table as well.
rewritten="$(mktemp)"
trap 'rm -f "$rewritten"' EXIT
status=0
awk -v version="$version" '
  /^[[:space:]]*\[/ {
    header = $0
    sub(/[[:space:]]*#.*$/, "", header)
    gsub(/[[:space:]]/, "", header)
    in_table = (header == "[workspace.package]")
  }
  in_table && /^[[:space:]]*version[[:space:]]*=/ {
    sub(/"[^"]*"/, "\"" version "\"")
    replaced++
  }
  { print }
  END { if (replaced != 1) exit 3 }
' "$manifest" >"$rewritten" || status=$?
if [ "$status" -ne 0 ]; then
  if [ "$status" -eq 3 ]; then
    echo "error: $manifest does not have exactly one version line in [workspace.package]" >&2
  fi
  exit 1
fi
cat "$rewritten" >"$manifest"

(cd "$workspace" && cargo update --workspace)

actual="$("$ROOT/scripts/workspace-version.sh" "$workspace")"
if [ "$actual" != "$version" ]; then
  echo "error: cargo reads the workspace version as $actual after setting it to $version" >&2
  exit 1
fi
echo "The workspace in $workspace is at version $version."
