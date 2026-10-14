---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, error-type-design, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1015-0356-keep-entry-paths-and-local-paths-out-of-the-explorer-hosts-events
created_at: 2026-10-14T18:56:31Z
updated_at: 2026-10-14T19:20:18Z
---

# fix(explorer-host): keep Entry Paths and local paths out of the explorer host's events

## Overview

The explorer host (`backend/crates/apps/coffret-explorer-host`) writes three
diagnostic events that break EL-1. The desktop app sends the host's events to
its persistent log (`coffret-desktop/src/startup.rs` →
`coffret-shell/src/logging.rs`), so what they carry outlives the process.

- `forward/not_answering.rs` logs `error_chain(cause)` of a `reqwest::Error`
  at WARN. A `reqwest::Error`'s Display carries the request URL, and a
  request for `/api/file?path=…` puts an Entry Path in the URL. This has been
  reproduced: a request for `/api/file?path=AUDIT_PRIVATE_BOOK%2Fpage.jpg`
  sent to a closed loopback port answered `502`, and the captured WARN's
  `cause` held the same path.
- `forward/mod.rs` logs `%cause` when the server's answer breaks off midway.
  The cause is a `reqwest::Error` of the same kind and can carry the same URL.
  It has not been tried at runtime; treat it as the same defect.
- `forward/headers.rs` logs `key_file = %self.key_file.display()` twice, and
  `%cause` of the key read once. The key file's path is a local path and
  contains the device-local Library name.

Record only what EL-1 allows: a fixed reason, the error's kind (connect,
timeout, body, decode, status, …), the operation that failed, and the server
address the host forwards to (an address, not a file). Do not get there by
stripping the URL from a rendered string. Build the event from an allowlist of
fields, so that nothing from the source chain reaches it. Apply the same rule
to the key-read failure: say that the key could not be read and why in
EL-1-safe terms (missing, unreadable, malformed), not where the file is.

The `502` text the page receives is a person-facing response, which EL-1
permits to name what was refused. Keep it useful, but build it separately
from the event (EL-1: "A rendering written for a response is not reused for an
event"). Do not leak the key file's path or the source chain into it either;
the page has no use for either.

Look through the rest of `coffret-explorer-host` for other events built from
`reqwest::Error`, `std::io::Error` or paths, and fix them in this task. A
crate-wide check that no event carries such values is welcome if it stays
simple.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A test drives the real host router against a closed loopback port with
      a request whose query holds a sentinel Entry Path, and asserts the
      captured events contain neither the sentinel nor any part of the URL,
      while the answer is still a `502`
- [x] A test makes the server's answer break off midway on a request whose
      query holds a sentinel, and asserts no captured event contains it
- [x] A test points the host at a missing key file and at an unreadable one
      whose paths contain a sentinel Library name, and asserts no captured
      event contains the sentinel or the path
- [x] The events that remain still say what failed: a test asserts the fixed
      reason and the error kind are present
- [x] The existing `forward` tests pass unchanged in meaning
- [x] `make check` passes
