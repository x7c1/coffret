---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && test ! -e backend/crates/domain/coffret-usecase/src/commit/settle.rs && test -f backend/crates/domain/coffret-usecase/src/commit/after_commit.rs && grep -qF "**After the commit.**" backend/crates/domain/coffret-usecase/src/commit/mod.rs && ! grep -rqE --exclude-dir=target "settle::(trash_removals|write_checkpoint)" backend/crates/ && ! grep -qF "the settle unchanged" backend/crates/domain/coffret-usecase/src/freeze/mod.rs && ! grep -rqiE "settles (which|it\b|neither|what the bytes|them)|is settled by|which is settled|settled nothing|settled loss" docs/spec/ && ! grep -rqiE --exclude-dir=target "settles nothing|settled nothing|entry table is settled" backend/crates/'
assignee: null
branch: task/0927-1439-keep-settle-for-what-an-interrupted-run-left-behind
created_at: 2026-09-27T14:39:43Z
updated_at: 2026-09-27T15:09:47Z
---

# refactor: keep settle for what an interrupted run left behind

## Overview

The Library concept registers *settle* for one act: "what an interrupted run left behind, before this one scans". The sync stage that does it is now `sync::settle`, `Settled` and `Phase::Settling`. Two other uses of the word remain, and a reader who trusts the vocabulary reads both as that act.

1. **The commit's follow-up step is also called a settle.** `backend/crates/domain/coffret-usecase/src/commit/mod.rs` lists step 7 as "**Settle.** Refresh the Index with the batch, trash what the batch removed, and write the checkpoint if the policy asks for one", and the step lives in `commit/settle.rs` (`settle::trash_removals`, `settle::write_checkpoint`, called from `commit/run.rs`). `freeze/mod.rs` says "the rebase, and the settle unchanged" in the same sense. Rename the module to `commit/after_commit.rs`, the step to "**After the commit.**", and every reference in doc comments and calls. This step is commit-protocol mechanics (CP-14, CK-8, CK-10, CK-11), so it takes a descriptive name and no concept-document entry.
2. **settle as a plain verb — decide, answer, resolve.** Across the spec, the concept documents and the code, *settle* also means "decide" ("the marker that settles which folder it is"), "answer" ("the read that settles it"), "resolve" ("the reserved name is settled by naming a different Entry Path"), "establish" ("Authentication settles what the bytes are"), "come to rest" ("a wait for the sync to settle") or "final" ("a settled loss"). Replace each with the verb its sentence means. In the spec these are in `docs/spec/entry-path/README.md` (EP-12 to EP-14 prose), `docs/spec/format/README.md` (FM-1 prose), `docs/spec/checkpoint-and-prune/README.md` (CK-11, "the refusal settled nothing") and `docs/spec/commit-protocol/README.md` (CP-3, "a settled loss"). In the code they are doc comments, comments, test names and messages across `coffret-device`, `coffret-server`, `coffret-format`, `coffret-usecase` and the explorer (`frontend/packages/apps/web/src/`) — `git grep -niE "\bsettl" -- backend/crates frontend/packages docs` lists every occurrence to classify. A user-facing sentence that uses the plain verb (for example a refusal telling a person what settles it) is reworded the same way.

Keep every occurrence that means the registered act — settling an interrupted run's pending rows or spools, the `Settled` outcome and its findings, `Phase::Settling` and the wire value `settling`, and the concept documents' own uses — exactly as they are. When a sentence is ambiguous, prefer the concept document's phrasing ("what an interrupted run left behind").

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `commit/settle.rs` is gone, `commit/after_commit.rs` exists, and the commit module names step 7 "After the commit." (file and grep gates)
- [x] No call reaches `settle::trash_removals` or `settle::write_checkpoint`, and `freeze/mod.rs` no longer calls the commit's follow-up "the settle" (grep gates)
- [x] The spec no longer uses the plain verb in the known phrasings ("settles which / it / neither / what the bytes / them", "is settled by", "which is settled", "settled nothing", "settled loss") (grep gate)
- [x] No backend comment says a refusal "settles nothing" (grep gate)
- [x] `make check` passes
