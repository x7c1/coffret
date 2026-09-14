---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [completeness, concept-alignment, error-type-design, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -rq "two_mappings_into_one_folder_are_each_refused_by_name" backend/crates/ && grep -rq "a_placed_file_that_is_gone_is_declined_rather_than_fetched_again" backend/crates/'
assignee: null
branch: task/0914-0626-name-every-mapping-a-refusal-is-about
created_at: 2026-09-14T06:26:10Z
updated_at: 2026-09-14T07:41:26Z
---

# fix(backend): name every mapping a refusal is about, and keep a lost file fetchable

## Overview

Two findings recorded beside each other, both about what this device says of a
mapped root. One is a defect in what a refusal reports; the other is behaviour
that is already right and that nothing holds in place.

### 1. Two mappings into one folder are reported as one

`EP-13` says of a root that will not vouch for itself:

> A refusal names the mapping and the reason, on the no-silent-selection posture
> EP-4 sets, and propagates the way a declined placement does (EP-11)

and `RefusedRoot` carries a `prefix` for exactly that, so its sentence can say
which of the device's mappings it is about:

> `<local root>` is not the folder `<mapping>` was recorded against: `<reason>`;
> nothing was placed into it, and recording that mapping again is what settles
> which folder it is

Both places that collect these refusals de-duplicate them by `local_root`:

```
backend/crates/domain/coffret-usecase/src/fetch/scatter.rs:72-77
    if !refused
        .iter()
        .any(|held| held.local_root == root.local_root)

backend/crates/domain/coffret-usecase/src/fetch/run.rs:172-178
    if !held.iter().any(|seen| seen.local_root == root.local_root)
```

`note_refusals` states the reason for de-duplicating, and the reason is sound
for what it was written about:

> Containers are fetched one after another and several of them may hold Entries
> under the same refused root, so the run keeps the *first* refusal for each
> root: the reason is a fact about the folder rather than about the Container
> that happened to meet it, and reporting one mapping several times would say
> nothing the first one did not.

**"One mapping several times" is not what this key collapses.** `EP-9` bounds
the *key* a mapping is recorded under — at most one Library-root mapping and at
most one per top-level component — and says nothing about the folder. Two
top-level components may name the same folder, and that is an ordinary thing to
do. When they do, one refusal is reported and one mapping is named; a person
follows the sentence, records that mapping again, and meets the other one, which
was never mentioned.

That the state is real elsewhere in this code is already written down:
`sync/scan/deletions.rs`'s doc says "one path can be reported by two mappings,
so the findings are deduplicated by path" — the same folder reached through two
mappings, handled there by choosing a key that matches what is being said.

The key here should be the thing `EP-13` asks the refusal to name. Which field
or fields express that is the implementation's judgement; what the task fixes is
that a device with two mappings into one refused folder reports both.

Name the test `two_mappings_into_one_folder_are_each_refused_by_name`.

### 2. A placed file somebody deleted answers 500

`GET /api/file` asks three questions in order, and the first one has a branch
for the row that outlived its file
(`backend/crates/apps/coffret-server/src/routes/file.rs:58-77`):

```
if library.state_of(&path).await? == EntryState::Present {
    match library.open_local_file(&path).await {
        Ok(Some(file)) => return Ok(served(&path, file, "present")),
        // The row says this device placed the file and the file is not
        // there now. That is a finding rather than a failure, and the fetch
        // below is what states it (spec: EP-10, EP-11).
        Ok(None) => {}
```

The comment is right about what should happen and the code does not reach it.
`EntryFetches::fetch` takes its shortcut from the catalog row alone:

```
if library.state_of(&path).await? == EntryState::Present {
    … EntryFetch::AlreadyPresent
```

so the fetch answers "already present" about a file that is not there, the
route re-opens it at
`backend/crates/apps/coffret-server/src/routes/file.rs:100`, gets `None` again,
and raises `ApiError::unreadable` — which is `ApiError::server`, a `500` whose
body deliberately says nothing about what happened. A person who deleted one of
their own files meets a server error about it.

**The finding is not that the file should be fetched again.** `fetch/select.rs`
already decides this case, and decides it the other way:

> This device placed the file and it is no longer what it wrote down — changed,
> or gone. Either way a pending local change the sync flow owns, and re-fetching
> would quietly undo it.

It surfaces `LocallyChanged`, for which `finding_reason.rs` already has the
sentence *"what this device wrote there has since changed or gone"*, and which
the upload route already renders as a decline. `EP-10` admits the file being
absent or the row and the disk agreeing, and putting a file back is an explicit
operation rather than one inferred from a row. So the answer owed here is that
decline, not the bytes.

What the shortcut is missing is that a row is not the whole of the question:
the pair it should read is the row **and** the file, which is the same pair a
placement vouches for. Where they disagree the flow should run and state the
finding.

Name the test `a_placed_file_that_is_gone_is_declined_rather_than_fetched_again`.

## Out of scope

- **This task was drafted saying the opposite of §2** — that the fall-through
  already worked and only wanted a test. It does not: the shortcut above it
  returns before the branch is reached. The claim was written from reading the
  route and not the call it makes, and it is recorded here because the body a
  reviewer reads should not quietly become the corrected one.

- **Whether two mappings into one folder should be allowed at all.** `EP-9`
  permits it and this change does not revisit that; it makes the refusal say
  what `EP-13` asks of it under the rules as they stand.
- **The de-duplication `note_refusals` was written for** — the same mapping met
  through several Containers — stays. Reporting one mapping once per Container
  is what its doc rules out, and it is right to rule out.
- **`Staging::discard`'s idempotence and the reserved-name folding** were
  recorded beside these two and are both already merged; nothing to do.
- **The failure a vouch meets that the operating system will not answer**
  (`unix_destinations/vouch.rs` handing back an I/O refusal that reaches a
  caller as one part's business) is a separate finding with its own reach
  question, recorded elsewhere.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A device with two mappings whose local roots are the same refused folder
      reports a refusal for each mapping, each naming its own mapping, on both
      the Container path and the run path. Asserted by
      `two_mappings_into_one_folder_are_each_refused_by_name`.
- [x] The same mapping met through several Containers is still reported once:
      no test that passed before is deleted rather than renamed, and a run that
      met one refused mapping in three Containers still lists it once.
- [x] `GET /api/file` for an Entry whose row says `Present` and whose local file
      has been deleted declines with the `LocallyChanged` finding rather than
      answering `500`, and does not put the file back. Asserted by
      `a_placed_file_that_is_gone_is_declined_rather_than_fetched_again`.
- [x] Every `(spec: …)` citation near an edited site still names a rule that
      says what the citing comment says it says.

### Manual / on-hardware (verified by a human before merge)

- [ ] Nothing here needs hardware. Both cases are driven against the gateway's
      own fakes and the server test harness.
