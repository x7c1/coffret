---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -rq "pub trait MappedRoots" backend/crates/domain/coffret-usecase/src && grep -rq "impl MappedRoots for UnixFs" backend/crates/gateway/coffret-local-fs/src && [ -z "$(grep -rlE "tokio::fs|std::fs|MetadataExt" backend/crates/domain/coffret-usecase/src | grep -vE "/(fetch|fetch_conformance)/")" ] && ! grep -rqE "tokio::fs|std::fs" backend/crates/domain/coffret-usecase/src/local_scan backend/crates/domain/coffret-usecase/src/sync_conformance backend/crates/domain/coffret-usecase/src/freeze_conformance && ! grep -rq "unix-dev:" backend/crates/domain/coffret-usecase/src && grep -rq "a_source_that_cannot_be_opened_fails_the_sync_before_any_spool_exists" backend/crates/domain/coffret-usecase && grep -rq "a_folder_that_cannot_be_listed_fails_the_sync_and_names_the_listing" backend/crates/domain/coffret-usecase && grep -rq "a_root_that_cannot_be_stated_fails_the_sync_and_names_the_stat" backend/crates/domain/coffret-usecase && grep -rq "a_member_that_cannot_be_read_while_packing_leaves_a_spooling_row_and_uploads_nothing" backend/crates/domain/coffret-usecase && grep -rq "mapped_roots_conformance" backend/crates/gateway/coffret-local-fs/tests && ! grep -rq "tokio::fs\|std::fs" backend/crates/apps/coffret-device/src/add/added_locally.rs'
assignee: null
branch: task/0907-0006-move-the-mapped-folder-walk-behind-a-mapped-roots-capability-of-the-local-fs-gateway
created_at: 2026-09-07T00:06:02Z
updated_at: 2026-09-07T07:22:08Z
---

# feat(backend): move the mapped-folder walk behind a MappedRoots capability of the local-fs gateway

## Overview

The spool already reaches the disk through the `Spool` capability and the
`coffret-local-fs` gateway (`UnixFs`), with `InMemoryFs` standing in for tests.
The *reading* side still calls the filesystem directly: `local_scan/root_state.rs`
(`tokio::fs::metadata` on the root, `read_dir` to see whether it holds anything,
`MetadataExt::dev()` spelled as `unix-dev:{dev}` into a `RootIdentity`),
`local_scan/walk_mappings.rs` (`read_dir` per folder, `symlink_metadata` per
child), `local_scan/source_file.rs` (`fs::read` and `File::open` of a source),
and `local_times.rs` (`Metadata` → `Mtime` / `Btime`). So a scan can be tested
against a folder shaped by hand, but never against a folder whose listing,
stat, or read *fails* at a chosen point, and the sync / freeze conformance
suites still need a real temporary directory for the mapped folder.

This task moves that surface behind a `MappedRoots` capability, the second of
three (the third, the confined placement a fetch writes, is a later task and
keeps its direct calls for now). What stays in the use case is every decision
about *what the walk means*: which folders to descend into (a folder that
another mapping represents, a name carrying the scratch prefix), how a name
becomes an Entry Path (EP-1 normalization, EP-2 shape, `UnrepresentableName`),
two files claiming one path (`PathCollision`), and EP-12's reading of a root —
missing is a verdict, empty on another filesystem is a verdict, non-empty is a
re-stamp. What moves to the gateway is how the operating system is asked:
stat the root following links, list one folder without following links, open a
regular file, and the platform's spelling of a filesystem identity.

### What to build

**1. The `MappedRoots` capability in `coffret-usecase`.** A `pub trait
MappedRoots: Send + Sync` (`async_trait`, beside `Spool`) with three
operations, every one failing with `LocalIoError`:

- `probe_root(&self, root: &Path) -> Result<Option<RootProbe>, LocalIoError>` —
  stats the root itself, *following* links (what `fs::metadata` does today, and
  what `read_dir` would resolve anyway; spec: EP-12). `Ok(None)` when the root
  is not there; `Some(RootProbe { identity: Option<RootIdentity> })` when it is,
  with the identity of the filesystem it stands on where the platform can say
  (`None` where it cannot). The `unix-dev:{dev}` spelling and `MetadataExt`
  move into the gateway; the use case keeps only the comparison.
- `list_folder(&self, dir: &Path) -> Result<Option<Vec<FolderEntry>>, LocalIoError>`
  — the children of one folder, each stated *without* following links
  (spec: EP-8). `Ok(None)` when the folder is not there (a subfolder that went
  away mid-walk, or a root that went between the probe and the listing). A
  child that vanished between the listing and its stat is left out. Each
  `FolderEntry` carries `name: OsString` (the operating system's own bytes —
  turning it into text is the use case's boundary, where `UnrepresentableName`
  is decided) and a `kind`: `File { size, mtime: Mtime, btime: Option<Btime> }`,
  `Folder`, or `Other` (a symbolic link or anything else that is neither).
- `open_source(&self, path: &Path) -> Result<Box<dyn SourceReader>, LocalIoError>`
  — opens a regular file for streaming reads; `pub trait SourceReader: Send`
  with `read(&mut self, buffer: &mut [u8]) -> Result<usize, LocalIoError>`.
  `SourceFile::read` (whole file) is written in the use case over the reader.

Keep the walk in `local_scan/` and its verdicts exactly as they are; only the
calls change. The use case must not branch on `io::ErrorKind` anywhere on this
path: absence is the `None` the capability answers with. `local_times.rs` loses
`mtime_of` / `btime_of` (they read `std::fs::Metadata`; they move into the
gateway and stop being public API of the use-case crate); `system_time_of`
stays for the fetch's stamp until the placement moves. `RootIdentity::new`
stays public so both the gateway and the fake can build one.

**2. `UnixFs` implements `MappedRoots`** in `coffret-local-fs`, moving the
bodies out of `root_state.rs`, `walk_mappings.rs`, `source_file.rs` and
`local_times.rs`: `fs::metadata` for the probe, `read_dir` + `symlink_metadata`
for the listing (mapping `NotFound` on the folder to `None`, skipping a child
whose stat says `NotFound`), `File::open` for the source, `MetadataExt::dev()`
spelled `unix-dev:{dev}` for the identity, and the `Metadata` → `Mtime` /
`Btime` conversions with their existing saturation rules and doc comments.

**3. `InMemoryFs` implements `MappedRoots`.** The fake grows a tree it can
answer these questions from: folders (created explicitly or by writing under
them), files with `bytes`, `mtime` (seconds), and optional `btime`, and a
marker for "something that is neither" so a case can plant a symbolic link
without a real filesystem. Give it what the fixtures need to arrange a case:
`write_file(path, bytes)` (creating the folders above it, stamping a default
mtime), `set_mtime(path, seconds)`, `set_btime(path, Option<seconds>)`,
`create_dir(path)`, `remove_dir_all(path)`, `remove_file(path)`, `plant_other(path)`,
`set_root_identity(path, RootIdentity)`, and readers for what a case asserts on
(`content`, `files_under`, `observed(path) -> Option<(u64, Mtime)>`, `born(path)`).
`probe_root` answers `Some` with the identity set for that path, else a fixed
`RootIdentity::new("in-memory:0")`, so a mapping recorded with today's
`another_filesystem()` (`unix-dev:0`) still mismatches. Extend `fail_on` to
`Listing` (the nth `list_folder`), `Stating` (the nth `probe_root`) and
`Reading` (the nth `open_source` *or* `SourceReader::read`, whichever comes
first in the count — say which in the doc).

**4. Thread the capability through the flows.** `SyncRequest` and
`FreezeRequest` gain `roots: &'a dyn MappedRoots` beside `spool`; `sync::scan`,
`freeze::scan`, `freeze::spool` (which streams each member through
`SourceFile::open`) and `local_scan` take it. `coffret-device` passes
`self.local_fs.as_ref()` for both (`UnixFs` implements both traits). Its
`add/added_locally.rs` reads a mapped folder with `fs::read_dir` and `mtime_of`
today: switch it to `MappedRoots::list_folder` on `self.local_fs`, keeping its
own filtering (scratch names, non-UTF-8 names, names that spell no Entry Path,
paths the Library holds, non-files). `add/added_at.rs`'s `fs::metadata` stays
for now (composition-root code the doc pass decides on).

**5. Conformance fixtures move the mapped folder into the fake.**
`SyncUnderTest` and `FreezeUnderTest` no longer take a folder path: each holds
one `InMemoryFs` that serves as both spool and mapped roots, and exposes it
(`fs()`), plus a fixed folder path inside it (`folder()` → `/folder`).
`FetchUnderTest` keeps a *real* `target_folder` (the fetch still places through
the real filesystem until the placement task) but its `source_folder` moves
into the fake, since the source device's sync now scans through `MappedRoots`.
Rewrite the fixture helpers that touch the folder (`write`, `touch`, `observed`,
`born`, `read`, `remove_dir_all`, the two "unmounted root" cases with
`another_filesystem`, the NFD-name case) to use the fake's API, keeping every
case's assertions. Update the six fixture call sites in `coffret-usecase/tests/`
and `gateway/s3-store/tests/` (they drop their folder tempdirs; the fetch ones
keep the target one), and `tests/fetch_confinement.rs`, whose source device
now writes its files into the fake and syncs through it while the target device
keeps fetching into a real directory.

**6. `mapped_roots_conformance`.** A shared suite behind the `conformance`
feature (a `MappedRootsUnderTest` fixture holding a `Box<dyn MappedRoots>` plus
a closure or small trait the backend supplies for arranging a folder: make a
folder, write a file with an mtime, plant something that is not a file, remove
a folder), run over `InMemoryFs` in `coffret-usecase/tests/` and over `UnixFs`
in `coffret-local-fs/tests/mapped_roots_conformance.rs` against a `tempfile`
directory. Cases: a missing root probes to `None`; a present root probes to
`Some` with an identity (on Unix); a missing folder lists to `None`; a listing
reports a file's size and mtime and a folder as `Folder`; a symbolic link (or
the fake's planted marker) lists as `Other`, never as the file it points at; a
source streams back the bytes that were written; opening a missing source is
refused as `Reading`. The real-filesystem case that makes a symbolic link is
Unix-only and says so.

**7. Failure-injection cases in `coffret-usecase/tests/scan_faults.rs`**,
beside `spool_faults.rs` and in the same shape (a `Device` over
`InMemoryStore`, `InMemoryIndex`, one `InMemoryFs` for roots and spool), each
asserting the returned error variant *and* the state left behind. Use these
exact names:

- `a_source_that_cannot_be_opened_fails_the_sync_before_any_spool_exists`
  (sync; the first `Reading` fails while the scan hashes a candidate) — the
  error is `SyncError::Io` with `LocalOperation::Reading`; no pending row, no
  spool file, nothing in the store.
- `a_folder_that_cannot_be_listed_fails_the_sync_and_names_the_listing` (sync;
  `Listing` fails) — `SyncError::Io` with `LocalOperation::Listing`; nothing
  written anywhere.
- `a_root_that_cannot_be_stated_fails_the_sync_and_names_the_stat` (sync;
  `Stating` fails on the probe) — `SyncError::Io` with `LocalOperation::Stating`.
- `a_member_that_cannot_be_read_while_packing_leaves_a_spooling_row_and_uploads_nothing`
  (freeze; `Reading` fails partway through a Pack's member stream, after the
  scan's own hashing reads) — `FreezeError::Io` with `Reading`; one pending row
  still `Spooling`, the store lists nothing, and the next run (unscripted)
  disposes of the row and packs the files once.

### Conventions

- `make check` is the gate; run it before finishing. On macOS, exporting
  `SSL_CERT_FILE=/etc/ssl/cert.pem` avoids a keychain flake in
  `coffret-device`'s tests, and `CC=/usr/bin/cc CXX=/usr/bin/c++` keeps
  `aws-lc-sys` off a non-system toolchain that cannot build it. Do not run two
  cargo invocations against the same `target/` concurrently.
- Documentation, comments, commit and PR text in English; Conventional Commits.
  Doc comments explain the reason, as the surrounding code does, and cite the
  spec register (`spec: EP-8`, `spec: EP-12`) where a rule comes from it.
- Error types: no `PartialEq`, causes kept as values, variants named as nouns,
  conversions explicit at the boundary; tests assert with `matches!`.
- One public type per module, named after the type; a module expected to grow
  starts as a directory.
- Delete what the move makes unused; leave no `allow(dead_code)`. The public
  `mtime_of` / `btime_of` re-exports go with their functions.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `coffret-usecase` defines `pub trait MappedRoots` (`probe_root`,
      `list_folder`, `open_source`) and `pub trait SourceReader`, failing with
      `LocalIoError`; `UnixFs` implements `MappedRoots` (both grepped by the
      check).
- [x] No file under `coffret-usecase/src` outside `fetch/` and
      `fetch_conformance/` names `tokio::fs`, `std::fs` or `MetadataExt`, and
      `local_scan/`, `sync_conformance/` and `freeze_conformance/` name neither
      `tokio::fs` nor `std::fs` (grepped by the check).
- [x] The `unix-dev:` spelling of a root identity no longer appears in
      `coffret-usecase/src` (grepped by the check); the fake answers
      `in-memory:0` by default and any identity a case sets.
- [x] The scan's verdicts are unchanged: every existing sync, freeze and fetch
      conformance case passes with the mapped folder in `InMemoryFs`, including
      the missing-root, empty-root-on-another-filesystem, NFD-name and
      birth-time cases.
- [x] `SyncUnderTest` and `FreezeUnderTest` take no folder path;
      `FetchUnderTest` takes only the target folder; all fixture call sites
      compile (`make check` builds the s3-store tests too).
- [x] `InMemoryFs::fail_on` covers `Listing`, `Stating` and `Reading`, and the
      four named scan-fault cases exist and pass (grepped by the check).
- [x] `mapped_roots_conformance` runs over `UnixFs` in `coffret-local-fs`
      (grepped by the check) and over `InMemoryFs` in `coffret-usecase`.
- [x] `coffret-device`'s `added_locally.rs` reads the folder through
      `MappedRoots::list_folder` and names no `tokio::fs` / `std::fs` (grepped by
      the check); `make check` (including `make deps`) is green.
