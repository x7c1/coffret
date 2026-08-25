---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && (cd backend && RUSTDOCFLAGS=-Dwarnings cargo doc --workspace --no-deps --document-private-items) && { grep -rEn --exclude-dir=node_modules --exclude-dir=dist-types -e "-[0-9]+ to (CK|CP|EP|FM|KD|KL|MR|OC|PK|RV)-[0-9]+" backend/crates frontend/packages docs/concepts docs/spec; test $? -eq 1; } && { grep -rn --exclude-dir=node_modules --exclude-dir=dist-types -e "has never seen the object" -e "refused rather than written" -e "Entries under the prefix" backend/crates frontend/packages; test $? -eq 1; } && { grep -n -e "Snapshot takes the" -e "reconciles" docs/concepts/master-key/README.md docs/spec/commit-protocol/README.md; test $? -eq 1; } && grep -q "PK-17" docs/spec/pack-construction/README.md && grep -rq "PK-17" backend/crates/domain/coffret-usecase/src/freeze && grep -q "EP-11" docs/concepts/entry-path/README.md && grep -q "pending row" docs/concepts/index/README.md && grep -q "adopt" docs/concepts/index/README.md && grep -q "materializ" docs/concepts/index/README.md && grep -q "materializ" docs/concepts/library/README.md && grep -q "spool" docs/concepts/library/README.md && grep -q "object_ref" docs/concepts/storage/README.md && grep -q "rebase" docs/concepts/journal/README.md && grep -q "snapshot slot" docs/concepts/index-snapshot/README.md && grep -qF "**key status**" docs/concepts/keyring/README.md'
assignee: null
branch: task/0825-1847-doc-follow-ups-30-to-37
created_at: 2026-08-25T09:47:11Z
updated_at: 2026-08-25T11:44:32Z
---

# docs: close the concept, spec, and rustdoc follow-ups left by #30 to #37

## Overview

Recent merged pull requests (#30 the follow-ups to #25–#29, #32 the Keyring
payload, #33 the commit flow, #34 folder sync, #35 adopting landed commits,
#36 fetching folders into mapped folders, #37 `freeze`) each left a short
list of documentation items that their refine passes raised but that were
out of their scope: a concept document that never
registered a word the code leans on, a spec rule that states a condition
without its consequence, a rustdoc paragraph that a later rule made false.
None needs new design. This task closes all of them in one pull request so
that the register, the concept documents, and the code's own prose stop
disagreeing with each other.

**No behavior changes.** Code files are touched for doc comments and comments
only. No function body, signature, type, test expectation, or fixture moves.
The one apparent exception — the `ContainerWriter` contract — is a rewritten
rustdoc that describes what the code already does, not a change to what it
does. Every decision below is already made: apply it; do not reopen it.

**Every concept document you touch goes through the litmus test in
`docs/concepts/README.md`.** A rule stays in a concept document only if
changing it would change what the term means or what a user can rely on;
a procedure, verification, or parameter needed only to build the system
correctly belongs in the [specification register](../../spec/), and the
concept document keeps at most a one-sentence summary citing the rule IDs as
plain text. Honour that file's other conventions too: one rule per bullet, at
most two sentences, caveats as sub-bullets, one why-clause on every
non-obvious rule, and no defensive negations. Nothing below introduces a rule
the register does not state.

### Concept docs (`docs/concepts/`)

#### `index/README.md`

- **Name the Snapshot as the other source of the cached Container facts.**
  The last Domain Rule (lines 53–59) closes with "All of it is a copy of what
  the Journal record that added the Container carried", which holds only for a
  device that replayed records. A device that restored from an
  [Index Snapshot](../index-snapshot/) got the same facts from that Snapshot's
  own list of current Containers. Add that missing half-sentence and cite
  FM-16 beside the existing IDs.
- **One sentence that device state cannot be rebuilt from Storage.** The first
  Domain Rule says the Index "can be rebuilt exactly from Storage (spec:
  RV-5)", and its sub-bullet (lines 36–40) says this device's own state is
  never uploaded. Add, as a further sub-bullet there, that the same fact cuts
  the other way: because no Index Snapshot and no Journal record carries
  device state, device state cannot be rebuilt from Storage at all — which is
  why an interrupted run's **pending row** is the only surviving record of
  what this device did, and the only way to finish the bookkeeping of a commit
  whose Index refresh failed (spec: CK-7, OC-7). That is the reason OC-7
  exists, and it belongs here rather than only in the register.
- **Give "pending row" a documented home.** Define it in the same place, once:
  the device-local record of a Container this device encoded and perhaps
  uploaded before any commit, naming the batch it belongs to, the spool file
  holding its ciphertext, and where the object went if it went (spec: OC-2,
  OC-7). The code carries the term throughout the sync and `freeze` flows and
  no concept document defines it today.
- **One verb for the act, and "present" tied to it.** Line 37 says "which
  Entries it has actually placed on disk"; the register's verb for that act is
  **materialize** (EP-10, EP-11). Use it here. Add one clause saying that the
  device-state record naming such an Entry *present* names the same
  transition, so a reader who meets both words resolves them to one act. Do
  not introduce a third word for it, and do not touch the separate, correct
  use of "place" for what a fetch does to a local path (EP-11).
- **Collocations** (lines 21–25): keep `rebuild` and `refresh`, and add
  `catch up` (a stale Index to the Library's head), `adopt` (a checkpoint from
  an Index Snapshot), and `complete` (an interrupted run's bookkeeping from
  its pending row).

#### `library/README.md`

- **Register the words the sync flow is made of.** The Library is the concept
  the flow is scoped to, and three of its load-bearing words are absent here:
  **materialize** (an Entry becoming a file this device vouches for),
  **spool** (writing a Container's ciphertext to a local file before it is
  uploaded, which is what makes an interrupted run recoverable), and the flow
  itself. Add one Domain Rule, at most two sentences, naming the stages in
  order — settle, scan, spool, upload, commit — and stating that only the
  commit changes the current Library state, everything before it being
  device-local work that an interrupted run leaves behind for the next one to
  settle (spec: CP-1, OC-2, OC-7). Keep it a one-sentence-shaped summary: the
  stages' obligations are the register's, not this document's.
- **`placed` → `materialized`.** The deletion rule (lines 55–58) says "only if
  this device itself had placed it — uploaded or fetched it". EP-10 calls that
  materializing; use the register's word here and in any other sentence in
  this file describing the same act, so the concept documents and the register
  read as one vocabulary.
- **A `freeze` invocation's scope is the folders the request names.** Add one
  Domain Rule: an invocation selects among the files under the folders the
  request names, so an update-eligible file outside them is outside that
  invocation's scope rather than a file it silently passed over — the
  surfacing obligation covers the files the scan considered, and a run over
  the other folder, or over the library root, considers the rest (spec: PK-17,
  PK-14). Cite the new rule PK-17 added below; do not cite PK-1 for this.
- **Document the reserved temp-prefix trade-off.** A fetch writes its
  temporary file inside a mapped folder, which is also a folder a scan walks,
  so coffret reserves a local filename prefix for it and a scan passes over
  every local name carrying that prefix. State the cost as a sub-bullet of the
  scan rule: anything of the user's own carrying that prefix is not backed up —
  a file, or a folder and everything under it, since the scan stops at the
  name and never looks inside — which is the trade for a crash never inventing
  an Entry out of a partial fetch (spec: EP-11). This is a limit on what the
  user can rely on, which is why it belongs in the concept and not only in the
  register.
- **Collocations** (lines 36–43): add `materialize` (an Entry into a file in a
  mapped folder), `spool` (a Container's ciphertext to a local file before
  uploading it), and `fetch` (a folder's files back onto this device). Say in
  the `fetch` entry that it is the Library-side name for what the
  [Pack](../pack/) concept calls `open` — the same act seen from the two
  concepts, one folder's files arriving by fetching the distinct Packs that
  hold them — so the two lists stop looking like two operations.

#### `journal/README.md`

- **Collocations** (lines 53–59): add `prepare` (a batch's Containers before
  any commit) and `rebase` (a losing writer's batch onto the new head). Both
  are already in the document's own prose — the Mental Model's *preparing*
  stage, and the retry CP-4 describes — and neither is registered.

#### `index-snapshot/README.md`

- **Collocations** (lines 53–56): add `adopt` (a checkpoint into a device's
  Index). Adoption is what a restoring or catching-up device does with a
  Snapshot, and only the Index side of the pair names it.
- **Define "snapshot slot" as a term.** The Mental Model already says the
  checkpoint carries "the next commit slot, for its successor", and a Domain
  Rule already says "Every Journal record reserves one place for a Snapshot of
  its head". Name that place: the **snapshot slot** is the single place on
  Storage where the Snapshot of one head may be created, reserved by that
  head's own record, which is what makes two devices writing the checkpoint at
  once end with one Snapshot rather than two rivals (spec: CK-10, CK-11). Bold
  the term at its definition, as this document already bolds *activation
  Snapshot*. If a name-keyed Storage carries no token for it, that is FM-15's
  business and stays there.

#### `keyring/README.md`

- **Define "key status".** The Definition (line 6) says the Keyring "records
  the key status of every current Container" and nothing defines the term,
  though FM-17 and KL-7 both use it. Define it once in the Mental Model, in
  the *State* section, as a bolded term: a Container's **key status** is the
  one thing the committed mapping records for it — either the
  [Key Envelope](../key-envelope/) that opens it or the explicit key-lost
  marker — so a current Container is never merely absent from the mapping
  (spec: KL-7). Keep the existing key-lost-marker wording; this adds the name
  for the pair, it does not restate them.

#### `master-key/README.md`

- **One verb for the commit slot.** Lines 51–53 say the activation Index
  Snapshot "takes the current commit slot". The register and every other
  concept document say **consume**; say it here too. This is the last
  occurrence, so no other file needs the same edit.

#### `storage/README.md`

- **One sentence stating what `object_ref` is.** Add it as a Domain Rule, at
  most two sentences: `object_ref` is Storage's own identifier for an object,
  the same value whichever device reads it, carried in control state as a
  cache so a device can fetch without listing Storage first; it is never
  evidence of membership, because a listing re-derives it and only the control
  state says what is current (spec: FM-15, FM-16). FM-15 already carries the
  full rule including a reader's fallback — keep this to the summary the
  litmus test allows and cite it rather than restating it.

#### `entry-path/README.md`

- **Add the three Domain Rules the register states and this document does
  not.** It stops at EP-7 while EP-9, EP-10, and EP-11 are all rules about
  what an Entry Path means on a device. One bullet each, stated from the Entry
  Path's side:
  - A device's local root mappings only **translate** Entry Paths into local
    paths; they never assert that the Entries under a mapped subtree are on
    this device, which is what lets a device hold part of a Library without
    the rest looking deleted (spec: EP-9, EP-10).
  - A scan reports an Entry as deleted locally only where this device
    **materialized** it and the file is gone; an Entry it never materialized
    is outside its scope, so it is never reported as changed, never selected
    for `update` or `freeze`, and never used as the source of a replacement
    (spec: EP-10).
  - A fetch **places** an Entry only where this device can vouch for what is
    at the path — nothing there, or its own materialization record agreeing
    with the file on disk — and reports every Entry it declines with the
    reason, because overwriting a file the Library never held would destroy
    content the Library never had a copy of (spec: EP-11, EP-4).
- Add one clause pointing at [Library](../library/) for the same ground stated
  from the Library's side, so the two documents read as one rule seen twice
  rather than two rules. Do not copy the Library document's sentences verbatim.
- **Collocations** (lines 18–22): add `translate` (an Entry Path into a local
  path through this device's mappings) and `place` (an Entry at its local path
  during a fetch).

### Spec register (`docs/spec/`)

- **PK-17 — a `freeze` invocation's folder scope.** `docs/spec/pack-construction/README.md`
  states which files are eligible (PK-1) and what a scan may not keep silent
  about (PK-14), but nothing states that an invocation's scope is the folders
  its request names. Add it as a new rule **PK-17** at the end of the list,
  marked *(Form: test)*: one `freeze` invocation considers the files under the
  folders the request names; an update-eligible file outside them is outside
  the invocation's scope rather than one it passed over, and PK-14's surfacing
  obligation covers exactly the files the scan considered. Add the
  corresponding cross-reference to PK-14 (a sub-bullet is enough) so the two
  rules are readable together. Keep every existing rule ID stable; PK-17 is
  the only new ID this task introduces.
- **OC-6 — a word for a removal that was requested and not completed.**
  `docs/spec/orphan-cleanup/README.md` OC-6 says removals "recorded by a
  committed Journal record but not yet physically deleted may be completed on
  recovery" and gives that state no name, though the commit flow reports the
  set (`CommitOutcome::untrashed`,
  `backend/crates/domain/coffret-usecase/src/commit/commit_outcome.rs`). Name
  it in OC-6: such a Container is an **untrashed removal** — a Container the
  committed record took out of the current set whose object no device has yet
  moved to the provider's trash. Add a sub-bullet distinguishing it from a
  suspected orphan: its removal is proven by the record, so OC-1's and OC-4's
  no-delete posture does not apply to it and any later run may complete the
  trashing, which is why completion is idempotent (CP-14).
- **EP-10 — state both consequences, not only the condition.**
  `docs/spec/entry-path/README.md` EP-10 already lists three consequences for
  an Entry the device never materialized ("never reported as modified, never
  selected for `update` or `freeze`, and never proposed for removal"). What is
  missing is that the two that matter are separate obligations and that the
  second is what `freeze`'s select step rests on: make explicit (a) that such
  an Entry is not reported deleted, and (b) that it is not used as the source
  of a replacement, since read-modify-replace would otherwise carry forward
  bytes this device never held. If the rule as edited already carries both,
  add nothing further — do not restate the list.
- **CP-4 — one word for the rebase.** `docs/spec/commit-protocol/README.md`
  CP-4 says a losing writer "refreshes the head, reconciles, and retries",
  while EP-7 calls the same act rebasing onto the new head. Say **rebase** in
  CP-4 as well, so the register has one word for it. "Reconcile" is dropped
  from the register entirely, for the reason in the next item.

### Disambiguating "reconcile"

"Reconcile" currently names two unrelated acts: the commit rebase CP-4
describes, and the settling of what an interrupted run left behind (OC-7).
Resolve it at the documentation level, in exactly this way:

- The commit-side act is **rebase** — CP-4 above, EP-7 already, and the
  Journal concept's new collocation.
- The interrupted-run act is **settle**, which is the word the sync flow's own
  prose already uses (`backend/crates/domain/coffret-usecase/src/sync/mod.rs`,
  step 1) and which OC-7 already describes as disposal or completion. Its
  landed sub-case is **complete** (the Index concept's new collocation).
- Renaming code identifiers is out of scope, so the documentation names the
  collision instead of hiding it: in the doc comment of `reconcile`
  (`backend/crates/domain/coffret-usecase/src/sync/reconcile.rs:68`) and of
  `Reconciled`
  (`backend/crates/domain/coffret-usecase/src/sync/reconciled.rs:18`), state
  in one sentence that these identifiers name the *settle* act of OC-7 and not
  the CP-4 rebase, and that the names predate the split. Do not add the note
  anywhere else, and do not touch `commit/settle.rs`, whose "settle" is the
  post-commit trashing and checkpointing and is not part of this collision.

### Code doc comments

All of the following are doc-comment edits. Write them in English, keep the
citation style of the surrounding prose (`spec: X` in Rust, bare IDs in
TypeScript, as each file already does).

- **`ContainerSummary::object_ref` is stale.**
  `backend/crates/domain/coffret-model/src/container_summary.rs:27-33` says a
  device that replayed a Journal record "has never seen the object, so it
  holds `None`". FM-15 makes the record itself carry `object_ref`, so a
  replaying device holds whatever the record carried and `None` means only
  that no one recorded a reference — a name-keyed Storage, or a writer that
  had none. Rewrite the field doc to match FM-15: the value is Storage's own
  identifier for the object, the same for every device, cached so a fetch
  needs no listing; it is never evidence of membership, and a device that
  cannot open the object it names falls back to the listing. Cite FM-15 and
  FM-16. Apply the identical correction to the TypeScript twin
  `frontend/packages/domain/format/src/model/containerSummary.ts:24-32`, which
  carries the same stale claim — the two sides must not disagree about a field
  they both decode.
- **`FreezeOutcome::packed_already`.**
  `backend/crates/domain/coffret-usecase/src/freeze/freeze_outcome.rs:31-32`
  says "How many Entries under the prefix a Pack already holds and the local
  file still matches". The count is not over the Entries under the prefix: the
  scan increments it per local file it considered
  (`freeze/scan/mod.rs`, `freeze/scan/examine.rs`) — that is, within this
  device's scope (EP-10) and under the folder the request named (PK-17). Say
  that: how many of the local files this run considered were already held by a
  Pack whose Entry the file still matches. Keep the existing "nothing to do,
  and nothing wrong" paragraph.
- **`freeze` cites PK-1 for a meaning PK-1 does not carry.** Two places claim
  folder scope on PK-1's authority:
  `backend/crates/domain/coffret-usecase/src/freeze/run.rs:23` ("Packs the
  eligible local files under one folder into Packs (spec: PK-1)") and
  `backend/crates/domain/coffret-usecase/src/freeze/scan/mod.rs:32-37` ("A
  freeze selects the eligible files under the folder one invocation names
  (spec: PK-1)"). Repoint both at PK-17, the rule that now states it. Leave
  every other PK-1 citation in those files alone: PK-1 is still the
  eligibility rule and is cited correctly for that.
- **`ContainerWriter`'s contract is wrong about failure.**
  `backend/crates/domain/coffret-format/src/container_writer.rs` claims at
  lines 30–35 that "a Container whose content is not what its table promises
  is refused rather than written", and `finish` repeats it at lines 178–180
  ("A Container short of what its table plans for is refused here"). Neither
  is true of the bytes: `write` and `finish` append sealed chunks to the
  caller's `out` as they go and return an error without rolling any of it
  back, so a caller that ignored the error would hold a prefix of a Container.
  Rewrite the contract to state what the code actually guarantees, at the type
  level and again in `finish`'s own doc:
  - On any error from `begin`, `write`, or `finish`, the writer and every byte
    it has appended to `out` must be discarded by the caller. The writer does
    not undo what it already appended.
  - What is guaranteed is that such a Container is never *completed*: the
    final chunk carries the final-chunk nonce domain (spec: FM-7) and only a
    successful `finish` produces it, so a mismatched or short stream can never
    be decoded as a Container — it can only be an unfinished prefix on the
    way to nowhere.
  - Keep the checks themselves described where they are (the per-Entry length
    and hash in `close_filled_entries`, the missing-Entry check in `finish`),
    and keep the doctest as it is. This is a public API of the format crate,
    so the wording is the contract: state the caller's obligation in the
    imperative and do not soften it. Behavior does not change in this task.
- **Expand every rule-ID range citation to explicit IDs.** Code-side prose
  still cites ranges, so a grep for one rule ID misses references to it.
  Replace each with the comma-separated IDs it covers (`CK-1 to CK-3` →
  `CK-1, CK-2, CK-3`; `FM-1 to FM-9` → all nine; `PK-9 to PK-12` → all four;
  `FM-15 to FM-17` → all three). The verbosity is the accepted price of
  grep-ability, which is what the register's plain-text citation convention
  rests on (`docs/concepts/README.md`). Every occurrence, all of them
  comments:
  - `backend/crates/domain/coffret-format/src/container_writer/tests.rs:89`
  - `backend/crates/domain/coffret-format/src/control/index_snapshot/mod.rs:4`
  - `backend/crates/domain/coffret-format/src/control/index_snapshot/round_trip_tests.rs:10`
  - `backend/crates/domain/coffret-model/src/index_checkpoint.rs:9`
  - `backend/crates/domain/coffret-model/src/snapshot_content.rs:26`
  - `backend/crates/domain/coffret-usecase/Cargo.toml:21`
  - `backend/crates/domain/coffret-usecase/src/fetch/mod.rs:45`
  - `backend/crates/domain/coffret-usecase/src/freeze/mod.rs:45` and `:72`
  - `backend/crates/domain/coffret-usecase/src/freeze_conformance/import.rs:32`
  - `backend/crates/domain/coffret-usecase/src/sync/mod.rs:46`
  - `backend/crates/domain/coffret-usecase/src/sync/run.rs:21`
  - `backend/crates/domain/coffret-usecase/src/sync_conformance/import.rs:17`
  - `frontend/packages/domain/format/src/control/indexSnapshot.ts:5`
  - `frontend/packages/domain/format/src/control/indexSnapshot.test.ts:47`
  - `frontend/packages/domain/format/src/model/indexCheckpoint.ts:52`
  - `frontend/packages/domain/format/src/model/snapshotContent.ts:16`

  Do not touch
  `backend/crates/domain/coffret-usecase/src/commit_conformance/refusals.rs:16`
  ("an orphan for OC-2 to reason about") — it is a citation followed by an
  infinitive, not a range. `frontend/packages/domain/format/dist-types/` is
  generated output; edit only `src/`.

Documentation, comments, commit message, and pull-request description are in
English. No rule ID other than PK-17 is added, and no existing ID changes.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes, and `cargo doc --workspace --no-deps
      --document-private-items` passes under `RUSTDOCFLAGS=-Dwarnings`.
- [x] No rule-ID range citation (`CK-1 to CK-3`, `FM-1 to FM-9`, and alike)
      remains under `backend/crates/`, `frontend/packages/` (generated
      `dist-types/` excluded), `docs/concepts/`, or `docs/spec/`
      (grep-gated), and
      `commit_conformance/refusals.rs` is unchanged.
- [x] None of the three replaced rustdoc claims survives anywhere in
      `backend/crates/` or `frontend/packages/`: "has never seen the object"
      (`ContainerSummary::object_ref` and its TypeScript twin), "refused
      rather than written" (`ContainerWriter`), "Entries under the prefix"
      (`FreezeOutcome::packed_already`) — each replaced by the contract stated
      above (grep-gated).
- [x] `docs/concepts/master-key/README.md` no longer says the activation
      Snapshot "takes" the commit slot, and `docs/spec/commit-protocol/README.md`
      no longer says a losing writer "reconciles" (grep-gated).
- [x] PK-17 exists in `docs/spec/pack-construction/README.md` and is what
      `backend/crates/domain/coffret-usecase/src/freeze/run.rs` and
      `freeze/scan/mod.rs` cite for a `freeze` invocation's folder scope
      (grep-gated).
- [x] `docs/concepts/entry-path/README.md` carries Domain Rules for EP-9,
      EP-10, and EP-11, plus the `translate` and `place` collocations
      (grep-gated on `EP-11`).
- [x] `docs/concepts/index/README.md` defines "pending row", uses
      "materialized" for the act EP-10 names, and lists `catch up`, `adopt`,
      and `complete` among its collocations (grep-gated on `pending row`,
      `materializ`, `adopt`).
- [x] `docs/concepts/library/README.md` registers `materialize`, `spool`, and
      `fetch`, and names the sync flow's stages in order (grep-gated on
      `materializ` and `spool`).
- [x] `docs/concepts/storage/README.md` states what `object_ref` is
      (grep-gated), `docs/concepts/journal/README.md` lists the `rebase`
      collocation (grep-gated), `docs/concepts/index-snapshot/README.md`
      defines "snapshot slot" (grep-gated), and
      `docs/concepts/keyring/README.md` defines **key status** as a bolded
      term (grep-gated).
- [x] OC-6 names the untrashed-removal state, and EP-10 states both
      consequences — not reported deleted, and not used as the source of a
      replacement.

## Out of scope

- The `freeze` partial-spool reclamation behavior fix — a behavior change
  handled on its own, not by this documentation pass.
- Renaming `Deferred` to `Surfaced` in the sync flow, and the wider
  defer-versus-surface vocabulary question — handled separately.
- Renaming `KeyringEntry`, and any other code-identifier rename. The
  "reconcile" disambiguation above is deliberately a documentation-only
  resolution for this reason.
- Restructuring any error type, adding variants, or moving values into them.
- Repack and compaction, deletion propagation, and `prune` — none of them has
  a follow-up in this list.
