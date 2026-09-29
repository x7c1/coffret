---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 2
retries_remaining: 1
check_command: 'make check && grep -qE "^check:.*shell-lint" Makefile && grep -q "make .*shell-lint" .github/workflows/ci.yml && make shell-lint && ! git grep -qE "CLIENT_ID they were made under|is the only one their folders|with their own\"" -- scripts/'
assignee: null
branch: task/0929-0605-lint-and-format-every-shell-script
created_at: 2026-09-29T13:15:38Z
updated_at: 2026-09-29T13:37:35Z
---

# build: lint and format every shell script with shellcheck and shfmt

## Overview

The repository carries 3,332 lines of bash under `scripts/` (the real-Drive
targets `drive-round-trip-it.sh` 790, `drive-index-layout-it.sh` 995,
`drive-it-reset.sh` 482, `e2e-it.sh` 762, `s3-store-it.sh` 100, and the spec
checks `spec-rule-ids.sh`, `spec-rule-ids-test.sh`, `spec-citations.sh`), and
nothing lints or formats any of it. The real-Drive targets only run when a
person gives OAuth consent, so a quoting bug, an unset variable or a dead
branch in them is found at the next hardware run at the earliest.

Introduce **shellcheck** and **shfmt** over every tracked shell script:

- A `shell-lint` Makefile target that runs `shellcheck` over every tracked shell
  script (found through `git ls-files`, by extension and by bash shebang, so a
  new script is covered without editing the target) and `shfmt -d` over the
  same set. Add it to `check`'s prerequisites, and run it in CI's `spec` job
  (the job that needs "git and a shell and nothing else").
- **Pin both tools**, following the repository's existing pinning conventions
  (`rust-toolchain.toml`, `packageManager`, `CARGO_DENY_VERSION` installed
  without third-party actions, images by digest). `make check` runs on a
  developer's host toolchain without sudo and without a container runtime, and
  must keep doing so: fetch pinned release binaries into a gitignored cache
  (e.g. under `.tmp/` or `backend/target/`), verified by a SHA-256 recorded in
  the repository, and have CI use the same pinned versions and the same
  mechanism rather than whatever the runner ships. Say in a comment why the
  versions are pinned and where to bump them.
- Choose shfmt options that match the scripts' existing style (indent width,
  `case` indentation, binary-operator placement) so the formatting diff is as
  small as the tool allows; record them once (an `.editorconfig` section or the
  flags in one place).
- Fix every shellcheck finding. Where a finding is a deliberate construct,
  disable it at that line with a directive and a one-line reason — never file-
  or repo-wide unless the reason genuinely applies everywhere, and then say so
  in `.shellcheckrc`.
- Also correct `scripts/drive-it-reset.sh`: its messages around lines 360–365,
  395–398 and 475–477 say a Library's folders answer only to the
  `COFFRET_DRIVE_CLIENT_ID` they were made under. What a grant reaches is decided
  by the account and the Cloud **project** the OAuth client belongs to; another
  client of the same project reaches the same folders. What is bound to a client
  is the grant's refresh (a refresh token is refreshed only through the client
  that obtained it). Reword those messages to say that: the reset has to run
  under a client of the project those Libraries were made under, with a grant
  obtained through that client.

Behaviour of the scripts must not change beyond what a shellcheck fix requires;
a fix that changes behaviour (e.g. a word-splitting bug) is named in the PR.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make shell-lint` runs shellcheck and `shfmt -d` over every tracked shell
      script and passes (appended to `check_command`).
- [x] `check` depends on `shell-lint`, and CI's `spec` job runs it (both greps
      appended to `check_command`).
- [x] shellcheck and shfmt are pinned by version and verified by SHA-256, and
      `make shell-lint` fetches them without sudo or a container runtime.
- [x] `drive-it-reset.sh` no longer says the folders answer only to the client
      they were made under (gate appended to `check_command`).

### Manual / on-hardware (verified by a human before merge)

- [ ] CI's `spec`, `s3-store` and `e2e` jobs are green on the PR (the last two
      run reformatted scripts).
- [ ] `make drive-round-trip-it` and `make drive-it-list` still run against real
      Drive after the reformat.
