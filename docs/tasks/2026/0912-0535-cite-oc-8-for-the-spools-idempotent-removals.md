---
status: completed
pipeline_phase: null
follow_up_of: null
base_ref: feat/mapped-root-marker
perspectives: [concept-alignment, completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! git grep -q "OC-6" -- backend/crates/domain/coffret-usecase/src/spool.rs backend/crates/domain/coffret-usecase/src/spool_conformance backend/crates/domain/coffret-usecase/src/local_io_error.rs backend/crates/domain/coffret-usecase/src/local_operation.rs backend/crates/domain/coffret-usecase/src/mapped_roots.rs backend/crates/domain/coffret-usecase/src/in_memory_fs/mod.rs backend/crates/domain/coffret-usecase/src/in_memory_fs/state/files.rs backend/crates/domain/coffret-usecase/src/lib.rs && ! git grep -q "OC-6" -- backend/crates/gateway/coffret-local-fs/Cargo.toml backend/crates/gateway/coffret-local-fs/src/lib.rs backend/crates/gateway/coffret-local-fs/src/unix_fs.rs && git grep -q "OC-8" -- backend/crates/domain/coffret-usecase/src/spool.rs && git grep -q "OC-8" -- backend/crates/domain/coffret-usecase/src/spool_conformance/mod.rs && git grep -q "OC-8" -- backend/crates/domain/coffret-usecase/src/spool_conformance/removal.rs && git grep -q "OC-8" -- backend/crates/domain/coffret-usecase/src/local_io_error.rs && git grep -q "OC-8" -- backend/crates/domain/coffret-usecase/src/local_operation.rs && git grep -q "OC-8" -- backend/crates/domain/coffret-usecase/src/mapped_roots.rs && git grep -q "OC-8" -- backend/crates/domain/coffret-usecase/src/in_memory_fs/mod.rs && git grep -q "OC-8" -- backend/crates/domain/coffret-usecase/src/in_memory_fs/state/files.rs && git grep -q "OC-8" -- backend/crates/domain/coffret-usecase/src/lib.rs && git grep -q "OC-8" -- backend/crates/gateway/coffret-local-fs/Cargo.toml && git grep -q "OC-8" -- backend/crates/gateway/coffret-local-fs/src/lib.rs && git grep -q "OC-8" -- backend/crates/gateway/coffret-local-fs/src/unix_fs.rs'
assignee: null
branch: task/0912-0535-cite-oc-8-for-the-spools-idempotent-removals
created_at: 2026-09-12T05:35:37Z
updated_at: 2026-09-12T14:37:06Z
---

# docs(backend): cite OC-8 for the spool's idempotent removals

## Overview

The orphan-cleanup register holds two separate idempotence rules, and the
code cites the wrong one of the two wherever it describes a device removing
a file it wrote for itself.

`docs/spec/orphan-cleanup/README.md`, verbatim:

> - **OC-6.** An **untrashed removal** is a Container a committed Journal record
>   took out of the current set whose object no device has yet moved to the
>   provider's trash. Any later run may trash it, and proven orphan cleanup and
>   that trashing are both idempotent (CP-14). *(Form: test)*
>   - An untrashed removal is not a suspected orphan (OC-1): its removal is proven
>     by the record rather than inferred from absence, so the no-delete posture of
>     OC-1 and OC-4 does not apply to it.

> - **OC-8.** Removing a local file this device wrote for its own purposes — a
>   spool file, or a scratch a fetch never published (EP-11) — is idempotent. A
>   file that is already gone is a successful removal, an interrupted clean-up is
>   simply run again, and neither removal ever checks what stands at the path
>   first, because absence is the outcome being sought. *(Form: test)*
>   - This is the posture CP-14 gives the Storage side, stated for the local one
>     rather than borrowed from it. OC-6 says trashing an untrashed removal is
>     idempotent, which is a claim about the Library's objects on Storage; a
>     device's own leftovers are not the Library's, so their removals need a rule
>     of their own and do not widen OC-6.

The neighbouring rule that ends up owning one of the citations, also verbatim:

> - **OC-2.** Automatic cleanup of a suspected orphan requires positive local
>   provenance that identifies the creating batch, plus proof that the batch
>   did not commit. *(Form: test)*
>   - The provenance is recorded before the ciphertext it accounts for exists: a
>     device writes the row naming a Container it is about to spool before the
>     spool file is created, so every local ciphertext it produces is named by a
>     row from the moment it can exist, and an interruption at any point leaves
>     nothing cleanup cannot reach.

OC-8's own sub-bullet draws the line the citations have to respect: OC-6 is a
claim about **the Library's objects on Storage**, OC-8 about **a device's own
leftovers**. OC-6 does cover the idempotence of proven orphan cleanup as well
as of the trashing, so a case that trashes a Container's object on Storage and
then runs again is OC-6's — but a spool file, a fetch's scratch, and a run that
never touched Storage at all are OC-8's.

### The count

Across the repository, `OC-6` is cited 36 times in 29 files outside
`docs/spec/`, `docs/concepts/`, and `docs/tasks/`; `OC-8` is cited nowhere
outside the register. Classified against the two rules above:

- **6 citations are correct and stay.** Five describe an untrashed removal
  directly, each in a file of its own — `commit/commit_outcome.rs`,
  `commit/settle.rs`, `commit/untrashed_removal.rs`,
  `commit_conformance/faulty_store.rs`, `commit_conformance/refusals.rs`. The
  sixth, `sync_conformance/interruption.rs:176`, sits in a case whose first run
  disposes of an abandoned Container *with its object trashed on Storage*
  (`Reconciled::Disposed { trashed: true }`) and whose second run finds
  nothing to do — which is exactly OC-6's "proven orphan cleanup … idempotent".
  Its file's three other citations move, so that file is the one place a
  file-wide sweep would take a correct citation with it.
- **22 citations in 19 files are miscited and belong to OC-8.** Every one of
  them describes a removal of a file this device wrote for itself: a spool
  file, a fetch's scratch, or the capability contract that promises absence is
  a successful removal.
- **7 citations in 4 files are about the catalog's provenance row**, not about
  a file. They are determined below.
- **1 citation illustrates neither rule** —
  `sync_conformance/completion.rs:160` explains why a run with no pending rows
  adds no Storage read, which is provenance-gating (OC-2), not idempotence.
- Two citations of `OC-6` in the concept documents
  (`docs/concepts/storage-object/README.md:89`) and in the register
  (`docs/spec/commit-protocol/README.md:89`, CP-14) are correct and are not
  touched here.

`frontend/` cites no `OC` rule at all — `git grep 'OC-[0-9]' -- frontend`
matches nothing — so nothing in the TypeScript workspace is in scope for this
correction.

### The provenance-row determination

The seven catalog-row citations are `index.rs:248`,
`index_conformance/device_state.rs:330` and `:388`,
`sync_conformance/interruption.rs:190`, `:224` and `:398`, and
`coffret-sqlite-index/src/device_state.rs:233`. The determination:

1. **OC-6 is wrong for all seven.** A pending-upload row is this device's own
   bookkeeping, never one of the Library's objects, and OC-8's sub-bullet says
   in so many words that a device's own leftovers "do not widen OC-6". Three of
   the seven sit in cases that dispose of a row with `trashed: false` — nothing
   on Storage was touched, so there is no untrashed removal for OC-6 to be
   about.
2. **Six of the seven belong with OC-8 in substance.** Dropping a row that is
   already gone, and an interrupted clean-up being simply run again, are
   OC-8's two sentences word for word. The row and the spool file it names are
   disposed of by one step (OC-2 writes the row before the file; OC-7 disposes
   of "the local ciphertext and the provenance itself" together), so splitting
   the citation between the file and its row would claim a distinction the
   flows do not make.
3. **OC-8's text says "a local file", and a row is not a file.** That is a gap
   in the rule's wording, not an undecidable case: the rule's subject —
   "removing … this device wrote for its own purposes" — covers the row, its
   enumeration does not name it, and widening the enumeration is a change to
   the register. That change is out of scope here and recorded below.
4. **The seventh is not a removal at all.**
   `index_conformance/device_state.rs:330` says that marking an already-Spooled
   row changes nothing. That is an idempotent *update*, which neither OC-6 nor
   OC-8 states, and the case's own headline already cites the rule that governs
   it (`OC-2`, at `:320`). That citation is to be deleted rather than moved.

### This task's share

This change is the **spool side**: the capability that owns the local
ciphertext, its contract suite, the in-memory disk, the crate-level prose that
names the promise, and the local filesystem gateway's spool implementation.
Twelve files, fifteen citations, every one a character-for-character swap of
`OC-6` to `OC-8` — the surrounding sentences already describe OC-8's behaviour,
so no prose moves with them and no line needs rewrapping.

## What to change

Each edit below replaces the quoted line with the quoted replacement. The two
tokens are the same length, so every line stays within its current width.

### `backend/crates/domain/coffret-usecase/src/spool.rs`

Line 19 — the trait's promise about an abandoned spool:

```
/// far its writing got (spec: OC-6) — and a filesystem that cannot be made to
```

becomes

```
/// far its writing got (spec: OC-8) — and a filesystem that cannot be made to
```

Line 70 — `discard`'s tolerance of a file that is already gone:

```
    /// removed, so an interrupted cleanup is simply run again (spec: OC-6). See
```

becomes

```
    /// removed, so an interrupted cleanup is simply run again (spec: OC-8). See
```

### `backend/crates/domain/coffret-usecase/src/spool_conformance/mod.rs`

Line 5 — the suite's subject. `OC-2` stays: the ordering that names a spool
before it exists is OC-2's, and only the idempotence half moves.

```
//! what an interruption leaves behind (spec: OC-2, OC-6). Those promises are
```

becomes

```
//! what an interruption leaves behind (spec: OC-2, OC-8). Those promises are
```

### `backend/crates/domain/coffret-usecase/src/spool_conformance/removal.rs`

Line 3 — the case that *is* OC-8's first sentence:

```
/// Removing a spool that is already gone is success (spec: OC-6).
```

becomes

```
/// Removing a spool that is already gone is success (spec: OC-8).
```

### `backend/crates/domain/coffret-usecase/src/local_io_error.rs`

Line 31 — the whole-line citation closing the paragraph on why no use-case
code reads an `io::ErrorKind`:

```
/// (spec: OC-6).
```

becomes

```
/// (spec: OC-8).
```

### `backend/crates/domain/coffret-usecase/src/local_operation.rs`

Line 62 — `Removing`, whose two examples are OC-8's two examples:

```
    /// file a failed fetch left, was being deleted (spec: OC-6, EP-11).
```

becomes

```
    /// file a failed fetch left, was being deleted (spec: OC-8, EP-11).
```

### `backend/crates/domain/coffret-usecase/src/mapped_roots.rs`

Line 34 — the cross-reference to what `discard` tolerates:

```
/// [`discard`](crate::Spool::discard) tolerates (spec: OC-6).
```

becomes

```
/// [`discard`](crate::Spool::discard) tolerates (spec: OC-8).
```

### `backend/crates/domain/coffret-usecase/src/in_memory_fs/mod.rs`

Line 42 — "the first" here is interruption, which is OC-8's:

```
/// [`Spool`] states the first (spec: OC-2, OC-6), [`MappedRoots`] the second
```

becomes

```
/// [`Spool`] states the first (spec: OC-2, OC-8), [`MappedRoots`] the second
```

### `backend/crates/domain/coffret-usecase/src/in_memory_fs/state/files.rs`

Line 132 — the fake disk's removal:

```
    /// Removes one file, absence being the same outcome (spec: OC-6).
```

becomes

```
    /// Removes one file, absence being the same outcome (spec: OC-8).
```

### `backend/crates/domain/coffret-usecase/src/lib.rs`

Three citations. Note that the first one's parenthetical wraps across lines
83 and 84: only line 84 changes.

Line 84:

```
//! OC-6), a mapped root that is not there says nothing about the Library rather
```

becomes

```
//! OC-8), a mapped root that is not there says nothing about the Library rather
```

Line 312 — the in-memory disk's module comment, whose citation list also wraps
(`EP-11, EP-12` is on line 313 and stays):

```
// absence, and a real filesystem refuses nothing on request (spec: OC-2, OC-6,
```

becomes

```
// absence, and a real filesystem refuses nothing on request (spec: OC-2, OC-8,
```

Line 354 — the `spool` module's comment:

```
// actually does when it fails (spec: OC-2, OC-6).
```

becomes

```
// actually does when it fails (spec: OC-2, OC-8).
```

### `backend/crates/gateway/coffret-local-fs/Cargo.toml`

Line 30 — the dependency comment explaining what `tracing` is for. The
citation wraps: line 29 ends the clause and line 30 opens with the
parenthetical, so only line 30 changes.

```
# (spec: OC-6), and a mapped root that was not there when it was stated
```

becomes

```
# (spec: OC-8), and a mapped root that was not there when it was stated
```

### `backend/crates/gateway/coffret-local-fs/src/lib.rs`

Line 13 — the gateway's crate prose, mirroring the use-case crate's:

```
//! states the first (spec: OC-2, OC-6),
```

becomes

```
//! states the first (spec: OC-2, OC-8),
```

### `backend/crates/gateway/coffret-local-fs/src/unix_fs.rs`

Line 65 — the `NotFound` arm of `discard`. `OC-2` stays, because the sentence
also explains why absence is an *ordinary* outcome and not only a repeated
one, which is OC-2's ordering:

```
            // never happened (spec: OC-2, OC-6). Swallowing it here is what
```

becomes

```
            // never happened (spec: OC-2, OC-8). Swallowing it here is what
```

### What must not change

- No sentence is reworded. Each of the fifteen citations already describes
  exactly what OC-8 states, which is why the swap alone is the whole fix.
- `OC-2` stays wherever it already stands beside the moved citation. It is not
  a duplicate of OC-8: OC-2 owns the ordering that lets a row name a file that
  never came to exist, and OC-8 owns what disposal does about it.
- No citation in this file set is deleted. Every one of the fifteen illustrates
  OC-8.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes for the workspaces this change touches. Doc comments are
  compiled and linted, so a malformed rustdoc line fails here.
- [x] No `OC-6` citation remains in the spool-side use-case files — every
  occurrence under that pathspec is one of the fifteen that move, so the
  pathspec can be swept as a whole:
  `! git grep -q "OC-6" -- backend/crates/domain/coffret-usecase/src/spool.rs backend/crates/domain/coffret-usecase/src/spool_conformance backend/crates/domain/coffret-usecase/src/local_io_error.rs backend/crates/domain/coffret-usecase/src/local_operation.rs backend/crates/domain/coffret-usecase/src/mapped_roots.rs backend/crates/domain/coffret-usecase/src/in_memory_fs/mod.rs backend/crates/domain/coffret-usecase/src/in_memory_fs/state/files.rs backend/crates/domain/coffret-usecase/src/lib.rs`.
  The pathspec deliberately spares `src/commit/`, `src/commit_conformance/`,
  `src/index.rs`, `src/index_conformance/`, `src/sync/`, `src/freeze/`,
  `src/sync_conformance/`, `src/destination.rs`, `src/destinations.rs`,
  `src/destinations_conformance/` and
  `src/in_memory_fs/in_memory_destination.rs`, which either hold a correct
  `OC-6` citation or are not this change's files.
- [x] No `OC-6` citation remains in the local filesystem gateway's spool files:
  `! git grep -q "OC-6" -- backend/crates/gateway/coffret-local-fs/Cargo.toml backend/crates/gateway/coffret-local-fs/src/lib.rs backend/crates/gateway/coffret-local-fs/src/unix_fs.rs`.
  `src/unix_destinations/` is spared: its citation moves with the fetch
  destinations, not with the spool.
- [x] Each of the twelve files now cites `OC-8`, so a citation that was removed
  instead of moved fails: `git grep -q "OC-8" -- <file>` for each of
  `backend/crates/domain/coffret-usecase/src/spool.rs`,
  `backend/crates/domain/coffret-usecase/src/spool_conformance/mod.rs`,
  `backend/crates/domain/coffret-usecase/src/spool_conformance/removal.rs`,
  `backend/crates/domain/coffret-usecase/src/local_io_error.rs`,
  `backend/crates/domain/coffret-usecase/src/local_operation.rs`,
  `backend/crates/domain/coffret-usecase/src/mapped_roots.rs`,
  `backend/crates/domain/coffret-usecase/src/in_memory_fs/mod.rs`,
  `backend/crates/domain/coffret-usecase/src/in_memory_fs/state/files.rs`,
  `backend/crates/domain/coffret-usecase/src/lib.rs`,
  `backend/crates/gateway/coffret-local-fs/Cargo.toml`,
  `backend/crates/gateway/coffret-local-fs/src/lib.rs`, and
  `backend/crates/gateway/coffret-local-fs/src/unix_fs.rs`.
  Paired with the two sweeps above, this holds every file to moving *all* of
  its citations: a file that moved one of two still matches `OC-6`.

## Out of scope

- **The commit side's `OC-6` citations stay.** `src/commit/commit_outcome.rs`,
  `src/commit/settle.rs`, `src/commit/untrashed_removal.rs`,
  `src/commit_conformance/faulty_store.rs` and
  `src/commit_conformance/refusals.rs` all describe a Container whose object
  Storage would not trash, which is what OC-6 names. They are not in the file
  set and the gates' pathspecs exclude them.
- **The fetch destinations.** `src/destination.rs`, `src/destinations.rs`,
  `src/destinations_conformance/removal.rs`,
  `src/in_memory_fs/in_memory_destination.rs` and
  `coffret-local-fs/src/unix_destinations/unix_destination.rs` carry the same
  miscitation about a scratch a fetch never published. They move with the
  change that touches the placement path, which keeps a placement's cleanup
  story in one diff.
- **The flows, the catalog's provenance row, and the judgement calls that go
  with them.** `src/index.rs`, `src/index_conformance/device_state.rs`,
  `src/sync/sync_request.rs`, `src/freeze/freeze_request.rs`,
  `src/sync_conformance/completion.rs`, `src/sync_conformance/interruption.rs`
  and `coffret-sqlite-index/src/device_state.rs` move with the change that
  touches the catalog — including the citation to be deleted at
  `index_conformance/device_state.rs:330`, the re-citation to `OC-2` at
  `sync_conformance/completion.rs:160`, and the one correct `OC-6` in
  `sync_conformance/interruption.rs` that must be spared. Those three
  judgement calls want the flows in front of the reader, and a file-wide sweep
  is unsafe there, so they do not belong in a pathspec this change can gate.
- **Widening OC-8's enumeration to name the provenance row.** OC-8 says "a
  local file this device wrote for its own purposes"; a pending-upload row is
  the same kind of leftover but is not a file. Saying so is a change to the
  register, which this change does not touch.
- **`docs/concepts/storage-object/README.md` and
  `docs/spec/commit-protocol/README.md`.** Both cite `OC-6` correctly, for an
  untrashed removal and for CP-14's monotonicity.
- **`frontend/`.** It cites no `OC` rule, so there is nothing to correct there.
