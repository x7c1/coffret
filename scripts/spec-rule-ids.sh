#!/usr/bin/env bash
#
# Refuse a cited spec rule with no home, or with two, and a test-bound rule out
# of step with the tests.
#
# Per docs/spec/README.md a rule is defined in exactly one place: by default in
# the register, as a bullet under docs/spec/ opening `**KD-4.**`; or, for a
# `Form: test` rule whose tests are all in one module, in that module's doc,
# a line opening `//! KD-12: …` under backend/ or frontend/ at any indentation —
# the register entry deleted in the same commit. Those two forms are the homes this
# collects. Every `XX-n` under backend/, frontend/, docs/concepts/ and
# docs/spec/ whose prefix is one the register's Mechanisms table defines is
# taken for a citation — bare or as `(spec: KD-4)`, in code, a comment or a
# document — and has to name an ID with a home: one without resolves to
# nothing, which is how a rule never written, or lost, looks. An ID with two
# homes is refused too, naming both: a rule left in the register after its
# statement moved into a module doc is a move done by half. docs/tasks/ is left
# out, because a task file is a record of past work and may name a rule as it
# stood then.
#
# A rule in the register whose Form tag opens `Form: test` — `*(Form: test)*`,
# a mixed `*(Form: test for …; prose for …)*`, or one saying which clause is
# `not yet implemented` — is test-bound, and has to be named by a comment under
# backend/ or frontend/ that opens with its ID and a colon: `// KD-4: …` or
# `/// KD-4: …`, alone or in a list as `// KD-4, KD-5: …`. A
# tag opening `Form: prose` is not test-bound, whatever it goes on to say. A tag
# ending `; unimplemented`, as `*(Form: test; unimplemented)*`, exempts the rule
# from being named — and a rule so marked that a comment does name is refused,
# since the behavior it says is not implemented is what that test samples. The
# phrase `not yet implemented` is not the mark. `//!` is only for a moved rule's
# definition: in Rust an indented `//!` is the inner doc of its module just as
# one at the margin is, so every `//! KD-4: …` is a home, and one on a rule the
# register holds is refused as a second home rather than read as a naming. A
# rule whose definition is a module doc is named by it.
#
# What it cannot see: a home written in any other form — a moved statement
# not opening its line as `//! XX-n:`, or a register entry not opening a line as
# a bullet in bold with its period — so such a rule reads as having no home and
# its citations fail; a prefix the Mechanisms table does not list, so `ZZ-1`
# passes, as the `UTF-8` and `SHA-256` it would otherwise refuse do; the upper
# end of a range, the `11` of `EP-9–11`; whether the rule a citation names is
# the rule it means, only that the ID has a home; whether a comment naming a
# rule sits on a test, opens its comment rather than a line in the middle of
# one, or samples the rule at all; whether every part of a mixed rule, or every
# clause of one partly implemented, is sampled, only that something names it;
# and a Form tag holding a `)` of its own, which ends the tag there.
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
moved="$(search 'spec rules moved into a module doc' -noE '^[[:space:]]*//! [A-Z]{2}-[0-9]+:' -- backend frontend)"
cited="$(search 'cited spec rules' -noE "(^|[^A-Za-z0-9_-])($prefixes)-[0-9]+" -- backend frontend docs/concepts docs/spec)"

# Each home as `ID<TAB>file:line`.
homes="$(printf '%s\n%s\n' "$registered" "$moved" |
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

# Each register rule that is test-bound or marked, as `ID<TAB>file:line<TAB>`
# `bound` or `marked`. A rule runs from its bullet to the next line that opens
# at the margin, and its Form tags may span lines, so the lines are joined.
register="$(search 'the register' -n -e '' -- docs/spec)"
forms="$(printf '%s\n' "$register" | awk '
  function flush(  rest, tag, bound, marked) {
    if (id == "") return
    rest = body
    while (match(rest, /\*\(Form:[^)]*\)\*/)) {
      tag = substr(rest, RSTART, RLENGTH)
      rest = substr(rest, RSTART + RLENGTH)
      if (tag ~ /^\*\(Form: +test/) bound = 1
      if (tag ~ /; *unimplemented\)\*$/) marked = 1
    }
    if (marked) print id "\t" at "\tmarked"
    else if (bound) print id "\t" at "\tbound"
    id = ""
  }
  {
    file = $0; sub(/:.*/, "", file)
    rest = substr($0, length(file) + 2)
    line = rest; sub(/:.*/, "", line)
    text = substr(rest, length(line) + 2)
  }
  file != current { flush(); current = file }
  file !~ /\.md$/ { next }
  match(text, /^- \*\*[A-Z][A-Z]-[0-9]+\.\*\*/) {
    found = substr(text, 5, RLENGTH - 7)
    flush(); id = found; at = file ":" line; body = text; next
  }
  text ~ /^[^ ]/ { flush(); next }
  id != "" { body = body " " text }
  END { flush() }')"

# Every ID a `//` or `///` comment under backend/ or frontend/ names, and every
# ID a module doc defines, one to a line.
naming="$(search 'rules a test names' -hoE '^[[:space:]]*///? [A-Z]{2}-[0-9]+(, [A-Z]{2}-[0-9]+)*:' -- backend frontend)"
named="$(printf '%s\n%s\n' "$naming" "$moved" | grep -oE '[A-Z]{2}-[0-9]+' | sort -u)" || true

unnamed="$(printf '%s\n' "$forms" | awk -F '\t' -v named="$(printf '%s\n' "$named" | paste -sd' ' -)" '
    BEGIN { n = split(named, ids, " "); for (i = 1; i <= n; i++) seen[ids[i]] = 1 }
    $3 == "bound" && !($1 in seen) { print "  " $1 " at " $2 }' | sort -k1,1V)"
implemented="$(printf '%s\n' "$forms" | awk -F '\t' -v named="$(printf '%s\n' "$named" | paste -sd' ' -)" '
    BEGIN { n = split(named, ids, " "); for (i = 1; i <= n; i++) seen[ids[i]] = 1 }
    $3 == "marked" && ($1 in seen) { print "  " $1 " at " $2 }' | sort -k1,1V)"

if [ -n "$twice" ]; then
  echo "a spec rule has two homes; one rule lives in exactly one place, per docs/spec/README.md:"
  printf '%s\n' "$twice"
fi
if [ -n "$homeless" ]; then
  echo "a spec rule is cited that has no home — no **XX-n.** entry under docs/spec, no //! XX-n: under backend or frontend:"
  printf '%s\n' "$homeless"
fi
if [ -n "$unnamed" ]; then
  echo "a Form: test rule is named by no test and not marked unimplemented — name it as // XX-n: … at a test that samples it, or mark it *(Form: test; unimplemented)*:"
  printf '%s\n' "$unnamed"
fi
if [ -n "$implemented" ]; then
  echo "a spec rule marked unimplemented is named by a test, so its behavior exists — drop the mark:"
  printf '%s\n' "$implemented"
fi
if [ -n "$twice$homeless$unnamed$implemented" ]; then
  exit 1
fi
