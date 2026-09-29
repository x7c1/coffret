#!/usr/bin/env bash
#
# Refuse a cited spec rule with no home, or with two.
#
# Per docs/spec/README.md a rule lives in exactly one place: in the register,
# as a bullet under docs/spec/ opening `**KD-4.**`, while it is prose; and once
# a `Form: test` rule has migrated, in the test comment holding its full
# statement, which opens a module doc as `//! KD-12: …` under backend/ or
# frontend/ — the register entry deleted in the same commit. Those two forms
# are the homes this collects. Every `XX-n` under backend/, frontend/,
# docs/concepts/ and docs/spec/ whose prefix is one the register's Mechanisms
# table defines is taken for a citation — bare or as `(spec: KD-4)`, in code,
# a comment or a document — and has to name an ID with a home: one without
# resolves to nothing, which is how a rule never written, or lost, looks. An ID
# with two homes is refused too, naming both: a rule left in the register after
# its statement moved into a test is a migration done by half. docs/tasks/ is
# left out, because a task file is a record of past work and may name a rule as
# it stood then. The `// KD-4: …` opening a test comment names a rule the case
# samples and is a citation, not a home.
#
# What it cannot see: a home written in any other form — a migrated statement
# not opening a line as `//! XX-n:`, or a register entry not in bold with its
# period — so such a rule reads as having no home and its citations fail; a
# prefix the Mechanisms table does not list, so `ZZ-1` passes, as the `UTF-8`
# and `SHA-256` it would otherwise refuse do; the upper end of a range, the
# `11` of `EP-9–11`; and whether the rule a citation names is the rule it
# means, only that the ID has a home.
#
# git grep exits 1 when nothing matches; above that it could not search at all,
# which fails rather than passing as a clean tree. Finding no prefix, or no rule
# in the register, fails too, since every other answer is read from them.

set -euo pipefail

# The repository to check: this one, unless a test names a tree of its own.
readonly ROOT="${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
cd "$ROOT"

# Runs git grep and prints what it found. No match is an empty answer; a
# status above 1 means it could not search, and the run stops there.
search() {
  local what="$1" status=0 found
  shift
  found="$(git grep --untracked "$@")" || status=$?
  if [ "$status" -gt 1 ]; then
    echo "git grep could not search for $what (exit $status)" >&2
    exit "$status"
  fi
  printf '%s' "$found"
}

# shellcheck disable=SC2016 # the backticks are literal: the table cell quotes the prefix in them
prefixes="$(search 'rule prefixes' -hoE '^\| \[[^]]*\]\([^)]*\) \| `[A-Z]{2}` \|' -- docs/spec/README.md)"
if [ -z "$prefixes" ]; then
  echo "read no rule prefix from the Mechanisms table of docs/spec/README.md" >&2
  exit 1
fi
# shellcheck disable=SC2016 # the backticks are literal, as above
prefixes="$(printf '%s\n' "$prefixes" | sed -E 's/.*`([A-Z]{2})`.*/\1/' | sort -u | paste -sd'|' -)"

registered="$(search 'rules in the register' -noE '\*\*[A-Z]{2}-[0-9]+\.\*\*' -- docs/spec)"
if [ -z "$registered" ]; then
  echo "read no rule from the register under docs/spec" >&2
  exit 1
fi
migrated="$(search 'migrated spec rules' -noE '^//! [A-Z]{2}-[0-9]+:' -- backend frontend)"
cited="$(search 'cited spec rules' -noE "(^|[^A-Za-z0-9_-])($prefixes)-[0-9]+" -- backend frontend docs/concepts docs/spec)"

# Each home as `ID<TAB>file:line`.
homes="$(printf '%s\n%s\n' "$registered" "$migrated" |
  sed -nE 's/^([^:]+:[0-9]+):.*([A-Z]{2}-[0-9]+).*$/\2\t\1/p' |
  sort -k1,1V -k2,2)"

twice="$(printf '%s\n' "$homes" | awk -F '\t' '
  { n[$1]++; at[$1] = at[$1] "\n    " $2 }
  END { for (id in n) if (n[id] > 1) print "  " id ":" at[id] }')"

homeless="$(printf '%s\n' "$cited" |
  sed -nE 's/^([^:]+:[0-9]+):.*([A-Z]{2}-[0-9]+)$/\2\t\1/p' |
  awk -F '\t' -v homes="$(printf '%s\n' "$homes" | cut -f1 | paste -sd' ' -)" '
      BEGIN { n = split(homes, ids, " "); for (i = 1; i <= n; i++) known[ids[i]] = 1 }
      !($1 in known) { print "  " $1 " cited at " $2 }' |
  sort -u -k1,1V -k4,4)"

if [ -n "$twice" ]; then
  echo "a spec rule has two homes; one rule lives in exactly one place, per docs/spec/README.md:"
  printf '%s\n' "$twice"
fi
if [ -n "$homeless" ]; then
  echo "a spec rule is cited that has no home — no **XX-n.** entry under docs/spec, no //! XX-n: under backend or frontend:"
  printf '%s\n' "$homeless"
fi
if [ -n "$twice$homeless" ]; then
  exit 1
fi
