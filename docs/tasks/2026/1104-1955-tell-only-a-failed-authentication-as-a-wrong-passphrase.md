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
branch: task/1104-1955-tell-only-a-failed-authentication-as-a-wrong-passphrase
created_at: 2026-11-04T10:55:51Z
updated_at: 2026-11-04T13:06:40Z
---

# fix(device): tell only a failed authentication as a wrong Passphrase

## Overview

**Starting point.** The working tree already holds a first implementation
of everything below (the new variant `MasterKeyNotDerived { path, cause }`,
the CLI kind `master_key_not_derived`, `key_in` passing through every
non-authentication refusal, and the tests), together with a new sub-bullet
under KD-9 in `docs/spec/key-derivation/README.md` that states the
obligation: only a stored form that fails to authenticate is told as a
Passphrase that does not open the Library; a missing or unreadable stored
Master Key file, or one whose Master Key does not come out of it for a
reason other than its Passphrase, is its own refusal naming the file, on
the account path too. Review it against this task, complete what is
missing, and keep the KD-9 sub-bullet's wording unless it is wrong about
the code. The KD-9 / SA-9 citations in doc comments and tests rely on that
sub-bullet.

Two refusals the first implementation left as they were:

- On the account path, a referencing Library whose stored Master Key file
  is missing is still reported as `NoSuchLibrary` ("no Library … is on this
  device"), which reads wrongly there: the Library is on the device, its key
  file is not. Make what is told name the missing file, without changing
  the failure kind or exit code `NoSuchLibrary` already has where it is
  reported today for a Library that is truly absent.
- The CLI reports the account-path refusal under the outer kind
  `library_not_created`. Leave it unless a kind can be made more precise
  without changing an existing one.

Two places in `backend/crates/apps/coffret-device/src/` still blur a key
refusal that never checked the Passphrase with one that did.

1. **The account path folds every other refusal into "not opened".**
   `Accounts::key_in` (`accounts.rs`) calls
   `StoredMasterKeyFile::unlock` and passes through only
   `MasterKeyFileMalformed` and `MasterKeyCostAboveCeiling`. Every other
   error — `NoSuchLibrary` (the file is missing), an I/O error reading it,
   `KeyMaterial` (the derivation could not run, or an authentic form holds
   no epoch) — becomes `None`, the answer for "this Passphrase does not
   open the Library". `key_for` then reports `AccountNotOpened`, which the
   CLI and desktop tell as a Passphrase that does not open the Library. A
   person retypes a right Passphrase forever.

   Fix: only `Error::MasterKeyNotUnlocked` (a failed authentication) is
   `None`. Every other refusal is `Some(Err(..))`, so `opened_through`
   keeps it as the refusal it records (another Library referencing the
   account is still tried, as today) and `key_for` returns it instead of
   `AccountNotOpened`. Update the doc comments of `key_in` and `key_for`
   to say so.

2. **`KeyMaterial` from `StoredMasterKeyFile::opened` does not say which
   file.** `opened` (`stored_master_key_file.rs`) maps every non-auth
   `FormatError` to `Error::KeyMaterial { cause }`, which carries no path.
   `KeyMaterial` is also used where no file is involved (key generation in
   `account_grant.rs`, `create_library/run.rs`, `join_library/run.rs`,
   `change_passphrase/mod.rs`, `account_envelope.rs`), so do not add a
   path to it. Add a variant that carries the stored Master Key file's path
   and the cause (a name such as `MasterKeyNotDerived { path, cause }`; pick
   the name that reads best beside `MasterKeyNotUnlocked` and
   `MasterKeyFileMalformed`), and handle it in `error/display.rs`,
   `error/redacted.rs`, `error/source.rs` and wherever the workspace
   matches on device errors (CLI failure kinds in
   `coffret-cli/src/answer/failure.rs`, server routes, desktop core's
   `refusal_line.rs`). Decide each match deliberately; none of them may
   tell the new variant as a wrong Passphrase. Follow how
   `MasterKeyFileMalformed` was threaded through (x7c1/coffret#414).

Tests:

- In `coffret-device` (`account_tests.rs` or beside `key_in`): a Library
  referencing the account whose stored Master Key file is missing, and one
  whose read fails, are refused as that error and not as
  `AccountNotOpened`; a wrong Passphrase is still `AccountNotOpened`; with
  two Libraries referencing the account, a refusal of the first does not
  stop the second from opening it.
- In `coffret-cli/tests/accounts.rs`, `lay_out` writes no stored Master Key
  file today. Give it one (or a variant of it) so that a case can drive the
  account path through the binary and show a missing or unreadable key file
  is not told as a Passphrase that does not open the Library.
- A unit test that `opened` returns the new variant with the Library's key
  file path for a non-authentication format error.

Guards:

- Do not change the stored Master Key file format, the CLI exit codes or
  failure kinds that already exist, or the HTTP status of any existing
  refusal. A new failure kind for the new variant is fine. If the fix needs
  to change one of those, or the behaviour a spec rule (`docs/spec/`, e.g.
  SA-8, SA-9, KD-9) states beyond the KD-9 sub-bullet described above, stop
  and report instead of changing it.
- `MasterKeyNotUnlocked` keeps meaning only a failed authentication.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `Accounts::key_in` returns `None` only for a failed authentication; a missing, unreadable, or underivable stored Master Key file reaches the caller as its own error (device tests)
- [x] With two Libraries referencing one account, a non-authentication refusal of the first still lets the second open the account (device test)
- [x] `StoredMasterKeyFile::opened` reports a non-authentication format error as a variant carrying the stored Master Key file's path (device test)
- [x] The CLI's account path tells a missing stored Master Key file apart from a Passphrase that does not open the Library (`coffret-cli/tests/accounts.rs`)
- [x] KD-9 states that only a failed authentication is told as a Passphrase that does not open the Library, and the code and tests cite it (`make spec-rule-ids` / `make spec-citations`)
- [x] `make check` passes
