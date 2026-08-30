---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && grep -q 'FM-18' docs/spec/format/README.md && ! grep -q 'fn create_folder' backend/crates/gateway/google-drive-store/tests/support/mod.rs"
assignee: null
branch: task/0830-0512-name-and-create-the-app-folder
created_at: 2026-08-30T05:12:00Z
updated_at: 2026-08-30T08:03:00Z
---

# feat(backend): name a Library's app folder after a random Library ID and create it from the Drive gateway

## Overview

Every Library keeps its objects flat inside one app folder on Storage (a
Drive folder, or an S3 key prefix), but nothing in the repository knows what
that folder is called or how it comes to exist. `DriveSettings`
(`backend/crates/gateway/google-drive-store/src/settings.rs`) takes a folder
id somebody else obtained; the only code that creates a folder is test
support (`backend/crates/gateway/google-drive-store/tests/support/mod.rs`,
`create_folder`, posting `application/vnd.google-apps.folder` straight at
the Drive API); `S3Settings::with_prefix` takes any prefix. There is no
`LibraryId` type anywhere. The upcoming `coffret init` needs all three: an
identifier to name the folder with, the naming rule, and a way to create
the folder.

The design is settled: the app folder is named **`coffret-<library id>`**,
where the Library ID is 8 bytes drawn from the OS CSPRNG at Library creation
and spelled as 16 lowercase hex characters. The ID is random, not derived
from the Master Key (a rotation must not rename the folder), it lives in
the device's settings and not in the Recovery Code, and a user may override
the name. On S3 the app folder is the key prefix `coffret-<library id>/`,
placed under whatever base prefix the user configured (`""` or a prefix
ending in `/`). Where the folder is placed — a parent folder id on Drive,
a base prefix on S3 — is separate Library configuration; the app never
reads the parent, it only creates its folder under it. A recovering device
finds the folder by enumerating `coffret-*` names and picking the one whose
Keyring authenticates under the Recovery Code's Master Key; that discovery
flow is a later task, but the rule it will rely on is this one.

Concretely:

1. **Spec.** Add **FM-18** to `docs/spec/format/README.md`, next to FM-12
   (control-object names): the app folder's name, the Library ID's size and
   spelling, the S3 prefix form, that the folder holds the Library's objects
   flat with no sub-folders, that the ID is independent of the Master Key
   and survives rotation, and that the name may be overridden by the user
   (in which case discovery by enumeration does not find it). The naming and
   spelling part is *(Form: test)*; the override clause is *(Form: prose)*.
   Extend the FM row of the Mechanisms table in `docs/spec/README.md`.
2. **Model, `backend/crates/domain/coffret-model`.** A `LibraryId` type:
   `from_bytes([u8; 8])`, `as_bytes()`, `Display` as 16 lowercase hex
   characters, a `FromStr`/parse that accepts exactly 16 lowercase hex
   characters and rejects anything else with a typed error, and
   `app_folder_name(&self) -> String` returning `coffret-<hex>`. Give
   `S3`-side callers `app_prefix(&self, base: &str) -> String` (or an
   equivalent on the S3 settings) that appends `coffret-<hex>/` to a base
   that is either empty or `/`-terminated and rejects any other base.
   Mirror how `ContainerId` is organised (`coffret-model` depends on nothing
   but `unicode-normalization`; keep it that way — `make deps` enforces it).
3. **Generation, `backend/crates/domain/coffret-format`.**
   `generate_library_id() -> Result<LibraryId>` beside
   `generate_container_id()`, from the same CSPRNG path.
4. **Drive gateway, `backend/crates/gateway/google-drive-store`.** A
   pre-open operation that creates the app folder: it takes the transport
   and token source the store is built from plus an optional parent folder
   id (`None` = top of My Drive, the parent field omitted — the same
   convention the test support uses for `"root"`) and a `LibraryId`, posts
   the folder resource with `fields=id`, and returns the new folder id.
   This sits outside the `ObjectStore` port (`coffret-usecase`'s
   `object_store.rs` operates inside one Library) and outside
   `GoogleDrive` itself: it belongs to the stage before a store exists, so
   give it its own module (for example `app_folder.rs` at the crate root)
   whose doc says so, and leave room for the recovery-time enumeration to
   join it later. Failures map to the crate's typed `Error` like the other
   Drive API calls (`src/api/`), with a variant that names folder creation
   as the failing step; do not wrap it in the retry policy — a folder
   `POST` is not idempotent, and a retry after a lost response would leave
   two folders. Replace the test support's `create_folder` with a call to
   this operation so the real-Drive conformance suite exercises it.
5. **S3.** Nothing to create; the prefix helper above is what `init` will
   feed to `S3Settings::with_prefix`.

Keep the wire format of every existing object untouched; this adds a name
outside the objects, not a field inside them.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `docs/spec/format/README.md` contains FM-18 (the `grep -q 'FM-18'`
      gate is appended to `check_command`; it matches nothing today).
- [x] `coffret-model` unit tests: `LibraryId` round-trips through its hex
      spelling; `app_folder_name()` of a known id is `coffret-<hex>`;
      parsing rejects uppercase hex, 15 and 17 characters, and non-hex
      characters, each with the typed error; the S3 prefix helper yields
      `coffret-<hex>/` for an empty base and `<base>coffret-<hex>/` for a
      `/`-terminated base, and rejects a base without the trailing `/`.
- [x] `coffret-format` has a test that two generated Library IDs differ and
      are 8 bytes.
- [x] The Drive gateway's folder creation has unit tests over the crate's
      stub transport (`src/test_support.rs`, `StubTransport`): the request
      body carries the folder MIME type and the `coffret-<hex>` name, the
      parent is present when given and absent for My Drive, the returned id
      is the one the API answered with, and an API failure surfaces as the
      typed folder-creation variant.
- [x] `tests/support/mod.rs` in the Drive gateway no longer defines its own
      `create_folder` (the `! grep -q 'fn create_folder'` gate is appended
      to `check_command`; it matches today, so the gate flips with the
      change).

### Manual / on-hardware (verified by a human before merge)

- [ ] `make drive-store-it` against a real Google account still passes with
      `COFFRET_DRIVE_FOLDER_ID=root` and with an existing folder id as the
      parent — the conformance suite's per-case folders are now created by
      the gateway operation.

## Out of scope

- Enumerating `coffret-*` folders / prefixes for recovery and the RV rule
  for that discovery — the restore flow's task.
- Persisting the Library ID anywhere (the device settings file is the next
  task).
- Trashing or purging an app folder.
