---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && grep -rqi 'redact' backend/crates/apps/coffret-server/src && grep -rqi 'sentinel' backend/crates/apps/coffret-server/tests"
assignee: null
branch: task/0903-0800-keep-entry-paths-out-of-the-server-log
created_at: 2026-09-03T08:00:00Z
updated_at: 2026-09-03T13:30:00Z
---

# fix(server): keep Entry Paths out of the server log

## Overview

The observability design draws a hard line: Entry Paths and local
filenames are what the Library encrypts its names to protect, and they
never belong in a log line — a log file that accumulates them is an
unencrypted copy of the very structure the format hides from Storage.
The server crosses that line today through one seam: `ApiError`'s
response recording renders the whole cause chain with `Display`, and
several error variants along the fetch/upload paths carry an Entry
Path (or, since the placement-confinement work, a blocked local
folder) in their `Display` text — unmaterializable paths, local path
collisions, missing or non-current Entries, unmapped paths. The
comment beside the recording claims the rendering carries no Entry
Path; the variants below it say otherwise. Human-facing refusal bodies
are allowed to name paths — the owner is the reader; the log is not.

Close the seam without blinding the log:

1. **Stop treating `Display` as log-safe.** The recording path renders
   a **redacted** view of the cause chain: each error in the chain
   contributes its type/kind identity and its log-safe facts, never
   free text that may embed a path. Decide the mechanism that fits the
   error taxonomy best — a trait the path-bearing variants implement
   (a redacted rendering beside `Display`), or an allowlist rendering
   in the recorder that knows the crossing types — and put the
   decision's reasoning where the recorder lives. What must survive in
   the log: which operation, which kind of failure, error identities
   down the chain, counts/sizes where they matter. What must not: any
   Entry Path, any local path, any name a user chose.
2. **Audit every logging site in the crossing crates, not just the
   recorder.** Trace each Entry-Path-bearing variant to wherever it is
   logged: the server's background runs (fill/sync/freeze/refresh
   warn/error lines), the retry logger in the usecase layer, and any
   `tracing` call whose fields interpolate an error. Provider response
   bodies and OAuth error bodies ride the same audit: they are
   third-party text and must not be interpolated raw into log lines
   (excerpt lengths and shapes are fine).
3. **A sentinel test that proves the absence.** Router tests plant a
   sentinel Entry Path (and a sentinel local folder name) into each
   failure that used to leak — an unmaterializable path, an unmapped
   path, a missing Entry — drive the request, and assert the captured
   logs never contain the sentinel while still containing the
   redacted identity (the log stays useful). The existing
   `CapturedLogs` harness and `assert_free_of` are the pattern.
4. **Keep the human-facing surfaces intact.** Refusal bodies, CLI
   findings, and explorer messages keep naming paths — that is their
   job (the owner reads them); nothing in this task changes a
   user-visible string. `make check` and the E2E journeys prove the
   visible behavior is untouched.

Fix the recorder's now-false comment as part of the change — it should
state the new invariant and where it is enforced.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The response recorder logs a redacted rendering of the cause
      chain — error identities and log-safe facts, no free-text
      `Display` — with the mechanism documented where it lives (the
      `redact` grep gate pins it).
- [x] Sentinel tests plant path-bearing failures on the routes that
      used to leak and assert the captured log is free of the sentinel
      path and folder name while still carrying the redacted identity
      (the `sentinel` grep gate pins them).
- [x] Background runs and the retry logger pass the same audit: no
      Entry Path, local path, or raw provider/OAuth body text reaches
      a log line from the crossing variants.
- [x] `make check` passes and no user-visible string changes — refusal
      bodies, findings, and explorer messages still name paths.

## Out of scope

- Log rotation, retention, and file permissions (already in place).
- The CLI's terminal output (stdout/stderr are the owner's screen, not
  the persistent log).
- Redesigning the error taxonomy — the redaction layers over it.
- The gateway's URL redaction (already present) beyond what the audit
  of interpolated bodies requires.
