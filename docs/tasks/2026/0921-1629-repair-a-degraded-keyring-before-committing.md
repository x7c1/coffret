---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, concept-alignment, error-type-design, clarity, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check"
assignee: null
branch: task/0921-1629-repair-a-degraded-keyring-before-committing
created_at: 2026-09-21T07:29:19Z
updated_at: 2026-09-21T09:49:22Z
---

# feat(backend): repair a degraded Keyring before committing, and refuse the commit when the repair fails

## Overview

The Keyring lifecycle register already says what a device owes a committed
replica set that has lost replicas (`docs/spec/keyring-lifecycle/`): the
set must be repaired before another write (KL-11), the repair is automatic
and re-materializes only the committed generation (KL-13), it is an
unconditional write confirmed by reading the replica back (KL-14), loss and
repair are surfaced and never silent (KL-15), and a repair that cannot
complete leaves the gate closed for writes while reads go on (KL-16).

None of that is implemented. `commit::keyring::read_committed` stops at the
first valid replica, logs `the committed Keyring is degraded and awaits
repair` when it had to step over one, and the commit then carries on and
writes the next generation. Replicas above the one that answered are never
looked at, so a set missing its last replica is not even noticed.

Implement the repair, in the commit flow:

1. **Where.** In `commit_batch`, on every attempt, after the catch-up and
   before `keyring::replicate`: when there is a committed Keyring, examine
   it and repair it. This is the "before another write" of KL-11. A Library
   with no committed head has nothing to repair. Fetch flows
   (`fetch`, `fetch_entry`) and the read in `freeze` stay reads: they keep
   stepping over an invalid replica and do not repair (their module docs
   already say so — keep those statements true, and adjust wording only
   where it now misdescribes who performs the repair).
2. **Detection is a full walk.** Examine every position the commitment
   declares, not only up to the first valid one: a position is in need of
   repair when the listing does not hold the name, or the object does not
   read back valid (`read_replica`, KL-1). A replica Storage merely failed
   to hand over (`InvalidReplica::Unfetchable`) is *not* known to be lost:
   do not rewrite it on that evidence — but the set cannot be called
   complete either, so treat it as a repair that could not complete (the
   gate of item 4). The cost — reading every replica on each commit — is
   accepted: a present-but-unreadable replica cannot be found any other way.
3. **Repair.** Take the mapping from any committed valid replica (KL-6,
   KL-13), encode it exactly as `replicate` does for that generation, epoch
   and `set_digest`, `put` each position in need of repair (the port's
   `put` replaces what is stored under the name, which is what makes a
   present-but-invalid replica repairable), then read each one back and
   require it valid (KL-14). Share the write-and-read-back code with
   `replicate` rather than copying it. Repair never deletes anything and
   never writes a position that read back valid. If no replica is valid,
   that is Keyring loss (KL-5, RV-7), not a repair: keep today's
   `KeyringUnreadable` refusal.
4. **The gate.** When a needed repair does not complete — a `put` fails, a
   read-back is invalid, or a position was unfetchable — the commit is
   refused with a `CommitError` variant of its own that carries the
   generation, the positions still in need and the cause, as structured
   values in the style of `IncompleteKeyring` / `KeyringUnreadable`. Nothing
   of the batch is committed. The next run examines and tries again; there
   is no partial relaxation (KL-16).
5. **Surfacing (KL-15).** `CommitOutcome` carries every repair the run
   performed: which positions of which generation were rewritten, one
   entry per attempt that put a position back (an attempt that repairs
   and then loses the commit slot did that work, and the rebase cannot
   find it again), and none when nothing needed rewriting. A refusal
   (item 4) carries the positions the refusing examination did put back,
   because no outcome exists to report them on. Carry that through whatever `sync` and `freeze`
   return to their callers, and have the CLI's `sync` and `freeze` print
   one plain line when a repair happened — in the vocabulary the concept
   docs use (replica, Keyring, repaired), saying how many replicas were
   missing or unreadable and that they were rewritten — and render the
   refusal of item 4 as a sentence a person can act on (what is degraded,
   what was put back, that nothing was committed, that reads and restores
   go on, that running again retries). Follow the
   event-logging register (EL-1…EL-5) for the diagnostic events: counts,
   generation and positions are fine; no names. The server's routes must
   keep compiling and must not swallow the new refusal into a generic
   error if the existing mapping has a natural place for it; a dedicated
   explorer notice is out of scope.
6. **The old warning.** `read_committed`'s "awaits repair" warning stays for
   the read-only callers, reworded if needed so that it does not promise a
   repair this read performs. The commit path should not log both that
   warning and the repair for the same finding.
7. **Docs.** The Keyring concept (`docs/concepts/keyring/`) already states
   the obligation; add what is missing so a reader can find *who* repairs
   and *when* (the committing device, before its commit), and register any
   new verb the code introduces. Do not change the register's rules; if an
   implementation decision genuinely needs a rule change, stop and report
   `needs_review` instead.

Tests, against the in-memory store and the existing fault-injection
helpers (`commit_conformance/faulty_store.rs` and friends), each citing the
rule it holds:

- a set missing one replica (first, middle, last position) is complete
  again after the next commit, the rewritten replica reads back valid, the
  valid ones were not rewritten, and the outcome names the positions
- a present-but-unreadable replica (corrupt bytes; wrong digest) is
  replaced and valid afterwards
- a failing `put` during repair refuses the commit with the new variant,
  commits nothing (no new head, Index unchanged), and a later run with the
  fault gone repairs and commits
- an unfetchable replica refuses the commit without rewriting that position
- no valid replica stays `KeyringUnreadable`
- a complete set costs no writes and reports no repair
- a repair that stops reports the positions it did put back, and a repair
  made by an attempt that then lost the commit slot is still reported
- fetch over a degraded set still succeeds and writes nothing (RV-2, KL-16)
- two devices repairing the same position both succeed (KL-14)

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `commit_batch` examines every declared replica of the committed Keyring before writing the next generation, rewrites exactly the positions that are absent or read back invalid from a committed valid replica, and confirms each by read-back (KL-11, KL-13, KL-14)
- [x] A repair that cannot complete refuses the commit with its own structured `CommitError` variant and commits nothing; reads are unaffected (KL-16)
- [x] `CommitOutcome` reports the repair, `sync` and `freeze` carry it to the CLI, and the CLI prints a line for a repair and an actionable sentence for the refusal (KL-15)
- [x] The tests listed in the Overview exist and pass, and `make check` passes

### Manual / on-hardware (verified by a human before merge)

- [ ] `make drive-round-trip-it` against real Drive still holds (the full walk adds reads to every commit)

## Out of scope

- Carrying an earlier attempt's repair on a run that ends in an error
  (`ConflictLimitReached`, or a later attempt's refusal)
- Telling a person who only fetches that the Keyring is degraded
- The CLI printing a commit refusal twice through the transparent
  `SyncError::Commit` / `FreezeError::Commit` wrappers, which every
  `CommitError` shares
- A dedicated notice in the explorer UI for a repaired or degraded Keyring
- Repair on `prune` and Master Key rotation, which do not exist yet
- Orphan cleanup of candidate replica sets (KL-12)
- Any change to the register's rules
