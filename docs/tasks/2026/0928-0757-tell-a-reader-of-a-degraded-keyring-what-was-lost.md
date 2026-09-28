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
branch: task/0928-0757-tell-a-reader-of-a-degraded-keyring-what-was-lost
created_at: 2026-09-28T07:57:14Z
updated_at: 2026-09-28T08:28:56Z
---

# fix: tell a reader of a degraded Keyring what was lost, once, whichever flow read it

## Overview

KL-15 says replica loss is never silent and never overstated. A flow that reads the committed Keyring and finds it degraded has to say so exactly once per run, say it as loss only when loss is established, and say it to whoever ran the flow — not only to a log. Three gaps break that on the read side.

1. **A replica Storage did not hand over is reported as lost.** `DegradedKeyring` (`coffret-usecase/src/commit/keyring.rs`, around lines 483-503) counts every stepped-over position with one counter, and `read_committed` (around lines 604-652) feeds it both the positions whose loss is established — `Absent` (around line 622), unreadable, a kind not admitted, a digest mismatch — and the ones that failed to fetch (around line 638). The warn then says "degraded and awaits repair" after a transient fetch failure, which the next run may read without trouble. `examine` already tells `Found::Unfetchable` apart (around line 142). Count the two separately; keep today's sentence when any position's loss is established, and when only fetches failed say that Storage did not hand some replicas over and that whether the set is degraded is not established. The sentence stays within EL-1 to EL-5: counts, the generation and positions only.
2. **The run-once guard is disarmed before the step that was to speak.** `commit/run.rs` (around lines 91-95) calls `report.examined()` and then `keyring::examine(...).await?`. When `examine` fails before it reaches its own warn, nobody reports the degraded set. Disarm the guard only when `examine` returns `Ok` or `UnrepairedKeyring` — the one error on which `examine` has already warned (`keyring.rs`, around line 198), so disarming later on every error would warn twice there — and leave it armed on `KeyringUnreadable` and on any other early return, so its drop speaks. Add the captured-log case: `examine` failing with `KeyringUnreadable` on a degraded set warns exactly once.
3. **A run that reads without committing never tells its caller.** `FetchOutcome` (`coffret-usecase/src/fetch/fetch_outcome.rs`, around lines 16-67) has no field for a degraded Keyring; `fetch/run.rs` (around lines 119-127) and `fetch/entry_run.rs` (around lines 96-104) only let the guard write to the log. A device that only fetches — a reading-only screen — never learns the set is degraded until something else writes. The same holds for a freeze that did not get as far as its commit (`freeze/run.rs`, around line 109). Carry the degraded report on the fetch, Entry-fetch and freeze outcomes, and turn it into a finding the device reports (`coffret-device/src/findings.rs`, around lines 100-133) and the server maps (`coffret-server/src/finding.rs`, `Finding::of`), following how the existing findings that need no attention are carried. Use the same distinction as item 1 in the finding's sentence.

Decisions this task makes, so a reviewer can check them:

- The new finding does not need attention: reads go on (RV-2), and the next flow that commits repairs the set before it commits (KL-13 to KL-16). A run that only reports it exits as it would without it.
- It reaches the explorer as well as the CLI, because a person who only uses the explorer is exactly the reader KL-15 is about. It takes the finding path the explorer already renders; name its wire kind after the concept (the Keyring's degraded state, KL-5), and add it to the wire contract and its golden fixtures.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A read that only fails to fetch a replica does not warn "degraded", and one that finds a replica `Absent` does, pinned with the fault-injection store and a captured log (`make check`)
- [x] `examine` failing with `KeyringUnreadable` on a degraded set warns exactly once, pinned in the captured-log conformance cases (`make check`)
- [x] A fetch, an Entry fetch and an uncommitted freeze of a degraded Keyring report the finding in their outcomes, and the CLI's exit status for such a run is unchanged (`make check`)
- [x] The server maps the finding and the wire contract's golden fixtures include its kind (`make check`)
- [x] `make check` passes
