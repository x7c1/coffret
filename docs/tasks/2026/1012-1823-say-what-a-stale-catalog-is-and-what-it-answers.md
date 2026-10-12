---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 0
check_command: 'make check && grep -q "\*\*stale\*\*" docs/concepts/index/README.md && grep -q "\*\*stale\*\*" docs/concepts/library/README.md && ! grep -q "first sync or fetch catches" docs/concepts/library/README.md && grep -qiE "key table .*kept|kept .*key table" docs/spec/keyring-lifecycle/README.md'
assignee: null
branch: task/1012-1823-say-what-a-stale-catalog-is-and-what-it-answers-retry-1
created_at: 2026-10-12T09:23:47Z
updated_at: 2026-10-12T10:27:56Z
---

# docs(concepts): say what a stale catalog is, how it differs from one reported behind, and what an Entry fetch answers from it

## Overview

A server's Entry fetches no longer catch up before they read the catalog
(`backend/crates/apps/coffret-device/src/entry_fetches/mod.rs`,
`backend/crates/apps/coffret-server/src/refresh/when_stale.rs`). They answer
from the catalog as it stands. A server catches up at startup, when the
explorer asks (`POST /api/refresh`), and on reconnect. Between those, another
device's commit can leave the catalog **stale**: its last catch-up finished,
but the Library's head has moved on since. The code now calls this state
stale and keeps it apart from **behind**, which LA-12 and the Library concept
reserve for a catalog whose last catch-up did not finish. The concept docs
define behind but never stale. The Index concept uses "stale" only once, in a
Collocation ("catch up (a stale Index to the available committed head)").
Concept docs, spec, and code must name the same thing the same way, so this
task writes the missing definitions down. It changes no behavior.

What to write (in the English concept docs and spec under `docs/`):

- **Index concept** (`docs/concepts/index/README.md`): define a **stale** Index (the term in bold, as the doc marks the terms it defines).
  Its last catch-up finished, and the Library's head has moved past it since.
  Nothing on this device knows that until a catch-up or a read refused by
  Storage shows it. State the consequence in the Domain Rules. A Container
  another device took out of the current set stays readable while it is an
  untrashed removal (OC-6), and after trash through a handle this device
  already holds. While that lasts, a read from a stale Index answers what the
  Index lists, which may be an Entry's earlier version. Staleness surfaces as
  a refusal only once the object is out of reach: trashed with no handle here
  naming it, or gone for good. The reader who catches up (a server's refresh,
  or the one catch-up a served Entry fetch makes on such a refusal) sees the
  current version.
- **Library concept** (`docs/concepts/library/README.md`): add a sub-bullet
  to the LA-12 bullet (the one defining **behind**, around line 386) that names
  **stale** in bold beside it. Being
  reported caught up says only that the last catch-up reached the head when
  it finished. A catalog left stale by a later commit elsewhere is not
  reported behind. Also fix the join bullet (around line 278): "until its
  first sync or fetch catches it up" no longer names the right actors. A
  served Entry fetch does not catch up; the command line's `fetch`, a sync,
  a freeze, and a server's startup catch-up do.
- **Spec, KL-15** (`docs/spec/keyring-lifecycle/README.md`): add a
  sub-bullet saying what happens when a run opens its Containers under a key
  table its process kept from an earlier read under the same Keyring
  commitment. Such a run does not read the Keyring again, and it carries the
  finding that earlier read made. A finding that the set was short is kept
  only until this process next catches up or runs a write (sync, freeze,
  delete). Either one forgets it, so the next run reads the set again and
  reports only what that read finds. A repair keeps the commitment it had
  (KL-13, KL-14), so the commitment alone cannot tell a repaired set from the
  one the finding was about. A key table read from a whole set may be reused
  across a catch-up that leaves the commitment unchanged. This records what
  `FetchMemory` does (`backend/crates/domain/coffret-usecase/src/fetch/fetch_memory/`,
  `forget_degraded_key_table` and `forget_key_table`). It does not change the
  rule. Use the Keyring concept's terms: a run's finding is that the set is
  **short**, and a set from which loss has been established is **degraded**.

Keep each addition to what a reader needs to tell the two states apart and
predict the answer. The code's doc comments already carry the mechanism
(`FetchError::may_mean_catalog_stale`, `refresh/when_stale.rs`); cite spec
rule ids the way the surrounding text does. Do not change LA-12's or any DK
rule's substance. If the text above cannot be written without doing so,
return `needs_review` and say why.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `docs/concepts/index/README.md` defines a stale Index and states what a read from one answers while a removed Container is still within reach (grep gate in `check_command`)
- [x] `docs/concepts/library/README.md` tells stale apart from behind under LA-12, and its join bullet names what catches a joining device up (grep gate in `check_command`)
- [x] `docs/spec/keyring-lifecycle/README.md` KL-15 says that a run opening Containers under a kept key table carries the earlier read's finding (grep gate in `check_command`)
- [x] Spec citation and rule-id checks pass (`make check` runs `scripts/spec-citations.sh` and `scripts/spec-rule-ids.sh`)

### Before merge (verified outside the check command)

- [ ] The Japanese copies of the changed concept docs are regenerated from the PR head and pushed before merge, then re-pointed at the merge commit afterwards

