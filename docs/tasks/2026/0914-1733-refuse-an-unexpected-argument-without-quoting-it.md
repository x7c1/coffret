---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -rq "a_secret_typed_as_a_bare_argument_is_not_echoed" backend/crates/apps/coffret-cli/tests/ && grep -q "argument_refused_without_being_quoted" backend/crates/apps/coffret-cli/src/main.rs && grep -q "argument_refused_without_being_quoted" backend/crates/apps/coffret-server/src/main.rs'
assignee: null
branch: task/0914-1733-refuse-an-unexpected-argument-without-quoting-it
created_at: 2026-09-14T17:33:13Z
updated_at: 2026-09-14T18:49:56Z
---

# fix(backend): refuse an argument clap did not expect without quoting it

## Overview

Both binaries hand their argument list to clap and, when clap refuses it,
print clap's own message. For an argument clap did not expect, that message
quotes the argument: `unexpected argument 'X' found`, or, at the top level of
`coffret`, `unrecognized subcommand 'X'`. Whoever types a secret where an
argument goes — `coffret sync --library alpha <passphrase>`,
`coffret join --name second <recovery code>`, `coffret <recovery code>`,
`coffret-server --library alpha <passphrase>` — therefore sees it repeated on
standard error, and into whatever collects standard error. DK-10 says no
refusal repeats what was entered.

`coffret_shell::stdin_flags::value_typed_after_a_secret_flag` already closes
one shape of this, by reading argv before clap for a value written after
`--recovery-code-stdin` or `--passphrase-stdin`. A bare argument has no flag
in front of it to recognise, so it cannot be caught before clap; it has to
be caught in clap's answer. No subcommand of `coffret` takes a positional
argument and neither does `coffret-server`, so an unexpected argument that
does not start with `-` is always a mistake of this shape, whatever it
carried.

### What to build

Add to `coffret-shell` (which is where the process-start behaviour both
binaries share lives) a function

```rust
pub fn argument_refused_without_being_quoted(error: &clap::Error) -> Option<String>
```

that returns `Some(refusal)` when the error is one clap would answer by
quoting a value that may be a secret, and `None` otherwise. Concretely:

- `ErrorKind::UnknownArgument` where the offending argument
  (`error.get(ContextKind::InvalidArg)`) does not start with `-`, and
  `ErrorKind::InvalidSubcommand` (`ContextKind::InvalidSubcommand`). A
  mistyped flag such as `--folder` still gets clap's own message with its
  suggestion, because a flag is not a secret and the person needs to see
  which one was wrong.
- The refusal never contains the argument. It says that this command takes
  no such argument, that a Passphrase or Recovery Code is given at the
  prompt or, for a script, on standard input behind `--passphrase-stdin` /
  `--recovery-code-stdin`, and — the part nothing else will tell the person —
  that if what was typed was a secret it had already reached the process's
  argument list and the shell's history before this refusal, so treat it as
  having been seen. Reuse the wording `stdin_flags::refusal` uses for that
  last part rather than writing a second version of it; if that means
  lifting the sentence into a shared helper inside `stdin_flags`, do so.
- Where the error carries a usage line, printing that line after the
  refusal is welcome, as long as it is checked not to carry the argument
  (clap's usage line does not; `error.render()` does).

`coffret-shell` does not depend on `clap` today; add it as a workspace
dependency of that crate with a one-line comment in `Cargo.toml` saying why
(the same style as the `zeroize` entry there). Place the function in
`stdin_flags.rs` or a sibling module, whichever reads better next to
`value_typed_after_a_secret_flag`; if a sibling, name it for what it is and
list it in `lib.rs`'s crate doc, which currently describes `stdin_flags` as
the whole of the pre-parse guard and should be brought to say what the two
halves are.

Then in `backend/crates/apps/coffret-cli/src/main.rs` and
`backend/crates/apps/coffret-server/src/main.rs`, in the `Err(error)` arm of
`try_parse()`, call it first: `Some(refusal)` prints `error: {refusal}` to
standard error and exits with failure; `None` keeps the current behaviour
(`error.print()`, exit by `use_stderr()`). Update the comments on both arms,
and the comment above the `value_typed_after_a_secret_flag` call, so that
together they say what each of the two guards catches and why there are two.

### Tests

- A unit test in `coffret-shell` that builds a small `clap::Command` (with a
  subcommand that takes no positionals, and a `--flag`) and uses
  `try_get_matches_from` to produce real errors: a bare sentinel after the
  subcommand, a sentinel in the subcommand position, `--flag` followed by a
  sentinel positional, and a mistyped `--flg`. Assert the first three return
  `Some` whose text does not contain the sentinel and does contain
  `having been seen`, and the last returns `None`.
- An integration test in `backend/crates/apps/coffret-cli/tests/setup.rs`
  named `a_secret_typed_as_a_bare_argument_is_not_echoed`, next to
  `a_value_after_recovery_code_stdin_is_not_echoed` and reusing its
  `TYPED_SECRET` sentinel and its `Device` harness. Drive at least these
  shapes through the binary: `join --name beta --s3 ... TYPED_SECRET`,
  `sync --library beta TYPED_SECRET`, and `TYPED_SECRET` alone as the
  subcommand. For each, assert a non-zero exit, that neither stdout nor
  stderr contains the sentinel, and that stderr says `having been seen`.
- `coffret-server` gets the same helper; its coverage is the unit test plus
  the grep gate in `check_command`. If `coffret-server/tests` already has a
  way to run the binary with arguments, add one case there too; do not build
  a harness for it.

### Keep out

Do not add positional arguments anywhere, do not change what
`value_typed_after_a_secret_flag` catches, and do not touch the spec or the
concept documents: DK-10 already states the rule this change makes true.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `coffret_shell` exports `argument_refused_without_being_quoted`, and
      both `coffret-cli/src/main.rs` and `coffret-server/src/main.rs` call it
      in the `Err` arm of `try_parse()` before printing clap's own error.
- [x] The `coffret-shell` unit test passes: an unexpected bare argument and
      an unrecognised subcommand yield a refusal that does not contain the
      argument and does say `having been seen`; a mistyped `--flag` yields
      `None`.
- [x] `a_secret_typed_as_a_bare_argument_is_not_echoed` passes: a sentinel
      typed as a bare argument after `join`, after `sync`, and in the
      subcommand position is refused with a non-zero exit, and neither
      stdout nor stderr contains it.
- [x] `make check` passes with the new tests.
