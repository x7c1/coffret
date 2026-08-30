---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, user-experience, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && make s3-store-it && test -f backend/crates/apps/coffret-device/src/lib.rs && test -f backend/crates/apps/coffret-cli/src/main.rs && ! grep -q 'coffret-usecase' backend/crates/apps/coffret-cli/Cargo.toml"
assignee: null
branch: task/0830-0514-add-the-coffret-cli-with-init-authorize-and-map
created_at: 2026-08-30T05:14:00Z
updated_at: 2026-08-30T11:34:34Z
---

# feat(backend): add the coffret CLI with init, authorize and map over a device settings file

## Overview

Every flow a user needs exists as a library call — `sync_folders`,
`freeze_folder`, `fetch_folders` in `backend/crates/domain/coffret-usecase`,
`SqliteIndex::open` in `backend/crates/gateway/coffret-sqlite-index`,
`GoogleDrive::new` / `OAuthTokens` / `TokenCache` / `Authorization` in
`backend/crates/gateway/google-drive-store`, `S3::new` in
`backend/crates/gateway/s3-store`, `StoredMasterKey` and
`generate_master_key` in `backend/crates/domain/coffret-format` — but nothing
composes them. No code writes a stored Master Key to disk, persists which
provider a Library lives on, or remembers where its Index and spool are; the
conformance suites hard-code `MasterKey::from_bytes([0x5a; 32])` and an
in-memory Index, and the only OAuth entry point is
`cargo run -p google-drive-store --example authorize` driven by environment
variables. A tester cannot create a Library. This task adds the device
layer that does, and a thin CLI over it.

Two constraints shape the layering, because the browser-based explorer
(`coffret-server`) will later share this code and eventually take over
setup: **the logic lives in a library crate and the CLI is a thin shell**,
and **the device settings file is a contract shared by the CLI and the
server**, not a CLI convenience.

### Crates

- **`backend/crates/apps/coffret-device`** (library) — what a device keeps
  for a Library and how it opens one. Depends on `coffret-usecase`,
  `coffret-format`, `coffret-model`, the three gateways and
  `coffret-logging`. This is the composition root the Index crate's docs
  defer to ("where a Library's catalog lives by default, and the permissions
  its file is created with, are the composition root's business"). Under
  `apps/` because it sits above the gateways; `make deps` only forbids
  gateway→gateway edges.
- **`backend/crates/apps/coffret-cli`** (binary named `coffret`, via
  `[[bin]] name = "coffret"`) — clap 4 derive subcommands, `anyhow::Result`
  in `main`, `#[arg(long)]` flags, `#[tokio::main]`; follows the
  `coffret-interop` `main.rs` dispatcher shape. It depends on
  `coffret-device` and **not** on `coffret-usecase` or any gateway (the
  `! grep` gate on its `Cargo.toml` enforces the thin-shell rule).

Both crates are picked up by the workspace glob; declare them in
`[workspace.dependencies]` like the others. New third-party dependencies
(`rpassword` for the passphrase prompt; nothing else expected — `serde_json`,
`clap`, `tokio`, `time`, `aws-sdk-s3`, `reqwest` are already in the
workspace) are pinned in `backend/Cargo.toml`.

### The Library directory and the settings file

One directory per Library, named by the device-side Library name the user
chooses at `init`, under the XDG state root the logging crate already uses:

```text
${XDG_STATE_HOME:-$HOME/.local/state}/coffret/libraries/<name>/
  settings.json      the device settings file (this task's contract)
  master-key.cfmk    the KD-9 stored form, 0600
  token-cache.cftc   the KD-10 sealed OAuth cache, 0600 (Drive only)
  index.sqlite       the SQLite Index, 0600
  spool/             encrypted spool files awaiting upload
```

(`layout-and-index.md` in the design already places the Index at
`…/coffret/libraries/<library>/index.sqlite`; everything else per Library
joins it. A separate `XDG_CONFIG_HOME` file was considered and rejected: the
file is machine-written by `init`/`authorize`/`map`, and splitting one
Library across two roots buys nothing.) `<name>` is validated as a single
path component (no separators, not `.`/`..`, non-empty). A `COFFRET_STATE_DIR`
override of the root is allowed for tests.

`settings.json` (serde, `serde_json`, pretty-printed, 0600):

```json
{
  "version": 1,
  "library_id": "<16 lowercase hex>",
  "provider": {
    "kind": "drive",
    "folder_id": "<app folder id>",
    "client_id": "<OAuth desktop client id>",
    "client_secret": "<optional>"
  }
}
```

or, for S3, `"provider": { "kind": "s3", "bucket": "…", "prefix":
"<base>coffret-<hex>/", "endpoint": "<optional URL>", "region": "…",
"path_style": true|false }`. The `kind` tag is `serde(tag = "kind")`.
Everything else a device needs is derived from the directory layout, so the
file carries no paths. An unknown `version`, an unknown `kind`, or an
unreadable file is a typed error that names the file; the CLI never
"repairs" it. The Drive client secret is persisted because Google's desktop
OAuth model treats it as non-confidential and the server will need it for
refresh; the file's 0600 mode is the protection. S3 credentials are never
stored — the AWS SDK's default provider chain (environment, profile) supplies
them, and the settings only say where the bucket is.

### `coffret-device` API (names are a proposal; keep the shape)

- `LibraryDir::resolve(name) -> Result<LibraryDir>` with path accessors for
  the five entries above.
- `DeviceSettings` — the struct above with `read(&LibraryDir)` /
  `write(&LibraryDir)`.
- `StoredMasterKeyFile` — `write(&LibraryDir, &StoredMasterKey)` (0600,
  temp file + rename) and `unlock(&LibraryDir, passphrase) ->
  Result<UnlockedMasterKey>`. A wrong passphrase is the format crate's
  typed error passed through; the file's bytes are never partially
  interpreted (DK-5, KD-9).
- `create_library(request) -> Result<CreatedLibrary>` — the `init` flow,
  in this order:
  1. refuse if `<name>/settings.json` already exists (state: *Library
     exists on this device*);
  2. if `<name>.partial/` exists from a crashed earlier `init`, remove it
     and start over (state: *leftover of an interrupted init* — nothing in
     it reached Storage under a persisted key; an empty Drive folder may
     have been left behind, and the message from step 5 names it when
     that happens);
  3. `generate_master_key()`, `generate_library_id()`; write the stored
     form under the passphrase into the staging dir `<name>.partial/`;
  4. Drive only: run `Authorization::run` (loopback PKCE) with the caller's
     URL callback, writing the token cache into the staging dir — the cache
     is sealed under the Master Key's `coffret/v1/token-cache` purpose key,
     which is why the key comes first;
  5. Drive: create the app folder under the requested parent (the previous
     task's gateway operation); S3: compute the prefix from the base and
     the Library ID — nothing is created, the first commit creates the
     objects;
  6. `SqliteIndex::open` on the staging dir's `index.sqlite` (creates the
     schema), create `spool/`;
  7. write `settings.json`, then rename `<name>.partial/` to `<name>/`;
  8. return the `RecoveryCode` (previous task's encoding) and the settings.
  On any failure after step 3 the staging dir is removed and the error
  names the step; a Drive folder created in step 5 before a later failure
  is reported by id so the user can delete it. Nothing is written to
  Storage besides that folder — Keyring generation 1 and Journal record 1
  are created by the first `sync`/`freeze` (`commit/keyring.rs:57`,
  `commit/journal.rs:147`).
- `authorize(name, passphrase, url_callback)` — re-run the OAuth flow for an
  existing Drive Library (the consent screen is in Testing, so refresh
  tokens expire after 7 days). States: no such Library → refuse; not a
  Drive Library → refuse; wrong passphrase → refuse without touching the
  cache (DK-2, DK-5); cache absent or present → the old cache, if any,
  survives until the flow has succeeded and the new one is written (check
  how `TokenCache` writes today and make it temp-file + rename if it is not
  already).
- `set_mapping(name, prefix: Option<&str>, local_root)` /
  `mappings(name)` — EP-9 mappings via the `Index` port; the Index is
  plaintext so no passphrase is needed. Prefix goes through
  `EntryPath::nfc` and is refused when it is not a valid path component;
  `local_root` is canonicalised and refused when it is not an existing
  directory (an unmounted root is EP-12's business at scan time, but
  recording a root that never existed is a typo, not a state). Re-mapping
  an existing prefix replaces it — that is `set_mapping`'s documented
  upsert.
- `open_library(name, passphrase) -> Result<OpenLibrary>` — builds the
  store (Drive: `ReqwestTransport`, `OAuthTokens` over the token cache,
  `DriveSettings::new(folder_id)`; S3: `aws_sdk_s3::Client` from the SDK's
  default config plus the endpoint / region / path-style from the settings,
  `S3Settings::new(bucket).with_prefix(prefix)`), opens the Index, derives
  `LibraryKeys` from the unlocked key and epoch, and returns them with the
  spool dir. A Drive Library whose token cache is missing or unreadable
  reports that (KD-10: unreadable is never empty) and tells the user to run
  `coffret authorize`. This task ships `open_library` with a unit test over
  the S3 path; the batch commands that consume it are the next task.
- A `DeviceClock` / `BatchId` helper is the next task's; do not add it here.

The unlocked Master Key stays in memory for the process lifetime and is
dropped on exit; a CLI process is one unlock (DK-9). The idle lock (DK-4)
is the long-running server's concern and is out of scope.

### CLI

- `coffret init --name <n> --drive [--parent <folder id>] [--client-id <id>]
  [--client-secret <s>]` / `coffret init --name <n> --s3 --bucket <b>
  [--prefix <base>] [--endpoint <url>] [--region <r>] [--path-style]`.
  `--client-id` defaults to `COFFRET_DRIVE_CLIENT_ID` and is required for
  Drive. Prompts for the passphrase twice (no echo, `rpassword`) unless
  `--passphrase-stdin` is given (reads one line — for scripts and tests).
  Prints the consent URL for Drive, then, on success, the Recovery Code in
  grouped form with an unmistakable instruction to write it down and keep
  it apart from the device — losing every device copy and every Recovery
  Code makes the Library unrecoverable (the Master Key concept doc's last
  Domain Rule). Then the Library directory path.
- `coffret authorize --library <n>` — passphrase prompt, prints the consent
  URL, confirms.
- `coffret map --library <n> [--prefix <p>] <local-root>` and
  `coffret mappings --library <n>` (prints prefix → root, root first).
- `coffret recovery-code --library <n>` — passphrase prompt, prints the code
  again in grouped form (for a user who lost the printout).
- Every command installs logging via `LogSettings::from_env()` +
  `coffret_logging::install` and prints the log path to stderr once
  (`examples/authorize.rs:80-96` in the Drive gateway is the precedent);
  URLs pass through `coffret_logging::redact` before logging. Errors print
  the typed chain (`{:#}`) and exit 1.

### Tests

`coffret-device` unit tests use `COFFRET_STATE_DIR` pointed at a `TempDir`.
The S3 provider path is exercised end-to-end against MinIO: add the two new
crates to `scripts/s3-store-it.sh`'s `cargo test` invocation so `make
s3-store-it` runs their MinIO-gated tests (same `COFFRET_S3_IT_*` variables
and skip-when-absent convention as `s3-store/tests/minio/mod.rs`). The CLI
integration test runs the built binary through
`std::process::Command::new(env!("CARGO_BIN_EXE_coffret"))` with
`--passphrase-stdin`; no `assert_cmd` dependency.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `coffret-device` unit tests: `DeviceSettings` round-trips both
      provider kinds through JSON; a settings file with `version: 2` or an
      unknown `kind` is refused with the typed error naming the file;
      `settings.json`, `master-key.cfmk` and `index.sqlite` are created
      with mode `0600` (unix); a Library name containing `/` or equal to
      `..` is refused before anything is created.
- [x] `create_library` for S3 (MinIO, `make s3-store-it`): produces the
      five-entry layout with a parsable `settings.json` whose `prefix` ends
      in `coffret-<library id>/`; a second call with the same name is
      refused and changes nothing; a leftover `<name>.partial/` is discarded
      and the call succeeds; the returned Recovery Code parses back to the
      Master Key the stored form unlocks to, at epoch 1.
- [x] `set_mapping` / `mappings` (unit, in-memory state dir): a root
      mapping and a prefix mapping are listed root-first; re-mapping a
      prefix replaces its root; a prefix with `/` in it and a non-existent
      local root are each refused with the typed error; `authorize` on an
      S3 Library is refused as not a Drive Library.
- [x] `open_library` for S3 (MinIO): returns a store that lists the empty
      app prefix without error and an Index whose `mappings()` matches what
      `set_mapping` recorded; a wrong passphrase is refused as the format
      crate's typed error and no file is modified.
- [x] CLI integration test (MinIO): `coffret init --s3 … --passphrase-stdin`
      exits 0 and prints a line starting with `coffret1`; `coffret map` then
      `coffret mappings` show the mapping; `coffret recovery-code
      --passphrase-stdin` prints the same code; a wrong passphrase to
      `recovery-code` exits 1 and prints nothing that starts with
      `coffret1`.
- [x] `backend/crates/apps/coffret-device/src/lib.rs` and
      `backend/crates/apps/coffret-cli/src/main.rs` exist, and the CLI
      crate's `Cargo.toml` does not depend on `coffret-usecase` (gates
      appended to `check_command`).

### Manual / on-hardware (verified by a human before merge)

- [x] `coffret init --name test --drive` against a real Google account
      (desktop OAuth client, `COFFRET_DRIVE_CLIENT_ID` set): the consent URL
      opens, the loopback redirect completes, a `coffret-<hex>` folder
      appears at the top of My Drive (and under `--parent <id>` when given),
      the Recovery Code prints, and `coffret authorize --library test`
      afterwards replaces the token cache.

## Out of scope

- `sync` / `freeze` / `fetch` commands and `join` (enrolling a second
  device from a Recovery Code) — the next task.
- Discovering an app folder by enumeration (restore flow), Master Key
  rotation, passphrase change (DK-6), idle auto-lock (DK-4).
- `coffret-server` reading the settings file — the explorer connection
  task; this task only makes the file the contract it will read.
- A bundled default OAuth client id.
