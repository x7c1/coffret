---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && (cd backend && RUSTDOCFLAGS=-Dwarnings cargo doc --workspace --no-deps --document-private-items) && grep -q "coffret-fetch-" docs/spec/entry-path/README.md && grep -qF -- "- settle (" docs/concepts/library/README.md && grep -qF -- "- stamp (" docs/concepts/library/README.md && grep -qF -- "- announce (" docs/concepts/index/README.md && grep -qF -- "- mark (" docs/concepts/index/README.md && grep -qF -- "- restore (" docs/concepts/index/README.md && grep -qF -- "- seal (" docs/concepts/purpose-key/README.md && grep -q "Spooling" docs/concepts/index/README.md && grep -q "Spooling" docs/spec/orphan-cleanup/README.md && grep -q "EP-12" docs/concepts/index/README.md && grep -q "PK-17" docs/spec/README.md && grep -qF "**unavailable root**" docs/concepts/library/README.md && grep -qiw "finding" docs/concepts/library/README.md && grep -qE "commit slot.*snapshot slot|snapshot slot.*commit slot" docs/concepts/index-snapshot/README.md && grep -qF "snapshot slot" docs/concepts/journal/README.md && git grep -qF "untrashed removal (spec: OC-6)" -- backend && ! git grep -qE "recorded (as )?complete" -- backend/crates/domain/coffret-usecase/src/sync/reconciled.rs && ! git grep -q "reported as changed" -- backend docs/concepts && ! grep -qF "**materialized**" docs/concepts/entry-path/README.md && ! grep -qF "the copy a record travels with" frontend/packages/domain/format/src/model/containerSummary.ts && { ! awk "/OC-6\\./,/OC-7\\./" docs/spec/orphan-cleanup/README.md | grep -qi complet; }'
assignee: null
branch: task/0828-1330-doc-follow-ups-38-to-45
created_at: 2026-08-28T13:30:00Z
updated_at: 2026-08-28T14:35:00Z
---

# docs: close the concept, spec, and rustdoc follow-ups left by #38 to #45

## Overview

The refine passes of recently merged pull requests (#38 the previous docs
batch, #39 row-first spool tracking, #40 unavailable mapped roots, #42 the
`Surfaced` rename, #45 the spool-vocabulary alignment) each left short lists
of documentation items that were out of their scope: a concept document that
never registered a word the code leans on, a spec rule that states one half of
the condition its implementation checks, a rustdoc paragraph a later rename
left speaking the old vocabulary. None needs new design. This task closes all
of them in one pull request so the register, the concept documents, and the
code's own prose stop disagreeing with each other.

**No behavior changes.** Code files are touched for doc comments, comments,
and string-free prose only. No function body, signature, type, test
expectation, or fixture moves.

**Every concept document you touch goes through the litmus test in
`docs/concepts/README.md`.** A rule stays in a concept document only if
changing it would change what the term means or what a user can rely on; a
procedure, verification, or parameter needed only to build the system
correctly belongs in the specification register (`docs/spec/`), and the
concept document keeps at most a one-sentence summary citing the rule IDs as
plain text. Honour that file's other conventions too: one rule per bullet, at
most two sentences, caveats as sub-bullets, one why-clause on every
non-obvious rule, and no defensive negations. Collocations entries follow the
existing style of each list. Every decision below is already made: apply it;
do not reopen it.

### Library concept (`docs/concepts/library/README.md`)

- **Register `settle` in the Collocations** — the sync-flow prose uses it
  twice for disposing of what a landed batch left; one list entry in the
  existing style.
- **Register `stamp` in the Collocations** — EP-12's central verb (a scan
  stamps the filesystem identity a mapped root stood on). Word the entry so it
  cannot be confused with `LocalOperation::Stamping` (fetch's mtime setting,
  FM-9/EP-11), which is the same spelling for a different act; if one entry
  cannot carry both senses cleanly, register the scan sense and let the fetch
  sense stay code-local.
- **Define *finding* in the Domain Rules** — the noun is established across
  `sync/run.rs`, `sync/surfaced.rs`, `sync/scan/deletions.rs`, and conformance
  case names, but no document defines it. Add one Domain Rules bullet to the
  effect that a run reports each file it surfaces as a *finding*, and a
  finding is not an error: it is re-reported by every later run until someone
  acts on it.
- **Make the existing "unavailable root" mention the noun's definition point**
  — the deletion bullet's sub-bullet (around line 67) already uses the noun;
  bold it there (`**unavailable root**`) and make the sub-bullet read as a
  one-sentence definition, since public types and outcome fields carry the
  noun with no documentation home.

### Index concept (`docs/concepts/index/README.md`)

- **Bring the pending row's Definition up to row-first** — the current text
  ("encoded and perhaps uploaded") predates #39: a row now also covers a
  Container the device is *about to* spool, and the row's state says whether
  its file is a whole Container. Update the Definition bullet and the
  device-state summary bullet accordingly.
- **Add a small Mental Model entry for the spool states** — `Spooling` /
  `Spooled`, in the document's existing style: two states; the only transition
  is made by the spool step that finished the file; a `Spooling` row never
  carries an object handle. (This documents `SpoolState` from #45.)
- **Register `announce` in the Collocations** — row-first's central verb (a
  device announces a spool by recording its pending row before the file
  exists); it appears in code prose 15+ times with no canonical home.
- **Register `mark` in the Collocations** — the port's idiom for a narrow
  state flip (`mark_spooled`, `mark_present`, `mark_absent`); word the entry
  so it is distinct from `complete`, which stays reserved for OC-7's
  bookkeeping completion.
- **Register `restore` in the Collocations with its direction** — the concept
  prose says a device restores *from* a Snapshot while code-derived prose
  takes the Snapshot as a direct object; register `restore (the Index from a
  Snapshot)` and align any prose in the changed documents that points the
  other way.
- **Add the missing device-state entry for root identities** — the "how it
  maps the Library onto its local folders" enumeration should extend to which
  filesystem each mapped root stood on when a scan last saw it, citing EP-12
  as plain text.

### Index Snapshot and Journal concepts

- **Contrast the two slots in `docs/concepts/index-snapshot/README.md`** —
  `commit slot` and `snapshot slot` both appear with no sentence telling them
  apart; add one bullet (or a sentence in the existing section) that names
  both and states each one's role, so a reader meeting either term can place
  it.
- **Name the snapshot slot in `docs/concepts/journal/README.md`** — the
  Mental Model describes the slot without naming it; use the term `snapshot
  slot` at that point.

### Purpose Key concept (`docs/concepts/purpose-key/README.md`)

- **Register `seal` in the Collocations** — the verb is used across the
  codebase and in three other concept documents for protecting a secret under
  a purpose key, but is registered nowhere. One entry in the existing style.

### Spec register (`docs/spec/`)

- **Record the fetch scratch prefix's actual value** — the documents state the
  "reserved local-name prefix" rule (EP-11) without ever giving the string, so
  a user cannot know which names to avoid. Add the literal `.coffret-fetch-`
  as a sub-bullet under EP-11 in `docs/spec/entry-path/README.md`, matching
  the value in `backend/crates/domain/coffret-usecase/src/scratch.rs`.
- **State OC-7's other half** — `reconcile::completes()` requires
  `state == Spooled` *and* membership in the current set, and its rustdoc
  says the state half is not implied by the membership half; OC-7 states only
  the membership half. Add one sub-bullet under OC-7 in
  `docs/spec/orphan-cleanup/README.md`: a `Spooling` row is never completed,
  whatever the current set says — it is disposed of as this device's own
  reclaimable leftovers. Review OC-2's wording at the same time and touch it
  only if it contradicts the new sub-bullet.
- **Reword OC-6 away from the verb `complete`** — the register uses
  "complete" both for OC-7's bookkeeping completion and for OC-6's finished
  trashing; move the OC-6 side to trash-vocabulary (e.g. "trashed" /
  "finished trashing") so the register's own use of `complete` is single-
  meaning.
- **List PK-17's subject in the Mechanisms table** — `docs/spec/README.md`'s
  table does not mention PK-17's topic (the folder scope of a freeze
  invocation); add it to the pack-construction row's description.

### Entry Path concept (`docs/concepts/entry-path/README.md`)

- **Stop bolding "materialized" at a non-definition point** — the bold at
  line 49 marks no definition (the definition lives in the Library concept's
  Collocations); unbold it and keep the sentence pointing at the term's real
  home. Also align this document's "reported as changed" (line 50) with the
  register's word (see the sweep below).

### Vocabulary sweeps in code prose (rustdoc / comments only)

- **`sync/reconciled.rs` still speaks the pre-#45 vocabulary** — its
  "recorded complete" phrasing (2 hits) collides with `complete`'s reserved
  OC-7 sense; align it with `sync/reconcile.rs`'s post-#45 wording ("marking
  it `Spooled`" / a row that calls its spool `Spooled`).
- **"reported as changed" → "reported as modified"** — the register says
  *modified*; sweep the remaining "reported as changed" occurrences in
  `backend/crates` (e.g. `sync_conformance/scope.rs`, `sync/mod.rs`,
  `sync/scan/mod.rs`) and `docs/concepts/entry-path/README.md` to the
  register's word. Leave ordinary uses of "changed" (e.g. "the batch has
  changed nothing") alone — only the *reported as* phrasing is drifting.
- **`CommitOutcome`'s `untrashed` rustdoc cites nothing** — OC-6 defines the
  untrashed removal it counts; make the field's rustdoc say "untrashed
  removal (spec: OC-6)" so the term's register home is one hop away.
- **TS `containerSummary.ts` trails its Rust twin** — the type-level doc's
  final sentence still reads "the copy a record travels with", which #38
  rewrote on the Rust side; mirror the current Rust wording in
  `frontend/packages/domain/format/src/model/containerSummary.ts` (the
  committed `dist-types/` copy follows from the frontend build).

Conventions per `CLAUDE.md`: English throughout, Conventional Commits,
self-contained commit and PR text (do not reference the merged PR numbers as
justification inside the documents themselves), `make check` as the gate.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The Library concept registers `settle` and `stamp` in its Collocations,
      defines *finding* in its Domain Rules, and bolds **unavailable root** at
      its existing deletion sub-bullet as the definition point (check gates on
      all four).
- [x] The Index concept's pending row text is row-first (mentions the
      `Spooling` / `Spooled` states), registers `announce`, `mark`, and
      `restore (… from a Snapshot)` in its Collocations, and extends the
      device-state enumeration with the EP-12 root identity (check gates:
      `Spooling`, the three Collocations entries, `EP-12`).
- [x] The Index Snapshot concept carries a sentence contrasting the commit
      slot with the snapshot slot, and the Journal concept names the snapshot
      slot in its Mental Model (check gates on both).
- [x] The Purpose Key concept registers `seal` in its Collocations (check
      gate).
- [x] The spec register records the literal `.coffret-fetch-` prefix under
      EP-11, states OC-7's state condition (`Spooling` appears in
      orphan-cleanup), no longer uses the verb `complete` inside OC-6, and
      lists PK-17's subject in `docs/spec/README.md`'s Mechanisms table
      (check gates on all four).
- [x] The vocabulary sweeps hold mechanically: `recorded complete` is gone
      from `sync/reconciled.rs`, `reported as changed` is gone from
      `backend/` and `docs/concepts/`, `**materialized**` is gone from the
      entry-path concept, `untrashed removal (spec: OC-6)` appears in the
      backend rustdoc, and the old "the copy a record travels with" sentence
      is gone from the TS `containerSummary.ts` source (check gates on all
      five).
- [x] `make check` and `RUSTDOCFLAGS=-Dwarnings cargo doc --workspace
      --no-deps --document-private-items` are clean — the doc-comment edits
      broke no build, test, or rustdoc link.

## Out of scope

- **Renaming `PendingUpload`, the `pending_uploads` table, or any code
  identifier** — this is a documentation pass; #45 already did the code-side
  renames and deliberately left the storage names.
- **The `WatchingIndex::refused()` cause string** — six refine passes across
  #45 explicitly left it as ordinary English; do not reopen.
- **The freeze conformance case name `surfaced` overlap noted by #40's
  refine** — recorded as harmless; no edit.
- **Error-type restructuring, NFC normalization, and every other code-side
  ledger item** — queued as separate tasks.
- **The Japanese mirrors of the concept documents** — not in this PR.
