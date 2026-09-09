---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, concept-alignment, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/0908-1938-keep-private-locations-out-of-diagnostic-events
created_at: 2026-09-08T19:38:45Z
updated_at: "2026-09-09T12:48:20Z"
---

# fix: keep private locations out of diagnostic events

## Overview

S3 listing passes the configured live prefix to the error translator as an
object name. A not-found failure then writes it into a debug event and returns
it in a port error whose diagnostic rendering treats object names as opaque.
Separately, local staging cleanup and the file-response stream render raw I/O
errors, even though a custom I/O error can carry a private path in its message.

Use a fixed diagnostic subject for listing, as the bucket check already does.
Audit the listing success and failure paths and their cause chains, including
provider text that echoes the configured prefix. Keep useful operation, status,
reason, size, and error-kind evidence while preventing the configured prefix
from reaching coffret events. Do not disable logging or flatten structured
errors into generic strings to hide them. Ordinary opaque object identifiers
and appropriately credential-redacted provider diagnostics remain useful.

Route local cleanup and streaming failures through their log-safe rendering;
inspect related raw error events in touched areas for the same defect. A
person-facing refusal can still identify the file they own, but an event must
not contain Entry Paths, local paths, device-local Library names, plaintext,
keys, Passphrases, Recovery Codes, or tokens. Keep this distinction explicit.

Give this diagnostic contract one home in a new event-logging spec mechanism,
using an unused rule prefix and updating the spec index. Document the existing
Redacted grammar, error-kind/path-length summaries, permitted Storage evidence,
and the cause-chain boundary. Correct references that currently cite EP-1 as
though path normalization defined log privacy. Update relevant concepts,
including the distinction between the device-local Library name and Library
ID, and distinguish diagnostic events from the Journal's control-object log.
Do not assert that arbitrary provider bodies are always safe merely because
object identifiers are opaque.

The retry suite already tests a gave-up event's counters and provider message.
Assess and add the missing privacy regression rather than duplicating that
test. Correct nearby inaccurate claims about who mints opaque object names.
Keep unrelated large error-module restructuring out of this change.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A real S3 adapter exercised against a deterministic local HTTP fixture
      keeps a recognizable configured prefix out of captured listing events
      and the returned error's diagnostic rendering, including not-found and
      an echoed-prefix provider failure; useful operation/status evidence stays.
- [x] Captured cleanup and file-stream events cannot reveal a custom I/O
      error's embedded private path, while retaining a useful error category.
- [x] Regression coverage exercises nested diagnostic causes and device-local
      Library names without printing their forbidden values.
- [x] Retry give-up privacy coverage and existing provider diagnostic tests
      preserve permitted evidence and the established credential redaction.
- [x] Existing backend, frontend, and interoperability checks continue to pass.

## Out of scope

No external telemetry or new logging backend. No root-marker format, filesystem
reader redesign, provider grant changes, or general renaming of untouched
error modules. Preserve the distinction between local diagnostic events and
explicit user-facing output.
