---
status: completed
pipeline_phase: null
follow_up_of: docs/tasks/2026/0908-1929-read-recovery-codes-without-command-line-arguments.md
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "A Recovery Code reaches" backend/crates/apps/coffret-device/src/lib.rs && grep -q "a way to ask for the secrets it needs" backend/crates/apps/coffret-cli/src/main.rs'
assignee: null
branch: task/0909-1129-follow-up-read-recovery-codes-without-command-line-arguments
created_at: 2026-09-09T11:29:51Z
updated_at: "2026-09-09T12:04:39Z"
---

# docs(backend): mention the Recovery Code where the device crate and CLI describe secret input

## Overview

Two crate-level doc comments still describe secret input as if the Passphrase
were the only secret a shell hands to `coffret-device`. Since `join_library`
now receives the Recovery Code through the same kind of callback, bring both
comments up to date. No behaviour changes: only doc-comment text is edited.

- `backend/crates/apps/coffret-device/src/lib.rs`, line 131, the last line of
  the callback paragraph (`//! Passphrase twice to be told about.`): replace
  that single line with these four lines:

  ```
  //! Passphrase twice to be told about. A Recovery Code reaches [`join_library`]
  //! the same way and for the same reason: it is the Master Key in the form a
  //! person writes down (spec: KD-11), so it is asked for after those refusals
  //! and never taken as a value a caller could print or leave in a command line.
  ```

- `backend/crates/apps/coffret-cli/src/main.rs`, lines 3-4 (`//! Everything here
  is a shell. Each subcommand reads what was typed, hands` and `//! \`coffret-device\`
  a way to ask for the Passphrase where one is needed, calls`): replace those two
  lines with these three:

  ```
  //! Everything here is a shell. Each subcommand reads what was typed, hands
  //! `coffret-device` a way to ask for the secrets it needs — a Passphrase, and
  //! the Recovery Code where a Library is being joined — calls
  ```

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `backend/crates/apps/coffret-device/src/lib.rs` names the Recovery Code in
      its callback paragraph: `grep -q "A Recovery Code reaches" backend/crates/apps/coffret-device/src/lib.rs`
- [x] `backend/crates/apps/coffret-cli/src/main.rs` describes both secrets in its
      crate doc: `grep -q "a way to ask for the secrets it needs" backend/crates/apps/coffret-cli/src/main.rs`
