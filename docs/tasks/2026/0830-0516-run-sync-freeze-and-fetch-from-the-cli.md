---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, user-experience, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && make s3-store-it && ! grep -q 'coffret-usecase' backend/crates/apps/coffret-cli/Cargo.toml && grep -q 'fn create_app_folder(' backend/crates/gateway/google-drive-store/src/app_folder.rs && ! grep -q 'parent: Option<&str>' backend/crates/gateway/google-drive-store/src/app_folder.rs"
assignee: null
branch: task/0830-0516-run-sync-freeze-and-fetch-from-the-cli
created_at: 2026-08-30T05:16:00Z
updated_at: 2026-08-30T15:10:00Z
---

# feat(backend): run sync, freeze and fetch from the coffret CLI, and join a Library from a Recovery Code

## Overview

With `coffret-device` able to create and open a Library (previous task), the
batch flows in `backend/crates/domain/coffret-usecase` — `sync_folders`
(`sync/run.rs`), `freeze_folder` (`freeze/run.rs`), `fetch_folders`
(`fetch/run.rs`) and `fetch_entry` (`fetch/entry_run.rs`) — still have no
caller outside the conformance suites. This task gives them a command each,
and adds the one setup command the round trip needs that `init` cannot
provide: **`join`**, which enrols a second device (or the same machine
under a second Library name) from a Recovery Code so that a `fetch` has
somewhere to fetch to. Without it the only local copy of every Entry is the
one `sync` uploaded from, and `fetch` has nothing to do.

### `coffret-device` additions

- `join_library(request)` — the `init` flow's sibling for an existing
  Library: takes the device-side name, the Recovery Code (parsed with the
  format crate; its epoch becomes the stored form's epoch), the passphrase,
  and the app folder's **explicit** location — `--folder-id` on Drive
  (discovery by enumerating `coffret-*` folder names (FM-18) is the restore
  flow's task, so for now the user pastes the id from `init`'s output or the
  Drive UI) or `--bucket`/`--prefix` (the full `…coffret-<hex>/` prefix) on S3 —
  and produces the same five-entry directory as `init`, with the Library ID
  parsed out of the folder name / prefix. Same staging, refusal and
  cleanup states as `create_library`; Drive runs the OAuth flow. It writes
  nothing to Storage. The first `sync`/`fetch` catches the empty Index up
  through the flows' own catch-up (`commit/catch_up.rs` is private to the
  usecase crate; do not expose it).
- `run_sync(name, passphrase) -> SyncOutcome`, `run_freeze(name,
  passphrase, prefix: Option<EntryPath>, target: u64) -> FreezeOutcome`,
  `run_fetch(name, passphrase, prefix: Option<EntryPath>) -> FetchOutcome`,
  `run_fetch_entry(name, passphrase, path: EntryPath) -> EntryFetch` — each
  opens the Library, builds the request (`SyncRequest::new(store, index,
  keys, spool_dir, batch, now)` etc.) and runs it with the default
  `CommitPolicy`. `now` is `DeviceTime::from_unix_seconds` of the system
  clock; the `BatchId` is `<RFC 3339 UTC seconds>-<8 random hex>` — it only
  has to be unique among this device's unfinished batches
  (`device_state/batch_id.rs`), and a timestamp makes a spool directory
  legible to a human.
- A `Findings` view over the outcomes' `surfaced` / `unavailable` /
  `locked` / `reconciled` lists, because every outcome's doc says a caller
  *must* read them: a run that returns `Ok` has not necessarily backed up
  or placed everything (PK-14, EP-11, EP-12). The CLI renders it; the
  server will too.

### CLI

- `coffret sync --library <n>`
- `coffret freeze --library <n> [--under <prefix>] [--target <bytes>]` —
  `--target` defaults to 1 GiB. The Pack target size is an open design
  question (1 GiB vs 2 GiB, to be settled by measurement), so it is a flag
  with a default, never a constant in the format crates.
- `coffret fetch --library <n> [--under <prefix>]` and
  `coffret fetch --library <n> --entry <path>` (mutually exclusive with
  `--under`).
- `coffret join --name <n> --recovery-code <code> --drive --folder-id <id>
  [--client-id …] [--client-secret …]` / `… --s3 --bucket <b> --prefix
  <full app prefix> [--endpoint …] [--region …] [--path-style]`. The code
  may be pasted in grouped form. Passphrase prompted twice, or
  `--passphrase-stdin`.
- Output: one summary line per outcome field a user cares about (`added N,
  replaced N, unchanged N, committed head <g>` for sync; packs / absorbed /
  packed-already for freeze; fetched / containers / skipped for fetch),
  then one line per finding (`surfaced <path>: <reason>`, `unavailable root
  <root>`, `locked container <id>`), then nothing else on stdout. Exit code
  **0** when the run succeeded with no findings, **2** when it succeeded
  but surfaced findings (so a script notices without parsing), **1** on
  error. The log path goes to stderr as in the previous task, and the
  usecase crates' `tracing` output goes to the JSONL log only, never to the
  terminal.

### Tests

Extend the MinIO-gated CLI integration test from the previous task into a
full round trip, on one machine with two Library names against one bucket:

1. `init` Library `a` (S3), `map` a temp folder holding a few files at a
   prefix, `sync` → exit 0, summary says `added <count>`.
2. `freeze --under <prefix> --target <small>` → exit 0, at least one pack.
3. `join` Library `b` with the Recovery Code from step 1 and the S3 prefix
   from `a`'s `settings.json`, `map` an empty temp folder at the same
   prefix, `fetch --under <prefix>` → exit 0, every file's bytes equal the
   originals, and `fetch --entry <one path>` on a third empty mapping
   places exactly that file.
4. Findings path: delete one source file from `a`'s folder after step 1 and
   `sync` again → exit 2 with a `surfaced` line naming it (deletion
   propagation is off by default, so the file is reported, not removed).
5. A wrong passphrase to `sync` → exit 1, nothing uploaded (bucket listing
   unchanged).

`coffret-device` unit tests cover `join_library`'s refusals (existing name,
malformed code, a Drive `--folder-id` whose name is not `coffret-<hex>` when
the name is known, an S3 prefix not ending in `coffret-<hex>/`) and the
`BatchId` spelling.

### Carried over from the setup commands (fix in the same change)

Walking the previous change's CLI as a user surfaced three things that the
setup commands got wrong and that every command here would repeat; fix
them at the `coffret-device` API so all commands benefit:

- **A passphrase is asked for before refusals that do not need it.**
  `init --name main` with `main` already present, `--name a/b`, a
  `--prefix` without its trailing `/`, `authorize --library ghost`,
  `recovery-code` on an unknown Library — each prompts (twice, for `init`)
  and only then refuses. Make `create_library`, `authorize`,
  `recovery_code` and every new `run_*` take the passphrase through a
  callback (the way `create_library` already takes the consent-URL
  callback) that the device layer invokes only once every refusal that
  needs no key has passed. The CLI's `passphrase::enter` / `choose` become
  that callback.
- **`init --s3` never touches the network**, so a mistyped bucket or
  missing credentials produce a complete "success" with a Recovery Code.
  Add one `HeadBucket` in the S3 branch of `create_library` (reported as
  its own `CreationStep`) so the failure happens at `init`, not at the
  first `sync`.
- **Re-mapping a prefix replaces the old root silently.** `set_mapping`
  returns the replaced `Option<Mapping>`, and `coffret map` prints
  `<prefix> was at <old>; it is now at <new>.` when one was replaced.
- **A Drive Library must be told where to go.** Creating the app folder at
  the top of My Drive is never what the user wants, so `init --drive`
  requires `--parent <folder id>` (and `join --drive` keeps requiring
  `--folder-id`); `google_drive_store::create_app_folder` takes the parent
  as `&str`, not `Option<&str>`, and the Drive conformance support no longer
  accepts `COFFRET_DRIVE_FOLDER_ID=root`. While there, make the conformance
  suite trash the per-case `coffret-<hex>` folders it creates once a case
  ends, so a real-Drive run leaves the account as it found it.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `coffret-device` unit tests: `join_library` refuses an existing
      Library name, a malformed Recovery Code (typed error from the format
      crate passed through), and an S3 prefix that does not end in
      `coffret-<16 hex>/`; a generated `BatchId` matches
      `^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z-[0-9a-f]{8}$`; the `Findings`
      view of an outcome with one surfaced path and one unavailable root
      renders both and reports "has findings".
- [x] CLI integration test (MinIO, `make s3-store-it`): the five-step round
      trip above passes — `sync` exits 0 with the added count, `freeze`
      exits 0 with at least one pack, `join` + `fetch` reproduce every
      original file byte-for-byte, `fetch --entry` places exactly one file,
      a deleted source file makes `sync` exit 2 with a `surfaced` line, and
      a wrong passphrase makes `sync` exit 1 with the bucket listing
      unchanged.
- [x] The CLI crate still does not depend on `coffret-usecase` (gate
      appended to `check_command`) — the outcome rendering lives in
      `coffret-device`.
- [x] Carried-over fixes: a CLI process test shows `init --name <existing>
      --passphrase-stdin` refusing without consuming stdin (the passphrase
      line is still unread — feed none and expect the refusal, not an
      "empty passphrase" error); `init --s3` against an absent MinIO bucket
      exits 1 naming the bucket, and nothing is created on the device;
      `map` of an already-mapped prefix prints the `was at …; it is now at
      …` line; `init --drive` without `--parent` is refused by clap before
      any prompt (`grep -q 'fn create_app_folder(' backend/crates/gateway/google-drive-store/src/app_folder.rs`
      still matches and `grep -q 'parent: Option<&str>' backend/crates/gateway/google-drive-store/src/app_folder.rs`
      no longer does — append both gates to `check_command`).

### Manual / on-hardware (verified by a human before merge)

- [ ] On a real Google Drive Library: `sync` of a folder with a few hundred
      images commits (the `coffret-<hex>` folder shows `head-1.cfrt`,
      `key-1-…` replicas and the Containers), `join` under a second name
      with the printed Recovery Code and `--folder-id`, then `fetch` places
      the files; the JSONL log holds the run and no plaintext path appears
      in the terminal beyond the findings the user asked for.

## Out of scope

- Discovering the app folder from the Recovery Code alone (enumeration of
  `coffret-*`) — the restore flow.
- Deletion propagation flags, evict, and Pack update/deletion (PK-9..12).
- `coffret-server` consuming `coffret-device` — the explorer connection.
- A daemon / watch mode; each command is one run.
