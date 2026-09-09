---
status: completed
pipeline_phase: null
follow_up_of: docs/tasks/2026/0909-1507-carry-key-custody-past-the-master-key.md
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "spec: DK-10" backend/crates/apps/coffret-shell/src/passphrase.rs'
assignee: null
branch: task/0909-1948-follow-up-carry-key-custody-past-the-master-key
created_at: 2026-09-09T19:48:58Z
updated_at: 2026-09-09T19:56:19Z
---

# docs(shell): cite DK-10 from the Passphrase reader

## Overview

`DK-10` states that a secret a device is given to hold — the Passphrase and
the Recovery Code — is taken from a non-echoing prompt or one explicitly
selected line of standard input, never from a command-line argument. The
Recovery Code reader in `coffret-shell` cites it; the Passphrase reader,
which is the other half of the rule and the place that decides no
`--passphrase <value>` exists, carries no pointer to it.

In `backend/crates/apps/coffret-shell/src/passphrase.rs`, replace the doc
comment line

```
/// process table where anyone on the machine could read it.
```

with

```
/// process table where anyone on the machine could read it (spec: DK-10).
```

No behaviour changes.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The Passphrase reader cites `DK-10`, verified by
      `grep -q "spec: DK-10" backend/crates/apps/coffret-shell/src/passphrase.rs`.
