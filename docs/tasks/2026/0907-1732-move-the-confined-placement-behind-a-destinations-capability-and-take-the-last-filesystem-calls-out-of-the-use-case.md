---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -rq "pub trait Destinations" backend/crates/domain/coffret-usecase/src && grep -rq "impl Destinations for UnixFs" backend/crates/gateway/coffret-local-fs/src && [ -z "$(grep -rlE "tokio::fs|std::fs|rustix|MetadataExt|spawn_blocking" backend/crates/domain/coffret-usecase/src)" ] && ! grep -q "rustix" backend/crates/domain/coffret-usecase/Cargo.toml && grep -q "rustix" backend/crates/gateway/coffret-local-fs/Cargo.toml && test -f backend/crates/gateway/coffret-local-fs/tests/fetch_confinement.rs && ! test -f backend/crates/domain/coffret-usecase/tests/fetch_confinement.rs && grep -rq "destinations_conformance" backend/crates/gateway/coffret-local-fs/tests && ! grep -rqE "tokio::fs|std::fs|rustix" backend/crates/apps/coffret-device/src/add/incoming_file.rs backend/crates/apps/coffret-device/src/add/added_at.rs && grep -rq "a_scratch_that_cannot_be_created_fails_the_fetch_and_leaves_nothing_behind" backend/crates/domain/coffret-usecase && grep -rq "a_write_that_fails_discards_the_scratch_and_fails_the_fetch" backend/crates/domain/coffret-usecase && grep -rq "a_flush_that_fails_discards_the_scratch_and_fails_the_fetch" backend/crates/domain/coffret-usecase && grep -rq "a_publish_that_fails_takes_the_scratch_with_it_and_fails_the_fetch" backend/crates/domain/coffret-usecase && grep -rq "a_discard_that_fails_after_a_failed_write_is_logged_and_the_write_failure_is_reported" backend/crates/domain/coffret-usecase && grep -rq "an_index_refusal_after_publish_leaves_the_placed_file_where_it_is" backend/crates/domain/coffret-usecase && ! grep -q "is not a port for the reason" backend/crates/domain/coffret-usecase/src/lib.rs'
assignee: null
branch: task/0907-1732-move-the-confined-placement-behind-a-destinations-capability-and-take-the-last-filesystem-calls-out-of-the-use-case
created_at: 2026-09-07T08:32:39Z
updated_at: 2026-09-07T16:10:24Z
---

# feat(backend): move the confined placement behind a Destinations capability and take the last filesystem calls out of the use case

## Overview

The spool (`Spool`) and the mapped-folder walk (`MappedRoots`) already reach
the disk through the `coffret-local-fs` gateway (`UnixFs`) and the `InMemoryFs`
fake. The fetch's *placement* still calls the operating system directly, and
it is the most security-sensitive part of the three: `fetch/confined_dir/`
descends from a mapped root one Entry Path component at a time with `openat` +
`O_NOFOLLOW | O_DIRECTORY` (spec: EP-4, EP-11), creates the scratch file with
`O_CREAT | O_EXCL | O_NOFOLLOW`, publishes with `renameat` against the open
folder, and removes with `unlinkat` (all `rustix`); `fetch/placement.rs` writes
through a `tokio::fs::File`, flushes with `sync_all`, stamps with `set_times`
on the handle, and `fetch/local_place.rs` runs the descent on
`spawn_blocking`. So the interruption windows EP-11 is about — a write that
fails, a flush that fails, a rename that fails, a cleanup that fails, an Index
update that fails after the file is already visible — cannot be opened at a
chosen point in a test, and `tests/fetch_confinement.rs` proves the confinement
only through the whole fetch flow on a real disk.

This task moves the placement behind a `Destinations` capability, the third
and last, and with it the last `tokio::fs` / `std::fs` / `rustix` use leaves
`coffret-usecase`. What stays in the use case: which Entry to place and where
(`translate`, EP-9), whether the device may write there (`select`, EP-10/EP-11),
the scratch-name convention (`scratch`), the order write → flush → verify the
plaintext hash against the catalog → stamp → publish → `mark_present`, and every
verdict (`Surfaced::UnreachablePlace`, `FetchError::UnmaterializablePath`,
`ContentMismatch`). What moves: how a folder is reached without following links,
how a scratch file is made exclusively, how bytes get to the device, how a name
is stamped and renamed against an open folder, and how an errno is read.

### What to build

**1. The `Destinations` capability in `coffret-usecase`**, beside `Spool` and
`MappedRoots`:

- `pub trait Destinations: Send + Sync` (`async_trait`) with
  `reach(&self, root: &Path, components: &[String]) -> Result<Box<dyn Destination>, DescentError>`
  — descends from the mapped root through every component but the last,
  making the folders that are not there yet (spec: EP-2) and refusing anything
  that is not a real folder of that root with `DescentError::Blocked { path }`
  (spec: EP-4, EP-11); the last component is the file's own name, kept on the
  returned `Destination`. And
  `look_up(&self, root: &Path, components: &[String]) -> Result<Option<Standing>, DescentError>`
  — the same walk making nothing: `None` where a folder on the way or the file
  itself is absent, `Blocked` where a link or a non-folder is in the way, and
  otherwise what stands at the name, stated without following links
  (spec: EP-8).
- `pub trait Destination: Send` — the folder the descent left open, holding
  the file's name: `create(&self, scratch_name: &str) -> Result<Box<dyn ScratchFile>, DescentError>`
  (exclusive: a name that exists is a refusal, `LocalOperation::Creating`; a
  link that took the name first is refused rather than followed),
  `remove(&self, name: &str) -> Result<(), DescentError>` (a name that is already
  gone is success; **synchronous**, so a drop guard can call it), and
  `path_of(&self, name: &str) -> PathBuf` (for a message, never to reach the
  disk).
- `pub trait ScratchFile: Send` with `write(&mut self, bytes: &[u8])` and
  `flush(self: Box<Self>) -> Result<Box<dyn FlushedFile>, DescentError>`;
  `pub trait FlushedFile: Send` with `stamp(&mut self, mtime: Mtime)` and
  `publish(self: Box<Self>) -> Result<(), DescentError>` (rename onto the
  destination's final name, within the same folder). The type transition is
  the contract: nothing can be published before it has been flushed, and
  nothing can be written after. Which of these are `async` is the gateway's
  need (the write and flush are; the rename and unlink are one syscall each
  and may stay synchronous; the stamp runs the way the gateway chooses) — say
  in each doc which and why.
- `Standing { size, mtime, is_file }` (today `fetch/standing.rs`, crate-private)
  becomes the port's public answer to `look_up`. `DescentError` (today
  `fetch/descent_error.rs`) becomes the port's error, at the crate root beside
  `LocalIoError`, with `Io(LocalIoError)` replacing the three inline fields —
  `FetchError::from_descent` and `coffret-device`'s `Error::descent` adapt. The
  `ELOOP` / `ENOTDIR` / `EMLINK` reading that makes a `Blocked` moves into the
  gateway; the use case never sees an errno or an `io::ErrorKind`.
- `LocalPlace::descend` and `LocalPlace::look` take `&dyn Destinations` and
  lose their `spawn_blocking`. `fetch/placement.rs` (`Placement`) keeps its
  role — hashing the plaintext as it passes, the `ContentMismatch` verdict, the
  stamp before the rename, `mark_present` after it, the cleanup posture of
  `discard_all` — over a `Box<dyn Destination>` and the scratch/flushed
  handles. `fetch/confined_dir/` is deleted from the use case; `local_times.rs`
  goes with it (`system_time_of` moves to the gateway's `local_times.rs`).
- `FetchRequest` and `FetchEntryRequest` gain `destinations: &'a dyn Destinations`;
  `fetch_folders`, `fetch_entry`, `select`, `container::fetch`, `range_read`
  and `Placement::open` take it.

**2. `UnixFs` implements `Destinations`** in `coffret-local-fs`, moving the
bodies of `fetch/confined_dir/*` (the `rustix` calls, `open_root` following
links for the root alone, `enter_or_make`, `enter`, `refusal`), the
`spawn_blocking` wrapping, `create` / `publish` / `remove`, the `set_times`
stamp with `system_time_of`, and their doc comments. `rustix` moves from
`coffret-usecase/Cargo.toml` to `coffret-local-fs/Cargo.toml` (keep the
why-comment). Unix only, as today, and say so where the crate does.

**3. `InMemoryFs` implements `Destinations`.** `reach` creates missing folders
in the tree and answers `Blocked` where a component is a file or a planted
"other" (the fake's stand-in for a symbolic link); `look_up` answers `None`
for an absent folder or file, `Blocked` for something in the way, and a
`Standing` otherwise (`is_file` false for a folder or an "other" standing at
the name). The fake's `Destination` / `ScratchFile` / `FlushedFile` share the
tree: `create` refuses an existing name, `write` appends, `flush` marks the
file durable, `stamp` sets its mtime, `publish` renames it onto the final name
(replacing what stood there, as `renameat` does), `remove` deletes and treats
absence as success. Extend `fail_on` to `Creating` (scratch), `Writing`,
`Flushing`, `Stamping`, `Renaming`, `Removing` on this path — the spool and
the placement share one counter per operation; say so in the doc, and keep the
existing `spool_faults` / `scan_faults` cases green.

**4. Fixtures and the confinement tests.** `FetchUnderTest` and
`FreezeUnderTest` drop their real `target_folder`: the target device places
into the same `InMemoryFs` (`/target`), and `fetch_conformance/fixtures/files.rs`
(`place`, `read`, `unplace`, `exists`, `observed`, `scratch_left`) is rewritten
over the fake's API; the six fixture call sites in `coffret-usecase/tests/`
and `gateway/s3-store/tests/` drop the last tempdir. `tests/fetch_confinement.rs`
(seven cases about symbolic links and files where folders must be) **moves to
`coffret-local-fs/tests/fetch_confinement.rs`** and runs the real `UnixFs` for
roots, spool and destinations against temporary directories — it is a test of
the real filesystem's confinement and belongs with the gateway. Its assertions
stay as they are.

**5. `destinations_conformance`**, the shared suite behind the `conformance`
feature (a `DestinationsUnderTest` with a `Box<dyn Destinations>`, a
`FolderArrangement`-style trait for arranging the root, and a way to read a
file back), run over `InMemoryFs` in `coffret-usecase/tests/` and over `UnixFs`
in `coffret-local-fs/tests/destinations_conformance.rs`. Cases: `reach` makes
the folders on the way and the file lands where the components say; `reach`
is `Blocked` at a component that is not a folder (a planted other / a symbolic
link, and a regular file where a folder must be), naming that component;
`look_up` of an absent place is `None`; `look_up` of a file reports its size,
mtime and `is_file`; `create` of a name that exists is refused as `Creating`;
write → flush → stamp → publish makes the content visible at the final name
with the stamped mtime and the scratch name gone; `publish` replaces a file
standing at the final name; `remove` of a name that is already gone succeeds.
The symbolic-link case is Unix-only and says so.

**6. Fault-injection cases in `coffret-usecase/tests/place_faults.rs`**, in
the shape of `spool_faults.rs` (a `Device` over `InMemoryStore`,
`InMemoryIndex`, one `InMemoryFs` for roots, spool and destinations; a source
device syncs one or two files, a target device fetches). Each asserts the
returned error variant *and* the state left behind (the target folder in the
fake — no scratch file, no final file unless stated — and the target Index's
local rows). Use these exact names:

- `a_scratch_that_cannot_be_created_fails_the_fetch_and_leaves_nothing_behind`
  (`Creating` fails on the scratch) — `FetchError::Io` with
  `LocalOperation::Creating`; the target folder holds nothing.
- `a_write_that_fails_discards_the_scratch_and_fails_the_fetch` (`Writing`) —
  `FetchError::Io` with `Writing`; the scratch is gone, no final file, no
  local row.
- `a_flush_that_fails_discards_the_scratch_and_fails_the_fetch` (`Flushing`) —
  the same shape with `Flushing`.
- `a_publish_that_fails_takes_the_scratch_with_it_and_fails_the_fetch`
  (`Renaming`) — `FetchError::Io` with `Renaming`; neither the scratch nor the
  final file remains; no local row.
- `a_discard_that_fails_after_a_failed_write_is_logged_and_the_write_failure_is_reported`
  (`Writing` fails, then the `Removing` of that scratch fails) — the error is
  the `Writing` one; the run logged a redacted `warn!` about the scratch it
  could not remove (assert on `CapturedLogs` as `spool_faults.rs` does, and
  that no path reaches the log); the scratch is still in the fake.
- `an_index_refusal_after_publish_leaves_the_placed_file_where_it_is` (the
  target Index refuses `mark_present` after the rename) — the fetch fails with
  `FetchError::Index`, and the verified file stays at its final name, because
  a bookkeeping failure never removes content the device verified.

**7. `coffret-device`.** `add/incoming_file.rs` is rewritten over the port:
`receive_file` reaches the destination through `LocalPlace::descend(self.local_fs.as_ref())`,
`IncomingFile` holds a `Box<dyn Destination>` and a `Box<dyn ScratchFile>`,
`keep` flushes and publishes, and the drop guard removes the scratch through
`Destination::remove` (synchronous). `add/added_at.rs` answers its "is a
regular file standing there" question through `Destinations::look_up` instead
of `fs::metadata`. `Error::descent` follows the new `DescentError` shape.
Everything else in `coffret-device` that touches the filesystem (the Library
directory, settings, staging, tests) is composition-root code and is not part
of this change.

**8. The crate doc of `coffret-usecase`** (`lib.rs`) still says the three flows
"touch the local filesystem, which is not a port for the reason a device's own
disk is not Storage". That sentence is now false. Rewrite that paragraph:
Storage is behind a port because of the trust boundary; the device's own disk
is behind capabilities (`MappedRoots`, `Spool`, `Destinations`) because the
recovery rules around an interrupted run can only be tested against a disk
that fails at a chosen step; the gateway is the one place the operating system
is asked. Update the paragraph that lists the ports and the one that lists the
conformance suites and fakes accordingly.

### Conventions

- `make check` is the gate; run it before finishing. On macOS, exporting
  `SSL_CERT_FILE=/etc/ssl/cert.pem` avoids a keychain flake in
  `coffret-device`'s tests, and `CC=/usr/bin/cc CXX=/usr/bin/c++` keeps
  `aws-lc-sys` off a non-system toolchain that cannot build it. Do not run two
  cargo invocations against the same `target/` concurrently, and do not leave
  background wait loops running.
- Documentation, comments, commit and PR text in English; Conventional Commits.
  Doc comments explain the reason and cite the spec register (`spec: EP-4`,
  `spec: EP-11`) where a rule comes from it; keep the existing rationale where
  code moves.
- Error types: no `PartialEq`, causes kept as values, variants named as nouns,
  conversions explicit at the boundary; tests assert with `matches!`.
- One public type per module, named after the type; a module expected to grow
  starts as a directory (the gateway's `Destinations` impl and its session
  types are several files).
- Delete what the move makes unused; leave no `allow(dead_code)`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `coffret-usecase` defines `pub trait Destinations` (`reach`, `look_up`)
      with `Destination`, `ScratchFile`, `FlushedFile`, public `Standing` and a
      port-level `DescentError { Blocked, Io(LocalIoError) }`; `UnixFs`
      implements `Destinations` (both grepped by the check).
- [x] No file under `coffret-usecase/src` names `tokio::fs`, `std::fs`,
      `rustix`, `MetadataExt` or `spawn_blocking`; `rustix` is gone from
      `coffret-usecase/Cargo.toml` and present in `coffret-local-fs/Cargo.toml`
      (all grepped by the check).
- [x] `tests/fetch_confinement.rs` lives in `coffret-local-fs` and no longer in
      `coffret-usecase`, its seven cases passing over `UnixFs` (checked by
      `test -f` / `! test -f`).
- [x] `FetchRequest` / `FetchEntryRequest` take `&dyn Destinations`; the fetch
      and freeze fixtures take no folder path at all and every fixture call
      site compiles (`make check` builds the s3-store tests too).
- [x] `destinations_conformance` runs over `UnixFs` in `coffret-local-fs`
      (grepped by the check) and over `InMemoryFs` in `coffret-usecase`.
- [x] `InMemoryFs::fail_on` covers the placement's `Creating`, `Writing`,
      `Flushing`, `Stamping`, `Renaming` and `Removing`, and the six named
      place-fault cases exist and pass (grepped by the check).
- [x] `coffret-device`'s `incoming_file.rs` and `added_at.rs` name no
      `tokio::fs` / `std::fs` / `rustix` (grepped by the check).
- [x] The use-case crate doc no longer says the filesystem "is not a port for
      the reason a device's own disk is not Storage" (grepped by the check);
      `make check` (including `make deps`) is green.
