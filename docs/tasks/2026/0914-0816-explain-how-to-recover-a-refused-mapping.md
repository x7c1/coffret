---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, concept-alignment, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/0914-0816-explain-how-to-recover-a-refused-mapping
created_at: 2026-09-14T08:16:04Z
updated_at: 2026-09-14T09:13:41Z
---

# fix(explorer): explain how to recover a refused mapping

## Overview

A refused mapped root currently ends with "record that mapping again with
`coffret map`". The explorer offers no mapping editor, and the sentence does
not tell the person that this is a command to run in a terminal on the device
serving the Library. A fill stopped by that same refusal also offers "bring
over again", although trying again cannot repair the mapping.

Make the recovery reachable from that explanation. Keep setup in the CLI:
tell the person where to run the command, how to inspect the recorded mappings
and find the command's arguments, and what to do in the explorer after the
folder is available again or the mapping has deliberately been recorded again.
Use the existing `coffret mappings` and `coffret map` interfaces as the authority
for the instructions. Do not interpolate a Library prefix or a local path into
an executable command. Preserve the distinction between reconnecting the
intended folder and deliberately adopting another one; do not present resetting
the marker as the routine answer to every refusal.

The shared `refused_root_said` in
`backend/crates/apps/coffret-server/src/api_error/mod.rs` is used by request
refusals and background findings in `noted.rs`. Keep their guidance consistent,
including the Library-root and named-prefix cases. Retain the mapping identity
in the user-facing sentence and keep local filesystem paths and credentials out
of API bodies and diagnostics. Correct adjacent recovery wording where the same
mapping action is offered, without changing the marker rules or adding a new
mapping-management API.

`frontend/packages/apps/web/src/StatusBar.tsx` currently offers a retry for
every stopped fill. Use the structured refusal reason, not the message text,
to suppress that offer for `refused_root`. Keep the explanation visible and
retain the existing retry for a stopped fill caused by a transient failure or
an older response with no structured reason. A later activity response must be
able to restore the ordinary retry offer; do not latch this state permanently.
Review the retry offers for the other background flows against the same rule
where their stopped response can carry that reason.

This change concerns recovery guidance and retry availability. Browser-based
mapping registration and idle-lock notification are separate behavior changes.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Server tests cover both the Library-root and named-prefix refusal:
      recovery is explicitly performed in a terminal on the serving device,
      identifies the existing mapping commands, preserves the mapping name,
      and does not disclose the local root or credentials. Background findings
      and request refusals use the same recovery wording where they name the
      same condition.
- [x] Rendering tests cover a stopped fill with `refused_root`: its explanation
      remains visible and no "bring over again" button is offered. Equivalent
      stopped sync/freeze refusals are handled consistently where supported.
- [x] Rendering tests keep retry available for an ordinary stopped failure and
      a stopped response without a reason; absent, running, done and superseded
      fills do not offer retry. Replacing a refused-root activity with an
      ordinary stopped activity restores the retry offer.
- [x] The revised recovery text and UI behavior are exercised by the server
      and frontend tests run by `make check`, including the existing privacy
      assertions and API refusal serialization checks.
