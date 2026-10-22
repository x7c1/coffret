---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make shell-lint && grep -q "install-deps" .github/workflows/ci.yml && grep -Eq "Dir::Cache::[Aa]rchives" .github/workflows/ci.yml'
assignee: null
branch: task/1023-0437-keep-the-e2e-jobs-apt-packages-in-the-actions-cache
created_at: 2026-10-22T19:37:28Z
updated_at: 2026-10-22T20:05:00Z
---

# ci(e2e): keep the system packages Chromium needs in the Actions cache, so a slow mirror cannot fail the job

## Overview

The `e2e` job in `.github/workflows/ci.yml` installs the system packages a headless Chromium needs with `pnpm exec playwright install-deps chromium`, under a 5-minute step timeout. Before it, the "Bound apt's waits on the mirror" step makes apt give up on a connection that sends nothing for 30 seconds and retry, which covers a stalled mirror. It does not cover a slow one. On 2026-10-22 the Azure Ubuntu mirror served about 25 MB of fonts and libraries at roughly 50–70 kB/s: each package took one to two minutes, nothing was ever silent for 30 seconds, and the step hit its 5-minute timeout at the seventh of thirteen packages. The change under test had nothing to do with it. On the seven runs before it the same step took 8 seconds to 2 minutes.

Re-running until the mirror is fast is not a fix. Most of what the step downloads is the same from run to run, so it should come from the Actions cache, and reach the mirror only when the set of packages or their versions change.

### Decisions (already made; implement them)

1. **Cache the `.deb` files the step installs.**
   - Keep a directory the runner user owns (for example `~/.cache/apt-archives`) in `actions/cache`.
   - Point apt at it for this job's apt runs with `Dir::Cache::Archives`, written under `/etc/apt/apt.conf.d/` next to the existing `80-stall-timeout`, so that it reaches the apt runs `playwright install-deps` makes. The directory needs the `partial/` subdirectory apt expects, and apt runs as root, so check that `actions/cache` can save what apt wrote. Make the files readable to the runner user before the post-job save if they are not.
   - Key the cache on the runner image (`ImageOS` and `ImageVersion` from the environment), and the Playwright version (the `steps.playwright.outputs.version` the job already computes), since those decide which packages and versions `install-deps` asks for. Give it a restore key without the image version, so that an image update reuses the `.deb` files whose versions did not change and fetches only the rest.
   - A cache hit must leave apt with nothing to download for an unchanged image. A miss behaves as today.
2. **Keep the existing bounds.** The 30-second stall timeout and retries, and the step's `timeout-minutes: 5`, stay. With the cache warm, the step no longer depends on the mirror's throughput. Update the comments on both steps so they say what each bound covers now: a stall, a cold cache, a slow mirror on a cold cache.
3. **Only the `e2e` job.** It is the only job in this repository that runs apt (check with `grep -n "apt" .github/workflows/*.yml`, and say in the PR if another job turns out to run it). Do not change the Playwright browser cache or the cargo cache.

Out of scope: changing the mirror, and running the journeys without `install-deps`.

## Acceptance criteria

### Automated (pipeline-verified)
- [x] `.github/workflows/ci.yml` points apt's archive directory at a cached directory (`Dir::Cache::Archives`) for the step that runs `playwright install-deps`
- [x] `shell-lint` passes

### Before merge (verified outside the check command)
- [x] On this PR's CI, the `e2e` job passes, and its cache step reports a save on the first run
- [x] A second run of the `e2e` job on this PR (a re-run, or a later commit) restores the cache, and the log of "Install Chromium's system packages" shows apt fetching no archive from the mirror (no `Get:` line for a `.deb`)
