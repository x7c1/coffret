---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, error-type-design, spec-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1102-1614-tell-a-malformed-stored-master-key-apart-from-a-wrong-passphrase
created_at: 2026-11-02T07:14:00Z
updated_at: 2026-11-02T09:32:16Z
---

# fix: tell a malformed stored Master Key apart from a wrong Passphrase

## Overview

Three loose ends from bounding the stored Master Key's Argon2id cost
(KD-9 ceilings). Each is about how a stored form is refused.

1. **A malformed form reads as a wrong Passphrase on the desktop.**
   `StoredMasterKeyFile::read`
   (`backend/crates/apps/coffret-device/src/stored_master_key_file.rs`) turns
   every format refusal except the cost ceiling into
   `Error::MasterKeyNotUnlocked`. That covers a wrong length (an oversized
   file included), an unknown magic or version, and a non-zero reserved
   byte. The same variant also carries a failed authentication.
   `coffret-desktop-core/src/refusal_line.rs` (`is_wrong_passphrase`) then
   tells all of them as a Passphrase that does not open the Library. A
   person retypes a correct Passphrase forever.
2. **Rust and TypeScript refuse a multi-fault form for different reasons.**
   Both check the header length, magic, version and reserved byte in that
   order. Then Rust
   (`backend/crates/domain/coffret-format/src/stored_master_key/layout.rs`)
   checks the cost ceilings before the total length, and TypeScript
   (`frontend/packages/domain/format/src/storedMasterKey/storedMasterKey.ts`,
   `parseLayout`) checks the total length first. KD-9 does not order its
   refusals.
3. **No test drives the account path with a cost refusal.**
   `Accounts::key_in` / `key_for` (`coffret-device/src/accounts.rs`) pass
   `MasterKeyCostAboveCeiling` through instead of reading it as "the
   Passphrase given does not open one". Nothing tests that.

## Decided

- KD-9 states the order in which a reader checks a stored form, and Rust and
  TypeScript follow it. The order is: header length, magic, version,
  reserved byte, total length against the recorded salt length, then the
  Argon2id cost (lower bounds Argon2id itself sets, then the KD-9 ceilings;
  memory, iterations, parallelism). Structure is checked before values. The
  bounded file read still refuses an oversized file as a length mismatch
  before parsing.
- A stored form refused for its shape is reported as its own device error,
  distinct from a Passphrase that does not open the key and from the cost
  refusal. Where it reaches a person (CLI and desktop), it says that the
  Library's key file is not one coffret can read, and that the Passphrase
  was not checked. It never says the Passphrase is wrong.

## Changes

- **Spec.** Add the check order to KD-9's reader rules in
  `docs/spec/key-derivation/README.md`. Say that a form with several faults
  is refused for the first one in that order.
- **Format.** Reorder Rust's `Layout::parse` so the total length is checked
  before the cost. Keep TypeScript's order and confirm it matches the spec
  in every step. Add a multi-fault case (for example a wrong total length
  plus iterations `u32::MAX`) to both test suites and to the Rust/TS interop
  tests (`interop.test.ts`), so both refuse it with the same kind.
- **Device.** Split the shape refusals out of `MasterKeyNotUnlocked` into a
  new variant (a name such as `MasterKeyFileMalformed`, carrying the path and
  the format cause), handled in `display.rs`, `redacted.rs` and `source.rs`
  like the others. `MasterKeyNotUnlocked` then means only a failed
  authentication. Check every match on `MasterKeyNotUnlocked` across the
  workspace (CLI kinds, server routes, desktop core) and decide each
  deliberately.
- **Account path.** In `Accounts::key_in`, decide whether a malformed form
  is passed through like the cost refusal or still skipped. Either way the
  final refusal must not tell a malformed key as a wrong Passphrase. Say in
  the PR which you chose and why.
- **CLI and desktop.** Give the new refusal its own CLI kind and its own
  sentence in `refusal_line.rs`, next to the cost one.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] KD-9 states the check order. Rust and TypeScript follow it, and a
      multi-fault form is refused with the same kind in both, including in
      the interop tests. The tests cite KD-9.
- [x] Unlocking a Library whose key file has the wrong length, an unknown
      magic, an unknown version or a non-zero reserved byte returns the new
      device error, not `MasterKeyNotUnlocked`. The desktop refusal line and
      the CLI kind for it do not read as a wrong Passphrase.
- [x] A wrong Passphrase on a well-formed key still returns
      `MasterKeyNotUnlocked` and the wrong-Passphrase line.
- [x] A test in `coffret-device/src/account_tests.rs` opens an account
      through a Library whose stored form records iterations `u32::MAX`, and
      gets `MasterKeyCostAboveCeiling`, not `AccountNotOpened`.
- [x] A test covers how the account path reports a malformed key, as the PR
      decides.
- [x] `make check` passes.
