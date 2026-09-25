---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, error-type-design]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check"
assignee: null
branch: task/0925-0851-bound-what-an-untrusted-length-can-make-this-device-do
created_at: 2026-09-25T08:51:37Z
updated_at: 2026-09-25T10:39:57Z
---

# fix: bound what an untrusted length can make this device do, and open local files by handle

## Overview

Security-leaning items still open in this repository. None is an open
hole today — each is a ceiling that exists on one side and not the other, a
file opened by a path where a handle was already in hand, a mode that depends
on the caller's umask, or a promise the register does not yet state. They
share one rule: **a number or a name that arrived from outside this process
decides nothing about what this process allocates, opens or keeps until it
has been held against a bound this process chose.**

### 1. A plaintext `meta_len` sizes a buffer before anything is authenticated

`ContainerOutline` reads `meta_len` from the plaintext header and the reader
allocates for it before the AEAD tag has said whether the header is genuine.
A hostile Storage writing a huge `meta_len` makes the allocation happen first.
Give `ContainerOutline::prefix_len` (or the point where the length is first
read) a ceiling of its own — the meta section's own `MetaSectionTooLong`
bound already exists for the authenticated read; apply the same number before
the allocation, refuse with the same variant, and pin it with a fixture whose
header claims more than the ceiling. Then carry the same two ceilings
(`metaLength` and the control-object ceiling) into the TypeScript second
implementation (`frontend/packages/domain/format/src/containerHeader.ts` and
its control counterpart), which has neither today, with a test each; the
interop fixtures are all valid objects, so `make interop` cannot see this.

### 2. The register does not say a candidate past the ceiling is stepped over

CK-9 is where catch-up decides which candidate heads are valid. The code
treats a candidate whose declared length exceeds the ceiling as not valid and
steps over it; CK-9 does not say so. Add the sentence, with its form. While
there, bound the other side of the same walk: catch-up keeps every head record
it lists in a `BTreeMap` — per record is bounded at 256 MiB, but the *count*
is whatever the listing returned. CK-12 states catch-up's retention and memory
budget; make the count obey it (or say in CK-12 why a count bound is not
needed, if the page cap already implies one) and pin whichever it is.

### 3. `routes/file.rs` reopens a file by path where a handle exists

The read-side confinement (spec: EP-8, EP-11) is complete for scans and for
`added_at.rs`, which checks metadata through the gateway without following
links. The server's file route still opens the plaintext by *path* on its
three ways in — present, added and freshly fetched — after the device layer
has already resolved and confined the location. Make the gateway hand back the
opened handle (or a capability that opens without following a link on the
final component) and have the route read from that, so no path is walked twice
with a window between. Count the three entry points and change all three;
pin with a test that swaps the target for a symlink between resolution and
open and sees a refusal, using the in-memory filesystem's fault injection if
it can express it, else the unix one under `tempfile`.

### 4. The catalog's file mode depends on the caller's umask

`SqliteIndex::open` creates `index.sqlite` with whatever mode the umask leaves.
`init` creates it at 0600 first, so the fresh path is fine, but a person who
deletes the file and runs `map` or opens the Library gets a catalog readable
by their group. Either `coffret-sqlite-index` takes the mode it must create
with, or the device layer places an empty 0600 file before opening; choose the
one that keeps the rule in one place, and pin it with a test that sets a
permissive umask around the create. In the same crate, `SqliteIndex::share()`
discards the answer of `PRAGMA journal_mode = WAL`: on a filesystem that
cannot do WAL the server-and-sync coexistence guarantee silently lapses. Read
the answer; if it is not `wal`, say so — a `warn!` event through the crate's
logging (add the `tracing` dependency it needs) is the minimum; decide whether
opening should refuse instead, and write the reason beside it.

### 5. What a person is told when a drop outruns the envelope

When the server answers 413 or 507 mid-stream, the browser observes a broken
transfer and the gateway's `asked()` turns it into "the coffret server did not
answer" — the one sentence the server did *not* write. In `request.ts` tell a
failure whose answer could be read apart from a transfer that broke; and, for
the request budget (spec: LA-9, LA-10), refuse before sending when
`Content-Length` already exceeds it, so the person reads the server's own
sentence with what to do next. The 413 / 507 body should also carry `written`
(the files that did land) so the page can reload the listing rather than
leave landed files invisible; and the refusal should name which file was too
large where the route knows it (the `outran` sentence is a static string
today; naming a file in a person-facing refusal is allowed by EL-1). A drop
with no `Content-Length` demands 1 GiB free by default and is untested; pin
that branch.

### 6. Small ones in the same files

- `Refused::Elsewhere` says the request came from the wrong address and not
  which address would do; say it (the message becomes a `String`).
- TypeScript's `KEY_HEADER` and Rust's `CAPABILITY_HEADER` name the same
  header (`x-coffret-key`); give them one name.
- `ObjectTooLarge` / `ControlObjectTooLong` / `MetaSectionTooLong`: one
  suffix. The register's word is *long*; rename the one that differs and its
  tests.
- `IncomingFile::write()` allocates a `PathBuf` per chunk; hold it once.
- `scripts/e2e-it.sh` uses `curl --fail`, which drops a 403 body; use
  `--fail-with-body` where no `--output` is combined with it, and check the
  places that are.

### 7. Verify, then strike or fix

An S3 listing answered 404 once put the configured key prefix into
`NotFound { object }`, which `redacted()` rendered verbatim into a log. Confirm
that the list path now answers `Missing::Listing` (never `Missing::Object` with
the prefix) and that `NotFound`'s redacted rendering carries no prefix; if any
path still does, fix it and pin it with a captured log.

### 8. Record only

Two servers started on one Library leave the first with its sidecar key
stolen, answering 403 only — a design question about detecting an existing
key at start, not for this change. `GoogleDrive::get`'s default 1 MiB ceiling
refuses a Container over 1 MiB only when a proxy re-chunks without
`Content-Length`; that is the intended fail-safe. Say both in the PR description.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes
- [x] a header claiming a `meta_len` past the ceiling is refused before any
      allocation for it, in Rust and in TypeScript, with a fixture each
- [x] CK-9 states that a candidate whose declared length exceeds the ceiling
      is stepped over, and catch-up's held-record count is bounded or CK-12
      says why it need not be
- [x] the file route reads present, added and fetched plaintext from a handle
      the gateway opened without following a link on the final component, and
      a test that swaps in a symlink between resolution and open sees a refusal
- [x] `index.sqlite` is created 0600 regardless of umask, and a non-WAL
      journal mode is reported (or refused) rather than ignored
- [x] a 413 / 507 whose body was read reaches the page as the server's own
      sentence, a request whose `Content-Length` exceeds the budget is refused
      before sending, the body carries `written`, and the no-`Content-Length`
      branch has a test
- [x] `Refused::Elsewhere` names an address that would do; one header
      constant name on both sides; one `TooLong` suffix

### Manual / on-hardware (verified by a human before merge)

- [ ] `make e2e-it` is green
- [ ] a book larger than the request budget was dropped from a real browser
      and the sentence on screen was the server's

## Out of scope

- Detecting an existing sidecar key at server start (design)
- Where a grant lives (a device and an account): a separate change
- The MinIO conformance target and the explorer hook tests: a separate
  test-and-CI change
