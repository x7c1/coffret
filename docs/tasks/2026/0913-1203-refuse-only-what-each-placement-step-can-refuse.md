---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [rust-module-structure, error-type-design, completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rq "prefix: Option<EntryPath>" backend/crates/apps/coffret-device/src/add && grep -rq "fn from_below_root(refused: BelowRootError, path: &EntryPath)" backend/crates/domain/coffret-usecase/src/fetch && ! grep -rq "RefusedRoot {" backend/crates/domain/coffret-usecase/src/fetch/fetch_error.rs && grep -rq "fn reach" backend/crates/domain/coffret-usecase/src/destinations.rs && grep -rqs "vouching" backend/crates/domain/coffret-usecase/src/destinations_conformance && grep -rq "RefusedRoot" backend/crates/domain/coffret-usecase/src/refused_root.rs'
assignee: null
branch: task/0913-1203-refuse-only-what-each-placement-step-can-refuse
created_at: 2026-09-13T12:03:12Z
updated_at: 2026-09-13T13:14:56Z
---

# refactor(backend): let each placement step refuse only what it can refuse

## Overview

`DescentError` has three variants, and every capability in the placement path
returns all three:

- `Blocked { stopped_at }` — a component on the way is a symbolic link, a
  non-directory, or became one while the descent walked past it (spec: EP-4,
  EP-11).
- `Refused { root, reason }` — the mapped root is not the root the mapping was
  recorded against (spec: EP-13).
- `Io(LocalIoError)` — the operating system refused for some other reason.

**Only one method can produce `Refused`.** `Destinations::reach` asks the marker
question, once, against the root handle it has just opened and before a single
component below it is descended; nothing below that point asks again. So
`Destination::create` and `remove`, `ScratchFile::write` and `flush`,
`FlushedFile::stamp` and `publish`, and `Destinations::look_up` all name a
refusal they cannot return.

That is not only untidy — it costs a field and an argument, and it puts a value
nobody can use in front of four `match` arms:

- `IncomingFile` carries `prefix: Option<EntryPath>`
  (`coffret-device/src/add/incoming_file.rs:69`) for one purpose: to hand it to
  `Error::descent(refused, prefix.as_ref(), &path)` at line 98, on the write
  path, where `Refused` never arrives. `IncomingFile::open` takes it as an
  argument for the same reason, and `receive_file.rs:85` passes it in.
- `FetchError::from_descent` (`coffret-usecase/src/fetch/fetch_error.rs:322`)
  takes `prefix: Option<&EntryPath>` and carries a `RefusedRoot` arm that is
  live only for the one caller that descends through `reach`.

### The change

Give the seven methods that cannot refuse an error type that says so — the two
variants they can actually produce — and leave the three-variant type to
`Destinations::reach`, the one method that asks the marker question. Both
implementations (`coffret-local-fs`'s `unix_destinations` and the in-memory
fake) and the `destinations_conformance` suites follow the signatures.

Then `IncomingFile` loses its `prefix` field and `open` loses that argument,
and `from_descent` loses both its `prefix` parameter and its `RefusedRoot` arm.
Where a refused root does have to be reported, it is reported at the place that
learned it: `receive_file.rs:84` already calls `reach` and already has the
mapping in hand.

**`FetchError::RefusedRoot` should carry the `RefusedRoot` value rather than its
parts.** `coffret-usecase/src/refused_root.rs` already defines the type that
says which of EP-13's cases was met; the error variant restates its fields
instead of holding it. Once `from_descent` no longer builds that arm, the one
place that does build it can hand the value straight through.

Name the narrow type for what it is about. It is not "a small DescentError" —
it is the set of ways a step *below an already-vouched root* can fail, and its
name should let a reader see why `reach` needs a wider one.

## What must not change

- **`Destinations::reach` keeps the full three-variant vocabulary**, and keeps
  asking the marker question against the handle it has just opened rather than
  a re-resolved path — that ordering is the race EP-13 rules out, and this
  change must not disturb it.
- **The `destinations_conformance` suites keep binding both implementations to
  one contract.** `vouching.rs`, `blocking.rs` and `round_trip.rs` exist so the
  real gateway and the fake cannot drift; narrowing a signature must narrow it
  in the contract too, not route around the suite.
- Every refusal a caller can see today it can still see. This change moves
  where a value is carried, never whether a failure is reported.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `Destination::create`/`remove`, `ScratchFile::write`/`flush`,
      `FlushedFile::stamp`/`publish` and `Destinations::look_up` return an error
      type with no refused-root case; `Destinations::reach` still returns one
      that has it.
- [x] `IncomingFile` no longer holds a Library-side prefix, and
      `IncomingFile::open` no longer takes one.
- [x] The conversion that was `FetchError::from_descent` no longer takes a
      prefix argument and no longer has an arm for a refused root. It is also
      renamed, since it no longer receives a descent's refusal: the old name is
      the one thing left asserting the old shape.
- [x] `FetchError`'s refused-root case carries the `RefusedRoot` value rather
      than restating its fields.
- [x] Both `Destinations` implementations and the `destinations_conformance`
      suites compile against the narrowed signatures, and every conformance case
      that passed before still passes — including the vouching cases, which
      still drive six of EP-13's seven refusal reasons through `reach`. The
      seventh, `MarkerNotARegularFile`, is driven per implementation in
      `coffret-local-fs/tests/fetch_confinement.rs` and
      `coffret-usecase/tests/place_faults.rs`; that arrangement predates this
      change and is unaltered by it.
- [x] A browser upload into a refused root still fails the request and names the
      mapping, and a folder fetch still continues past one and reports it — the
      two behaviours EP-11 distinguishes are unchanged by the move.

### Manual / on-hardware (verified by a human before merge)

- [ ] None. The change is confined to how failures are typed on a path the
      conformance suites and the existing fetch and upload tests already drive
      end to end with both a real and a fake filesystem.
