---
status: completed
pipeline_phase: null
follow_up_of: docs/tasks/2026/0911-1738-leave-the-management-area-out-of-a-local-listing.md
base_ref: feat/mapped-root-marker
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -q "the mapping can vouch for" backend/crates/gateway/coffret-local-fs/tests/fetch_confinement.rs && grep -q "The path side of the same reservation the scan reads by name" backend/crates/domain/coffret-usecase/src/root_marker.rs && grep -q "spec: LA-3" backend/crates/apps/coffret-device/Cargo.toml && ! grep -q "Nothing here is key material" backend/crates/apps/coffret-device/Cargo.toml'
assignee: null
branch: task/0911-1832-follow-up-leave-the-management-area-out-of-a-local-listing
created_at: 2026-09-11T18:32:00Z
updated_at: 2026-09-12T02:14:21Z
---

# docs(backend): say who vouches, who asks the reservation, and what the entropy is drawn for

## Overview

Three comments describe the code as it was before a mapped root gained an
identity of its own (spec: EP-13) and before a local listing began asking the
reserved-name question (spec: EP-14). Each names a subject, a caller, or a
purpose that has since changed. This is documentation only: no behaviour, no
signature, and no test changes.

### 1. `backend/crates/gateway/coffret-local-fs/tests/fetch_confinement.rs` (line 517)

This repository writes EP-12's availability check as *the device vouching for
the root* and EP-13's marker check as *the root vouching for itself*. One
assertion message takes the mapping as the subject instead, which is neither.
It is the last one; replace the line

```
        "a root whose marker is a link is not a root the mapping can vouch for",
```

with

```
        "a root whose marker is a link is not a root that vouches for itself",
```

### 2. `backend/crates/domain/coffret-usecase/src/root_marker.rs` (lines 58-61)

`carries_management_area`'s doc opens by calling itself "the placement side of
the same reservation the scan reads by name". A placement is no longer the only
caller: `OpenLibrary::added_locally` now asks it of the folder it was handed,
and a local listing is not a placement. What actually distinguishes this
predicate from `is_management_area` is that its caller holds a path rather than
a walk. Replace those four doc lines with:

```
/// The path side of the same reservation the scan reads by name. A scan asks
/// [`is_management_area`] of each local name as it walks; a caller holding an
/// Entry Path has no walk to ask it during — the question is settled before a
/// single component is descended — so it asks it of the path's own components
/// instead.
```

### 3. `backend/crates/apps/coffret-device/Cargo.toml` (lines 30-32)

The comment above `getrandom` names one thing this crate draws random bytes for
and asserts the dependency draws no key material. It now draws two: the four
bytes that separate two batches, and the thirty-two of the server key
(`KEY_BYTES`, `server_key.rs`). The assertion still holds — a server key is not
Library key material — but it holds for a reason the comment does not give.
Replace

```
# The four random bytes that separate two batches started in the same second
# (spec: OC-2). Nothing here is key material — what a device calls its own
# unfinished batches never leaves it.
```

with

```
# The four random bytes that separate two batches started in the same second
# (spec: OC-2), and the thirty-two a server draws the key it admits its callers
# by from (spec: LA-3). Neither is Library key material: what a device calls its
# own unfinished batches never leaves it, and a server's key is spent when the
# server that drew it stops.
```

## Out of scope

- Any behaviour, signature, test name, or assertion *semantics* change. Item 1
  changes the text of an assertion message and nothing it asserts.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes.
- [x] No assertion takes the mapping as the subject of *vouch*, verified by
      `! grep -q "the mapping can vouch for" backend/crates/gateway/coffret-local-fs/tests/fetch_confinement.rs`.
- [x] `carries_management_area`'s doc distinguishes itself from
      `is_management_area` by what its caller holds rather than by what its
      caller is, verified by
      `grep -q "The path side of the same reservation the scan reads by name" backend/crates/domain/coffret-usecase/src/root_marker.rs`.
- [x] The `getrandom` dependency comment names both draws and the reason neither
      is Library key material, verified by
      `grep -q "spec: LA-3" backend/crates/apps/coffret-device/Cargo.toml` and
      `! grep -q "Nothing here is key material" backend/crates/apps/coffret-device/Cargo.toml`.
