---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && cd backend && RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --workspace && cd .. && grep -q 'a fetch over a degraded Keyring repairs nothing' backend/crates/domain/coffret-usecase/src/fetch_conformance/mod.rs && ! grep -q 'fresh subfolder' backend/crates/gateway/google-drive-store/tests/support/mod.rs && ! grep -q 'Only `fetch_entry` raises it' backend/crates/domain/coffret-usecase/src/fetch/fetch_error.rs"
assignee: null
branch: task/0921-1945-bring-stale-comments-up-to-what-the-code-does
created_at: 2026-09-21T10:45:21Z
updated_at: 2026-09-21T11:32:34Z
---

# docs: bring stale comments in code and scripts up to what the code does

## Overview

Comments only: rustdoc, `//` and `//!` comments, TypeScript and shell
comments. No executable line, no string a program prints or a person reads
at run time, no test assertion, no identifier. Each item below was seen in
the tree recently; re-find it by its text, read the code it describes, and
make the comment true with the smallest change. If an item is already
fixed, or cannot be made true without touching code, leave it and say so.
Write in the voice the surrounding comments have.

1. `backend/crates/domain/coffret-usecase/src/fetch/fetch_error.rs`,
   `EntryNotCurrent`: the doc says "Only `fetch_entry` raises it" and
   "Raised after the catch-up". There are two raisers now — the local-path
   lookup shares the target resolution and does not catch up first. Say
   who raises it and when, for both.
2. `backend/crates/apps/coffret-device/src/join_library/`: the doc cites
   `(spec: CK-9, RV-1)` where RV-1 is a weak fit — check the register and
   cite the rule that actually covers what the sentence says; and it says
   a join "catches it up **from** the Library's head" where the
   vocabulary's collocation is catching an Index up **to** a head.
3. `backend/crates/gateway/google-drive-store/tests/support/mod.rs`: the
   module header still says "fresh subfolder", the word from before the
   app folder had a name. Use the current vocabulary (FM-18).
4. `backend/crates/domain/coffret-usecase/src/error.rs`,
   `Error::LengthMismatch`: the variant doc gives only the first meaning
   (a stream shorter than the length it declared). `ByteStream::collect_exact`
   also raises it for a body that is not the length the caller asked for
   as a range. Say both.
5. `scripts/e2e-it.sh`: the comment beside the sync that runs while the
   server holds the catalog says "Both of them are writing to one SQLite
   file", which is not strictly true of that one sync (it has nothing new
   and commits nothing). Say what is true: two processes have the file
   open, and what the step shows.
6. `scripts/e2e-it.sh`: the comment explaining the port overrides gives as
   its example "`s3-store-it`, which keeps its own container on 19000" —
   an example that cannot collide with the ports this target binds. Give
   an example that can, or drop the example.
7. `frontend/packages/apps/e2e/journeys/server.ts`: `STARTUP_TIMEOUT_MS`'s
   doc still says the wait is for the server to open the Library and
   answer; it now also covers the catch-up the server does at startup.
   And "the outage journey" is singular in several places where more than
   one journey now kills the server.
8. `backend/crates/apps/coffret-server/src/api_error/mod.rs`, the
   `too_large` doc: "a later drop that lands something arms a sync"
   describes one of the two flows; after a freeze, the freeze picks the
   file up. Say both, briefly.
9. `frontend/packages/apps/web/src/` (`StatusBar.tsx`, `fill.ts`,
   `FileList.tsx` — wherever the comment is): comments that say files are
   "going up" while a fill is writing them into the mapped folder; the
   upload is the later sync. Fix the **comments** only — leave any string
   the UI shows.
10. `backend/crates/domain/coffret-usecase/src/fetch_conformance/mod.rs`:
    the overview introduces the counting store as one that "counts what a
    run reads", but a case now relies on it counting writes — a fetch over
    a degraded Keyring repairs nothing, which only the absence of a write
    shows (spec: KL-13, RV-2). Bring the overview up to date; the sentence
    "a fetch over a degraded Keyring repairs nothing" must appear on one
    line.
11. `backend/crates/apps/coffret-server/tests/routes.rs`, the `QUIET`
    doc: a sentence built from DK-9's subject and DK-4's predicate. Not
    false, but it reads as one rule; separate the two.
12. The TypeScript `failFault` helper's comment on `malformed` lists the
    malformed Recovery Code shapes and leaves out "nothing before the
    separator", which a test fixes (`'1qqqqqqq'`). Add it.
13. `backend/crates/domain/coffret-format/src/container_reader/testing/mod.rs`:
    the module doc describes the helpers' reach without the refusal
    cases they are now also used for. Widen the sentence.
14. `Listing.mapped` in the gateway and `ListingDto.mapped` in the
    server: the docs predate the root listing's suppressed banner. Read
    how the field is used now and make both docs say it.
15. `scripts/s3-store-it.sh`: the header comment lists what the target
    exercises and does not count the CLI round trip it now runs.
16. `ContainerSummary::object_ref`'s rustdoc says a device that replayed
    the Journal holds `None`; FM-15 has the record carry the cached
    reference. Check the code and the rule, and correct the doc.
17. Test comments in `coffret-device` that say "Library name" for the
    device-local name a person gives a Library: the concept documents use
    **device-local Library name** for that, and "Library name" elsewhere
    means a file's name inside the Library. Use the concept's term in the
    comments, in step with `testing/mod.rs`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes and the workspace's rustdoc builds with warnings denied
- [x] `fetch_conformance/mod.rs` says that a fetch over a degraded Keyring repairs nothing; the Drive test support header no longer says "fresh subfolder"; `EntryNotCurrent`'s doc no longer says only `fetch_entry` raises it

### Manual / on-hardware (verified by a human before merge)

- [ ] Each of the seventeen items is either done as described or reported as left alone with the reason, and the change touches comments only

## Out of scope

- Any executable line, identifier, test assertion, CLI help text or UI string
- The word `viewer` in comments, which waits on a decision between `explorer` and `reader`
- Expanding `CK-1 to CK-3` range citations, and the `(spec: …)` citation convention
