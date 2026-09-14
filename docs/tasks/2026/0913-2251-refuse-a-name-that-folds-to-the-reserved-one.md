---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [completeness, concept-alignment, error-type-design, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -rq "a_case_variant_of_the_reserved_name_is_refused_rather_than_skipped" backend/crates/ && grep -rq "a_folder_that_folds_to_the_reserved_name_is_not_called_an_interrupted_registration" backend/crates/ && grep -rq "a_scan_reports_a_folder_that_folds_to_the_reserved_name" backend/crates/'
assignee: null
branch: task/0913-2251-refuse-a-name-that-folds-to-the-reserved-one
created_at: 2026-09-13T22:51:40Z
updated_at: 2026-09-14T01:56:26Z
---

# fix(backend): refuse a name that folds to the reserved one, and say whose folder it is

## Overview

`EP-14` reserves the name `.coffret` and says a scan "decides from the name
alone". On a filesystem that folds ASCII case — APFS as macOS ships it, and an
exFAT volume on either platform — the kernel and this code do not agree on what
"the name" is, and they fail in opposite directions on the same volume.

### 1. The kernel folds and the code does not

`backend/crates/apps/coffret-device/src/mapping/root_marker/management_area.rs`
opens the area by name:

```
fn enter(directory: &OwnedFd, name: &str) -> std::result::Result<OwnedFd, Errno> {
    rustix::fs::openat(
        directory,
```

On a folding volume `openat(dir, ".coffret", …)` opens an on-disk `.COFFRET`,
and **what it returns is a file descriptor, which carries no name**. Nothing
downstream of that open can tell which spelling it reached.

Every name-side check compares exactly.
`backend/crates/domain/coffret-usecase/src/root_marker.rs`:

```
pub fn is_management_area(name: &str) -> bool {
    name == MANAGEMENT_AREA
}
```

and `carries_management_area` asks the same of each component of an Entry Path.
Both are asked of a spelling that came from a directory listing or from a
catalog — the on-disk one.

So two failures, both real, both on one volume:

- **A person's own folder named `.COFFRET`.** Registration's `enter_or_make`
  opens it, finds no `root` inside, and raises `ManagementAreaIncomplete`, whose
  sentence (`backend/crates/apps/coffret-device/src/error.rs`) reads, with the
  two name constants rendered as a person sees them:

  > holds a `.coffret` folder with no `root` in it, which is what an interrupted
  > registration leaves; nothing was written and nothing was recorded, and
  > recording the mapping again meets this same refusal until that folder is out
  > of the way

  It is not debris from an interrupted run. It is their folder, and the sentence
  tells them to move it.

- **Coffret's own area, when the on-disk spelling is not exactly `.coffret`.**
  Every name-side check says no, so the reservation is simply not in force:
  `local_scan/walk_mappings.rs` walks into it and reports what is under it as
  files to back up — this device's own marker among them; `fetch/select.rs` and
  `add/receive_file.rs` let a fetch or a browser drop place a file inside it;
  `local_scan/root_state.rs`'s `holds_nothing` counts it as content, so a root
  holding only the management area stops reading as empty under `EP-12`.

`EP-14` says nothing about how the name is compared, and nothing about a volume
that folds. That silence is what let the two halves drift apart.

### 2. Fold ASCII case, and refuse rather than skip

Compare with ASCII case folded. Where the spelling is **exactly** `.coffret`,
keep today's behaviour. Where it folds to `.coffret` but is not it, **refuse and
report** rather than silently stepping over it.

The asymmetry is the point, and the reason is `EP-14`'s cost clause:

> The cost is the one EP-11 states for its reserved prefix: anything of the
> user's own under a folder named `.coffret` is not backed up.

One name is reserved and the register says what that costs. `.coffret` has seven
letters, so folding ASCII case admits 2^7 = 128 spellings. Skipping all of them
would remove any of the 128 from backup without a word of it in the register —
the same stated cost, multiplied by 128, silently. A person who names a folder
`.COFFRET` for their own reasons is owed a sentence, not an omission they find
out about when they need the files back.

So: the exact name keeps the silent skip the register already prices, and every
other spelling that folds to it is refused and said out loud. Each of the seven
sites decides which of the two it is doing — the five that step over today
(`add/added_at.rs`, `add/added_locally.rs` twice, `local_scan/walk_mappings.rs`,
`local_scan/root_state.rs`) and the two that already refuse
(`add/receive_file.rs`, `fetch/select.rs`).

Name the placement-side test
`a_case_variant_of_the_reserved_name_is_refused_rather_than_skipped` and the
scan-side one `a_scan_reports_a_folder_that_folds_to_the_reserved_name`.

`EP-14` has to say this before the code can: how the name is compared, that a
spelling folding to it is refused rather than passed over, and what that leaves
of the cost clause.

### 3. Read the spelling where the refusal is composed

An fd carries no name, so the identification cannot ride the open. Put it on the
**refusal path only**: when registration is about to raise
`ManagementAreaIncomplete`, read the parent directory's entries and find the one
that folds to `.coffret`. If it is exactly `.coffret`, the interrupted-run
sentence is right and stands. If it is not, the sentence says which name is
standing there and that this volume does not tell the two apart — and never
tells them an interrupted registration left it.

A directory read on a path that is already failing costs nothing anybody
notices, and it is the only place where the spelling is still available.

Name the test
`a_folder_that_folds_to_the_reserved_name_is_not_called_an_interrupted_registration`.

## Out of scope

- **Identifying the management area by identity rather than by name** — a
  depth-1 `(dev, ino)` comparison against the handle `EP-13` already opens.
  It is the better long-run answer and it is deliberately not this change: it
  is a separate decision, and it would not remove the name question anyway,
  because `carries_management_area` judges an Entry Path for a place that may
  not exist yet.
- **Unicode case folding.** ASCII only. A volume that folds by Unicode rules has
  collisions ASCII folding does not catch, and saying which volumes coffret
  claims to serve is a judgement about the product rather than a defect in this
  code.
- **The scratch prefix `.coffret-fetch-` has the same shape of defect** —
  `scratch::is_scratch` is `name.starts_with(PREFIX)`, compared exactly, and it
  is asked beside `is_management_area` at three of the sites above. It belongs
  with its own reservation (`EP-11`) rather than here: it is a prefix and not a
  name, so "refuse rather than skip" does not transfer without first deciding
  what a prefix collision costs, and the consequence differs — what a folding
  volume hides there is coffret's own half-written scratch rather than a
  person's folder.
- **The other findings recorded beside this one** — `/api/file` answering `500`
  for a deleted file, `scatter.rs`'s de-duplication collapsing a mapping, and
  `Staging::discard` recording a warning for a directory that was already gone.
  None of them is about the reserved name.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A local name that folds to `.coffret` under ASCII case folding but is not
      exactly `.coffret` is refused and reported wherever the reservation is
      asked, rather than stepped over. Asserted by
      `a_case_variant_of_the_reserved_name_is_refused_rather_than_skipped` on
      the placement side and
      `a_scan_reports_a_folder_that_folds_to_the_reserved_name` on the scan
      side.
- [x] A name that is exactly `.coffret` behaves as it does today at every one of
      the seven sites: no test that passed before is deleted rather than
      renamed, and nothing that was skipped becomes a refusal.
- [x] Registration that meets a folder folding to the reserved name without a
      marker in it does not say an interrupted registration left it, and says
      which name is standing there. Asserted by
      `a_folder_that_folds_to_the_reserved_name_is_not_called_an_interrupted_registration`.
- [x] The spelling is read only where a refusal is being composed — no extra
      directory read on a path that succeeds.
- [x] `EP-14` states how the reserved name is compared, that a spelling folding
      to it is refused rather than passed over, and what remains of its cost
      clause once only the exact name is priced.
- [x] A refusal about a folder a person owns names that folder to the person and
      keeps it out of the diagnostic event (spec: EL-1).

### Manual / on-hardware (verified by a human before merge)

- [ ] On a case-folding volume (macOS APFS as shipped, or an exFAT volume),
      create a folder named `.COFFRET` in a folder to be mapped, run
      `coffret map` against it, and confirm the refusal names that folder rather
      than calling it an interrupted registration. The automated tests drive the
      fold in the comparison, not a real folding filesystem, so this is the one
      check that the kernel behaves as assumed.
