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
branch: task/0928-0948-tell-the-explorer-about-the-keyring-repairs
created_at: 2026-09-28T09:48:54Z
updated_at: 2026-09-28T10:15:42Z
---

# fix: tell the explorer about the Keyring repairs its sync and freeze runs made

## Overview

KL-15 says a repair performed on the committed Keyring is never silent. The CLI says every repair a run made — on success through `report::repair_line` (`coffret-cli/src/report.rs`) and, since the failed-commit change, on the failure path too — but the server tells the explorer about none of them: a sync or freeze run the explorer started that put a replica back reports nothing of it on the work answer, whether the run committed or failed after repairing. A person who only uses the explorer never hears that the Library's Keyring was short and was mended.

1. **Carry the repairs as findings.** Give the device a finding for a Keyring repair (built from `KeyringRepair`, naming the generation and how many replica positions were rewritten, within EL-1 to EL-5), produced from a sync's and a freeze's `CommitOutcome.repairs` and from the repairs a `CommitFailure` carries. It needs no attention: the set is whole again, which is the news. The server maps it through `Finding::of` to a new wire reason, added to `finding-reasons.json`, `work.json` and `work.ts` with their golden fixtures, and puts it on the sync or freeze run's findings — on a run that stopped as well as one that finished.
2. **Show it.** The status bar shows it with the run's other findings, in the warn tone at most, never as a stop.
3. **One sentence for both shells.** Where the CLI's `repair_line` and the new finding would say the same thing, have them say it in the same words, so the repair reads alike at the terminal and in the explorer.

Guard: `redacted()` stays free of anything the person did not see before; the finding's sentence counts positions and names the generation, never a path or a name.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A sync that repaired the Keyring and committed reports a repair finding on the work answer, pinned in a server route case (`make check`)
- [x] A sync whose commit failed after a repair reports the repair finding on its stopped run (`make check`)
- [x] The work contract's golden fixture carries the new reason and `work.ts` reads it (`make check`)
- [x] The status bar shows the repair finding without turning the run stopped, pinned in `StatusBar.test.tsx` (`make check`)
- [x] `make check` passes
