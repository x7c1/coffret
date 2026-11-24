---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, spec-alignment, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check"
assignee: null
branch: task/1124-1804-call-materialization-records-by-their-name-and-fix-two-citations
created_at: 2026-11-24T09:04:26Z
updated_at: 2026-11-24T09:25:20Z
---

# docs(spec): call the materialization records by their name, and fix two citations

## Overview

The Index concept doc (`docs/concepts/index/README.md`) defines the device's own records of what it has on disk as **materialization records**, and keeps **present** for a state: an Entry is present or remote, and a materialization record says present or absent. The spec and the code's comments call the same records "the present records" in several places — LA-45's text and its Form note, LA-48, and doc comments and test comments under `backend/crates/`. A reader who takes the concept doc's vocabulary as given reads "present records" as something else, or as only the records that say present.

Apply one rule to every site: **the device's records of what it has on disk are called materialization records, as the concept doc calls them.** Find the sites with a search for `present record` over `docs/spec/`, `docs/concepts/`, `backend/crates/` and `frontend/packages/`, and search again with the two words split across a line break (the spec wraps its lines, and LA-48 has one such site). For each site, read the sentence and decide which of two things it means:

- the records as a kind — "the Entries, the present records, the Containers", "a write of the present records": say **materialization records**;
- the records that say present, as a subset — "every present record under a folder", what `present_under` answers: keep the meaning, and say it in the concept doc's words where the short form would be read as the kind (for example "the materialization records that say present"). Where the surrounding text already makes the subset plain, leave it.

Change words in prose, comments and doc comments only. Rename no identifier, table, column, function or test; change no behaviour and nothing a rule asks for. Keep each rule's Form note true to its text. Do not edit files under `docs/tasks/`.

Two citations are corrected in the same change, since both are one-line corrections to the same rules' text and comments:

- **LA-46 cites LA-12 for something LA-12 does not say.** LA-46 (`docs/spec/loopback-access/README.md`) says the preview answers "whether a deletion is already running, behind which this one would wait (LA-12)". LA-12 describes the running state and the log; it does not say that deletions run one at a time and that a later one waits behind an earlier one. Read the code that queues deletions (the server's delete run and its tests) and make the citation true: either state in LA-12, or in the rule that owns the queueing of work, that a deletion asked for while another runs waits behind it — only if the code and a test show exactly that — or cite the rule that does say it, or drop the citation and keep the clause. Write what the code does; add no rule that asks for behaviour the code lacks.
- **`backend/crates/apps/coffret-device/src/browse/folders.rs` cites LA-45 alone.** Its doc comment describes what `GET /api/folders` answers, which LA-47 now defines. Cite LA-47 for what is answered and keep LA-45 for what it costs.

Change nothing under `.github/`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes
- [x] A search for `present record` over `docs/spec/`, `docs/concepts/`, `backend/crates/` and `frontend/packages/`, including the two words split across a line break, finds no site that names the records as a kind
- [x] LA-46's clause about waiting behind a running deletion cites a rule that says so, or cites none
- [x] `browse/folders.rs` cites LA-47

### Before merge (verified outside the check command)

- [x] The PR description lists the sites changed, the sites left as they were with the reason, and what was done about LA-46's citation and the code it was read from
