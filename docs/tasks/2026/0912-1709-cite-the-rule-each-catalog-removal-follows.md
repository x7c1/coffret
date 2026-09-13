---
status: completed
pipeline_phase: null
follow_up_of: null
base_ref: feat/mapped-root-marker
perspectives: [concept-alignment, completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -q "simply run again (spec: OC-6" backend/crates/domain/coffret-usecase/src/index.rs && grep -q "simply run again (spec: OC-8" backend/crates/domain/coffret-usecase/src/index.rs && ! grep -q "spool step is simply run again (spec: OC-6" backend/crates/domain/coffret-usecase/src/index_conformance/device_state.rs && grep -q "spool step is simply run again. And marking a Container the catalog holds" backend/crates/domain/coffret-usecase/src/index_conformance/device_state.rs && ! grep -q "error (spec: OC-2, OC-6)" backend/crates/domain/coffret-usecase/src/index_conformance/device_state.rs && grep -q "error (spec: OC-2, OC-8)" backend/crates/domain/coffret-usecase/src/index_conformance/device_state.rs && ! grep -q "cannot be removed (spec: OC-2, OC-6" backend/crates/domain/coffret-usecase/src/sync/sync_request.rs && grep -q "cannot be removed (spec: OC-2, OC-8" backend/crates/domain/coffret-usecase/src/sync/sync_request.rs && ! grep -q "cannot be removed (spec: OC-2, OC-6" backend/crates/domain/coffret-usecase/src/freeze/freeze_request.rs && grep -q "cannot be removed (spec: OC-2, OC-8" backend/crates/domain/coffret-usecase/src/freeze/freeze_request.rs && ! grep -q "(spec: OC-6), and a run with nothing to upload" backend/crates/domain/coffret-usecase/src/sync_conformance/completion.rs && grep -q "(spec: OC-2), and a run with nothing to upload" backend/crates/domain/coffret-usecase/src/sync_conformance/completion.rs && ! grep -q "again (spec: OC-6)." backend/crates/domain/coffret-usecase/src/sync_conformance/interruption.rs && grep -q "again (spec: OC-8)." backend/crates/domain/coffret-usecase/src/sync_conformance/interruption.rs && grep -q "than something to fail at (spec: OC-6)." backend/crates/domain/coffret-usecase/src/sync_conformance/interruption.rs && ! grep -q "failing at what the first already did (spec: OC-6" backend/crates/domain/coffret-usecase/src/sync_conformance/interruption.rs && grep -q "failing at what the first already did (spec: OC-8" backend/crates/domain/coffret-usecase/src/sync_conformance/interruption.rs && ! grep -q "/// what the first already did (spec: OC-6" backend/crates/domain/coffret-usecase/src/sync_conformance/interruption.rs && grep -q "/// what the first already did (spec: OC-8" backend/crates/domain/coffret-usecase/src/sync_conformance/interruption.rs && ! grep -q "simply run again (spec: OC-6" backend/crates/gateway/coffret-sqlite-index/src/device_state.rs && grep -q "simply run again (spec: OC-8" backend/crates/gateway/coffret-sqlite-index/src/device_state.rs'
assignee: null
branch: task/0912-1709-cite-the-rule-each-catalog-removal-follows
created_at: 2026-09-12T17:09:34Z
updated_at: 2026-09-13T04:20:36Z
---

# docs(backend): cite the rule each catalog removal follows

## Overview

The orphan-cleanup register holds two idempotence rules, and `OC-8` now names
the provenance row alongside the local files it always covered. The flows and
the catalog still cite `OC-6` for eleven sentences, and only one of the eleven
is `OC-6`'s. Eight of them are about this device disposing of its own
leftovers, one is about a count of Storage reads and not about idempotence at
all, and one is about an idempotent *update* that neither rule states.

`docs/spec/orphan-cleanup/README.md`, as it currently reads:

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

> - **OC-2.** Automatic cleanup of a suspected orphan requires positive local
>   provenance that identifies the creating batch, plus proof that the batch
>   did not commit. *(Form: test)*
>   - The provenance is recorded before the ciphertext it accounts for exists: a
>     device writes the row naming a Container it is about to spool before the
>     spool file is created, so every local ciphertext it produces is named by a
>     row from the moment it can exist, and an interruption at any point leaves
>     nothing cleanup cannot reach.

`OC-8`'s enumeration settles what used to be the hard part: the provenance row
is named in the rule, so a row this device drops is `OC-8`'s in the register's
own words and not by analogy. `OC-8`'s sub-bullet then draws the line the
citations have to respect — `OC-6` is a claim about **the Library's objects on
Storage**, `OC-8` about **a device's own leftovers**, and the latter "do not
widen OC-6". And `OC-2` is neither: it is the rule that makes the provenance
row the thing cleanup is *gated on*, which is a claim about what cleanup may
do rather than about a repeat of it.

### The count

Across `backend/`, sixteen lines cite `OC-6`. Eleven of them are in the
flows and the catalog, which is this change's subject:

- **Eight belong to `OC-8`.** Each is about dropping a pending row, or about
  a spool file that could not be removed — this device's own leftovers, twice
  over where the row and the file it names go together.
- **One belongs to `OC-2`** — `sync_conformance/completion.rs:160`, which is
  not about a repeat at all. It is determined below.
- **One is deleted rather than moved** —
  `index_conformance/device_state.rs:330`, which describes an idempotent
  *update* that neither idempotence rule states, in a case whose own heading
  already cites the rule that governs it.
- **One is correct and must survive this change** —
  `sync_conformance/interruption.rs:176`.

The other five `OC-6` citations in `backend/` are outside this change's files,
all on the commit side: each describes a Container whose object Storage would
not trash, which is an untrashed removal and correctly `OC-6`. The placement
path carried five more of the same miscitation this change corrects; a sibling
change has already moved those to `OC-8`, which is why `backend/` holds sixteen
`OC-6` citations and not twenty-one.

### The correct `OC-6`, and why a file-wide sweep would destroy it

`sync_conformance/interruption.rs` holds four of the eleven, and one of the
four is right. The case at `:139` disposes of an abandoned Container whose
object had already gone up, so the first run trashes it on Storage
(`Reconciled::Disposed { trashed: true }`) and the second, at `:176`, finds
nothing to do. That is `OC-6`'s own second sentence — "proven orphan cleanup
and that trashing are both idempotent" — because what the first run did
reached the Library's object and not only this device's disk.

Its three neighbours are not that. `:190` and `:224` sit in a case whose row
names a spool that is already gone and whose disposal reports
`trashed: false`; `:398` sits in a case whose row was never followed by a file
at all, also `trashed: false`. Nothing on Storage is touched in either, so
there is no trashing for `OC-6` to be about, and what is disposed of — the row
and the spool — is exactly what `OC-8` enumerates.

So the one file this change edits most also holds the one citation it must not
touch. That is why **every absence gate below pins the sentence it means**
rather than sweeping a file or a directory for the bare rule ID: a
`! grep -q "OC-6" sync_conformance/interruption.rs` would pass only by deleting
a correct citation, and a pathspec sweep over `sync_conformance/` would do the
same. The gates are deliberately narrow and deliberately numerous. Collapsing
them into one sweep per file would make the check reward the one edit this
change forbids.

### The determination on the Storage-read count

`sync_conformance/completion.rs:152` heads the case
`a_run_with_no_pending_rows_reads_the_head_once`, and the sentence at `:158`
reads: "the catch-up before the scan is the whole of it, the settling adds
nothing to it because there is nothing to settle (spec: OC-6), and a run with
nothing to upload commits nothing and reads no head to commit against
(spec: CP-1)". That citation becomes `OC-2`, and it is worth saying why,
because a reading that keeps it at `OC-6` has been put and has to be answered.

**The rejected reading.** `OC-6` is about the step that trashes an untrashed
removal, and `commit/settle.rs:32` cites `CP-14, OC-6` over exactly that
step — so "the settling adds nothing to it because there is nothing to settle"
is a correct `OC-6` of the same kind as `interruption.rs:176`.

**Why it loses.** Three things, each sufficient on its own.

1. **It is the wrong settle.** Two different steps in two different modules
   share the word. `commit/settle.rs`'s `trash_removals` is the *commit*
   flow's, and its subject is "what the batch removed" — Containers a
   committed record took out of the current set, which is `OC-6`'s defined
   term and `CP-14`'s monotonicity, correctly cited there. The settling this
   sentence counts is the *sync* flow's step 2, which `sync/mod.rs:18` names
   "**Settle** what an interrupted run left behind (spec: OC-2, OC-3, OC-7)"
   and whose subject is this device's own pending rows. Neither
   `sync/reconcile.rs` nor `Reconciled` cites `OC-6` anywhere.
2. **Even that settle's trashing is not an untrashed removal.** Where the sync
   settle does trash, it trashes an *abandoned* batch's object — a Container
   no record ever made current. `OC-6`'s subject is "a Container a committed
   Journal record took out of the current set", and its sub-bullet says an
   untrashed removal "is not a suspected orphan (OC-1)". So the sync settle's
   trashing reaches `OC-6` only through the "proven orphan cleanup" half of
   its second sentence, which is what `interruption.rs:176` illustrates and
   which requires a removal to have been performed.
3. **This case performs no removal, and states no idempotence.** Its claim is
   a count — one walk of Storage, exact rather than a bound — and both `OC-6`
   and `OC-8` state that a *repeat* is not an error. The second
   `sync_folders` here is an ordinary run over an untouched folder, not a
   rerun of an interrupted cleanup. So neither idempotence rule is the rule
   being cited.

**Why `OC-2` is the rule.** The sentence asserts that the settling reads
nothing from Storage when there is nothing to settle, and the mechanism is in
`sync/reconcile.rs:96`: the step's entire input is `pending_uploads()`, and an
empty answer returns before the current set is read. `OC-2` is the rule that
makes those rows the gate — automatic cleanup "requires positive local
provenance" — so no provenance means no cleanup and therefore no question put
to Storage. The tree already states that shape of claim under `OC-2`:
`sync/reconciled.rs:15` says a row is reclaimed "with no question put to the
Library at all (spec: OC-2)", and this very case's own assertion at `:175`
cites `OC-2` for a committed batch leaving no rows behind. The citation joins
the `CP-1` beside it, and the sentence needs no rewording.

One thing to hold alongside this, because a reader may meet the two accounts
together: a change already merged describes this same line as being about a
run's head-read count being exact rather than a bound, and not about
idempotence at all. That is the sentence's **subject**; provenance gating is
its **mechanism** — the count is exact *because* an empty `pending_uploads()`
returns before Storage is read. The two accounts are the same fact from either
end, and neither has to give way for the other.

## What to change

Eleven sentences, eleven determinations. Nine are a same-length swap of one
rule ID, one is a deletion with the paragraph rewrapped, and one is left
exactly as it stands.

### `backend/crates/domain/coffret-usecase/src/index.rs`

Line 248, closing `clear_pending_upload`'s doc comment. The subject is the
pending row, which `OC-8` now names — **move to `OC-8`**:

```
    /// simply run again (spec: OC-6).
```

becomes

```
    /// simply run again (spec: OC-8).
```

### `backend/crates/domain/coffret-usecase/src/index_conformance/device_state.rs`

Two citations, and they go different ways.

**Line 330 — delete the citation.** The sentence says that marking an
already-`Spooled` row changes nothing. That is an idempotent update of a row,
not a removal of anything, so neither `OC-6` nor `OC-8` states it — and the
case's own heading at `:319` already cites the rule that does: "A Spooling row
becomes Spooled when its spool file does, and only then (spec: OC-2)." A
second citation is not needed and no rule fits it. Removing the parenthetical
reflows the paragraph, so lines 330 to 333:

```
/// spool step is simply run again (spec: OC-6). And marking a Container the
/// catalog holds no row for changes nothing either, rather than failing or
/// inventing one — a row is what a spool step announced, and the operation says
/// that such a row's file is whole.
```

become

```
/// spool step is simply run again. And marking a Container the catalog holds
/// no row for changes nothing either, rather than failing or inventing one —
/// a row is what a spool step announced, and the operation says that such a
/// row's file is whole.
```

No word is added or dropped beyond the parenthetical itself, the line count is
unchanged, and every line stays inside the width the file already keeps. Line
329, which ends "so an interrupted", is untouched.

**Line 388 — move to `OC-8`.** "Clearing it twice is not an error" is about
the row, so `OC-6` becomes `OC-8`. `OC-2` stays beside it: the heading is also
about a row being *recorded* until its batch settles, which is `OC-2`'s
ordering.

```
/// error (spec: OC-2, OC-6).
```

becomes

```
/// error (spec: OC-2, OC-8).
```

### `backend/crates/domain/coffret-usecase/src/sync/sync_request.rs`

Line 31, on the `spool` field — **move to `OC-8`**. What the sentence promises
a case can ask about is a spool file that cannot be removed, which is the
first item of `OC-8`'s enumeration. `OC-2` stays: the same field is how a
spool comes to exist under a row that already names it.

```
    /// be flushed, or cannot be removed (spec: OC-2, OC-6).
```

becomes

```
    /// be flushed, or cannot be removed (spec: OC-2, OC-8).
```

### `backend/crates/domain/coffret-usecase/src/freeze/freeze_request.rs`

Line 31, the same sentence over a Pack's spool — **move to `OC-8`**, for the
same reason and with `OC-2` kept for the same reason.

```
    /// be flushed, or cannot be removed (spec: OC-2, OC-6).
```

becomes

```
    /// be flushed, or cannot be removed (spec: OC-2, OC-8).
```

### `backend/crates/domain/coffret-usecase/src/sync_conformance/completion.rs`

Line 160 — **move to `OC-2`**, on the argument made above. The line's
parenthetical opens it; the clause it belongs to ends on line 159 and is not
touched.

```
/// (spec: OC-6), and a run with nothing to upload commits nothing and reads no
```

becomes

```
/// (spec: OC-2), and a run with nothing to upload commits nothing and reads no
```

### `backend/crates/domain/coffret-usecase/src/sync_conformance/interruption.rs`

Four citations. **Line 176 is correct and is left exactly as it is** — see
"The correct `OC-6`" above. The other three **move to `OC-8`**.

Line 190, closing the stale-row case whose disposal touches nothing on
Storage:

```
/// again (spec: OC-6).
```

becomes

```
/// again (spec: OC-8).
```

Line 224, the second run inside that same case:

```
    // rather than failing at what the first already did (spec: OC-6).
```

becomes

```
    // rather than failing at what the first already did (spec: OC-8).
```

Line 398, the case whose row was never followed by a file:

```
/// what the first already did (spec: OC-6).
```

becomes

```
/// what the first already did (spec: OC-8).
```

### `backend/crates/gateway/coffret-sqlite-index/src/device_state.rs`

Line 233, the gateway's `clear_pending_upload`, mirroring the trait's —
**move to `OC-8`**:

```
/// simply run again (spec: OC-6).
```

becomes

```
/// simply run again (spec: OC-8).
```

### What must not change

- **`sync_conformance/interruption.rs:176`.** Its case trashes a Container's
  object on Storage and runs again, which is `OC-6`'s "proven orphan cleanup …
  idempotent". A gate below pins its sentence so the change cannot take it.
- **No sentence is reworded** except the one paragraph the deletion reflows
  and one two-word repair the moves themselves force. Every moved citation
  already describes what `OC-8` or `OC-2` states, which is why the swap alone
  is the whole fix — with a single exception. `freeze/freeze_request.rs` said
  the `Spool` port lets a case ask what the run does when **a Pack** cannot be
  created, flushed, or removed. Putting `OC-8` on that sentence made its
  subject wrong: `OC-8` covers what this device wrote for its own purposes,
  and a Pack is a Container on Storage. The sibling `sync/sync_request.rs`
  already says "a spool" in the same sentence. So this one now reads "a Pack's
  spool", which is the phrase `docs/concepts/library/README.md` itself uses.
  The repair belongs in this change because this change is what introduced the
  mismatch.
- **`OC-2` stays wherever it already stands** beside a moved citation. It is
  not a duplicate: `OC-2` owns the ordering that lets a row name a file that
  may never come to exist, and `OC-8` owns what disposal does about either.

## Acceptance criteria

### Automated (pipeline-verified)

- [ ] `make check` passes for the workspaces this change touches. Doc comments
  are compiled and linted, so a malformed rustdoc line or an over-wide comment
  fails here.
- [ ] The pending-row sentence in the `Index` trait no longer cites `OC-6`:
  `! grep -q "simply run again (spec: OC-6" backend/crates/domain/coffret-usecase/src/index.rs`.
- [ ] That same sentence cites `OC-8`, so the citation was moved and not
  dropped:
  `grep -q "simply run again (spec: OC-8" backend/crates/domain/coffret-usecase/src/index.rs`.
- [ ] The idempotent-marking sentence in the catalog contract suite no longer
  cites `OC-6`:
  `! grep -q "spool step is simply run again (spec: OC-6" backend/crates/domain/coffret-usecase/src/index_conformance/device_state.rs`.
- [ ] That paragraph was reflowed rather than left with a hole where the
  parenthetical was:
  `grep -q "spool step is simply run again. And marking a Container the catalog holds" backend/crates/domain/coffret-usecase/src/index_conformance/device_state.rs`.
- [ ] The clearing-twice heading in that file no longer cites `OC-6`:
  `! grep -q "error (spec: OC-2, OC-6)" backend/crates/domain/coffret-usecase/src/index_conformance/device_state.rs`.
- [ ] It cites `OC-8` beside the `OC-2` it keeps:
  `grep -q "error (spec: OC-2, OC-8)" backend/crates/domain/coffret-usecase/src/index_conformance/device_state.rs`.
- [ ] The sync request's `spool` field no longer cites `OC-6`:
  `! grep -q "cannot be removed (spec: OC-2, OC-6" backend/crates/domain/coffret-usecase/src/sync/sync_request.rs`.
- [ ] It cites `OC-8` beside the `OC-2` it keeps:
  `grep -q "cannot be removed (spec: OC-2, OC-8" backend/crates/domain/coffret-usecase/src/sync/sync_request.rs`.
- [ ] The freeze request's `spool` field no longer cites `OC-6`:
  `! grep -q "cannot be removed (spec: OC-2, OC-6" backend/crates/domain/coffret-usecase/src/freeze/freeze_request.rs`.
- [ ] It cites `OC-8` beside the `OC-2` it keeps:
  `grep -q "cannot be removed (spec: OC-2, OC-8" backend/crates/domain/coffret-usecase/src/freeze/freeze_request.rs`.
- [ ] The Storage-read count no longer cites `OC-6`:
  `! grep -q "(spec: OC-6), and a run with nothing to upload" backend/crates/domain/coffret-usecase/src/sync_conformance/completion.rs`.
- [ ] It cites `OC-2`, which is the determination this change carries:
  `grep -q "(spec: OC-2), and a run with nothing to upload" backend/crates/domain/coffret-usecase/src/sync_conformance/completion.rs`.
- [ ] The stale-row case's closing citation moves off `OC-6` **and the correct
  `OC-6` two cases above it survives** — two gates, because they are the two
  halves of one requirement in one file:
  `! grep -q "again (spec: OC-6)." backend/crates/domain/coffret-usecase/src/sync_conformance/interruption.rs`
  and
  `grep -q "than something to fail at (spec: OC-6)." backend/crates/domain/coffret-usecase/src/sync_conformance/interruption.rs`.
  The second gate holds now and must go on holding: it is a guard, not a goal,
  and it belongs to this criterion because the absence gate beside it is the
  one an over-broad file sweep would satisfy by deleting the sentence the guard
  pins.
- [ ] That same sentence cites `OC-8`:
  `grep -q "again (spec: OC-8)." backend/crates/domain/coffret-usecase/src/sync_conformance/interruption.rs`.
- [ ] The second run inside the stale-row case no longer cites `OC-6`:
  `! grep -q "failing at what the first already did (spec: OC-6" backend/crates/domain/coffret-usecase/src/sync_conformance/interruption.rs`.
- [ ] It cites `OC-8`:
  `grep -q "failing at what the first already did (spec: OC-8" backend/crates/domain/coffret-usecase/src/sync_conformance/interruption.rs`.
- [ ] The never-created-spool case no longer cites `OC-6`. The gate keeps the
  `///` prefix, which is what separates this sentence from the inline comment
  above whose wording it otherwise shares:
  `! grep -q "/// what the first already did (spec: OC-6" backend/crates/domain/coffret-usecase/src/sync_conformance/interruption.rs`.
- [ ] It cites `OC-8`:
  `grep -q "/// what the first already did (spec: OC-8" backend/crates/domain/coffret-usecase/src/sync_conformance/interruption.rs`.
- [ ] The gateway's row-clearing sentence no longer cites `OC-6`:
  `! grep -q "simply run again (spec: OC-6" backend/crates/gateway/coffret-sqlite-index/src/device_state.rs`.
- [ ] It cites `OC-8`:
  `grep -q "simply run again (spec: OC-8" backend/crates/gateway/coffret-sqlite-index/src/device_state.rs`.

## Out of scope

- **`sync_conformance/interruption.rs:176`'s `OC-6` citation, which is
  correct.** Its case's first run trashes an abandoned Container's object on
  Storage and its second finds nothing to do, which is `OC-6`'s own claim that
  proven orphan cleanup and that trashing are both idempotent. It is the one
  `OC-6` inside the files this change edits, and the guard gate above exists
  so the change cannot take it. Nobody may replace the per-sentence gates with
  a file-wide or directory-wide sweep for the bare rule ID: such a sweep
  passes only if this citation is destroyed.
- **The commit side's five `OC-6` citations.** `commit/commit_outcome.rs`,
  `commit/settle.rs`, `commit/untrashed_removal.rs`,
  `commit_conformance/faulty_store.rs` and `commit_conformance/refusals.rs`
  each describe a Container whose object Storage would not trash, which is
  exactly the untrashed removal `OC-6` defines. `commit/settle.rs`'s
  `CP-14, OC-6` pair is correct where it stands, and the determination above
  turns on that step being a different one from the sync flow's settle, not on
  its citation being wrong.
- **The placement path's `OC-6` citations.** `destination.rs`,
  `destinations.rs`, `destinations_conformance/removal.rs`,
  `in_memory_fs/in_memory_destination.rs` and
  `coffret-local-fs/src/unix_destinations/unix_destination.rs` carried the same
  miscitation about a scratch a local writer never published. A sibling change
  has already moved all five to `OC-8`, so they are out of scope here because
  they are done rather than deferred.
- **Replacing "temporary file" with "scratch".** `OC-8` and `EP-11` call it a
  scratch, and wherever the placement path still calls it a temporary file that
  is a vocabulary change of its own; no part of it belongs here. This change
  moves rule IDs, reflows one paragraph, and adds two words to the one sentence
  a move would otherwise have left wrong; it touches nothing else. None of the
  seven files edited here uses the phrase at all.
- **`docs/spec/` and `docs/concepts/`.** The register's own cross-references
  between `OC-6`, `OC-8` and `CP-14` are correct, and the concept documents
  cite `OC-6` for an untrashed removal.
- **`frontend/`.** It cites no `OC` rule, so there is nothing to correct
  there.
