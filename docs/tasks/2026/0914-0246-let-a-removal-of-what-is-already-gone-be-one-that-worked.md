---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [completeness, error-type-design, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -rq "a_discard_of_a_staging_directory_that_is_already_gone_records_nothing" backend/crates/ && grep -rq "a_removal_under_begin_still_runs_where_the_directory_is_already_gone" backend/crates/'
assignee: null
branch: task/0914-0246-let-a-removal-of-what-is-already-gone-be-one-that-worked
created_at: 2026-09-14T02:46:11Z
updated_at: 2026-09-14T03:28:10Z
---

# fix(backend): let a removal of what is already gone be one that worked

## Overview

`OC-8` says what every removal this device makes for its own purposes is:

> What is already gone is a successful removal, an interrupted clean-up is
> simply run again, and no removal has to check what is there first, because
> absence is the outcome being sought.

Two removals in `backend/crates/apps/coffret-device/src/staging.rs` do not do
that, and they are the only local removals in the tree that do not. Every other
one inherits the tolerance from the gateway it goes through:
`coffret-local-fs`'s `unix_fs.rs` swallows `ErrorKind::NotFound` for a spool
with the reason written out beside it, `unix_destinations/unix_destination.rs`
swallows `Errno::NOENT` for a scratch, and
`spool_conformance/removal.rs` and `destinations_conformance/removal.rs` hold
each of them to it. `Staging` calls `std::fs` directly, so it inherits nothing.

### 1. `discard` records a successful removal as a cleanup failure

`discard` sends every `Err` from `fs::remove_dir_all` to
`record_cleanup_failure`, which warns `could not remove what an interrupted
attempt left`. `remove_dir_all` reports an absent directory as `NotFound`, so
the one case `OC-8` calls a successful removal is a case that produces that
warning.

Nothing downstream breaks — `discard` returns `()` — but the log then says a
cleanup failed where nothing failed, and reading a log correctly is the whole
reason the event exists.

### 2. `begin` fails the whole attempt when the directory goes away underneath it

```
if staging.path().exists() {
    …
    fs::remove_dir_all(staging.path())
        .map_err(Error::local(LocalOperation::Removing, staging.path()))?;
```

The `exists()` and the `remove_dir_all` are not one operation. Anything that
removes the directory in between — another run, a person clearing space — turns
the `NotFound` into `Error::local(LocalOperation::Removing, …)` and fails
`create_library` or `join_library` outright, in exactly the case `OC-8` calls a
successful removal.

The comment directly above it already says the opposite, in the rule's own
words: "which are idempotent by rule, so an attempt interrupted at any point
costs the next one nothing beyond this removal (spec: OC-8)".

### 3. The sentence that records the gap comes out

`discard`'s doc names the gap today, because the code had it:

> Not checking is as far as the rule is carried here — `remove_dir_all` reports
> a directory already gone as `NotFound`, and the branch below records that as a
> cleanup refusal rather than as the successful removal OC-8 says it is.

Once the code carries the rule, that sentence is false. Remove it. The rest of
the paragraph and its `(spec: OC-8)` stay — they say why the removal is
attempted without checking at all.

## What this change has to decide

Where the tolerance lives is yours to judge, and the choice is real:

- **At the call**, the way `unix_fs.rs` does it — match `NotFound`, say in a
  `debug!` that it was already gone, and cite the rule. Smallest, and it repeats
  a decision two other places have already made.
- **Through a capability**, the way a spool and a scratch get theirs, so
  `Staging` inherits the tolerance instead of restating it. This is the shape
  `OC-8`'s existing conformance tests assume, and it is the larger change.

Either satisfies the criteria below. What must not survive: a removal in this
file that can fail because what it was removing is not there.

`begin`'s `exists()` should end up deciding only whether there is anything to
say, not whether the removal is allowed to fail.

## Out of scope

- **`publish`'s `fs::rename`.** Not a removal, and `OC-8` governs removals.
- **The other `LocalOperation::Removing` sites.** They already have the
  tolerance, and their conformance tests already hold them to it.
- **Widening `OC-8`'s own enumeration.** Three other things this device discards
  idempotently — the half-written neighbour `replace_marker.rs` drops, the
  half-written marker `write_marker.rs` drops, and the temporary neighbour
  `owner_only.rs` drops — are not in the rule's list, so they cannot be cited
  without changing the register. That is a register change and belongs with the
  other ones.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A `discard` of a staging directory that is already gone records nothing,
      asserted by
      `a_discard_of_a_staging_directory_that_is_already_gone_records_nothing`.
- [x] A staging directory removed between `begin`'s look and its removal does
      not fail the attempt, asserted by
      `a_removal_under_begin_still_runs_where_the_directory_is_already_gone`.
- [x] Every other failure these two can meet is reported exactly as it is today:
      a refusal that is not absence still warns from `discard`, and still fails
      `begin`.
- [x] `discard`'s doc no longer says the rule is carried only as far as not
      checking, and still cites `OC-8`.
- [x] Wherever absence is swallowed, the reason is written beside it, as
      `unix_fs.rs` writes its own.

### Manual / on-hardware (verified by a human before merge)

- [ ] Nothing here needs hardware: both paths are driven by a temporary
      directory the test creates and removes.
