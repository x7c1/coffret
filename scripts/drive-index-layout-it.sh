#!/usr/bin/env bash
#
# What a Library on real Google Drive does when this device finds an Index laid
# out by an older build, and what it does when the grant on it is gone — from
# one command and with nothing typed.
#
# Three outcomes are being checked, and none of them can be decided against a
# mock. Scenario A is that an Index whose catalog belongs to an older layout is
# thrown away and rebuilt from Storage — from what Drive holds, at the ids Drive
# minted — without a single Container going back up. Scenario B is that an Index
# too old to be carried forward is still not a dead end: `coffret mappings`
# reads the mappings straight out of the refused file, so that the folders this
# device had mapped can be mapped back in rather than remembered. Scenario C is
# that a Library whose grant is gone says so in one line and names the command
# that renews it, rather than failing somewhere further up: that is what a
# person meets the week their consent screen's Testing-mode grant expires, and
# no other target checks that the CLI says it.
#
# A and B are the two sides of one boundary — the oldest layout this build
# carries forward, and the current one — and where that boundary is empty they
# have nothing to check and are skipped, with the run saying so. Scenario C
# stands on no boundary and runs either way, so a build whose two layout
# versions are equal still has a scenario here rather than none.
#
# A and B were checked by hand once, on one of the owner's own Libraries, with a
# person at the keyboard for the Passphrase. That is what this target replaces.
# The Library it makes is its own and it is tiny — three files of a few
# kilobytes, written by this script — because the question is about the Index
# and not about how much can be carried: a whole run is a handful of Drive
# calls, and the run after it uploads nothing at all.
#
# The state is deliberately kept rather than thrown away. Everything lives under
# `.tmp/drive-index-layout/`, which is gitignored, so the second run finds the
# Library the first one made, answers no consent, and checks the same outcomes
# again on a Library that already existed — which is the more interesting of the
# two runs.
#
# What a run said is kept there as well, in two files rather than one:
# `transcript.log` is what the CLI printed, and `report.log` is what this script
# made of it — the headings, every assertion, and the verdict at the end —
# appended run after run, each block ending with the status the run exited on. A
# run nobody stood over would otherwise leave the one thing it was started for,
# the answer, on the terminal alone.
#
# Nothing here trashes or purges anything on Drive. The Library's app folder is
# created once and reused by every later run, so a run that finishes leaves the
# account with one `coffret-<library id>` folder rather than one per run.
# `make drive-it-list` shows what the account holds and `make drive-it-reset`
# takes it away again, this target's state with it: discarding a Library from
# the command line is a flow coffret does not have yet, so that is a reset
# rather than a deletion the CLI knows about.
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
#   COFFRET_DRIVE_CLIENT_ID      the OAuth desktop client to authorize as,
#                                which this script passes to `init` as
#                                `--client-id`; needed by a run with a consent
#                                to answer, because `init` records it in the
#                                settings of the Library it puts here
#   COFFRET_DRIVE_CLIENT_SECRET  for a client registered with one. It has no
#                                flag: the CLI reads this variable out of the
#                                environment itself, so nothing here passes it
#                                on a command line
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

# The name this device knows the Library by, and the names the two copies of it
# are given: the one scenario B stamps too old to open, and the one scenario C
# takes the grant away from. All three are device-side names and nothing on
# Drive carries any of them.
readonly LIBRARY="layout"
readonly REFUSED="refused"
readonly UNGRANTED="ungranted"

# The file a Library's sealed grant is kept in, beside its settings, its stored
# Master Key and its Index. Taking it away is the whole of what makes scenario
# C's copy a Library with no grant.
readonly TOKEN_CACHE="token-cache.cftc"

# The one top-level component the Library maps, and the folder it is mapped to.
readonly PREFIX="notes"
readonly LOCAL_ROOT="$WORK/$LIBRARY/$PREFIX"

# The Index files the scenarios read the stamp out of and write it back into.
readonly INDEX="$STATE_DIR/libraries/$LIBRARY/index.sqlite"
readonly REFUSED_DIR="$STATE_DIR/libraries/$REFUSED"
readonly REFUSED_INDEX="$REFUSED_DIR/index.sqlite"
readonly UNGRANTED_DIR="$STATE_DIR/libraries/$UNGRANTED"
readonly UNGRANTED_INDEX="$UNGRANTED_DIR/index.sqlite"

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

# The first of those two on its own, as a fixed string, because scenario C
# arranges exactly that half — a cache that holds nothing — and asserting on the
# pair would let the other one pass for it. Cut out of the constant rather than
# written again, so that the line scenario C expects and the line this script
# stops at cannot drift apart.
readonly NO_CACHED_GRANT="${NO_GRANT%%|*}"

# And the command the CLI names in it, which is the other half of what scenario
# C is about: a refusal that says what went wrong without saying what to do
# about it leaves a person to go looking.
readonly RENEWAL="coffret authorize"

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
# A no-op before the report exists, which is the skip below and the `sqlite3`
# check under it.
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

# The copy a scenario has made and nothing should find after the run, or nothing
# where there is none. Set while the copy is there and cleared once the scenario
# has taken it away itself, which is what the trap below reads.
COPY_TO_REMOVE=""

# What is due on the way out: the copy, and then the last line of this run's
# block, which says how the run ended.
#
# One trap rather than one per errand, because a shell has one EXIT trap and a
# second `trap ... EXIT` would silently replace the first. Scenarios B and C each
# leave a Library this device would otherwise offer on the next `mappings`
# listing and was never one, so the removal goes on happening whatever stopped
# the run — including an assertion that stopped it.
#
# A removal that fails is said and then left behind rather than allowed to stop
# this. Every run is promised a last line, and a directory this script could not
# take away is no reason to break that promise — or to turn a run that held into
# one that failed, which is what letting `set -e` have the failure would do.
#
# The line is written from here because this is the one place that runs whatever
# happened: `fail`, the end of the script, and a line nobody wrote a `fail` for
# alike. A run that died under `set -e` on such a line leaves a block that simply
# stops, and a block that stops reads the same as a run still going or one whose
# terminal was closed. A run somebody stopped ends up here too, by way of the
# signal traps below; a terminal that was closed on one still does not.
#
# The report is let go of between the two errands rather than after both: a
# removal that failed is something to see at the terminal, and a run stopped
# part-way is still writing to one. The line goes after that, once this shell
# holds the file rather than the pipe, because a signal that stopped the run can
# have taken the `tee` copying into the file with it — and a line written into a
# pipe nobody reads is a line nobody gets.
on_the_way_out() {
  local status=$?
  if [ -n "$COPY_TO_REMOVE" ] && ! rm -rf "$COPY_TO_REMOVE"; then
    echo "$COPY_TO_REMOVE is still here: the next coffret mappings on this"
    echo "device will list it as a Library, and it never was one."
  fi
  flush_the_report
  case "$status" in
    0) echo "=== run exited 0: every assertion made held ===" ;;
    1) echo "=== run exited 1: an assertion did not hold, or the lines above say what stopped the run ===" ;;
    130|143) echo "=== run exited $status: stopped by a signal, and says nothing either way ===" ;;
    *) echo "=== run exited $status ===" ;;
  esac
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
# person at the terminal read them in. After the skip and the `sqlite3` check
# above, so that a run configured for nothing leaves no file behind, and before
# the first word about this run, so that the report holds all of it.
#
# Appended rather than written over, because the run before this one is a
# reading of the same questions and worth keeping beside this one. Which is
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
#
# The copying is kept out of the way of a `kill`, because everything in a run is
# one process group and a signal sent to the group reaches this `tee` as well as
# the script. The shell prints a note when a command is terminated under it, and
# with the `tee` gone that note goes into a pipe nobody reads: the write kills
# the shell before the trap below can exit with the 143 it means to. Ctrl-C
# needs nothing of the kind: an interrupted command gets no such note, and a
# child the shell started in the background — which is what this one is —
# ignores that signal already.
exec > >(trap '' TERM; tee -a "$REPORT") 2>&1
REPORT_TEE=$!
readonly REPORT_TEE

# Installed with the report and not before it: until there is a file, a run that
# skipped has nothing to say how it ended in — and the copies the trap removes
# are made further down, long after this.
#
# The two signals are turned into an ordinary exit rather than trapped in their
# own right, because a shell a signal kills outright runs no EXIT trap at all —
# neither the line nor the copy would be seen to. Ctrl-C at the consent is where
# that happens: waiting five minutes for a browser is the one stretch of a run
# long enough for anybody to give up on it. The statuses are the ones a shell
# gives a signal, 128 and the number of the signal.
trap on_the_way_out EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

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

# Scenarios A and B are the two sides of one boundary, and where the boundary is
# empty there is nothing for them to check: a build whose device-local group
# moved with its catalog discards nothing and refuses everything older, so
# scenario A would be asserting on a refusal. They are skipped rather than run,
# and the run says why — a failure that reads as a defect is worse than a
# scenario that says it had nothing to ask.
#
# It is those two that are skipped and not the run. Scenario C is about a grant
# and not about a layout, so it has the same question to ask on either build,
# and a target that refused outright here would leave this build with no
# scenario at all.
if [ "$DEVICE_SCHEMA_VERSION" -lt "$SCHEMA_VERSION" ]; then
  readonly LAYOUT_SCENARIOS=yes
  readonly SCENARIOS_RUN="scenarios A, B and C"
else
  readonly LAYOUT_SCENARIOS=no
  readonly SCENARIOS_RUN="scenario C alone"
fi

# Why they were skipped. Said where the scenarios would have been rather than
# here, so that it is read in its place.
readonly EMPTY_BOUNDARY="DEVICE_SCHEMA_VERSION is $DEVICE_SCHEMA_VERSION and SCHEMA_VERSION is $SCHEMA_VERSION: no stamp lies between them, so there is no older layout for this build to discard."

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

# The same, with the two streams kept apart, and without the stop at a dead
# grant.
#
# For the commands whose standard output is itself an assertion: the mappings
# listing has to come back as the CLI printed it and nothing else, and the two
# refused syncs are read for what they say on standard error. None of them waits
# on anybody, so nothing is lost by capturing rather than watching — and all of
# them are echoed afterwards, because a run that asserts on output should show
# the output it asserted on.
#
# The stop is left to the caller because one caller is scenario C, where a dead
# grant is the answer being checked rather than a reason to halt.
run_cli_apart_without_the_stop() {
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
  return "$status"
}

# And with it, which is what every caller but scenario C wants: a grant that has
# died in the middle of a run says nothing about the outcome being checked, and
# going on would spend the rest of the run asserting on a Library nothing can
# reach.
run_cli_apart() {
  local status=0
  run_cli_apart_without_the_stop "$@" || status=$?
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
#
# Out of the merged copy by default, and out of whichever file was given where
# the two streams were kept apart: the line is one the CLI writes to standard
# error, so a captured run has it in `$LAST_ERR` and nowhere else.
log_of_the_last_run() {
  sed -n 's/^Logging this run to \(.*\)\.$/\1/p' "${1:-$LAST}" | head -n 1
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
# beside the generated ones — and the whole of scenarios A and B is read against
# what this run says the Library holds before they start.
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
echo "Scenarios:         $SCENARIOS_RUN"
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
#
# What there is to put right depends on which scenarios run: where A and B are
# skipped, nothing restamps the Index and the only copy made is scenario C's.
if [ "$LAYOUT_SCENARIOS" = yes ]; then
  echo "Stopping this part-way costs nothing to put right: the next run's first"
  echo "sync rebuilds an Index left at an older stamp, and the copies the later"
  echo "scenarios make are removed on the way out and again by the run after them."
else
  echo "Stopping this part-way costs nothing to put right: the copy scenario C"
  echo "makes is removed on the way out and again by the run after it."
fi
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
    --client-id "$COFFRET_DRIVE_CLIENT_ID" \
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

# 4. The starting point scenarios A and B are read against. On the first run
#    this is the upload; on every later one it is the run that proves there is
#    nothing left to upload.
echo
echo "--- carrying $PREFIX into $LIBRARY ---"
status=0
run_cli sync --library "$LIBRARY" --passphrase-stdin || status=$?
[ "$status" = 0 ] ||
  fail "the first sync exited $status, and this target's Library holds nothing anybody deleted: the lines above say what it found."

echo
echo "--- what the Library holds before any scenario ---"
assert_equal "the catalog holds the $FILES files under $PREFIX/ and nothing else" \
  "$(expected_paths)" "$(paths_in "$INDEX")"
assert_equal "the Index is stamped at the current layout" \
  "$SCHEMA_VERSION" "$(stamp_of "$INDEX")"

# What a captured command said is in a file rather than on the terminal, so a
# failure of one shows it: a verdict with the error it was reached from nowhere
# on the screen is a run nobody can act on without going looking.
run_cli_apart mappings --library "$LIBRARY" || {
  cat "$LAST_ERR" >&2
  fail "coffret mappings failed on $LIBRARY before any scenario had run."
}
mappings_before="$(cat "$LAST_OUT")"
entries_before="$(entries_in "$INDEX")"
containers_before="$(containers_in "$INDEX")"
[ -n "$mappings_before" ] || fail "$LIBRARY has no mappings recorded; the scenarios need at least one."
[ -n "$containers_before" ] || fail "the catalog holds no Containers; the scenarios have nothing to compare."
echo "  mapped: $mappings_before"

# 5. Scenarios A and B, where this build has a boundary for them to stand on.
#    Where it has none they are skipped, with the reason said in their place,
#    and the run goes on to the scenario that does not need one.
#
#    The log file the rebuild wrote is named at the end of the run, so the name
#    is declared here rather than inside: a scenario A that did not run leaves
#    no log to name, and the summary leaves that line out rather than reading
#    an unset name.
log=""
if [ "$LAYOUT_SCENARIOS" = no ]; then
  echo
  echo "=== scenarios A and B: skipped ==="
  echo
  echo "$EMPTY_BOUNDARY"
  echo
  echo "Both of them are about the boundary between those two layouts, so on this"
  echo "build there is nothing for either to ask. Scenario C below is about a"
  echo "grant rather than a layout and runs as it always does."
else
  # Scenario A. The catalog is thrown away by the next open, and the sync after
  # it rebuilds from Drive rather than uploading to it.
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

  # Scenario B. A copy of the Library, stamped older than anything this build
  # carries forward, so that the file is refused rather than discarded.
  #
  # A copy because the outcome being checked is what a person is left with when
  # the Index is a dead end, and leaving the real Library there would be leaving
  # this target's own state in that condition for the next run. It points at the
  # same folder on Drive and nothing in this scenario reaches Drive at all: the
  # Index is refused before any of it is opened.
  echo
  echo "=== scenario B: a refused Index still lists its mappings ==="
  echo
  echo "--- copying $LIBRARY to $REFUSED and stamping it at $TOO_OLD, below the $DEVICE_SCHEMA_VERSION this build carries forward ---"
  rm -rf "$REFUSED_DIR"
  cp -r "$STATE_DIR/libraries/$LIBRARY" "$REFUSED_DIR"
  # From here on the copy goes whatever happens. Handed to the trap above rather
  # than trapped here, so that the line saying how the run ended is not replaced
  # by it.
  COPY_TO_REMOVE="$REFUSED_DIR"
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
  COPY_TO_REMOVE=""
  echo
  echo "$REFUSED is gone from this device again."
fi

# 6. Scenario C. A second copy of the Library, with its sealed grant taken away,
#    so that the next call through Drive's Storage has nothing to spend.
#
#    A copy for scenario B's reason: the outcome is what a person is left with
#    when the grant on a Library is gone, and arranging that on the Library this
#    target keeps would arrange it for every later run as well. The copy points
#    at the same folder on Drive and nothing here reaches Drive at all — the
#    missing cache is found while the store is being built, before the catalog
#    is opened and long before a call goes out — so this scenario spends no
#    grant and asks nobody for a consent.
#
#    Taking the file away is the half of a dead grant that can be arranged from
#    here. The other half, a refresh token Google has expired, needs the token
#    itself, which the sealed cache does not hand out; the token endpoint's own
#    refusal is covered by the gateway's unit tests.
echo
echo "=== scenario C: a Library whose grant is gone says so and names the renewal ==="
echo
echo "--- copying $LIBRARY to $UNGRANTED and taking its $TOKEN_CACHE away ---"
rm -rf "$UNGRANTED_DIR"
cp -r "$STATE_DIR/libraries/$LIBRARY" "$UNGRANTED_DIR"
# As in scenario B: the copy goes whatever happens.
COPY_TO_REMOVE="$UNGRANTED_DIR"
# Asserted to be there before it is removed, because a Library that never held
# one would leave this scenario checking the same refusal for a different
# reason — and saying nothing about a grant that was spent and is gone.
[ -f "$UNGRANTED_DIR/$TOKEN_CACHE" ] ||
  fail "$LIBRARY has no $TOKEN_CACHE for the copy to lose; this scenario is about a grant that was there."
rm -f "$UNGRANTED_DIR/$TOKEN_CACHE"

# What the copy's Index holds before the refused run — its stamp and its
# Entries — read off the copy rather than carried over from the working Library
# or from the layout constants: the assertion is that this run left the file
# where it found it, and a baseline taken here says so whatever the scenarios
# above did to the Library this copy was made from.
ungranted_stamp_before="$(stamp_of "$UNGRANTED_INDEX")"
ungranted_entries_before="$(entries_in "$UNGRANTED_INDEX")"
echo

# Without the stop at a dead grant, which every other command here runs with:
# the dead grant is what this scenario arranged and what it reads the answer of.
status=0
run_cli_apart_without_the_stop sync --library "$UNGRANTED" --passphrase-stdin || status=$?
cat "$LAST_OUT"
cat "$LAST_ERR" >&2
ungranted_log="$(log_of_the_last_run "$LAST_ERR")"
echo
if [ "$status" = 0 ]; then
  broke "coffret sync refused the Library with no grant" "a non-zero exit" "0"
else
  held "coffret sync refused the Library with no grant, exiting $status"
fi
assert_says "it said the cache holds no usable grant" "$NO_CACHED_GRANT" "$LAST_ERR"
assert_says "and named $RENEWAL, the command that renews one" "$RENEWAL" "$LAST_ERR"
assert_equal "the refusal left the Index stamped where it was" \
  "$ungranted_stamp_before" "$(stamp_of "$UNGRANTED_INDEX")"
assert_equal "and left the same Entries in it" \
  "$ungranted_entries_before" "$(entries_in "$UNGRANTED_INDEX")"
if [ -n "$ungranted_log" ]; then
  assert_equal "and uploaded no Container" 0 "$(uploads_in "$ungranted_log")"
else
  broke "the refused run said which log file it was writing to" \
    "a log file named on standard error" "no such line"
fi

rm -rf "$UNGRANTED_DIR"
COPY_TO_REMOVE=""
echo
echo "$UNGRANTED is gone from this device again."

# 7. What the run decided, in one block, so that nobody has to read back up.
library_id="$(settings_value "$LIBRARY" library_id)"
folder_id="$(settings_value "$LIBRARY" folder_id)"

echo
# Which scenarios the count is out of, and not the count alone: the same number
# of assertions holding means a different thing on a build where A and B ran.
if [ "$failures" = 0 ]; then
  echo "=== $SCENARIOS_RUN held: $assertions assertions ==="
else
  echo "=== $failures of $assertions assertions did not hold, in $SCENARIOS_RUN ==="
fi
echo
echo "App folder:      coffret-$library_id"
echo "  its id:        $folder_id"
echo "  inside:        $COFFRET_DRIVE_FOLDER_ID"
echo "  to look at it: https://drive.google.com/drive/folders/$folder_id"
if [ "$LAYOUT_SCENARIOS" = yes ]; then
  echo "Layout:          A discarded at $DEVICE_SCHEMA_VERSION and rebuilt to $SCHEMA_VERSION; B refused at $TOO_OLD"
else
  echo "Layout:          A and B skipped; $SCHEMA_VERSION is both the current layout and the oldest carried forward"
fi
echo "Grant:           C refused a copy with no $TOKEN_CACHE, without reaching Drive"
echo
echo "Transcript:      $TRANSCRIPT"
echo "Report:          $REPORT"
# The directory holds one file per run, this run's among every earlier run's, so
# the ones an assertion was read out of are named under it. An "uploaded no
# Container" that did not hold sends whoever reads it to a log file, and a
# directory holding every run's is not one.
echo "CLI logs:        $LOG_DIR"
if [ -n "$log" ]; then
  echo "  the rebuild's: $log"
fi
if [ -n "$ungranted_log" ]; then
  echo "  the refusal's: $ungranted_log"
fi
echo "Libraries:       $STATE_DIR/libraries"
echo
echo "Run this again to check the same outcomes without a consent. Nothing on the"
echo "account is removed by it, and nothing is added either: the app folder above"
echo "is the only one coffret made, and every run re-syncs the same $FILES files."

flush_the_report
[ "$failures" = 0 ] || exit 1
