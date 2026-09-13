---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "LA-9" docs/spec/loopback-access/README.md && ! grep -rq "pub struct Envelope" backend/crates/apps/coffret-server/src/ && grep -q "pub struct Allowance" backend/crates/apps/coffret-server/src/allowance.rs && grep -q "64 MiB" docs/spec/format/README.md && grep -q "256 MiB" docs/spec/format/README.md'
assignee: null
branch: task/0913-1739-let-the-register-state-the-bounds-the-code-already-keeps
created_at: 2026-09-13T17:39:09Z
updated_at: 2026-09-13T18:25:35Z
---

# docs(spec): let the register state the bounds the code already keeps

## Overview

Several of this repository's resource bounds exist only as Rust constants. The
register describes the shapes they bound — a Container's header, a control
object, one HTTP request — without ever saying how large any of them may be.
That is why a second implementation can disagree with the first about what is a
valid Container, and why a structure with no entry in the register has taken a
name the register spends on something else.

**This task states what the code already does. It adds no bound and changes no
refusal.** Every number below is enforced today and covered by a test; the
register simply never said so.

### 1. The server's three budgets have no entry in the register

`backend/crates/apps/coffret-server/src/envelope.rs` bounds one HTTP request
three ways — how many bytes a request may carry, how many one part of it may,
and how many parts there may be — and asks one further question, whether the
volume has room for what is still coming. The values are
`MOST_PER_REQUEST` (64 GiB), `MOST_PER_PART` (1 GiB) and `MOST_PARTS` (4096).

Its module documentation already reads as a specification, including the part
that says what this is *not*:

> The Library's storage layer is deliberately size-agnostic: a five-gigabyte
> scan belongs in a Pack exactly as a five-hundred-kilobyte page does — an Entry
> larger than a Pack's size target is a Pack of its own rather than a file
> refused (spec: PK-3) — and nothing in the format or the flows puts a number on
> a file. This is not that contract and does not weaken it. It is the *server's*
> own, about one HTTP request from one browser on this device — a boundary the
> Library does not have and does not want.

That distinction is exactly what a register entry has to carry, because without
it a reader meets a number and cannot tell whether coffret refuses large files.

**`docs/spec/loopback-access/` is where it belongs.** Its own header says it
covers "the fences every request passes before a route sees it", and these are
those fences. Add rules from `LA-9`. State each budget; state that passing one
stops the request rather than refusing a file; state that the room question is
asked of each part before that part is taken; and state the boundary the module
doc draws — that none of this is a limit on what a Library may hold.

Move what belongs in the register out of the module doc rather than leaving two
copies to drift; what stays beside the code is why these particular numbers,
which is an implementation note, not a rule.

### 2. That structure is called `Envelope`, and the register owns the word

`coffret-server`'s `Envelope` is one import away from `coffret-model`'s
`KeyEnvelope`, and a Key Envelope is a concept the register defines
(`docs/concepts/key-envelope/`). The two have nothing to do with each other: one
is a wrapped key, the other is what one HTTP request is taken within.

Rename the server's type to something the register does not already spend, and
give it the `spec:` citation its new rules earn. Both halves matter — a name
that stops colliding but still cites nothing leaves the next reader where this
one started. The same rule settled `translate` → `classify` in the Storage
gateways: **the register owns a word.**

### 3. `FM-2` and `FM-11` lay out fields whose ceilings they never state

`FM-2` gives the Container header field by field, including `meta section length
M`, and says it "is the length of the padded meta section exactly as it appears
in the object". It never says how large `M` may be. `Header::MAX_META_LEN` is
64 MiB (`backend/crates/domain/coffret-format/src/header.rs`), refused as the
header is parsed and before any caller buffers anything, with
`decode/adversarial_length_tests.rs` pinning both sides of the boundary.

`FM-11` does the same for a control object, whose ceiling depends on its kind
(`backend/crates/domain/coffret-format/src/control/ceiling.rs`): a Journal
record 256 MiB, an Index Snapshot 512 MiB, a Keyring 64 MiB.

State both, as sub-bullets of the rules that lay out the fields. A reader who
has the layout and not the ceiling has what they need to write a decoder that
accepts an object this one refuses — which is not hypothetical; it is why the
TypeScript decoder and the Rust decoder currently disagree.

Cite the ceilings from the code as well, so the constant and the rule can be
read against each other.

## Out of scope

- **The TypeScript decoder does not enforce the meta-section ceiling.**
  `frontend/packages/domain/format/src/containerHeader.ts` reads the length and
  checks nothing, so a Container the Rust decoder refuses is accepted there.
  That is the defect this task's rule makes statable, and fixing it belongs with
  the other implementation work — not here, where nothing but documentation and
  one rename changes.
- **Catch-up holds every decoded control object it fetched, without bound.**
  `CK-9` describes the replay and sets no limit on what is retained. Stating a
  retention rule here would put a rule in the register that the code does not
  keep; the rule and the bound belong in one change, and that change is not this
  one.
- **`PK-3`** already says an oversized Entry becomes a Pack of its own, so its
  size target needs nothing added. Read it before concluding otherwise.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `docs/spec/loopback-access/` carries rules from `LA-9` stating the three
      budgets one request is taken within, the room question asked before each
      part, and that passing a budget stops the request rather than refusing a
      file.
- [x] Those rules state that they bound one HTTP request on this device and not
      what a Library may hold, so a reader cannot take them for a limit on file
      size.
- [x] The server's budget type no longer takes the name `Envelope`, and its new
      name carries a `spec:` citation to the rules above.
- [x] `FM-2` states the ceiling on the meta section length — 64 MiB — as part of
      the rule that lays out the field.
- [x] `FM-11` states the per-kind ceilings on a control object: a Journal record
      256 MiB, an Index Snapshot 512 MiB, a Keyring 64 MiB.
- [x] The constants in `header.rs` and `control/ceiling.rs` cite the rules that
      now state them, and no behaviour changes: every existing test passes
      unmodified except where the rename touches it.

### Manual / on-hardware (verified by a human before merge)

- [ ] Nothing here is observable at runtime: the change is register text, doc
      comments, and one rename. No manual check is needed.
