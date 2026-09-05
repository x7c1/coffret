---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && ! grep -rqE 'pub (size|mtime):' backend/crates/domain/coffret-usecase/src/object_info.rs && ! grep -rqE 'cast_signed|cast_unsigned' backend/crates/gateway/coffret-sqlite-index/src && ! grep -rq 'unwrap_or_default' backend/crates/gateway/google-drive-store/src/api/file_resource.rs && ! grep -rqE 'max\\(0\\) as u64' backend/crates/gateway/s3-store/src && grep -rq 'a_negative_integer_in_a_catalog_column_makes_the_catalog_unreadable' backend/crates/gateway/coffret-sqlite-index/tests && grep -rq 'a_value_past_the_catalogs_integer_range_is_refused_on_write' backend/crates/gateway/coffret-sqlite-index/tests && grep -rq 'a_refused_write_leaves_the_catalog_as_it_was' backend/crates/gateway/coffret-sqlite-index/tests && grep -rq 'a_listed_file_without_a_name_is_a_malformed_response' backend/crates/gateway/google-drive-store/src && grep -rq 'a_listed_file_with_an_empty_id_is_a_malformed_response' backend/crates/gateway/google-drive-store/src && grep -rq 'a_listed_object_without_a_key_is_a_malformed_response' backend/crates/gateway/s3-store/src"
assignee: null
branch: task/0904-1758-stop-turning-missing-or-malformed-external-values-into-defaults
created_at: 2026-09-04T17:58:28Z
updated_at: 2026-09-05T03:36:22Z
---

# refactor(gateway): stop turning missing or malformed external values into defaults

## Overview

Two gateways still convert a value that is missing or malformed on the
outside into a normal-looking domain value, so the code past them cannot
tell "the provider said 0" from "the provider did not answer", or "the row
holds a huge number" from "the row holds a negative one":

- **Storage listings.** `ObjectInfo` (`backend/crates/domain/coffret-usecase/src/object_info.rs`)
  carries `size: u64` and `mtime: Mtime`, and nothing in production reads
  either (the listing consumers — `commit/control_listing.rs`,
  `conformance_library.rs` — read `name` and `object_ref`; `upload/run.rs`
  reads `hash`). To fill them, the Google Drive adapter
  (`backend/crates/gateway/google-drive-store/src/api/file_resource.rs::to_object_info`)
  turns a missing or non-numeric `size` into `0`, a missing or unparsable
  `modifiedTime` into the Unix epoch, and a missing `name` — a field the
  request asked for by name — into the empty string, which
  `control_listing.rs` then silently fails to parse as a control-object
  name and keeps as an `""` key. The S3 adapter
  (`backend/crates/gateway/s3-store/src/s3.rs::describe`) does the same
  with `unwrap_or_default().max(0) as u64` for `size` and the epoch for
  `last_modified`, and its `get` turns a missing `content_length` into a
  declared length of `0`, so a body that then delivers one byte is reported
  as `LengthOverrun` rather than as a response without a length.
- **Catalog rows.** The SQLite gateway stores every `u64` (`ciphertext_len`,
  `offset`, `size`, `observed_size`, `master_key_epoch`, every generation) as
  the same 64 bits reinterpreted (`rows.rs::to_integer` / `from_integer`,
  `cast_signed` / `cast_unsigned`), so a negative INTEGER — which no writer
  produces and which is therefore a sign of a damaged or hand-edited file
  — reads back as a value near `u64::MAX` and is accepted. The round trip
  of the full `u64` range is what that bought; no value the format can
  physically produce needs the upper half (Storage providers cap an object
  at a few terabytes, and a generation counts commits), and detecting the
  corruption sign is worth more than the range.

Make the gateways report what they got: a missing value that the provider
may legitimately omit stays an `Option`; a missing or malformed value that
the request required is a gateway error; a catalog integer outside the
range a writer can produce is an unreadable catalog. Nothing is converted
into a plausible default.

### 1. The listing port (`coffret-usecase`)

- **Remove `ObjectInfo.size` and `ObjectInfo.mtime`.** They have no
  consumer, and a port that carries a value nobody reads is a port that
  will one day be read by code that cannot tell an answer from a fill-in.
  When a consumer appears, the field comes back as `Option<…>` with the
  provider's own semantics stated. Update the port doc, `in_memory_store.rs`,
  and every fixture that built an `ObjectInfo`.
- **`ProviderHash`'s type doc** already says that Drive reports an MD5 and
  that an S3 ETag is an MD5 only for a single-part upload; keep it, and
  make sure the `ObjectInfo.hash` field doc points at it rather than
  restating "the provider's own digest". `hash` stays
  `Option<ProviderHash>`: a provider that reports no digest is a legitimate
  answer, not a malformed one.
- Opaque tokens (`ObjectRef`, `PageToken`) stay as they are — every byte
  string is a possible value and no format is invented for them. The one
  exception is emptiness where the token is used to build a request path:
  see the Drive `id` below.

### 2. Google Drive (`google-drive-store`)

- `FileResource::to_object_info` becomes fallible. A listing entry whose
  `name` is absent is `Error::MalformedResponse` (the `fields` parameter
  asked for it, so its absence is the provider answering something other
  than what was asked); an entry whose `id` is the empty string is
  `MalformedResponse` too (an empty id would make the next `get` address
  `files/`, which is not this object). The `size` and `modifiedTime`
  parsing goes with the fields.
- The listing path (`google_drive.rs`, where `FileList` becomes an
  `ObjectPage`) propagates the refusal with the existing error vocabulary;
  `control_listing.rs` keeps ignoring names that are not control-object
  names, which is a different, legitimate case (a Container's name).
- Unit tests beside the existing `file_resource.rs` tests:
  `a_listed_file_without_a_name_is_a_malformed_response`,
  `a_listed_file_with_an_empty_id_is_a_malformed_response`.

### 3. S3 (`s3-store`)

- `describe` distinguishes the two reasons a listed object yields nothing:
  a key that is outside this Library's layout (a stray key under the
  prefix, the trash) is still skipped, as the doc says; a listed object
  with **no key at all** is `Error::MalformedResponse`, not a skip. The
  `size` / `last_modified` conversions go with the fields.
- `get`: a response without `content_length`, or with a negative one, is
  `Error::MalformedResponse` — S3 always states the length of a
  `GetObject` body, so its absence is not "length zero". (The Drive
  transport's behaviour for a missing `Content-Length` — collect within the
  ceiling — is a different provider's documented behaviour and stays.)
- Unit test in `s3.rs` (the SDK's `Object` has a builder):
  `a_listed_object_without_a_key_is_a_malformed_response`. If the crate has
  no HTTP mock for `get` (check `single_request_limit.rs` and `tests/`),
  the `content_length` refusal is covered by the negative grep gate and a
  doc sentence; do not build a mock harness for it.

### 4. SQLite (`coffret-sqlite-index`)

- Keep the `INTEGER` column type and the schema version (the stored
  representation of every value a writer can produce is unchanged); narrow
  the accepted set. Replace `to_integer` / `from_integer` with two fallible
  helpers: on write, `i64::try_from(value)` refuses `2^63` and above; on
  read, a negative integer is `unreadable(operation, <column>, value)` →
  `IndexError::UnreadableCatalog`, the same verdict a malformed path or
  extent gets. Every one of the 16 call sites (`rows.rs`, `library_state.rs`,
  `device_state.rs`) goes through them.
- The write refusal needs a home in `IndexError` (`backend/crates/domain/coffret-usecase/src/index_error.rs`):
  neither `UnreadableCatalog` (it is not the catalog that is wrong) nor
  `Backend` (the store did not fail) fits. Add one variant naming *why* —
  a value the catalog cannot hold, carrying `operation`, the column, and
  the value — with a doc stating that no value the format produces in
  practice reaches it (a Storage object is at most a few terabytes; a
  generation counts commits) and that a Library which somehow did would be
  refused by this device rather than stored under a wrong sign. Its
  `Display` and `Redacted` renderings carry the number (an offset or a
  generation is the format's own arithmetic, not Library content).
- Tests in `tests/` (build rows with `rusqlite` the way `schema.rs` does):
  `a_negative_integer_in_a_catalog_column_makes_the_catalog_unreadable`
  (a negative `offset`, and separately a negative `head_generation`, each
  read back as `UnreadableCatalog` naming the column),
  `a_value_past_the_catalogs_integer_range_is_refused_on_write` (a
  `SnapshotContent` whose entry extent starts at `2^63` is refused by
  `restore` with the new variant), and
  `a_refused_write_leaves_the_catalog_as_it_was` (after that refusal the
  checkpoint, containers, and entries are exactly what they were — the
  refusal happens inside the transaction, or before it).

### Out of scope

- The `Mapping` value's EP-9 one-component rule on read (device state,
  not an external value), `local_root` existence, `RootIdentity`, `BatchId`,
  and every other opaque token: all bytes are valid, nothing to check.
- OAuth `expires_in`'s conservative default and the Drive error envelope's
  diagnostic defaults: a default that is safer than the absent value is not
  a fill-in that hides an answer.
- `ByteStream`'s declared-length ceiling and the transports' resource
  ceilings: they bound what an untrusted length may allocate and stay as
  they are.
- Changing the SQLite schema or its version: the stored representation of
  every value a writer produces is unchanged, so no existing Library needs
  a rebuild.
- The TypeScript side: it has no listing or catalog code. `make interop`
  (inside `make check`) must stay green.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `ObjectInfo` carries no `size` or `mtime`
- [x] A Drive listing entry without a `name`, or with an empty `id`, is
      `MalformedResponse`; `file_resource.rs` converts nothing with
      `unwrap_or_default`
- [x] An S3 listed object without a key is `MalformedResponse` (a key outside
      the layout is still skipped); an S3 `get` without a non-negative
      `content_length` is `MalformedResponse`; no `max(0) as u64` remains
- [x] The SQLite gateway stores integers through a fallible narrowing and
      reads them back refusing negatives; `cast_signed` / `cast_unsigned` are
      gone; a negative column is `UnreadableCatalog`, a value at or past
      `2^63` is refused on write with a named `IndexError` variant, and a
      refused write leaves the catalog unchanged
- [x] The tests named in sections 2–4 exist under the directories the check
      command greps
- [x] `make check` (backend fmt / build / test / clippy, frontend, interop) is
      green
