---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [completeness, concept-alignment, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -rq "OC-8" backend/crates/apps/coffret-device/ && grep -q "EP-12" backend/crates/domain/coffret-usecase/src/local_io_error.rs && ! grep -rq "scratch file" backend/crates/'
assignee: null
branch: task/0914-0203-let-the-citations-catch-up-with-the-rules
created_at: 2026-09-14T02:03:00Z
updated_at: 2026-09-14T02:39:57Z
---

# docs(backend): let the citations catch up with the rules they state

## Overview

Four doc comments state a rule's content and cite the wrong rule, a narrower
rule, or none at all; one sentence loses its reader to an elided verb; and one
noun is spelled twice. None of them changes behaviour. All of them mislead the
next reader in the same way — by making the register look like it says something
other than what it says.

### 1. `coffret-device` states `OC-8` three times and cites it nowhere

`OC-8` (`docs/spec/orphan-cleanup/README.md`) reads:

> Removing what this device wrote for its own purposes — a spool file, a scratch
> a local writer never published (EP-11), the staging directory an interrupted
> attempt at putting a Library on this device left, or the provenance row (OC-2)
> in the device's own catalog that announced the spool — is idempotent. What is
> already gone is a successful removal, an interrupted clean-up is simply run
> again, and no removal has to check what is there first, because absence is the
> outcome being sought.

**`grep -r "OC-8" backend/crates/apps/coffret-device/` finds nothing**, while
three sites in that crate are doing exactly what the rule describes:

- `staging.rs`'s `begin` removes a staging directory an interrupted attempt left
  and explains why it discards rather than resumes;
- `staging.rs`'s `discard` removes the same directory and records a failure
  rather than reporting it, because absence is the outcome;
- `add/incoming_file.rs` records a scratch an unfinished upload left behind.

The rule is the reason each of them is written the way it is. A reader who wants
to know whether the tolerance is deliberate has nothing to follow.

### 2. `local_io_error.rs` cites one of the two capabilities its sentence covers

`backend/crates/domain/coffret-usecase/src/local_io_error.rs`:

> where absence is an ordinary outcome the capability's own contract says so, and
> swallowing it is the gateway's (spec: OC-8).

Two capabilities meet that condition, not one: `Spool::discard`, which `OC-8`
covers, and `MappedRoots`' absent root, which `EP-12` does. The neighbouring
`mapped_roots.rs` puts the two side by side. Adding the second is a citation the
sentence already earned, not a new claim.

### 3. `LocalOperation::Removing`'s enumeration is a rule behind

`backend/crates/domain/coffret-usecase/src/local_operation.rs`:

> A spool file whose Container was committed or abandoned, or a scratch a failed
> fetch left, was being deleted (spec: OC-8, EP-11).

`OC-8` names four things this device removes for its own purposes — a spool
file, a scratch, the staging directory, and a provenance row — and
`Staging::discard` raises `Removing` for one this enumeration leaves out: the
staging directory. The citation is right and the enumeration under it is not.

### 4. A sentence that loses its reader to an elided verb

`backend/crates/domain/coffret-usecase/src/destinations.rs`:

> Reading the errno that says so is the gateway's, exactly as swallowing the
> absence a [`discard`](crate::Spool::discard) tolerates is (spec: OC-8).

The final `is` belongs to `swallowing … is the gateway's`, four clauses back, and
a reader reaches `tolerates is` before knowing that. The sentence is correct and
unreadable. Say it so the verb arrives where the reader can use it.

### 5. `scratch file` says the noun twice

`EP-11` makes `scratch` a noun: "A **scratch** is the file a local writer fills
before the rename that publishes it." Seven places in `backend/crates/` still
write `scratch file`, which reads as though there were some other kind of
scratch. Two were recorded when the vocabulary landed
(`gateway/coffret-local-fs/src/lib.rs` and
`domain/coffret-usecase/tests/destinations_conformance.rs`); the rest arrived
since. Fix all of them, so the phrase does not come back a third time.

## Out of scope

- **The `choose` collocation** recorded beside these. Its proposal lives only in
  a run log that was never carried forward, so there is nothing to act on — the
  finding is the absence of a record rather than a defect in the tree. It is
  dropped rather than guessed at.
- **`OC-6` citations that should have been `OC-8`** (`index.rs`,
  `index_conformance/device_state.rs`, `destination.rs`) and **the concept
  documents' entry to `OC-8`**: both were recorded as open and both are **already
  closed** in the current tree — those three sites cite `OC-8`, and
  `docs/concepts/library/README.md` cites it twice. Nothing to do.
- **`api_error/mod.rs` arguing `EL-1` from first principles**: also already
  closed; `refused_root_said`'s doc cites `EL-1` where it draws the boundary.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The three sites in `coffret-device` that act on `OC-8`'s tolerance cite it,
      so a reader of any of them can reach the rule that makes the tolerance
      deliberate.
- [x] `local_io_error.rs`'s sentence cites both capabilities it covers rather
      than one.
- [x] `LocalOperation::Removing`'s enumeration covers every removal that raises
      it, the staging directory included.
- [x] `destinations.rs`'s sentence about the errno reads in one pass: no reader
      meets a verb whose subject is four clauses behind.
- [x] The phrase `scratch file` appears nowhere under `backend/crates/`, and each
      site it left still says what it said.
- [x] No rule id cited anywhere in this change names a rule that does not say
      what the citing sentence says it says.

### Manual / on-hardware (verified by a human before merge)

- [ ] Nothing here is observable at runtime: every change is a comment. No manual
      check is needed.
