---
status: completed
pipeline_phase: null
plan: null
base_ref: feat/mapped-root-marker
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "^- \*\*EP-13\.\*\*" docs/spec/entry-path/README.md && grep -q "^- \*\*EP-14\.\*\*" docs/spec/entry-path/README.md && grep -q "^- \*\*OC-8\.\*\*" docs/spec/orphan-cleanup/README.md && grep -q "\.coffret/root" docs/spec/entry-path/README.md && ! grep -q "temporary file" docs/spec/entry-path/README.md && grep -qi "change-detection" docs/spec/entry-path/README.md && grep -qE "^- decline " docs/concepts/entry-path/README.md && grep -q "(spec: EP-13" docs/concepts/entry-path/README.md'
assignee: null
branch: task/0910-1206-state-the-mapped-root-marker-and-placement-rules
created_at: 2026-09-10T12:06:32Z
updated_at: 2026-09-10T12:34:21Z
---

# docs(spec): state the mapped-root marker and the placement rules around it

## Overview

A device maps local roots to a Library (EP-9 … EP-12 in
`docs/spec/entry-path/README.md`), and today the only thing tying a mapping
to the folder it names is the path. An unplugged disk, a mount point that
came back empty, or a folder swapped for another one of the same name all
look like the configured root. EP-12 protects deletion inference from that
with the filesystem identity a scan stamps; nothing protects **placement**: a
fetch, a drag-and-drop upload, or a sync write goes wherever the path
resolves. This change states the rules that close that gap, and lands four
already-decided wording changes in the same register. It changes
documentation only; the code follows in later changes on the same
integration branch, which is why every new rule is `Form: test` and stays in
the register until its test exists.

Write the rules below in the register's own voice (read `docs/spec/README.md`
first: bare IDs inside the register, `(spec: X-n)` outside it, IDs are never
renumbered or reused, so the new ones are EP-13, EP-14, and OC-8; each rule
ends with its Form tag; sub-bullets carry the reasoning). Do not invent
mechanics beyond what is stated here; where this overview is silent, say
nothing rather than guess.

### EP-13 — the mapped-root marker (`docs/spec/entry-path/README.md`, after EP-12)

- Recording a mapping (EP-9) also records an **identity for the root**: a
  marker file at `<mapped root>/.coffret/root` holding a random identifier,
  and the same identifier kept as the mapping's expected identity in the
  device's own state — device state like the mappings themselves, never
  uploaded (CK-7). The marker's content is the identifier as sixteen
  lowercase hexadecimal characters (eight random bytes), optionally followed
  by one newline, and nothing else; a device reads at most 64 bytes of it and
  treats anything longer or shaped otherwise as malformed. (Spell the
  identifier the way `LibraryId` and `ContainerId` are spelled; say so in a
  sub-bullet.)
- **Before placing anything** — a fetch (EP-11), an upload received from the
  browser, a sync write — the device opens the configured root as the user
  named it (it may pass through a symbolic link, as EP-8 allows), then
  descends `.coffret` and `root` from that open handle without following
  links, requires the first to be a directory and the second a regular file,
  reads and parses the marker, and compares it with the expected identity.
  Placement then proceeds from that same opened root handle, never from a
  re-resolved path. The device **refuses to place** — reporting the mapping
  and the reason, on the no-silent-selection posture EP-4 sets — when the
  root is missing; when `.coffret` or `root` is missing, is a symbolic link,
  or is not the required kind; when the marker is malformed or over the cap;
  when the identifier differs from the expected one; and when the mapping has
  no expected identity recorded at all (a mapping read back from a
  device-state file the device could not otherwise use, for instance). A
  folder fetch continues past a refused mapping and reports it; a single
  writer that was refused — an upload route, a write already in progress —
  fails that request as a whole.
- **Ordinary operation never creates or repairs the marker.** A scan, a
  fetch, an upload, and a sync neither create the root, nor `.coffret/`, nor
  `root`, nor rewrite a marker, nor change the expected identity. Only
  recording the mapping does, and it does so conservatively: where `.coffret/`
  is absent it creates the directory and the marker with a fresh identifier
  and records it; where a valid marker already exists it **adopts** that
  identifier without rewriting the file, so several mappings or several
  devices sharing one root share one identity and none of them destroys
  another's; where the marker is malformed, over the cap, or a symbolic link,
  where `.coffret` exists without `root` (an interrupted registration), or
  where `.coffret` is not a directory, recording the mapping is an error and
  writes nothing. A person who wants a root to carry a new identifier — two
  roots that ended up with the same one after a copy — asks for it explicitly
  when recording the mapping; the register describes that as an explicit
  request to issue a new identity and leaves the command-line spelling to the
  tool.
- **What the rule guarantees and does not.** It certifies that the folder
  the device is about to write into is the one that was registered; it does
  not distinguish a faithful copy of that folder from the original, does not
  resist deliberate forgery, and does not vouch for what stands on a different
  mount below the root — a marker check at the root says nothing about a
  filesystem mounted further down. A root whose mount path changed is
  recorded again under its new path. Automatic volume discovery and the
  operating system's volume identifiers are not part of the rule. *(Form:
  test)*

### EP-14 — `.coffret` is a reserved name (same file, after EP-13)

- The name `.coffret`, as a path component at **any depth** under a mapped
  root, is reserved for the device's own management area. A scan decides from
  the name alone, exactly as it does for the `.coffret-fetch-` prefix in
  EP-11: it never enters a folder of that name and never reports anything
  under it as a file to back up; a fetch, an upload, and a sync never place a
  file at a path that has that component; an Entry Path carrying that
  component is refused for placement and reported. Where mappings overlap —
  one mapped root standing inside another — the management area of the inner
  root is not content of the outer one either. The cost, stated the way EP-11
  states the prefix's cost: anything of the user's own under a folder named
  `.coffret` is not backed up. *(Form: test)*
- For EP-12, a root holding nothing but its management directory still
  counts as **holding nothing**: the availability comparison looks past
  `.coffret/`, so the asymmetry EP-12 relies on (skip deletion inference, or
  re-stamp; never infer a deletion from an empty root on a foreign
  filesystem) is unchanged by the marker's presence. Add that as a sentence
  or sub-bullet to EP-12 itself, next to its "what the rule does not cover"
  paragraph.

### EP-11 amendments (same file)

- **Scratch.** EP-11 says "temporary file" three times; the word is
  **scratch**: a scratch is the file a fetch writes under the reserved
  prefix `.coffret-fetch-` before the rename that publishes it. Rewrite those
  sentences with the word and keep the prefix rule and its cost exactly as
  they are.
- **Agreement.** EP-11's "materialization record agreeing with the file on
  disk" is undefined. Define it in a sub-bullet: the record and the disk agree
  when the path, opened without following links, is a regular file whose byte
  length equals the recorded length and whose modification time equals the
  recorded one at second precision. That is a **change-detection condition**
  — it decides whether this device's own placement is still what stands
  there — and not content authentication: a file altered to the same length
  and time is not detected, and the rule does not claim otherwise.
- **Declined placement and the single writer.** State, in a sub-bullet, that
  a folder fetch continues past an Entry it declines and reports each one,
  while a single writer — the upload route the browser drops a file into, or
  a write already in progress — fails as a whole when its one placement is
  declined; and that a **place** is the noun for the local path a fetch
  resolves an Entry to, so "an unreachable place" and "placing an Entry"
  are one word seen twice.

### OC-8 — local clean-up is idempotent (`docs/spec/orphan-cleanup/README.md`, after OC-7)

- OC-6 is about the provider's trash. Local clean-up borrows its idempotence
  today without a rule of its own. State one: removing a spool file the
  device wrote, and removing a scratch a fetch never published, are
  idempotent — a file that is already gone is a successful removal, an
  interrupted clean-up is simply run again, and neither removal ever checks
  what stands at the path first, because absence is the outcome being sought.
  Cite EP-11 for what a scratch is and CP-14 for the same posture on the
  Storage side; do not widen OC-6. *(Form: test)*
- Code comments that cite OC-6 for local removals are corrected in a later
  change on the branch; do not edit code here.

### Entry Path concept (`docs/concepts/entry-path/README.md`)

- Add `decline` to Collocations, after `place`: a fetch **declines** to
  place an Entry and reports why. No new rule ID for it.
- Under the Domain Rule that mappings only translate (the bullet citing
  EP-9, EP-10), add a sub-bullet saying the second thing a mapping records is
  the root's identity, kept in a marker inside the root's management
  directory, checked before anything is placed and never created by ordinary
  operation, so a disk that came back empty or a folder swapped for another
  is refused rather than written into `(spec: EP-13, EP-14)`.
- Align the placement bullet's wording with the amended EP-11 (scratch,
  agreement, decline) without restating the spec; a concept document keeps
  meaning and guarantees, the register keeps mechanics.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The three new rules exist with their IDs and Form tags:
      `grep -q "^- \*\*EP-13\.\*\*" docs/spec/entry-path/README.md`,
      `grep -q "^- \*\*EP-14\.\*\*" docs/spec/entry-path/README.md`,
      `grep -q "^- \*\*OC-8\.\*\*" docs/spec/orphan-cleanup/README.md`.
- [x] The marker's location and the reserved name are stated in the register:
      `grep -q "\.coffret/root" docs/spec/entry-path/README.md`.
- [x] EP-11 says scratch and defines agreement:
      `! grep -q "temporary file" docs/spec/entry-path/README.md`,
      `grep -qi "change-detection" docs/spec/entry-path/README.md`.
- [x] The concept gains the collocation and the marker rule with its citation:
      `grep -qE "^- decline " docs/concepts/entry-path/README.md`,
      `grep -q "(spec: EP-13" docs/concepts/entry-path/README.md`.
- [x] Existing backend, frontend, and interoperability checks continue to pass.

## Out of scope

Any code change: the marker type, registration, the schema bump, placement
checks, scan exclusion, and their tests follow on the same branch. Correcting
the `(spec: OC-6)` citations in code comments about local removals. The
command-line spelling of the explicit new-identity request. Automatic volume
discovery or operating-system volume identifiers.
