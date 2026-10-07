---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make shell-lint && bash .github/scripts/release-summary.test.sh && test -f .github/workflows/create-release-pr.yml && test -f .github/workflows/validate-release-pr.yml && test -f .github/workflows/release.yml && test -f .github/workflows/bundle.yml && test -f docs/guides/release.md && test -f docs/guides/install/macos.md && test -f docs/guides/install/ubuntu.md && ! grep -rn "softprops/\|tauri-apps/tauri-action" .github/workflows'
assignee: null
branch: task/1007-0520-cut-releases-through-a-release-pr-and-attach-the-desktop-bundles
created_at: 2026-10-07T05:19:00Z
updated_at: 2026-10-07T05:39:08Z
---

# ci: cut releases through a release PR and attach the desktop bundles

## Overview

Coffret has no release process: the workspace version is `0.1.0`, there are no
tags, and the desktop app (`coffret-desktop`, built by `make desktop-build`)
reaches nobody who does not build it from a checkout. This change adds the
release flow of a sibling project, `x7c1/delta` (https://github.com/x7c1/delta),
adapted to this repository. Read its files from a local clone beside this
repository (`../delta`) when there is one, or from GitHub.

**1. Release PR.** Port delta's `.github/workflows/create-release-pr.yml`,
`.github/workflows/validate-release-pr.yml` and the scripts they call under
`.github/scripts/` (`create-release-pr.sh`, `generate-changelog.sh`,
`release-summary.sh`, `release-pr-lookup.sh`, `update-release-pr-links.sh`,
`validate-release-pr-title.sh`, `validate-release-summary.sh`, and
`release-summary.test.sh`). The version lives only in `backend/Cargo.toml`'s
`[workspace.package].version`, as in delta; the frontend `package.json`
versions are not bumped. Adapt names (crate `coffret-desktop` wherever delta
reads `delta-server`'s version, artifact names `coffret-*`) and reformat every
script to this repository's `make shell-lint` (shellcheck, shfmt with two-space
indent). Run `release-summary.test.sh` from the CI job that already runs
`make shell-lint`.

When `RELEASE_PAT` is not set, `create-release-pr` must finish green with a
notice saying the secret is missing and no PR was made, instead of failing on
every push to `main`: the secret is created by a person (see Before merge), and
this change can merge before that.

**2. The first version.** No tag exists yet. Delta's script bumps from the last
tag (default `0.0.1`), and its release trigger looks for a version change
between `HEAD~1` and `HEAD`, so a release PR for the version already in
`Cargo.toml` would have nothing to commit. Lower the workspace version to
`0.0.0` in this change (with `Cargo.lock`) and make the script's default for a
repository with no tag `0.1.0`, so the first release PR is "Release v0.1.0", a
minor bump from v0.0.0 that the title validator accepts. Nothing reads the
version but `env!("CARGO_PKG_VERSION")` in two tests; confirm that still holds.

**3. Release.** Port `release.yml` (runs on `workflow_run` of `CI` on `main`,
only on success) and `scripts/check-version-change.sh`: when the workspace
version changed, create the tag and the GitHub Release whose body is the merged
release PR's summary, fix the PR's compare links, then build the bundles into
that Release. Use the `gh` CLI for the tag, the Release and the uploads, not
`softprops/action-gh-release`: this repository avoids third-party actions
(see the `cargo-deny` comment in `.github/workflows/ci.yml`); first-party
`actions/*` are fine.

**4. Bundles.** Port `bundle.yml` as a reusable workflow (called by
`release.yml` with the release id or tag) that also runs on pull requests
touching the desktop crate, the frontend or the workflow itself, and on
`workflow_dispatch`, uploading artifacts there. Build with `cargo tauri build`
the way `scripts/desktop.sh` does (`--features embed-web` after the explorer is
built; the project is `backend/crates/apps/coffret-desktop`), not
`tauri-apps/tauri-action`. Never set `TAURI_CONFIG` there: that override makes
the development app (`io.github.x7c1.coffret.dev`), and a release is the
installed app (`io.github.x7c1.coffret`). Matrix: `macos-14` arm64 `.dmg`, and
`ubuntu-22.04` x86_64 `.deb` (the deb's name follows `tauri.linux.conf.json`).
Install the toolchain with `rustup show` (the repository pins it in
`rust-toolchain.toml`) and the Linux packages the backend job in `ci.yml`
lists.

**5. Docs.** Add `docs/guides/release.md` (how a release is cut: what the
release PR is, the summary to write, what merging it does, how to re-run a
failed release) and `docs/guides/install/{README,macos,ubuntu}.md` (the
unsigned app: on macOS the quarantine attribute or Privacy & Security's
"Open Anyway"; on Ubuntu `sudo apt install ./<deb>`; where the app keeps its
state, which is the binaries' default state directory as
`docs/guides/environments.md` says). Leave out what is delta's own (its
terminal, tmux). Link both from the README's documentation list.

Out of scope: signing and notarization, Windows, Intel macOS, publishing
anywhere but GitHub Releases, changing `make desktop*`.

## Acceptance criteria

### Automated (pipeline-verified)
- [x] The four workflows and the release scripts exist, use no `softprops/*` or `tauri-apps/tauri-action`, and pass `make shell-lint`
- [x] `release-summary.test.sh` passes and runs in CI
- [x] The workspace version is `0.0.0` and the release PR script defaults to `0.1.0` when there is no tag
- [x] `docs/guides/release.md` and `docs/guides/install/{macos,ubuntu}.md` exist and the README links them
- [x] `make check` passes

### Before merge (verified outside the check command)
- [x] The PR's own `bundle` run builds both the `.dmg` and the `.deb` and uploads them as artifacts — an agent checks the run with `gh` and reports the artifact names
- [ ] The `create-release-pr` job, run with `workflow_dispatch` or on the branch, ends green with the missing-secret notice while `RELEASE_PAT` is absent — an agent checks, or explains why it cannot run before merge
- [ ] Needs a person, before the first release rather than before this merge: create a fine-grained token for this repository (Contents and Pull requests write, and Workflows write if a release PR may touch workflow files) as the secret `RELEASE_PAT`; allow GitHub Actions to create and approve pull requests in the repository's Actions settings; optionally add `validate-title` to the required checks (it passes on PRs that are not release PRs)
