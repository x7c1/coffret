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

## deps: assert the crates that must stay dependency-free still are
.PHONY: deps
deps:
	@cd backend && extra=$$(cargo tree --quiet -p coffret-model --edges normal | tail -n +2); \
	if [ -n "$$extra" ]; then \
		echo "coffret-model must have zero third-party dependencies, found:"; \
		echo "$$extra"; \
		exit 1; \
	fi

## check: full pre-PR gate — deps + interop + backend fmt/build/test/clippy + frontend build/typecheck/test/lint
.PHONY: check
check: deps interop
	cd backend && cargo fmt --all -- --check && cargo build && cargo test && cargo clippy --all-targets -- -D warnings
	cd frontend && pnpm -r build && pnpm -r typecheck && pnpm -r test && pnpm -r lint
