---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, spec-alignment, concept-alignment, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check"
assignee: null
branch: task/1125-0032-refuse-a-freeze-whole-when-a-file-would-make-a-path-a-file-and-a-folder
created_at: 2026-11-24T15:32:15Z
updated_at: 2026-11-24T16:22:49Z
---

# fix: refuse a freeze whole when a file it selects would make a path both a file and a folder

## Overview

A freeze packs the files of one folder into one Pack. Since additions are held to "no Entry Path is both a file and a folder" (spec: EP-6), a freeze leaves out each selected file whose addition would break that rule, reports it, and packs the rest (`backend/crates/domain/coffret-usecase/src/freeze/scan/files_and_folders.rs`, `NotFrozen::FileAndFolder`).

That is right for a sync, whose files are added one by one, and wrong for a freeze. A Pack is one thing — a book, an album — and the file left out is one the person meant to be in it and could have been, had a name been changed first. Once the Pack is committed there is no way to put the file in: it can only become an Entry of its own beside the Pack, or the Pack must be deleted and frozen again. And the freeze's preview does not ask the rule at all, so it counts the file among those it would pack.

Make a freeze all or nothing where this rule is concerned:

- **The run.** Where any file a freeze selects would make a path both a file and a folder, the freeze packs nothing and commits nothing. It reports every such file — the file, the path that would be both, and what to do — and says that nothing was frozen and that the freeze can be asked again once the names are changed. The other reasons a freeze leaves a file alone (already held by a Pack, changed in a Pack, still being received, and so on) keep the behaviour they have.
- **Reservations.** A drop that reserved its files for a Pack keeps its reservation through such a refusal, as it does through any run that stops before deciding it (spec: PK-17), so the Pack the person asked for is still what the next freeze makes. PK-17 at present lists "left out because adding it would make a path both a file and a folder (EP-6)" among the reasons a reservation is released; that no longer happens, so take it out and say what does.
- **The preview.** A preview asks the same question the run will, and says when the freeze would be refused: how many files stand in the way (and enough to name them, in whatever form the preview's other counts and the explorer's dialog suit). The explorer's freeze dialog then says so before the person confirms, with the same remedy, and does not offer to start a freeze that will be refused.
- **Where it is read.** The refusal reaches the CLI's answer and the explorer (the dialog for a preview, the Activity panel for a run), in plain words: "file", "folder", the path quoted, what to do. Where the folder is one a top-level mapping represents, keep the remedy the finding gives for that case (rename the file; renaming that folder on disk changes nothing).
- **Spec and concepts.** EP-6's sentence about what a writer does with such a file now differs by writer: a sync leaves the file out and carries on; a freeze is refused whole. Say so there. EP-6 also says that "renaming one of the two is what to do", which is not so where the folder is one a top-level mapping represents — renaming that folder on disk changes nothing, and only renaming the file helps; make the sentence true of that case too. Keep PK-17 and the concept docs that describe a freeze consistent, and cite the rules from the code and tests that implement them.

A sync is not changed. Site names are from when this was written; confirm each. Change nothing under `.github/`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes
- [x] A freeze conformance case pins that a freeze selecting one file that would make a path both a file and a folder, among others that would not, packs nothing, commits nothing, and reports that file with the path
- [x] A freeze conformance case pins that a reservation covering such a file is kept as it was through the refusal, and that the freeze packs everything once the clash is gone
- [x] A test pins that the preview reports the files standing in the way, and that it agrees with what the run then does
- [x] A web test pins that the freeze dialog says the freeze cannot run and why, and offers no way to start it
- [x] Tests pin the sentence the CLI gives and the one the server gives the explorer for the refusal
- [x] The spec says what a sync and what a freeze does with such a file, PK-17 no longer lists it among the reasons a reservation is released, and every rule cited by the new code and tests exists (`make spec-citations spec-rule-ids`)

### Before merge (verified outside the check command)

- [x] The PR description says what a person sees in the preview and after a refused run, in the CLI and in the explorer, and says that the look of the explorer's dialog was checked by tests only
