---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, error-type-design, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -rq "change-passphrase" backend/crates/apps/coffret-cli/src/ && grep -q "change-passphrase" docs/guides/environments.md'
assignee: null
branch: task/1014-0336-add-change-passphrase-to-the-cli
created_at: 2026-10-13T18:36:00Z
updated_at: 2026-10-14T00:02:00Z
---

# feat(cli): add `change-passphrase`, which re-protects this device's Master Key under a new Passphrase

## Overview

A device's Passphrase cannot be changed. The spec has said how a change
behaves since the custody rules were written — DK-6 in
`docs/spec/device-key-custody/README.md:63-67`: each device has its own
Passphrase, changing it re-protects only that device's stored Master Key, the
Master Key itself is unchanged, and no Container and no control object is
rewritten — and the Passphrase concept (`docs/concepts/passphrase/README.md:20-24`)
says the same, but no command does it. The original CLI task left it out
(`docs/tasks/2026/0830-0514-add-the-coffret-cli-with-init-authorize-and-map.md:272`).
Add it to the CLI first; the desktop app may offer it later.

### The command

`coffret change-passphrase --library <name> [--passphrase-stdin]`.

- It takes the current Passphrase, then the new one, the way the CLI already
  takes Passphrases (`coffret-shell/src/passphrase.rs`): `entering` for the
  current one, `choosing` for the new one (typed twice at a prompt; one line
  on standard input). With `--passphrase-stdin` the current Passphrase is the
  first line and the new one the next, following the Recovery Code / Passphrase
  order DK-10 sets for `join`. An empty new Passphrase is refused as `init`
  refuses one. A new Passphrase equal to the current one is accepted and
  simply rewrites the file (there is nothing to refuse).
- A wrong current Passphrase leaves the device as it was (DK-5): the refusal
  is the existing `MasterKeyNotUnlocked`, and nothing is written.
- It writes nothing to Storage and needs no network: it never opens the
  Library's Storage, so it works offline and against a Library whose grant
  has lapsed.
- The answer, in text and in `--json` (`answer::SHAPE` lists every command's
  fields in `coffret-cli/src/answer/mod.rs:126-142`; add a line
  `change-passphrase  library`), names the Library. The text form says the
  Passphrase was changed on this device only and that other devices keep
  theirs, in one or two lines.
- A server or the desktop app holding the Library open keeps its derived
  keys until it locks; the next unlock reads `master-key.cfmk` again and so
  needs the new Passphrase. Say so in the command's help text. Do not take
  `server.lock` for this: the file is replaced atomically and the running
  process never re-reads it mid-run.

### The usecase

In `coffret-device`, beside `recovery_code` (`src/recovery_code/mod.rs`, the
smallest existing shape: resolve the `LibraryDir`, `unlock_asking` with the
Passphrase, answer): `change_passphrase(name, enter_current, choose_new)`
unlocks the stored Master Key with the current Passphrase, builds a new
`StoredMasterKey::create(&new, &unlocked.master_key, unlocked.epoch)` (fresh
salt and nonce, `Argon2Params::INITIAL`, which is also what KD-6 means by
re-deriving the protection key), and writes it over `master-key.cfmk`
through `StoredMasterKeyFile::write`, which `owner_only::write_file` makes
atomic. The test helper `keep_under` in
`coffret-device/src/account_tests.rs:774-787` already does exactly these
steps; replace it with a call to the usecase. Re-export the usecase from
`coffret-device/src/lib.rs` beside `recovery_code`.

`Passphrase` (`coffret-model/src/passphrase.rs`) zeroizes on drop and is not
`Clone`; keep both Passphrases in that type for their whole life, and let the
new one move into `create` rather than be copied. The DK-7 inventory test in
`coffret-usecase/src/zeroization.rs` must not need a new entry: no new
secret-bearing type is introduced.

### Where it is documented

- `docs/guides/environments.md:70-75` lists the commands the desktop app does
  not offer and that run on the production Library through `make cli`; add
  `change-passphrase` to that list (the gate in `check_command` greps for it).
- The clap doc comment on the subcommand is its `--help`; the JSON shape line
  in `answer::SHAPE` is the other place the command list lives.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A CLI integration test (`coffret-cli/tests/setup.rs` style, with
      `Device` and `init_s3`): after `change-passphrase` with the current and
      a new Passphrase on standard input, `recovery-code` with the new
      Passphrase prints the same code `init` printed, and with the old one
      exits 1 and prints no code (DK-5, DK-6)
- [x] A CLI integration test: a wrong current Passphrase exits 1 with the
      `master_key_not_unlocked` kind in `--json`, and `master-key.cfmk` is
      byte-for-byte what it was
- [x] A CLI integration test: an empty new Passphrase is refused as
      `empty_passphrase`, and two differing new Passphrases at a prompt are
      refused as `passphrases_differ` (the stdin form takes one line, so this
      case uses whatever the existing `choosing` tests use)
- [x] A JSON golden test (`coffret-cli/tests/json.rs` style) for the
      `change-passphrase` answer: `{ "library": "<name>" }` inside the usual
      envelope with `command: "change-passphrase"`
- [x] A `coffret-device` unit test beside the usecase: after the change the
      stored file unlocks under the new Passphrase to the same Master Key and
      epoch, and not under the old one; `settings.json` and every other file
      in the Library directory are unchanged
- [x] `account_tests.rs` `keep_under` is replaced by the usecase and the
      account tests still pass
- [x] `make check` passes; `change-passphrase` appears under
      `coffret-cli/src/` and in `docs/guides/environments.md` (both gates in
      `check_command`)

### Before merge (verified outside the check command)

- [x] On the agent's Library on this machine (`source .tmp/agent/env.sh`,
      `COFFRET_STATE_DIR` set to each of the two agent state directories in
      turn): both devices' Passphrases are changed to a new value;
      `.tmp/agent/up.sh 1` with the new value serves the Library and
      `GET /api/library` answers; starting the server with the old value is
      refused; MinIO's bucket listing (`aws s3api list-objects` in the
      aws-cli container) shows the same keys and sizes before and after. The
      new value is written into `.tmp/agent/env.sh`
