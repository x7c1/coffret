---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && ! grep -n 'spec: OC-6' backend/crates/domain/coffret-usecase/src/sync/reconcile.rs && ! grep -rni 'stream' backend/crates/domain/coffret-usecase/src/ | grep 'spec: PK-3'"
assignee: null
branch: task/0830-0137-correct-misattributed-spec-citations
created_at: 2026-08-30T01:37:16Z
updated_at: 2026-08-30T03:02:00Z
---

# docs(backend): correct spec citations that point at the wrong rule

## Overview

Two families of `(spec: …)` citations in `coffret-usecase` doc comments
attribute a claim to a rule that does not state it. Comment-only change;
no behavior moves.

1. **PK-3 cited for streaming / memory-boundedness claims.** PK-3
   (`docs/spec/pack-construction/README.md`) is the segmentation rule —
   Entry Path order, size target, oversized singletons. It says nothing
   about the entry table preceding the content or about streaming. Doc
   comments that cite it for "the entry table is settled before the
   content streams" / "nothing needs to fit in memory" claims are pointing
   at the wrong register entry; the authority for layout-before-content is
   the format register (FM-2 — header/meta section precede the chunked
   stream — and the FM-9 entry-table rules; cite the rule(s) whose text
   actually carries the claim, checking `docs/spec/format/README.md`).
   Known suspects (verify each against the rule text; keep citations that
   genuinely make segmentation claims):
   - `src/freeze/freeze_error.rs` ("… lets the content stream (spec: PK-3)")
   - `src/sync/spool.rs` ("… through the streaming encoder instead (spec: PK-3, PK-5)")
   - `src/local_scan/source_file.rs` ("neither step is bounded by what fits in memory (spec: PK-3, PK-5)")
   Legitimate PK-3 citations (ordering / size-target / singleton claims in
   `src/freeze/{mod,run,survey,segment,freeze_outcome,freeze_request}.rs`)
   stay as they are unless reading the rule text says otherwise.

2. **OC-6 cited for claims OC-2 / OC-4 own.** Both `(spec: OC-6)`
   references in `src/sync/reconcile.rs` (a doc comment and an inline
   comment) assert bookkeeping facts about pending rows and provenance —
   OC-2 / OC-4 territory (`docs/spec/orphan-cleanup/README.md`). OC-6 is
   about trashing completion. Re-point each to the rule whose text states
   the claim.

For every citation touched, read the cited rule's actual text and the
surrounding prose, and re-cite the rule that states the claim — do not
mechanically swap identifiers. If a sentence turns out to make two claims
owned by two rules, cite both. The change is comments only: no code,
no test, and no spec register edits.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `src/sync/reconcile.rs` no longer cites OC-6
      (`! grep -n 'spec: OC-6' …/sync/reconcile.rs` — appended to
      `check_command`; it matches twice today, so the gate flips with the
      change).
- [x] No line in `coffret-usecase` that speaks of streaming still cites
      PK-3 (`! grep -rni 'stream' …/coffret-usecase/src/ | grep 'spec: PK-3'`
      — appended to `check_command`; it matches `freeze_error.rs` and
      `spool.rs` today, so the gate flips with the change).
- [x] The tree still passes `make check` (comment-only change: rustdoc,
      clippy, fmt, and the test suite are unaffected in behavior but must
      stay green).

## Out of scope

- Any change to the spec registers themselves (`docs/spec/`).
- Any code or test change.
- Other citation families not named above — new misattributions found
  while reading may be reported in the PR description but belong to their
  own follow-up unless they are the same two families.
