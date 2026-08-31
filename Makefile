# Coffret — unified entry point.
#
# Run `make help` (the default) for the full target list. Every target wraps
# the per-part cargo/pnpm commands so the repo has one place to run things
# from: the repo root.

.DEFAULT_GOAL := help

# Optional overrides, machine-wide then per-checkout (later wins): toolchain
# pins like `export CC := /usr/bin/cc`, parameter defaults, extra targets.
# Both absent on a fresh clone; `-include` skips a missing file silently.
-include $(HOME)/.config/coffret/local.mk
-include local.mk

# Which Library on this device `server` serves, by the name it was created
# under rather than by a path: where a device keeps a Library is the state
# directory's business, and COFFRET_STATE_DIR is what moves that.
LIBRARY ?= main

# Parameters for the fixture generator below.
OUT ?= .tmp/fixtures
PHOTOS ?= 3000
PAGES ?= 300

# Scratch space for the interop exchange. Absolute, because the steps run from
# backend/ and frontend/ in turn; gitignored, because fixture sets are output.
INTEROP ?= $(CURDIR)/.tmp/interop

## help: list available targets
.PHONY: help
help:
	@awk '/^## / { sub(/^## /, ""); i = index($$0, ": "); printf "  \033[36m%-8s\033[0m %s\n", substr($$0, 1, i - 1), substr($$0, i + 2) }' $(MAKEFILE_LIST)

## build: build backend and frontend
.PHONY: build
build:
	cd backend && cargo build
	cd frontend && pnpm -r build

## test: run backend and frontend tests
.PHONY: test
test:
	cd backend && cargo test
	cd frontend && pnpm -r test

## lint: rustfmt + clippy (backend) + eslint (frontend)
.PHONY: lint
lint:
	cd backend && cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings
	cd frontend && pnpm -r lint

## interop: prove the Rust and TypeScript format implementations interoperate
#
# Each side opens what the other encrypted, by exchanging fixture sets through
# INTEROP: Rust writes one, the TypeScript suite opens it and writes one back,
# and Rust opens that. A failure here means the specification or one
# implementation is wrong — never a reason to loosen a check.
.PHONY: interop
interop:
	rm -rf $(INTEROP)
	cd backend && cargo run -p coffret-interop -- generate --out $(INTEROP)/from-rust
	cd frontend && COFFRET_INTEROP_IN=$(INTEROP)/from-rust COFFRET_INTEROP_OUT=$(INTEROP)/from-typescript \
		pnpm --filter @coffret/format test:interop
	cd backend && cargo run -p coffret-interop -- verify --in $(INTEROP)/from-typescript

## s3-store-it: run the conformance suites and the device-layer cases against MinIO in Docker
#
# Separate from `check` because it is the one target that needs a container
# runtime; CI runs it as its own job. The script starts MinIO, runs all five
# suites plus the cases that open a Library from what a device recorded about
# it, and removes the container again, so the target leaves nothing behind
# either way.
#
# What the implementation answered is the point of running it, so the run logs
# every call under ${XDG_STATE_HOME:-$HOME/.local/state}/coffret/logs and prints
# the file it chose. The log is the one thing that outlives the container, which
# is what makes it worth having: an implementation that answers something
# unfamiliar stays readable afterwards instead of being torn down with it.
# Nothing in it is a credential or a path of yours — the keys are opaque and the
# rest is the implementation's own answer. COFFRET_LOG_DIR moves the directory
# and COFFRET_LOG_MAX_BYTES changes the ceiling on how much is kept there.
#
# The file is JSONL: one JSON object per line, each with the fields the event
# was emitted with, so questions about a run are asked of the records rather
# than of a message line. Every refusal and the reason it gave, for instance:
#
#   jq -R 'fromjson? // empty | select(.level == "WARN") | .fields.reason' \
#     "${XDG_STATE_HOME:-$HOME/.local/state}"/coffret/logs/coffret-*.log |
#     sort | uniq -c
#
# `fromjson? // empty` is not decoration. A record too large for one file is cut
# rather than dropped, which leaves one line that is not JSON, followed by a
# marker record carrying "truncated": true. That filter steps over the cut line
# and keeps the marker, so a query says a record was lost there; a plain `jq .`
# would stop at it instead.
#
# COFFRET_LOG is the level, and after it the crates to keep beyond coffret's
# own: `COFFRET_LOG=debug,aws_smithy_runtime` adds the AWS SDK's account of its
# own retries and endpoint resolution. That is off by default because the
# ceiling is shared — the SDK writes hundreds of kilobytes per run, and what
# gets pruned to make room is the older evidence you came here for.
.PHONY: s3-store-it
s3-store-it:
	./scripts/s3-store-it.sh

## drive-authorize: run the Google authorization flow once and cache the grant
#
# Needs a person at a browser, so it is never part of a test run. Set
# COFFRET_DRIVE_CLIENT_ID, COFFRET_DRIVE_TOKEN_CACHE, and COFFRET_MASTER_KEY
# first — the cache is encrypted under that Master Key, and whatever reads it
# afterwards needs the same one, so keep the value instead of minting it
# inline; it is base64 of 32 bytes, which `openssl rand -base64 32` produces.
# The client must be a desktop one: the flow redirects to a loopback port the
# OS picks, which a web client cannot be registered for.
#
# A flow that fails does so against Google's answer, so the run logs that answer
# under ${XDG_STATE_HOME:-$HOME/.local/state}/coffret/logs and prints the file
# it chose. No token is written there.
.PHONY: drive-authorize
drive-authorize:
	cd backend && cargo run -p google-drive-store --example authorize

## drive-store-it: run the same ObjectStore conformance suite, plus the Drive-only observation cases, against a real Google Drive folder
#
# Manual: it needs an account and a grant, so CI never runs it. Authorize first,
# then set COFFRET_DRIVE_FOLDER_ID alongside the variables above. Without them
# the cases report themselves skipped. Any folder id of your own serves, one made
# in the Drive web interface included: a `drive.file` grant may name any folder
# as the parent of something it creates, and each case only creates a subfolder
# there and stays inside it — and trashes it again when the case ends, so a run
# leaves the account as it found it. `root` is refused: it is an alias for a
# folder this application did not create rather than an id it may name, and the
# placement it stands for is the top of My Drive, which is not where a test's
# folders belong.
#
# What Drive answered is the point of running it, so the run logs every call
# under ${XDG_STATE_HOME:-$HOME/.local/state}/coffret/logs and prints the file
# it chose. Nothing in it is a token, a key, or a path of yours: the names Drive
# is sent are opaque, and the rest of what is recorded is Drive's own answer.
# COFFRET_LOG_DIR moves the directory and COFFRET_LOG_MAX_BYTES changes the
# ceiling on how much is kept there. COFFRET_LOG is the level, and after it the
# crates to keep beyond coffret's own — off by default, because the ceiling is
# shared and a dependency that fills it costs you the older evidence.
#
# The file is JSONL, read the same way as the one `s3-store-it` leaves; the
# `jq` recipe above works on it unchanged, and so does the reason it filters
# with `fromjson?`.
.PHONY: drive-store-it
drive-store-it:
	cd backend && cargo test -p google-drive-store --test conformance --test pre_minted_id_reuse -- --nocapture

## drive-round-trip-it: take a folder into a Library on real Google Drive and back out of it, from one command
#
# Manual: it needs an account and a grant, so CI never runs it. The same
# journey `s3-store-it` makes against MinIO, against the one provider that
# needs a person at a browser — and the two consents the first run asks for are
# the only part of it that is not unattended.
#
# Set COFFRET_DRIVE_FOLDER_ID to the folder the Library's own folder is to be
# created in, and COFFRET_DRIVE_CLIENT_ID (with COFFRET_DRIVE_CLIENT_SECRET
# where the client was registered with one) to the desktop client to authorize
# as. Without the folder id the run says so and does nothing.
#
# The state it keeps is the point of it. Everything lives under
# .tmp/drive-round-trip/, so the second run opens the Libraries the first one
# made: it needs no consent, adds another batch of files, and commits the next
# head — which is what says an existing Library still works, not just that one
# can be created. Nothing on the account is trashed either way: the app folder
# is made once and reused, and removing it is the account owner's to do.
.PHONY: drive-round-trip-it
drive-round-trip-it:
	./scripts/drive-round-trip-it.sh

## fixtures: generate a synthetic benchmark library (OUT, PHOTOS, PAGES override defaults)
.PHONY: fixtures
fixtures:
	cd backend && cargo run --release -p coffret-fixtures -- --out ../$(OUT) --photos $(PHOTOS) --pages $(PAGES)

## server: serve the Library named by LIBRARY (default main) at http://localhost:8787
#
# It asks for the Passphrase once and holds the derived keys for as long as it
# runs: one process is one unlock. Which Libraries it can see is
# COFFRET_STATE_DIR's answer, so pointing it at what another run built is a
# matter of setting that — which is why this one target does not `cd` anywhere.
# Every other target here runs from `backend/`, and a relative COFFRET_STATE_DIR
# would then mean a directory under it rather than the one that was typed.
#
#   COFFRET_STATE_DIR=.tmp/drive-round-trip/state make server LIBRARY=second
.PHONY: server
server:
	cargo run --release --manifest-path backend/Cargo.toml -p coffret-server -- --library $(LIBRARY)

## web: run the frontend dev server at http://localhost:5173 (proxies /api to the coffret server)
.PHONY: web
web:
	cd frontend && pnpm --filter @coffret/web dev

## deps: assert the crate boundaries the layering rests on still hold
#
# Three things, all of which a compiler happily accepts and none of which anyone
# notices going wrong: coffret-model growing a dependency, one gateway reaching
# into another instead of meeting at the port, and an app binary reaching past
# coffret-device for the domain.
#
# The model's list is short and deliberately not empty: NFC is an invariant of
# `EntryPath` (spec: EP-1), which takes the Unicode composition tables, and
# `tinyvec` is what those pull in. Anything else appearing in that tree is a
# domain type reaching for a library, which is what this catches.
MODEL_DEPS := unicode-normalization tinyvec tinyvec_macros

# The two shells over coffret-device — the command line and the explorer's
# server — plus coffret-shell, which both of them start through, and what none of
# the three may name directly. Every flow a shell drives is a call on
# coffret-device, so either shell can be replaced without a flow moving with it.
# A gateway or a use case named here would be a decision the other shell then has
# to make again, and differently. The remaining crates under `apps/` are tools
# rather than shells — the fixture generator draws images, the interop harness
# writes the format directly — and are deliberately not held to this.
#
# Direct dependencies only — `--depth 1` — because reaching the domain *through*
# coffret-device is the arrangement rather than the mistake. coffret-shell is on
# the list for that same reason inverted: it is the one other crate a binary
# reaches the domain through, so depth 1 on the binaries alone would not see past
# it. Dev-dependencies are outside `--edges normal` and so outside this: a case
# may build a Library out of the use case's in-memory adapters, which is not
# something the binary ships.
APPS := coffret-cli coffret-server coffret-shell
APP_FORBIDDEN := coffret-usecase coffret-sqlite-index google-drive-store s3-store

.PHONY: deps
deps:
	@cd backend && allowed=$$(echo "$(MODEL_DEPS)" | tr ' ' '|'); \
	extra=$$(cargo tree --quiet -p coffret-model --edges normal --prefix none \
		| tail -n +2 | awk '{print $$1}' | sort -u | grep -Ev "^($$allowed)$$"); \
	if [ -n "$$extra" ]; then \
		echo "coffret-model depends on nothing but $(MODEL_DEPS), found:"; \
		echo "$$extra"; \
		exit 1; \
	fi
	@cd backend && gateways="coffret-sqlite-index google-drive-store s3-store"; \
	for one in $$gateways; do \
		for other in $$gateways; do \
			[ "$$one" = "$$other" ] && continue; \
			if cargo tree --quiet -p $$one --edges normal | grep -q " $$other v"; then \
				echo "$$one must not depend on $$other: a gateway meets the rest of the backend at a port, not at another gateway"; \
				exit 1; \
			fi; \
		done; \
	done
	@cd backend && for app in $(APPS); do \
		direct=$$(cargo tree --quiet -p $$app --edges normal --prefix none --depth 1 \
			| tail -n +2 | awk '{print $$1}' | sort -u); \
		for one in $(APP_FORBIDDEN); do \
			if echo "$$direct" | grep -qx "$$one"; then \
				echo "$$app must reach the domain through coffret-device, and depends on $$one directly"; \
				exit 1; \
			fi; \
		done; \
	done

## check: full pre-PR gate — deps + interop + backend fmt/build/test/clippy + frontend build/typecheck/test/lint
.PHONY: check
check: deps interop
	cd backend && cargo fmt --all -- --check && cargo build && cargo test && cargo clippy --all-targets -- -D warnings
	cd frontend && pnpm -r build && pnpm -r typecheck && pnpm -r test && pnpm -r lint
