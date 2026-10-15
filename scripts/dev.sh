#!/usr/bin/env bash
#
# The explorer for everyday use, from one command. `make dev` starts the
# server for a Library, a build of the explorer in watch mode, and the
# development host that serves that build in front of the server, all in the
# background, and `make down` stops them again; `make down dev` is the
# restart, which is what a Library that has locked itself after idling needs,
# since a locked server is started afresh rather than unlocked in place.
# `make web` is the last two in the foreground, in front of a server started
# some other way. It is for a Library under the development state directory;
# the installed desktop app serves the production Library
# (docs/guides/environments.md).
#
#   scripts/dev.sh up <library> <port> <web-port>
#   scripts/dev.sh web <library> <port> <web-port>
#   scripts/dev.sh down <library>
#
# The development host (`coffret-explorer-dev`) is the one browser entrance,
# as the desktop app's host is: it serves the build on disk, admits a browser
# only by a browser token, and forwards /api to the server with the key the
# server published. A browser gets its token by opening the launch file the
# host writes and prints the path of; the address on its own shows a page
# saying so. A change to the frontend is picked up by reloading the tab once
# the build in watch mode has written it.
#
# What a run leaves behind lives under .tmp/dev/<library>/: a pid file and a
# log per process. The pid files are what `down` reads. The logs are where
# the three processes' own words go — the server's startup lines, the build's,
# the host's address and its launch file, and whatever each said as it
# stopped — since none of them has a terminal to say them in.
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
  echo "usage: scripts/dev.sh up|web <library> <port> <web-port> | scripts/dev.sh down <library>" >&2
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
WEB_PORT=""
case "$COMMAND" in
  up | web)
    [ $# -eq 4 ] || usage
    PORT="$3"
    WEB_PORT="$4"
    ;;
  down)
    [ $# -eq 2 ] || usage
    ;;
  *)
    usage
    ;;
esac
readonly PORT WEB_PORT

# The name is also a directory name below, so the two things a directory name
# cannot be are refused here; everything else about it is for the server to
# judge.
case "$LIBRARY" in
  '' | . | .. | */*)
    fail "\"$LIBRARY\" is not a Library name."
    ;;
esac

# A relative COFFRET_STATE_DIR means a directory under the repository root, as
# it does for `make server`. Made absolute here, because the build is started
# from its own package directory, where the same relative path would name
# somewhere else, and so that the host resolves the Library the server serves.
if [ -n "${COFFRET_STATE_DIR:-}" ]; then
  case "$COFFRET_STATE_DIR" in
    /*) ;;
    *) export COFFRET_STATE_DIR="$ROOT/$COFFRET_STATE_DIR" ;;
  esac
fi

readonly RUN_DIR="$ROOT/.tmp/dev/$LIBRARY"
readonly SERVER_PID="$RUN_DIR/server.pid"
readonly SERVER_LOG="$RUN_DIR/server.log"
readonly BUILD_PID="$RUN_DIR/build.pid"
readonly BUILD_LOG="$RUN_DIR/build.log"
readonly HOST_PID="$RUN_DIR/host.pid"
readonly HOST_LOG="$RUN_DIR/host.log"
readonly SERVER="$ROOT/backend/target/release/coffret-server"
readonly HOST="$ROOT/backend/target/release/coffret-explorer-dev"
readonly WEB_PACKAGE="$ROOT/frontend/packages/apps/web"
readonly VITE="$WEB_PACKAGE/node_modules/.bin/vite"
# What the build writes, and the host serves.
readonly DIST="$WEB_PACKAGE/dist"

# The pid in a pid file, where that process is still running and is still the
# one the file was written for: a pid is reused once its process is gone, and a
# file left behind by a run that was killed outright would otherwise name
# whatever has the number now. What the process was started as is compared
# against what this script starts, by its full path and the Library it was
# started for, so that another checkout's build or host on a reused pid is not
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

# What `ps` shows for the three processes this script starts: the server and
# the host by their binaries and Library, and the build by the vite inside this
# checkout (node shows the script's path, not the bin's).
readonly SERVER_STARTED_AS="$SERVER --library $LIBRARY "
readonly BUILD_STARTED_AS="$WEB_PACKAGE/node_modules/"
readonly HOST_STARTED_AS="$HOST --library $LIBRARY "

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
    fail "something is already answering at http://127.0.0.1:$PORT, and it was not started by \`make dev\` for the Library \"$LIBRARY\".
If it is a coffret-server from \`make server\`, stop it; or pass another PORT."

  # Built before it is started rather than by `cargo run`, so that the
  # compiler's output is over before the server asks for the Passphrase.
  cargo build --release --manifest-path backend/Cargo.toml -p coffret-server

  echo "Starting the server for the Library \"$LIBRARY\" at http://127.0.0.1:$PORT. It asks for the Passphrase:"
  : >"$SERVER_LOG"
  # Hangups ignored, so that closing the terminal this was started from does
  # not take the server with it: `down` is what stops it. Not detached from
  # the terminal the way the build and the host below are, because the server
  # has to open it once, for the Passphrase. Standard input from nowhere, because the
  # server reads the Passphrase from the terminal device itself and must not
  # be left holding this shell's.
  trap abandon INT TERM
  (
    trap '' HUP
    exec "$SERVER" --library "$LIBRARY" --port "$PORT"
  ) </dev/null >>"$SERVER_LOG" 2>&1 &
  STARTED=$!
  echo "$STARTED" >"$SERVER_PID"

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

# A process started as a detached child of a node one-liner: its own session,
# with no terminal to hang up on it, and the pid of the process itself written
# down, so that `down` stops that process rather than a wrapper it could
# outlive. Not the way the server above is started, because node resets every
# signal disposition it inherits as it starts, so an ignored hangup would not
# reach vite; and not `setsid`, which macOS does not ship.
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

# Builds the development host, before anything waits on it.
build_host() {
  cargo build --release --manifest-path backend/Cargo.toml -p coffret-explorer-dev
}

# Waits until `log` holds `line`, while the process in `pid_file` runs, for at
# most `seconds`; `what` names the process in a failure.
await_line() {
  local log="$1" line="$2" pid_file="$3" what="$4" seconds="$5" _
  for _ in $(seq "$seconds"); do
    if grep -q "$line" "$log" 2>/dev/null; then
      return 0
    fi
    kill -0 "$(cat "$pid_file")" 2>/dev/null || {
      rm -f "$pid_file"
      fail "$what stopped before it was ready. It said:
$(last_words "$log")"
    }
    sleep 1
  done
  fail "$what was not ready within ${seconds}s; see $log"
}

# The explorer, built in watch mode: written into $DIST once, and again each
# time a file it is built from changes. In development mode, so that the
# gateway package is read from its source as it changes too (vite.config.ts).
start_build() {
  : >"$BUILD_LOG"
  # NO_COLOR, because what the build said is read back out of the log below,
  # and vite colours it wherever FORCE_COLOR or CI is in the environment,
  # terminal or not.
  NO_COLOR=1 node -e "$SPAWN_DETACHED" -- "$VITE" "$WEB_PACKAGE" "$BUILD_LOG" \
    build --watch --mode development >"$BUILD_PID"
  await_line "$BUILD_LOG" 'built in' "$BUILD_PID" "the explorer's build" 120
}

# The development host, in front of the server at PORT, on WEB_PORT.
start_host() {
  : >"$HOST_LOG"
  node -e "$SPAWN_DETACHED" -- "$HOST" "$ROOT" "$HOST_LOG" \
    --library "$LIBRARY" --server-port "$PORT" --port "$WEB_PORT" --dist "$DIST" >"$HOST_PID"
  # The line it prints once it has seen the server and written the launch
  # file, which with the server already answering is its first look.
  await_line "$HOST_LOG" 'Open the explorer by opening' "$HOST_PID" "the development host" 30
}

# Where to open the explorer from: the launch file, read off the host's log,
# which is the one way in.
say_where() {
  local opening
  opening="$(grep 'Open the explorer by opening\|Open .* in the browser again' "$HOST_LOG" 2>/dev/null | tail -n 1 || true)"
  echo "The explorer is at http://127.0.0.1:$WEB_PORT/, over the server at http://127.0.0.1:$PORT."
  echo "${opening:-The launch file to open is named in $HOST_LOG.}"
  echo "Logs are under $RUN_DIR. \`make down\` stops all three."
}

up() {
  local server build host
  # Everything that can be refused is refused here, before the server is built
  # and somebody is asked for a Passphrase to no purpose.
  for tool in curl cargo node; do
    command -v "$tool" >/dev/null 2>&1 ||
      fail "$tool is needed and was not found on PATH."
  done
  [ -x "$VITE" ] ||
    fail "the frontend's dependencies are not installed; run \`pnpm install\` in frontend/ first."
  mkdir -p "$RUN_DIR"
  server="$(running "$SERVER_PID" "$SERVER_STARTED_AS")"
  build="$(running "$BUILD_PID" "$BUILD_STARTED_AS")"
  host="$(running "$HOST_PID" "$HOST_STARTED_AS")"
  # A server that is running is only reused where it answers at this PORT: one
  # started for another port, or one still at its prompt from a run that was
  # cut short, would otherwise be reported as serving where nothing is.
  if [ -n "$server" ] && ! something_answers; then
    fail "a server for the Library \"$LIBRARY\" is running (pid $server) but nothing answers at http://127.0.0.1:$PORT.
\`make down\` stops it; or pass the PORT it was started with."
  fi
  if [ -n "$server" ] && [ -n "$build" ] && [ -n "$host" ]; then
    echo "Already up for the Library \"$LIBRARY\": the server (pid $server), the build (pid $build) and the host (pid $host)."
    say_where
    return 0
  fi
  build_host
  [ -n "$server" ] || start_server
  [ -n "$build" ] || start_build
  [ -n "$host" ] || start_host
  say_where
}

# The build in watch mode in the background and the host in the foreground, in
# front of a server started some other way; Ctrl-C stops both.
web() {
  [ -x "$VITE" ] ||
    fail "the frontend's dependencies are not installed; run \`pnpm install\` in frontend/ first."
  mkdir -p "$RUN_DIR"
  build_host
  [ -z "$(running "$BUILD_PID" "$BUILD_STARTED_AS")" ] ||
    fail "a build for the Library \"$LIBRARY\" is already running from \`make dev\`; \`make down\` stops it."
  start_build
  # shellcheck disable=SC2064 # the pid file's path is fixed now
  trap "kill \"\$(cat '$BUILD_PID')\" 2>/dev/null || true; rm -f '$BUILD_PID'" EXIT
  echo "The explorer's build is watching for changes; what it says is in $BUILD_LOG."
  "$HOST" --library "$LIBRARY" --server-port "$PORT" --port "$WEB_PORT" --dist "$DIST"
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
  STOPPED=0
  stop "$HOST_PID" "$HOST_STARTED_AS" "the development host"
  stop "$BUILD_PID" "$BUILD_STARTED_AS" "the explorer's build"
  stop "$SERVER_PID" "$SERVER_STARTED_AS" "the server for the Library \"$LIBRARY\""
  if [ "$STOPPED" -eq 0 ]; then
    echo "Nothing that \`make dev\` started for the Library \"$LIBRARY\" is running."
  fi
}

case "$COMMAND" in
  up) up ;;
  web) web ;;
  down) down ;;
esac
