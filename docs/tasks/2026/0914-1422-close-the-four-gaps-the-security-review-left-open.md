---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && test -f frontend/packages/gateway/api/src/surfaced-findings.json && ! grep -qE "^  .UnreachablePlace.,$" frontend/packages/gateway/api/src/refusal.ts && grep -rq "surfaced-findings.json" backend/crates/apps/coffret-server/ && grep -rq "a_value_after_recovery_code_stdin_is_not_echoed" backend/crates/apps/coffret-cli/tests/ && grep -qE "pub use zeroize" backend/crates/apps/coffret-device/src/lib.rs && grep -rq "giving_up_names_neither_bucket_nor_prefix" backend/crates/domain/coffret-usecase/src/retry/'
assignee: null
branch: task/0914-1422-close-the-four-gaps-the-security-review-left-open
created_at: 2026-09-14T14:22:04Z
updated_at: 2026-09-14T15:18:15Z
---

# test: close the four gaps the security review left open

## Overview

Four items from the security review are still open because each needed a
small piece of code or a test rather than a sentence. None of them changes
what the product does for a person; each closes a way for a defect to pass
unnoticed. They are one change because they are one kind of work — making a
guarantee the review asked for checkable — and each is small.

### The explorer's list of surfaced findings is a hand-written copy

`frontend/packages/gateway/api/src/refusal.ts` holds `FINDINGS`, a literal
list of the six names the server puts in a refusal's `surfaced` field
(`ForeignFile`, `LocallyChanged`, `WitnessedDeletion`, `UnreachablePlace`,
`KeyLost`, `ReservedComponent`). The server spells them in `name_of` in
`backend/crates/apps/coffret-server/src/api_error/mod.rs`. Nothing ties the
two: a variant renamed or added on the Rust side makes the explorer read the
new name as `null` and show the generic sentence, silently.

Make one side the source. Put the names in a committed JSON file,
`frontend/packages/gateway/api/src/surfaced-findings.json` (a plain array of
strings), have `refusal.ts` import it in place of the literal (the workspace
enables `resolveJsonModule`), and add a Rust test in `coffret-server` that
builds the list from `name_of` over every `Surfaced` variant — through an
exhaustive `match`, so a new variant fails to compile until it is named —
and compares it with the file's content, reading the file by a path relative
to `CARGO_MANIFEST_DIR`. Keep the file sorted the way the Rust side emits it
so the comparison is exact. Update `refusal.test.ts` if it asserts the
literal. The result: renaming a variant fails `cargo test` until the JSON is
regenerated, and the explorer follows the JSON with no second list to
forget. The `SurfacedFinding` type union stays as it is: it is a
compile-time shape, and what a rename breaks silently is the runtime list.

### A value typed after `--recovery-code-stdin` is echoed by clap

`backend/crates/apps/coffret-cli/src/join.rs` declares `recovery_code_stdin`
as a bare flag. A script migrating from an older form may write
`--recovery-code-stdin <code>`; clap then refuses the unexpected argument
and prints it to stderr — which is the Recovery Code, a secret (spec: KD-11).
Make that path not echo the value: whichever way is smallest with this clap
version (a value parser that refuses any value with a message of its own, a
`num_args` that rejects a value, or handling the parse error before clap
prints it), the refusal must say that the flag takes no value and that the
code is read from standard input, and must not contain the value. Add a
test named `a_value_after_recovery_code_stdin_is_not_echoed` beside the
others in `backend/crates/apps/coffret-cli/tests/setup.rs`, using a sentinel
that cannot appear by accident, asserting a non-zero exit, a stderr that
guides, and a stderr and stdout that do not contain the sentinel. Check
whether `--passphrase-stdin` has the same shape and treat it the same way.

### `coffret-device` names a type it does not export

`join_library` (`backend/crates/apps/coffret-device/src/join_library/run.rs`)
takes a callback returning `Result<Zeroizing<String>>`, so a caller has to
name `zeroize::Zeroizing` — a dependency the crate does not re-export.
`coffret-cli` works today only because it reaches the type another way. Add
`pub use zeroize::Zeroizing;` (or `pub use zeroize;`) to the crate's public
surface in `lib.rs` with a one-line doc saying why, and make `coffret-cli`
name it through `coffret_device` where it currently does not. Do not add
`zeroize` as a direct dependency anywhere it is not already one.

### The retry's "gave up" line is not proven free of Storage names

`backend/crates/domain/coffret-usecase/src/retry/retry_policy/run.rs` logs
`gave_up` with `error = %error.redacted()`. The redaction is what keeps an
S3 bucket or prefix, or a Drive folder name, out of the diagnostic event
(spec: EL-1, EL-5), and `retry/tests.rs` proves attempts and waits but not
that. Add a test named `giving_up_names_neither_bucket_nor_prefix` in
`retry/tests.rs`: drive the policy to give up on an error whose cause
carries a bucket name and a prefix that cannot appear by accident (build it
the way the Storage gateways build theirs — look at how `Error` carries a
Storage cause and how `redacted()` treats it), capture the `warn!` with the
crate's existing test logging support (see how other tests in this crate
capture events; if none does, use `tracing-test` or a subscriber writing to
a buffer, as the workspace already does elsewhere), and assert the captured
line names neither. If the redaction turns out not to cover a field the
Storage error carries, that is a defect to fix in `redacted()` in the same
change, with the test as its proof.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `refusal.ts` no longer carries a literal list of surfaced findings; it
      imports `surfaced-findings.json`, and a Rust test under
      `backend/crates/apps/coffret-server/` compares that file with the names
      `name_of` gives every `Surfaced` variant.
- [x] `a_value_after_recovery_code_stdin_is_not_echoed` passes: a value
      typed after the flag is refused without being printed.
- [x] `coffret_device` re-exports `zeroize`'s `Zeroizing`, and the CLI names
      the type through it.
- [x] `giving_up_names_neither_bucket_nor_prefix` passes: the retry's
      give-up event carries neither the bucket nor the prefix of the cause.
- [x] `make check` passes with the new tests.
