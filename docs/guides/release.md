# Release

How a Coffret release is cut. Releases are driven by a single rolling pull
request kept in sync with `main`; marking it ready for review is the decision
to release, and merging it cuts the release.

## Overview

The release mechanics are not in this repository: they are the reusable
workflows of [colophon](https://github.com/x7c1/colophon), pinned to v0.3.0,
which this repository calls from three small workflows of its own (see
[Workflows involved](#workflows-involved)). colophon's
[usage guide](https://github.com/x7c1/colophon/blob/v0.3.0/docs/usage.md) describes the flow those workflows run — the release
PR's title and version, its body with the summary above the
`<!-- changelog:auto -->` marker, the allowed title transitions, and what a
merge does — and this guide does not repeat it. What follows is what is
particular to Coffret.

The version lives in one place: `[workspace.package].version` in
`backend/Cargo.toml`, which every crate of the backend workspace inherits. The
`version` fields of the frontend's `package.json` files are not release
versions and are not bumped. Versions are semver (`Release vX.Y.Z`, tag
`vX.Y.Z`). Once the Release exists, the desktop bundles are built and
attached to it (see [Desktop bundles](#desktop-bundles)).

## Normal flow

1. Every push to `main` opens or refreshes one PR titled `Release vX.Y.Z` on a
   `release/...` branch, labelled `release`. By default `X.Y.Z` is the last
   tag patch-bumped; to cut a minor or major release, edit the PR title
   (`Release v0.1.1` → `Release v0.2.0`), and the refresh that edit starts
   keeps it. The PR's one commit sets the workspace version to `X.Y.Z` in
   `backend/Cargo.toml` and `backend/Cargo.lock` (see
   [Setting the version](#setting-the-version)).
2. **The PR opens as a draft.** An open release PR does not mean anyone has
   decided to release it. While it is a draft, `validate / release-pr` passes
   without judging it, and CI and the bundles skip it (see
   [CI on the draft](#ci-on-the-draft)).
3. Write the release summary in the PR body, above the `<!-- changelog:auto -->`
   marker, and delete the `<!-- release-summary:todo -->` comment. The summary
   becomes the body of the GitHub Release. Write it for someone deciding
   whether to install the release: what they can now do, what changed in what
   they already did, and anything they have to do themselves when updating.
   The per-commit changelog below the marker already says what changed commit
   by commit, and is regenerated on every refresh.
4. **When you decide to release, mark the PR ready for review.** That runs CI,
   the bundles and `validate / release-pr` on it; the last fails while the
   title is not an allowed next version or the summary is unwritten.
5. **Any refresh turns the PR back into a draft** — a push to `main`, a title
   edit, or a manual run — because the content you decided on has changed.
   Review the new content and mark it ready again.
6. Squash-merge the PR. When CI passes on the merge commit, the `Release`
   workflow tags it `vX.Y.Z`, publishes the GitHub Release, and attaches the
   desktop bundles.

## Setting the version

colophon writes the version with its `command` writer, which runs
`scripts/set-workspace-version.sh` with the version the title names. The
script sets `[workspace.package].version` in `backend/Cargo.toml` and runs
`cargo update --workspace`, so that `backend/Cargo.lock` records every
workspace crate at the new version and a `--locked` build accepts it. It runs
on colophon's runner, which carries nothing beyond the runner image, so it
installs the toolchain `rust-toolchain.toml` names itself and does not use
cargo-edit; its header says how and why. `make set-workspace-version-test`
runs it against throwaway workspaces.

The desktop bundles read the version back through `scripts/workspace-version.sh`,
so the bundles' version matches the tag by construction.

## CI on the draft

colophon cannot keep this repository's own CI off the draft release PR, so the
workflows here skip it themselves. In `ci.yml` and `bundle.yml`, every job but
the gathering ones (`all-green`, `bundle-green`) carries the condition

```yaml
if: ${{ !(github.event_name == 'pull_request' && github.event.pull_request.draft && startsWith(github.head_ref, 'release/')) }}
```

and both workflows run on `ready_for_review` besides the default
`pull_request` types, so marking the PR ready runs everything on it. The
gathering jobs accept skipped jobs, so the required checks pass on the draft.
Other draft PRs still run CI.

## Desktop bundles

Every Release carries bundles of the desktop app, the Electron app in
`frontend/packages/apps/desktop`, built by `.github/workflows/bundle.yml`:

| File | Platform |
|---|---|
| `Coffret_<version>_aarch64.dmg` | macOS, Apple silicon |
| `coffret-desktop_<version>_amd64.deb` | Ubuntu and other Debian-based distributions, x86_64 |

The bundles are made the way `make desktop-build` makes them
(`scripts/desktop.sh build`): the explorer, the Node-API addon with the
explorer compiled in (`make napi-addon`), the app's code, and then
electron-builder with the package's `electron-builder.config.js`, which names
the files and says what goes into them. The version in each file name, the
`.app`'s version and the `.deb`'s `Version` are the workspace version of
`backend/Cargo.toml`, which the script reads through `cargo metadata`
(`scripts/workspace-version.sh`) and hands to electron-builder, so they match
the tag by construction; the version in the package's `package.json` is a
placeholder nothing reads. The `.deb` is the package `coffret-desktop`, the
name the desktop bundles have always had, so that `apt` upgrades an installed
one in place. Each bundle is the installed app (`io.github.x7c1.coffret`),
never the development one, which is only ever run from a checkout.

The Electron binary in each has its fuses set as it is packaged: running it as
plain Node (`ELECTRON_RUN_AS_NODE`), `NODE_OPTIONS`, and the `--inspect` flags
are off, and the app is loaded from its asar archive alone. The macOS app is
signed ad hoc with the hardened runtime, with the entitlements `allow-jit`
(which V8 needs under it) and `disable-library-validation` (an ad hoc signature
carries no Team ID, so library validation would refuse Electron's own
framework); it has no Developer ID signature and is not notarized. What that
means for someone installing either bundle is in [the install
guide](install/README.md).

Before a bundle is uploaded, the job checks it against that configuration
(`scripts/check-desktop-bundle.sh`, which `make desktop-build` runs too), and
fails if any of it does not hold:

- the fuses, as `@electron/fuses read --app` reads them from the binary in the
  bundle: `RunAsNode`, `EnableNodeOptionsEnvironmentVariable` and
  `EnableNodeCliInspectArguments` disabled, `OnlyLoadAppFromAsar` enabled;
- on macOS, from the `.app` in the attached `.dmg`: `codesign -dv` shows an
  ad hoc signature (`Signature=adhoc`) whose flags include `runtime`, the
  signature verifies, and the entitlements include `allow-jit`; the `.app`
  carries its Icon Composer icon in `Assets.car` (`CFBundleIconName` is `Icon`)
  and an `icon.icns`;
- on Linux: `dpkg-deb -I` shows the package `coffret-desktop` at the workspace
  version for `amd64`, and `dpkg-deb -c` lists the desktop entry
  (`/usr/share/applications/coffret-desktop.desktop`, whose `StartupWMClass` is
  `io.github.x7c1.coffret`), `chrome-sandbox` and the AppArmor profile;
- in both, the addon (`coffret.node`) and the icons ship beside the asar
  archive, and the file name carries the workspace version.

- **Order.** The `bundles` job of the `Release` workflow runs only after the
  `publish` job has created the tag and the Release, and uploads to that
  Release. A bundle failure therefore never blocks the tag or the Release; it
  shows up as a failed `bundles` job.
- **Pull requests.** A pull request that changes how the bundle is made —
  the desktop app's package, the addon's crates (`coffret-napi`,
  `coffret-desktop-core`), the lockfiles, the `Makefile`, the scripts that
  build and check the bundle, or `bundle.yml` (the `changes` job in
  `bundle.yml` decides) — runs
  the same builds and uploads each platform's bundle as a workflow artifact
  (`coffret-macos-aarch64`, `coffret-linux-x86_64`), so a broken bundle is
  caught before a release depends on it, and reviewers can download and try
  it. A change to the explorer or the server alone does not run them; the
  next release bundles it. To try one, run the workflow by hand (below). A
  draft release PR runs none of them until it is marked ready for review (see
  [CI on the draft](#ci-on-the-draft)).
- **Manual runs.** The `Bundle` workflow can be run by hand
  (`workflow_dispatch`) with any branch, tag or SHA as `ref`; it uploads
  workflow artifacts and touches no Release.

## Re-running a failed release

- **`Release PR` failed.** Re-run it from the Actions tab, or run the workflow
  by hand on `main`. Each run rebuilds the release branch from `main`.
- **`Release` failed because the summary was missing.** The `plan` job fails
  before anything is tagged. Write the summary into the merged release PR's
  body (a merged PR's description is still editable) and re-run the failed
  run from the Actions tab. A re-run judges the same commit as the original
  run, even after other commits have landed on `main`.
- **`Release` failed after the Release was created.** Re-run the failed jobs:
  an existing Release is kept, and the steps after it run again.
- **A bundle failed.** Open the `Release` run and use "Re-run failed jobs":
  only the failed matrix entries of `bundles` run again, against the same tag
  and Release, and an upload replaces a file of the same name. To build a tag
  without touching its Release, run the `Bundle` workflow by hand with the
  tag as `ref`.
- **CI was red on `main`.** The `Release` workflow considers only a green CI
  run, so nothing was tagged. CI runs again on the commit that fixes it, but
  that commit is not the merge of the release PR; re-run the failed CI run of
  the release commit instead. When that re-run turns green, its completion
  starts a new `Release` run. Re-running the `Release` run the red CI started
  does not help: a re-run replays the original event, which still says CI
  failed.
- **Abandoning a release PR.** Close it and delete its branch; the next push
  to `main` opens a fresh one with the default title.

## Workflows involved

- `.github/workflows/release-pr.yml` — calls colophon's `release-pr.yml`, which
  opens or refreshes the release PR as a draft on every push to `main` (but
  not the one that lands a release), when the release PR's title is edited,
  and on a manual run on `main`. It writes the version with
  `scripts/set-workspace-version.sh`.
- `.github/workflows/validate-release-pr.yml` — calls colophon's
  `validate-release-pr.yml` from its job `validate`, so the check reads
  `validate / release-pr`. It passes on every PR that is not a release PR and
  on a draft one; on a release PR marked ready for review it enforces the
  allowed title transitions and fails while the summary is unwritten.
- `.github/workflows/release.yml` — when CI completes on `main`, colophon's
  `release-plan.yml` (job `plan`) decides whether the commit CI judged is the
  squash of a merged PR labelled `release`, and reads the version from its
  title; if it is, `release-publish.yml` (job `publish`) tags that commit and
  publishes the GitHub Release with the PR's summary, and the `bundles` job
  calls `bundle.yml` to attach the desktop bundles.
- `.github/workflows/bundle.yml` — builds the desktop bundles on macOS (Apple
  silicon only) and Linux; attaches them to a Release when called from
  `release.yml`, and uploads them as workflow artifacts on pull requests and
  manual runs.

Every colophon call is pinned to the commit of colophon v0.3.0, as the other
actions here are pinned; Dependabot proposes updates to it with the other
GitHub Actions.

The bundles are built and attached without a third-party action: they are
made with electron-builder, a pinned development dependency of the desktop
package, and uploaded with the `gh` CLI, for the reason the `cargo-deny` job
in `.github/workflows/ci.yml` gives.

## Release automation setup

The release PR is opened as a **GitHub App**, so that CI and
`validate / release-pr` run on its refreshes too: GitHub starts no workflows
for events caused by the default `GITHUB_TOKEN`. The App's private key is
turned into a token for this repository only, valid for an hour, so there is
no long-lived token to rotate.

**One-time setup, by a person with admin access to the repository:**

1. Create the GitHub App and install it on this repository, as colophon's
   [usage guide](https://github.com/x7c1/colophon/blob/v0.3.0/docs/usage.md#tokens) describes (Contents and Pull requests: Read
   and write).
2. Add the App's Client ID as the Actions variable `RELEASE_APP_CLIENT_ID`,
   and its private key as the Actions secret `RELEASE_APP_PRIVATE_KEY`.
3. Allow squash merge, which the release relies on: the squash's default
   title (the PR title) is how a release landing is recognized.

Set both: with the variable but no secret, `Release PR` fails because
colophon cannot create the App's token; with neither, colophon falls back to
`GITHUB_TOKEN`, which can open the release PR only if the repository's
Actions settings allow GitHub Actions to create pull requests, and CI does not
run on the PR's refreshes. Once the App is set up, run the `Release PR` workflow by
hand on `main` to open the release PR without waiting for the next push.

The `Release` and `Bundle` workflows use the default `GITHUB_TOKEN`: they run
after a merge a person made, and no workflow here listens to tag pushes or
Release events, so nothing has to be triggered by what they create.

## Required checks

`main` requires one check per workflow that runs on every pull request, and
nothing else:

| Check | Workflow | Passes when |
|---|---|---|
| `all-green` | `ci.yml` | every CI job passed or was skipped |
| `bundle-green` | `bundle.yml` | the bundles were built and checked, or the pull request does not change how they are made, or it is a draft release PR |
| `validate / release-pr` | `validate-release-pr.yml` | the title and summary are valid, or the pull request is not a release PR, or it is a draft |

The individual jobs are not required. `all-green` and `bundle-green` pass only
when every job of their workflow succeeded or was skipped, and fail on any
other result (failed, cancelled, or abandoned without a runner), so a failure anywhere blocks
the merge — including for auto-merge, which waits only for required checks. A
job that a pull request has no reason to run is skipped, and a skipped job does
not fail them. Requiring a job directly would hold every pull request that
skips it, since a required check that never reports is waited for forever.

A new CI job goes into the `needs` of `all-green`, and carries the draft
release PR condition (see [CI on the draft](#ci-on-the-draft)); the required
checks stay as they are. A new workflow that runs on pull requests gets a
gathering job of its own, added to the required checks.
