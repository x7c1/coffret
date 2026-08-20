# Coffret — unified entry point.
#
# Run `make help` (the default) for the full target list. Every target wraps
# the per-part cargo/pnpm commands so the repo has one place to run things
# from: the repo root.

.DEFAULT_GOAL := help

# Parameters for the viewer-spike targets below.
OUT ?= .tmp/fixtures
PHOTOS ?= 3000
PAGES ?= 300
LIBRARY ?= .tmp/fixtures

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

## s3-store-it: run the ObjectStore conformance suite against MinIO in Docker
#
# Separate from `check` because it is the one target that needs a container
# runtime; CI runs it as its own job. The script starts MinIO, runs the suite
# against it, and removes the container again, so the target leaves nothing
# behind either way.
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
.PHONY: drive-authorize
drive-authorize:
	cd backend && cargo run -p google-drive-store --example authorize

## drive-store-it: run the same conformance suite against a real Google Drive folder
#
# Manual: it needs an account and a grant, so CI never runs it. Authorize first,
# then set COFFRET_DRIVE_FOLDER_ID alongside the variables above. Without them
# the cases report themselves skipped. The grant reaches only what coffret
# created, so that folder is `root` on a first run — never one copied out of the
# Drive web interface.
.PHONY: drive-store-it
drive-store-it:
	cd backend && cargo test -p google-drive-store --test conformance -- --nocapture

# --- Viewer performance spike -------------------------------------------------

## fixtures: generate a synthetic benchmark library (OUT, PHOTOS, PAGES override defaults)
.PHONY: fixtures
fixtures:
	cd backend && cargo run --release -p coffret-fixtures -- --out ../$(OUT) --photos $(PHOTOS) --pages $(PAGES)

## server: run the viewer spike server against LIBRARY (default .tmp/fixtures)
.PHONY: server
server:
	cd backend && cargo run --release -p coffret-server -- --library ../$(LIBRARY) --thumbs ../.tmp/thumbs

## web: run the frontend dev server at http://localhost:5173 (proxies /api to the spike server)
.PHONY: web
web:
	cd frontend && pnpm --filter @coffret/web dev

## deps: assert the crate boundaries the layering rests on still hold
#
# Two things, both of which a compiler happily accepts and neither of which
# anyone notices going wrong: coffret-model growing a dependency, and one
# gateway reaching into another instead of meeting at the port.
.PHONY: deps
deps:
	@cd backend && extra=$$(cargo tree --quiet -p coffret-model --edges normal | tail -n +2); \
	if [ -n "$$extra" ]; then \
		echo "coffret-model must have zero third-party dependencies, found:"; \
		echo "$$extra"; \
		exit 1; \
	fi
	@cd backend && for pair in "s3-store google-drive-store" "google-drive-store s3-store"; do \
		set -- $$pair; \
		if cargo tree --quiet -p $$1 --edges normal | grep -q " $$2 v"; then \
			echo "$$1 must not depend on $$2: gateways meet at the ObjectStore port, not each other"; \
			exit 1; \
		fi; \
	done

## check: full pre-PR gate — deps + interop + backend fmt/build/test/clippy + frontend build/typecheck/test/lint
.PHONY: check
check: deps interop
	cd backend && cargo fmt --all -- --check && cargo build && cargo test && cargo clippy --all-targets -- -D warnings
	cd frontend && pnpm -r build && pnpm -r typecheck && pnpm -r test && pnpm -r lint
