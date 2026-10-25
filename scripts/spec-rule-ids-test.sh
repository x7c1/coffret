#!/usr/bin/env bash
#
# Show that spec-rule-ids.sh refuses what it exists to refuse.
#
# A check that never fails proves nothing about the tree it passes, and this one
# passes on the repository as it stands, so it is run here against small trees
# of its own: one where every citation resolves and every test-bound rule is
# named or marked, one citing a rule nothing defines, two holding a rule in both
# homes — once left in the register after its move, once opened with `//!` in
# an inline module, which is a definition at any indentation — one with a
# test-bound rule no test names, and one naming a rule marked unimplemented.
# Each is a throwaway git repository under a temporary directory, so the
# repository itself is never touched, and each case says which answer it
# expected when it does not get it.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT
readonly CHECK="$ROOT/scripts/spec-rule-ids.sh"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

failures=0

# A tree with a register of one mechanism, `AB`, that passes. AB-2 is defined in
# a test's module doc and the rest in the register: AB-1 is named by a doc
# comment, AB-3 — of mixed Form, its tag over two lines — in a list beside AB-1,
# AB-4 is prose and named by nothing, and AB-5 is marked unimplemented and named
# by nothing. AB-1 and AB-2 are cited.
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
- **AB-1.** A test-bound rule. *(Form: test)*
- **AB-3.** A rule of mixed Form. *(Form: test for this;
  prose for that)*
  - A clause of it.
- **AB-4.** A rule only prose can state. *(Form: prose — a reason)*
- **AB-5.** A rule not yet implemented. *(Form: test; unimplemented)*
EOF
  cat >"$dir/backend/src/ab_tests.rs" <<'EOF'
//! AB-2: A rule whose statement has moved into this test's module doc.

/// AB-1: a case sampling AB-1.
fn samples_one() {}

// AB-1, AB-3: a case sampling both.
fn samples_both() {}
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
printf '// Cites a rule nothing defines (spec: AB-9).\n' >"$homeless/backend/src/cites.rs"
expect "a cited rule with no home is named where it is cited" "$homeless" 1 "AB-9 cited at backend/src/cites.rs:1"

twice="$(tree twice)"
printf -- '- **AB-2.** The moved rule, left behind in the register.\n' >>"$twice/docs/spec/ab/README.md"
expect "a rule with two homes is named at both" "$twice" 1 "backend/src/ab_tests.rs:1"
expect "a rule with two homes names the register too" "$twice" 1 "docs/spec/ab/README.md:7"

indented="$(tree indented)"
printf 'mod tests {\n    //! AB-1: an inner doc, which defines rather than names.\n}\n' >"$indented/backend/src/inline.rs"
expect "an indented //! on a register rule is a second home" "$indented" 1 "backend/src/inline.rs:2"

unnamed="$(tree unnamed)"
printf -- '- **AB-6.** A test-bound rule no test names.\n  *(Form: test)*\n' >>"$unnamed/docs/spec/ab/README.md"
expect "a test-bound rule no test names is named in the register" "$unnamed" 1 "AB-6 at docs/spec/ab/README.md:7"

implemented="$(tree implemented)"
printf '// AB-5: a case sampling the rule marked unimplemented.\n' >"$implemented/frontend/ab.test.ts"
expect "a rule marked unimplemented that a test names is refused" "$implemented" 1 "AB-5 at docs/spec/ab/README.md:6"

if [ "$failures" -gt 0 ]; then
  echo "spec-rule-ids.sh did not answer as expected in $failures case(s)"
  exit 1
fi
