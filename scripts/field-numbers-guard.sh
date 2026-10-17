#!/usr/bin/env bash
#
# Refuse an edit to the field-number registry that would change what a stored
# map means.
#
# docs/spec/format/field-numbers.json assigns every field of every stored CBOR
# map the field number it travels under (spec: FM-9). A Container is immutable
# and every Storage Object outlives the build that wrote it, so a row, once on
# main, says for good what that field number means in that map. This compares
# the registry in the working tree with its version on the merge base of HEAD
# and the base ref (origin/main unless FIELD_NUMBERS_BASE_REF names another) and
# refuses any row that is gone, or whose map, name or field number changed — a
# row is known by its map and field number, so a changed map or field number
# reads as the row having gone — and any retired row brought back to life.
# Adding a row, and adding `"retired": true` to one, are the only edits it
# accepts. A registry the merge base does not have yet has nothing to be held
# to.
#
# A new row that reuses a field number an old row of the same map holds is
# refused, as that field number then names more than one row. What it cannot
# see: whether two new rows share a field number — the two implementations' own
# tests refuse that — and whether a retired field number is still read or
# written somewhere, which those tests own too.
#
# A base ref that cannot be resolved fails rather than passing, as does a
# registry that is not JSON on either side: every answer is read from them.

set -euo pipefail

# The repository to check: this one, unless a test names a tree of its own.
readonly ROOT="${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
readonly REGISTRY="docs/spec/format/field-numbers.json"
readonly BASE_REF="${FIELD_NUMBERS_BASE_REF:-origin/main}"
cd "$ROOT"

if ! base="$(git merge-base HEAD "$BASE_REF" 2>/dev/null)"; then
  echo "found no merge base of HEAD and $BASE_REF to compare $REGISTRY against" >&2
  exit 1
fi

if ! git cat-file -e "$base:$REGISTRY" 2>/dev/null; then
  echo "$REGISTRY is not on the merge base with $BASE_REF yet; nothing to hold it to"
  exit 0
fi

if [ ! -f "$REGISTRY" ]; then
  echo "$REGISTRY is gone, and every row it held on $BASE_REF with it" >&2
  exit 1
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
git show "$base:$REGISTRY" >"$work/base.json"

# One line per refused row; none when every old row stands as it was.
# shellcheck disable=SC2016 # the dollars are jq's, not the shell's
problems="$(jq -rn --slurpfile old "$work/base.json" --slurpfile new "$REGISTRY" '
  $old[0].fields[] as $row
  | ($new[0].fields | map(select(.map == $row.map and .number == $row.number))) as $now
  | if ($now | length) == 0 then
      "removed, or its map or field number changed: \($row.map).\($row.name) = \($row.number)"
    elif ($now | length) > 1 then
      "\($row.map) gives field number \($row.number) to more than one row"
    elif $now[0].name != $row.name then
      "renamed: \($row.map) field number \($row.number) was \($row.name), now \($now[0].name)"
    elif ($row.retired // false) and (($now[0].retired // false) | not) then
      "brought back: \($row.map).\($row.name) = \($row.number) was retired"
    else
      empty
    end
')"

if [ -n "$problems" ]; then
  echo "$REGISTRY changes rows $BASE_REF already holds; add a row or retire one instead:" >&2
  printf '  %s\n' "$problems" >&2
  exit 1
fi
echo "$REGISTRY keeps every row $BASE_REF holds"
