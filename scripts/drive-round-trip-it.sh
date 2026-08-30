#!/usr/bin/env bash
#
# A folder into a Library on real Google Drive, and back out of it again, from
# one command.
#
# What this covers is what no mock stands in for: the ids Drive mints, the
# grant it hands back, and what it says when a folder is listed — carrying a
# Library from the device that made it to a device that only ever had its
# Recovery Code.
#
# Two Libraries stand for two devices, because that is what a second device is
# from Storage's point of view: `main` creates the Library and syncs a folder
# into it, `second` takes the same Library up from the Recovery Code and
# fetches it into a folder of its own.
#
# The state is deliberately kept rather than thrown away. Everything lives
# under `.tmp/drive-round-trip/`, which is gitignored, so a second run finds
# the two Libraries the first one made: it asks for no consent, adds a fresh
# batch of files beside the earlier ones, and commits the next head.
#
# Nothing here trashes or purges anything on Drive. The Library's app folder is
# created once and reused by every later run, so a run that finishes leaves the
# account with one `coffret-<library id>` folder rather than one per run.
# Removing it is the account owner's to do: discarding a Library from the
# command line is a flow coffret does not have yet.
#
# One way a second folder turns up is a first run that failed after Drive
# had already minted one, which by then means a failure writing the Library's
# own files: the folder is minted only once the consent has been answered, so a
# consent nobody answered in time leaves nothing behind. Adopting an abandoned
# folder is a flow coffret does not have either, so the next run creates a
# Library of its own beside it rather than taking it up. `init` says so in its
# refusal, by id where it has one and by "look for a `coffret-` folder" where
# the create is what failed and the id never came back.
#
# What it needs:
#
#   COFFRET_DRIVE_FOLDER_ID      the folder on Drive to create the Library's
#                                app folder in, by the id in its address
#   COFFRET_DRIVE_CLIENT_ID      the OAuth desktop client to authorize as;
#                                needed by a run with a consent to answer,
#                                because `init` and `join` each record it in
#                                the settings of the Library they put here
#   COFFRET_DRIVE_CLIENT_SECRET  for a client registered with one; same

set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Everything this script writes is under one directory, and none of it is
# temporary: the run after this one opens the Libraries this one leaves.
readonly WORK="$ROOT/.tmp/drive-round-trip"
readonly STATE_DIR="$WORK/state"
readonly LOG_DIR="$WORK/logs"
readonly TRANSCRIPT="$WORK/transcript.log"
# What the command being run said, on its own, for this script to read back.
readonly LAST="$WORK/last-command.log"

# The two names the two devices know the one Library by. They are device-side
# names and nothing on Drive carries either of them.
readonly UPLOADER="main"
readonly JOINER="second"

# The one top-level component both devices map, and the folder each maps it to.
readonly PREFIX="runs"
readonly UPLOADER_ROOT="$WORK/$UPLOADER/$PREFIX"
readonly JOINER_ROOT="$WORK/$JOINER/$PREFIX"

# The Passphrase both Libraries are created under and opened with.
#
# Fixed and in the clear on purpose: it protects a Library of generated JPEGs
# that exists to be re-created, and a round trip nobody can re-run unattended
# is not one this target could offer. Nothing you would keep belongs in the
# Libraries under `.tmp/drive-round-trip/` for exactly that reason.
readonly PASSPHRASE="a coffret round trip against real Drive"

# How much each run adds. Small: this is a round trip and not a benchmark, and
# every run's files stay on the account and on this disk.
PHOTOS="${COFFRET_ROUND_TRIP_PHOTOS:-12}"
PAGES="${COFFRET_ROUND_TRIP_PAGES:-3}"

# What a run that succeeded but left findings exits with.
readonly FINDINGS=2

# The two ways a grant that has died reaches the terminal, and they are two
# because a refresh token Google has expired is not a refresh token that was
# never there. The first is what the CLI says when the Library's token cache
# holds nothing or will not open. The second is the token endpoint's own
# refusal on the way up, and it is the one a run weeks after the last one gets:
# the cache still holds a refresh token, so nothing notices until Google is
# asked to spend it — and the consent screen a desktop client starts out on is
# in Testing, where Google expires a refresh token after seven days.
readonly NO_GRANT='no usable grant on Google Drive|Storage rejected the credentials'

fail() {
  echo "$*" >&2
  exit 1
}

# The skip comes before everything, including the build, so that a caller
# checking that an unconfigured run is a no-op waits on nothing.
if [ -z "${COFFRET_DRIVE_FOLDER_ID:-}" ]; then
  echo "skipping the Drive round trip: COFFRET_DRIVE_FOLDER_ID is not set."
  echo "Set it to the id of a folder on Drive to keep the Library's own folder"
  echo "in: open that folder in Drive and take the last part of its address, the"
  echo "part after /folders/. Set COFFRET_DRIVE_CLIENT_ID to the OAuth desktop"
  echo "client to authorize as."
  exit 0
fi

mkdir -p "$WORK" "$STATE_DIR" "$LOG_DIR" "$UPLOADER_ROOT" "$JOINER_ROOT"

# The Libraries and the log files both go under this directory rather than
# under the state directory of whoever started the run: a target that keeps
# state has to keep it somewhere it says, and this is where it says.
export COFFRET_STATE_DIR="$STATE_DIR"
export COFFRET_LOG_DIR="$LOG_DIR"

# Whether a Library is on this device is the settings file and not the
# directory: an interrupted creation leaves a directory that nothing opens.
library_present() {
  [ -f "$STATE_DIR/libraries/$1/settings.json" ]
}

# One string out of a Library's settings file.
#
# Read back rather than remembered, so that the report says where the Library
# is on every run and not only on the one that created it.
settings_value() {
  local file="$STATE_DIR/libraries/$1/settings.json"
  grep -o "\"$2\"[[:space:]]*:[[:space:]]*\"[^\"]*\"" "$file" |
    sed -n '1s/.*"\([^"]*\)"$/\1/p'
}

# Which Library a command was for, out of what it was told.
#
# For the renewal command below, which has to name one: `--library` on the
# commands that act on a Library this device has, `--name` on the two that put
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
  printf '%s\n' "$UPLOADER"
}

# Runs the CLI, showing what it says as it happens and keeping a copy of it.
#
# Both streams are merged, in the order they happened: the consent URL goes to
# standard error and the summary to standard output, and a transcript that had
# to be read in two halves would be a worse account of the run than the
# terminal gave. What this script reads back are lines distinct enough that
# nothing needs the two apart.
#
# Live rather than captured and printed afterwards, because one of these runs
# blocks on a person answering a consent screen the CLI is in the middle of
# printing the URL for.
#
# The status answered is the CLI's own — 0, 1, or 2 — rather than the
# pipeline's, so that a run which left findings is told apart from one that
# failed.
run_cli() {
  local status
  set +e
  printf '%s\n' "$PASSPHRASE" | "$COFFRET" "$@" 2>&1 | tee "$LAST"
  status=${PIPESTATUS[1]}
  set -e
  cat "$LAST" >>"$TRANSCRIPT"

  # An expired grant is the one failure this script can say something useful
  # about, and what it says is the command that renews it — spelled out here
  # rather than quoted from the CLI. Only one of the two failures above carries
  # a command at all, and the form it carries is the one a person with coffret
  # installed would run: `coffret` is not on this PATH, and the Libraries this
  # target keeps are under its own state directory rather than the one the CLI
  # looks in when nothing says otherwise. So the line the CLI gave is shown for
  # what it says went wrong, and the runnable form is given under it.
  if grep -Eq "$NO_GRANT" "$LAST"; then
    local library
    library="$(library_of "$@")"
    echo >&2
    grep -E "$NO_GRANT" "$LAST" >&2
    cat >&2 <<EOF

The grant on $library is gone. Renewing it means opening a browser at somebody,
which is a question this script was never asked, so it stops here instead. Run:

  printf '%s\n' '$PASSPHRASE' |
    COFFRET_STATE_DIR='$STATE_DIR' '$COFFRET' authorize --library $library --passphrase-stdin

EOF
    fail "It prints a URL of its own: open it, answer there, then run this target again."
  fi

  return "$status"
}

# The first line of a run's output that matches, which is the summary line.
said() {
  grep -m 1 "$1" "$LAST" || fail "$2"
}

# The Recovery Code the last command printed, or nothing if it printed none. It
# is the line on standard output; everything around it is on standard error
# (spec: KD-11).
said_recovery_code() {
  sed -n '/^coffret1/{s/[[:space:]]*$//p;q;}' "$LAST"
}

# What this run adds, named after the moment it was made so that no two runs
# write to the same Entry Path.
#
# A second is not fine enough on its own: two runs one after the other land in
# the same one, and the second of them would generate the same files into the
# same folder and find it had added nothing to the Library. So a name already
# taken is counted past rather than reused.
run_folder_name() {
  local stamp name attempt=1
  stamp="$(date -u +%Y%m%dT%H%M%SZ)"
  name="$stamp"
  while [ -e "$UPLOADER_ROOT/$name" ]; do
    attempt=$((attempt + 1))
    name="$stamp-$attempt"
  done
  printf '%s\n' "$name"
}

readonly RUN="$(run_folder_name)"
readonly RUN_FOLDER="$UPLOADER_ROOT/$RUN"

echo "=== coffret round trip against Google Drive, run $RUN ==="
echo
echo "Libraries and logs:  $WORK"
echo "Parent folder:       $COFFRET_DRIVE_FOLDER_ID"
echo "Passphrase:          a fixed test string; it protects generated JPEGs and nothing else"
echo

# What the run is going to ask of whoever started it, said before the build
# rather than when the first URL appears: a run that asks for nothing can be
# walked away from, and a run that asks is one to stay at the terminal for.
consents=0
library_present "$UPLOADER" || consents=$((consents + 1))
library_present "$JOINER" || consents=$((consents + 1))

if [ "$consents" -gt 0 ]; then
  if [ -z "${COFFRET_DRIVE_CLIENT_ID:-}" ]; then
    fail "COFFRET_DRIVE_CLIENT_ID is not set, and putting a Library on this device needs an OAuth desktop client to authorize as."
  fi
  echo "Consents to answer: $consents. Each one prints a URL for you to open —"
  echo "nothing opens a browser for you — and waits there, giving up after five"
  echo "minutes. The build comes first; the rest is the CLI's own output, and"
  echo "the whole run takes a few minutes."
else
  echo "$UPLOADER and $JOINER are already on this device: no consent to answer,"
  echo "and nothing in this run waits on you."
fi
echo

# 1. The binaries. The fixture generator is built alongside the CLI rather than
#    run through cargo later, so that the one build is the whole build.
echo "--- building the CLI and the fixture generator ---"
cd "$ROOT/backend"
cargo build --release -p coffret-cli -p coffret-fixtures
readonly COFFRET="$ROOT/backend/target/release/coffret"
readonly FIXTURES="$ROOT/backend/target/release/coffret-fixtures"

# 2. Two devices, on the first run only.
#
#    The two halves are asked separately rather than as one first run, so that
#    a consent nobody answered in time costs the run it was in and not the
#    target: what the next run does is whichever half is still missing.
#
#    The Recovery Code gets no file of its own: it is the only copy of the
#    Master Key that exists off the device, and a file kept for it beside the
#    Libraries it opens would be a key kept next to its lock. It is in the
#    transcript, because the transcript is what the CLI printed and what this
#    script reads the code back out of — one more reason nothing you would keep
#    belongs in the Libraries under `.tmp/drive-round-trip/`.
if ! library_present "$UPLOADER"; then
  echo
  echo "--- creating the Library as $UPLOADER ---"
  echo "The first of two consents: a URL is about to be printed, and the run"
  echo "waits at it until you have opened it in a browser and answered. Nothing"
  echo "on the account is read or changed beyond the folder coffret creates —"
  echo "a new one, so where the Libraries were removed from this device by hand,"
  echo "the folder the run before them made stays on the account untouched."
  echo
  run_cli init \
    --name "$UPLOADER" \
    --drive \
    --parent "$COFFRET_DRIVE_FOLDER_ID" \
    --passphrase-stdin ||
    fail "
$UPLOADER was not created; the lines above say what stopped it, and a consent
nobody answered is one of the things they can say. Nothing was kept on this
device, so running this target again makes the same first run over — but where
those lines name a folder on Drive, or say to look for one, that folder was
created before the failure and is the account's to remove."

  recovery_code="$(said_recovery_code)"
  [ -n "$recovery_code" ] || fail "init printed no Recovery Code."

  # The CLI's warning is the right one for a Library somebody keeps, and the
  # wrong one to act on here, so the run says which of the two this is.
  echo
  echo "That warning is the CLI's own. Nothing here needs writing down:"
  echo "$JOINER is joined with the code in a moment, and any later run reads it"
  echo "back out of $UPLOADER with the fixed Passphrase above."

  # The app folder as `init` said it, which is what a person joining from a
  # second device has to go on (spec: FM-18).
  app_folder_id="$(sed -n 's/^On Storage: the Google Drive folder //p' "$LAST" | head -n 1)"
  [ -n "$app_folder_id" ] || fail "init did not say which Drive folder the Library is in."
fi

if ! library_present "$JOINER"; then
  # Where a run before this one created the Library and then lost the joining
  # half, the code is asked of the Library that has it rather than made again:
  # a second `init` would be a second Library and a second folder on the
  # account.
  if [ -z "${recovery_code:-}" ]; then
    echo
    echo "--- reading $UPLOADER's Recovery Code back, to join with ---"
    run_cli recovery-code --library "$UPLOADER" --passphrase-stdin
    recovery_code="$(said_recovery_code)"
    [ -n "$recovery_code" ] || fail "$UPLOADER printed no Recovery Code."
    app_folder_id="$(settings_value "$UPLOADER" folder_id)"
    [ -n "$app_folder_id" ] || fail "$UPLOADER's settings name no Drive folder."
  fi

  echo
  echo "--- taking the same Library up as $JOINER ---"
  echo "The second consent. It is a second one because a grant belongs to a"
  echo "Library on this device rather than to the account, so the Library this"
  echo "device is joining has none of its own yet. Answer it as the account the"
  echo "Library was created under: the folder it is in is that account's."
  echo
  run_cli join \
    --name "$JOINER" \
    --recovery-code "$recovery_code" \
    --drive \
    --folder-id "$app_folder_id" \
    --passphrase-stdin ||
    fail "
$JOINER did not join. Running this target again asks $UPLOADER for the Recovery
Code and puts the second consent again; it creates no second Library and no
second folder on the account."
fi

unset recovery_code

# 3. What this run adds.
echo
echo "--- generating this run's files under $PREFIX/$RUN ---"
"$FIXTURES" --out "$RUN_FOLDER" --photos "$PHOTOS" --pages "$PAGES"
generated="$(find "$RUN_FOLDER" -type f | wc -l | tr -d ' ')"
[ "$generated" -gt 0 ] || fail "the fixture generator wrote nothing to $RUN_FOLDER."
echo "$generated files."

# 4. Into the Library. The mapping is recorded again on every run; the CLI says
#    what the prefix stood for before, and saying it again about the same
#    folder is the answer that nothing moved.
echo
echo "--- carrying $PREFIX into the Library from $UPLOADER ---"
run_cli map --library "$UPLOADER" --prefix "$PREFIX" "$UPLOADER_ROOT"

# The upload is the longest quiet stretch of the run, and a terminal that has
# gone quiet is worth saying something about before it does.
echo "uploading $generated files to Drive; nothing is printed until it is done."
status=0
run_cli sync --library "$UPLOADER" --passphrase-stdin || status=$?

# A run after the first one inherits the findings the deletion step below
# leaves: the Entry stays in the Library, the file stays gone from this disk,
# and the sync says so again every time until somebody acts on it. So the
# status this step holds to is "nothing new was surfaced", which is what the
# path check says — and the earlier runs' findings are reported rather than
# swallowed.
case "$status" in
  0) ;;
  "$FINDINGS")
    echo
    echo "note: the findings above are the deletions earlier runs made; this"
    echo "      run's own files are all accounted for."
    ;;
  *) fail "sync on $UPLOADER failed with status $status." ;;
esac
if grep -q "^surfaced $PREFIX/$RUN/" "$LAST"; then
  fail "sync surfaced a file this run had just written."
fi

summary="$(said '^added ' "sync printed no summary.")"
case "$summary" in
  *"committed head "*) ;;
  *) fail "sync committed nothing, and this run had $generated new files: $summary" ;;
esac
committed_head="${summary##*committed head }"
echo
echo "committed head $committed_head."

# 5. And out of it again, on the device that has only ever had the Recovery
#    Code.
echo
echo "--- fetching $PREFIX into $JOINER ---"
run_cli map --library "$JOINER" --prefix "$PREFIX" "$JOINER_ROOT"

# Counted before the fetch, because what the folder already held is what says
# which of the two answers below is the right one.
held_before="$(find "$JOINER_ROOT" -type f | wc -l | tr -d ' ')"

# Quiet in the same way the sync is, and on the run after a join it is the whole
# Library coming down rather than one batch.
echo "downloading into $JOINER; nothing is printed until it is done."
status=0
run_cli fetch --library "$JOINER" --under "$PREFIX" --passphrase-stdin || status=$?
[ "$status" = 0 ] || fail "fetch on $JOINER failed with status $status."

summary="$(said '^fetched ' "fetch printed no summary.")"
fetched="${summary#fetched }"
fetched="${fetched%%,*}"
if [ "$held_before" = 0 ]; then
  # The joiner's first fetch, which is the run after a join: it fills an empty
  # folder with the whole Library, so this run's files are some of what it
  # places rather than all of it.
  [ "$fetched" -ge "$generated" ] ||
    fail "a first fetch placed $fetched files and this run alone wrote $generated: $summary"
else
  # Every later one is incremental. What earlier runs put in is already on this
  # disk and is skipped rather than fetched, so what comes back is this run's
  # files and nothing besides.
  [ "$fetched" = "$generated" ] ||
    fail "fetch placed $fetched files and this run wrote $generated: $summary"
fi

if ! diff -r "$RUN_FOLDER" "$JOINER_ROOT/$RUN"; then
  fail "what came back out of the Library is not what went in."
fi
echo "$generated files, byte for byte what went in."

# 6. A file gone from the folder it was synced from. Propagating a deletion is
#    a flow of its own, so the run reports it and leaves the Library exactly as
#    it is — and says so with a status a script notices without reading a line.
#
#    Nothing is restored afterwards: the Entry stays in the Library, which is
#    the outcome being checked, and a script that quietly put the file back
#    would be hiding the state the next run has to cope with.
echo
echo "--- a file deleted from $UPLOADER, which the next sync surfaces ---"
# The first by name rather than whichever the directory happened to hand back
# first, so that two runs told apart by their output differ in what they did
# and not in which file they picked.
victim="$(find "$RUN_FOLDER" -type f | sort)"
victim="${victim%%$'\n'*}"
relative="${victim#"$UPLOADER_ROOT/"}"
rm "$victim"
echo "removed $PREFIX/$relative from this device."
echo

status=0
run_cli sync --library "$UPLOADER" --passphrase-stdin || status=$?
[ "$status" = "$FINDINGS" ] ||
  fail "a sync that left something to act on must exit $FINDINGS, and this one exited $status."
grep -qF "surfaced $PREFIX/$relative: this device had it and it is gone from disk" "$LAST" ||
  fail "the sync did not surface $PREFIX/$relative."

# 7. What the run did, in one block, so that nobody has to read back up.
library_id="$(settings_value "$UPLOADER" library_id)"
folder_id="$(settings_value "$UPLOADER" folder_id)"

echo
echo "=== the round trip held ==="
echo
echo "App folder:      coffret-$library_id"
echo "  its id:        $folder_id"
echo "  inside:        $COFFRET_DRIVE_FOLDER_ID"
echo "  to look at it: https://drive.google.com/drive/folders/$folder_id"
echo "Head committed:  $committed_head"
echo "Round-tripped:   $generated files, as $PREFIX/$RUN"
echo "Surfaced:        $PREFIX/$relative, deleted here and kept in the Library"
echo
echo "Transcript:      $TRANSCRIPT"
echo "CLI logs:        $LOG_DIR"
echo "Libraries:       $STATE_DIR/libraries"
echo
echo "Run this again to add another batch without a consent. Nothing on the"
echo "account is removed by it: the app folder above is the only one coffret"
echo "made, and every run reuses it."
