#!/usr/bin/env bash
#
# Smoke-test rename against a real Library, from one command.
#
#   scripts/rename-smoke.sh <library>
#
# CI holds rename to its rules against MinIO, and the conformance suites hold
# rename detection to them in memory. What neither says is whether the build in
# this checkout renames correctly against a Library on a person's own Storage —
# a development Library on Drive, say. This is that check: a sync, a `coffret
# rename` of a folder, two files renamed on disk, and a sync that carries them
# as renames, each step read back out of its `--json` answer and the commit it
# made read back out of its log.
#
# What it works on is the Library named on the command line, under
# COFFRET_STATE_DIR, with the logs under COFFRET_LOG_DIR. Both are required: the
# binary's own default is the production Library, and a smoke test reaches it
# only where somebody names it. `make rename-smoke` sets both as `make cli`
# does: the development state directory, unless COFFRET_STATE_DIR is given, and
# the `logs` folder under it.
#
# The Passphrase is asked for once, without echoing it, and handed to every step
# on standard input (`--passphrase-stdin`); it is never in an argument, in the
# environment of a step, or in a file. A Drive client secret is not needed: the
# Library keeps the client its grant was issued to.
#
# Where it writes: a fresh folder `rename-smoke-<UTC timestamp>/` under the
# Library-root mapping, or under the first mapping where there is no root one,
# holding two small books of a few generated SVG pages each. Nothing outside it
# is written, renamed or removed. The steps sync the whole Library, though, as a
# sync does, so whatever else is waiting in the mapped folders is carried in
# too: the first sync, before the smoke folder exists, is what settles that, and
# is also where a wrong Passphrase is refused before anything has been written.
#
# What it leaves: the smoke folder, in the Library and on disk. The CLI has no
# way to take an Entry out of a Library, so the folder is left for a person to
# delete from the explorer, and the run says where it is either way. What it
# uploads is those few pages, a few hundred bytes each.
#
# It needs `jq` on the PATH, as the Drive targets do and for the same reason:
# every fact checked is read out of the JSON a step answered with rather than
# out of sentences written for a person.
#
# COFFRET_RENAME_SMOKE_BIN names a coffret binary to run instead of building
# one. The integration test that runs this script against MinIO sets it to the
# binary cargo has just built for the test; a person leaves it unset.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT

# How many pages each book is generated with.
readonly PAGES=3

# Set once the smoke folder has been created, so that every way out says where
# it is.
smoke_dir=""
smoke_entry=""

on_exit() {
  if [ -n "$smoke_dir" ]; then
    echo >&2
    echo "rename-smoke: the smoke folder stays in the Library: $smoke_entry" >&2
    echo "rename-smoke: on this device it is $smoke_dir" >&2
    echo "rename-smoke: delete it from the explorer once you are done with it." >&2
  fi
}
trap on_exit EXIT

fail() {
  echo "rename-smoke: FAILED: $*" >&2
  exit 1
}

say() {
  echo "rename-smoke: $*" >&2
}

library="${1:-}"
[ -n "$library" ] || fail "name the Library to smoke-test: $0 <library>"
[ -n "${COFFRET_STATE_DIR:-}" ] ||
  fail "COFFRET_STATE_DIR is not set. It names the state directory the Library is under; \`make rename-smoke\` sets it to the development one."
[ -n "${COFFRET_LOG_DIR:-}" ] ||
  fail "COFFRET_LOG_DIR is not set. The run's log is where its commits are checked; \`make rename-smoke\` sets it beside the state directory."
command -v jq >/dev/null ||
  fail "jq is not on this PATH, and this script reads what the CLI answers with it."

# The binary: the one named, or the build `make cli` makes, built once here.
if [ -n "${COFFRET_RENAME_SMOKE_BIN:-}" ]; then
  bin="$COFFRET_RENAME_SMOKE_BIN"
else
  say "building the CLI"
  cargo build --release --manifest-path "$ROOT/backend/Cargo.toml" -p coffret-cli >&2 ||
    fail "the CLI did not build; cargo's own account is above."
  bin="$ROOT/backend/target/release/coffret"
fi
[ -x "$bin" ] || fail "$bin is not an executable coffret binary."

# The Passphrase, once. A prompt only where somebody is typing: a script that
# pipes it in reads no prompt.
if [ -t 0 ]; then
  printf 'Passphrase for %s: ' "$library" >&2
fi
passphrase=""
IFS= read -rs passphrase || true
if [ -t 0 ]; then
  echo >&2
fi
[ -n "$passphrase" ] || fail "no Passphrase was given."

# What the last step answered, and the status it exited with.
answer=""
status=0

# Runs one step with `--json`, the Passphrase on its standard input. printf is
# a builtin, so the Passphrase is never in a process's arguments.
run_step() {
  status=0
  answer="$(printf '%s\n' "$passphrase" | "$bin" --json "$@" --passphrase-stdin)" || status=$?
}

# One value out of the last answer, by a jq filter.
answered() {
  jq --raw-output "$@" <<<"$answer"
}

# Fails with what the CLI said where the last step failed outright.
refuse_failed_step() {
  local what="$1" message
  if [ "$status" -ne 0 ] && [ "$status" -ne 2 ]; then
    message="$(answered '.error.message // empty' 2>/dev/null || true)"
    fail "$what failed (exit status $status): ${message:-the CLI said nothing on standard output; its standard error is above}"
  fi
}

# A sync may end with findings (exit status 2) about files the person has
# elsewhere in the Library; that is theirs, and only a finding that needs
# attention under the smoke folder is this run's business.
refuse_smoke_findings() {
  local what="$1" found
  # shellcheck disable=SC2016 # a jq filter: its $names are jq's, bound by --arg
  found="$(answered --arg under "$smoke_entry/" \
    '[.findings[] | select(.needs_attention and ((.path // "") | startswith($under))) | .said] | join("\n")')"
  [ -z "$found" ] || fail "$what left findings under the smoke folder:
$found"
}

# Checks in the last step's log that the commit it made added and removed no
# Container: the `committed the batch` event of the generation it committed.
refuse_uploaded_containers() {
  local what="$1" log head event additions removals
  log="$(answered '.log // empty')"
  head="$(answered '.answer.committed_head // empty')"
  [ -n "$head" ] || fail "$what committed nothing, and it should have committed one record."
  if [ -z "$log" ] || [ ! -f "$log" ]; then
    fail "$what answered with no log file to check its commit in (log: ${log:-null})."
  fi
  # shellcheck disable=SC2016 # a jq filter: its $names are jq's, bound by --argjson
  event="$(jq -R -c --argjson head "$head" \
    'fromjson? // empty | select(.fields.message == "committed the batch" and .fields.generation == $head) | .fields' \
    "$log" | tail -n 1)"
  [ -n "$event" ] || fail "$what committed head $head, and its log $log has no \`committed the batch\` event for it."
  additions="$(jq -r '.additions' <<<"$event")"
  removals="$(jq -r '.removals' <<<"$event")"
  if [ "$additions" != 0 ] || [ "$removals" != 0 ]; then
    fail "$what committed head $head with $additions Container(s) added and $removals removed, where a rename uploads none: $event"
  fi
}

# Fails unless the last answer's `jq` filter $2 is $3.
expect() {
  local what="$1" filter="$2" want="$3" got
  got="$(answered "$filter")"
  [ "$got" = "$want" ] || fail "$what: expected $filter to be $want, and it was $got. The answer was: $(answered -c '.answer')"
}

# What page $2 of book $1 holds: an SVG that names the run, so that no two
# pages of this run or any other are the same bytes.
page() {
  printf '<svg xmlns="http://www.w3.org/2000/svg" width="120" height="160">'
  printf '<rect width="120" height="160" fill="#%02x%02x%02x"/>' "$((RANDOM % 256))" "$((RANDOM % 256))" "$((RANDOM % 256))"
  printf '<text x="10" y="80">%s %s page %d</text></svg>\n' "${smoke_dir##*/}" "$1" "$2"
}

# 0. The mapped root the smoke folder goes under.
status=0
answer="$("$bin" --json mappings --library "$library" </dev/null)" || status=$?
refuse_failed_step "listing the mappings of $library"
refused="$(answered '.answer.refused.message // empty')"
[ -z "$refused" ] || fail "the Index of $library could not be opened: $refused"
mapping="$(answered -c '.answer.mappings | (map(select(.prefix == null)) + .) | .[0] // empty')"
[ -n "$mapping" ] ||
  fail "$library has no mappings on this device, so there is no folder to put the smoke folder in. Map one first with \`coffret map\`."
local_root="$(jq -r '.local_root' <<<"$mapping")"
prefix="$(jq -r '.prefix // empty' <<<"$mapping")"
[ -d "$local_root" ] || fail "$library maps ${prefix:-the Library root} to $local_root, which is not a folder on this device."
say "using the mapping of ${prefix:-the Library root} at $local_root"

# 1. A sync before anything is written: it checks the Passphrase, and carries in
#    whatever was already waiting, so that what the next sync adds is this
#    run's files alone.
run_step sync --library "$library"
refuse_failed_step "the first sync"
settled="$(answered -c '.answer')"

# The smoke folder, and its two books.
name="rename-smoke-$(date -u +%Y%m%dT%H%M%SZ)"
[ ! -e "$local_root/$name" ] || fail "$local_root/$name is already there; run again in a second."
mkdir "$local_root/$name"
smoke_dir="$local_root/$name"
smoke_entry="${prefix:+$prefix/}$name"
for book in book-a book-b; do
  mkdir "$smoke_dir/$book"
  for number in $(seq "$PAGES"); do
    page "$book" "$number" >"$smoke_dir/$book/page-0$number.svg"
  done
done
files=$((PAGES * 2))

# 2. A sync that adds them.
run_step sync --library "$library"
refuse_failed_step "the sync that adds the books"
refuse_smoke_findings "the sync that adds the books"
expect "the sync that adds the books" '.answer.added' "$files"
added="$(answered -c '.answer')"

# 3. `coffret rename` of the first book's folder, which uploads nothing and
#    takes this device's files along with it.
# An indexed array, keyed by the page number: bash 3.2, which macOS ships as
# /bin/bash, has no associative ones.
contents=()
for number in $(seq "$PAGES"); do
  contents[number]="$(cat "$smoke_dir/book-a/page-0$number.svg")"
done
run_step rename --library "$library" \
  --from "$smoke_entry/book-a" --to "$smoke_entry/renamed/book-a"
refuse_failed_step "coffret rename"
[ "$status" -eq 0 ] || fail "coffret rename ended with findings: $(answered -c '.findings')"
expect "coffret rename" '.answer.renamed' "$PAGES"
expect "coffret rename" '.answer.folder' true
refuse_uploaded_containers "coffret rename"
for number in $(seq "$PAGES"); do
  old="$smoke_dir/book-a/page-0$number.svg"
  new="$smoke_dir/renamed/book-a/page-0$number.svg"
  [ -f "$new" ] || fail "coffret rename left nothing at $new."
  [ "$(cat "$new")" = "${contents[number]}" ] || fail "$new does not hold what $old held."
  [ ! -e "$old" ] || fail "coffret rename left $old where it was."
done
renamed="$(answered -c '.answer')"

# 4. Two renames on disk in the second book: one page within its folder, and
#    one into a new sub-folder of it.
mv "$smoke_dir/book-b/page-01.svg" "$smoke_dir/book-b/cover.svg"
mkdir "$smoke_dir/book-b/extra"
mv "$smoke_dir/book-b/page-02.svg" "$smoke_dir/book-b/extra/page-02.svg"

# 5. A sync that carries both as renames, uploading nothing.
run_step sync --library "$library"
refuse_failed_step "the sync after the renames on disk"
refuse_smoke_findings "the sync after the renames on disk"
expect "the sync after the renames on disk" '.answer.renamed' 2
expect "the sync after the renames on disk" '.answer.added' 0
refuse_uploaded_containers "the sync after the renames on disk"
carried="$(answered -c '.answer')"

echo "rename-smoke: passed against $library"
echo "  first sync:        $settled"
echo "  sync of the books: $added"
echo "  coffret rename:    $renamed"
echo "  sync of renames:   $carried"
