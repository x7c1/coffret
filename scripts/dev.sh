#!/usr/bin/env bash
#
# The explorer for everyday use, from one command. `make dev` starts the
# server for a Library and the frontend's dev server in front of it, both in
# the background, and `make down` stops them again; `make down dev` is the
# restart, which is what a Library that has locked itself after idling needs,
# since a locked server is started afresh rather than unlocked in place.
#
# `make prod` starts the production pair instead: the same server, and the
# explorer built and served by `vite preview` rather than by the dev server,
# which reads the checkout's source as it changes. Before it builds anything it
# pins the checkout to the head of `main` — it refuses on another branch or
# with uncommitted changes, and otherwise fast-forwards to `origin/main` — so
# that the production build is always that head. `make down` stops either
# pair. docs/guides/environments.md says which checkout runs which.
#
#   scripts/dev.sh up <library> <port>
#   scripts/dev.sh prod <library> <port>
#   scripts/dev.sh down <library>
#
# What a run leaves behind lives under .tmp/dev/<library>/: a pid file and a
# log per process, and a file saying which kind of pair — `dev` or `prod` —
# was started. The pid files are what `down` reads, and the kind is what keeps
# one kind from being started over the other for the same Library: `up` and
# `prod` share these files, so either reports a pair of the other kind as
# already up rather than starting a second server beside it. The logs are where
# the two processes' own words go — the server's startup lines, the explorer's
# address, and whatever either said as it stopped — since neither has a
# terminal to say them in.
#
# The Passphrase never passes through here. The server asks for it itself, on
# the terminal this was started from: it opens the terminal device directly,
# so it is asked even though the server's output goes to its log. `up` waits
# for the server to answer before it starts anything else, so that the prompt
# is the only thing on the screen while it is being answered.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT
cd "$ROOT"

usage() {
  echo "usage: scripts/dev.sh up|prod <library> <port> | scripts/dev.sh down <library>" >&2
  exit 2
}

fail() {
  echo "error: $*" >&2
  exit 1
}

[ $# -ge 2 ] || usage
readonly COMMAND="$1"
readonly LIBRARY="$2"
PORT=""
case "$COMMAND" in
  up | prod)
    [ $# -eq 3 ] || usage
    PORT="$3"
    ;;
  down)
    [ $# -eq 2 ] || usage
    ;;
  *)
    usage
    ;;
esac
readonly PORT

# The name is also a directory name below, so the two things a directory name
# cannot be are refused here; everything else about it is for the server to
# judge.
case "$LIBRARY" in
  '' | . | .. | */*)
    fail "\"$LIBRARY\" is not a Library name."
    ;;
esac

# A relative COFFRET_STATE_DIR means a directory under the repository root, as
# it does for `make server`. Made absolute here, because the dev server is
# started from its own package directory, where the same relative path would
# name somewhere else.
if [ -n "${COFFRET_STATE_DIR:-}" ]; then
  case "$COFFRET_STATE_DIR" in
    /*) ;;
    *) export COFFRET_STATE_DIR="$ROOT/$COFFRET_STATE_DIR" ;;
  esac
fi

readonly RUN_DIR="$ROOT/.tmp/dev/$LIBRARY"
readonly SERVER_PID="$RUN_DIR/server.pid"
readonly SERVER_LOG="$RUN_DIR/server.log"
readonly WEB_PID="$RUN_DIR/web.pid"
readonly WEB_LOG="$RUN_DIR/web.log"
readonly KIND_FILE="$RUN_DIR/kind"
readonly SERVER="$ROOT/backend/target/release/coffret-server"
readonly VITE="$ROOT/frontend/packages/apps/web/node_modules/.bin/vite"

# The pid in a pid file, where that process is still running and is still the
# one the file was written for: a pid is reused once its process is gone, and a
# file left behind by a run that was killed outright would otherwise name
# whatever has the number now. What the process was started as is compared
# against what this script starts, by its full path and the Library it was
# started for, so that another checkout's dev server on a reused pid is not
# taken for ours. Prints nothing for a missing or stale file.
running() {
  local file="$1" started_as="$2" pid
  [ -f "$file" ] || return 0
  pid="$(cat "$file")"
  [ -n "$pid" ] || return 0
  kill -0 "$pid" 2>/dev/null || return 0
  case "$(ps -o args= -p "$pid" 2>/dev/null)" in
    *"$started_as"*) echo "$pid" ;;
  esac
}

# What `ps` shows for the two processes this script starts: the server by its
# binary and Library, and the explorer — the dev server or `vite preview` — by
# the vite inside this checkout (node shows the script's path, not the bin's).
readonly SERVER_STARTED_AS="$SERVER --library $LIBRARY "
readonly WEB_STARTED_AS="$ROOT/frontend/packages/apps/web/node_modules/"

# Whether anything at all is listening at the server's port, whatever it
# answers with. A server not started here refuses a request carrying no key,
# and a refusal is still something holding the port.
something_answers() {
  curl --silent --output /dev/null --max-time 2 "http://127.0.0.1:$PORT/api/library"
}

# The last few lines a process wrote before it stopped, for a failure to quote.
last_words() {
  tail -n 5 "$1" 2>/dev/null || true
}

# What this run has started and not yet confirmed. A Ctrl-C at the prompt is
# the server's own to handle, and it stops over it. One after the prompt —
# while the server is catching up with Storage — or a TERM from elsewhere ends
# this script and `make` but not the server: an asynchronous command of a
# shell without job control is left ignoring SIGINT. So the interruption is
# caught here and the server stopped with it, and the terminal put back as the
# prompt found it, since a server stopped mid-prompt does not get to restore
# the echo it turned off.
STARTED=""
abandon() {
  trap - INT TERM
  if [ -n "$STARTED" ]; then
    kill "$STARTED" 2>/dev/null || true
    rm -f "$SERVER_PID"
    stty echo </dev/tty 2>/dev/null || true
    echo
    echo "Stopped the server that was being started." >&2
  fi
  exit 130
}

start_server() {
  # Nothing may be answering there yet. A server started by `make server`, or
  # one left over from a run of this script whose pid file is gone, holds the
  # port; the one started below would stop on the bind, and `up` would then be
  # waiting on a server that is not the one it started.
  ! something_answers ||
    fail "something is already answering at http://127.0.0.1:$PORT, and it was not started by \`make dev\` or \`make prod\` for the Library \"$LIBRARY\".
If it is a coffret-server from \`make server\`, stop it; or pass another PORT."

  # Built before it is started rather than by `cargo run`, so that the
  # compiler's output is over before the server asks for the Passphrase.
  cargo build --release --manifest-path backend/Cargo.toml -p coffret-server

  echo "Starting the server for the Library \"$LIBRARY\" at http://127.0.0.1:$PORT. It asks for the Passphrase:"
  : >"$SERVER_LOG"
  # Hangups ignored, so that closing the terminal this was started from does
  # not take the server with it: `down` is what stops it. Not detached from
  # the terminal the way the dev server below is, because the server has to
  # open it once, for the Passphrase. Standard input from nowhere, because the
  # server reads the Passphrase from the terminal device itself and must not
  # be left holding this shell's.
  trap abandon INT TERM
  (
    trap '' HUP
    exec "$SERVER" --library "$LIBRARY" --port "$PORT"
  ) </dev/null >>"$SERVER_LOG" 2>&1 &
  STARTED=$!
  echo "$STARTED" >"$SERVER_PID"
  echo "$KIND" >"$KIND_FILE"

  # No deadline: what this waits on is a person typing a Passphrase, and after
  # it the server catching up with Storage, which is on a deadline of its own.
  until something_answers; do
    kill -0 "$STARTED" 2>/dev/null || {
      rm -f "$SERVER_PID"
      STARTED=""
      fail "the server stopped before it answered. It said:
$(last_words "$SERVER_LOG")"
    }
    sleep 1
  done
  STARTED=""
  trap - INT TERM
}

# The explorer, started as a detached child of a node one-liner: its own
# session, with no terminal to hang up on it, and the pid of vite itself
# written down, so that `down` stops that process rather than a wrapper it
# could outlive. Not the way the server above is started, because node resets
# every signal disposition it inherits as it starts, so an ignored hangup would
# not reach vite; and not `setsid`, which macOS does not ship. The variables
# are the ones `make web` sets: which Library's key the proxy reads, and which
# port it forwards /api to. `vite preview` reads them the same way, since
# vite.config.ts gives the preview the dev server's proxy.
readonly SPAWN_DETACHED='
const { spawn } = require("node:child_process");
const { openSync } = require("node:fs");
const [command, cwd, log, ...args] = process.argv.slice(1);
const out = openSync(log, "a");
const child = spawn(command, args, { cwd, detached: true, stdio: ["ignore", out, out] });
child.on("error", (error) => {
  console.error(error.message);
  process.exit(1);
});
console.log(child.pid);
child.unref();
'

# The dev server for `up`, and `vite preview` over the built explorer for
# `prod`. Each listens on vite's own default port — 5173 and 4173 — which is
# what lets a production pair and a development pair run at once on one
# device; either moves to the next free port when its own is held.
start_web() {
  local mode=()
  [ "$KIND" = prod ] && mode=(preview)
  : >"$WEB_LOG"
  # NO_COLOR, because the address is read back out of the log below, and vite
  # colours it wherever FORCE_COLOR or CI is in the environment, terminal or
  # not.
  COFFRET_LIBRARY="$LIBRARY" COFFRET_PORT="$PORT" NO_COLOR=1 \
    node -e "$SPAWN_DETACHED" -- "$VITE" "$ROOT/frontend/packages/apps/web" "$WEB_LOG" ${mode[@]+"${mode[@]}"} >"$WEB_PID"
  echo "$KIND" >"$KIND_FILE"

  local _
  for _ in $(seq 60); do
    if grep -q 'Local:' "$WEB_LOG" 2>/dev/null; then
      return 0
    fi
    kill -0 "$(cat "$WEB_PID")" 2>/dev/null || {
      rm -f "$WEB_PID"
      fail "the explorer's $(web_name) stopped before it answered. It said:
$(last_words "$WEB_LOG")"
    }
    sleep 1
  done
  fail "the explorer's $(web_name) did not say where it is within 60s; see $WEB_LOG"
}

# What serves the explorer for a kind of pair, for messages to name.
web_name() {
  case "${1:-$KIND}" in
    prod) echo "\`vite preview\`" ;;
    *) echo "dev server" ;;
  esac
}

# Where to open the explorer. Read off vite's log rather than assumed, because
# it takes the next free port when its usual one is held by another vite on
# this device. The dev server and `vite preview` print their address on the
# same `Local:` line, so one reading serves both.
say_where() {
  local url
  url="$(grep -o 'http://localhost:[0-9]*/' "$WEB_LOG" 2>/dev/null | head -n 1 || true)"
  echo "The explorer is at ${url:-the address in $WEB_LOG}, over the server at http://127.0.0.1:$PORT."
  echo "Logs are under $RUN_DIR. \`make down\` stops both."
}

# The kind of pair this Library's files were last started as. A missing file
# counts as a `dev` pair, the only kind ever started without writing one.
recorded_kind() {
  if [ -f "$KIND_FILE" ]; then
    cat "$KIND_FILE"
  else
    echo dev
  fi
}

# Puts the checkout at the head of `main` before a production build, or
# refuses. On another branch the build would be that branch's, and with
# uncommitted changes it would be something no commit names; either way it
# would not be what `main` says production is. A checkout that has diverged
# from `origin/main` is refused by the merge itself, since only a fast-forward
# is allowed, and one whose `main` holds commits `origin/main` does not is
# refused after it, since the merge leaves such a `main` where it is.
pin_to_main() {
  local branch dirty
  branch="$(git symbolic-ref --quiet --short HEAD || true)"
  [ "$branch" = main ] ||
    fail "\`make prod\` builds the head of main, and this checkout is on ${branch:-a detached HEAD}.
The production checkout stays on main: \`git switch main\` first."
  dirty="$(git status --porcelain)"
  [ -z "$dirty" ] ||
    fail "\`make prod\` builds the head of main, and this checkout has changes no commit holds:
$dirty
Commit, stash or remove them first."
  echo "Bringing main up to origin/main."
  git fetch origin
  git merge --ff-only origin/main ||
    fail "main cannot be fast-forwarded to origin/main. The production checkout takes main as it is on origin and nothing else."
  [ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] ||
    fail "main holds commits origin/main does not. The production checkout takes main as it is on origin and nothing else."
}

# The built explorer `vite preview` serves, from what the checkout holds now.
# The dependencies are installed first, as the lockfile pins them, because the
# fast-forward above may have moved the lockfile past what node_modules holds.
build_web() {
  echo "Building the explorer."
  (cd "$ROOT/frontend" && pnpm install --frozen-lockfile && pnpm --filter @coffret/web build)
}

up() {
  local server web recorded tools
  # Everything that can be refused is refused here, before the server is built
  # and somebody is asked for a Passphrase to no purpose.
  tools=(curl cargo node)
  [ "$KIND" = prod ] && tools+=(git pnpm)
  for tool in "${tools[@]}"; do
    command -v "$tool" >/dev/null 2>&1 ||
      fail "$tool is needed and was not found on PATH."
  done
  [ -x "$VITE" ] ||
    fail "the frontend's dependencies are not installed; run \`pnpm install\` in frontend/ first."
  mkdir -p "$RUN_DIR"
  server="$(running "$SERVER_PID" "$SERVER_STARTED_AS")"
  web="$(running "$WEB_PID" "$WEB_STARTED_AS")"
  # One server per Library, whichever kind of pair it belongs to: a pair of the
  # other kind is reported rather than joined or started beside.
  if [ -n "$server" ] || [ -n "$web" ]; then
    recorded="$(recorded_kind)"
    if [ "$recorded" != "$KIND" ]; then
      local held=""
      [ -n "$server" ] && held="the server (pid $server)"
      [ -n "$web" ] && held="${held:+$held and }the $(web_name "$recorded") (pid $web)"
      fail "the Library \"$LIBRARY\" is already up from \`make $recorded\`: $held.
\`make down\` stops it first; one server serves a Library at a time."
    fi
  fi
  # A server that is running is only reused where it answers at this PORT: one
  # started for another port, or one still at its prompt from a run that was
  # cut short, would otherwise be reported as serving where nothing is.
  if [ -n "$server" ] && ! something_answers; then
    fail "a server for the Library \"$LIBRARY\" is running (pid $server) but nothing answers at http://127.0.0.1:$PORT.
\`make down\` stops it; or pass the PORT it was started with."
  fi
  if [ -n "$server" ] && [ -n "$web" ]; then
    echo "Already up for the Library \"$LIBRARY\": the server (pid $server) and the $(web_name) (pid $web)."
    say_where
    return 0
  fi
  if [ "$KIND" = prod ]; then
    # Not pinned while a server is running: moving main under it would leave
    # the server built from one head and the explorer from another.
    [ -n "$server" ] || pin_to_main
    # Built before the server is started, so that its output is over before
    # the Passphrase is asked for.
    [ -n "$web" ] || build_web
  fi
  [ -n "$server" ] || start_server
  [ -n "$web" ] || start_web
  say_where
}

# Stops the process a pid file names, where it is still that process, and
# removes the file either way. A process that has not stopped ten seconds
# after being asked is stopped outright: neither holds anything a kill would
# leave inconsistent, and `down` is for stopping.
stop() {
  local file="$1" started_as="$2" what="$3" pid
  pid="$(running "$file" "$started_as")"
  rm -f "$file"
  [ -n "$pid" ] || return 0
  kill "$pid" 2>/dev/null || true
  local _
  for _ in $(seq 10); do
    kill -0 "$pid" 2>/dev/null || break
    sleep 1
  done
  if kill -0 "$pid" 2>/dev/null; then
    kill -KILL "$pid" 2>/dev/null || true
  fi
  echo "Stopped $what (pid $pid)."
  STOPPED=$((STOPPED + 1))
}

down() {
  local recorded
  recorded="$(recorded_kind)"
  STOPPED=0
  stop "$WEB_PID" "$WEB_STARTED_AS" "the $(web_name "$recorded")"
  stop "$SERVER_PID" "$SERVER_STARTED_AS" "the server for the Library \"$LIBRARY\""
  rm -f "$KIND_FILE"
  if [ "$STOPPED" -eq 0 ]; then
    echo "Nothing that \`make dev\` or \`make prod\` started for the Library \"$LIBRARY\" is running."
  fi
}

case "$COMMAND" in
  up)
    readonly KIND=dev
    up
    ;;
  prod)
    readonly KIND=prod
    up
    ;;
  down) down ;;
esac
