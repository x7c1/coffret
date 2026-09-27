---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && C=docs/concepts && grep -qE "^- discard \(" $C/library/README.md && [ "$(grep -cE "^- discard \(" $C/library/README.md)" -ge 2 ] && grep -qiE "stopped with|what stopped it" $C/library/README.md && grep -qE "^- rewrite \(a (missing|replica)" $C/keyring/README.md && grep -qiE "already .?Spooled.? .*object handle|object handle.*already .?Spooled" $C/index/README.md && [ "$(git diff origin/main -- docs/spec/ | grep -cE "^\+.*Form: (test|prose)")" -ge 1 ]'
assignee: null
branch: task/0927-2021-let-the-concept-documents-catch-up-with-the-code
created_at: 2026-09-27T20:21:00Z
updated_at: 2026-09-27T20:50:01Z
---

# docs: let the concept documents and the spec register catch up with the code

## Overview

Recent changes gave the code rules and words that the concept documents and the spec register do not yet state, and in one case gave a registered word a narrower scope than the code has long used it in. A reader who trusts the concept documents as the vocabulary and the spec register as the rules draws the wrong conclusion at each of these. Every fix here is prose in `docs/concepts/` and `docs/spec/`; no code changes.

1. **`discard` is used for more than the queue.** The Library concept (`docs/concepts/library/README.md`, Collocations) now registers *discard* for "the folders waiting behind a run, when the run's worker ends without an answer". The code has long used the same verb for a local writer throwing away what it will not publish: a fetch's scratch (`Scratch::discard`, `Placement::discard`, `Decoding::discard`, `discard_all` in `coffret-usecase/src/fetch/`), and a spool (`Spool::discard`). Register that sense as its own entry, the way the Collocations already list *stamp* and *vouch* twice with different objects, for example `discard (a scratch or a spool a local writer will not publish, before the rename that would have published it)`. Set the two entries against each other the way *supersede* / *displace* are.
2. **A stopped run says what stopped it.** The work answer now types this: a run whose status is stopped carries the refusal that stopped it, a run that did not stop carries none, and a displaced run keeps the refusal it stopped with. The Library concept states nothing of it. Add a Domain Rule saying so, and add to the *displace* entry that a displaced run is always one that stopped. Cite LA-12 (`docs/spec/loopback-access/README.md`) where the work answer is specified, and add to LA-12 the sub-rule that a run's reported status and its refusal go together (stopped ⇔ a refusal; the catalog's catch-up likewise — behind ⇔ a refusal), in the register's own form (`docs/spec/README.md`: rules keep their IDs, a sub-bullet under the rule it refines, a Form tag).
3. **Marking a row that is already spooled keeps its object handle.** The index conformance suite now requires it (`coffret-usecase/src/index_conformance/device_state.rs`, and the port doc of `Index::mark_spooled` in `coffret-usecase/src/index.rs` says it). The Index concept's "Spool states of a pending row" table (`docs/concepts/index/README.md`) does not. Add one sentence after the table.
4. **Repair rewrites replica positions.** The Keyring concept's Collocations register *rewrite* only for "the Keyring when rotating the Master Key", while its Mental Model and KL-13 / KL-15 use *rewrite* for putting back a missing or unreadable replica position from a surviving replica during repair (`coffret-usecase/src/commit/rewritten_replicas.rs`, the CLI's repair line). Register that sense beside the existing one.
5. **An upload is verified against the provider's own digest in its write answer.** The code now checks each uploaded Container against the digest the provider reports in the answer to the write (`ObjectStore::put` → `UploadedObject`; a mismatch is `TransferCorrupted`; an S3 write answered without an ETag, or a Drive write without `md5Checksum`, is refused). No spec rule states this. Add one to the register where upload integrity belongs — read `docs/spec/commit-protocol/README.md` (CP-11 and the upload rules around it) and `docs/spec/format/README.md` (FM-15) and place it by the register's own rules (`docs/spec/README.md`: a new ID is appended, never reused; Form tag). Say what is compared and what a mismatch or a missing digest does; do not name provider SDKs.
6. **The Index's *catalog* has two senses.** The Index concept (`docs/concepts/index/README.md`) opens with "Index is a device-local catalog", and its Domain Rule says "This device's own state is kept beside the catalog rather than in it" — the narrow sense, the Library-wide listing. The port doc (`coffret-usecase/src/index.rs`: "The device-local catalog of one Library"), the concept index (`docs/concepts/README.md`: "the local catalog of the Library"), and the SQLite adapter use the word for the Index as a whole. Resolve it in the concept: say in the Definition which sense the word carries where, so both readings are registered rather than one silently contradicting the other. Change code prose only if one sense turns out to be wrong where it stands (none is expected).

Guard: no code changes, no rule removed or renumbered. Every addition cites the rule IDs it rests on. The Japanese copies of the concept documents live outside this repository and are not part of this change.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The Library concept registers *discard* in two senses (count gate), and states that a stopped run says what stopped it (grep gate)
- [x] The Keyring concept registers *rewrite* for putting back a replica position (grep gate)
- [x] The Index concept states that marking an already-spooled row keeps its object handle (grep gate)
- [x] `docs/spec/` gains Form-tagged rule text for item 2 and item 5 (grep gate over the diff)
- [x] `make check` passes

