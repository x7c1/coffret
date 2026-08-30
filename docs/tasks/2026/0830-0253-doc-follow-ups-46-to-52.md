---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && (cd backend && RUSTDOCFLAGS=-Dwarnings cargo doc --workspace --no-deps --document-private-items) && grep -qF "**spool**" docs/concepts/index/README.md && grep -qF -- "- dispose (" docs/concepts/index/README.md && grep -qF "local provenance" docs/concepts/index/README.md && grep -qF "**batch**" docs/concepts/journal/README.md && grep -qF -- "- survey (" docs/concepts/library/README.md && grep -qF "modification time" docs/concepts/library/README.md && grep -qF "untrashed removal" docs/concepts/storage-object/README.md && grep -qF -- "- open (" docs/concepts/purpose-key/README.md && grep -qF "(spec: EP-1)" docs/concepts/entry-path/README.md && grep -qF -- "- range-read (" docs/concepts/container/README.md && grep -q "FM-5" docs/concepts/container/README.md && { awk "/^- \*\*FM-5\./,/^- \*\*FM-6\./" docs/spec/format/README.md | grep -qi writer; } && grep -qF "chunk run" docs/spec/format/README.md && grep -qF "entry table" docs/spec/pack-construction/README.md && { awk "/^- \*\*PK-16\./,/^- \*\*PK-17\./" docs/spec/pack-construction/README.md | grep -qi asked; } && grep -qF -- "- open (" docs/concepts/storage-object/README.md'
assignee: null
branch: task/0830-0253-doc-follow-ups-46-to-52
created_at: 2026-08-30T02:53:14Z
updated_at: 2026-08-30T04:35:00Z
---

# docs: close the concept and spec follow-ups left by #46 to #52

## Overview

Recent merges left a batch of small documentation debts: domain nouns the
code now leans on that no concept document defines, verbs in live use
that no Collocations list registers, and register rules that state only
half of a claim the code cites them for. Each item below names its target
document and the shape of the fix. Concept documents follow the existing
Definition / Domain Rules / Collocations structure; register additions
follow each register's rule style, and a `*(Form: test)*` marker is added
only where existing tests genuinely cover the sentence it marks. English
throughout. No behavior changes; the only source-file candidate is one
doc-comment citation (last item).

**Index concept** (`docs/concepts/index/README.md`):

1. Define the noun **spool**. The pending-row prose (and `sync` /
   `freeze` code prose throughout `backend/crates/domain/coffret-usecase`)
   uses "spool" as a noun — the local file holding a Container's
   ciphertext before upload — but only the verb is registered. Add a bold
   **spool** definition where the pending row is described.
2. Register the verb `dispose` in Collocations. It is the disposal half
   of reconcile (`Reconciled::Disposed`, `reconcile.rs`), currently
   sharing prose with `reclaim`; register `dispose (…)` and either fold
   `reclaim` into it or draw the line between the two.
3. Bridge to the register's term: the orphan-cleanup register calls the
   pending row's testimony *local provenance* (OC-2), and neither
   document says the two names are one thing. Add one sentence at the
   pending-row definition ("the register calls this *local provenance*
   (OC-2)").

**Journal concept** (`docs/concepts/journal/README.md`):

4. Define **batch** in the Definition or Mental Model. The word carries
   the commit flow's unit (`PreparedBatch`, "one batch, one record") and
   is used throughout commit prose without a documented home.

**Library concept** (`docs/concepts/library/README.md`):

5. Give the `SourceChanged` event an address. `FreezeError::SourceChanged`
   (a file changing between the survey and the read stops the Pack) is a
   public failure with no canonical sentence behind it. Add one Domain
   Rule stating that a freeze refuses to pack a file that changed after
   the survey, and register `survey (the files a freeze will pack)` in
   Collocations — the verb the freeze code uses for that first pass.
6. Register the second sense of `stamp`. Collocations already carry the
   EP-12 sense (stamping a root's filesystem identity); the fetch path
   also "stamps a fetched file with the Entry's own modification time"
   (EP-11 / FM-9, `LocalOperation::Stamping`). Add that collocation so
   the two senses sit side by side deliberately.

**Storage Object concept** (`docs/concepts/storage-object/README.md`):

7. Add one sentence for **untrashed removal** — the OC-6 term
   (`CommitOutcome`'s public field) for a removal whose trashing did not
   complete; the concept documents never mention it.
8. Register `open (a Storage Object, once its bytes have arrived)` in
   Collocations. The canon treats fetch-then-open as two names for one
   act while the code performs them as two steps; registering the verb
   with this gloss closes the gap.

**Purpose Key concept** (`docs/concepts/purpose-key/README.md`):

9. Register `open (data sealed under a purpose key)` — the inverse of
   the registered `seal`, already used by canon prose (e.g. the token
   cache: sealed on write, opened on read).

**Entry Path concept** (`docs/concepts/entry-path/README.md`):

10. Add the boundary rule to Domain Rules with its citation. The document
    says an Entry Path has exactly one canonical byte form but never says
    who owes it that form; EP-1's sub-clauses assign the duty (normalize
    external text at the boundary; reject stored bytes that are not
    already canonical). Add one Domain Rules line citing `(spec: EP-1)`.

**Container concept** (`docs/concepts/container/README.md`):

11. Extend the "Fetched whole" Domain Rule with PK-16's second half: a
    client may range-read the chunks covering one Entry as a step inside
    fetching the containing Container, and that does not make an Entry a
    fetch unit. Register `range-read (the chunks covering one Entry of a
    Container)` in Collocations. This also settles the working vocabulary:
    *range read* is the mechanism (PK-16's own words); "partial fetch"
    remains the informal name of the flow that uses it.
12. Add a "Streamable" Domain Rule: the entry table travels ahead of the
    content (FM-2, FM-9) and every chunk authenticates alone (FM-5), so
    neither writing nor reading a Container requires holding it in
    memory. This is the most-cited compound claim in the Rust code
    (eleven call sites cite `FM-2, FM-5, FM-9` together) and currently
    has no concept-level home.

**Format register** (`docs/spec/format/README.md`):

13. FM-5 states its memory-independence only for the reader ("a reader
    never needs the whole Container in memory") while encode-side code
    cites it for the same property. Add a sub-bullet making the writer
    side explicit: chunking is fixed by the header alone, so a writer
    can emit chunk by chunk and never needs the whole Container in
    memory either.
14. Name the **chunk run** in FM-5: consecutive chunks covering one
    plaintext extent form a chunk run, and its ciphertext extent follows
    from the header and the meta section alone. This is the noun the
    `container_reader` API (`ChunkRun`, `ChunkRunReader`) is built on.

**Pack-construction register** (`docs/spec/pack-construction/README.md`):

15. Add the producer-side invariant: a Pack's entry table is settled
    before any content is written — the layout rules (FM-2, FM-9) fix
    the bytes' order, but no PK rule obliges `freeze` to have the table
    final before streaming content, and four code sites assert exactly
    that. Add it as a PK-1 sub-bullet or a new rule, citing the FM rules
    it leans on.
16. Add the range-read verification duty to PK-16: a range read holds
    what came back against the extent it asked for, so a provider that
    ignored or inflated the range is refused rather than decoded, and a
    range the object's own stream does not reach names no chunk. Mark
    `*(Form: test)*` only if the existing conformance cases (the partial
    fetch cases in `coffret-usecase/src/fetch_conformance/partial.rs`
    and the `collect_exact` / `ChunkRunTruncated` / `ChunkRunOverrun`
    unit tests) genuinely cover the sentence.

**Source-file citation (comment-only, optional):**

17. `backend/crates/domain/coffret-format/src/meta/encode.rs:24` cites
    PK-3; verify the sentence against PK-3's text (segmentation: Entry
    Path order, size target) and re-cite the rule that states the claim
    if it is misattributed (the same correction family as the recent
    citation fixes; `container_writer.rs:26` was checked and is
    legitimate). If the citation is correct, leave it and say so in the
    PR description.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The Index concept defines the noun **spool**, registers `dispose`,
      and bridges *local provenance* (grep gates on `**spool**`,
      `- dispose (`, and `local provenance` in
      `docs/concepts/index/README.md` — all absent today, appended to
      `check_command`).
- [x] The Journal concept defines **batch** (grep gate on `**batch**`).
- [x] The Library concept gains the survey rule and the second `stamp`
      sense (grep gates on `- survey (` and `modification time`).
- [x] The Storage Object concept mentions **untrashed removal** and
      registers `open` (grep gates on `untrashed removal` and
      `- open (`).
- [x] The Purpose Key concept registers `open` (grep gate on `- open (`).
- [x] The Entry Path concept's Domain Rules cite EP-1 (grep gate on
      `(spec: EP-1)`).
- [x] The Container concept registers `range-read` and gains a Domain
      Rule citing FM-5 (grep gates on `- range-read (` and `FM-5`).
- [x] FM-5 speaks to the writer side and names the chunk run (gates: the
      FM-5..FM-6 range contains `writer`; the register contains
      `chunk run`).
- [x] The pack-construction register states the table-before-content
      invariant and the range verification duty (gates: the register
      contains `entry table`; the PK-16..PK-17 range contains `asked`).
- [x] `make check` and rustdoc (`RUSTDOCFLAGS=-Dwarnings`) stay green.

## Out of scope

- Any behavior, test, or wire-format change.
- Cross-cutting code renames (e.g. unifying `finding` / `claimed`
  vocabulary in Rust identifiers) — those are their own tasks.
- Restructuring any concept document beyond the additions above.
