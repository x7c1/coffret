#!/usr/bin/env bash
#
# What the manual Drive targets have left on the account, and the one command
# that takes it back to nothing.
#
# `drive-round-trip-it` and `drive-index-layout-it` each keep a Library, and a
# Library's objects live in a `coffret-<library id>` folder created by the
# first run and reused by every run after it — a grant belongs to a Library on
# a device, so a new Library would mean a new consent. Nothing trashes those
# folders. Their names carry a Library ID and nothing else: not when they were
# made, and not which target made them. So an account these targets have run
# against for a while holds folders whose only difference is whether a Library
# on this device still points at one, and a run that failed inside `init` after
# Drive had already minted the folder — which by then means a failure writing
# the Library's own files, since the folder is minted only once the consent has
# been answered — leaves one that nothing here points at at all.
#
# Three modes, and the first of them changes nothing:
#
#   list            every `coffret-` folder under COFFRET_DRIVE_FOLDER_ID,
#                   oldest first, and beside each one the scenario Library that
#                   points at it, or `stale` where none does. This is the
#                   answer to "which of these is old".
#   trash <id>...   the same listing, then only the folders named into Drive's
#                   trash. Nothing under `.tmp/` is removed: the Libraries that
#                   were already live keep their state and their grants. An id
#                   the listing does not hold, and an id it marks as in use,
#                   are both refused before anything is trashed — this is the
#                   answer to "that one is stale, now take it away".
#   reset           the same listing, then every folder in it into Drive's
#                   trash, then the two scenario directories under `.tmp/`
#                   removed — so the next run of either target starts from a
#                   Library of its own, and asks its consents again. This is
#                   how a live Library is given up, and the only way.
#
# Only folders under COFFRET_DRIVE_FOLDER_ID are ever listed or trashed: the
# query names that parent, and nothing walks out of it. `trash` refuses an id
# the listing does not hold for that same reason — an id typed by hand is the
# one way a folder under another parent could have been reached. A parent
# shared with a Library somebody keeps is therefore the one mistake this cannot
# make on its own — point the targets at a folder of their own.
#
# Trashed rather than purged, because a folder that turns out to have mattered
# is recoverable from Drive's trash for a while. Emptying it is the account
# owner's.
#
# The tool needs a grant of its own, kept under `.tmp/drive-admin/` and sealed
# under the Master Key fixed below. A reset never removes that directory, so
# the grant outlives the Libraries it is used to clear and the one interactive
# step here — the consent, run through the `authorize` example when the cache
# is missing — is answered once per machine, and again only when Google has
# expired the grant: a consent screen still in Testing loses its refresh token
# after seven days, and what asks for a new one is removing the cache file and
# running this again, which is what a run that cannot reach Drive says.
#
# What it needs:
#
#   COFFRET_DRIVE_FOLDER_ID      the folder the targets create their Libraries'
#                                app folders in, by the id in its address.
#                                Without it this does nothing, as the targets
#                                themselves do
#   COFFRET_DRIVE_CLIENT_ID      the OAuth desktop client to authorize as, and
#                                it has to be the client the targets were
#                                authorized under: a `drive.file` grant reaches
#                                what that client created, so a grant of
#                                another client's would see none of these
#                                folders
#   COFFRET_DRIVE_CLIENT_SECRET  for a client registered with one; same

set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# The two targets' state, which a reset removes: a Library with no folder on
# Drive is a Library that cannot be opened, so the two go together.
readonly ROUND_TRIP="$ROOT/.tmp/drive-round-trip"
readonly INDEX_LAYOUT="$ROOT/.tmp/drive-index-layout"

# This tool's own corner, which a reset keeps. The grant here is what makes the
# reset after this one cost no consent.
readonly WORK="$ROOT/.tmp/drive-admin"
readonly TOKEN_CACHE="$WORK/token-cache.cftc"
readonly LOG_DIR="$WORK/logs"
# What the listing said, for this script to read back and to print.
readonly LISTING="$WORK/last-listing.tsv"

# The Master Key the tool's token cache is sealed under, base64 of 32 bytes.
#
# Fixed and in the clear on purpose, like the Passphrases the two targets are
# run with: what it protects is a test grant on a test folder — a `drive.file`
# grant that reaches what coffret itself created as that OAuth client, and
# nothing else in the account — and a reset that stopped to ask a person for a
# key would not be a reset anybody could run. Nothing you would keep belongs
# under `.tmp/drive-admin/` for exactly that reason.
readonly MASTER_KEY="Y29mZnJldCBkcml2ZS1pdC1yZXNldCB0ZXN0IGtleSE="

MODE="${1:-list}"
readonly MODE
shift || true
# What `trash` was told to take away, and empty in the other two modes.
WANTED=("$@")
readonly WANTED

fail() {
  echo "$*" >&2
  exit 1
}

# The skip comes before everything, including the build, so that a caller
# checking that an unconfigured run is a no-op waits on nothing.
if [ -z "${COFFRET_DRIVE_FOLDER_ID:-}" ]; then
  echo "skipping: COFFRET_DRIVE_FOLDER_ID is not set, and it is what says which"
  echo "folder the Libraries were created in. Set it to the id in that folder's"
  echo "address, the part after /folders/ — the same value the drive-*-it"
  echo "targets are run with."
  exit 0
fi

case "$MODE" in
  list | reset)
    [ "${#WANTED[@]}" = 0 ] ||
      fail "$MODE takes no folder ids, and was given: ${WANTED[*]}. Naming folders is what trash is for: \`make drive-it-trash IDS=\"${WANTED[*]}\"\`."
    ;;
  trash)
    [ "${#WANTED[@]}" -gt 0 ] ||
      fail "trash takes the ids of the folders to trash, and was given none: \`make drive-it-trash IDS=\"<id> [<id>...]\"\`, with IDS naming folders \`make drive-it-list\` printed as stale."
    ;;
  *) fail "this script takes list, trash or reset, and was given: $MODE" ;;
esac

# Wanted on every run and not only on the first one: an access token is minted
# against the client the grant was given to, so a cache that is already here is
# still opened with these credentials. Asked for before the build, for the
# reason the skip above comes before it.
[ -n "${COFFRET_DRIVE_CLIENT_ID:-}" ] ||
  fail "COFFRET_DRIVE_CLIENT_ID is not set, and this tool both authorizes and refreshes against it: name the OAuth desktop client the targets were authorized under."

mkdir -p "$WORK" "$LOG_DIR"

# The tool's grant and its log go under this directory rather than under the
# state directory of whoever started the run: what is kept has to be kept
# somewhere the script says, and this is where it says.
export COFFRET_DRIVE_TOKEN_CACHE="$TOKEN_CACHE"
export COFFRET_MASTER_KEY="$MASTER_KEY"
export COFFRET_LOG_DIR="$LOG_DIR"
export COFFRET_LOG=info

# One string out of a Library's settings file, by file rather than by name:
# the two targets keep their Libraries in directories of their own, and this
# reads both.
settings_value() {
  grep -o "\"$2\"[[:space:]]*:[[:space:]]*\"[^\"]*\"" "$1" |
    sed -n '1s/.*"\([^"]*\)"$/\1/p'
}

# Which scenario Library on this device points at a folder on Drive, if any.
#
# The folder id is read back out of every `settings.json` the two targets keep
# rather than remembered anywhere: a Library's settings are the only place that
# pairing is written down, and a folder no settings name is a folder no run
# will ever open again.
scenario_of() {
  local folder="$1" work settings library found=""
  for work in "$ROUND_TRIP" "$INDEX_LAYOUT"; do
    for settings in "$work"/state/libraries/*/settings.json; do
      [ -f "$settings" ] || continue
      [ "$(settings_value "$settings" folder_id)" = "$folder" ] || continue
      library="$(basename "$(dirname "$settings")")"
      found="${found:+$found, }$(basename "$work")/$library"
    done
  done
  printf '%s\n' "${found:-stale}"
}

echo "=== coffret app folders on Google Drive ==="
echo
echo "Parent folder:  $COFFRET_DRIVE_FOLDER_ID"
echo "Mode:           $MODE"
echo "This tool:      $WORK (kept by a reset)"
echo

if [ "$MODE" = reset ]; then
  cat <<EOF
Resetting. Every folder listed below goes into Drive's trash — recoverable
there for a while, and nothing outside the parent above is touched — and
$ROUND_TRIP
and
$INDEX_LAYOUT
are removed with them. The next run of drive-round-trip-it or
drive-index-layout-it then creates a Library of its own and asks its consents
again: one URL per Library, to be answered at a browser.

Nothing is trashed before that listing has been printed. If looking is all
you wanted, stop here: \`make drive-it-list\` prints the same listing and
changes nothing. If one stale folder is all that is in the way,
\`make drive-it-trash IDS=<id>\` takes that one and leaves the live Libraries,
their folders and their grants where they are.

EOF
fi

if [ "$MODE" = trash ]; then
  cat <<EOF
Trashing the ${#WANTED[@]} folder(s) named here, and nothing else:

  ${WANTED[*]}

They go into Drive's trash — recoverable there for a while — once the listing
below has been printed. Nothing under .tmp/ is removed: that state belongs to
the Libraries still pointing at the folders left alone, and removing it would
cost them their grants. An id the listing does not hold, or one it marks as in
use, stops the run with nothing trashed.

EOF
fi

# The tool's own grant, which is not either target's: those belong to a Library
# and are sealed under its stored Master Key, and this one belongs to the
# machine.
if [ ! -f "$TOKEN_CACHE" ]; then
  echo "--- authorizing this tool, once on this machine ---"
  echo "It prints a URL and waits there; nothing opens a browser for you. The"
  echo "grant is cached under $WORK and every run after this one uses it."
  echo
  cd "$ROOT/backend"
  cargo run --release -p google-drive-store --example authorize ||
    fail "the authorization did not finish, so there is no grant to list with; the lines above say why."
  echo
fi

echo "--- building the folder tool ---"
cd "$ROOT/backend"
cargo build --release -p google-drive-store --example app_folders
readonly APP_FOLDERS="$ROOT/backend/target/release/examples/app_folders"

"$APP_FOLDERS" list "$COFFRET_DRIVE_FOLDER_ID" >"$LISTING" ||
  fail "the folders under $COFFRET_DRIVE_FOLDER_ID could not be listed; the lines above say why. A grant this tool no longer has is the ordinary one — Google expires the refresh token of a consent screen in Testing after seven days — and removing $TOKEN_CACHE makes the next run ask for the consent again."

mapfile -t folders <"$LISTING"

# The folders and the Libraries are paired here rather than by the tool: which
# Library a folder belongs to is this device's business, and the tool's is the
# account's.
ids=()
# The `IN USE BY` column, kept beside the ids rather than asked for twice: what
# `trash` refuses is decided on the same answer that was printed.
in_use_by=()
echo
if [ "${#folders[@]}" = 0 ]; then
  echo "No coffret- folder under this parent: the account holds nothing either"
  echo "target left, so there is nothing to trash."
else
  printf '%-34s %-26s %-26s %s\n' "FOLDER ID" "NAME" "CREATED" "IN USE BY"
  for line in "${folders[@]}"; do
    IFS=$'\t' read -r id name created <<<"$line"
    scenario="$(scenario_of "$id")"
    ids+=("$id")
    in_use_by+=("$scenario")
    printf '%-34s %-26s %-26s %s\n' "$id" "$name" "$created" "$scenario"
  done
fi

if [ "$MODE" = list ]; then
  echo
  echo "Nothing was changed. A folder marked stale is one no Library here opens:"
  echo "\`make drive-it-trash IDS=<id>\` trashes that one and leaves the rest of"
  echo "the listing and all of the state under .tmp/ alone. To start over"
  echo "instead, \`make drive-it-reset\` trashes every folder above and clears"
  echo "both targets' state, so the run after it opens a fresh Library and asks"
  echo "its consents again."
  exit 0
fi

if [ "$MODE" = trash ]; then
  echo
  refusals=()
  for wanted in "${WANTED[@]}"; do
    listed=""
    for i in "${!ids[@]}"; do
      [ "${ids[$i]}" = "$wanted" ] || continue
      listed="yes"
      [ "${in_use_by[$i]}" = stale ] ||
        refusals+=("$wanted is in use by ${in_use_by[$i]}. Giving up a live Library is what \`make drive-it-reset\` is for, and it gives up every Library at once: it trashes every folder above and clears both targets' state, so the next run of either target starts from a fresh Library and asks its consents again.")
      break
    done
    [ -n "$listed" ] ||
      refusals+=("$wanted is not in the listing above, so this refuses it: the id is mistyped, or that folder is already in the trash, or it is under another parent — and a folder outside $COFFRET_DRIVE_FOLDER_ID is one this must never touch.")
  done

  if [ "${#refusals[@]}" -gt 0 ]; then
    echo "--- refusing to trash anything ---" >&2
    for refusal in "${refusals[@]}"; do
      echo "$refusal" >&2
    done
    fail "nothing was trashed, and nothing under .tmp/ was removed: every id has to be a stale folder of this listing before any of them is taken away."
  fi

  echo "--- trashing ${#WANTED[@]} folder(s) ---"
  "$APP_FOLDERS" trash "${WANTED[@]}" ||
    fail "not every folder was trashed; the lines above say which and why, and the state under .tmp/ was left alone, so this can be run again on what is still listed."

  echo
  echo "=== trashed ==="
  echo
  echo "Folders trashed:  ${#WANTED[@]}"
  echo "Kept:             every other folder above, and all state under .tmp/,"
  echo "                  so the Libraries still in use open with no new consent"
  echo
  echo "\`make drive-it-list\` prints what is left."
  exit 0
fi

if [ "${#ids[@]}" -gt 0 ]; then
  echo
  echo "--- trashing ${#ids[@]} folder(s) ---"
  "$APP_FOLDERS" trash "${ids[@]}" ||
    fail "not every folder was trashed; the lines above say which and why, and the state under .tmp/ was left alone so this can be run again."
fi

echo
echo "--- removing the two targets' state ---"
rm -rf "$ROUND_TRIP" "$INDEX_LAYOUT"
echo "$ROUND_TRIP"
echo "$INDEX_LAYOUT"

echo
echo "=== reset ==="
echo
echo "Folders trashed:  ${#ids[@]}"
echo "Kept:             $WORK, so this tool needs no consent again"
echo
echo "The next \`make drive-round-trip-it\` creates its two Libraries from"
echo "nothing and asks for two consents; \`make drive-index-layout-it\` asks for"
echo "one. Stay at the terminal for them."
