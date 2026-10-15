---
status: completed
pipeline_phase: null
plan: null
base_ref: feat/explorer-client-auth
perspectives: [completeness, clarity, concept-alignment, error-type-design]
max_refine_rounds: 2
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1015-0902-let-a-new-explorer-tab-take-the-least-recently-used-session-at-the-cap
created_at: 2026-10-15T00:02:00Z
updated_at: 2026-10-15T00:49:18Z
---

# fix(explorer-host): let a new explorer tab take the least recently used session at the cap

## Overview

The explorer host holds at most 8 browser sessions (LA-18), and at the cap
it refuses a new exchange with `409 session_limit`. A session lasts 12 hours
and does not end when its tab is closed: the page keeps its token in the
tab's `sessionStorage` and revokes it only through *Disconnect this tab*. The
desktop app's tray item *Open the explorer* issues a fresh ticket every time.
So a person who opens the explorer from the tray nine times within 12 hours,
closing tabs as they go, is refused on the ninth, and is told to "Close one",
which does not help. That breaks daily use.

Change the cap so that it never refuses the owner:

- When an exchange would exceed the cap, the host revokes the session that
  was **least recently used**, then admits the new one. "Used" means
  authenticated by any request, `/auth/session` included. A tab whose
  session was taken this way gets `401` on its next request and shows the
  existing no-session message.
- This is safe because an exchange needs a launch ticket, and only the owning
  account can get one (LA-17, LA-21). A page that holds no ticket cannot
  evict anything.
- Record the eviction as an event by kind and counts only (EL-1).
- The `session_limit` refusal and its `409` no longer occur. Remove them from
  the host, from the generated refusal contract, and from the page's refusal
  kinds, together with any page handling of them.

Update LA-18 to say this, in place of the refusal at the cap. Update
`docs/guides/environments.md`, which now tells a person to disconnect tabs at
the cap, and `docs/guides/install/README.md` if it mentions the cap.

Also, in the same crate: `LaunchDir::sweep` (`launch_file/launch_dir.rs`)
reports a failed removal as `LaunchFileError::Io`, whose doc and `Display`
speak only of writing ("a launch file could not be written: removing a stale
file failed"). Give removal its own variant, or widen the type's doc and
`Display` so the message says what happened.

Keep everything else as it is: the 12-hour expiry, the ticket rules, the
generation rules, and lock behaviour.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] LA-18 says that at the cap the least recently used session is revoked to
      admit a new exchange, and the spec checks pass
- [x] Unit test (credentials state): with the cap reached, an exchange
      succeeds and the session used least recently stops authenticating, while
      the others keep working; "use" is refreshed by `authenticate`
- [x] Router test: nine exchanges in a row all succeed; the first token is
      refused with `401` afterwards, and the ninth works
- [x] No code, contract or page path still produces or handles
      `session_limit` (a test or grep gate)
- [x] An event records the eviction with no token, ticket or digest in it
      (sentinel test)
- [x] A failed sweep removal displays as a removal, not a write (test)
- [x] `make check` passes
