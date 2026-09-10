---
status: completed
pipeline_phase: null
plan: null
base_ref: feat/mapped-root-marker
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -q "temporary file" docs/concepts/library/README.md && grep -qE "^- scratch " docs/concepts/library/README.md && grep -q "(spec: EP-13" docs/concepts/library/README.md && grep -q "(spec: EP-13" docs/concepts/index/README.md'
assignee: null
branch: task/0910-1245-let-the-library-and-index-concepts-say-what-the-marker-rules-changed
created_at: 2026-09-10T12:45:12Z
updated_at: 2026-09-10T15:58:31Z
---

# docs(concepts): let the Library and Index concepts say what the marker rules changed

## Overview

The register now states the mapped-root marker (EP-13), the reserved name
`.coffret` (EP-14), the word *scratch* and the definition of a materialization
record *agreeing* with the file on disk (EP-11), and idempotent local clean-up
(OC-8), and the Entry Path concept follows it. Two other concept documents
still describe the older state:

- `docs/concepts/library/README.md` says "A fetch writes its temporary file
  inside a mapped folder …" where the register now says *scratch*, its
  Collocations list has `spool` but no `scratch`, and its **unavailable root**
  sub-bullet (under the rule that a scan reports an Entry as deleted locally
  only where the device materialized it) describes what stops a root from
  being used with the filesystem identity alone; EP-13 adds a second, separate
  refusal — a root whose marker is missing, malformed, or carries another
  identifier is not written into even when it is available — and the concept
  does not say so.
- `docs/concepts/index/README.md` enumerates the device's own state kept
  beside the catalog (mappings, which filesystem each mapped root stood on,
  which Entries it has materialized, spool state); EP-13 adds the expected
  identity recorded for each mapping, and EP-11's definition of agreement
  makes explicit that the materialization record carries the placed file's
  byte length and modification time.

Bring both documents in line with the register. A concept document keeps
meaning and guarantees; cite the rule rather than restate its mechanics:

- Library concept: change the fetch bullet to say *scratch* (the word EP-11
  uses for the file a fetch writes under the reserved prefix before the rename
  that publishes it), keeping the prefix's cost sub-bullet as it is; add
  `scratch` to the Collocations next to `spool`, in the list's existing form
  (`scratch (…)` with a short parenthetical); and add, under the unavailable
  root sub-bullet, one sub-bullet stating that a root is also refused for
  placement when the marker recorded for it at registration is absent or does
  not match, so a disk that came back empty or a folder that answers to the
  registered name is never written into `(spec: EP-13)`.
- Index concept: extend the device-state enumeration with the expected
  identity recorded for each mapping, and say that the record naming an Entry
  present also carries the placed file's length and modification time, which
  is what a fetch compares before it will replace that file; cite
  `(spec: EP-13, EP-11)` alongside the existing citations.

Change nothing else in either document. Both are cold-reader documents:
read each top to bottom first, keep the sentence rhythm of its neighbours,
and wrap at the document's existing width.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The Library concept says scratch and lists it as a collocation:
      `! grep -q "temporary file" docs/concepts/library/README.md`,
      `grep -qE "^- scratch " docs/concepts/library/README.md`.
- [x] Both concepts cite the marker rule:
      `grep -q "(spec: EP-13" docs/concepts/library/README.md`,
      `grep -q "(spec: EP-13" docs/concepts/index/README.md`.
- [x] Existing backend, frontend, and interoperability checks continue to pass.

## Out of scope

Any change to the register or to the Entry Path concept. Any code change.
