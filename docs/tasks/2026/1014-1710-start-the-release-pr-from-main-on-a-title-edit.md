---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1014-1710-start-the-release-pr-from-main-on-a-title-edit
created_at: 2026-10-14T07:07:46Z
updated_at: 2026-10-14T08:11:19Z
---

# ci(release): build the Release PR from main when its title is edited

## Overview

`.github/workflows/create-release-pr.yml` also runs on
`pull_request: types: [edited]`, so that editing the Release PR's title
re-renders it. That run checks out the PR's merge ref, and
`.github/scripts/create-release-pr.sh` computes the last tag and the
changelog (`git log "${last_tag}..HEAD" --no-merges`, or the last 20 commits
when there is no tag) on that checkout before it switches to `origin/main`.
The merge ref's history holds the release branch's own `Release vX.Y.Z`
commit, so the changelog gains a line `- Release vX.Y.Z`, and with no tag yet
the 20-commit window drops one commit of main. The next push to main renders
it correctly again; the Release body is not affected.

At the start of `main()` in `create-release-pr.sh`, before anything reads
history, fetch `origin main` and check out `origin/main` detached, with a
comment saying why (a title edit's checkout is the PR's merge ref; everything
the script describes is main). Correct the script's header comment where it
says the script runs only on a push to main. `update_release_pr` already
rebuilds the branch from `origin/main` and keeps working as before.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `create-release-pr.sh` checks out `origin/main` before it reads the
      last tag or the changelog, with the reason in a comment
- [x] `make check` passes (it runs shellcheck on the scripts)

### Before merge (verified outside the check command)

- [ ] In a scratch clone: check out `pull/247/merge`, run the two new lines,
      then `generate_changelog`; the output has no `Release v` line (it has
      one without them)
