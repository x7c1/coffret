---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, concept-alignment, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && grep -rqi 'idle' backend/crates/apps/coffret-server/src && grep -rq '/api/lock' backend/crates/apps/coffret-server/src"
assignee: null
branch: task/0902-2050-lock-the-server-explicitly-and-on-idle
created_at: 2026-09-02T11:50:00Z
updated_at: 2026-09-02T15:40:00Z
---

# feat(server): lock the server explicitly and on idle

## Overview

The Device Key Custody register describes two states and the moves
between them: locked and unlocked (DK-1), an explicit lock available
whenever the device is unlocked (DK-3), a lock forced by inactivity
(DK-4), operations while locked failing with "the Passphrase is
required" and never partially succeeding (DK-2), and no readable copy of
the Master Key surviving a lock (DK-7 — the type-level half already
holds: every key type zeroizes on drop and none is cloneable). The
long-running server keeps none of this: its crate doc says plainly that
the process is one unlock and the derived keys live as long as the
process. A person who walks away from their machine leaves a server that
can open the whole Library for as long as it runs.

Give the server the lock lifecycle the register describes:

1. **Keys behind a single custody cell.** Put the unlocked key material
   (`LibraryKeys` and whatever hangs off it) behind one shared holder
   that can be emptied — the state the router hands to handlers keeps a
   handle to the holder, not to the keys. Emptying the holder is the
   lock: the `Arc` inside drops, and the zeroize-on-drop types from the
   custody groundwork do the wiping. In-flight requests that already
   took a key handle finish their work and release the last reference;
   new work refuses immediately — that is what DK-3's "has taken effect
   by the time it returns" means for a concurrent server, and DK-2's "no
   partial success" is per-operation, not per-connection.
2. **An explicit lock entry point.** `POST /api/lock` (inside the same
   capability fence as every other route) empties the holder and
   returns once it is empty. The explorer gets a plain way to call it —
   a small control near the library name is enough; do not build a
   session UI.
3. **Idle lock.** Inactivity for a configured interval locks the same
   way. Activity is an authorized request **to a route that needs the
   keys** — a person turning pages, uploading, syncing. The keyless
   routes (identity, activity reporting, lock itself) are the
   explorer's own background polling and must not count: an open tab
   quietly asking "anything new?" is not a person at the keyboard, and
   if it kept the server awake the idle lock would never fire in
   exactly the walked-away-mid-read case it exists for. The interval is
   a policy parameter (a CLI flag with an environment fallback and a
   sensible default measured in minutes — the register forbids making
   it a format constant, DK-4). The timer must not hold the keys
   itself, must survive extreme interval values without panicking, and
   must not fire mid-request in a way that breaks atomicity: a request
   that began unlocked completes. A keyed operation counts as activity
   for its whole span, not just its first moment — the clock is
   refreshed when the operation's key handle is released, so a freeze
   that outlives the interval does not leave the server locked under
   the very work it just finished. The idle clock starts when serving
   starts, not before it.
4. **Locked means "the Passphrase is required".** Every route that
   needs keys answers a locked server with a refusal whose kind and
   message say exactly that (DK-2's wording), distinct from the
   capability refusals — being locked is the owner's own state, not an
   intruder's. The explorer surfaces the message it already knows how
   to show, and its background activity (fill, catch-up) stops cleanly
   instead of half-running. Routes that need no keys (the library
   identity route, lock itself) keep answering.
5. **Unlock is the Passphrase's move.** The server takes its Passphrase
   at startup, and restarting it is the one unlock path this task
   ships; say so in the refusal message so the owner knows what to do.
   (An unlock endpoint would carry the Passphrase through the browser —
   a boundary this product has deliberately not crossed; if it ever
   does, that is its own decision with its own review.)
6. **Tests.** Router tests: explicit lock refuses subsequent keyed
   routes with the locked kind while the identity route still answers;
   a request in flight when the lock lands completes; lock is
   idempotent; the idle timer fires after quiet and does not fire
   across steady activity (drive the clock, do not sleep real minutes);
   a locked server's refusal names the Passphrase. E2E: the existing
   journeys keep passing (the default idle interval must comfortably
   exceed a journey's span); one journey step locking via the new
   route and seeing the explorer report the locked state is enough —
   do not build a full lock/unlock journey around a server restart.

Update the server crate doc: the "one unlock, keys live as long as the
process" paragraph is the thing this task retires — the keys live from
unlock to lock, and DK-1 through DK-4 name the moves.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `POST /api/lock` empties the custody holder and has taken effect
      by the time it returns: a router test shows keyed routes refusing
      afterwards with the locked kind while keyless routes still answer
      (the `/api/lock` grep gate pins the route).
- [x] Inactivity for the configured interval locks the server; the
      interval is a CLI/env policy parameter with a default, and tests
      drive a mock clock in both directions — fires after quiet, does
      not fire across activity (the `idle` grep gate pins the
      mechanism).
- [x] A locked server refuses keyed operations atomically with a
      message saying the Passphrase is required and how to provide it;
      a request already in flight when a lock lands completes.
- [x] The E2E journeys pass unchanged, plus one step exercising the
      lock route and the explorer's rendering of the locked state.

## Out of scope

- An unlock endpoint or any path that carries the Passphrase through
  the browser — restart-with-Passphrase is the unlock this ships.
- Locking the CLI's short-lived processes (they end and drop their keys
  already).
- The OAuth token cache's on-disk custody.
- Changing the spec register's text.
