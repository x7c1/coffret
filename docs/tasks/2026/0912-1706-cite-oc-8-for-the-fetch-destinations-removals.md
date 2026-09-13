---
status: completed
pipeline_phase: null
follow_up_of: null
base_ref: feat/mapped-root-marker
perspectives: [concept-alignment, completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -q "OC-6" backend/crates/domain/coffret-usecase/src/destination.rs backend/crates/domain/coffret-usecase/src/destinations.rs backend/crates/domain/coffret-usecase/src/destinations_conformance/removal.rs backend/crates/domain/coffret-usecase/src/in_memory_fs/in_memory_destination.rs && ! grep -q "OC-6" backend/crates/gateway/coffret-local-fs/src/unix_destinations/unix_destination.rs && grep -q "(spec: OC-8, EP-11)" backend/crates/domain/coffret-usecase/src/destination.rs && grep -q "(spec: OC-8)" backend/crates/domain/coffret-usecase/src/destinations.rs && grep -q "(spec: OC-8," backend/crates/domain/coffret-usecase/src/destinations_conformance/removal.rs && grep -q "(spec: OC-8, EP-11)" backend/crates/domain/coffret-usecase/src/in_memory_fs/in_memory_destination.rs && grep -q "(spec: OC-8, EP-11)" backend/crates/gateway/coffret-local-fs/src/unix_destinations/unix_destination.rs'
assignee: null
branch: task/0912-1706-cite-oc-8-for-the-fetch-destinations-removals
created_at: 2026-09-12T17:06:17Z
updated_at: 2026-09-13T03:04:31Z
---

# docs(backend): cite OC-8 for the fetch destinations' removals

## Overview

The orphan-cleanup register holds two separate idempotence rules, and the
destination capability — the third of the capabilities over this device's own
disk, the one a fetch writes a placed Entry through — cites the wrong one of
the two everywhere it describes removing a scratch that a placement never
published.

`docs/spec/orphan-cleanup/README.md`, verbatim:

> - **OC-6.** An **untrashed removal** is a Container a committed Journal record
>   took out of the current set whose object no device has yet moved to the
>   provider's trash. Any later run may trash it, and proven orphan cleanup and
>   that trashing are both idempotent (CP-14). *(Form: test)*
>   - An untrashed removal is not a suspected orphan (OC-1): its removal is proven
>     by the record rather than inferred from absence, so the no-delete posture of
>     OC-1 and OC-4 does not apply to it.

> - **OC-8.** Removing what this device wrote for its own purposes — a spool
>   file, a scratch a local writer never published (EP-11), the staging directory
>   an interrupted attempt at putting a Library on this device left, or the
>   provenance row (OC-2) in the device's own catalog that announced the spool —
>   is idempotent. What is already gone is a successful removal, an interrupted
>   clean-up is simply run again, and no removal has to check what is there
>   first, because absence is the outcome being sought. *(Form: test)*
>   - This is the posture CP-14 gives the Storage side, stated for the local one
>     rather than borrowed from it. OC-6 says trashing an untrashed removal is
>     idempotent, which is a claim about the Library's objects on Storage; a
>     device's own leftovers are not the Library's, so their removals need a rule
>     of their own and do not widen OC-6.

OC-8's own sub-bullet draws the line the citations have to respect, in the
register's words: OC-6 is a claim about **the Library's objects on Storage**,
OC-8 about **a device's own leftovers**, which "are not the Library's, so
their removals need a rule of their own and do not widen OC-6". OC-8's
enumeration then names the exact thing a destination removes — "a scratch a
local writer never published (EP-11)".

That settles every citation this change touches. Nothing a destination removes
is a Container, nothing it removes has an object on Storage, and no removal it
makes is reached from a committed Journal record: a destination removes the
scratch its own run created and did not rename. `docs/spec/entry-path/README.md`
says where that scratch comes from — under EP-11, "the bytes reach the
destination directory as a scratch that is then renamed into place, so no
reader ever observes a partial or unverified file" — and the scratch that is
left when the rename never happens is what OC-8 calls a device's own leftover.
So the citation becomes `OC-8`, and the `EP-11` standing beside it stays,
because EP-11 is what makes the scratch exist in the first place.

### What was measured, and where it differed

`git grep -n "OC-6" -- backend/` finds 21 citations. Five of them are this
change's, and one of those five is not the shape the rest are:

- `destination.rs:48`, `in_memory_fs/in_memory_destination.rs:61` and
  `unix_destinations/unix_destination.rs:62` each read
  `(spec: OC-6, EP-11)` whole on one line.
- `destinations_conformance/removal.rs:4` ends its line at `(spec: OC-6,`
  and carries `EP-11).` onto line 5.
- `destinations.rs:55` cites **`OC-6` alone**, with no `EP-11` — its sentence
  is about the gateway swallowing an absence rather than about a placement, so
  it never had the second rule to keep.

The remaining sixteen citations are determined under "Out of scope".

### Whether any of the five should gain a second rule

Only `destinations.rs:55` is a single-rule citation, so it is the only
candidate, and it should stay single-rule. Its sentence says that reading the
errno behind an absence is the gateway's job, "exactly as swallowing the
absence a `discard` tolerates is" — an analogy to the spool's tolerance, not a
statement about a placement. The same sentence already stands, in nearly the
same words, at `mapped_roots.rs:34`: "the gateway swallows it, exactly as it
swallows the absence a [`discard`](crate::Spool::discard) tolerates
(spec: OC-8)" — one rule, no `EP-11`. Adding `EP-11` here would claim the
analogy is about the fetch's scratch when its subject is the spool's file.

The other four keep the `EP-11` they already carry: each of them *is* about
the scratch, and OC-8's enumeration names EP-11 for exactly that case.

## What to change

Each edit below replaces the quoted line with the quoted replacement. `OC-6`
and `OC-8` are the same length, so every line keeps its current width and
nothing needs rewrapping.

### `backend/crates/domain/coffret-usecase/src/destination.rs`

Line 48 — the whole-line citation closing the idempotence paragraph of
`remove`'s doc comment, whose sentence says a cleanup racing the failure it is
cleaning up after still succeeds:

```
    /// (spec: OC-6, EP-11).
```

becomes

```
    /// (spec: OC-8, EP-11).
```

### `backend/crates/domain/coffret-usecase/src/destinations.rs`

Line 55 — the trait's statement that a path this device cannot materialize is
`Blocked` and never an I/O refusal, closing on the analogy to the spool's
`discard`. There is no `EP-11` on this line and none is added:

```
/// [`discard`](crate::Spool::discard) tolerates is (spec: OC-6).
```

becomes

```
/// [`discard`](crate::Spool::discard) tolerates is (spec: OC-8).
```

### `backend/crates/domain/coffret-usecase/src/destinations_conformance/removal.rs`

Line 4 — the headline of the case that *is* OC-8's first sentence. The
citation wraps: `EP-11).` sits on line 5 and does not change.

**The citation is the only thing this change touches on this line.** The
sentence's noun is left exactly as it stands — whatever word line 4 uses for
the file, this change reproduces it unchanged and moves `OC-6` to `OC-8` and
nothing else:

```
/// Removing a scratch that is already gone is success (spec: OC-6,
```

becomes

```
/// Removing a scratch that is already gone is success (spec: OC-8,
```

The noun reads `scratch` because the separately queued vocabulary change
reached this line first; that is the arrangement working as intended. Whatever
noun the line carries, change only the rule ID.

### `backend/crates/domain/coffret-usecase/src/in_memory_fs/in_memory_destination.rs`

Line 61 — the fake destination's `remove`, whose comment says absence is the
outcome the caller wanted "here as in the spool":

```
        // (spec: OC-6, EP-11), so nothing is checked before the removal.
```

becomes

```
        // (spec: OC-8, EP-11), so nothing is checked before the removal.
```

### `backend/crates/gateway/coffret-local-fs/src/unix_destinations/unix_destination.rs`

Line 62 — the `ENOENT` arm of `remove`, which swallows the absence so the
layer above never reads an errno to find out which it was:

```
            // (spec: OC-6, EP-11). Swallowing it here is what keeps the layer
```

becomes

```
            // (spec: OC-8, EP-11). Swallowing it here is what keeps the layer
```

### What must not change

- **No sentence is reworded.** Each of the five citations already describes
  what OC-8 states, which is why moving the rule ID is the whole fix.
- **No `EP-11` is removed, and none is added.** The four that carry it keep
  it; `destinations.rs:55` does not gain it.
- **No citation is deleted.** All five illustrate OC-8.
- **No noun in any of these sentences is changed.** The words a removal's
  subject is called are settled
  elsewhere, and touching them here would put two unrelated changes on one
  line.
- **No file outside the five is edited**, and in particular no correct `OC-6`
  citation elsewhere in these crates is swept up. The gates' pathspecs name
  the five files one by one for exactly that reason.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes for the workspaces this change touches. Doc comments
  are compiled and linted, so a malformed rustdoc line fails here.
- [x] No `OC-6` citation remains in the four use-case destination files:
  `! grep -q "OC-6" backend/crates/domain/coffret-usecase/src/destination.rs backend/crates/domain/coffret-usecase/src/destinations.rs backend/crates/domain/coffret-usecase/src/destinations_conformance/removal.rs backend/crates/domain/coffret-usecase/src/in_memory_fs/in_memory_destination.rs`.
  The pathspec names files rather than directories deliberately: a directory
  sweep of `coffret-usecase/src` would take the correct `OC-6` citations in
  `src/commit/`, `src/commit_conformance/` and
  `src/sync_conformance/interruption.rs` with it.
- [x] No `OC-6` citation remains in the local filesystem gateway's destination
  file:
  `! grep -q "OC-6" backend/crates/gateway/coffret-local-fs/src/unix_destinations/unix_destination.rs`.
- [x] `backend/crates/domain/coffret-usecase/src/destination.rs` cites the
  moved rule with `EP-11` intact:
  `grep -q "(spec: OC-8, EP-11)" backend/crates/domain/coffret-usecase/src/destination.rs`.
- [x] `backend/crates/domain/coffret-usecase/src/destinations.rs` cites the
  moved rule on its own, as the sentence's analogy calls for:
  `grep -q "(spec: OC-8)" backend/crates/domain/coffret-usecase/src/destinations.rs`.
- [x] `backend/crates/domain/coffret-usecase/src/destinations_conformance/removal.rs`
  cites the moved rule. The citation wraps across two lines there, so the gate
  pins only the part that sits whole on one line:
  `grep -q "(spec: OC-8," backend/crates/domain/coffret-usecase/src/destinations_conformance/removal.rs`.
- [x] `backend/crates/domain/coffret-usecase/src/in_memory_fs/in_memory_destination.rs`
  cites the moved rule with `EP-11` intact:
  `grep -q "(spec: OC-8, EP-11)" backend/crates/domain/coffret-usecase/src/in_memory_fs/in_memory_destination.rs`.
- [x] `backend/crates/gateway/coffret-local-fs/src/unix_destinations/unix_destination.rs`
  cites the moved rule with `EP-11` intact:
  `grep -q "(spec: OC-8, EP-11)" backend/crates/gateway/coffret-local-fs/src/unix_destinations/unix_destination.rs`.

Paired with the two sweeps, the five presence gates hold every file to moving
*all* of its citations: a file that removed a citation instead of moving it
fails its presence gate, and a file that gained `OC-8` while leaving an `OC-6`
behind fails its sweep.

## Out of scope

- **The commit side's `OC-6` citations, which are correct and stay.**
  `src/commit/commit_outcome.rs:25`, `src/commit/settle.rs:32`,
  `src/commit/untrashed_removal.rs:6`,
  `src/commit_conformance/faulty_store.rs:54` and
  `src/commit_conformance/refusals.rs:129` each describe a Container whose
  object Storage would not trash — an untrashed removal, which is precisely
  what OC-6 names. They are not in the file set, and the gates' pathspecs
  exclude them.
- **The one correct `OC-6` beside the flow citations.**
  `src/sync_conformance/interruption.rs:176` sits in a case whose first run
  disposes of a Container with its object trashed on Storage and whose second
  run finds nothing to do, which is OC-6's "proven orphan cleanup … idempotent".
  It stays, and nothing here sweeps its file.
- **The flows and the catalog's provenance row.** `src/index.rs:248`,
  `src/index_conformance/device_state.rs:330` and `:388`,
  `src/sync/sync_request.rs:31`, `src/freeze/freeze_request.rs:31`,
  `src/sync_conformance/completion.rs:160`,
  `src/sync_conformance/interruption.rs:190`, `:224` and `:398`, and
  `coffret-sqlite-index/src/device_state.rs:233` all cite `OC-6` about a row
  or a run rather than about a file a destination removes. Each needs the
  flows in front of the reader — `src/sync_conformance/completion.rs:160` is
  about a run's head-read count being exact rather than a bound, not about
  idempotence at all, and `src/index_conformance/device_state.rs:330` is about
  an idempotent *update* rather than a removal — so they are corrected where
  that reading can be done, and a file-wide sweep would be unsafe there.
- **The spool side.** The capability that owns the local ciphertext, its
  contract suite, the in-memory disk and the local filesystem gateway's spool
  already cite `OC-8`; nothing about them is revisited here.
- **Replacing `temporary file` with `scratch`.** Bringing the prose onto the
  register's word is its own change: it touches sentences rather than
  citations. The two were deliberately kept from colliding on
  `destinations_conformance/removal.rs:4`, where a citation and a noun share a
  line — that change alters only the noun and this one only the rule ID, and
  the gates here pin no noun. The vocabulary change reached that line first, so
  it already reads `scratch`; the arrangement held.
- **The register itself.** `docs/spec/orphan-cleanup/README.md` and
  `docs/spec/entry-path/README.md` are read here and quoted, never edited.
  The `OC-6` citations in `docs/concepts/` and elsewhere under `docs/spec/`
  are correct as they stand.
- **`frontend/`.** `git grep "OC-[0-9]" -- frontend` matches nothing, so the
  TypeScript workspace has no citation to correct.
