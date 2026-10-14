---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && (cd backend && RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items -p coffret-usecase) && grep -q "release" docs/concepts/container/README.md && ! grep -q "approve pull requests" docs/guides/release.md'
assignee: null
branch: task/1014-0342-name-what-the-concepts-and-spec-leave-unnamed-and-fix-stale-guide-text
created_at: 2026-10-13T18:42:00Z
updated_at: 2026-10-14T06:34:54Z
---

# docs: name release, alongside placement and unheld parcels in the concepts, define the range read in PK-16, and drop stale guide text

## Overview

Six findings about documentation, gathered from the concept-alignment reviews
of PRs #246, #253, #270 and #271 and from using the release flow. All are
text; none changes a rule's meaning. The Container, Pack and Index concepts
have Japanese copies kept outside this repository; they are regenerated from
this change after it is opened, so this task touches only the English text.

### 1. The Container concept names `release`

`docs/concepts/container/README.md` Collocations (`:32-43`) list read, open,
hold, trash; the Domain Rules at `:77` and `:94` already say an Entry is
*released* to the reader as soon as the chunks covering it have arrived, and
the code uses the word (`parcel_read`'s herald and the fetch outcome). Add a
Collocations entry `release (an Entry to the reader, once the chunks covering
it have arrived, while the rest of the parcel still streams)` beside `read`.
The gate in `check_command` greps `release` in the Container concept.

### 2. The Pack concept says what is placed alongside

`docs/concepts/pack/README.md` Domain Rules `:101-110` say the pages beside
a wanted page "that share" its parcels are already there. Say it as the code
does: an Entry that lies wholly inside a parcel read for another Entry is
placed alongside it, in the same fetch, and is never asked of Storage
(`EntryFetchOutcome.alongside`). One sentence in that bullet, keeping its
spec citations (PK-16, PK-21).

### 3. The Index concept says what an unheld parcel is

`docs/concepts/index/README.md:32-40` describes a held parcel and how it is
let go, and says nothing of the case the code reports as `UnheldParcel`
(`coffret-usecase/src/fetch/unheld_parcel.rs:5-25`,
`coffret-device/src/finding.rs:167-180`): a parcel the record says is held
whose file is gone or does not authenticate is not held after all; the device
says so, because the second read is one the provider observes (PK-20), and
reads the parcel whole again (PK-21). Add that as one or two sentences at the
end of the paragraph, with the same spec citations.

### 3b. The Library concept says what a sync does to the folders its moves emptied

Since the emptied-folder removal landed, a sync removes a folder below the
mapped root that its moves to the desktop's trash left with nothing in it
(EP-15). The Library concept's departed paragraph
(`docs/concepts/library/README.md` ~`:228-239`) stops at moved / kept /
forgotten, and its Collocations reserve *move* for the departed file and
*delete* for Entries but have no verb for the folder. Add, at the end of the
departed paragraph: "A folder below the mapped root that a sync's moves to the
trash leave with nothing at all in it is removed with them, and each folder
above it emptied in turn — never the mapped root, and never a folder that was
empty before the run or still holds anything (spec: EP-15)." Add to the
Collocations, right after the `move` line: "- remove (a folder below a mapped
root that a sync's moves to the trash emptied, once every move is made) —
never *delete*, which is the Library's verb for Entries". Also add, to the
Collocations, the turn the three flows take at this device's pending work:
"- take (the turn at this device's pending work, as a server's sync, freeze or
deletion does before it asks for the Library; a run armed while another holds
it waits for the holder)", matching the turn paragraph in the Domain Rules.

### 3c. The Index concept's Collocations name the sweep

Since a server discards parcel files no row names as it opens the Library
(PK-21, the held-parcel paragraph of the Index concept), add to the Index
concept's Collocations, after `let go`: "- discard (a parcel file no row
names, as a server opens the Library, before it serves anything)".

### 4. PK-16 defines the range read and stops saying `hold`

`docs/spec/pack-construction/README.md:158-164` says "A range read holds what
came back against the extent it asked for". *Hold* is the Container
concept's word for keeping a parcel on the device (`container/README.md:40`),
and *range read* is defined nowhere (it appears in this one sub-item only).
Reword to "A range read — one request for one byte range of the object —
checks what came back against the extent it asked for …", so the term is
defined where it is used and the verb no longer collides.

### 5. The release guide drops a setup step that does nothing

`docs/guides/release.md:256-264` lists, as one-time setup, "allow GitHub
Actions to create and approve pull requests". The workflow never uses the
default token to create a PR: `create-release-pr.yml` checks out and runs
`gh pr create` under `RELEASE_PAT` (`:77`, `:104`), and the first Release PR
was created with that setting off. Remove step 2 and renumber; keep the
sentence about there being no fallback to `GITHUB_TOKEN`. The gate in
`check_command` greps for the removed phrase.

### 6. `cargo doc --document-private-items` passes for `coffret-usecase`

`error.rs:62,554-555` and `descent_error.rs:18,30,229` write links as
`` [`Missing`](crate::Missing) `` where rustdoc resolves the bare name
already, and `--document-private-items` reports "redundant explicit link
target". Write them as `` [`Missing`] `` or `` [`crate::Missing`] ``,
whichever keeps the sentence reading right, and fix whatever else that
invocation reports for the crate, since the gate in `check_command` runs it
with warnings denied. Do not add the flag to `make check` itself: that is a
repository-wide decision this task does not make.

### 7. DK-4 says how work that outlives its answer counts

`docs/spec/device-key-custody/README.md` DK-4 says activity is the span of a
keyed operation, but not how that applies to work a request starts and that
goes on after the request is answered — the rest of a fetch behind a page
already served, a fill. The doc of `shared()` in `key_handle.rs` infers the
answer from DK-4. Read what the code does (`key_handle.rs`, the fetch and fill
paths that take the key) and state it as a sub-item of DK-4: whether such work
keeps its own hold on the key, whether a lock ends it (DK-2), and whether it
counts as activity that defers the lock. Describe the current behaviour; do not
change it. Point the doc of `shared()` at the new sub-item instead of the
inference.

### 8. LA-12 says who takes the turn and what is a conflict

The Library concept's turn paragraph says that within one server a sync, a
freeze and a deletion take turns at this device's pending work, in the order
the runs ask for them, that the server's report names the run holding the turn
and those waiting for it, and that across processes there is no queue: the
second of a server's run and a command-line sync or freeze is refused at once
as a conflict. `docs/spec/loopback-access/README.md` LA-12 says what a server
holds of the work it runs but not this. Add it to LA-12 as a sub-item (no new
rule ID), citing OC-2 for the ownership of the pending work, and cite LA-12
from the concept paragraph where it does not already. Describe the current
behaviour; do not change it.

### 9. The lock comments stop naming *try again*

`frontend/packages/apps/web/src/lock.ts:64` and `lock.test.ts:106` say
"without anybody pressing *try again*"; the locked state no longer offers try
again — it offers unlock. Reword those comments to the current control, keeping
what they mean.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The Container concept's Collocations include `release`; the Pack
      concept says an Entry wholly inside a parcel read for another is placed
      alongside it; the Index concept's held-parcel paragraph says what an
      unheld parcel is and that it is said and read whole again (gate greps
      `release`; the other two are read in the diff — refine's
      concept-alignment perspective checks the wording against the code)
- [x] The Library concept's departed paragraph says the emptied folders are
      removed, and its Collocations have `remove` (the folder) and `take`
      (the turn)
- [x] The Index concept's Collocations have `discard` for the parcel file no
      row names
- [x] PK-16 defines the range read in its own sub-item and no longer uses
      `holds` there
- [x] `docs/guides/release.md` no longer tells the admin to allow Actions to
      create and approve pull requests (gate in `check_command`)
- [x] DK-4 has a sub-item on work that outlives the request that started it,
      matching the code, and `shared()`'s doc cites it
- [x] LA-12 has a sub-item on the turn within one server and the conflict
      across processes, and the Library concept's turn paragraph cites LA-12
- [x] `lock.ts` and `lock.test.ts` no longer name *try again*
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items -p coffret-usecase`
      passes (gate in `check_command`), and `make check` passes
