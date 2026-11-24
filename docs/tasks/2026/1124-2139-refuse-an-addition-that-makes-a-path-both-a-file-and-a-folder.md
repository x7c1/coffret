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
branch: task/1124-2139-refuse-an-addition-that-makes-a-path-both-a-file-and-a-folder
created_at: 2026-11-24T12:39:25Z
updated_at: 2026-11-24T13:57:30Z
---

# fix: refuse an addition that would make one path both a file and a folder

## Overview

A Library can come to hold an Entry at a path and other Entries under that same path — `notes` beside `notes/x.pdf`. No device can materialize both, because a file system gives one name in a folder to one thing (spec: EP-4). A rename is already refused where it would leave such a pair (spec: EP-5, `RenameRefusal::FileAndFolder`, `backend/crates/domain/coffret-usecase/src/rename/plan.rs`), but an addition is held only to "no two Entries at one path" (spec: EP-6, `backend/crates/domain/coffret-usecase/src/commit/candidate.rs`), so additions make what renames may not:

- two devices, one adding a file at `docs/notes` and the other a file at `docs/notes/x.pdf`;
- one device with a Library-root mapping holding a file `notes` and a top-level mapping `notes` holding files — in either order, since adding a mapping does not ask whether an Entry stands at its prefix;
- a detected rename refused as `FileAndFolder`, whose file is then "carried in as a new file instead" (`backend/crates/apps/coffret-device/src/findings.rs`) — the addition makes the pair the rename was refused for.

Close this for additions, in the spec and in the code. Site names and line numbers are from when this was written; confirm each.

**The rule.** An addition is refused where the state its commit would leave has one Entry Path both an Entry and a folder of Entries: the added Entry's path lies under a path a current Entry stands at, or a current Entry stands under the added Entry's path. Removals and renames of the same batch apply first, as they do for the path-uniqueness check, and the additions of one batch are held to the rule against each other too. Write it in `docs/spec/entry-path/README.md` where it belongs (EP-6, with EP-5's sentence about renames and EP-7's rebase rules kept consistent with it), and in the concept docs that state what an addition is refused for.

- **At commit.** The candidate check refuses such a batch before anything is written, naming the path, with its own error (not `EntryPathCollision`: the paths differ, and a person needs to be told which path would be both). Do not read every Entry of the Library for it: ask the Index only about the paths involved, as `rename/plan.rs` does for a rename (`standing_near`, `clash`); share that logic rather than writing it twice.
- **After a lost commit race.** A writer that rebases repeats the check against the new head, so two devices adding `a` and `a/b` at once end with the later one refused as a conflict, like two writes to one path (spec: EP-7, CP-7).
- **A catalog replay is not held to it.** A device applying committed records must keep applying them; the rule is about what a commit may newly make. EP-5 already says a Library holding such a pair keeps it. No migration and no repair of an existing pair is wanted; say in the rule what an addition beside an existing pair meets (it is refused like any other the rule covers).

**Where a person meets it.** A refused commit holds back the whole batch, so the commit check is the backstop, not the usual way to learn of this.

- **Sync.** Before a file is packed or uploaded, a sync leaves out a local file whose addition the rule would refuse against the catalog the device holds, reports it — the file, the path that would be both a file and a folder, and what to do (rename one of them) — and carries on with the rest. The report reaches every place a sync's findings reach (the CLI's answer, the explorer's Activity panel). Check every other path that makes additions the same way (files dropped into the explorer, a detected rename that falls back to an addition, anything else you find) and hold each to the rule; list them in your report.
- **A file standing at another mapping's prefix.** Today the walk of a Library-root mapping skips every top-level name another mapping represents, whether a file or a folder stands there (`backend/crates/domain/coffret-usecase/src/local_scan/walk_mappings.rs`, "this is where a subtree another mapping represents is left to it"). A file there is then seen by no walk: it is never added, and if it has a materialization record it is reported as deleted on this device while it still stands. A file is not a subtree. Make the Entry at a top-level mapping's own prefix part of the remainder the Library-root mapping represents: the root mapping's walk skips the name only when a folder stands there, the scan's deletion read and the Index reads behind it (`present_outside`, `present_in`, `present_under`, `sync/scan/deletions.rs`'s `Read`) attribute the Entry at the prefix to the root mapping, and EP-9 says so. With that, such a file meets the rule above like any other and is reported, instead of vanishing. A conformance case added earlier pins the opposite attribution (`a_file_at_a_top_level_mappings_own_prefix_is_reported_once_within_the_root`, and the "notes" row in `the_present_rows_outside_some_folders_are_the_remainder`); change them to the new rule, and keep "reported once, never twice".
- **Adding a mapping.** Adding a top-level mapping whose prefix is a path a current Entry stands at is refused with a message naming the Entry, since nothing under it could be added. The state can still arise afterwards (another device adds the file first); the rule above covers that.

Keep the wording a person reads plain: say "file" and "folder", name the path, and say what to do. Add or change no other behavioural rule in `docs/spec/`. Change nothing under `.github/`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes
- [x] A commit conformance case pins that a batch adding an Entry under a path a current Entry stands at, and one adding an Entry at a path current Entries stand under, are each refused before anything is written, naming the path; and that the same paths pass when the batch removes or renames the other Entry away
- [x] A commit conformance case pins that of two writers adding `a` and `a/b` at once, the one that loses the race is refused after its rebase
- [x] A sync conformance case pins that a local file whose addition would make a path both a file and a folder is left out and reported, and that the other files of the same run are still added
- [x] A sync conformance case pins that a file standing at a top-level mapping's own prefix under the Library-root mapping is walked by the root mapping, is not reported as deleted while it stands, and is reported once when it is gone
- [x] A test pins that adding a top-level mapping at a path a current Entry stands at is refused
- [x] Each Index port method added or changed has a conformance case run against both indexes
- [x] The spec states the rule for additions, EP-9 states which mapping represents the Entry at a top-level mapping's own prefix, and every rule cited by the new code and tests exists (`make spec-citations spec-rule-ids`)

### Before merge (verified outside the check command)

- [x] The PR description lists every path that makes additions and how each is held to the rule, says what a person sees when a file is left out, and says that no existing pair is migrated or repaired
