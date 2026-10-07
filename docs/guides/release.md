# Release

How a Coffret release is cut. Releases are driven by a single rolling pull
request that a bot keeps in sync with `main`; cutting a release is merging
that PR.

## Overview

A workflow opens and updates one rolling release PR against `main`; merging it
triggers the `Release` workflow, which creates the matching `vX.Y.Z` tag and a
GitHub Release. The Release body is a summary written by hand in the release
PR; the generated per-commit changelog stays on the PR. Once the Release
exists, the desktop bundles are built and attached to it (see
[Desktop bundles](#desktop-bundles)).

The version lives in one place: `[workspace.package].version` in
`backend/Cargo.toml`, which every crate of the backend workspace inherits. The
`version` fields of the frontend's `package.json` files are not release
versions and are not bumped.

## Normal flow (patch bump)

1. The `Create Release PR` workflow opens or refreshes a single open PR
   titled `Release vX.Y.Z` on every push to `main`. By default `X.Y.Z` is
   the last tag patch-bumped (e.g. `v0.1.0` → `Release v0.1.1`). The PR's
   one commit sets the workspace version to `X.Y.Z` in `backend/Cargo.toml`
   and `backend/Cargo.lock`.
2. The PR body has two parts: a summary region you write by hand, and the
   changelog since the previous tag below it (auto-generated from `git log`
   by `.github/scripts/generate-changelog.sh`). See
   [Release summary](#release-summary).
3. Write the summary. `Validate Release PR` fails while it is unwritten, which
   blocks the merge once `validate-title` is a required check (see
   [Release automation setup](#release-automation-setup)).
4. Merge the PR when you want to cut the release. The `Release` workflow
   then runs after CI completes on `main`, creates the `vX.Y.Z` tag, and
   publishes a GitHub Release carrying that summary plus links back to the
   release PR and the compare view.

## The first release

No tag exists before the first release, and the workspace version is `0.0.0`
until then. With no tag, the release PR defaults to `Release v0.1.0`, a minor
bump from the implicit `v0.0.0` baseline that the title validator accepts.
Its changelog lists the last 20 commits, since there is no tag to start from.
Merging it cuts `v0.1.0`. From then on the default is the last tag
patch-bumped.

## Release summary

The release PR body is split in two by a marker line:

```markdown
## Summary

<!-- release-summary:todo -->
_Write the release summary here. It becomes the body of the GitHub Release.
Delete the marker comment above once written._

<!-- changelog:auto -->

## Features

- feat: ...
```

- Everything **above** `<!-- changelog:auto -->` is yours. The workflow
  carries it through verbatim every time it regenerates the body, so it
  survives the force-pushes that rebuild the release branch on each push to
  `main`.
- Everything from the marker down is regenerated from `git log` on every push
  to `main`; edits made there are overwritten.

Write the summary for someone deciding whether to install the release: what
they can now do, what changed in what they already did, and anything they have
to do themselves when updating. The per-commit list below the marker already
says what changed commit by commit.

Because the summary is the body of the GitHub Release, the release is gated on
it: `Validate Release PR` fails while the summary is empty or still carries the
`<!-- release-summary:todo -->` sentinel. That workflow also runs on body
edits, so saving the summary re-runs the check and turns it green without any
further push. If a release nonetheless reaches the `Release` workflow without
a summary, that workflow fails before creating the tag rather than publishing
a Release without one.

## Promoting to minor or major

To cut a minor or major release, **edit the PR title only**:

- `Release v0.1.1` → `Release v0.2.0` (minor)
- `Release v0.1.1` → `Release v1.0.0` (major)

Saving the title edit triggers the workflow immediately; the next push to
`main` does too. Body-only edits and no-op title saves are ignored. When the
workflow runs, it:

1. Reads the new title and extracts the target version.
2. Runs `cargo set-version --workspace <version>` to update
   `backend/Cargo.toml` and `backend/Cargo.lock`.
3. Force-pushes the result to the existing release branch.
4. Updates the PR body's compare link.

The title is the single source of truth for the next version; you never
edit `Cargo.toml` by hand.

## Allowed title transitions

`.github/scripts/validate-release-pr-title.sh` runs on every release PR edit
and enforces a strict semver progression against the last tag. Only
single-step bumps with the lower components reset are allowed:

| Last tag | PR title | Result |
|---|---|---|
| v0.3.10 | `Release v0.3.11` | ✅ patch |
| v0.3.10 | `Release v0.4.0` | ✅ minor |
| v0.3.10 | `Release v1.0.0` | ✅ major |
| v0.3.10 | `Release v0.3.9` | ❌ downgrade |
| v0.3.10 | `Release v0.4.1` | ❌ minor bump with non-zero patch |
| v0.3.10 | `Release v0.5.0` | ❌ minor skip |
| v0.3.10 | `Release v2.0.0` | ❌ major skip |

The check is `validate-title`. It passes on every PR that is not a release
PR, so it can be listed in the repository's required checks; until it is, a
failing title is reported but does not block the merge.

## Branch naming

The release branch is named `release/since-<UTC %Y-%m-%d-%H%M>` (e.g.
`release/since-2026-10-07-1431`), the time it was opened. It carries no
version, so promoting the title keeps the same branch for the lifetime of
the PR.

## What merging does

When CI finishes green on the commit that merged the release PR, the `Release`
workflow:

1. Checks out that commit — the one CI ran on, not whatever is newest on
   `main` by then — and runs `scripts/check-version-change.sh`, which compares
   the workspace version with the one in the commit before it. Only a raised
   version is a release; an unchanged or lowered one ends the run there.
2. Finds the merged release PR titled `Release vX.Y.Z` and reads its summary.
   Without one it fails here, before anything is created.
3. Creates the tag `vX.Y.Z` at that commit and the GitHub Release, with the
   `gh` CLI. A Release that already exists is left as it is.
4. Rewrites the merged PR's compare link from the deleted release branch to
   the tag (`.github/scripts/update-release-pr-links.sh`).
5. Builds the desktop bundles and attaches them to the Release (the
   `bundles` job; see [Desktop bundles](#desktop-bundles)).

## Desktop bundles

Every Release carries unsigned bundles of the desktop shell
(`backend/crates/apps/coffret-desktop`), built by
`.github/workflows/bundle.yml`:

| File | Platform |
|---|---|
| `Coffret_<version>_aarch64.dmg` | macOS, Apple silicon |
| `coffret-desktop_<version>_amd64.deb` | Ubuntu and other Debian-based distributions, x86_64 |

The version in each file name is the workspace version Tauri reads from the
shell crate, so it matches the tag by construction. The `.deb` takes its name
from `tauri.linux.conf.json`'s `productName`. The bundles are built the way
`make desktop-build` builds them — the explorer first, then
`cargo tauri build --features embed-web` — and, like that target, without
`TAURI_CONFIG`: they are the installed app (`io.github.x7c1.coffret`), not
the development one. They are neither signed nor notarized; what that means
for someone installing them is in [the install guide](install/README.md).

- **Order.** The `bundles` job of the `Release` workflow runs only after the
  `release` job has created the tag and the Release, and uploads to that
  Release. A bundle failure therefore never blocks the tag or the Release; it
  shows up as a failed `bundles` job.
- **Pull requests.** A pull request that changes how the bundle is made —
  the desktop app's crate, `scripts/desktop.sh`, `bundle.yml`, or the
  lockfile a tauri bump lands in (the `paths` filter in `bundle.yml`) — runs
  the same builds and uploads each platform's bundle as a workflow artifact
  (`coffret-macos-aarch64`, `coffret-linux-x86_64`), so a broken bundle is
  caught before a release depends on it, and reviewers can download and try
  it. A change to the explorer or the server alone does not run them; the
  next release bundles it. To try one, run the workflow by hand (below).
- **Manual runs.** The `Bundle` workflow can be run by hand
  (`workflow_dispatch`) with any branch, tag or SHA as `ref`; it uploads
  workflow artifacts and touches no Release.

## Re-running a failed release

- **`Create Release PR` failed.** Re-run it from the Actions tab. The script
  is idempotent: it rebuilds the release branch from `origin/main` on every
  run.
- **`Release` failed because the summary was missing.** No tag was created.
  Write the summary into the merged release PR's body (a merged PR's
  description is still editable) and re-run the failed run from the Actions
  tab. A re-run checks out the same commit as the original run, so it still
  sees the version raised even after other commits have landed on `main`.
- **`Release` failed after the Release was created** (the link rewrite, say).
  Re-run the failed jobs: the existing Release is left as it is and the steps
  after it run again.
- **A bundle failed.** Open the `Release` run and use "Re-run failed jobs":
  only the failed matrix entries of `bundles` run again, against the same tag
  and Release, and an upload replaces a file of the same name. To build a tag
  without touching its Release, run the `Bundle` workflow by hand with the
  tag as `ref`.
- **CI was red on `main`.** The `Release` workflow runs only after a green CI,
  so nothing was tagged. CI runs again on the commit that fixes it, but by
  then the version change is no longer in the newest commit; re-run the
  failed CI run of the release commit instead. When that re-run turns green,
  its completion starts a new `Release` run. Re-running the `Release` run the
  red CI started does not help: a re-run replays the original event, which
  still says CI failed.
- **Abandoning a release PR.** Close it and delete its branch; the next push
  to `main` opens a fresh one with the default title.

## Workflows involved

- `.github/workflows/create-release-pr.yml` — opens or updates the release PR
  on every push to `main` (skipped when the push is itself the merge of a
  release PR, to avoid looping), when a release-labelled PR's title is edited,
  and on a manual run on `main`.
- `.github/workflows/validate-release-pr.yml` — on every release PR edit,
  enforces the allowed title transitions above and fails while the release
  summary is unwritten.
- `.github/workflows/release.yml` — when CI completes successfully on `main`,
  checks whether the workspace version was raised; if it was, reads the
  summary from the merged release PR, creates the tag and the GitHub Release,
  and calls `bundle.yml` to attach the desktop bundles.
- `.github/workflows/bundle.yml` — builds the desktop bundles on macOS (Apple
  silicon only) and Linux; attaches them to a Release when called from
  `release.yml`, and uploads them as workflow artifacts on pull requests and
  manual runs.

The scripts they call are under `.github/scripts/`, plus
`scripts/check-version-change.sh`. The summary helpers have an offline test,
`.github/scripts/release-summary.test.sh`, which CI runs beside
`make shell-lint`.

No workflow here uses a third-party action to make the tag, the Release or
the bundles: they are made with the `gh` CLI and `cargo tauri`, for the reason
the `cargo-deny` job in `.github/workflows/ci.yml` gives.

## Release automation setup

`Create Release PR` pushes the `release/since-…` branch and calls
`gh pr create` under a **user-scoped personal access token**, exposed to the
workflow as the repository secret **`RELEASE_PAT`**.

A user-scoped token is required because GitHub's recursion-prevention rule
suppresses `pull_request` workflow runs on PRs authored by
`github-actions[bot]`. With the default `GITHUB_TOKEN` the release PR would be
bot-authored, so `CI` and `Validate Release PR` would never run on it. Pushing
and opening the PR under a user's token makes the PR user-authored, which lets
the checks run normally.

**One-time setup, by a person with admin access to the repository:**

1. Create a fine-grained personal access token for this repository with
   **Contents: Read and write** and **Pull requests: Read and write** — and
   **Workflows: Read and write** if a release PR may carry changes to files
   under `.github/workflows/` — and add it as the Actions secret
   `RELEASE_PAT`.
2. In the repository's Actions settings, allow GitHub Actions to create and
   approve pull requests.
3. Optionally, add `validate-title` to the required checks of `main`.

Until `RELEASE_PAT` exists, `Create Release PR` ends green with a notice
saying the secret is missing and that no release PR was made. There is no
fallback to `GITHUB_TOKEN`, since a PR opened under it would never be checked.
Once the secret is set, run the workflow by hand on `main` to open the first
release PR without waiting for the next push.

The `Release` and `Bundle` workflows use the default `GITHUB_TOKEN`: they run
after a merge a person made, and no workflow here listens to tag pushes or
Release events, so nothing has to be triggered by what they create.
