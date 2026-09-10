---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, error-type-design, user-experience, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "LA-8" docs/spec/loopback-access/README.md && grep -rq "LA-8" backend/crates/apps/coffret-device/src && grep -q server_lock_file backend/crates/apps/coffret-device/src/library_dir.rs && grep -q authority backend/crates/apps/coffret-server/src/authorize/refused.rs && grep -q AUTHORITY backend/crates/apps/coffret-server/tests/routes.rs'
assignee: null
branch: task/0909-2011-refuse-a-second-server-for-a-library
created_at: 2026-09-09T20:11:57Z
updated_at: 2026-09-09T21:05:31Z
---

# feat(server): refuse a second server for a Library and say where the first one is

## Overview

A server publishes the key it admits callers by into the Library's own
directory, unconditionally: `ServerKey::publish`
(`backend/crates/apps/coffret-device/src/server_key.rs`) writes
`<library>/server-key` over whatever is there, and
`backend/crates/apps/coffret-server/src/main.rs` calls it on every start.
Nothing stops a second `coffret-server` starting over the same Library. When one
does, the first server — still running, still holding the Library open — becomes
a process that answers 403 to everything, including the proxy in front of the
explorer, which re-reads that file per request
(`frontend/packages/apps/web/vite.config.ts`). The port is no defence: it is
an argument, and two servers on two ports over one Library is the case that
breaks.

Refuse the second server and leave the first one untouched. Before it opens the
Library, a server takes an exclusive advisory lock (`rustix::fs::flock` with
`FlockOperation::NonBlockingLockExclusive`; the `fs` feature is already in
`backend/Cargo.toml`) on a new `server.lock` in the Library's directory, and
holds it for the life of the process. A start that finds the lock held is
refused before the Passphrase is asked for, guarded by the existing
`LibraryDir::is_present()` so a Library that is not on this device still gets
`open_library`'s own refusal. The operating system releases the lock however
the holder ends, `SIGKILL` included, so a killed server leaves a lock nobody
holds and the next start simply takes it — no file has to be deleted by hand,
which is what LA-4 already promises about the key file. `flock` locks are per
open file description, so a second open-and-lock from the same process
conflicts too, which is what makes the rule testable in one process.

Add `ServerLock` beside `ServerKey` in `coffret-device` (its own module,
`server_lock.rs`), `LibraryDir::server_lock_file()`, an open-or-create helper
in `owner_only.rs` at `OWNER_ONLY_FILE`, and
`Error::LibraryAlreadyServed { name, by }`, whose sentence names the Library
and the process already serving it — roughly "the Library "photos" is already
being served on this device, by process 4213. One server at a time serves a
Library: stop that one before starting another." — and names neither the key,
the key file, nor the lock file. `by` is optional because the holder writes
its PID just after taking the lock: the guarantee rests on the lock, the
process number only on the sentence. The crate-level directory listing in
`coffret-device/src/lib.rs`, the "six things" doc on `LibraryDir`, and the
`every_file_is_under_the_directory_named_after_the_library` test are updated
for the new file. `ServerKey::publish` itself does not change; the comment on
`a_second_run_draws_a_key_of_its_own` is rewritten to say that what stops a
second server is LA-8, one layer up.

The register gains one rule in `docs/spec/loopback-access/README.md`:

- **LA-8.** One server at a time serves a Library on a device. A server takes
  an exclusive advisory lock on a file in the Library's own directory before it
  opens the Library, and a start that finds the lock held is refused: the
  server already running keeps its key, its callers and its hold on the
  Library, and nothing about it is disturbed. The lock is the operating
  system's, so it is released however the holding process ends — a killed
  server leaves a lock nobody holds, and the next start takes it (LA-4). The
  refusal names the Library and, where it can, the process already serving it,
  and never the key or the file it is in. *(Form: test)*

**LA-4** gains a closing clause pointing at it: "A second server for the same
Library never reaches the point of publishing one at all (LA-8)." The
`Loopback Access` row of the mechanism table in `docs/spec/README.md` gains
"the one server at a time that serves a Library", and the loopback Domain Rule
in `docs/concepts/library/README.md` gains a sub-bullet for it citing LA-8.
Rule IDs are permanent; LA-1 … LA-7 keep their text apart from the LA-4 and
LA-5 clauses named here. Inside the register a rule cites a sibling bare;
outside, `(spec: LA-8)`.

The tests LA-8 owes, in `server_lock.rs`:
`a_second_server_for_one_library_is_refused`,
`the_refusal_names_the_library_and_the_process_holding_it`,
`the_refusal_names_neither_the_key_nor_the_file_it_is_in`,
`a_lock_a_stopped_server_left_behind_is_taken_by_the_next_one`,
`two_libraries_are_served_at_once`, and `the_lock_file_is_owner_only`
(`#[cfg(unix)]`, as the key-file test is). Each cites LA-8 in its comment.

One smaller repair on the same surface. A request whose `Host` names
somewhere the server is not is told only that it asked for "a different one"
(`backend/crates/apps/coffret-server/src/authorize/refused.rs`). The address
the server actually bound is in hand — `Admission::authority`, built in
`authorize/mod.rs` from what the listener reported — so name it: "this
Library is served at 127.0.0.1:8787 and nowhere else, and this request named a
different address", with the real bound authority interpolated. Keep `Refused`
the fieldless `Copy` enum it is and take the authority as an argument to
`recorded`/`message` (returning `Cow<'static, str>`), passed from
`authorize/admit.rs`, where the `Admission` is already extracted; widen
`ApiError::unauthorized` to `impl Into<String>`, which its `String` field
already wanted. **LA-5** gains a clause: "It may name the address the server
bound, which is an address the caller has already reached." Tests:
`the_elsewhere_refusal_names_the_address_this_server_bound` in
`authorize/tests.rs` and `a_host_refusal_names_the_address_this_server_bound`
in `tests/routes.rs` (using `support::AUTHORITY`);
`a_refusal_never_says_what_the_key_is` passes unchanged.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A second server for a Library on this device is refused while the first
      one runs, and the first one keeps its key, its callers and its hold on the
      Library — `a_second_server_for_one_library_is_refused` and
      `two_libraries_are_served_at_once` in
      `backend/crates/apps/coffret-device/src/server_lock.rs`, run by
      `make check`.
- [x] A lock a stopped server left behind is taken by the next start rather than
      refusing it — `a_lock_a_stopped_server_left_behind_is_taken_by_the_next_one`,
      run by `make check`.
- [x] The refusal names the Library and the process serving it, and names
      neither the key nor the file it is in —
      `the_refusal_names_the_library_and_the_process_holding_it` and
      `the_refusal_names_neither_the_key_nor_the_file_it_is_in`, run by
      `make check`; the lock file is created owner-only —
      `the_lock_file_is_owner_only`.
- [x] The register carries LA-8, the device crate cites it, and the Library
      directory knows the new file:
      `grep -q "LA-8" docs/spec/loopback-access/README.md`,
      `grep -rq "LA-8" backend/crates/apps/coffret-device/src`,
      `grep -q server_lock_file backend/crates/apps/coffret-device/src/library_dir.rs`.
- [x] A refusal for a `Host` naming somewhere else names the address the server
      bound — `the_elsewhere_refusal_names_the_address_this_server_bound` and
      `a_host_refusal_names_the_address_this_server_bound`, run by
      `make check`, gated by
      `grep -q authority backend/crates/apps/coffret-server/src/authorize/refused.rs`
      and `grep -q AUTHORITY backend/crates/apps/coffret-server/tests/routes.rs`;
      and it still says nothing about the key —
      `a_refusal_never_says_what_the_key_is` passes unchanged.
- [x] Existing backend, frontend and interoperability checks continue to pass,
      and the E2E journeys still start, kill and restart their server without
      meeting the new refusal (the CI `e2e` job).

### Manual / on-hardware (verified by a human before merge)

- [ ] Start `coffret-server` over a Library, then start a second one over the
      same Library on another port from another terminal: the second refuses
      before it asks for the Passphrase, and says which process is already
      serving it. The first keeps answering the explorer in the browser.
- [ ] Kill the first server with `SIGKILL` and start another over the same
      Library: it starts, without anything having been deleted by hand.
- [ ] Point a browser at the server through a hostname that resolves to
      `127.0.0.1` and read the 403: it names the address the server is really
      at.

## Out of scope

The key itself does not change: `ServerKey::publish` still draws 32 bytes from
the CSPRNG and still writes the file unconditionally — what is added is the
exclusion above it, so LA-3 and LA-4 keep their current code and their current
tests. The three admission fences are untouched: the same requests are admitted
and refused as before, and only the `Elsewhere` sentence changes. The lock
file does not learn which address the first server bound — the process is
enough to find and stop. No lock over anything else the Library directory
holds: `coffret sync` running beside a server is an arrangement the E2E stage
exercises on purpose and it stays as it is. Renaming the header constant on
both sides, moving the E2E script to `--fail-with-body`, and the two text
defects in the frontend are a separate change.
