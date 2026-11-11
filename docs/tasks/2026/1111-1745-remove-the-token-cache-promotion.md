---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, spec-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it'
assignee: null
branch: task/1111-1745-remove-the-token-cache-promotion
created_at: 2026-11-11T08:45:00Z
updated_at: 2026-11-11T09:05:00Z
---

# refactor: remove the promotion of a Library's own token cache to its account

## Overview

A Library made before Storage grants moved to accounts kept its OAuth token
cache of its own, sealed under its `coffret/v1/token-cache` purpose key
(KD-4, KD-10). Opening such a Library promotes that cache to a device-local
account (SA-8): `backend/crates/apps/coffret-device/src/account_grant.rs`,
its `PromotionObstacle` and `PromotionNeedsName` errors, the CLI's answers to
them, and the tests that build a Library of the older form to exercise it.

No Library of the older form is left to promote, so the path is dead weight.
Remove it:

1. Remove the promotion and everything that exists only for it: the code in
   `account_grant.rs` (and the module itself if nothing else is left in it),
   the errors and refusals only it raises, their handling in the CLI, the
   server and the desktop app, any HTTP refusal code or CLI exit code used
   only by them, and the tests and fixtures that build or promote an
   older-form Library.
2. SA-8 is the rule for account grants as a whole, and the promotion is
   only its last clause. Remove that clause from SA-8's statement (the
   `//! SA-8:` module doc) and every statement or citation that promises the
   promotion; keep SA-8 itself and its citations for the other clauses.
3. Keep the `coffret/v1/token-cache` purpose in KD-4's table, marked as
   retired for Libraries: no Library's cache is opened under it any longer,
   and the label is never reused. The manual Drive tools that seal their own
   token cache are not changed by this task; say in KD-4 or KD-10 what they
   still use, so the table does not claim nothing does. Change KD-10 and any
   other rule that names the promotion so that it no longer promises it.
4. A Library that still holds such a cache on disk, if one did, is not
   migrated and not refused for it: whatever opening it does today apart
   from the promotion stays as it is. Say so where the removed rule was, in
   one sentence, if a reader would otherwise wonder.

Do not change the key file's format, the account's on-disk format, or the
refusals and exit codes any path other than the promotion uses. If removing
the promotion turns out to need one of those changed, stop and say so
instead. Do not change `.github/`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] No code, test or spec text promotes a Library's own token cache
      to an account: `git grep -n -i promot -- backend frontend docs` finds
      no promotion of a token cache.
- [x] KD-4 lists `coffret/v1/token-cache` as retired.
- [x] `make check` (including the spec-citation and rule-id checks) and
      `make e2e-it` pass.
