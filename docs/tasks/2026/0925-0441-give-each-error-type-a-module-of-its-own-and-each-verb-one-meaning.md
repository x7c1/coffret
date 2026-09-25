---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check"
assignee: null
branch: task/0925-0441-give-each-error-type-a-module-of-its-own-and-each-verb-one-meaning
created_at: 2026-09-25T04:41:03Z
updated_at: 2026-09-25T08:43:36Z
---

# refactor: give each error type a module of its own, and each verb one meaning

## Overview

The structural leftovers of the error-type work: files that hold several
public types, error modules that have grown along four axes at once, and a
few identifiers that carry two meanings. Nothing here changes what any
function does; the diff should read as moves and renames (`git diff -M`),
with tests moved alongside what they test.

### 1. Error modules that outgrew one file

| module | lines | public types |
| --- | --- | --- |
| `coffret-device/src/error.rs` | 2012 | `Error`, `NameDefect`, `CreationStep` |
| `google-drive-store/src/error.rs` | 1217 | `Error`, `AppFolderDefect`, `TokenCacheDefect`, `TokenResponseDefect`, `RedirectStep` |
| `coffret-format/src/error.rs` | 1212 | `Error` |

Each of these grew along four axes — the variants and their docs, `Display`,
`From` conversions, and the `Redacted` rendering — plus a test module at the
bottom. Turn each into an `error/` directory: `mod.rs` declares the type and
its variants, and the sibling files hold what belongs apart (each secondary public type in a module named after it in
snake_case — `name_defect.rs`, `app_folder_defect.rs`, …) and what splits by responsibility (`display.rs`, `from.rs`,
`redacted.rs`, `tests.rs`, or whatever cut the file's own seams suggest).
Re-export from `mod.rs` so that every `use crate::error::…` and
`coffret_device::Error` path outside the module compiles unchanged. The
smaller error modules (`coffret-usecase` at 631 lines, `coffret-model` at
502) hold one type each; leave them unless the split of the three above makes
a shared shape obvious — say so either way.

### 2. Three types in `local_scan/walked.rs`

`Walked`, `WalkedRoot` and `RootState` share one file, and `root_state.rs`
already exists beside it. Give each its module; resolve the name collision by
reading what `root_state.rs` holds and naming the two things apart (a state a
root is in versus what the walk found the root to be, if that is the
distinction), and say in the module docs which is which.

### 3. `Noted` is the documented term `finding`

`coffret-server/src/noted.rs` holds `Noted`, and `routes/activity.rs` puts it
on the wire as `NotedDto`. The concept documents call the thing a sync or
freeze reports about one file a **finding** (`docs/concepts/library/` and the
sync vocabulary). Rename the type and its module to the documented word, rename
the wire field to `findings`, and — since a finding's reason is currently
folded into prose — carry the reason as its own discriminator field beside the
sentence, so a page can branch on it rather than parse it. The TypeScript
mirror in `frontend/packages/gateway/api` and every reader in
`frontend/packages/apps/web` move with it, and a Vitest test pins the new
shape. This is the one item here that changes bytes on the wire; keep it to
the rename and the added field.

### 4. One verb, two meanings

`open` is the Container verb — `Decoding::open` opens the meta section with the
key, `ContainerOutline::open` likewise — and it is also what
`Placement::open`, `Scatter::open`, `LocalPlace::open` and `SourceFile::open`
call creating or opening a local file. The concept vocabulary already has
words for the local side (materialize, scratch, spool, take in); choose one
verb for "make the local file this will write into" — `create`, `begin` or the
vocabulary's own word, whichever reads true at every call site — apply it to
all four, and leave `open` to the Container. Likewise
`oauth/authorization/mod.rs::run(open: F)` names the closure that shows the
person a URL `open`; call it `visit`, `show` or what it does.

### 5. Verify, then strike or fix

- `DescentError::Blocked` was once noted as carrying a field named `path`
  while its doc said "component". The field is `stopped_at: PathBuf` today.
  Confirm and do nothing.
- `coffret-format`'s test helpers that make keys (`other_key()` in
  `control/rejection_tests.rs` and its siblings) were noted as duplicated
  between the meta and control test modules. If a crate-level
  `#[cfg(test)] mod testing` removes the duplication without widening any
  visibility beyond `pub(crate)`, do it; otherwise report it and leave it.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes
- [x] the three error modules are `error/` directories with one public type per
      file, and no path outside a module changed to reach a type
- [x] `Walked`, `WalkedRoot` and `RootState` each live in their own module
- [x] the server's per-file report is named `finding` in Rust, on the wire and
      in TypeScript, carries a reason discriminator, and a Vitest test pins
      the shape
- [x] `open` names only the Container verb in `coffret-usecase`; a method
      that makes a local file to write into says `create`, and one that opens
      an existing file for reading says `reader`

### Manual / on-hardware (verified by a human before merge)

- [ ] `make e2e-it` is green
- [ ] the error-module split is moves, not rewrites: each sibling file's body
      is the old file's text apart from imports and intra-doc link targets
      (`git diff -M15% --stat` shows `error.rs => error/mod.rs` for all three;
      git's default 50% threshold sees only the format one)

## Out of scope

- Any change to what a variant means or which one a call raises — the two
  preceding error-type changes settled those
- Directory-modularising `library_id/`, `app_folder/`, `manifest/payload_fields.rs`
  and `generate/control_payloads.rs`: each was noted with a trigger (a
  second operation, a second schema) that has not arrived
- Sharing `every_link` across crates — nine copies in nine test targets that
  Rust's visibility cannot join
