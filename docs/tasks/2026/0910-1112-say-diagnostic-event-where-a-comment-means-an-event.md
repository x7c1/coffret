---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rqiE "log line" backend/crates/apps backend/crates/domain backend/crates/gateway --include="*.rs" && grep -qi "one JSON object per line" backend/crates/libs/coffret-logging/src/lib.rs && grep -q "no part of a local path may reach a diagnostic event" backend/crates/domain/coffret-usecase/src/local_io_error.rs && grep -q "no part of a local path may reach a diagnostic event" backend/crates/domain/coffret-usecase/src/descent_error.rs'
assignee: null
branch: task/0910-1112-say-diagnostic-event-where-a-comment-means-an-event
created_at: 2026-09-10T11:12:17Z
updated_at: 2026-09-10T11:42:14Z
---

# docs(backend): say "diagnostic event" where a comment means an event rather than a line

## Overview

The register names what coffret records about its own operation a
**diagnostic event** (`docs/spec/event-logging/README.md`, EL-1 … EL-5), and
the concept documents follow it (`docs/concepts/journal/README.md` separates
the Journal from "diagnostic events"; `docs/concepts/library/README.md` and
`docs/concepts/storage/README.md` cite EL-1 with that word). The backend's
comments say "log line" for the same thing in 60 places across 37 files under
`backend/crates/apps`, `backend/crates/domain`, and `backend/crates/gateway`
(`grep -rniE "log line" backend/crates --include='*.rs'` lists them; none is
an identifier). Every one of them means the event — what a local path may
never reach, what `Redacted` renders, what a variant lets an event say — and
none means a physical line of a file. Two of the 60 are assertion messages in
tests rather than comments (`backend/crates/domain/coffret-usecase/src/local_io_error.rs`
and `backend/crates/domain/coffret-usecase/src/descent_error.rs`, both
`"no part of a local path may reach a log line"`).

Rewrite each occurrence, one by one, to say what it means in the register's
word: `reach a log line` → `reach a diagnostic event`, `what a log line
renders` → `what a diagnostic event renders`, `the log line says` → `the
diagnostic event says`, `every log line along the way` → `every diagnostic
event along the way`, and so on. Where a sentence already has "event" nearby
and repeating it would read badly, `an event` alone is enough; where the
sentence is about the rendering the event is built from (for example
`backend/crates/gateway/google-drive-store/src/error.rs`, "the rendering a log
line is built from"), keep the sentence's structure and change only the noun.
Do not change a sentence's meaning, do not touch code, identifiers, or test
behaviour, and do not reflow paragraphs beyond what the changed words require.
The two assertion messages take the same replacement; they are messages, not
behaviour.

**`backend/crates/libs/coffret-logging` is not part of this change.** There,
"line" is the physical JSONL line — one JSON object per line, a half-written
line after a crash, the cut line the cap writes — and that meaning is correct
as it stands (`src/lib.rs`, `src/jsonl.rs`, `src/rotating_files/cap.rs`,
`src/testing/logged_event.rs`). The crate contains no "log line" today and
must contain none afterwards either, but its "line" wording stays.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] No comment, doc comment, or test message under the application, domain,
      or gateway crates says "log line" any more:
      `! grep -rqiE "log line" backend/crates/apps backend/crates/domain backend/crates/gateway --include="*.rs"`.
- [x] The two assertion messages say what they check in the register's word:
      `grep -q "no part of a local path may reach a diagnostic event" backend/crates/domain/coffret-usecase/src/local_io_error.rs`,
      `grep -q "no part of a local path may reach a diagnostic event" backend/crates/domain/coffret-usecase/src/descent_error.rs`.
- [x] `coffret-logging` keeps its physical-line wording:
      `grep -qi "one JSON object per line" backend/crates/libs/coffret-logging/src/lib.rs`.
- [x] Existing backend, frontend, and interoperability checks continue to pass;
      no test changes its assertion beyond the message text.

## Out of scope

Any change to what an event records or how `Redacted` renders. Any wording
change in `coffret-logging`. Renaming "message" or "record" anywhere. Editing
the concept documents or the register.
