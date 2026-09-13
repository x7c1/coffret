---
status: completed
pipeline_phase: null
follow_up_of: docs/tasks/2026/0912-1706-cite-oc-8-for-the-fetch-destinations-removals.md
base_ref: feat/mapped-root-marker
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "(spec: OC-8, EP-11)" backend/crates/domain/coffret-usecase/src/fetch/placement.rs && grep -q "(spec: EP-11, OC-8)" docs/concepts/library/README.md'
assignee: null
branch: task/0913-0306-follow-up-cite-oc-8-for-the-fetch-destinations-removals
created_at: 2026-09-13T03:06:34Z
updated_at: 2026-09-13T03:47:09Z
---

# docs: say which rule makes a discarded scratch's removal idempotent

## Overview

`OC-8` says removing what this device wrote for its own purposes is
idempotent, and its enumeration names **"a scratch a local writer never
published (EP-11)"** among those things. Two places state that rule's content
without citing it.

### `backend/crates/domain/coffret-usecase/src/fetch/placement.rs`, lines 303-306

`Placement::discard`'s doc comment says the rule almost word for word:

```
/// Removes the scratch, this placement having come to nothing.
///
/// One that is already gone is the same outcome as one this call removed, so
/// a cleanup that races the failure it is cleaning up after still succeeds.
```

That second sentence is the same claim the capability it calls now closes with
`(spec: OC-8, EP-11)` — `destination.rs:45-48` reads "One that is already gone
is the outcome this wanted, so a cleanup racing the failure it is cleaning up
after still succeeds". The caller states the rule and the callee cites it.

Close the second sentence with `(spec: OC-8, EP-11)`. The citation will not fit
on line 306, so put it on a continuation line, the way `destination.rs` does:

```
/// One that is already gone is the same outcome as one this call removed, so
/// a cleanup that races the failure it is cleaning up after still succeeds
/// (spec: OC-8, EP-11).
```

`EP-11` belongs beside `OC-8` here, as it does at the four placement-path
sites: the thing removed is a scratch, and `EP-11` is what makes it one. Note
that this file carries **no `OC-` citation at all** today, so no change that
corrects a rule ID can reach it — there is no wrong ID here, only a missing
right one.

### `docs/concepts/library/README.md`, lines 148-152

The `scratch` Domain Rule says what a scratch is and who writes it, and cites
`EP-11` alone:

> A local writer writes its **scratch** — the file it fills before the rename
> that publishes it — inside a mapped folder, which is also a folder a scan
> walks, so coffret reserves a local filename prefix for those files and a
> scan passes over every local name carrying it. A fetch is one such writer,
> and so is an upload the browser drops into a mapped folder (spec: EP-11).

It never says what becomes of a scratch whose rename never came — although
`OC-8` names exactly that case, and the code that removes one cites `OC-8` in
five places. The document's only `OC-8` statement is in the sync bullet above
this one, scoped to what a settle reclaims, which is the spool side.

Extend this bullet's citation to `(spec: EP-11, OC-8)` and add a sentence, in
the register's own terms, saying that removing a scratch whose rename never
came is idempotent because absence is the outcome being sought. Keep it to the
document's voice: a Domain Rule states the promise, not the mechanism.

**No behaviour changes.** One doc comment and one Domain Rule; no code, no
test, and no value the build or the runtime reads.

## Out of scope

- **Any `OC-6` → `OC-8` correction.** Neither site cites `OC-6`; one cites
  nothing and the other cites `EP-11`. A separately queued change owns the
  corrections.
- **The `temporary` / `scratch` vocabulary.** Both sites already say
  `scratch`.
- **The other places that describe removing a device's own leftovers without
  citing a rule** — `coffret-device/src/add/incoming_file.rs`'s drop guard, and
  the staging-directory removals in `coffret-device/src/staging.rs`, which
  `create_library` and `join_library` both call rather than doing themselves.
  That crate cites `OC-2` and `OC-7` and no `OC-6`/`OC-8` at all, so its
  citation convention is a separate question from this one.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes.
- [x] the placement's discard cites the rule its own sentence states:
  `grep -q "(spec: OC-8, EP-11)" backend/crates/domain/coffret-usecase/src/fetch/placement.rs`
- [x] the Library concept's scratch rule carries the cleanup rule as well as the
  one that defines a scratch:
  `grep -q "(spec: EP-11, OC-8)" docs/concepts/library/README.md`
