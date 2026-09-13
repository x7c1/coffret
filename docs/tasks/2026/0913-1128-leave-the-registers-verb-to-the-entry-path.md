---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [error-type-design, completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rq "translate" backend/crates/gateway/s3-store/src && ! grep -rq "translate" backend/crates/gateway/coffret-sqlite-index/src && ! grep -rqi "translate" backend/crates/gateway/google-drive-store && ! grep -q "each gateway translates its provider" backend/crates/domain/coffret-usecase/src/lib.rs && ! grep -q "a gateway translates whatever its SDK" backend/crates/domain/coffret-usecase/src/error.rs && ! grep -q "a gateway translates its provider" backend/crates/domain/coffret-usecase/src/commit/commit_error.rs && grep -rq "fn translate" backend/crates/domain/coffret-usecase/src/fetch && grep -q "translate (an Entry Path into a local path" docs/concepts/entry-path/README.md && grep -rqE "fn [a-z_]+_conditional_create" backend/crates/gateway/s3-store/src && grep -rqE "fn [a-z_]+_listing" backend/crates/gateway/s3-store/src && grep -rqE "fn [a-z_]+_transport" backend/crates/gateway/s3-store/src && grep -rqE "fn [a-z_]+_object" backend/crates/gateway/s3-store/src'
assignee: null
branch: task/0913-1128-leave-the-registers-verb-to-the-entry-path
created_at: 2026-09-13T11:28:44Z
updated_at: 2026-09-13T11:56:43Z
---

# refactor(backend): leave the register's verb to the Entry Path it names

## Overview

`translate` is a defined term. `docs/concepts/entry-path/README.md:23` lists it
in the Entry Path's Collocations as "translate (an Entry Path into a local path
through this device's mappings)", and EP-10 in `docs/spec/entry-path/` states
the rule it names: "A device's mappings (EP-9) only translate Entry Paths into
local paths". `coffret-usecase/src/fetch/translate.rs` is that verb in code, and
it keeps the name.

Two Storage gateways borrowed the same word for something else — turning a
driver's error into this crate's error:

- `backend/crates/gateway/s3-store/src/error.rs` — `translate`,
  `translate_object`, `translate_listing`, `translate_conditional_create` and
  `translate_transport`, called from `check_bucket.rs` and `s3/mod.rs`, and
  named in rustdoc links in those files.
- `backend/crates/gateway/coffret-sqlite-index/src/error.rs` — `translate`,
  whose returned closure is applied across `device_state.rs`, `library_state.rs`,
  `query.rs`, `rows/columns.rs`, `schema.rs` and `sqlite_index.rs`.
- `backend/crates/gateway/google-drive-store/` — the `TranslateFailure` type
  alias in `src/upload.rs` and the `translate` parameter it names, plus a prose
  use in `src/error.rs`. The earlier survey of this family grepped for
  `fn translate`, which a type alias and a parameter name do not match, so this
  one was missed. It is the same rule in a third gateway, so it belongs here
  rather than in a task of its own.

Neither gateway holds an Entry Path, which makes the borrowed use look
harmless — but that reasoning looks only at each module's scope. The collision
is in the vocabulary the register defines, and the register is what a reader
reaches for when a word looks like a term. A reader who has learnt what `translate` means
meets it in the S3 gateway and has to discover it means something unrelated
there.

Rename the whole family in both gateways — not only the bare `translate`, since
every one of them borrows the verb. Pick a name that says what these functions
do to an error. Nothing about behaviour changes: same signatures, same
variants, same call sites, one word each.

Two constraints on the new name:

- It must not be another word the register defines. Check
  `docs/concepts/*/README.md` Collocations before settling on one.
- The five S3 functions should keep reading as one family, the way they do now
  — one stem with the same suffixes.

## What stays

**Every use of `translate` that means the register's verb** — turning an Entry
Path into a local path through this device's mappings. That is most of
`coffret-usecase` (`fetch/translate.rs` and its callers,
`device_state/mapping.rs`, `commit/`, `sync/`), all of `coffret-device`'s uses,
`coffret-server/src/routes/file.rs`, `coffret-local-fs`'s
`unix_destinations/descent.rs`, the frontend's `journeys/uploader.ts`, and
everything under `docs/`. The Entry Path concept's Collocations and EP-10 must
read exactly as they do now. A sweep that reaches any of these is the opposite
of this change.

**The distinction is the sense, not the crate.** An earlier draft of this
section named whole crates as out of scope, which was wrong: three doc comments
in `coffret-usecase` use the borrowed sense — they describe what a *gateway*
does to a *provider's error*, not what a mapping does to an Entry Path — and
they belong to this change. Read each site rather than trusting its address.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] No item in `s3-store`, `coffret-sqlite-index` or `google-drive-store` is
      named `translate` or `translate_*`, and the word does not survive in a doc
      comment, a rustdoc link or a prose sentence in any of the three crates.
- [x] The three doc comments that describe a gateway's act in the port crate
      (`coffret-usecase/src/lib.rs`, `src/error.rs`, `src/commit/commit_error.rs`)
      say it with the same word the gateways now use.
- [x] `coffret-usecase/src/fetch` still defines `fn translate`, and the Entry
      Path concept and spec still state the rule in those words.
- [x] The five S3 functions still read as one family: one shared stem with the
      `_object`, `_listing`, `_conditional_create` and `_transport` suffixes.
- [x] All three gateways use the same stem, so a reader who learns the word in
      one meets it unchanged in the others.
- [x] `make check` passes — the rename is signature-preserving, so every
      existing test covering these error paths still covers them.

### Manual / on-hardware (verified by a human before merge)

- [ ] None. A rename with no behaviour change is fully covered by the build and
      the existing suites.
