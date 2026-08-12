# Coffret — unified entry point.
#
# Run `make help` (the default) for the full target list. Every target wraps
# the per-part cargo/pnpm commands so the repo has one place to run things
# from: the repo root.

.DEFAULT_GOAL := help

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

## check: full pre-PR gate — backend fmt/build/test/clippy + frontend build/typecheck/test/lint
.PHONY: check
check:
	cd backend && cargo fmt --all -- --check && cargo build && cargo test && cargo clippy --all-targets -- -D warnings
	cd frontend && pnpm -r build && pnpm -r typecheck && pnpm -r test && pnpm -r lint
