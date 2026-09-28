---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/0928-1053-name-the-check-a-malformed-input-failed
created_at: 2026-09-28T10:53:41Z
updated_at: 2026-09-28T11:21:21Z
---

# fix: name the check a malformed input failed, and never suggest a secret into the wrong field

## Overview

A refusal of something a person typed names what was wrong with it, precisely enough that the person knows what to change — and for input that may carry a secret, it never points them at a field where the secret would be exposed. Two refusals fall short.

1. **`MalformedRecoveryCode` folds three different mistakes into one.** KD-11 says a Recovery Code is refused "each with an answer naming the check that failed" (`docs/spec/key-derivation/README.md`, around lines 163-172), but its refusal list puts "a character outside the alphabet, or no separator" in one item, and the implementations follow it: Rust's `coffret-format` has one variant (`recovery_code/parse.rs`, around lines 124-131; `error/mod.rs`, around lines 658-667; `error/display.rs`, around line 296), and TypeScript one code (`frontend/packages/domain/format/src/errors.ts`, around line 93; `recoveryCode/recoveryCode.ts`, around lines 143 and 182, `failFault`). A person who typed a code without its separator and one who typed a character that cannot be in a prefix read the same answer. Split the check into separate refusals in KD-11 and in both implementations, with matching variant and code names: no separator; a human-readable part that is empty; a human-readable part holding a character a prefix cannot. The device's wrapper (`coffret-device/src/error/mod.rs`, around line 535) keeps carrying the format error as its cause. Add the three inputs — `1qqqqqqq` (empty before the separator), `coffretqqqq` (no separator), a non-ASCII prefix — to both implementations' tests, and to the interop corrupted-exchange cases (`apps/coffret-interop/tests/corrupted_exchange.rs`) so the two implementations are held to the same answer. If the concept document for the Recovery Code lists the refusals, update it too.
2. **A misspelt `--client-secret` can still be suggested into `--client-id`.** The shell refuses `--client-secret` without quoting its value and without clap's suggestion, because clap would suggest `--client-id` and invite the person to paste the secret into the ID (`coffret-shell/src/parser_refusal.rs`, the reason around lines 53-66, the exact match around lines 70-73). A misspelling such as `--client-scret` is not matched and gets clap's suggestion, which is the harm the carve-out exists to prevent. Refuse the same way whenever clap's suggestion (`ContextKind::SuggestedArg`) is `--client-id` and what was typed is not `--client-id`, since the suggestion itself is the harm. Check whether clap quotes the value of `--client-scret=VALUE` in its error; if it does, the same refusal closes that too. Add the misspelt forms, with and without `=value`, to the existing tests (around line 282).

Decision this task makes: the new TypeScript error codes and Rust variant names are part of the format library's public surface. They are named here, before a release, rather than later, because KD-11 already promises the distinction and the single code breaks that promise today.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] KD-11's refusal list names the three checks separately, and Rust and TypeScript refuse `1qqqqqqq`, `coffretqqqq` and a non-ASCII prefix with the matching three answers (`make check`, including the interop corrupted-exchange cases)
- [x] A misspelt `--client-secret`, with and without `=value`, is refused without clap's `--client-id` suggestion and without its value in the output (`make check`)
- [x] `make check` passes
