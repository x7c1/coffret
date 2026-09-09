---
status: completed
pipeline_phase: null
follow_up_of: docs/tasks/2026/0909-1507-give-loopback-access-and-the-storage-grant-rule-ids.md
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "spec: LA-1" backend/crates/apps/coffret-server/src/main.rs'
assignee: null
branch: task/0909-1549-follow-up-give-loopback-access-and-the-storage-grant-rule-ids
created_at: 2026-09-09T15:49:31Z
updated_at: 2026-09-09T15:59:14Z
---

# docs(server): cite LA-1 where the server binds to loopback

## Overview

`LA-1` states that a server serving a Library listens on the loopback
interface alone. It is a `Form: prose` rule whose parenthetical says it is
honored by construction and review, and the bind site in
`backend/crates/apps/coffret-server/src/main.rs` is that construction — yet
it is the only rule in the loopback-access and storage-authorization
mechanisms with no citation anywhere in the code. The sibling prose rule
`LA-7` is cited at its construction site in `authorize/shows_the_key.rs`, and
`main.rs` itself already cites `DK-4` twice.

In `backend/crates/apps/coffret-server/src/main.rs`, replace the comment line

```
    // business rather than the address's. See the crate documentation.
```

(the last line of the comment block directly above
`let address = format!("127.0.0.1:{}", args.port);`) with these two lines:

```
    // business rather than the address's. See the crate documentation
    // (spec: LA-1).
```

No behaviour changes.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The loopback bind site cites `LA-1`, verified by
      `grep -q "spec: LA-1" backend/crates/apps/coffret-server/src/main.rs`.
