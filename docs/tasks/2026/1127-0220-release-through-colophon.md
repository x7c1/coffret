---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: null
max_refine_rounds: 3
retries_remaining: 1
check_command: make shell-lint && actionlint && make set-workspace-version-test
assignee: null
branch: task/1127-0220-release-through-colophon
created_at: '2026-11-26T17:20:14Z'
updated_at: '2026-11-26T17:41:07Z'
---

# ci: release through the shared colophon workflows

## Overview

Coffret's release automation is a private copy of what the reusable
workflows in [x7c1/colophon](https://github.com/x7c1/colophon) now provide:
`create-release-pr.yml`, `validate-release-pr.yml`, `release.yml`, and the
scripts under `.github/scripts/`. Replace the copy with calls to colophon
v0.3.0, so that release mechanics are maintained in one place. Read colophon's
`docs/usage.md` at v0.3.0 first; the callers below follow its examples.

Pin every colophon call to the v0.3.0 commit
`a4d0af8d30a36d9978d393e28b540bdb3c641c6f` with a `# v0.3.0` comment, as the
repository pins other actions.

With colophon, the release PR opens as a draft and every refresh turns it back
into one. Marking it ready for review is the decision to release. While it is
a draft, validate passes without judging it, and coffret's CI skips it.

### Release PR (`release-pr.yml` replaces `create-release-pr.yml`)

- Call `x7c1/colophon/.github/workflows/release-pr.yml` on the same triggers as
  today (`push` to `main`, `pull_request` `edited`, `workflow_dispatch`) with
  `version-scheme: semver`.
- Open the PR as the GitHub App: `app-client-id: ${{ vars.RELEASE_APP_CLIENT_ID }}`
  and the `app-private-key` secret from `secrets.RELEASE_APP_PRIVATE_KEY`.
  `RELEASE_PAT` is no longer used; remove every reference to it.
- Write the version with colophon's `command` writer. The command runs a new
  `scripts/set-workspace-version.sh`, which sets the workspace version in
  `backend/Cargo.toml` (`[workspace.package]`) and keeps `backend/Cargo.lock`
  consistent with it, exactly as `cargo set-version --workspace` plus the lock
  update do today. The command runs inside colophon's job on a fresh
  `ubuntu-latest` runner, with the repository checked out at its root and
  nothing installed beyond the runner image; there is no step before it to
  install a toolchain or a cargo subcommand. So the script must work there: if
  it needs the toolchain named by `rust-toolchain.toml` it installs it itself
  (`rustup` does not install it on first use), and it must not depend on
  `cargo-edit` being present. Choose the approach that keeps the lock file
  correct and the refresh reasonably fast, and say in the script's header why.
- Add `make set-workspace-version-test`, which runs the script against a
  throwaway copy of the workspace (like `scripts/check-version-change-test.sh`)
  and checks that the workspace version, the version every member crate reads
  through `scripts/workspace-version.sh`, and the `Cargo.lock` entries of the
  workspace crates all become the new version, and that `cargo metadata
  --locked --offline` then succeeds.

### Validate (`validate-release-pr.yml`)

- Call `x7c1/colophon/.github/workflows/validate-release-pr.yml` with
  `version-scheme: semver`, on `pull_request` types `[opened, edited,
  synchronize, reopened, ready_for_review]`. Name the calling job `validate`,
  so the check reads `validate / release-pr`.

### Release (`release.yml`)

- Keep the `workflow_run` trigger on CI for `main`. Replace the `release` job
  with colophon's `release-plan.yml` (job `plan`) and `release-publish.yml`
  (job `publish`, with `tag`, `sha` and `pr-number` from `plan`).
- Keep the desktop bundles after the Release exists, so a bundle failure never
  blocks the tag: the `bundles` job needs `[plan, publish]`, runs when
  `needs.plan.outputs.released == 'true'`, and calls `./.github/workflows/bundle.yml`
  with `ref` and `release_tag` set to `needs.plan.outputs.tag`. Do not pass
  `artifact-name` to `release-publish.yml`.
- Check how colophon decides a release (the merged PR labelled `release` whose
  squash produced the commit) against what `scripts/check-version-change.sh`
  did (the workspace version went up), and keep anything a caller still needs.
  Remove what nothing uses any more: `scripts/check-version-change.sh`,
  `scripts/check-version-change-test.sh`, its Makefile target and its place in
  `make check` and CI, and the `.github/scripts/` files colophon replaces
  (including `release-summary.test.sh` and its CI step). Keep
  `scripts/workspace-version.sh`, which the desktop build uses.

### CI on a draft release PR

- In `ci.yml` and `bundle.yml`, add `ready_for_review` to the `pull_request`
  types (keeping the existing `branches` filter and the default types
  `opened, synchronize, reopened`). Skip every job except the aggregate jobs
  (`all-green`, `bundle-green`) on a draft PR whose branch starts with
  `release/`, with the condition colophon's usage shows:
  `!(github.event_name == 'pull_request' && github.event.pull_request.draft && startsWith(github.head_ref, 'release/'))`.
  Where a job already has an `if:`, combine the two. The aggregate jobs must
  keep passing when the jobs they need are skipped; confirm their `needs` and
  conditions still hold (for example, `bundle`'s `needs.changes.result` test
  when `changes` itself is skipped).

### Docs

- Rewrite `docs/guides/release.md` for the new flow: colophon does the work;
  the release PR opens as a draft; mark it ready for review when you decide to
  release, which runs CI and validate; a refresh turns it back into a draft;
  the App setup (`RELEASE_APP_CLIENT_ID` variable, `RELEASE_APP_PRIVATE_KEY`
  secret, the App installed on the repository) replaces the `RELEASE_PAT`
  setup; the required checks are `all-green`, `bundle-green` and
  `validate / release-pr`. Point to colophon's `docs/usage.md` instead of
  repeating it. Update any other document or comment that names a removed file
  or the PAT.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make set-workspace-version-test` passes: the script sets the workspace version and `Cargo.lock` consistently, and `cargo metadata --locked --offline` succeeds afterwards
- [x] `actionlint` passes on every workflow
- [x] `make shell-lint` passes on every shell script, including the new one

### Before merge (verified outside the check command)

- [x] No file in the repository references `RELEASE_PAT`, `create-release-pr.sh`, `check-version-change`, or a removed `.github/scripts/` file, except history in `CHANGELOG`-like documents if any
- [x] Every colophon call is pinned to `a4d0af8d30a36d9978d393e28b540bdb3c641c6f` with a `# v0.3.0` comment, and passes the inputs and secrets its v0.3.0 `docs/usage.md` documents
- [x] In `ci.yml` and `bundle.yml`, every job other than `all-green` and `bundle-green` carries the draft-release skip, both `pull_request` triggers list `ready_for_review`, and the aggregate jobs still pass when the jobs they need are skipped (read the conditions)
- [x] `docs/guides/release.md` describes the draft flow, the App setup and the required checks, with no remaining description of the PAT or the removed scripts
