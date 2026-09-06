---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && test -f backend/crates/gateway/coffret-local-fs/Cargo.toml && grep -q 'coffret-local-fs' Makefile && grep -q 'pub trait Spool' -r backend/crates/domain/coffret-usecase/src && [ -z \"$(grep -rlE 'tokio::fs|std::fs' backend/crates/domain/coffret-usecase/src | grep -vE '/(local_scan|fetch|sync_conformance|freeze_conformance|fetch_conformance)/|/local_times\\.rs$')\" ] && ! grep -rqE 'tokio::fs|std::fs' backend/crates/domain/coffret-usecase/src/sync_conformance/interruption.rs && grep -rq 'a_spool_that_cannot_be_created_leaves_a_spooling_row_and_uploads_nothing' backend/crates/domain/coffret-usecase && grep -rq 'a_spool_flush_that_fails_leaves_a_spooling_row_and_uploads_nothing' backend/crates/domain/coffret-usecase && grep -rq 'a_pack_spool_flush_that_fails_leaves_a_spooling_row_and_uploads_nothing' backend/crates/domain/coffret-usecase && grep -rq 'a_discard_that_fails_after_a_commit_keeps_the_commit' backend/crates/domain/coffret-usecase && grep -rq 'spool_conformance' backend/crates/gateway/coffret-local-fs"
assignee: null
branch: task/0906-1928-move-the-spools-filesystem-writes-behind-a-spool-capability-in-a-local-fs-gateway
created_at: 2026-09-06T19:28:08Z
updated_at: 2026-09-06T23:25:27Z
---

# feat(backend): move the spool's filesystem writes behind a Spool capability in a new local-fs gateway

## Overview

`coffret-usecase` reaches Storage through the `ObjectStore` port and the
device-local catalog through the `Index` port, but it writes the spool — the
encrypted Containers waiting to be uploaded — with `tokio::fs` directly:
`spool_file.rs` (`File::create`, `write_all`, `sync_all`, `remove_file`),
`sync/run.rs` and `freeze/run.rs` (`create_dir_all` of the spool directory),
`upload/run.rs` (`File::open` of a spool to stream it), and
`sync/reconcile.rs` (discarding what an interrupted run left). Because of
that, the recovery rules the flows promise around the spool — the pending row
is recorded before the spool file exists (spec: OC-2), the row flips to
`Spooled` only after the flush, an abandoned spool is disposed of idempotently
(spec: OC-6) — cannot be tested against a filesystem that fails at a chosen
step. The conformance suites inject Storage faults (`mangling_store`) and Index
faults (a refused refresh, a refused `mark_spooled`), but not one filesystem
fault.

This task is the first of a series that moves every local-filesystem
operation out of the use-case crate and behind capability traits the use-case
crate defines, implemented by a new gateway crate. The series follows the same
shape as the two existing ports: the trait and an in-memory fake live in
`coffret-usecase`, the concrete implementation lives under
`backend/crates/gateway/`, and `make deps` enforces the dependency direction.
This task does the spool alone — the smallest lifecycle — and establishes the
crate, the fake, and the fault-injection pattern the next tasks (the mapped
roots a scan reads, the confined placement a fetch writes) build on.

### What to build

**1. The `Spool` capability in `coffret-usecase`.** A `pub trait Spool: Send +
Sync` (`async_trait`, like `ObjectStore`) with:

- `prepare_dir(&self, dir: &Path)` — makes the spool directory and the folders
  above it where they are missing (what `create_dir_all` does today in
  `sync/run.rs:89` and `freeze/run.rs:101`).
- `create(&self, path: &Path) -> Box<dyn SpoolWriter>` — opens a spool file for
  writing, replacing anything at that path (today's `File::create`).
- `open(&self, path: &Path) -> Box<dyn AsyncRead + Send + Unpin>` — the reader
  an upload streams a finished spool from (today's `fs::File::open` in
  `upload/run.rs:50`; it goes into `ByteStream::new(len, reader)`).
- `discard(&self, path: &Path)` — removes a spool file; a file that is already
  gone is success (today's `spool_file::discard`).

and a `pub trait SpoolWriter: Send` with `write(&mut self, bytes: &[u8])` and
`finish(self: Box<Self>)`, where `finish` flushes to the device (`sync_all`)
and consumes the writer, so a spool can only be handed on once it is durable.

The BLAKE3 and MD5 digests, `WRITE_CHUNK`, and the `Digests` value stay in the
use-case crate: they are what the Journal record and the provider comparison
need, not a filesystem promise. Keep `SpoolFile` as the use-case wrapper that
folds the digests in around a `Box<dyn SpoolWriter>`.

Every method fails with one new public error, `LocalIoError { operation:
LocalOperation, path: PathBuf, cause: io::Error }` — the three fields
`LocalError::Io` carries today, as a struct a gateway can construct. Implement
`Display`, `std::error::Error` (`source` = the cause) and `Redacted` (the
operation and the cause's `kind()` only — never the path). `LocalError::Io`
becomes a wrapper around it (or is built from it via `From`); the flows' public
errors (`SyncError::Io`, `FreezeError::Io`) keep their shape so callers see no
change. The use-case crate must not branch on `io::ErrorKind` to interpret a
`Spool` answer: absence on `discard` is the gateway's to swallow, so the trait's
contract states it and the use case simply calls `discard`.

**2. The gateway crate `backend/crates/gateway/coffret-local-fs`.** Add it to
the workspace (`[workspace.dependencies]` in `backend/Cargo.toml`) with
`coffret-usecase`, `coffret-model`, `async-trait`, `tokio`, `tracing` as
dependencies (`rustix` is not needed yet). One concrete provider, `pub struct
UnixFs`, implements `Spool` by moving the bodies out of `spool_file.rs`,
`upload/run.rs` and the two `create_dir_all` sites. The crate has
`#![forbid(unsafe_code)]` and `#![warn(missing_docs)]` like every other, and a
crate doc that says what it is: the device's own disk, reached through the
capabilities the use-case crate names, and the one place the operating system's
filesystem API is called.

**3. The fake: `InMemoryFs` in `coffret-usecase`.** Behind
`#[cfg(any(test, feature = "conformance"))]`, next to `InMemoryStore` and
`InMemoryIndex`, implementing `Spool` over an in-memory map from `PathBuf` to
file content (plus which directories exist, so `create` under a directory that
was never prepared fails the way a real one would). It carries a fault script:
`fail_on(&self, operation: LocalOperation, nth: usize)` makes the `nth`
(1-based) invocation of that operation — `Creating`, `Writing`, `Flushing`,
`Removing`, `Reading` — return a `LocalIoError` whose cause has
`io::ErrorKind::Other`. Expose what the cases need to read the fake's state:
the files under a directory and one file's bytes.

**4. Thread the capability through the flows.** `SyncRequest` and
`FreezeRequest` gain a `spool: &'a dyn Spool` parameter (keep the `spool_dir`
path); `upload::upload`, `sync::reconcile`, `sync::spool`, `freeze::spool`
and the two `run` functions call the capability instead of `tokio::fs`. Nothing
in `spool_file.rs`, `upload/`, `sync/run.rs`, `freeze/run.rs` or
`sync/reconcile.rs` may name `tokio::fs` or `std::fs` afterwards. The scan
side (`local_scan/`, `local_times.rs`) and the fetch side (`fetch/`) keep their
direct calls for now — they are the next two tasks.

**5. Fix one posture while the code is open.** Today `sync/run.rs` and
`freeze/run.rs` do `spool_file::discard(...).await?` *after* the commit landed
and the commit's refresh already dropped the pending rows. A discard that fails
there fails the whole run with `Io` even though the Library has changed, and
leaves a spool file no row names. Change both to the posture `fetch`'s
`discard_all` already takes: a cleanup failure after a successful commit is
recorded with `warn!` (redacted — no path) and does not fail the run; the
outcome reports the commit. Say so in the doc comment.

**6. Conformance fixtures run the spool on the fake.** `SyncUnderTest`,
`FreezeUnderTest` and `FetchUnderTest` construct an `InMemoryFs` themselves
and hand it to every request they build; their constructors drop the spool
directory argument (the fixture picks a fixed spool path inside the fake). The
mapped folder stays a real directory for now. Rewrite the helpers that touch
the spool directly — `fixtures::spooled` (counts spool files) and
`interruption::interrupted` (plants a spool file beside a row) — to go through
the fake. Update the three fixture call sites in `coffret-usecase/tests/` and
`gateway/s3-store/tests/`, and `tests/fetch_confinement.rs`.

**7. Composition root.** `coffret-device`'s `OpenLibrary` gains a
`local_fs: Arc<UnixFs>` built in `open_library`, and `run_sync.rs` /
`run_freeze.rs` pass it as `&dyn Spool`. `coffret-device` depends on the new
crate; the apps above it must not (see 8). Its test support (`testing/mod.rs`)
uses whichever fits.

**8. Dependency gate.** In the `Makefile`, add `coffret-local-fs` to the
`gateways` list in `deps` (a gateway meets the rest of the backend at a port)
and to `APP_FORBIDDEN` (apps reach the domain through `coffret-device`). Add a
line to the "Repository layout" section of `CLAUDE.md` only if the layout
sentence there would otherwise be wrong; the fuller doc pass is a later task.

**9. Failure-injection cases in `coffret-usecase`.** Write them where the
crate's own flow tests live (a new test module under `sync_conformance` /
`freeze_conformance` that only the in-memory fixture runs is acceptable, or
`tests/`; they need the fake's script, so they are not backend-agnostic).
Each drives a real `sync_folders` / `freeze_folder` over `InMemoryStore`,
`InMemoryIndex`, a real temp folder with one or two files, and an `InMemoryFs`
scripted to fail, then asserts the returned error variant *and* the state left
behind: the pending row's presence and `SpoolState`, the store holding no
object, the fake holding (or not) a spool file. Use these exact names:

- `a_spool_that_cannot_be_created_leaves_a_spooling_row_and_uploads_nothing`
  (sync; `Creating` fails) — the error is `SyncError::Io` with
  `LocalOperation::Creating`; one row, `Spooling`, `object_ref: None`; the
  store lists nothing; and the *next* run (unscripted) disposes of the row and
  commits the file once.
- `a_spool_write_that_fails_leaves_a_spooling_row_and_uploads_nothing` (sync;
  `Writing` fails).
- `a_spool_flush_that_fails_leaves_a_spooling_row_and_uploads_nothing` (sync;
  `Flushing` fails) — the file exists in the fake, the row is still
  `Spooling`, nothing was uploaded; the next run disposes of it.
- `a_pack_spool_flush_that_fails_leaves_a_spooling_row_and_uploads_nothing`
  (freeze; the same for a Pack).
- `a_discard_that_fails_after_a_commit_keeps_the_commit` (sync; `Removing`
  fails on the post-commit cleanup) — the run returns `Ok`, the outcome
  carries the commit, the catalog holds the Entry, no pending row remains, and
  the spool file is still in the fake.

**10. The gateway's own tests.** A `spool_conformance` test in
`coffret-local-fs` (under `tests/`) exercises `UnixFs` as a `Spool` against a
`tempfile` directory: prepare, create, write, finish, open reads back the same
bytes, discard removes, discard again succeeds, and `create` under a directory
that was never prepared fails with `LocalOperation::Creating`. Run the same
assertions against `InMemoryFs` from the use-case crate's tests, so the fake
and the real gateway are held to one contract (a shared helper behind the
`conformance` feature is fine; name it `spool_conformance`).

### Conventions

- `make check` is the gate; run it before finishing. `cargo test` compiles the
  s3-store integration tests too (they skip without MinIO), so the fixture
  signature change is compile-checked there.
- Documentation, comments, commit and PR text in English; Conventional Commits.
  Doc comments explain the reason, as the surrounding code does. Cite the spec
  register (`spec: OC-2`) where a rule comes from it.
- Error types: no `PartialEq`, causes kept as values (never `to_string()`),
  variants named as nouns, conversions explicit at the boundary.
- Do not leave `allow(dead_code)`; delete what the move makes unused.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `backend/crates/gateway/coffret-local-fs` exists in the workspace with
      `UnixFs` implementing `coffret_usecase::Spool`, and `make deps` lists it
      among the gateways and in `APP_FORBIDDEN` (`grep -q 'coffret-local-fs'
      Makefile` is appended to the check).
- [x] `coffret-usecase` defines `pub trait Spool` and `pub trait SpoolWriter`
      with the four operations above, failing with `LocalIoError`; the use
      case never inspects `io::ErrorKind` to interpret a `Spool` answer.
- [x] No production file of `coffret-usecase` outside `local_scan/`,
      `local_times.rs`, `fetch/` and the three conformance directories names
      `tokio::fs` or `std::fs`, and `sync_conformance/interruption.rs` names
      neither (both greps are appended to the check).
- [x] `SyncRequest` and `FreezeRequest` take `&dyn Spool`; `upload`,
      `reconcile` and the two spool steps write, read and discard through it.
- [x] A discard failure after a successful commit no longer fails a sync or a
      freeze: it is logged (redacted) and the outcome carries the commit.
- [x] `InMemoryFs` exists behind the `conformance` feature with a
      `fail_on(operation, nth)` script, and the five named failure-injection
      cases exist and pass (their names are grepped by the check).
- [x] The three conformance fixtures build their own `InMemoryFs` and no
      longer take a spool directory; the in-memory and s3-store fixture call
      sites compile.
- [x] `coffret-local-fs` has a `spool_conformance` test over `UnixFs`
      (grepped by the check), and the same assertions run over `InMemoryFs`.
- [x] `coffret-device` builds `UnixFs` in `open_library` and passes it to the
      sync and freeze requests; `make check` (which includes `make deps`) is
      green.
