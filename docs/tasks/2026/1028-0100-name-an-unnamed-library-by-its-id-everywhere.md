---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, spec-alignment, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1028-0100-name-an-unnamed-library-by-its-id-everywhere
created_at: 2026-10-28T01:00:00Z
updated_at: 2026-10-27T16:40:48Z
---

# fix: name an unnamed Library by its ID in the refusals that still put it in the name's place

## Overview

#373 made coffret say a Library has no name yet, and name it by its ID
as its ID, in advice, reports and the explorer (spec: FM-15: "named by its
ID as its ID — never with its ID in the name's place"). A few refusals
still take `LibraryDir::name()` — the ID's hex for a Library with no name —
as a `String` and print it as `the Library "<hex>"`. Bring them in line,
and close the related gaps found alongside.

### Refusals

Give each of these the Library as a `LibraryRef` (`dir.which()`) and
display it through the same `TheLibrary` wording the other variants in
`coffret-device/src/error/display.rs` use:

1. `Error::LibraryExists`, built by `Staging::vacant`
   (`coffret-device/src/staging.rs`) when a join or create finds the
   Library already here. Update `join_library/tests.rs`, which matches the
   field by its type today.
2. `Error::NotADriveLibrary`, built in `coffret-device/src/authorize/mod.rs`.
3. `Error::UnreadableAccountEnvelope`, built in
   `coffret-device/src/account_envelope.rs`.
4. `Error::AccountNotOpened`, built in `coffret-device/src/accounts.rs`,
   and the Passphrase prompt that names the Library through
   `referencing.ask(library.name())` (`coffret-shell/src/passphrase.rs`
   `referencing_prompt`, "Enter the Passphrase of the Library …").
5. `Error::ClientMismatch`, whose `library` comes from
   `Accounts::require_client(.., dir.name(), ..)` in
   `coffret-device/src/account_grant.rs`. Check each `dir.name()` there:
   where `dir` is an account's directory, its name is the account's and is
   right as it is; where it is a Library's, pass the Library.
6. `Error::NoSuchLibrary`: a `--library-id <hex>` that finds nothing says
   `no Library "<hex>" is on this device`, quoting the ID as a name. Say
   `no Library with ID <hex> is on this device` for an ID, and keep the
   quoted form for a name.

### One wording

7. "the Library with ID {id} (it has no name yet)" is built in three
   places: `TheLibrary` in `coffret-device/src/error/display.rs`,
   `changed()` in `coffret-cli/src/change_passphrase.rs`, and
   `ServerState::the_library()` in `coffret-server/src/state.rs`. Give it
   one home that the three use — for instance a `Display` on
   `LibraryOnDevice` or on `LibraryRef` in `coffret-device` — so the
   wording cannot drift.

### Docs

8. `docs/concepts/library/README.md`:
   - Domain Rules, after the bullet on the directory named after the
     Library ID: a device holds a Library once; two directories that hold
     one Library ID — a copy, or an earlier build's directory beside the
     moved one — are a fault: the Library is listed once, marked as needing
     attention, and not opened by its name or its ID until one directory is
     gone (spec: FM-15).
   - Definition, after "Where two Libraries on one device carry one name,
     it is their IDs that tell them apart.": a Library a device knows no
     name for — one joined before its Catalog caught up — is said to have
     none and named by its ID as its ID, never with its ID in the name's
     place (spec: FM-15).
9. FM-15 in `docs/spec/format/README.md` says two directories "whose
   settings carry one Library ID" are a fault, but `library_dir/find.rs`
   also counts a directory whose settings cannot be read when its name is
   that Library ID. Keep the code and widen the rule: two directories that
   hold one Library ID, by the ID their settings carry or, where the
   settings cannot be read, by the ID the directory is named after.
10. `coffret-explorer-dev/src/main.rs`: its `--library` and `--library-id`
    help says nothing of a Library in more than one directory, or of
    `--library-id` naming a Library with no name yet. Match the wording
    `coffret-cli/src/library_args/which_library.rs` and
    `coffret-server/src/main.rs` use.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `coffret-device` error tests: for a Library with no name,
      `LibraryExists`, `NotADriveLibrary`, `UnreadableAccountEnvelope`,
      `AccountNotOpened` and `ClientMismatch` name it by its ID as its ID
      and never as `the Library "<hex>"`; a named Library is quoted by name
      as before
- [x] A `coffret-shell` test: the Passphrase prompt for an unnamed Library
      names it by its ID as its ID
- [x] A `coffret-device` test: `NoSuchLibrary` for `--library-id` says
      `no Library with ID <hex>`, and for `--library` quotes the name
- [x] The three places that described an unnamed Library use one shared
      wording, and their existing tests pass unchanged
- [x] `make check` passes (including the spec gates)

### Before merge (verified outside the check command)

- [x] On a throwaway state directory under `.tmp/` holding a copy of a
      Library's directory with its name copy removed, with
      `COFFRET_STATE_DIR` and `COFFRET_LOG_DIR` set: `coffret join` of the
      same Library again is refused naming it by its ID as its ID, and
      `coffret sync --library-id 0000000000000000` says
      `no Library with ID 0000000000000000`
