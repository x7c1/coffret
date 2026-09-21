---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, error-type-design, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && ! grep -q 'STARTUP_TIMEOUT_MS = 60_000' frontend/packages/apps/e2e/journeys/server.ts && grep -q 'UnrepairedReplica' backend/crates/apps/coffret-device/src/lib.rs && grep -q 'mod tests' backend/crates/apps/coffret-cli/src/report.rs && grep -q 'COFFRET_DRIVE_CLIENT_SECRET' backend/crates/apps/coffret-cli/tests/support/device.rs"
assignee: null
branch: task/0921-2149-close-the-small-leftovers-the-last-three-prs-named
created_at: 2026-09-21T12:49:08Z
updated_at: 2026-09-21T14:52:37Z
---

# fix: close the small leftovers the last three merged PRs named

## Overview

Eleven leftovers that #186, #184 and #179 recorded rather than fixed. They
share nothing but their size: each is small, each is already located, and
none of them needs a design decision that this task has to take. Work them
one at a time and keep each one's change to what the item asks for. If an
item turns out to be already fixed, or cannot be done without a decision
the item does not contain, leave it and say so in the report.

### From #186 (stale comments)

1. **The e2e startup wait and the server's startup catch-up have the same
   minute.** `frontend/packages/apps/e2e/journeys/server.ts`'s
   `STARTUP_TIMEOUT_MS` is 60 s and
   `backend/crates/apps/coffret-server/src/refresh/catch_up_at_startup.rs`'s
   `DEADLINE` is 60 s. The catch-up runs before the socket is bound, so a
   catch-up that uses its whole deadline gives up, binds and answers only
   after the suite has already stopped waiting — a flaky e2e run.

   Keep the order as it is: the catch-up runs before the bind on purpose,
   so that a page that reaches the server sees a caught-up catalog. Give
   the suite's wait room on top of the server's minute instead — the wait
   has to outlast the deadline plus the bind and the first answer. Say in
   `STARTUP_TIMEOUT_MS`'s doc that it is the server's own deadline plus
   room, name `catch_up_at_startup.rs`'s `DEADLINE` as the thing it is
   measured against, and correct the sentence there that currently says
   this minute *is* that minute.

2. **`scripts/e2e-it.sh`, the port-override comment.** It says the block
   overrides "one of the ports"; the block it sits on binds one port
   (19010), and `SERVER_PORT` / `WEB_PORT` below it are the others. Say
   which ports the overrides cover so the sentence is true of the block
   it heads.

3. **`backend/crates/apps/coffret-server/src/api_error/mod.rs`, the
   `too_large` doc.** `Either flow` reads as an instruction to the reader
   until the colon that follows resolves it. Rephrase so the sentence does
   not ask the reader to hold a question.

### From #184 (Keyring repair)

4. **The CLI prints a commit's refusal twice.** `SyncError::Commit` and
   `FreezeError::Commit` render the wrapped `CommitError` transparently in
   `Display` *and* return it from `source()`, so `{error:#}` prints the
   same sentence twice in the chain. Read the convention the `coffret-device`
   crate states at the top of `src/error.rs` — a wrapper says what layer
   the failure belongs to and lets the typed cause carry the lower layer's
   own answer — and bring these two variants to it. Survey the other
   transparent variants of both enums (`Index`, `Format`, and any sibling
   that renders its cause verbatim while also returning it) and fix the
   ones with the same shape; leave the ones that differ and say why.
   `redacted()` must keep saying what it says now.

5. **A freeze warns twice about one degraded set.** `freeze/run.rs` reads
   the Keyring and `commit_batch` examines it again, and each logs its own
   warn line for the same generation. Bundle the read and the examination
   so one run says it once. Log lines only — nothing a person sees at the
   CLI changes.

6. **`UnrepairedReplica` and `InvalidReplica` are not re-exported from
   `coffret-device`.** A caller holding a `coffret_device::Error` cannot
   name the `cause` it carries. Re-export them where the crate re-exports
   the rest of its error vocabulary.

7. **`report::repaired` and `report::committed` have no tests.** Both build
   a sentence whose singular and plural have to agree with a count, which
   is what regresses. Add unit tests in
   `backend/crates/apps/coffret-cli/src/report.rs` covering none, one and
   more than one, including the `0 =>` arm of `repair_line` that stands in
   for the prose invariant on `KeyringRepair::rewritten`.

### From #179 (`--client-id` required, secret from the environment)

8. **A secret-bearing client with `COFFRET_DRIVE_CLIENT_SECRET` unset fails
   late and does not name the variable.** `init` takes the person through
   consent in the browser and only then fails, at the code exchange, with a
   refusal that does not say which variable was missing. In the gateway's
   `oauth/authorization/mod.rs`, have the code-exchange refusal carry the
   fact that the authorization was made without a secret; in the CLI, name
   `COFFRET_DRIVE_CLIENT_SECRET` in what the person reads. Do not add a
   pre-flight check that guesses whether a client has a secret — a client
   without one is legitimate.

9. **Typing the removed `--client-secret` makes clap suggest
   `--client-id`.** A person who takes the suggestion pastes their secret
   where the ID goes. `coffret-shell/src/parser_refusal.rs` keeps clap's
   message for anything spelled like a flag; carve `--client-secret` out of
   that rule and answer it with a refusal of our own that says the secret
   is read from `COFFRET_DRIVE_CLIENT_SECRET` and nowhere else. Do not
   quote what was typed. Every other flag keeps clap's message.

10. **The CLI test harness hands the parent's environment to the child.**
    `backend/crates/apps/coffret-cli/tests/support/device.rs` lets
    `COFFRET_DRIVE_CLIENT_ID` and `COFFRET_DRIVE_CLIENT_SECRET` through, so
    a machine set up for the Drive targets runs these tests in a different
    environment from CI. `env_remove` both. The tests that spawn the binary
    directly to get an empty secret do so for this reason — check whether
    the harness now serves them and say so either way.

11. **Say in the harness why it removes them.** One comment, in the voice
    of the ones around it, so the next person does not add a variable back
    by reflex.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes
- [x] `STARTUP_TIMEOUT_MS` is no longer 60 s and its doc names the server's
      `DEADLINE` as what it is measured against
- [x] `UnrepairedReplica` is re-exported from `coffret-device`
- [x] `report.rs` has tests, covering none / one / more than one
- [x] the CLI test harness removes `COFFRET_DRIVE_CLIENT_SECRET` from the
      child's environment
- [x] no `{error:#}` chain prints one sentence twice for a refused commit
      (a test asserting the rendered chain for `SyncError::Commit` and
      `FreezeError::Commit`)

### Manual / on-hardware (verified by a human before merge)

- [ ] `make e2e-it` is green
- [ ] each of the eleven items is done as described or reported as left
      alone with its reason

## Out of scope

- The five design items #184 left — whether an erroring run carries the
  repairs an earlier attempt made, whether a fetch-only user is told about
  a degraded Keyring, the explorer's `502 storage` wording, carrying every
  failed position's cause rather than the first, and lifting
  `KeyringRepair::rewritten`'s non-emptiness into a type. They are one
  question (how far a degraded Keyring's news travels) and get their own
  task
- Unifying the three places that raise `LengthMismatch`
  (`ByteStream::collect_exact`, `fetch/range_read.rs`, `fetch/container.rs`)
- The vocabulary and register items #184 and #185 left — `replica index`
  versus `position`, the address of `health event`, the concept's Mental
  Model against its Domain Rules. They belong to the next docs pass
- Changing the order in which the server catches up and binds its socket
