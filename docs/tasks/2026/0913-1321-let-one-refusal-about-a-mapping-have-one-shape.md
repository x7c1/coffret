---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [error-type-design, completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && [ "$(grep -rc "fn mapping_named" backend/crates --include=\*.rs | awk -F: "{s+=\$2} END{print s}")" -eq 1 ] && ! grep -rl "enum FindingReason" backend/crates/apps/coffret-device/src | xargs grep -qE "^#\\[derive\\(.*PartialEq" && [ "$(grep -c "^pub \(struct\|enum\)" backend/crates/domain/coffret-usecase/src/refused_root.rs)" -eq 1 ] && ! grep -rq "RootRefused {" backend/crates/apps/coffret-device/src && ! grep -rq "Self::UnavailableRoot { local_root, reason }" backend/crates/apps/coffret-device/src'
assignee: null
branch: task/0913-1321-let-one-refusal-about-a-mapping-have-one-shape
created_at: 2026-09-13T13:21:49Z
updated_at: 2026-09-13T14:17:49Z
---

# refactor(backend): let one refusal about a mapping have one shape

## Overview

Two findings answer two questions about the same mapping. EP-12's asks whether
the root is there to be read from; EP-13's asks whether the folder standing at
it is the one whose marker the mapping recorded. They are separate findings on
purpose — a root can be perfectly available and still be the wrong folder.

But only one of them says *which mapping*.

`Finding::RefusedRoot` (`coffret-device/src/finding.rs:62`) carries a `prefix`,
and its own documentation explains why: it is "the half of the mapping a finding
may name", a name inside the Library rather than a path on this device, so it
reaches the person who asked for the run and never a diagnostic event
(spec: EL-1). `Finding::UnavailableRoot` (`finding.rs:43`) carries only
`local_root` and `reason`.

The value it is built from already has the prefix.
`coffret_usecase::UnavailableRoot` (`unavailable_root.rs:25`) holds
`prefix: Option<EntryPath>` with the same EL-1 rule written on it — and
`findings.rs:155` drops it on the way across:

```rust
roots.iter().map(|root| Finding::UnavailableRoot {
    local_root: root.local_root.clone(),
    reason: root.reason,
})
```

So a person told their root is unavailable learns the folder but not which of
their mappings it was, while the same person told a root was refused learns
both. The comment above `RefusedRoot`'s `Display` arm even says it speaks "in
the voice the unavailable root above is said in" — which is true of everything
except the one thing it names extra.

### The change

Carry the prefix across, and say it in the message the way EP-13's finding
already does. `noted.rs:49` maps this finding into a diagnostic event and must
go on dropping the prefix there — EL-1 forbids it in an event, and the field's
documentation on both types says so.

**Two more places where one refusal has two shapes**, fixed with it:

- **`mapping_named` exists twice**, character for character:
  `coffret-device/src/error.rs:521` and
  `coffret-usecase/src/fetch/fetch_error.rs:598`. It renders a prefix into the
  phrase a message uses for the mapping — the exact job this change gives the
  unavailable-root finding, which is why the duplication is worth closing now
  rather than living with a third caller. `coffret-device` already depends on
  `coffret-usecase`, so one of them can be the other's.

- **`Error::RootRefused { prefix, root, reason }`**
  (`coffret-device/src/error.rs:266`) restates the three fields of
  `coffret_usecase::RefusedRoot` instead of holding the value.
  `FetchError::RefusedRoot` carries that value whole, and its documentation
  gives the reason: restating the fields makes one refusal two shapes. The same
  argument applies here, and this one also spells the middle field `root` where
  the value calls it `local_root`. Collapsing it reaches
  `coffret-server/src/routes/upload/refusal.rs:55`, which matches on the
  variant.

### Two local cleanups in the files this opens

Named separately so a reviewer can see they were chosen rather than swept in:

- **`refused_root.rs` holds two public types** — `RefusedRoot` (line 39) and
  `RootRefused` (line 67) — and the module is named after one of them. Give
  `RootRefused` its own module and add the `mod` / `pub use` pair beside the
  existing one.
- **`FindingReason` derives `PartialEq, Eq`** (`finding_reason.rs:22`) and
  nothing compares one. An error-shaped type that derives equality invites a
  test to compare whole values, which binds the test to the representation of a
  failure rather than its meaning and makes a later field addition breaking.

## What must not change

- **EP-12's finding and EP-13's stay separate findings.** They answer different
  questions and the documentation on both says so; this change makes them agree
  on how they name a mapping, not on what they mean.
- **No prefix reaches a diagnostic event.** `noted.rs` is where that line is
  held for these findings, and the tests that pin it stay.
- The `Display` output for `RefusedRoot` keeps naming the folder, the mapping
  and the gesture; this change adds the mapping to its sibling rather than
  taking anything from it.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `Finding::UnavailableRoot` carries the mapping's Library-side prefix, and
      the conversion at `findings.rs` passes the one the value already holds
      rather than dropping it.
- [x] Its `Display` names the mapping the way EP-13's finding does, asserted by
      a test that distinguishes a prefixed mapping from the Library root.
- [x] A test asserts that the diagnostic event built from this finding carries
      no prefix, so EL-1 still holds on the path this change widens.
- [x] `mapping_named` is defined once in the workspace and used by both callers.
- [x] `coffret-device`'s refused-root error carries the `RefusedRoot` value
      rather than restating its fields, and no site spells the local root two
      different ways.
- [x] `RootRefused` lives in a module named after it, and `refused_root.rs`
      declares exactly one public type.
- [x] `FindingReason` does not derive `PartialEq` or `Eq`, and every test that
      inspects one matches on its variant.

### Manual / on-hardware (verified by a human before merge)

- [ ] None. Every path here is covered by the existing device and server test
      suites; the message text is asserted rather than eyeballed.
