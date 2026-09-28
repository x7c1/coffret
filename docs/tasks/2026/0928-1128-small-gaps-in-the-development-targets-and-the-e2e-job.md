---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it && grep -qE "if: failure\(\)" .github/workflows/ci.yml && grep -qF "transcript.log" .github/workflows/ci.yml && make -n server PORT=9999 | grep -qF -- "--port 9999"'
assignee: null
branch: task/0928-1128-small-gaps-in-the-development-targets-and-the-e2e-job
created_at: 2026-09-28T11:28:07Z
updated_at: 2026-09-28T13:49:21Z
---

# chore: small gaps in the development targets, the e2e job and the CLI's setup tests

## Overview

Five small gaps in what a developer runs and what CI keeps, none of which changes what coffret does for a person using it.

1. **`make web` does not pick up gateway source edits.** `frontend/packages/apps/web/package.json` (around line 7) runs `pnpm --filter @coffret/api build && vite`, so the API package is built once at start and an edit under `frontend/packages/gateway/api/src/` is not seen until a restart (`Makefile`, around lines 448-449). In development only, resolve `@coffret/api` to its source through vite's `resolve.alias`, or run the package's build in watch mode alongside vite — whichever keeps the production build unchanged.
2. **`make server` cannot be given a port, and neither binary answers `--version`.** `Makefile` (around lines 434-435) starts the server without a way to pass `--port` (default 8787, `coffret-server/src/main.rs`, around lines 37-38), and vite reads `COFFRET_PORT` (`frontend/packages/apps/web/vite.config.ts`, around line 10). Accept `PORT` in `make server` and pass it to `make web` as `COFFRET_PORT` too. Add clap's `version` to both binaries' commands (`coffret-server/src/main.rs`, around lines 23-28; `coffret-cli/src/main.rs`, around lines 71-75); both mains already say in a comment that `--version` comes through the parse path, which is false today. The version is the workspace package's.
3. **A failed e2e run keeps only its screenshots.** `.github/workflows/ci.yml` (around lines 301-307) uploads `.tmp/e2e/screenshots/`; the Playwright traces (`retain-on-failure`, `playwright.config.ts` around lines 31 and 45, written under the run's work directory), the process logs, `transcript.log` and `last-command.log` (`scripts/e2e-it.sh`, around lines 42-45) are lost. Upload them too on failure.
4. **The e2e script's check right after an upload can lose a race.** `scripts/e2e-it.sh` (around lines 623-626) expects the uploaded file's state to be `added` immediately, but a sync can already have carried it in; the browser journey accepts either (`frontend/packages/apps/e2e/journeys/03-drop.spec.ts`, around lines 36-43). Accept `added` or `present` the same way. This is a flaky check, fixed at its cause.
5. **`init --drive` without `--parent` is not shown to refuse before asking for the Passphrase.** The `unparented` case in `coffret-cli/tests/setup.rs` (around lines 273-283, in `a_provider_has_to_be_named_and_only_one_of_them`) only checks exit status 1. Run it with no stdin and assert that stderr names `--parent` and that no Passphrase prompt appears, the way the test around line 906 does for its case.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The e2e job uploads Playwright traces and the run's logs on failure (grep gates on `ci.yml` for an `if: failure()` step and `transcript.log`)
- [x] The e2e script's post-upload check accepts `added` or `present`, and `make e2e-it` passes
- [x] The `unparented` setup case asserts that stderr names `--parent` and that no Passphrase was asked for (`make check`)
- [x] Both binaries answer `--version`, pinned by a test in each (`make check`)
- [x] `make server PORT=9999` passes `--port 9999` to the server (gate on `make -n server PORT=9999`)
- [x] `make check` passes

### Manual / on-hardware (verified by a human before merge)

- [ ] With `make web` running, an edit under `frontend/packages/gateway/api/src/` shows up without a restart, and `make web PORT=<n>` reaches a server started with `make server PORT=<n>`

## Last subagent report

Work phase returned `needs_review`: all five items are in and the full check_command exited 0 on its second run. The first run's `make e2e-it` exited 2 because `01-browse-and-read` failed on a Chromium `Page.captureScreenshot` protocol error at its first screenshot; that journey and the browser start-up are untouched by this change, and the second run passed without edits. Flagged because a flaky case is to be fixed at its cause, not passed by rerunning.

Operator decision: this change does not touch the failing journey, so the task continues; the screenshot flake is watched in this task's own check and in CI, and is fixed at its cause in a change of its own if it recurs.
