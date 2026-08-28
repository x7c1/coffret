---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, error-type-design, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && (cd backend && RUSTDOCFLAGS=-Dwarnings cargo doc --workspace --no-deps --document-private-items) && ! git grep -qF 'EntryPath::new' -- backend frontend && grep -q 'fn nfc' backend/crates/domain/coffret-model/src/entry_path.rs && grep -q 'fn stored' backend/crates/domain/coffret-model/src/entry_path.rs && grep -q 'unicode-normalization' backend/crates/domain/coffret-model/Cargo.toml && grep -q 'refuses' docs/spec/entry-path/README.md && grep -rq \"normalize('NFC')\" frontend/packages/domain/format/src"
assignee: null
branch: task/0829-0450-make-nfc-an-entry-path-invariant
created_at: 2026-08-29T04:50:00Z
updated_at: 2026-08-29T06:30:00Z
---

# feat(backend): make NFC a type invariant of EntryPath

## Overview

EP-1 says every Entry Path is NFC; the scan boundary now normalizes (the
`nfc()` helper in `coffret-usecase`), but nothing stops a future construction
site from skipping it — the guarantee is a convention, not an invariant. This
task makes "an `EntryPath` exists ⇒ it is NFC" hold at the type level, and
makes readers of stored data refuse what violates it.

**1. Split the constructor (`coffret-model/src/entry_path.rs`).** Remove the
unconditional `EntryPath::new` and replace it with two constructors that force
every call site to say which side of the boundary its text comes from:

- `EntryPath::nfc(text: impl Into<String>) -> Self` — for text from outside
  the Library (scanned names, mapping prefixes, request prefixes, literals in
  tests and doc examples): normalizes to NFC unconditionally. Idempotent for
  text that is already NFC.
- `EntryPath::stored(text: impl Into<String>) -> Result<Self, Error>` — for
  text the Library already holds (stored-payload and device-catalog decodes):
  validates the text is NFC and refuses otherwise with a typed
  `coffret_model::Error` variant (name it for the finding — the stored path is
  not in the canonical form EP-1 requires; the error value carries the
  offending path, which is fine in a value, and per `coffret-logging` rules it
  must never reach a log field). Validation must not silently rewrite: a
  non-NFC stored path is malformed data, and the reader's job is to say so
  (the same posture KD-10 takes for an unreadable token cache).

Move the `unicode-normalization` dependency into `coffret-model` (it provides
both `nfc()` and the `is_nfc` check); `coffret-usecase`'s `nfc.rs` helper and
its own copy of the dependency then become redundant — delete the helper and
have the scan/mapping/translate sites construct through `EntryPath::nfc`
directly (normalizing the joined string is equivalent to per-component
normalization, as the scanned-name NFC task already established). The `EntryPath` type
doc drops its "the type carries the path verbatim" framing and states the
invariant instead.

**2. Migrate every construction site.** `EntryPath::new` must be gone from
the whole workspace (check gate spans `backend` and `frontend`):

- Outside-text sites → `nfc(...)`: `local_scan/walk_mappings.rs`,
  `fetch/translate.rs`, `FetchRequest::under` / `FreezeRequest::under` request
  prefixes (these were left unnormalized — the constructor now
  fixes them for free), all test fixtures and doc examples (ASCII literals are
  unchanged by NFC).
- Stored-decode sites → `stored(...)?`: `coffret-format`'s
  `meta/wire_entry.rs` and `meta/wire_derived_from.rs` (a non-NFC path in a
  decoded Container meta or `derived_from` is a malformed payload — wire it
  into the format layer's existing malformed-data error vocabulary, house
  style, with a rejection test each in the existing rejection-test style);
  `coffret-sqlite-index`'s `rows.rs` path columns (a non-NFC row is an
  unreadable catalog — map to the `UnreadableCatalog`-family error the module
  already uses for a stored text no reader knows; the Index file's
  discard-and-rebuild posture already covers recovery, and no migration is
  needed per the standing decision);
  `coffret-interop`'s generators keep producing NFC fixtures (their literals
  are ASCII — migrate the constructor calls, nothing else changes).

**3. Simplify the dual-prefix carry once the invariant holds.** The
scanned-name NFC task made
`WalkedRoot` carry a normalized `prefix` beside the recorded `mapping` because
a recorded mapping could be non-NFC. With the invariant, every
`Mapping.prefix` that decodes successfully IS NFC, so the two spellings are
one: collapse the dual carry (and the duplicated `prefixes` normalization
blocks in `walk_mappings.rs` / `fetch/translate.rs`, which duplicate each
other) to whatever minimal shape remains honest. Re-stamping under
"the recorded key" and reporting "the walk's spelling" become the same thing;
say so where the code used to explain the difference.

**4. TypeScript parity (`@coffret/format`).** The second implementation's
decode of entry paths (Container meta, `derived_from`, and any control-payload
path fields) adds the same NFC validation: refuse a decoded path `p` where
`p !== p.normalize('NFC')`, using the format package's existing error
vocabulary and rejection-test style. If the interop harness supports rejection
fixtures, add one non-NFC-path fixture that both implementations must refuse;
if it only supports accept-fixtures, per-implementation rejection tests
suffice — survey the harness and pick accordingly, recording which in the PR.

**5. Register the boundary rule (`docs/spec/entry-path/README.md`).** EP-1
gains one sub-bullet stating the rule this task enforces: text from outside
the Library normalizes on the way in; a stored path that is not NFC is
malformed and a reader refuses it rather than normalizing it. This also
resolves the EP-9 tension that task left open (a non-NFC mapping prefix can
no longer be recorded, because it cannot exist as an `EntryPath`); if EP-9's
wording needs a clarifying sub-bullet to say the top-level-component key is
the NFC spelling, add it in the register's own style. No concept-document
changes (the register is the home; concepts cite EP rule IDs as plain text
already).

Conventions per `CLAUDE.md`: no `PartialEq` on error types; a test per
variant a caller matches on; `make check` as the gate; English throughout;
self-contained commit and PR text.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `EntryPath::new` no longer exists anywhere in the workspace (check
      gate), replaced by `EntryPath::nfc` (normalizing) and
      `EntryPath::stored` (validating, fallible) with unit tests covering:
      `nfc` composes a decomposed input and is identity on NFC input;
      `stored` accepts NFC, refuses a decomposed input with the new error
      variant, and the error value carries the offending path.
- [x] The stored-decode boundaries refuse non-NFC paths: `coffret-format`
      rejection tests cover a Container meta path and a `derived_from` path in
      NFD, and the SQLite adapter maps a non-NFC path column to its
      unreadable-catalog error family with a conformance/unit test — while all
      existing accept-path tests and the interop fixtures pass byte-identical
      (`make check`'s interop job unmodified).
- [x] `@coffret/format` refuses the same shapes (check gate:
      `normalize('NFC')` used under the package's `src/`), with rejection
      tests in the package's existing style.
- [x] The `coffret-usecase` `nfc.rs` helper and its `unicode-normalization`
      dependency are gone (the dependency now lives in `coffret-model` —
      check gate on the model's Cargo.toml), the dual-prefix carry in
      `WalkedRoot` is collapsed, and the conformance case
      `an_nfd_local_name_becomes_an_nfc_entry_path` still passes unchanged in
      meaning.
- [x] EP-1 carries the boundary sub-bullet (check gate: `refuses` appears in
      the entry-path register), and `make check` plus
      `RUSTDOCFLAGS=-Dwarnings cargo doc --workspace --no-deps
      --document-private-items` are clean.

## Out of scope

- **Migration of pre-invariant data** — decided unnecessary (test data); a
  device catalog holding an NFD row is refused and rebuilt, per the Index
  file's existing posture.
- **Case/width folding or any normalization beyond NFC** (EP-3).
- **Concept-document changes and the Japanese mirrors** — the register is the
  home for this rule.
- **The `claimed`→`represent` vocabulary sweep and `local_scan/walked.rs`'s
  one-type-per-mod split** — separate ledger items.
