---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -q "the places this device writes a fetched Entry" backend/crates/domain/coffret-usecase/src/destinations.rs && ! grep -q "where a fetched Entry is placed" backend/crates/domain/coffret-usecase/src/lib.rs && ! grep -q "the one file it was handed" backend/crates/apps/coffret-device/src/add/receive_file.rs && ! grep -q "Which way a fetch was declined" backend/crates/apps/coffret-server/src/reported.rs && grep -q freeze backend/crates/apps/coffret-server/src/reported.rs && ! grep -q "through a fill and through a click on a file in it, and reading" backend/crates/apps/coffret-server/src/noted.rs && grep -qiE "unavailable|unresponsive|does not answer" backend/crates/apps/coffret-device/src/add/added_at.rs && ! grep -q "mapping recorded against that root and goes on" backend/crates/domain/coffret-usecase/src/fetch/placement.rs && ! grep -rqiwE "l[e]dger(ed)?|m[i]lestone" docs/tasks/2026/ && ! grep -q "that part of the Library" docs/concepts/library/README.md && grep -q "^- drop" docs/concepts/library/README.md && grep -qi "unlocked" docs/concepts/library/README.md'
assignee: null
branch: task/0914-1315-let-the-prose-say-what-the-code-now-does
created_at: 2026-09-14T13:15:23Z
updated_at: 2026-09-14T14:15:06Z
---

# docs: let the prose say what the code now does

## Overview

Several rounds of review left a list of places where a comment, a doc
comment or a concept document describes the code as it was rather than as
it is. None of them changes behaviour; all of them mislead a reader. This
task applies the one rule — the prose says what the code now does, in the
vocabulary the concept documents and the spec register use — across every
site found, in one change, so the correction is read as one correction.

### The `Destinations` capability has two callers, not one

`backend/crates/domain/coffret-usecase/src/destinations.rs` opens with
"Everything the flows ask of the places this device writes a fetched Entry
into", and `src/lib.rs` calls it "where a fetched Entry is placed". The
capability is also what the upload route writes through:
`backend/crates/apps/coffret-device/src/add/incoming_file.rs` holds a
`Box<dyn Destination>` and places a dropped file with it, and
`receive_file.rs` and the scratch rules (EP-11) already treat the drop as a
local writer alongside the fetch. Reword the capability's framing — the
trait doc, the module doc in `lib.rs`, and any sentence in the same crate or
in `backend/crates/gateway/coffret-local-fs` that says the capability is the
fetch's — so it names both writers, or names neither and speaks of "a local
writer" the way EP-11 does. Sentences about the fetch's own order of steps
(write, flush, hold, stamp, publish, mark present) stay the fetch's; what
changes is only who the capability is for.

### Four small disagreements between a sentence and its code

- `backend/crates/apps/coffret-device/src/add/added_at.rs`: the doc lists the
  reasons `added_at` answers `None`. It does not list a mapped root that is
  not there or does not answer (spec: EP-12) — check what the look-up does
  when `MappedRoots` reports the root unavailable, and either add that case
  to the list in the doc's own words or, if the code refuses rather than
  answers `None`, say so where the doc already lists the one refusal.
- `backend/crates/apps/coffret-device/src/add/receive_file.rs`, the
  `RootRefused` paragraph: it says "this device is placing the one file it
  was handed" and, in the same sentence, "a caller handed several at once, as
  one upload's files are". Say one thing: the request fails as a whole, and
  every file of one upload that this mapping reaches fails with it.
- `backend/crates/apps/coffret-server/src/reported.rs`: the `reason` field's
  doc says "Which way a fetch was declined" while the sibling field speaks of
  "something"; and the type doc says three flows keep a `Reported` — the
  fill, the sync and the upload's refused parts — while `Reported::recorded`
  has four callers (the freeze is missing). Make both true.
- `backend/crates/apps/coffret-server/src/noted.rs`: the paragraph on why
  one refusal sentence is shared says a person meets the folder "through a
  fill and through a click on a file in it"; a drop into that folder meets
  the same state through the upload route. Add the third way.

### One doc that overstates when a refusal is reported

`backend/crates/domain/coffret-usecase/src/fetch/placement.rs`, the doc of
`Opened`: "a folder fetch reports it once for each mapping recorded against
that root". A mapping under which no Entry of the fetched folder is placed
produces no refusal — read `fetch/scatter.rs` and the fetch flow to state
exactly which mappings a folder fetch reports, and say that.

### Committed task files that borrowed a planning vocabulary

Seven task files under `docs/tasks/2026/` use two nouns that name nothing
in this repository — a word for a running account book of open items, and a
word for a delivery stage — where they mean "a follow-up recorded elsewhere"
or "this change". The files:
`0828-1330-doc-follow-ups-38-to-45.md`,
`0828-1405-carry-error-causes-as-values.md`,
`0829-0145-carry-flattened-causes-in-store-errors.md`,
`0829-0450-make-nfc-an-entry-path-invariant.md`,
`0831-1157-browse-and-read-the-library-in-the-explorer.md`,
`0901-0050-drop-files-into-the-explorer.md`,
`0901-1104-run-the-explorer-end-to-end-against-minio.md`. Find the two
nouns with a case-insensitive whole-word grep over that directory and
replace each use with plain words that say the same thing ("a recorded
follow-up", "left for a later change", "after this change"); keep every
other word of those files as it is, including their front-matter and their
ticked criteria. Do not use either noun anywhere in this task's own text.

### The Library concept document

`docs/concepts/library/README.md`:

- In the EP-9 rule under Domain Rules, "a top-level mapping represents that
  part of the Library" becomes "that subtree of the Library" — the word EP-9
  itself uses (`docs/spec/entry-path/README.md`), and the word the server
  crate's comments now use for the same thing. Verify the register's wording
  before writing it.
- Add `drop` to the Collocations, in the list's own form: files a browser
  drops onto a mapped folder, for a flow — a sync or a freeze — to carry them
  in (spec: LA-9 to LA-11, EP-11). The noun is already the server's, the
  explorer's, and the register's; only the concept document lacks it.
- Add one Domain Rule under the rules about a served Library: a Library
  served on a device is `locked` or `unlocked` exactly as that device holds
  the Master Key (spec: DK-1), and a browser asking what the server is doing
  is told which — cite DK-4 — so that a page left open can give up what it
  decrypted. Keep it to two sentences, in the document's voice, and cite the
  Master Key concept's lock rule rather than repeating it.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The `Destinations` trait doc and the usecase crate's module doc no
      longer call the capability the fetch's; both callers are named or the
      sentence speaks of a local writer.
- [x] The four doc disagreements are gone: `receive_file.rs` no longer says
      "the one file it was handed"; `reported.rs` no longer says a fetch was
      declined and names the freeze among the flows; `noted.rs` names the
      drop as a way of meeting the folder; `added_at.rs` accounts for a root
      that is unavailable or does not answer.
- [x] The `Opened` doc in `fetch/placement.rs` no longer says a fetch
      reports once for each mapping recorded against the root.
- [x] No task file under `docs/tasks/2026/` uses either planning noun.
- [x] The Library concept says "subtree of the Library" in its EP-9 rule,
      lists `drop` in Collocations, and has a rule naming a served Library's
      `locked` / `unlocked` state.
- [x] `make check` passes (comments and docs only; `cargo doc -D warnings`
      still resolves every intra-doc link).
