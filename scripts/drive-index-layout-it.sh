#!/usr/bin/env bash
#
# What a Library on real Google Drive does when this device finds an Index laid
# out by an older build, from one command and with nothing typed.
#
# Two outcomes are being checked, and neither of them can be decided against a
# mock. The first is that an Index whose catalog belongs to an older layout is
# thrown away and rebuilt from Storage — from what Drive holds, at the ids Drive
# minted — without a single Container going back up. The second is that an Index
# too old to be carried forward is still not a dead end: `coffret mappings`
# reads the mappings straight out of the refused file, so that the folders this
# device had mapped can be mapped back in rather than remembered.
#
# Both were checked by hand once, on one of the owner's own Libraries, with a
# person at the keyboard for the Passphrase. That is what this target replaces.
# The Library it makes is its own and it is tiny — three files of a few
# kilobytes, written by this script — because the question is about the Index
# and not about how much can be carried: a whole run is a handful of Drive
# calls, and the run after it uploads nothing at all.
#
# The state is deliberately kept rather than thrown away. Everything lives under
# `.tmp/drive-index-layout/`, which is gitignored, so the second run finds the
# Library the first one made, answers no consent, and checks the same two
# outcomes again on a Library that already existed — which is the more
# interesting half of the two.
#
# What a run said is kept there as well, in two files rather than one:
# `transcript.log` is what the CLI printed, and `report.log` is what this script
# made of it — the headings, every assertion, and the verdict at the end —
# appended run after run. A run nobody stood over would otherwise leave the one
# thing it was started for, the answer, on the terminal alone.
#
# Nothing here trashes or purges anything on Drive. The Library's app folder is
# created once and reused by every later run, so a run that finishes leaves the
# account with one `coffret-<library id>` folder rather than one per run.
# Removing it is the account owner's to do: discarding a Library from the
# command line is a flow coffret does not have yet.
#
# The two layout versions are taken out of the source at run time rather than
# written down here. They move whenever the Index gains a table, this script
# lives in the same tree that moves them, and a number copied into a test is a
# number that goes stale without saying so.
#
# What it needs:
#
#   COFFRET_DRIVE_FOLDER_ID      the folder on Drive to create the Library's
#                                app folder in, by the id in its address
#   COFFRET_DRIVE_CLIENT_ID      the OAuth desktop client to authorize as;
#                                needed by a run with a consent to answer,
#                                because `init` records it in the settings of
#                                the Library it puts here
#   COFFRET_DRIVE_CLIENT_SECRET  for a client registered with one; same
#
# And `sqlite3` on the PATH, which the assertions read the Index file with.

set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Everything this script writes is under one directory, and none of it is
# temporary: the run after this one opens the Library this one leaves.
readonly WORK="$ROOT/.tmp/drive-index-layout"
readonly STATE_DIR="$WORK/state"
readonly LOG_DIR="$WORK/logs"
readonly TRANSCRIPT="$WORK/transcript.log"
# And this script's own account of the run, which the transcript is not: the
# CLI's output is the evidence, and the headings, the assertions and the verdict
# are what was made of it. Kept because the verdict is the whole point of the
# run and a terminal nobody was sitting at keeps nothing.
readonly REPORT="$WORK/report.log"
# What the command being run said, on its own, for this script to read back.
readonly LAST="$WORK/last-command.log"
# And the same two streams kept apart, for the commands this script compares
# standard output of rather than watches go by.
readonly LAST_OUT="$WORK/last-command.out"
readonly LAST_ERR="$WORK/last-command.err"

# The name this device knows the Library by, and the name the copy of it that
# scenario B refuses is given. Both are device-side names and nothing on Drive
# carries either of them.
readonly LIBRARY="layout"
readonly REFUSED="refused"

# The one top-level component the Library maps, and the folder it is mapped to.
readonly PREFIX="notes"
readonly LOCAL_ROOT="$WORK/$LIBRARY/$PREFIX"

# The Index files the scenarios read the stamp out of and write it back into.
readonly INDEX="$STATE_DIR/libraries/$LIBRARY/index.sqlite"
readonly REFUSED_DIR="$STATE_DIR/libraries/$REFUSED"
readonly REFUSED_INDEX="$REFUSED_DIR/index.sqlite"

# The Passphrase the Library is created under and opened with.
#
# Fixed and in the clear on purpose: it protects three generated text files that
# exist to be re-created, and a check nobody can re-run unattended is not one
# this target could offer. A script that stops to ask a person for it would be
# the manual verification this one exists to replace. Nothing you would keep
# belongs in the Library under `.tmp/drive-index-layout/` for exactly that
# reason.
readonly PASSPHRASE="a coffret index layout check against real Drive"

# How many files the Library holds, and how many lines each of them is. Three,
# because the assertions are about the catalog holding the same Entries after a
# rebuild as before it and three is enough to tell an ordering apart from a
# count — and a few kilobytes each, because bytes are what a run spends on Drive
# and none of the outcomes here is about size.
readonly FILES=3
readonly FILE_LINES=64

# Where the two layout versions are written down, and the source of both.
readonly SCHEMA_FILE="$ROOT/backend/crates/gateway/coffret-sqlite-index/src/schema.rs"

# The two ways a grant that has died reaches the terminal, and they are two
# because a refresh token Google has expired is not a refresh token that was
# never there. The first is what the CLI says when the Library's token cache
# holds nothing or will not open. The second is the token endpoint's own
# refusal on the way up, and it is the one a run weeks after the last one gets:
# the cache still holds a refresh token, so nothing notices until Google is
# asked to spend it — and the consent screen a desktop client starts out on is
# in Testing, where Google expires a refresh token after seven days.
readonly NO_GRANT='no usable grant on Google Drive|Storage rejected the credentials'

# Waits for the copy of this run to be written before the run is over.
#
# The shell does not wait for the `tee` below on its way out, so the last lines
# of a run — the verdict, or whatever `fail` said about why there is none — can
# still be on their way to the file when whoever started the run reads it. This
# is what makes the report finished by the time the run is.
#
# `tee` copies until this shell's end of the pipe is gone, so letting go of the
# pipe has to come first: waiting on it while still holding it would be waiting
# forever. What is let go onto is the report file itself rather than nothing, so
# that a line printed after this — by an EXIT trap, which is the only thing that
# prints this late — is still written where the rest of the run was.
#
# A no-op before the report exists, which is the skip above and the two checks
# under it.
flush_the_report() {
  [ -n "${REPORT_TEE:-}" ] || return 0
  exec >>"$REPORT" 2>&1
  wait "$REPORT_TEE" 2>/dev/null || true
}

fail() {
  echo "$*" >&2
  flush_the_report
  exit 1
}

# The skip comes before everything, including the build, so that a caller
# checking that an unconfigured run is a no-op waits on nothing.
if [ -z "${COFFRET_DRIVE_FOLDER_ID:-}" ]; then
  echo "skipping the Index layout check: COFFRET_DRIVE_FOLDER_ID is not set."
  echo "Set it to the id of a folder on Drive to keep the Library's own folder"
  echo "in: open that folder in Drive and take the last part of its address, the"
  echo "part after /folders/. Set COFFRET_DRIVE_CLIENT_ID to the OAuth desktop"
  echo "client to authorize as."
  exit 0
fi

# The stamp is a `PRAGMA` and the Entries and Containers are rows, and this
# script asserts on both by asking the file itself rather than the CLI: what the
# CLI reports is the other half of the evidence, and a check that took the CLI's
# word for the state of the file would be checking it against itself.
command -v sqlite3 >/dev/null ||
  fail "sqlite3 is not on this PATH, and this target reads the Index file directly."

mkdir -p "$WORK" "$STATE_DIR" "$LOG_DIR" "$LOCAL_ROOT"

# From here on, everything this script says goes to the report as well as to the
# terminal — both streams, in the order they were said, which is the order a
# person at the terminal read them in. After the skip and the two checks above,
# so that a run configured for nothing leaves no file behind, and before the
# first word about this run, so that the report holds all of it.
#
# Appended rather than written over, because the run before this one is a
# reading of the same two questions and worth keeping beside this one. Which is
# why each run says at the top of its own block when it ran and what it was: an
# answer is only worth reading against the build it was asked of.
printf '\n=== run %s on %s %s ===\n' \
  "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" \
  "$(git -C "$ROOT" rev-parse --abbrev-ref HEAD 2>/dev/null || echo 'no branch')" \
  "$(git -C "$ROOT" rev-parse --short HEAD 2>/dev/null || echo 'no commit')" \
  >>"$REPORT"
# A process substitution rather than a pipeline, so that `pipefail` goes on
# answering for the commands this script runs and not for the copying. And
# `tee` holds nothing back: what reaches it is passed on as it arrives, so the
# consent URL still appears the moment the CLI prints it, which matters because
# it is the one thing in a run somebody is waiting at the terminal for.
exec > >(tee -a "$REPORT") 2>&1
REPORT_TEE=$!
readonly REPORT_TEE

# The Library and the log files both go under this directory rather than under
# the state directory of whoever started the run: a target that keeps state has
# to keep it somewhere it says, and this is where it says. It is also what keeps
# a run off the Libraries a person actually has.
export COFFRET_STATE_DIR="$STATE_DIR"
export COFFRET_LOG_DIR="$LOG_DIR"

# And the level, for the same reason the directory is set rather than read. Two
# of scenario A's assertions are about what the run recorded — that the discard
# was logged once, and that no Container went up — and an event that never
# reached the file is indistinguishable from one that never happened. A
# `COFFRET_LOG=warn` in the environment of whoever started the run would leave
# "uploaded no Container" holding on a run that had uploaded all three, which is
# the one failure this target exists to catch. `info` is the CLI's own default,
# so this pins what the assertions already assume rather than asking for more.
export COFFRET_LOG=info

# One of the two layout versions, out of the source that declares it.
schema_const() {
  local value
  value="$(sed -n "s/^pub(crate) const $1: i64 = \([0-9]\{1,\}\);.*/\1/p" "$SCHEMA_FILE")"
  [ -n "$value" ] || fail "$SCHEMA_FILE declares no $1; this script reads both versions from it."
  printf '%s\n' "$value"
}

SCHEMA_VERSION="$(schema_const SCHEMA_VERSION)"
DEVICE_SCHEMA_VERSION="$(schema_const DEVICE_SCHEMA_VERSION)"
readonly SCHEMA_VERSION DEVICE_SCHEMA_VERSION

# The two scenarios are the two sides of one boundary, and where the boundary is
# empty there is nothing here to check: a build whose device-local group moved
# with its catalog discards nothing and refuses everything older, so scenario A
# would be asserting on a refusal. Saying so is better than a run whose failures
# read as defects.
[ "$DEVICE_SCHEMA_VERSION" -lt "$SCHEMA_VERSION" ] ||
  fail "DEVICE_SCHEMA_VERSION is $DEVICE_SCHEMA_VERSION and SCHEMA_VERSION is $SCHEMA_VERSION: no stamp lies between them, so there is no older layout for this build to discard."

# What scenario B stamps its copy with: one below the oldest layout this build
# can carry forward, which is the whole of what makes it refused.
readonly TOO_OLD=$((DEVICE_SCHEMA_VERSION - 1))

# Whether a Library is on this device is the settings file and not the
# directory: an interrupted creation leaves a directory that nothing opens.
library_present() {
  [ -f "$STATE_DIR/libraries/$1/settings.json" ]
}

# One string out of a Library's settings file.
#
# Read back rather than remembered, so that the report says where the Library is
# on every run and not only on the one that created it.
settings_value() {
  local file="$STATE_DIR/libraries/$1/settings.json"
  grep -o "\"$2\"[[:space:]]*:[[:space:]]*\"[^\"]*\"" "$file" |
    sed -n '1s/.*"\([^"]*\)"$/\1/p'
}

# Which Library a command was for, out of what it was told.
#
# For the renewal command below, which has to name one: `--library` on the
# commands that act on a Library this device has, `--name` on the one that puts
# one there, and those are the only two flags that name one.
library_of() {
  local previous=""
  local word
  for word in "$@"; do
    case "$previous" in
      --library | --name)
        printf '%s\n' "$word"
        return
        ;;
    esac
    previous="$word"
  done
  printf '%s\n' "$LIBRARY"
}

# An expired grant is the one failure this script can say something useful
# about, and what it says is the command that renews it — spelled out here
# rather than quoted from the CLI. Only one of the two failures above carries a
# command at all, and the form it carries is the one a person with coffret
# installed would run: `coffret` is not on this PATH, and the Library this target
# keeps is under its own state directory rather than the one the CLI looks in
# when nothing says otherwise. So the line the CLI gave is shown for what it says
# went wrong, and the runnable form is given under it.
stop_at_a_dead_grant() {
  local file="$1"
  shift
  grep -Eq "$NO_GRANT" "$file" || return 0

  local library
  library="$(library_of "$@")"
  echo >&2
  grep -E "$NO_GRANT" "$file" >&2
  cat >&2 <<EOF

The grant on $library is gone. Renewing it means opening a browser at somebody,
which is a question this script was never asked, so it stops here instead. Run:

  printf '%s\n' '$PASSPHRASE' |
    COFFRET_STATE_DIR='$STATE_DIR' '$COFFRET' authorize --library $library --passphrase-stdin

EOF
  fail "It prints a URL of its own: open it, answer there, then run this target again."
}

# Runs the CLI, showing what it says as it happens and keeping a copy of it.
#
# Both streams are merged, in the order they happened: the consent URL goes to
# standard error and the summary to standard output, and a transcript that had
# to be read in two halves would be a worse account of the run than the terminal
# gave. What this script reads back out of the merged copy are lines distinct
# enough that nothing needs the two apart.
#
# Live rather than captured and printed afterwards, because the first run blocks
# on a person answering a consent screen the CLI is in the middle of printing the
# URL for.
#
# The status answered is the CLI's own — 0, 1, or 2 — rather than the pipeline's,
# so that a run which left findings is told apart from one that failed.
run_cli() {
  local status
  set +e
  printf '%s\n' "$PASSPHRASE" | "$COFFRET" "$@" 2>&1 | tee "$LAST"
  status=${PIPESTATUS[1]}
  set -e
  cat "$LAST" >>"$TRANSCRIPT"
  stop_at_a_dead_grant "$LAST" "$@"
  return "$status"
}

# The same, with the two streams kept apart.
#
# For the two commands whose standard output is itself an assertion: the
# mappings listing has to come back as the CLI printed it and nothing else, and
# the refused sync is read for what it says on standard error. Neither of them
# waits on anybody, so nothing is lost by capturing rather than watching — and
# both are echoed afterwards, because a run that asserts on output should show
# the output it asserted on.
run_cli_apart() {
  local status
  # Said before it runs, and not only written to the transcript: what these
  # commands print arrives all at once when they are done, and a listing that
  # came up with nothing above it would not say which command had printed it.
  printf '$ coffret %s\n' "$*"
  set +e
  printf '%s\n' "$PASSPHRASE" | "$COFFRET" "$@" >"$LAST_OUT" 2>"$LAST_ERR"
  # The CLI's own status, by position rather than by the pipeline's — which
  # under `pipefail` is the rightmost command that failed, and the Passphrase
  # being written is a command too. A command that never reads it and exits
  # while the line is still on its way — `mappings` is one — leaves `printf`
  # writing to a pipe nobody holds, and the run would read that as the CLI
  # having failed.
  status=${PIPESTATUS[1]}
  set -e
  {
    printf '$ coffret %s\n' "$*"
    cat "$LAST_OUT" "$LAST_ERR"
  } >>"$TRANSCRIPT"
  stop_at_a_dead_grant "$LAST_ERR" "$@"
  return "$status"
}

# The first line of a run's output that matches, which is the summary line.
said() {
  grep -m 1 "$1" "$LAST" || fail "$2"
}

# The log file the last run chose, which the CLI prints to standard error as it
# starts. Every run opens one of its own, so this is what makes "in that run's
# log" a question with an answer — the directory holds every earlier run's too.
log_of_the_last_run() {
  sed -n 's/^Logging this run to \(.*\)\.$/\1/p' "$LAST" | head -n 1
}

# The stamp in an Index file, and writing one back.
stamp_of() {
  sqlite3 "$1" 'PRAGMA user_version'
}

restamp() {
  sqlite3 "$1" "PRAGMA user_version = $2"
}

# What the catalog holds, in the two forms the scenarios compare.
#
# The Entries carry their Container and their hash rather than their path alone:
# a rebuild that lost the join or re-uploaded the bytes would leave the same
# three paths behind, and the point of scenario A is that neither happened.
entries_in() {
  sqlite3 -noheader -separator '|' "$1" \
    "SELECT path, lower(hex(container_id)), size, lower(hex(hash)) FROM entries ORDER BY path"
}

containers_in() {
  sqlite3 -noheader "$1" "SELECT lower(hex(id)) FROM containers ORDER BY 1"
}

# The Entry Paths the catalog holds, in the order the column collates in.
#
# The count on its own would be satisfied by three Entries that are not these
# three — a mapping left pointing at another folder, or a file somebody dropped
# beside the generated ones — and the whole of both scenarios is read against
# what this run says the Library holds before either of them starts.
paths_in() {
  sqlite3 -noheader "$1" "SELECT path FROM entries ORDER BY path"
}

rows_in() {
  sqlite3 -noheader "$1" "SELECT count(*) FROM $2"
}

# How many lines of a log file are the WARN this discard leaves.
#
# Matched on the fields rather than on the message alone, because the two
# versions are the whole of what the event is evidence for: a WARN that named no
# numbers would say a catalog had been discarded without saying which layout for.
# `grep` rather than `jq`, so that the target needs nothing on the device that
# the CLI it is checking does not.
discard_warnings_in() {
  grep -F '"level":"WARN"' "$1" |
    grep -F 'older layout' |
    grep -F "\"found\":$DEVICE_SCHEMA_VERSION" |
    grep -cF "\"supported\":$SCHEMA_VERSION" || true
}

uploads_in() {
  grep -cF 'uploaded a Container' "$1" || true
}

# The assertions, counted as they are made so that the summary can say how many
# held out of how many — and none of them stops the run. A scenario that stopped
# at its first disagreement would report one difference per run where it could
# have reported all of them, and these runs cost a person a consent to repeat.
assertions=0
failures=0

held() {
  assertions=$((assertions + 1))
  echo "  ok    $1"
}

broke() {
  assertions=$((assertions + 1))
  failures=$((failures + 1))
  echo "  FAIL  $1"
  echo "        expected: $2"
  echo "        found:    $3"
}

assert_equal() {
  if [ "$2" = "$3" ]; then
    held "$1"
  else
    broke "$1" "$2" "$3"
  fi
}

assert_says() {
  if grep -qF "$2" "$3"; then
    held "$1"
  else
    broke "$1" "a line holding \"$2\"" "nothing in $3 does"
  fi
}

# One of this Library's three files, the same bytes on every device and every
# run. Deterministic so that a second run finds what the first one synced rather
# than something new to upload, and generated here rather than by
# `coffret-fixtures` because a JPEG would be bytes spent on a question about the
# Index.
file_body() {
  local line
  for ((line = 1; line <= FILE_LINES; line++)); do
    printf 'entry %s line %03d %s\n' "$1" "$line" \
      "................................................"
  done
}

# Written only where what is on disk is not already it. Rewriting the same bytes
# would move the mtime, and a scan that finds a file it has seen before with a
# new mtime is a scan with work to do — which is the one thing scenario A
# asserts there is none of.
write_the_files() {
  local n path body
  for ((n = 1; n <= FILES; n++)); do
    path="$LOCAL_ROOT/entry-$n.txt"
    body="$(file_body "$n")"
    if [ -f "$path" ] && [ "$(cat "$path")" = "$body" ]; then
      continue
    fi
    printf '%s\n' "$body" >"$path"
  done
}

# The Entry Paths those files are carried into the Library as: the prefix the
# folder is mapped under, and the name under it (spec: EP-9).
expected_paths() {
  local n
  for ((n = 1; n <= FILES; n++)); do
    printf '%s/entry-%s.txt\n' "$PREFIX" "$n"
  done
}

echo "=== coffret Index layout against Google Drive ==="
echo
echo "Library and logs:  $WORK"
echo "Parent folder:     $COFFRET_DRIVE_FOLDER_ID"
echo "Passphrase:        a fixed test string; it protects generated text files and nothing else"
echo "Layout versions:   $SCHEMA_VERSION current, $DEVICE_SCHEMA_VERSION the oldest this build carries forward"
echo

# What the run is going to ask of whoever started it, said before the build
# rather than when the URL appears: a run that asks for nothing can be walked
# away from, and a run that asks is one to stay at the terminal for.
if library_present "$LIBRARY"; then
  echo "$LIBRARY is already on this device: no consent to answer, and nothing in"
  echo "this run waits on you."
else
  if [ -z "${COFFRET_DRIVE_CLIENT_ID:-}" ]; then
    fail "COFFRET_DRIVE_CLIENT_ID is not set, and putting a Library on this device needs an OAuth desktop client to authorize as."
  fi
  echo "One consent to answer. It prints a URL for you to open — nothing opens a"
  echo "browser for you — and waits there, giving up after five minutes. The"
  echo "build comes first, and the whole run takes a couple of minutes."
fi
echo

# And what stopping it part-way costs, said before there is anything to stop:
# the state this target keeps is state it also repairs, and a person who has
# just pressed Ctrl-C has no way of knowing that from what is on the screen.
echo "Stopping this part-way costs nothing to put right: the next run's first"
echo "sync rebuilds an Index left at an older stamp, and the copy the second"
echo "scenario makes is removed on the way out and again by the run after it."
echo

# 1. The binary. Nothing else is built: the files this Library holds are written
#    by this script.
echo "--- building the CLI ---"
cd "$ROOT/backend"
cargo build --release -p coffret-cli
readonly COFFRET="$ROOT/backend/target/release/coffret"

# 2. The Library, on the first run only.
if ! library_present "$LIBRARY"; then
  echo
  echo "--- creating the Library as $LIBRARY ---"
  echo "A URL is about to be printed, and the run waits at it until you have"
  echo "opened it in a browser and answered. Nothing on the account is read or"
  echo "changed beyond the folder coffret creates — a new one, so where the"
  echo "Library was removed from this device by hand, the folder the run before"
  echo "it made stays on the account untouched."
  echo
  run_cli init \
    --name "$LIBRARY" \
    --drive \
    --parent "$COFFRET_DRIVE_FOLDER_ID" \
    --passphrase-stdin ||
    fail "
$LIBRARY was not created; the lines above say what stopped it, and a consent
nobody answered is one of the things they can say. Nothing was kept on this
device, so running this target again makes the same first run over — but where
those lines name a folder on Drive, or say to look for one, that folder was
created before the failure and is the account's to remove."

  # The CLI's warning is the right one for a Library somebody keeps, and the
  # wrong one to act on here, so the run says which of the two this is.
  echo
  echo "That warning is the CLI's own. Nothing here needs writing down: the"
  echo "Library holds $FILES generated files and any later run opens it with the"
  echo "fixed Passphrase above."
fi

# 3. The files, and the folder they are in recorded as part of the
#    Library. The mapping is recorded again on every run; the CLI says what the
#    prefix stood for before, and saying it again about the same folder is the
#    answer that nothing moved.
echo
echo "--- the $FILES files under $PREFIX/ ---"
write_the_files
echo "$(find "$LOCAL_ROOT" -type f | wc -l | tr -d ' ') files, $(du -sk "$LOCAL_ROOT" | cut -f1) KB."
echo
run_cli map --library "$LIBRARY" --prefix "$PREFIX" "$LOCAL_ROOT"

# 4. The starting point both scenarios are read against. On the first run this
#    is the upload; on every later one it is the run that proves there is
#    nothing left to upload.
echo
echo "--- carrying $PREFIX into $LIBRARY ---"
status=0
run_cli sync --library "$LIBRARY" --passphrase-stdin || status=$?
[ "$status" = 0 ] ||
  fail "the first sync exited $status, and this target's Library holds nothing anybody deleted: the lines above say what it found."

echo
echo "--- what the Library holds before either scenario ---"
assert_equal "the catalog holds the $FILES files under $PREFIX/ and nothing else" \
  "$(expected_paths)" "$(paths_in "$INDEX")"
assert_equal "the Index is stamped at the current layout" \
  "$SCHEMA_VERSION" "$(stamp_of "$INDEX")"

# What a captured command said is in a file rather than on the terminal, so a
# failure of one shows it: a verdict with the error it was reached from nowhere
# on the screen is a run nobody can act on without going looking.
run_cli_apart mappings --library "$LIBRARY" || {
  cat "$LAST_ERR" >&2
  fail "coffret mappings failed on $LIBRARY before either scenario had run."
}
mappings_before="$(cat "$LAST_OUT")"
entries_before="$(entries_in "$INDEX")"
containers_before="$(containers_in "$INDEX")"
[ -n "$mappings_before" ] || fail "$LIBRARY has no mappings recorded; the scenarios need at least one."
[ -n "$containers_before" ] || fail "the catalog holds no Containers; the scenarios have nothing to compare."
echo "  mapped: $mappings_before"

# 5. Scenario A. The catalog is thrown away by the next open, and the sync after
#    it rebuilds from Drive rather than uploading to it.
echo
echo "=== scenario A: an older layout is discarded and rebuilt from Drive ==="
echo
echo "--- stamping the Index at $DEVICE_SCHEMA_VERSION, the oldest layout this build carries forward ---"
restamp "$INDEX" "$DEVICE_SCHEMA_VERSION"
assert_equal "the Index now claims the older layout" \
  "$DEVICE_SCHEMA_VERSION" "$(stamp_of "$INDEX")"
echo

echo "--- syncing again, the open that discards the catalog and rebuilds it ---"
status=0
run_cli sync --library "$LIBRARY" --passphrase-stdin || status=$?
log="$(log_of_the_last_run)"
[ -n "$log" ] || fail "the sync did not say which log file it was writing to."

echo
assert_equal "the sync succeeded" 0 "$status"

summary="$(said '^added ' "sync printed no summary.")"
assert_equal "it added nothing and found the $FILES files unchanged" \
  "added 0, replaced 0, unchanged $FILES" "${summary%,*}"
assert_equal "the Index is stamped at the current layout again" \
  "$SCHEMA_VERSION" "$(stamp_of "$INDEX")"

run_cli_apart mappings --library "$LIBRARY" || {
  cat "$LAST_ERR" >&2
  fail "coffret mappings failed on $LIBRARY after the rebuild."
}
assert_equal "the mappings survived the discard" "$mappings_before" "$(cat "$LAST_OUT")"
assert_equal "the rebuilt catalog holds the same $FILES Entries" \
  "$entries_before" "$(entries_in "$INDEX")"
assert_equal "and the same Containers, so nothing was re-packed" \
  "$containers_before" "$(containers_in "$INDEX")"
assert_equal "nothing is left spooled to upload" 0 "$(rows_in "$INDEX" pending_uploads)"
assert_equal "the run logged the discard once, with both versions" \
  1 "$(discard_warnings_in "$log")"
assert_equal "and uploaded no Container" 0 "$(uploads_in "$log")"

# 6. Scenario B. A copy of the Library, stamped older than anything this build
#    carries forward, so that the file is refused rather than discarded.
#
#    A copy because the outcome being checked is what a person is left with when
#    the Index is a dead end, and leaving the real Library there would be leaving
#    this target's own state in that condition for the next run. It points at the
#    same folder on Drive and nothing in this scenario reaches Drive at all: the
#    Index is refused before any of it is opened.
echo
echo "=== scenario B: a refused Index still lists its mappings ==="
echo
echo "--- copying $LIBRARY to $REFUSED and stamping it at $TOO_OLD, below the $DEVICE_SCHEMA_VERSION this build carries forward ---"
rm -rf "$REFUSED_DIR"
cp -r "$STATE_DIR/libraries/$LIBRARY" "$REFUSED_DIR"
# From here on the copy goes whatever happens, including an assertion that
# stopped the run: it is a Library this device would otherwise offer on the next
# `mappings` listing, and it was never one.
trap 'rm -rf "$REFUSED_DIR"' EXIT
restamp "$REFUSED_INDEX" "$TOO_OLD"
echo

status=0
run_cli_apart mappings --library "$REFUSED" || status=$?
cat "$LAST_OUT"
cat "$LAST_ERR" >&2
echo
assert_equal "coffret mappings succeeded on the refused file" 0 "$status"
assert_equal "and listed what the working Library lists" "$mappings_before" "$(cat "$LAST_OUT")"
assert_says "it said the Index cannot be opened" "cannot be opened" "$LAST_ERR"
assert_says "and named coffret map to record them back" "coffret map" "$LAST_ERR"
assert_says "and coffret sync to finish with" "coffret sync" "$LAST_ERR"
assert_equal "reading a refused file did not restamp it" "$TOO_OLD" "$(stamp_of "$REFUSED_INDEX")"

status=0
run_cli_apart sync --library "$REFUSED" --passphrase-stdin || status=$?
cat "$LAST_ERR" >&2
echo
if [ "$status" = 0 ]; then
  broke "coffret sync refused the older layout" "a non-zero exit" "0"
else
  held "coffret sync refused the older layout, exiting $status"
fi
assert_says "and said which layout it found" "schema version $TOO_OLD" "$LAST_ERR"

rm -rf "$REFUSED_DIR"
trap - EXIT
echo
echo "$REFUSED is gone from this device again."

# 7. What the run decided, in one block, so that nobody has to read back up.
library_id="$(settings_value "$LIBRARY" library_id)"
folder_id="$(settings_value "$LIBRARY" folder_id)"

echo
if [ "$failures" = 0 ]; then
  echo "=== both scenarios held: $assertions assertions ==="
else
  echo "=== $failures of $assertions assertions did not hold ==="
fi
echo
echo "App folder:      coffret-$library_id"
echo "  its id:        $folder_id"
echo "  inside:        $COFFRET_DRIVE_FOLDER_ID"
echo "  to look at it: https://drive.google.com/drive/folders/$folder_id"
echo "Layout:          discarded at $DEVICE_SCHEMA_VERSION, rebuilt to $SCHEMA_VERSION; refused at $TOO_OLD"
echo
echo "Transcript:      $TRANSCRIPT"
echo "Report:          $REPORT"
echo "CLI logs:        $LOG_DIR"
echo "  the rebuild's: $log"
echo "Libraries:       $STATE_DIR/libraries"
echo
echo "Run this again to check the same two outcomes without a consent. Nothing on"
echo "the account is removed by it, and nothing is added either: the app folder"
echo "above is the only one coffret made, and every run re-syncs the same $FILES files."

flush_the_report
[ "$failures" = 0 ] || exit 1
