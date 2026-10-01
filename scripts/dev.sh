#!/usr/bin/env bash
#
# The explorer for everyday use, from one command. `make dev` starts the
# server for a Library and the frontend's dev server in front of it, both in
# the background, and `make down` stops them again; `make down dev` is the
# restart, which is what a Library that has locked itself after idling needs,
# since a locked server is started afresh rather than unlocked in place.
#
#   scripts/dev.sh up <library> <port>
#   scripts/dev.sh down <library>
#
# What a run leaves behind lives under .tmp/dev/<library>/: a pid file and a
# log per process. The pid files are what `down` reads, and the logs are where
# the two processes' own words go — the server's startup lines, the dev
# server's address, and whatever either said as it stopped — since neither has
# a terminal to say them in.
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
  echo "usage: scripts/dev.sh up <library> <port> | scripts/dev.sh down <library>" >&2
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
  up)
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
readonly SERVER="$ROOT/backend/target/release/coffret-server"
readonly VITE="$ROOT/frontend/packages/apps/web/node_modules/.bin/vite"

# The pid in a pid file, where that process is still running and is still the
# one the file was written for: a pid is reused once its process is gone, and a
# file left behind by a run that was killed outright would otherwise name
# whatever has the number now. Prints nothing for a missing or stale file.
running() {
  local file="$1" word="$2" pid
  [ -f "$file" ] || return 0
  pid="$(cat "$file")"
  [ -n "$pid" ] || return 0
  kill -0 "$pid" 2>/dev/null || return 0
  case "$(ps -o args= -p "$pid" 2>/dev/null)" in
    *"$word"*) echo "$pid" ;;
  esac
}

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

start_server() {
  # Nothing may be answering there yet. A server started by `make server`, or
  # one left over from a run of this script whose pid file is gone, holds the
  # port; the one started below would stop on the bind, and `up` would then be
  # waiting on a server that is not the one it started.
  ! something_answers ||
    fail "something is already answering at http://127.0.0.1:$PORT, and it was not started by \`make dev\`.
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
  (
    trap '' HUP
    exec "$SERVER" --library "$LIBRARY" --port "$PORT"
  ) </dev/null >>"$SERVER_LOG" 2>&1 &
  echo $! >"$SERVER_PID"

  # No deadline: what this waits on is a person typing a Passphrase, and after
  # it the server catching up with Storage, which is on a deadline of its own.
  until something_answers; do
    kill -0 "$(cat "$SERVER_PID")" 2>/dev/null || {
      rm -f "$SERVER_PID"
      fail "the server stopped before it answered. It said:
$(last_words "$SERVER_LOG")"
    }
    sleep 1
  done
}

# The dev server, started as a detached child of a node one-liner: its own
# session, with no terminal to hang up on it, and the pid of the dev server
# itself written down, so that `down` stops that process rather than a wrapper
# it could outlive. Not the way the server above is started, because node
# resets every signal disposition it inherits as it starts, so an ignored
# hangup would not reach the dev server; and not `setsid`, which macOS does not
# ship. The variables are the ones `make web` sets: which Library's key the
# proxy reads, and which port it forwards /api to.
readonly SPAWN_DETACHED='
const { spawn } = require("node:child_process");
const { openSync } = require("node:fs");
const [command, cwd, log] = process.argv.slice(1);
const out = openSync(log, "a");
const child = spawn(command, [], { cwd, detached: true, stdio: ["ignore", out, out] });
child.on("error", (error) => {
  console.error(error.message);
  process.exit(1);
});
console.log(child.pid);
child.unref();
'

start_web() {
  [ -x "$VITE" ] ||
    fail "the frontend's dependencies are not installed; run \`pnpm install\` in frontend/ first."

  : >"$WEB_LOG"
  COFFRET_LIBRARY="$LIBRARY" COFFRET_PORT="$PORT" \
    node -e "$SPAWN_DETACHED" -- "$VITE" "$ROOT/frontend/packages/apps/web" "$WEB_LOG" >"$WEB_PID"

  local _
  for _ in $(seq 60); do
    if grep -q 'Local:' "$WEB_LOG" 2>/dev/null; then
      return 0
    fi
    kill -0 "$(cat "$WEB_PID")" 2>/dev/null || {
      rm -f "$WEB_PID"
      fail "the dev server stopped before it answered. It said:
$(last_words "$WEB_LOG")"
    }
    sleep 1
  done
  fail "the dev server did not say where it is within 60s; see $WEB_LOG"
}

# Where to open the explorer. Read off the dev server's log rather than assumed,
# because it takes the next free port when its usual one is held by another
# dev server on this device.
say_where() {
  local url
  url="$(grep -o 'http://localhost:[0-9]*/' "$WEB_LOG" 2>/dev/null | head -n 1 || true)"
  echo "The explorer is at ${url:-the address in $WEB_LOG}, over the server at http://127.0.0.1:$PORT."
  echo "Logs are under $RUN_DIR. \`make down\` stops both."
}

up() {
  local server web
  for tool in curl cargo node; do
    command -v "$tool" >/dev/null 2>&1 ||
      fail "$tool is needed and was not found on PATH."
  done
  mkdir -p "$RUN_DIR"
  server="$(running "$SERVER_PID" coffret-server)"
  web="$(running "$WEB_PID" vite)"
  if [ -n "$server" ] && [ -n "$web" ]; then
    echo "Already up for the Library \"$LIBRARY\": the server (pid $server) and the dev server (pid $web)."
    say_where
    return 0
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
  local file="$1" word="$2" what="$3" pid
  pid="$(running "$file" "$word")"
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
  STOPPED=0
  stop "$WEB_PID" vite "the dev server"
  stop "$SERVER_PID" coffret-server "the server for the Library \"$LIBRARY\""
  if [ "$STOPPED" -eq 0 ]; then
    echo "Nothing that \`make dev\` started for the Library \"$LIBRARY\" is running."
  fi
}

case "$COMMAND" in
  up) up ;;
  down) down ;;
esac
