---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check"
assignee: null
branch: task/1124-2335-say-what-helps-when-a-file-clashes-with-a-mapped-folder
created_at: 2026-11-24T14:35:05Z
updated_at: 2026-11-24T15:11:19Z
---

# fix: say what helps when a file clashes with a folder a mapping represents

## Overview

A sync or a freeze leaves out a file whose addition would make one Entry Path both a file and a folder, and says so (spec: EP-6): `it was left out, because adding it would make "notes" both a file and a folder; rename the file or the folder, then run again` in the CLI, and the same sentence ending "then try again" in the explorer's Activity panel.

Where the folder is one a top-level mapping of this device represents, that advice misleads. Take a Library-root mapping holding a file `notes` and a top-level mapping `notes` whose local folder is `~/Notes`. Whichever of the two is left out — the file `notes`, or every file under the mapping — the sentence names `"notes"` and says to rename the file or the folder. A person takes "the folder" to be `~/Notes` and renames it, and nothing changes: a mapping's place in the Library is the name it was mapped at, not its local folder's name, and a device has no way to take a mapping away or move it to another name. What helps is renaming the file `notes`.

Make the sentence say that in this case: when the path that would be both a file and a folder is the name a top-level mapping of this device is mapped at, the finding says the folder is the one mapped at that name, that renaming that folder on disk changes nothing, and that the remedy is to rename the file — never "rename the folder", and never a step the device does not offer. In every other case the sentence stays as it is. This holds for both runs that leave such a file out (sync and freeze) and for both places the finding is read (the CLI's answer and the explorer's Activity panel, whose sentences come from the server).

Where to tell the two cases apart is yours to choose: the place that already knows this device's mappings when it turns a run's finding into what a person reads is the natural one, and the run's own finding (`Surfaced::FileAndFolder`, `NotFrozen::FileAndFolder`) need not change if it can be told there. Keep the wording plain: say "file" and "folder", quote the path, and say what to do. Site names are from when this was written; confirm each.

Change no behavioural rule in `docs/spec/`; where a concept doc or a contract fixture quotes the sentence, keep it true. Change nothing under `.github/`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes
- [x] A test pins the CLI's sentence for a file left out because of a folder a top-level mapping represents: it names the path, says the folder is mapped at that name, offers renaming the file, and does not offer renaming the folder or mapping it elsewhere
- [x] A test pins the same for the sentence the server gives the explorer
- [x] A test pins that where no mapping stands at the path, both sentences are unchanged
- [x] The freeze's finding is covered as the sync's is

### Before merge (verified outside the check command)

- [x] The PR description quotes the sentences before and after, for the CLI and the explorer
