#!/usr/bin/env bash
#
# The explorer's journeys, end to end, against real processes on real Storage.
#
# Everything the explorer does is proven by router tests over in-memory
# adapters, and then by a person at a browser walking the same journeys by hand.
# The layer between the two is the one nothing reaches: a real `coffret-server`
# process, a real Index on disk, real Storage, and a `coffret sync` running
# beside the server. This is that layer, in two stages.
#
#   1. The API stage, here. Two devices are created out of two state
#      directories, one of them by joining from a Recovery Code; the server is
#      started over the joined one — which is where that device's catalog first
#      learns what the Library holds, since nothing else has told it; and the
#      routes are asked what a person would have clicked to find out.
#
#   2. The browser stage, in `frontend/packages/apps/e2e`. A real Chromium walks
#      seven journeys through the built explorer and photographs each checkpoint.
#
# Nothing is left behind but the pictures. MinIO is started here and removed on
# the way out, `.tmp/e2e/` is made again on every run, and no state carries from
# one run to the next — which is the opposite of `drive-round-trip-it.sh`, and
# deliberately: what that one would be throwing away is a grant on somebody's
# real account, and what this one would be throwing away is a container.

set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Everything this run writes, and nothing outside it. Remade per run: the
# journeys are written against a Library that has just been created, and a
# second run finding the first one's fetched files would be running different
# journeys.
readonly WORK="$ROOT/.tmp/e2e"

# The one thing a run leaves on purpose. The pictures are the deliverable: what
# a machine cannot judge — whether the filer reads right, whether a chip is
# legible, whether the outage notice says something to act on — a person judges
# from these.
readonly SCREENSHOTS="$WORK/screenshots"

readonly LOG_DIR="$WORK/logs"
readonly TRANSCRIPT="$WORK/transcript.log"
readonly LAST="$WORK/last-command.log"
readonly ARTIFACTS="$WORK/playwright"

# Overridable for a machine where the name or one of the ports is taken — by
# `s3-store-it`, which keeps its own container on 19000, or by anything else.
# Not so that two of these can run at once: everything a run has besides the
# container is under one `.tmp/e2e/`, and a run starts by deleting it, so the
# second one to start takes the first one's Libraries out from under it.
CONTAINER="${COFFRET_E2E_MINIO_CONTAINER:-coffret-minio-e2e}"
MINIO_PORT="${COFFRET_E2E_MINIO_PORT:-19010}"
IMAGE="${COFFRET_E2E_MINIO_IMAGE:-quay.io/minio/minio:latest}"

# Fixed rather than asked of the operating system, and that is not laziness: the
# explorer is served by `vite preview`, which is aimed at the server once when
# it starts, and the outage journey kills the server and starts it again. A
# server that came back on a port the operating system chose would be one the
# page could no longer reach.
SERVER_PORT="${COFFRET_E2E_SERVER_PORT:-18787}"
WEB_PORT="${COFFRET_E2E_WEB_PORT:-14173}"

# MinIO refuses a root password shorter than eight characters. These credentials
# only ever reach a container this script starts and stops.
readonly ACCESS_KEY="coffret-e2e"
readonly SECRET_KEY="coffret-e2e-secret"
readonly BUCKET="coffret-e2e"

# The Passphrase both Libraries are created under and opened with.
#
# Fixed and in the clear on purpose: it protects a Library of generated JPEGs
# that is thrown away at the end of the run, and a target nobody can run
# unattended is not one CI could have. Nothing you would keep belongs under
# `.tmp/e2e/` for exactly that reason.
readonly PASSPHRASE="a coffret journey against MinIO"

# How much the journeys walk through. Still small — this is a journey and not a
# benchmark, and every file here is encrypted, uploaded, fetched back and drawn
# in a browser — but the album is one whole album rather than a handful, and
# that is the backfill journey's doing.
#
# What that journey is about is the folder coming over behind the reader, and
# the explorer says so by polling the server about once a second. A dozen
# photographs come over a loopback MinIO in a tenth of that, so the line saying
# it is happening is never drawn at all: the run passes, and the picture a
# person is asked to judge the progress by has no progress in it. A hundred
# takes long enough to be seen, and costs a few seconds and a dozen megabytes.
#
# A hundred is also the ceiling: the generator lays photos out in albums of a
# hundred, and the journeys walk one album.
PHOTOS="${COFFRET_E2E_PHOTOS:-100}"
PAGES="${COFFRET_E2E_PAGES:-4}"

# How many pages the two imported books have. Small on purpose: what the freeze
# journey is about is the shape of what the Library ends up holding — Packs
# rather than one Container per page — and three pages state that as well as
# three hundred while costing a few seconds rather than a few minutes.
IMPORT_PAGES="${COFFRET_E2E_IMPORT_PAGES:-3}"

# The two names the two devices know the one Library by, and the top-level part
# of it each maps.
readonly UPLOADER="main"
readonly JOINER="second"
readonly PREFIX="runs"

# The album and the book, as the fixture generator names them, and the folder
# that exists for this script rather than for the journeys: the API stage fetches
# an Entry and adds one, and both leave the folder they touch changed — so it
# touches one no journey looks at, and the album and the book reach the browser
# stage exactly as a device that has only just joined sees them.
readonly ALBUM="$PREFIX/album-000"
readonly BOOK="$PREFIX/book-000"
readonly CHECKED_NAME="checked"
readonly CHECKED="$PREFIX/$CHECKED_NAME"

# How long MinIO gets to answer its health check, and how long the API stage
# waits for a sync it armed to commit.
readonly STARTUP_TIMEOUT_SECONDS=60
readonly SYNC_TIMEOUT_SECONDS=120

fail() {
  echo "$*" >&2
  exit 1
}

for tool in docker curl jq pnpm cargo; do
  command -v "$tool" >/dev/null 2>&1 ||
    fail "$tool is needed to run the explorer's journeys and is not on this PATH."
done

[ "$PHOTOS" -le 100 ] ||
  fail "COFFRET_E2E_PHOTOS is $PHOTOS. The journeys walk one album and the fixture
generator lays photos out in albums of a hundred, so anything above that would put
the photographs the backfill journey waits for in an album it never opens."

# And a floor, so that a run cut down to be quick says so here rather than in a
# browser: the journeys name the first two photographs of the album and the last
# one, and the first three pages of the book, and a generator asked for fewer
# makes files nothing on the screen will ever be called.
[ "$PHOTOS" -ge 2 ] && [ "$PAGES" -ge 3 ] ||
  fail "COFFRET_E2E_PHOTOS is $PHOTOS and COFFRET_E2E_PAGES is $PAGES, and the
journeys are written against at least two photographs and three pages."

# The imported books have a floor of their own, and it is the freeze's rather
# than a journey's: what says a book was packed is that it comes back out of
# fewer Containers than it has pages, and one page can never say that.
[ "$IMPORT_PAGES" -ge 2 ] ||
  fail "COFFRET_E2E_IMPORT_PAGES is $IMPORT_PAGES. A book of one page cannot show
that a freeze packed it, since one Container for one page is what an ordinary sync
would leave behind."

server_pid=""

teardown() {
  if [ -n "$server_pid" ]; then
    kill "$server_pid" >/dev/null 2>&1 || true
    wait "$server_pid" >/dev/null 2>&1 || true
  fi
  docker rm --force "$CONTAINER" >/dev/null 2>&1 || true
}

# Also clears a container left behind by a run that was killed outright.
trap teardown EXIT
teardown

rm -rf "$WORK"
mkdir -p "$WORK" "$LOG_DIR" "$SCREENSHOTS"

readonly UPLOADER_STATE="$WORK/$UPLOADER/state"
readonly JOINER_STATE="$WORK/$JOINER/state"
readonly UPLOADER_ROOT="$WORK/$UPLOADER/$PREFIX"
readonly JOINER_ROOT="$WORK/$JOINER/$PREFIX"
readonly SPARE="$WORK/spare"
readonly DROP_FILE="$WORK/dropped/dropped.jpg"
# The pages of two books that are in neither device's folder: one the API stage
# drops onto a folder of its own, and one the browser stage drops onto a folder
# it makes in the explorer. Both go in as books — one request, and a freeze
# rather than a sync — so what the Library ends up holding is Packs.
readonly API_BOOK="$WORK/books/for-the-api"
readonly BROWSER_BOOK="$WORK/books/for-the-browser"
# What the *other* device commits while the browser stage is watching: one while
# this device's server is up, for the refresh journey, and one while it is
# stopped, for the journey that starts it again. They sit outside both mapped
# folders until a journey asks for them to be committed.
readonly REFRESH_FILE="$WORK/elsewhere/for-the-refresh.jpg"
readonly RESTART_FILE="$WORK/elsewhere/for-the-restart.jpg"
mkdir -p "$UPLOADER_STATE" "$JOINER_STATE" "$UPLOADER_ROOT" "$JOINER_ROOT" \
  "$SPARE" "$(dirname "$DROP_FILE")" "$(dirname "$REFRESH_FILE")" \
  "$API_BOOK" "$BROWSER_BOOK"

echo "=== the explorer's journeys, against MinIO ==="
echo
echo "Working directory:  $WORK"
echo "Screenshots:        $SCREENSHOTS"
echo "Logs:               $LOG_DIR"
echo "Passphrase:         a fixed test string; it protects generated JPEGs and nothing else"
echo

# ---------------------------------------------------------------------------
# What the run needs, built once and up front.
# ---------------------------------------------------------------------------

echo "--- building the binaries and the explorer ---"
cd "$ROOT/backend"
cargo build --release -p coffret-cli -p coffret-server -p coffret-fixtures
readonly COFFRET="$ROOT/backend/target/release/coffret"
readonly SERVER="$ROOT/backend/target/release/coffret-server"
readonly FIXTURES="$ROOT/backend/target/release/coffret-fixtures"

cd "$ROOT/frontend"
pnpm --filter @coffret/web build

# Idempotent: a browser already downloaded is left where it is. `--with-deps`
# only where the machine is disposable — it installs system packages, which is
# not something a target may do to somebody's own machine.
echo "--- making sure Chromium is here ---"
if [ -n "${CI:-}" ]; then
  pnpm --filter @coffret/e2e exec playwright install --with-deps chromium
else
  pnpm --filter @coffret/e2e exec playwright install chromium
fi

# ---------------------------------------------------------------------------
# MinIO.
# ---------------------------------------------------------------------------

echo
echo "--- starting MinIO in $CONTAINER on port $MINIO_PORT ---"
docker run --detach \
  --name "$CONTAINER" \
  --publish "127.0.0.1:${MINIO_PORT}:9000" \
  --env "MINIO_ROOT_USER=${ACCESS_KEY}" \
  --env "MINIO_ROOT_PASSWORD=${SECRET_KEY}" \
  "$IMAGE" server /data >/dev/null

for _ in $(seq "$STARTUP_TIMEOUT_SECONDS"); do
  if curl --fail --silent --show-error "http://127.0.0.1:${MINIO_PORT}/minio/health/live" >/dev/null 2>&1; then
    ready=1
    break
  fi
  sleep 1
done
[ "${ready:-}" = 1 ] || {
  docker logs "$CONTAINER" >&2 || true
  fail "MinIO did not become healthy within ${STARTUP_TIMEOUT_SECONDS}s"
}

# The bucket. `init` checks that one answers and never creates one — where a
# Library goes is a decision somebody made about their own account — so the
# bucket is made here, with the `mc` the MinIO image already carries.
docker exec \
  --env "MC_HOST_here=http://${ACCESS_KEY}:${SECRET_KEY}@127.0.0.1:9000" \
  "$CONTAINER" mc mb --ignore-existing "here/${BUCKET}" >/dev/null

# What the Libraries sign with. A Library's settings say where its bucket is and
# never how to sign for it, so opening one takes its credentials from the SDK's
# own resolution — which is the environment first, and this is that environment.
export AWS_ACCESS_KEY_ID="$ACCESS_KEY"
export AWS_SECRET_ACCESS_KEY="$SECRET_KEY"
export AWS_REGION="us-east-1"
readonly ENDPOINT="http://127.0.0.1:${MINIO_PORT}"

# ---------------------------------------------------------------------------
# Two devices.
# ---------------------------------------------------------------------------

# Runs the CLI as one of the two devices, showing what it says as it happens and
# keeping a copy for this script to read back out of.
#
# Both streams are merged in the order they happened: the summary goes to
# standard output and everything around it to standard error, and a transcript
# read in two halves would be a worse account of the run than the terminal gave.
# The status answered is the CLI's own — 0, 1, or 2 — rather than the pipeline's,
# so a run that left findings is told apart from one that failed.
run_cli() {
  local state="$1"
  shift
  local status
  set +e
  printf '%s\n' "$PASSPHRASE" |
    COFFRET_STATE_DIR="$state" COFFRET_LOG_DIR="$LOG_DIR" "$COFFRET" "$@" 2>&1 |
    tee "$LAST"
  status=${PIPESTATUS[1]}
  set -e
  cat "$LAST" >>"$TRANSCRIPT"
  return "$status"
}

echo
echo "--- creating the Library as $UPLOADER ---"
run_cli "$UPLOADER_STATE" init \
  --name "$UPLOADER" \
  --s3 \
  --bucket "$BUCKET" \
  --endpoint "$ENDPOINT" \
  --path-style \
  --passphrase-stdin ||
  fail "$UPLOADER was not created."

# The Library's own prefix, as `init` said it: it is exactly what a second
# device is given to join with (spec: FM-18).
library_prefix="$(sed -n "s|^On Storage: s3://${BUCKET}/||p" "$LAST" | head -n 1)"
[ -n "$library_prefix" ] || fail "init did not say where in the bucket the Library is."

recovery_code="$(sed -n '/^coffret1/{s/[[:space:]]*$//p;q;}' "$LAST")"
[ -n "$recovery_code" ] || fail "init printed no Recovery Code."

# `init` has just told the terminal, at length, to write that code down and keep
# it off this device. That is the right thing to say about a Library somebody
# means to keep, and it is on the screen of anybody who runs this target for the
# first time — about a Library that is deleted before the next run starts. Said
# here rather than left to be worked out, as `drive-round-trip-it.sh` says it.
echo
echo "That warning is the CLI's own. Nothing here needs writing down:"
echo "$JOINER joins with the code in a moment, and the Library it opens is"
echo "thrown away with the rest of .tmp/e2e/ when the next run starts."

echo
echo "--- generating what the journeys walk through ---"
"$FIXTURES" --out "$UPLOADER_ROOT" --photos "$PHOTOS" --pages "$PAGES"

# Five more photographs from one more run of the generator, none of them part of
# what the journeys walk through. One goes into the Library for the API stage to
# fetch back out of it and one is what the joined device carries in beside the
# running server; the other three stay off the Library, to be handed to the
# browser stage: the file it drags onto a folder, and the two the *other* device
# commits while this one is looking — one with its server up, for the refresh to
# find, and one with its server stopped, for the next start to find.
"$FIXTURES" --out "$SPARE" --photos 5 --pages 0
mkdir -p "$UPLOADER_ROOT/$CHECKED_NAME"
cp "$SPARE/album-000/img-00000.jpg" "$UPLOADER_ROOT/$CHECKED_NAME/served.jpg"
cp "$SPARE/album-000/img-00001.jpg" "$DROP_FILE"
cp "$SPARE/album-000/img-00003.jpg" "$REFRESH_FILE"
cp "$SPARE/album-000/img-00004.jpg" "$RESTART_FILE"

# And two more books, in neither device's folder: they are brought in the way a
# scanned book is — dropped whole onto a folder made for them, and packed rather
# than synced. One goes in through the routes and one through the explorer.
"$FIXTURES" --out "$WORK/books/generated" --photos 0 --pages "$IMPORT_PAGES"
cp "$WORK/books/generated/book-000/"*.jpg "$API_BOOK/"
cp "$WORK/books/generated/book-000/"*.jpg "$BROWSER_BOOK/"

generated="$(find "$UPLOADER_ROOT" -type f | wc -l | tr -d ' ')"
echo "$generated files under $PREFIX."

echo
echo "--- carrying $PREFIX into the Library from $UPLOADER ---"
run_cli "$UPLOADER_STATE" map --library "$UPLOADER" --prefix "$PREFIX" "$UPLOADER_ROOT"
run_cli "$UPLOADER_STATE" sync --library "$UPLOADER" --passphrase-stdin ||
  fail "sync on $UPLOADER failed."
grep -q "committed head " "$LAST" || fail "sync on $UPLOADER committed nothing."

echo
echo "--- taking the same Library up as $JOINER ---"
run_cli "$JOINER_STATE" join \
  --name "$JOINER" \
  --recovery-code "$recovery_code" \
  --s3 \
  --bucket "$BUCKET" \
  --prefix "$library_prefix" \
  --endpoint "$ENDPOINT" \
  --path-style \
  --passphrase-stdin ||
  fail "$JOINER did not join."
unset recovery_code

run_cli "$JOINER_STATE" map --library "$JOINER" --prefix "$PREFIX" "$JOINER_ROOT"

# And no sync here, deliberately. $JOINER's Index holds nothing at all at this
# point — joining reads a Recovery Code and writes a directory, and touches
# Storage for nothing but the folder's name — so every folder the two stages
# below walk was put in its catalog by the server catching up as it started
# (spec: CK-9). A `coffret sync` run here would do the same thing on its way to
# committing, and would hide the entry point this target exists to exercise.
#
# One file of its own is left in the folder it maps, for the sync that runs
# beside the running server further down: a sync with nothing to commit commits
# nothing, and a second device adding a photograph of its own is the ordinary
# thing a second device does.
mkdir -p "$JOINER_ROOT/$CHECKED_NAME"
cp "$SPARE/album-000/img-00002.jpg" "$JOINER_ROOT/$CHECKED_NAME/from-second.jpg"

# ---------------------------------------------------------------------------
# Stage 1: the routes, over the joined device.
# ---------------------------------------------------------------------------

readonly API="http://127.0.0.1:${SERVER_PORT}/api"

# The server answers nobody who cannot show the key it drew as it started, and
# it writes that key into the Library's own directory, readable by this account
# and no other. This stage reads it off the disk exactly as the explorer's proxy
# does.
readonly SERVER_KEY_FILE="$JOINER_STATE/libraries/$JOINER/server-key"

# The key of whichever server is running now. Read per call rather than once:
# every start draws a new one, and this stage starts more than one server.
server_key() {
  cat "$SERVER_KEY_FILE" 2>/dev/null
}

# One route's answer, or a failure that stops the run. `--fail` so that a
# refusal is not read as an answer: every route here is being asked a question
# this stage claims has one.
api() {
  curl --fail --silent --show-error \
    --header "x-coffret-key: $(server_key)" \
    "$API/$1"
}

# Whether anything at all is listening there, whatever it answers with.
#
# Deliberately not `api`: a coffret-server left over from an earlier run refuses
# a request carrying this run's key, and a refusal is still something holding
# the port.
something_answers() {
  curl --silent --output /dev/null --max-time 2 "$API/library"
}

# What one folder holds; the Library root is the empty string.
listing() {
  case "$1" in
    '') api "list" ;;
    *) api "list?path=$(printf '%s' "$1" | jq -sRr @uri)" ;;
  esac
}

start_server() {
  # Nothing may be answering there yet, and this asks before starting anything.
  # A `coffret-server` that outlived a run killed outright still holds this
  # port: the one started below would die on the bind, but the routes would go
  # on being answered — by a server holding open the Library of a run whose
  # state directory has since been deleted. The stage would then walk a Library
  # nobody has, and say so several assertions later in terms that make no sense.
  ! something_answers ||
    fail "something is already answering at $API. If it is a coffret-server left over
from a run that was killed, stop it; or set COFFRET_E2E_SERVER_PORT to a free port."

  COFFRET_STATE_DIR="$JOINER_STATE" COFFRET_LOG_DIR="$LOG_DIR" \
    "$SERVER" --library "$JOINER" --passphrase-stdin --port "$SERVER_PORT" \
    <<<"$PASSPHRASE" >>"$LOG_DIR/coffret-server.log" 2>&1 &
  server_pid=$!

  for _ in $(seq "$STARTUP_TIMEOUT_SECONDS"); do
    if api library >/dev/null 2>&1; then
      return 0
    fi
    kill -0 "$server_pid" 2>/dev/null ||
      fail "the server stopped before it answered; see $LOG_DIR/coffret-server.log"
    sleep 1
  done
  fail "the server did not answer at $API within ${STARTUP_TIMEOUT_SECONDS}s"
}

stop_server() {
  [ -n "$server_pid" ] || return 0
  kill "$server_pid" >/dev/null 2>&1 || true
  wait "$server_pid" >/dev/null 2>&1 || true
  server_pid=""
}

echo
echo "--- stage 1: the routes, over $JOINER, on port $SERVER_PORT ---"
start_server

# Which Library the browser would be looking at.
api library | jq --exit-status --arg name "$JOINER" \
  '.name == $name and .provider == "s3"' >/dev/null ||
  fail "/api/library did not answer for $JOINER: $(api library)"

# Every folder in it, flat — every path a separator implies. This is also the
# whole of the startup catch-up as a test: $JOINER has run no sync and no fetch,
# so its catalog held nothing at all a moment ago, and every folder named here
# reached it because the server replayed the Journal as it opened the Library.
api folders | jq --exit-status --arg album "$ALBUM" --arg book "$BOOK" \
  '(.folders | index($album)) != null and (.folders | index($book)) != null' >/dev/null ||
  fail "/api/folders did not name the generated folders, so the catch-up the server
runs as it starts did not reach $JOINER's catalog: $(api folders)"

# And the listing, all the way down. Every folder the root reaches is asked
# what it holds, and what comes back names the folders below it; the walk ends
# where nothing names another. It is `--fail`ing curl the whole way, so a folder
# that does not answer stops the run where it stands.
#
# The root is the empty first line, because the Library root is a place to stand
# and not a path (spec: EP-2).
pending="$WORK/pending"
printf '\n' >"$pending"
reached=""
while [ -s "$pending" ]; do
  folder="$(head -n 1 "$pending")"
  tail -n +2 "$pending" >"$pending.rest"
  mv "$pending.rest" "$pending"

  held="$(listing "$folder")" || fail "/api/list did not answer for '$folder'."
  printf '%s' "$held" | jq --exit-status --arg path "$folder" '.path == $path' >/dev/null ||
    fail "/api/list?path=$folder answered about another folder: $held"

  case "$folder" in
    "$PREFIX"*)
      # Every part of the Library this device maps a folder for, which is every
      # part below the mapped top-level component (spec: EP-9).
      printf '%s' "$held" | jq --exit-status '.mapped' >/dev/null ||
        fail "$folder is not mapped on $JOINER: $held"
      ;;
  esac

  case "$folder" in
    "$ALBUM" | "$BOOK")
      # The two folders the journeys walk, as a device that has only just joined
      # sees them: every file is in the Library and not one of them is here.
      # `$CHECKED` is deliberately not held to this — it is the folder this
      # script does its own fetching and adding in, which is why it exists.
      printf '%s' "$held" |
        jq --exit-status 'all(.files[]; .state == "remote")' >/dev/null ||
        fail "a row under $folder is not remote before anything has been fetched: $held"
      ;;
  esac

  reached="$reached$folder
"
  printf '%s' "$held" | jq -r '.folders[].path' >>"$pending"
done

# What the walk found is what the Library has: a folder the tree draws and the
# listing never reaches would be a folder nobody could open.
walked="$(printf '%s' "$reached" | sed '/^$/d' | sort)"
flat="$(api folders | jq -r '.folders[]' | sort)"
[ "$walked" = "$flat" ] ||
  fail "the listing reaches
$walked
and /api/folders names
$flat"

# One Entry's plaintext: fetched from MinIO because this device does not have
# it, placed in the mapped folder, and served from there.
served="$WORK/served.jpg"
content_type="$(
  curl --fail --silent --show-error \
    --header "x-coffret-key: $(server_key)" \
    --output "$served" \
    --write-out '%{content_type}' \
    "$API/file?path=$(printf '%s' "$CHECKED/served.jpg" | jq -sRr @uri)"
)" || fail "/api/file did not serve $CHECKED/served.jpg."
case "$content_type" in
  image/*) ;;
  *) fail "/api/file served $CHECKED/served.jpg as $content_type" ;;
esac
[ -s "$served" ] || fail "/api/file served $CHECKED/served.jpg as nothing at all."
[ -f "$JOINER_ROOT/$CHECKED_NAME/served.jpg" ] ||
  fail "the fetch did not place the file in the folder $JOINER maps."
echo "served $CHECKED/served.jpg as $content_type, $(wc -c <"$served" | tr -d ' ') bytes."

# A sync from the command line while the server holds the same Index open. Both
# of them are writing to one SQLite file on one device, and this is the one
# place that arrangement is exercised against real files.
run_cli "$JOINER_STATE" sync --library "$JOINER" --passphrase-stdin ||
  fail "a sync beside the running server failed."
listing "$CHECKED" >/dev/null || fail "the listing stopped answering after a sync ran beside it."
echo "a sync ran beside the server, and the listing still answers."

# And the one route that carries anything into the Library. A file added to a
# mapped folder is in the folder from that moment and in the Library when the
# sync the server armed has committed it.
added="$WORK/added.jpg"
cp "$SPARE/album-000/img-00000.jpg" "$added"
curl --fail --silent --show-error --output /dev/null \
  --header "x-coffret-key: $(server_key)" \
  --form "file=@${added};filename=added.jpg" \
  "$API/upload?path=$(printf '%s' "$CHECKED" | jq -sRr @uri)" ||
  fail "/api/upload did not take the file."

listing "$CHECKED" |
  jq --exit-status 'any(.files[]; .name == "added.jpg" and .state == "uploading" and .container == null)' \
    >/dev/null ||
  fail "the added file is not listed as uploading: $(listing "$CHECKED")"

for _ in $(seq "$SYNC_TIMEOUT_SECONDS"); do
  if listing "$CHECKED" |
    jq --exit-status 'any(.files[]; .name == "added.jpg" and .state == "present" and .container == "one-file")' \
      >/dev/null; then
    committed=1
    break
  fi
  sleep 1
done
[ "${committed:-}" = 1 ] ||
  fail "the sync the upload armed did not carry the file in within ${SYNC_TIMEOUT_SECONDS}s: $(listing "$CHECKED")"
echo "a dropped file was listed as uploading and became an Entry."

# And the other way of carrying files in: a book. The pages go up in one request
# onto a folder that does not exist yet, with `freeze=true` saying what the
# gesture is — which is what a browser sends for a drop onto a folder somebody
# just made in it. What the server does with them is a freeze rather than a
# sync, so what the Library ends up holding is Packs.
#
# The Pack-kind check is here rather than in the browser stage because the kind
# of Container an Entry lives in is not something a person sees: the listing
# carries it, the explorer draws a state and not a Container, and a stage that
# asked the routes is the right place to hold the answer to it.
readonly IMPORTED_NAME="imported-by-the-api"
readonly IMPORTED="$CHECKED/$IMPORTED_NAME"
book_parts=()
for page in "$API_BOOK"/*.jpg; do
  book_parts+=(--form "file=@${page};filename=$(basename "$page")")
done
curl --fail --silent --show-error --output /dev/null \
  --header "x-coffret-key: $(server_key)" \
  "${book_parts[@]}" \
  "$API/upload?path=$(printf '%s' "$IMPORTED" | jq -sRr @uri)&freeze=true" ||
  fail "/api/upload did not take the book."

for _ in $(seq "$SYNC_TIMEOUT_SECONDS"); do
  if listing "$IMPORTED" |
    jq --exit-status --argjson pages "$IMPORT_PAGES" \
      '(.files | length) == $pages and all(.files[]; .state == "present" and .container == "pack")' \
      >/dev/null; then
    packed=1
    break
  fi
  sleep 1
done
[ "${packed:-}" = 1 ] ||
  fail "the freeze the book drop armed did not pack the pages within ${SYNC_TIMEOUT_SECONDS}s: $(listing "$IMPORTED")"
echo "a dropped book was packed: $IMPORT_PAGES pages, every one of them in a Pack."

# And the other device reads it back, which is two answers at once. It has none
# of these files, so what it fetches it fetches from Storage; and it fetches them
# out of fewer Containers than there are pages, because the fetch unit is the
# whole Container however many of its Entries were wanted (spec: PK-16). A folder
# carried in one Container per page would answer with one Container each.
fetched="$(run_cli "$UPLOADER_STATE" fetch --library "$UPLOADER" --under "$IMPORTED" --passphrase-stdin)" ||
  fail "the other device could not fetch the packed book."
read -r pages_back containers_back <<<"$(
  printf '%s' "$fetched" |
    sed -n 's/^fetched \([0-9]*\), containers \([0-9]*\).*/\1 \2/p' | head -n 1
)"
[ "${pages_back:-0}" = "$IMPORT_PAGES" ] ||
  fail "the other device fetched ${pages_back:-no} of the $IMPORT_PAGES pages: $fetched"
[ -n "${containers_back:-}" ] && [ "$containers_back" -lt "$IMPORT_PAGES" ] ||
  fail "the other device read the book out of ${containers_back:-?} Containers for
$IMPORT_PAGES pages, which is what a folder carried in one Container per file looks
like rather than a packed one: $fetched"
echo "the other device read the $IMPORT_PAGES-page book back out of $containers_back Container(s)."

# The browser stage starts a server of its own, on this same port, because two
# of its journeys have to kill one and start it again.
stop_server

# ---------------------------------------------------------------------------
# Stage 2: the journeys, in a browser.
# ---------------------------------------------------------------------------

echo
echo "--- stage 2: seven journeys in Chromium ---"
# The outage journey kills the server, and so does the one that starts it again
# over a Library another device has moved on. For as long as it is down the vite
# proxy in front of the explorer prints a stack trace for every request the page
# makes. They land in the middle of a passing run, between one journey's tick and
# the next, and a run that prints `Error: connect ECONNREFUSED` is worth telling
# somebody about before they read it as the run going wrong.
echo "Two of them take the server away on purpose. While it is gone the proxy"
echo "the explorer is served behind prints a connection error for every request"
echo "the page makes, and those errors are those journeys working."
echo
cd "$ROOT/frontend"
COFFRET_E2E_SERVER_BIN="$SERVER" \
COFFRET_E2E_STATE_DIR="$JOINER_STATE" \
COFFRET_E2E_LIBRARY="$JOINER" \
COFFRET_E2E_LOG_DIR="$LOG_DIR" \
COFFRET_E2E_PASSPHRASE="$PASSPHRASE" \
COFFRET_E2E_SERVER_PORT="$SERVER_PORT" \
COFFRET_E2E_WEB_PORT="$WEB_PORT" \
COFFRET_E2E_SCREENSHOTS="$SCREENSHOTS" \
COFFRET_E2E_ARTIFACTS="$ARTIFACTS" \
COFFRET_E2E_ALBUM="$ALBUM" \
COFFRET_E2E_BOOK="$BOOK" \
COFFRET_E2E_PHOTOS="$PHOTOS" \
COFFRET_E2E_PAGES="$PAGES" \
COFFRET_E2E_DROP_FILE="$DROP_FILE" \
COFFRET_E2E_IMPORT_DIR="$BROWSER_BOOK" \
COFFRET_E2E_IMPORT_PAGES="$IMPORT_PAGES" \
COFFRET_E2E_CLI_BIN="$COFFRET" \
COFFRET_E2E_UPLOADER_STATE_DIR="$UPLOADER_STATE" \
COFFRET_E2E_UPLOADER_LIBRARY="$UPLOADER" \
COFFRET_E2E_UPLOADER_ROOT="$UPLOADER_ROOT" \
COFFRET_E2E_REFRESH_FILE="$REFRESH_FILE" \
COFFRET_E2E_RESTART_FILE="$RESTART_FILE" \
  pnpm --filter @coffret/e2e run test:e2e

echo
echo "=== the journeys held ==="
echo
echo "Screenshots:  $SCREENSHOTS"
echo "Transcript:   $TRANSCRIPT"
echo "Logs:         $LOG_DIR"
echo
echo "The pictures are the part a machine did not judge. Look at them."
