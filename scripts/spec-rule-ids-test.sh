#!/usr/bin/env bash
#
# Show that spec-rule-ids.sh refuses what it exists to refuse.
#
# A check that never fails proves nothing about the tree it passes, and this one
# passes on the repository as it stands, so it is run here against small trees
# of its own: one where every citation resolves, one citing a rule nothing
# defines, and one holding a rule in both homes. Each is a throwaway git
# repository under a temporary directory, so the repository itself is never
# touched, and each case says which answer it expected when it does not get it.

set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly CHECK="$ROOT/scripts/spec-rule-ids.sh"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

failures=0

# A tree with a register of one mechanism, `AB`: AB-1 lives in the register,
# AB-2 has migrated into a test's module doc, and both are cited.
tree() {
  local dir="$work/$1"
  mkdir -p "$dir/docs/spec/ab" "$dir/docs/concepts/thing" "$dir/backend/src" "$dir/frontend"
  git -C "$dir" init -q
  cat >"$dir/docs/spec/README.md" <<'EOF'
| Mechanism | Prefix | Covers |
| --- | --- | --- |
| [Alpha Beta](ab/) | `AB` | what the fixture needs |
EOF
  cat >"$dir/docs/spec/ab/README.md" <<'EOF'
- **AB-1.** A rule still in the register.
EOF
  cat >"$dir/backend/src/ab_tests.rs" <<'EOF'
//! AB-2: A rule whose statement has migrated into this test.
EOF
  cat >"$dir/docs/concepts/thing/README.md" <<'EOF'
Cites both homes (spec: AB-1, AB-2).
EOF
  printf '%s\n' "$dir"
}

# Runs the check on a tree and holds its exit status and output against what
# the case expects.
expect() {
  local name="$1" dir="$2" want_status="$3" want_text="$4" status=0 said
  said="$("$CHECK" "$dir" 2>&1)" || status=$?
  if [ "$status" -ne "$want_status" ]; then
    echo "FAIL $name: exit $status, expected $want_status"
    printf '%s\n' "$said" | sed 's/^/    /'
    failures=$((failures + 1))
  elif [ -n "$want_text" ] && ! grep -qF -- "$want_text" <<<"$said"; then
    echo "FAIL $name: output does not say \"$want_text\""
    printf '%s\n' "$said" | sed 's/^/    /'
    failures=$((failures + 1))
  else
    echo "ok   $name"
  fi
}

clean="$(tree clean)"
expect "every citation has one home" "$clean" 0 ""

homeless="$(tree homeless)"
printf '// Cites a rule nothing defines (spec: AB-3).\n' >"$homeless/backend/src/cites.rs"
expect "a cited rule with no home is named where it is cited" "$homeless" 1 "AB-3 cited at backend/src/cites.rs:1"

twice="$(tree twice)"
printf -- '- **AB-2.** The migrated rule, left behind in the register.\n' >>"$twice/docs/spec/ab/README.md"
expect "a rule with two homes is named at both" "$twice" 1 "backend/src/ab_tests.rs:1"
expect "a rule with two homes names the register too" "$twice" 1 "docs/spec/ab/README.md:2"

if [ "$failures" -gt 0 ]; then
  echo "spec-rule-ids.sh did not answer as expected in $failures case(s)"
  exit 1
fi
