---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! git grep -q -e locked_container -e "locked container" -- backend frontend docs/spec docs/concepts docs/guides && ! git grep -qF "\"locked\"" -- backend/crates/apps/coffret-server/src/api_error/declines.rs backend/crates/apps/coffret-server/src/finding.rs'
assignee: null
branch: task/1008-0351-name-a-key-lost-entry-key-lost-in-what-the-server-says
created_at: 2026-10-07T18:51:37Z
updated_at: 2026-10-07T19:26:13Z
---

# refactor(api): name a key-lost Entry or Container key_lost in what the server and the command line say

## Overview

A Container whose key the Library no longer holds is **key-lost**, and its
content stays unreadable (`docs/concepts/keyring/README.md`,
`docs/spec/recovery/README.md` RV-2, RV-7). "Locked" is a different state: a
running Library that does not hold its Master Key. Inside the code the
key-lost case is now named that way (`Finding::KeyLostContainer`,
`FetchOutcome::key_lost`), but what the server and the command line put out
still calls it "locked":

- The server's refusal and finding `reason` for a key-lost Entry or Container
  is `"locked"` (`coffret-server/src/api_error/declines.rs`,
  `coffret-server/src/finding.rs`, and the vocabulary its doc comments list),
  while the refusal *kind* `"locked"` (`error: "locked"`) means the Library is
  locked. A page reading both sees one word for two states.
- The command line's JSON kind is `"locked_container"`
  (`coffret-cli/src/answer/found.rs`, the list in `answer/mod.rs`), and the
  device's text line is `"locked container <id>"`
  (`coffret-device/src/finding.rs`, `findings.rs`).

Rename the key-lost reason to `"key_lost"`, the command-line kind to
`"key_lost_container"`, and the text line to `"key-lost container <id>"`. The
refusal kind `"locked"` for a locked Library, and the work answer's
`library: "locked"`, stay as they are — they mean the locked Library.

The server, the explorer and the command line ship together from this
repository, so there is no external reader to keep compatible. Update together:

- the server's reason strings and their doc comments (the vocabulary list in
  `finding.rs`, `declines.rs`, `api_error/mod.rs`, `api_error/contract.rs`);
- the API contract fixtures the frontend reads
  (`frontend/packages/gateway/api/src/contract/refusals.json` and any other
  contract file carrying the key-lost reason), and any TypeScript type or
  branch that names the reason;
- the command line's kind, its documented list, and tests that pin any of
  these strings;
- any spec or guide text that names the old strings.

Out of scope: the locked-Library refusal kind and state, and any change to
when a key-lost Entry is reported.

## Acceptance criteria

### Automated (pipeline-verified)
- [x] The server reports a key-lost Entry or Container with reason `key_lost`, covered by the existing route / contract tests updated to the new string
- [x] The command line's JSON kind is `key_lost_container` and its text line says "key-lost container", with tests
- [x] No `locked_container` or "locked container" remains in code or docs
- [x] The locked-Library refusal kind `locked` and `library: "locked"` are unchanged (existing tests still pass)
- [x] `make check` passes
