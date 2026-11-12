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
branch: task/1111-1610-close-the-verification-gaps
created_at: 2026-11-11T07:10:00Z
updated_at: 2026-11-12T18:04:20Z
---

# test: close four verification gaps in the route fences, the root-run tests and the session spec

## Overview

Four gaps in what the checks verify:

1. **`/api/filter` is missing from the fence test.** `EVERY_ROUTE` in
   `backend/crates/apps/coffret-server/tests/routes/fences.rs` lists the
   routes the LA-2 / LA-5 fences are checked against, and `/api/filter`
   (added with the filter) is not in it. Add it, and make the test fail when
   a route the server mounts is missing from the list, so the next route
   cannot be left out the same way (compare against the router's own list of
   routes rather than a second hand-kept one, if the server exposes one, or
   otherwise add the cheapest guard that catches the omission).
2. **Two checks fail when `make check` runs as root.** Root ignores file
   permissions and owns everything, so
   - `scripts/desktop-dev-sandbox-test.sh`, case `setuid-sandbox-not-root`,
     fails at "check let the app start", and
   - `upload::a_root_the_system_would_not_answer_about_stops_the_drop` in
     `backend/crates/apps/coffret-server/tests/routes/upload.rs` fails,
   both on `main` too. Make each check what it means to check whoever runs
   it — for example by making the condition the test needs in a way root
   cannot see through, or by asserting the root-specific outcome when run as
   root — without skipping, disabling or weakening what it checks for a
   normal user. Say in a comment why it is written that way.
3. **The session rules LA-18 and LA-21 were rewritten without a spec
   review.** Review `docs/spec/loopback-access/` LA-18 and LA-21 against
   the code and tests that implement them (the desktop app's fresh ticket
   before the token expires, the page swapping the token without reloading,
   and the desktop reloading the window when the session ends anyway): fix
   citations that name the wrong rule or restate it inaccurately, name the
   rules from the tests that sample them, and report any other statement
   that no longer holds rather than changing what a rule says. One statement
   is settled already: LA-21 says the desktop app waits only while the
   Library is locked and the Passphrase window is in front, but the app
   (`desktop.ts`) waits for as long as the Library is locked. Rewrite that
   sentence of LA-21 to match the code — while the Library is locked, the
   app waits for it to be unlocked before reloading the window. Behaviour
   does not change.
4. **Journey 14 (`14-filter-read-on.spec.ts`) failed 2 times in 3 in a
   container session**, never in CI. Run `make e2e-it` at least three times
   on this branch. If journey 14 (or any other journey) fails, find the cause
   from its trace and fix it; do not raise a timeout, add a retry to the
   journey, or skip it. If all runs pass, say so in the PR body with the
   number of runs.

Do not change `.github/`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The fence test covers `/api/filter`, and fails when a mounted route is
      missing from what it checks.
- [x] `scripts/desktop-dev-sandbox-test.sh` and
      `a_root_the_system_would_not_answer_about_stops_the_drop` pass both as
      root and as a normal user, still checking the same refusal for a normal
      user.
- [x] The tests that sample LA-18 and LA-21 name them in the repository's
      form, and the spec-citation checks in `make check` pass.
- [x] `make check` and `make e2e-it` pass.
