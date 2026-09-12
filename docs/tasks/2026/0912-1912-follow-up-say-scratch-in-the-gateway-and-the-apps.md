---
status: completed
pipeline_phase: null
follow_up_of: docs/tasks/2026/0912-1710-say-scratch-in-the-gateway-and-the-apps.md
base_ref: feat/mapped-root-marker
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rqi "fetch into" backend/crates/gateway/coffret-local-fs/src/unix_destinations/descent.rs && grep -q "local writer" backend/crates/gateway/coffret-local-fs/src/unix_destinations/descent.rs'
assignee: null
branch: task/0912-1912-follow-up-say-scratch-in-the-gateway-and-the-apps
created_at: 2026-09-12T19:12:59Z
updated_at: 2026-09-12T19:33:50Z
---

# docs(backend): let the descent doc name the writer it actually serves

## Overview

`backend/crates/gateway/coffret-local-fs/src/unix_destinations/descent.rs`, lines 20 and 24. The doc explains why the descent makes
the folders below a mapped root but never the root itself, and it states the
rule for a fetch alone:

```
/// the answer about one folder (spec: EP-13). The root itself is *not* made — a
/// folder no registration ever visited carries no marker, so a fetch into one
/// refuses rather than creating a folder nobody recorded.
```

and four lines later:

```
/// whole of what a folder is (spec: EP-2): a device fetching
/// `albums/2026/spring.jpg` into an empty mapped root has to make both.
```

**More than a fetch reaches this descent.** `Destinations::reach` is called
through `LocalPlace::descend` from `coffret-usecase/src/fetch/placement.rs` and
from `coffret-device/src/add/receive_file.rs`, and an upload the browser drops
into an unregistered root refuses for exactly the reason this doc gives — the
root carries no marker, so nothing may be written under it.

The rule is not the fetch's, and the citation already on the sentence says so.
`EP-13` — the rule this paragraph cites, about the marker and the descent that
reads it — enumerates the writers itself: "Before placing anything — a fetch
(EP-11), an upload received from the browser, a sync write — the device opens
the configured root as the user named it". So the sentence should not name one
of three. `coffret-device/src/add/incoming_file.rs:84-85` already says the same
thing in its own words, that "a person dropping a folder is adding the folders
in it".

Say it for whichever writer arrives. Line 20's `a fetch into one` becomes a
local writer's descent into one; line 24's illustration names the writer
generically rather than a device *fetching*. The sibling module doc
`backend/crates/gateway/coffret-local-fs/src/unix_destinations/mod.rs:1` now reads "Where a local writer puts a file on this device",
so the vocabulary is already in place beside this file.

Leave the rest of the paragraph as it stands. The `EP-13` and `EP-2`
citations are correct, and so is everything the doc says about *why* the root
is not made and the folders below it are — only the writer is misnamed.

**No behaviour changes.** This is one module's doc comment; no code, no test,
and no value the build or the runtime reads.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes.
- [x] the descent's doc no longer scopes the rule to a fetch, and names the
  writer the register names:
  `! grep -rqi "fetch into" backend/crates/gateway/coffret-local-fs/src/unix_destinations/descent.rs`
  and `grep -q "local writer" backend/crates/gateway/coffret-local-fs/src/unix_destinations/descent.rs`.
  The absence gate is case-insensitive, as every prose gate in this series is:
  a sentence-initial capital has slipped past a case-sensitive one before, and
  a gate that reads as satisfied while the prose says otherwise is worse than
  no gate.
