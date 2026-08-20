---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, error-type-design]
max_refine_rounds: 2
retries_remaining: 1
check_command: "make s3-store-it && cd backend && cargo fmt --all -- --check && cargo build && cargo test && cargo clippy --all-targets -- -D warnings"
assignee: null
branch: task/0820-1336-drop-error-equality
created_at: 2026-08-20T13:36:42Z
updated_at: 2026-08-21T01:45:00Z
---

# refactor(backend): stop deriving equality on error types and assert on variants instead

## Overview

Four of the workspace's error types derive `PartialEq, Eq`, and the test suite
compares whole error values with `assert_eq!` in eighty-odd places. Both are
wrong for the same reason: an equality-comparable error couples every
assertion to the *representation* of a failure rather than its meaning. The
practical costs are concrete, not stylistic — adding a field to any existing
variant becomes a breaking change for every comparison site, and any variant
that ever needs to carry dynamic context (a source error, a path, a line
number) cannot, because equality would stop being stable. Today's variants are
all cheaply comparable, which is exactly why the constraint is invisible; it
becomes visible the first time one of them needs to grow.

The four types that derive equality, and where their comparisons live:

| error type | files converted | comparisons removed |
| --- | --- | --- |
| `coffret-format` | 14 | 67 |
| `coffret-model` | 7 | 11 |
| `google-drive-store` (`http::TransportError`) | 3 | 4 |
| `coffret-usecase` | 2 | 1 |

`s3-store` needs no change: its error type never derived equality, and nothing
compares one. A first pass counted ten hits there, but each was an `Err(Error::…)`
being *constructed* and returned, not compared — a reminder that the grep below
locates candidates and the compiler decides.

The change is mechanical but must land as one commit per error type's blast
radius — removing a derive without converting its comparison sites does not
compile — so all four move together here.

What to do:

- Remove `PartialEq, Eq` from the error enums at
  `backend/crates/domain/coffret-model/src/error.rs`,
  `backend/crates/domain/coffret-format/src/error.rs`,
  `backend/crates/domain/coffret-usecase/src/error.rs`, and
  `backend/crates/gateway/google-drive-store/src/http/transport_error.rs`.
  Leave `Debug` and `Clone` alone.
- Convert every assertion that compares an error value into one that matches
  on the variant — `assert!(matches!(actual, Err(Error::Variant { .. })))`, or
  a `match` where the failure message is worth writing out.
- **Preserve the value assertions that exist.** Many of these comparisons do
  more than name a variant: they pin the fields, as in
  `assert_eq!(stream.into_bytes().await, Err(Error::LengthMismatch { expected: 64, actual: 10 }))`.
  Literal patterns inside `matches!` keep that check
  (`matches!(r, Err(Error::LengthMismatch { expected: 64, actual: 10 }))`); for
  a field that cannot be written as a pattern, destructure and assert the
  field. Do not quietly weaken an assertion to "some error of this variant"
  when it used to pin the values — that trades one kind of test-rot for
  another.
- Where a converted assertion loses its failure message (a bare `matches!`
  prints nothing useful on failure), give it one, so a red test still says
  what it expected.

Search for the sites with, from `backend/`:

```
grep -rn "Err(Error::\|assert_eq!(.*Error::" --include="*.rs" crates
```

Treat that as a starting point, not the definition of done: comparisons
written through a `Result` binding, an aliased import, or `assert_ne!` will
not match it. The compiler finds the rest — once a derive is gone, every
remaining comparison is a build error.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] None of the four error enums derives `PartialEq` or `Eq`; the whole
      workspace still builds, and `cargo clippy --all-targets -- -D warnings`
      is clean.
- [x] No test compares an error value for equality: from `backend/`,
      `grep -rn "assert_eq!(.*Error::\|assert_ne!(.*Error::" --include="*.rs" crates`
      returns no matches.
- [x] Every assertion that previously pinned a variant's field values still
      pins them — the reviewer can confirm this per site in the diff, and the
      suite still fails if a field's value changes. Spot-check that the
      `LengthMismatch` assertion in
      `backend/crates/domain/coffret-usecase/src/byte_stream.rs` still checks
      `expected` and `actual`, not just the variant.
- [x] `cargo test` passes across the workspace, and the MinIO conformance run
      (`make s3-store-it`) still passes — the conformance suite is one of the
      places asserting on typed errors.

## Out of scope

- **Restructuring the `detail: String` fields.** The error-design convention
  says a cause should keep its structure rather than being flattened into a
  string, and at first glance every variant of `coffret_usecase::Error`
  violates it. It does not: that type is deliberately a provider-neutral
  vocabulary — its own documentation says it is "not any one provider's error
  catalogue", and the whole point of a gateway translating into it is that
  callers never inspect a provider's message to decide what happened. Carrying
  the SDK error would reintroduce exactly the coupling the port exists to
  remove. The rendered `detail` is for humans reading a log. Leave it, and do
  not let a review pass "fix" it.
- Adding, removing, or renaming error variants, and changing any
  `is_retryable` verdict.
- Converting assertions that do not involve an error type.
- Any behavior change at all. This task must not alter what the code does —
  only what the tests are allowed to depend on.
