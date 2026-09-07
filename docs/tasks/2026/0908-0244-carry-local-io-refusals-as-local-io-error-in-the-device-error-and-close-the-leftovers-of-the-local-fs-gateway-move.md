---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, error-type-design]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "Local(LocalIoError)" backend/crates/apps/coffret-device/src/error.rs && ! grep -rq "doing: &.static str" backend/crates/apps/coffret-device/src && ! grep -rq "fn state(&self)" backend/crates/domain/coffret-usecase/src/in_memory_fs/in_memory_destination.rs backend/crates/domain/coffret-usecase/src/in_memory_fs/in_memory_scratch_file.rs backend/crates/domain/coffret-usecase/src/in_memory_fs/in_memory_flushed_file.rs backend/crates/domain/coffret-usecase/src/in_memory_fs/in_memory_writer.rs backend/crates/domain/coffret-usecase/src/in_memory_fs/in_memory_source_reader.rs && grep -q "a_stamp_that_fails_discards_the_scratch_and_fails_the_fetch" backend/crates/domain/coffret-usecase/tests/place_faults.rs && grep -q "probing_a_root_that_is_a_regular_file_answers_and_leaves_the_refusal_to_the_listing" backend/crates/domain/coffret-usecase/src/mapped_roots_conformance/probing.rs && grep -q "the device.s own filesystem" CLAUDE.md && grep -B1 "^tempfile" backend/crates/apps/coffret-interop/Cargo.toml | grep -q "^#" && grep -B1 "^tempfile" backend/crates/gateway/google-drive-store/Cargo.toml | grep -q "^#" && grep -B1 "^tempfile" backend/crates/apps/coffret-server/Cargo.toml | grep -q "^#" && ! grep -q "only under the directories the backend hands it" backend/crates/domain/coffret-usecase/src/freeze_conformance/mod.rs && ! grep -q "only under the one directory the backend hands it" backend/crates/domain/coffret-usecase/src/sync_conformance/mod.rs'
assignee: null
branch: task/0908-0244-carry-local-io-refusals-as-local-io-error-in-the-device-error-and-close-the-leftovers-of-the-local-fs-gateway-move
created_at: 2026-09-07T17:44:54Z
updated_at: 2026-09-07T22:12:27Z
---

# refactor(backend): carry local I/O refusals as LocalIoError in the device error and close the leftovers of the local-fs gateway move

## Overview

The three local-disk capabilities — `Spool`, `MappedRoots`, `Destinations` —
are in place, `UnixFs` in `coffret-local-fs` answers all of them, and
`coffret-usecase` makes no filesystem call of its own. This task closes what
the three moves left open: a device error that still carries local refusals as
prose, a fake whose five handle types each repeat the same lock helper, two
conformance cases the reviews found missing, three `tempfile` dev-dependencies
without the why-comment every other one has, two conformance module docs that
still describe real directories, and a repository layout note
that still calls every gateway "external I/O". Nothing here changes what the
flows do; the one behaviour visible from outside is the wording of a device
error message, which now comes from `LocalIoError` instead of a hand-written
phrase.

### What to build

**1. `coffret-device::Error::Local` carries a `LocalIoError`.** Today the
variant is `Local { doing: &'static str, path: PathBuf, cause: io::Error }`,
and two places — `Error::descent` and `add/added_locally.rs` — each write a
`match refused.operation { ... => "a file could not be ..." }` table to turn a
`LocalOperation` back into prose, while the composition-root callers
(`owner_only.rs`, `staging.rs`, `library_files.rs`, `device_settings/read.rs`,
`stored_master_key_file.rs`) pass their own phrase. The variant becomes
`Local(LocalIoError)`: the operation, the path and the `io::Error` travel as
the values they are (`coffret_usecase::LocalIoError`), the two tables go, and a
caller that reads which operation failed matches on `LocalOperation` instead
of a sentence.

- `impl From<LocalIoError> for Error` — the same failure, mechanically, so
  `?` is right here; `Error::descent` keeps its explicit `match` because the
  `Blocked` arm changes meaning (to `FetchError::UnmaterializablePath`) and
  only the `Io` arm becomes `Self::Local(refused)`.
- `Error::local(doing, path)` becomes `Error::local(operation: LocalOperation,
  path)` returning the same `impl FnOnce(io::Error) -> Self` shape, building a
  `LocalIoError::new(operation, path, cause)` inside.
- `owner_only::write_file` / `create_dir` / `create_empty_file` / `create`
  lose their `doing: &'static str` parameter: each step names its own
  operation (`Creating` for the exclusive create and the directory,
  `Writing` for `write_all`, `Flushing` for `sync_all`, `Renaming` for the
  rename into place). Their callers (`library_files.rs`, the settings and
  Master Key writers, staging) drop the phrase they passed. `staging.rs`'s
  `remove_dir_all` is `Removing`; its `publish` rename is `Renaming`;
  `device_settings/read.rs` and `stored_master_key_file.rs` are `Reading`.
- `Display` for `Local` delegates to `LocalIoError`'s `Display` (which already
  says what the operation was and names the path for the person); `redacted()`
  becomes `format!("Device::Local: {}", refused.redacted())`, the same
  chaining shape `DescentError::Io` uses; `source()` returns the `io::Error`
  through the value as before.
- `add/added_locally.rs` stops rewording: the `map_err` becomes
  `Error::from` (or `?`), because the operation the refusal carries already
  says whether it was the folder's listing or one child's stat, and the path
  it carries names which — the comment there explaining why the two are told
  apart moves to sit beside the `LocalOperation` doc it now relies on, or goes
  if that doc already says it.
- Keep the doc comments' reasoning; update the variant's doc to say the value
  is the use case's own `LocalIoError` and why (one vocabulary for the device's
  disk whichever crate touched it — `local_operation.rs` already promises
  this).

**2. One lock helper in the fake.** `in_memory_fs/mod.rs`,
`in_memory_writer.rs`, `in_memory_source_reader.rs`, `in_memory_destination.rs`,
`in_memory_scratch_file.rs` and `in_memory_flushed_file.rs` each define the same
`fn state(&self) -> MutexGuard<'_, State>` with the same three-line doc about
poisoned locks. Replace the six with one `pub(in crate::in_memory_fs) fn
lock(state: &Mutex<State>) -> MutexGuard<'_, State>` in `state/mod.rs`, carrying
the doc once; the six call sites become `lock(&self.state)`. Behaviour identical.

**3. Two conformance cases the reviews found missing.**

- `tests/place_faults.rs`:
  `a_stamp_that_fails_discards_the_scratch_and_fails_the_fetch` (`fail_on(Stamping, 1)`)
  — `FetchError::Io` with `LocalOperation::Stamping`; the scratch is gone, no
  final file, no local row. Same shape as the `Flushing` case beside it.
- `mapped_roots_conformance/probing.rs`:
  `probing_a_root_that_is_a_regular_file_answers_and_leaves_the_refusal_to_the_listing`
  — arrange a regular file at the root path; `probe_root` answers `Ok(Some(_))`
  (the probe follows links and stats what is there; a file is there), and
  `list_folder` on it is refused with `LocalOperation::Listing`. This pins the
  contract `MappedRoots::probe_root`'s doc states, over both `InMemoryFs` and
  `UnixFs`; add the case to the macro's list in `mapped_roots_conformance/mod.rs`.

**4. Three `tempfile` why-comments.** Every other `tempfile` dev-dependency in
the workspace says what its directory is for; these three do not:

- `backend/crates/apps/coffret-interop/Cargo.toml` — what the interop cases
  write under a temporary directory (read the tests to say it precisely).
- `backend/crates/gateway/google-drive-store/Cargo.toml` — the folder the
  freeze and fetch suites place into when they run against Google Drive, the
  same reason `s3-store/Cargo.toml` gives (mirror that comment's wording).
- `backend/crates/apps/coffret-server/Cargo.toml` — what the server's tests
  need a temporary directory for (read them; the state directory a case runs
  under, most likely — say what is true).

**5. Repository layout note.** `CLAUDE.md`'s layout bullet says `gateway/`
is "external I/O". Since `coffret-local-fs` the layer also holds the provider
for the device's own filesystem, which is not external. Reword the parenthesis
to name both: the storage services and the device's own filesystem, behind
the ports and capabilities `domain/` declares. Keep it to one line.

**6. Two conformance module docs that still describe real directories.**
`freeze_conformance/mod.rs` (lines 42–45) says the suite writes "only under
the directories the backend hands it", and `sync_conformance/mod.rs` (lines
39–42) says the sync suite reads a folder "which the other three suites do
not" and writes "only under the one directory the backend hands it". Both
fixtures make one `InMemoryFs` and put every folder in it (their own docs say
so), so neither sentence is true. Reword each in the shape the fetch suite's
module doc now uses — every file is in the in-memory disk the fixture makes
rather than on a directory the backend hands it — and drop the "other three
suites" contrast. Comment text only.

### Conventions

- `make check` is the gate; run it before finishing. On macOS, exporting
  `SSL_CERT_FILE=/etc/ssl/cert.pem` avoids a keychain flake in
  `coffret-device`'s tests, and `CC=/usr/bin/cc CXX=/usr/bin/c++` keeps
  `aws-lc-sys` off a non-system toolchain that cannot build it. Do not run two
  cargo invocations against the same `target/` concurrently, and do not leave
  background wait loops running.
- Documentation, comments, commit and PR text in English; Conventional Commits.
  Doc comments explain the reason and cite the spec register where a rule
  comes from it; keep the existing rationale where code moves.
- Error types: no `PartialEq`, causes kept as values, conversions explicit
  where the meaning changes and `From` where it does not; tests assert with
  `matches!`.
- Delete what the change makes unused; leave no `allow(dead_code)`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `coffret-device::Error::Local` is `Local(LocalIoError)`; no
      `doing: &'static str` remains anywhere under `coffret-device/src`
      (both grepped by the check).
- [x] None of the fake's five handle / reader / writer modules defines its own
      `fn state(&self)`; the lock helper lives once in `in_memory_fs/state/`
      (grepped by the check).
- [x] `a_stamp_that_fails_discards_the_scratch_and_fails_the_fetch` exists in
      `place_faults.rs` and passes;
      `probing_a_root_that_is_a_regular_file_answers_and_leaves_the_refusal_to_the_listing`
      exists in `mapped_roots_conformance/probing.rs` and passes over both
      backends (grepped by the check; run by `make check`).
- [x] The three `tempfile` dev-dependencies (`coffret-interop`,
      `google-drive-store`, `coffret-server`) each have a comment line directly
      above them (grepped by the check).
- [x] `CLAUDE.md`'s layout bullet names the device's own filesystem as part of
      what `gateway/` provides (grepped by the check).
- [x] `freeze_conformance/mod.rs` and `sync_conformance/mod.rs` no longer say
      the suite writes only under directories the backend hands it (grepped by
      the check).
- [x] `make check` (including `make deps`) is green.
