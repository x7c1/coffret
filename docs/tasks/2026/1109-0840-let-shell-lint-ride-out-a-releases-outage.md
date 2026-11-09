---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -rq -- "--retry-all-errors" scripts/shell-lint.sh'
assignee: null
branch: task/1109-0840-let-shell-lint-ride-out-a-releases-outage
created_at: 2026-11-09T08:40:17Z
updated_at: 2026-11-09T08:48:08Z
---

# ci: let shell-lint ride out a passing GitHub Releases outage

## Overview

`scripts/shell-lint.sh` (run by `make shell-lint`, by `make check`, and by
the `spec` job in `.github/workflows/ci.yml`) downloads the pinned shellcheck
and shfmt release binaries from GitHub Releases in `fetch_verified`, with
`curl -fsSL --retry 3`. curl's default retry waits 1, 2 and 4 seconds, so a
download gives up about seven seconds after the first failure. GitHub
Releases sometimes answers `504 Gateway Timeout` for longer than that: the
`spec` job of a pull request failed this way on the shfmt download, and so did
a local `make check`, both clearing on their own minutes later. A check that
fails for a reason unrelated to the change, and passes on a re-run, is a
flaky check, and the cause should be fixed rather than re-run away.

Make the download ride out a transient outage of a minute or two:

- retry on every transient failure, not only the ones curl counts as
  transient by default (`--retry-all-errors`, together with `-f` so an HTTP
  error is a failure), with more attempts and a total budget of roughly two
  minutes (`--retry-max-time`), and a connect timeout so a stalled connection
  does not use up the budget on its own;
- keep the downloaded file verified against its pinned SHA-256 as today, so a
  retried download is never trusted without the check;
- keep the existing message when the download still fails after the budget,
  and say how long it tried.

The `spec` job has `timeout-minutes: 5`; the new budget for both downloads
must stay well inside it (state the worst case in a comment next to the job
or the curl call).

Out of scope: caching the tools across CI runs.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `fetch_verified` in `scripts/shell-lint.sh` downloads with
      `--retry-all-errors` and a bounded total retry time
      (`grep` for `--retry-all-errors` appended to `check_command`), and
      `make shell-lint` (part of `make check`) still passes, which shows the
      verified download and the lint still work.
