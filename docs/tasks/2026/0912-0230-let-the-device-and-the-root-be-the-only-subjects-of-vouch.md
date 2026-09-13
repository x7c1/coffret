---
status: completed
pipeline_phase: null
follow_up_of: null
base_ref: feat/mapped-root-marker
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rq "the walk could not vouch" backend/crates && ! grep -rq "run could not vouch" backend/crates && ! grep -rq "the capability would not vouch" backend/crates && grep -q "whose roots the device could not vouch for" backend/crates/domain/coffret-usecase/src/local_scan/walked.rs && grep -q "whose roots the device could not vouch for" backend/crates/apps/coffret-device/src/findings.rs && grep -q "will not vouch for itself" backend/crates/domain/coffret-usecase/src/fetch/fetch_error.rs && grep -q "will not vouch for itself" backend/crates/apps/coffret-device/src/error.rs && ! grep -q "a mapped root it could not vouch for" backend/crates/apps/coffret-server/src/routes/activity.rs && ! grep -q "the mapped roots it could not" backend/crates/apps/coffret-device/src/lib.rs && ! grep -q "a path it could not vouch for" backend/crates/apps/coffret-device/src/run_fetch.rs && ! grep -q "a path it cannot vouch for" backend/crates/apps/coffret-device/src/finding_reason.rs && ! grep -q "the local state is one it" backend/crates/domain/coffret-usecase/src/fetch/mod.rs'
assignee: null
branch: task/0912-0230-let-the-device-and-the-root-be-the-only-subjects-of-vouch
created_at: 2026-09-12T02:30:00Z
updated_at: 2026-09-12T03:53:36Z
---

# docs(backend): let the device and the root be the only subjects of vouch

## Overview

Two rules use the verb *vouch*, and this repository writes each with its own
subject. EP-12 is the **device** vouching for a mapped root — whether the root
is there to be read from, which a scan settles by stamping what it found
(`docs/concepts/library/README.md`: *"A mapped root this device cannot vouch
for … is an **unavailable root**"*). EP-13 is the **root** vouching for
itself — whether the folder standing there is the one whose marker the mapping
recorded. Keeping the two subjects apart is what lets a reader tell which
question a sentence is about.

Eleven doc comments take a third subject. Five name it outright — *the walk*,
*a run*, *the run*, and *the capability* twice — and six more reach it by a
pronoun bound to the flow that happened to be running. Each is the mechanism that happened to ask the question
rather than the thing the rule is about, and each sits next to prose that uses
the rule's own subject, so the reader meets two shapes for one rule within a
few lines. This is documentation only: no behaviour, no signature, and no test
changes.

EP-12 and EP-13 are not the only rules that use the verb: EP-11 asks whether
the device can vouch for the local state at a path, and `fetch/mod.rs:85`
writes that one with the device too. So the repair is the same everywhere —
the device, or the root itself, and nothing else.

Sites 1 to 5 name the wrong subject and are stated with their exact
replacements. Sites 6 to 11 reach it through a pronoun, so each is stated as
the sentence it sits in and the subject it must take; write text that fits the
file's width rather than a line this file dictates.

### 1. `backend/crates/domain/coffret-usecase/src/local_scan/walked.rs` (line 45)

```
/// The mappings the walk could not vouch for, in mapping order (spec: EP-12).
```

The walk is how the scan asks; EP-12 is about the device. The two sibling
fields this one is the source for already say it the other way —
`sync/survey.rs:26` and `freeze/survey.rs:18` both read *"The mappings whose
roots the device cannot vouch for, in mapping order."* Replace the line with:

```
/// The mappings whose roots the device could not vouch for, in mapping order
/// (spec: EP-12).
```

### 2. `backend/crates/apps/coffret-device/src/findings.rs` (line 152)

```
/// The findings for the mappings a run could not vouch for (spec: EP-12).
```

A run is what was happening at the time. Replace the line with:

```
/// The findings for the mappings whose roots the device could not vouch for
/// (spec: EP-12).
```

### 3. `backend/crates/domain/coffret-usecase/src/fetch/fetch_error.rs` (line 289)

```
    /// A root the capability would not vouch for becomes
    /// [`RefusedRoot`](Self::RefusedRoot), which is the verdict a *single* write
```

The paragraph cites EP-13, whose subject is the root. The capability is the
port the question travels through. Replace the first of those lines with:

```
    /// A root that will not vouch for itself becomes
```

### 4. `backend/crates/apps/coffret-device/src/error.rs` (line 963)

```
    /// A mapped root the capability would not vouch for is
    /// [`RootRefused`](Self::RootRefused), carrying the folder and which of
```

The same sentence in the device crate, and the paragraph goes on to name
EP-13's cases explicitly. Replace the first of those lines with:

```
    /// A mapped root that will not vouch for itself is
```

### 5. `backend/crates/domain/coffret-usecase/src/freeze/freeze_outcome.rs` (line 60)

```
    /// request named (spec: PK-17): it names every mapping the device holds whose
    /// root the run could not vouch for, including one standing for a subtree
```

The run is what was happening; the same sentence has already named the device
as the one that holds the mappings, so the subject is there to bind to.
Replace the second of those lines with:

```
    /// root it could not vouch for, including one standing for a subtree
```

### 6-7. `backend/crates/apps/coffret-server/src/routes/activity.rs` (lines 62-64 and 81-82)

Both DTO fields say *"What the run found and did not act on — … a mapped root
**it** could not vouch for"*, where the pronoun binds to the run. EP-12's
subject is the device, and the first of the two sentences already names it
(*"a file this device no longer has"*), so the reader meets both subjects in
one sentence. Give the clause the device.

### 8. `backend/crates/apps/coffret-device/src/lib.rs` (lines 115-116)

*"the files a run left alone, the mapped roots **it** could not vouch for, the
Containers it has no key for, the batches it settled"* — EP-12 is cited two
lines below. Give the mapped-roots clause the device. Read the pronouns that
follow it before you do: what has no key and what settles a batch is the
device, so binding them to the device as well is right, but confirm it rather
than assuming it.

### 9. `backend/crates/apps/coffret-device/src/run_fetch.rs` (line 28)

*"every Entry the run declined — a path **it** could not vouch for"* — this one
is EP-11 rather than EP-12, and EP-11's subject is the device
(`fetch/mod.rs:85`: *"EP-11 places bytes only where the device can vouch for
what is there"*). Give it the device.

### 10. `backend/crates/apps/coffret-device/src/finding_reason.rs` (line 8)

*"a fetch declines a path **it** cannot vouch for"*, in a sentence enumerating
what each of the three flows calls the same answer. The flow is the one
declining; the device is the one that cannot vouch. Give the vouching clause
the device and leave the declining to the fetch.

### 11. `backend/crates/domain/coffret-usecase/src/fetch/mod.rs` (lines 27-28)

*"A fetch places a file only where the local state is one **it** can vouch
for"* — the same file says it the other way at line 85. Give it the device.

## Out of scope

- `local_scan/mod.rs:16`'s *"both would otherwise draw a conclusion from an
  absence they cannot vouch for"*. The subject is the two flows, but what they
  cannot vouch for is an *absence* — the inference a flow would draw from an
  empty folder — rather than a root or a path. That sentence is about what a
  flow may conclude, not about either rule's subject, so it is left as it is.
- A grep cannot decide this rule: what matters is what a pronoun binds to, not
  the string. `freeze_outcome.rs`'s repaired line reads *"whose root it could
  not vouch for"* and is correct, because the same clause names the device.
  Do not sweep for `it … vouch` and replace it everywhere.
- Any behaviour, signature, test name, or assertion change.
- `refused_root.rs`'s *"so there is no folder this device may vouch for"* and
  the two copies of that sentence in `destinations_conformance/vouching.rs` and
  `tests/place_faults.rs`. It describes `NoExpectedIdentity` — a mapping with no
  identity recorded at all — where there is nothing for the root to vouch
  *against*, so the device is the only subject left. The three read alike and
  are not the drift this change is about.
- Documenting the two subjects in the concept documents. That belongs with the
  other concept-document work, which is its own change; nothing under
  `docs/concepts/` or `docs/spec/` moves here.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes.
- [x] No doc takes the walk, a run, or the capability as the subject of *vouch*:
      `! grep -rq "the walk could not vouch" backend/crates`,
      `! grep -rq "run could not vouch" backend/crates`, and
      `! grep -rq "the capability would not vouch" backend/crates`.
- [x] The two EP-12 fields take the device, in the words their siblings already
      use:
      `grep -q "whose roots the device could not vouch for" backend/crates/domain/coffret-usecase/src/local_scan/walked.rs`
      and
      `grep -q "whose roots the device could not vouch for" backend/crates/apps/coffret-device/src/findings.rs`.
- [x] The two EP-13 paragraphs take the root:
      `grep -q "will not vouch for itself" backend/crates/domain/coffret-usecase/src/fetch/fetch_error.rs`
      and
      `grep -q "will not vouch for itself" backend/crates/apps/coffret-device/src/error.rs`.
- [x] No pronoun binds a mapped root's or a path's vouching to the flow that was
      running:
      `! grep -q "a mapped root it could not vouch for" backend/crates/apps/coffret-server/src/routes/activity.rs`,
      `! grep -q "the mapped roots it could not" backend/crates/apps/coffret-device/src/lib.rs`,
      `! grep -q "a path it could not vouch for" backend/crates/apps/coffret-device/src/run_fetch.rs`,
      `! grep -q "a path it cannot vouch for" backend/crates/apps/coffret-device/src/finding_reason.rs`,
      and `! grep -q "the local state is one it" backend/crates/domain/coffret-usecase/src/fetch/mod.rs`.
