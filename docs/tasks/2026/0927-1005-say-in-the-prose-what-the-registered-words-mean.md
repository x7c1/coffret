---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rqF "reports the root itself" docs/concepts/ && ! grep -rqF "the reclaiming half of a settle, as against" docs/concepts/ && ! grep -rqF "settles the Pack" docs/concepts/ && ! grep -rqF "different findings about the Library" backend/ && ! grep -rq "is_surfaced_by_a_freeze" backend/ && ! grep -rqF "before a fetch places" docs/concepts/ && ! grep -rqF "verified file, leaves all of them" backend/crates/gateway/'
assignee: null
branch: task/0927-1005-say-in-the-prose-what-the-registered-words-mean
created_at: 2026-09-27T10:05:44Z
updated_at: 2026-09-27T10:23:09Z
---

# docs: say in the prose what the registered words mean

## Overview

Six places use a word the concept documents register in a sense the register does not give it, or give a registered word a narrower scope than the code and the spec use. A reader who trusts that the concept documents, the spec and the code name the same thing with the same word draws the wrong conclusion at each of them. Every fix here is prose — concept documents, doc comments, and one test function name — and none changes behaviour.

1. **EP-12 in the Library concept.** `docs/concepts/library/README.md` (the *unavailable root* bullet, around line 152) says "the run reports the root itself" and contrasts that with EP-13's refused root, where "the run reports the mapping". The spec says otherwise: EP-12 in `docs/spec/entry-path/README.md` has the run report "the mapping and the reason", and `UnavailableRoot` in `backend/crates/domain/coffret-usecase/src/unavailable_root.rs` says the run reports the mapping. Bring the concept in line with the spec, and move the contrast between the two bullets to what each check establishes — whether the root is there to be read from, against whether the folder standing at it is the one whose marker the mapping recorded — rather than to what gets reported.
2. **`dispose` in the Index concept.** The Collocations entry in `docs/concepts/index/README.md` (around line 58) defines dispose as "the reclaiming half of a settle, as against completing the bookkeeping of a spool whose batch did commit". OC-7 in `docs/spec/orphan-cleanup/README.md` uses the same word on the completion side too ("the local ciphertext and the provenance itself are disposed of"). Widen the gloss so it covers both halves of a settle.
3. **`settle` as a plain verb.** The survey rule in `docs/concepts/library/README.md` (around line 255) says the survey "settles the Pack's entry table", and the same bullet then uses the registered *settle* ("for the next run to settle"). Replace the first with a verb that does not collide (for example *fixes* or *determines*).
4. **`finding` as a plain noun.** The doc comment on `parse` in `backend/crates/domain/coffret-model/src/control_object_name/parse.rs` (around line 15) calls a corrupt digest field and a non-control object "two different findings about the Library". *Finding* is the registered word for what a run reports without failing (Library concept, "Each file a run surfaces is reported as a finding"). Use *verdicts*, the word `InvalidReplica`'s doc in `coffret-usecase/src/commit/commit_error.rs` already uses for the same idea.
5. **`surface` for a root.** The conformance case `a_missing_mapped_root_is_surfaced_by_a_freeze` in `backend/crates/domain/coffret-usecase/src/freeze_conformance/roots.rs` checks the outcome's `unavailable` field. *Surface* is registered for "a file a run reports rather than silently skips", and `FreezeOutcome::surfaced` is the files. Rename the case after the field it checks (for example `a_missing_mapped_root_is_reported_unavailable_by_a_freeze`), and follow the rename wherever the case is listed.
6. **`vouch` limited to a fetch.** The Collocations entry in `docs/concepts/entry-path/README.md` (around line 29) says the device vouches for a local path "before a fetch places an Entry there". Both local writers vouch — the fetch and the add path (a file taken into a mapped folder from a drop) go through the same `Destinations` capability. Say *before a local writer puts a file there*, in the words EP-11 uses. The neighbouring `place (an Entry at its local path during a fetch)` is right as it is: only a fetch places an Entry. In the same family, the crate doc of `backend/crates/gateway/coffret-local-fs/src/lib.rs` (around lines 17–18) speaks of "the rename that publishes a verified file"; an added file's scratch is not verified against anything, so say what both writers publish.

Guard: `docs/concepts/entry-path/README.md` keeps `place (an Entry at its local path during a fetch)` as it is; the fix in item 6 is to `vouch` alone.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `docs/concepts/` no longer says the EP-12 run "reports the root itself" (grep gate)
- [x] The Index concept's `dispose` gloss no longer limits the word to the reclaiming half (grep gate on "the reclaiming half of a settle, as against")
- [x] The survey rule no longer uses "settles the Pack" (grep gate)
- [x] `backend/` no longer calls the two parse outcomes "different findings about the Library" (grep gate)
- [x] No test name under `backend/` says a missing root "is_surfaced_by_a_freeze" (grep gate)
- [x] `docs/concepts/` no longer limits `vouch` to "before a fetch places" (grep gate), and the local-fs crate doc no longer says the rename publishes a "verified file" (grep gate)
- [x] `make check` passes

### Manual / on-hardware (verified by a human before merge)

- [ ] Reading the EP-12 and EP-13 bullets of the Library concept side by side, the contrast is about what each check establishes, and both agree with the spec about what the run reports
