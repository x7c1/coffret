---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, error-type-design, concept-alignment]
max_refine_rounds: 2
retries_remaining: 1
check_command: 'make check && ! git grep -q "fn first_head" -- backend/crates'
assignee: null
branch: task/0929-0602-ask-whether-any-head-survives-when-a-join-looks-for-a-library
created_at: 2026-09-29T06:02:10Z
updated_at: 2026-09-29T06:40:32Z
---

# fix(backend): ask whether any head or Snapshot survives when a join looks for a Library

## Overview

`coffret join` decides whether the prefix (S3) or folder (Drive) it was pointed
at holds a Library by asking about exactly one object: the generation-0 Journal
record, `head-0.cfrt` (`first_head()` in
`backend/crates/apps/coffret-device/src/join_library/run.rs`, around line 225–246;
the single-object probes are `check_library_object` in
`backend/crates/apps/coffret-device/src/s3.rs`, `s3_store::check_object` in
`backend/crates/gateway/s3-store/src/check_object.rs`, and
`google_drive_store::check_object` in
`backend/crates/gateway/google-drive-store/src/check_object.rs`).

That answer is correct only while nothing is ever deleted. CK-4 makes Journal
records at or before a Snapshot's last applied generation eligible for pruning,
and CK-6's `prune` deletes exactly those — generation 0 among them, from the
first checkpoint a Library prunes past. A pruned Library still holds its
Journal and every Entry it committed but no longer holds `head-0.cfrt`, and a
join of it would be told that Storage holds nothing of the Library. `prune` is
not implemented yet, so the fault is latent; the doc comment on `first_head()`
already says whoever implements `prune` has to change this. This task makes that
change now, so that `prune` does not have to remember it.

Change the question from "does `head-0` exist" to "does **any** head record
or Index Snapshot exist under this Library's place". Heads alone are not
enough: by CK-4 the Journal record a Snapshot last applied is itself eligible,
so once a Snapshot covers the current generation, `prune` may leave no head at
all — and what such a Library still holds is that Snapshot (CK-2 makes it the
source of the next slot). A Library that has committed anything therefore
always holds at least one head or one Snapshot, whatever has been pruned:

- Add a prefix-listing probe to both gateways that answers `true` as soon as one
  object whose name is a head record or an Index Snapshot exists, and `false`
  only when the listing finishes with neither. S3: `ListObjectsV2` with the Library's head prefix (through
  the crate's `KeyLayout`, so the key asked about is the key the store writes)
  and `max-keys` 1. Drive: a files query in the Library folder matching head
  names, reusing the existing listing machinery and its bounded-page behaviour
  from `check_object`. How a head's name is spelled belongs to the format
  (`ControlObjectName::head`); derive the prefix from there rather than spelling
  `head-` a second time.
- Keep the existing error discipline: anything that is not an answer about the
  place (refused grant, unreachable endpoint, missing bucket, a listing that
  never finishes) stays an error, never a `false`. Keep the provider-text
  redaction the current probes apply (spec: EL-5).
- Replace the join's use of the single-object probe with the new one, and delete
  `first_head()` and any single-object probe left without a caller. Rewrite the
  doc comments that explain the old choice so they explain the new one
  (including the `prune` remark). If a concept document or spec rule describes
  how a join recognises a Library, bring it in line.

Out of scope: implementing `prune`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The S3 probe answers `true` for a prefix holding only `head-1.cfrt` (no
      `head-0.cfrt`), `true` for one holding `head-0.cfrt`, and `false` for an
      empty prefix, in s3-store tests against the crate's loopback stub.
- [x] The Drive probe answers the same three cases (only a later head, the first
      head, none) in google-drive-store tests over the stub transport, and a
      listing that keeps returning empty pages with a continuation token is an
      error rather than `false`.
- [x] A refused / failing listing on either provider surfaces as an error, not
      as "no Library here", in a test per provider.
- [x] Both probes answer `true` for a place holding an Index Snapshot and no
      head at all (a Library pruned past its last head).
- [x] A join-level test (coffret-device, over the stub Drive transport or the
      S3 stub) finds a Library whose Storage holds a later head but not
      generation 0, and one whose Storage holds a Snapshot and no head.
- [x] `first_head` no longer exists in `backend/crates` (gate appended to
      `check_command`).

### Manual / on-hardware (verified by a human before merge)

- [ ] CI's `s3-store` and `e2e` jobs (MinIO) are green on the PR.
- [ ] `make drive-round-trip-it` is green against real Drive — its second-device
      `join` goes through the new probe.
