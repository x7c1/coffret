#!/usr/bin/env bash
#
# Refuse a spec rule cited bare, as (KD-4), outside the register.
#
# Per docs/spec/README.md a rule is cited bare only inside the register;
# anywhere else it takes the `spec:` prefix, `(spec: KD-4)`, so the reader sees
# where the token resolves. Only code under backend/ and frontend/ is searched,
# and the `// KD-4: …` opening a test comment may use is not a citation.
#
# An ID that opens the parentheses is caught whatever follows it — `(KD-4)`,
# `(KD-4 and KD-5)`, `(EP-9–11)`. One behind something else, as in
# `([Error::X], EP-1)`, or on the line after its `(`, is not seen.
#
# git grep exits 1 when nothing matches; above that it could not search at all,
# which fails rather than passing as a clean tree.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT
cd "$ROOT"

status=0
bare="$(git grep --untracked -nE '\([A-Z]{2}-[0-9]+' -- backend frontend ':!*.md')" || status=$?
if [ "$status" -gt 1 ]; then
  echo "git grep could not search for bare spec citations (exit $status)" >&2
  exit "$status"
fi
if [ -n "$bare" ]; then
  echo "a spec rule is cited bare outside the register; write it as (spec: XX-n), per docs/spec/README.md:"
  printf '%s\n' "$bare"
  exit 1
fi
