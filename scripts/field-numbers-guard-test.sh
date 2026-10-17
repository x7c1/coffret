#!/usr/bin/env bash
#
# Show that field-numbers-guard.sh refuses what it exists to refuse.
#
# The guard passes on this repository whenever nobody has edited the registry,
# which says nothing about whether it would refuse anything, so it is run here
# against throwaway git repositories of its own under a temporary directory:
# each holds a registry committed on a `main` that stands in for origin/main,
# and one edit to it in the working tree — every forbidden kind, and the two
# allowed ones. The repository itself is never touched, and each case says
# which answer it expected when it does not get it.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT
readonly GUARD="$ROOT/scripts/field-numbers-guard.sh"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

failures=0

readonly BASE_REGISTRY='{
  "fields": [
    { "map": "meta", "name": "schema", "number": 1 },
    { "map": "meta", "name": "kind", "number": 2 },
    { "map": "meta_entry", "name": "captured_path", "number": 0 }
  ]
}'

# A repository whose main holds the base registry, with `registry` written over
# it in the working tree afterwards.
tree() {
  local dir="$work/$1" registry="$2"
  mkdir -p "$dir/docs/spec/format"
  git -C "$dir" init -q -b main
  git -C "$dir" config user.email "guard@example.invalid"
  git -C "$dir" config user.name "guard test"
  printf '%s\n' "$BASE_REGISTRY" >"$dir/docs/spec/format/field-numbers.json"
  git -C "$dir" add docs
  git -C "$dir" -c commit.gpgsign=false -c core.hooksPath=/dev/null commit -q -m "registry"
  printf '%s\n' "$registry" >"$dir/docs/spec/format/field-numbers.json"
  printf '%s\n' "$dir"
}

# Runs the guard on a tree, against its own main, and holds its exit status
# and output against what the case expects.
expect() {
  local name="$1" dir="$2" want_status="$3" want_text="$4" status=0 said
  said="$(FIELD_NUMBERS_BASE_REF=main "$GUARD" "$dir" 2>&1)" || status=$?
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

unchanged="$(tree unchanged "$BASE_REGISTRY")"
expect "an untouched registry passes" "$unchanged" 0 ""

added="$(tree added '{
  "fields": [
    { "map": "meta", "name": "schema", "number": 1 },
    { "map": "meta", "name": "kind", "number": 2 },
    { "map": "meta", "name": "pad_len", "number": 3 },
    { "map": "meta_entry", "name": "captured_path", "number": 0 }
  ]
}')"
expect "an added row passes" "$added" 0 ""

retired="$(tree retired '{
  "fields": [
    { "map": "meta", "name": "schema", "number": 1 },
    { "map": "meta", "name": "kind", "number": 2, "retired": true },
    { "map": "meta_entry", "name": "captured_path", "number": 0 }
  ]
}')"
expect "a row marked retired passes" "$retired" 0 ""

removed="$(tree removed '{
  "fields": [
    { "map": "meta", "name": "schema", "number": 1 },
    { "map": "meta_entry", "name": "captured_path", "number": 0 }
  ]
}')"
expect "a removed row is refused" "$removed" 1 "meta.kind = 2"

renumbered="$(tree renumbered '{
  "fields": [
    { "map": "meta", "name": "schema", "number": 1 },
    { "map": "meta", "name": "kind", "number": 5 },
    { "map": "meta_entry", "name": "captured_path", "number": 0 }
  ]
}')"
expect "a changed field number is refused" "$renumbered" 1 "meta.kind = 2"

renamed="$(tree renamed '{
  "fields": [
    { "map": "meta", "name": "schema", "number": 1 },
    { "map": "meta", "name": "container_kind", "number": 2 },
    { "map": "meta_entry", "name": "captured_path", "number": 0 }
  ]
}')"
expect "a changed name is refused" "$renamed" 1 "was kind, now container_kind"

remapped="$(tree remapped '{
  "fields": [
    { "map": "meta", "name": "schema", "number": 1 },
    { "map": "meta", "name": "kind", "number": 2 },
    { "map": "addition_entry", "name": "captured_path", "number": 0 }
  ]
}')"
expect "a changed map is refused" "$remapped" 1 "meta_entry.captured_path = 0"

reused="$(tree reused '{
  "fields": [
    { "map": "meta", "name": "schema", "number": 1 },
    { "map": "meta", "name": "kind", "number": 2 },
    { "map": "meta", "name": "pad_len", "number": 2 },
    { "map": "meta_entry", "name": "captured_path", "number": 0 }
  ]
}')"
expect "an added row reusing a held field number is refused" "$reused" 1 "meta gives field number 2 to more than one row"

revived="$(tree revived "$BASE_REGISTRY")"
cat >"$revived/docs/spec/format/field-numbers.json" <<'EOF'
{
  "fields": [
    { "map": "meta", "name": "schema", "number": 1 },
    { "map": "meta", "name": "kind", "number": 2, "retired": true },
    { "map": "meta_entry", "name": "captured_path", "number": 0 }
  ]
}
EOF
git -C "$revived" -c commit.gpgsign=false -c core.hooksPath=/dev/null commit -q -am "retire kind"
printf '%s\n' "$BASE_REGISTRY" >"$revived/docs/spec/format/field-numbers.json"
expect "a retired row brought back is refused" "$revived" 1 "was retired"

if [ "$failures" -gt 0 ]; then
  echo "field-numbers-guard.sh did not answer as expected in $failures case(s)"
  exit 1
fi
