---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, error-type-design, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "pub enum Missing" backend/crates/domain/coffret-usecase/src/missing.rs && grep -q "pub use missing::Missing" backend/crates/domain/coffret-usecase/src/lib.rs && ! grep -q "\"the listing\"" backend/crates/gateway/s3-store/src/error.rs && ! grep -q "\"the bucket\"" backend/crates/gateway/s3-store/src/check_bucket.rs && ! grep -q "\"a commit slot\"" backend/crates/gateway/google-drive-store/src/google_drive.rs && ! grep -q "\"the configured folder\"" backend/crates/gateway/google-drive-store/src/app_folder.rs && ! grep -q "live_prefix()" backend/crates/gateway/s3-store/src/s3/listed_object.rs && grep -q "spec: EL-1, EL-5" docs/concepts/storage/README.md'
assignee: null
branch: task/0910-0455-give-a-missing-storage-subject-a-type
created_at: 2026-09-10T04:55:55Z
updated_at: 2026-09-10T11:10:35Z
---

# fix: give a missing Storage subject a type instead of a sentence in the object name

## Overview

`Error::NotFound { object: String }` in `coffret-usecase` carries the object an
operation asked for, and five callers put something else in that field:
`"the listing"` (`backend/crates/gateway/s3-store/src/error.rs`,
`translate_listing`), `"the bucket"` (`s3-store/src/check_bucket.rs`, the
`SUBJECT` constant), `"a listing"` and `"a commit slot"`
(`google-drive-store/src/google_drive.rs`, the `read_json` calls for `list`
and `reserve_create`), and `"the configured folder"`
(`google-drive-store/src/app_folder.rs`, the `PARENT_SUBJECT` constant the
app-folder create passes to `FailedResponse::into_error`). `Display` renders `no object named "the listing" in Storage`, and
`Redacted` for the type is `to_string()`, so the sentence reaches the log as
stored data.

Give the variant a typed subject. New module
`backend/crates/domain/coffret-usecase/src/missing.rs`, exported from `lib.rs`:

```rust
/// What a Storage operation asked for and did not find.
#[derive(Debug, Clone)]
pub enum Missing {
    /// One object, by the opaque name coffret minted or the provider minted.
    Object(String),
    /// The Library's listing, which addresses the configured location rather
    /// than any one object.
    Listing,
    /// The bucket, or the provider folder, the Library was configured into.
    Location,
    /// A provider resource that names no object of the Library's — the
    /// identifier-minting endpoint, for one.
    Endpoint,
}
```

`Error::NotFound { object: String }` becomes `Error::NotFound { missing: Missing }`.
`Display` for `Error` delegates to `Display` for `Missing`: `Object(name)` →
`no object named {name:?} in Storage` (unchanged), `Listing` → `Storage holds
no listing for this Library`, `Location` → `Storage holds no bucket or folder
for this Library`, `Endpoint` → `Storage answered a call with nowhere for it
to go`. `Missing::subject(&self) -> &str` returns the opaque name for
`Object` and a fixed word for the others (`the listing`, `the configured
location`, `the endpoint`); that is what the `object` event field is built
from in `s3-store/src/error.rs` and `google-drive-store/src/api/failed_response.rs`,
so the JSONL field stays `object` and stays queryable while the prose becomes
a rendering of a typed subject rather than stored data. It also keeps the
configured location out of the Drive 404 event. No `PartialEq`, no new string
sentinel, causes unaffected, conversion at the gateway boundary as the crate's
other conversions are. Sites that construct the variant: `s3-store/src/error.rs`, where
`translate(operation, object, error, private)` is the one classification
table every S3 call goes through and its `object: &str` both names the event
field and fills `NotFound` — make it take `Missing` and build the event field
from `Missing::subject` (`S3` callers → `Missing::Object`, `translate_listing`
→ `Missing::Listing`, `check_bucket` → `Missing::Location`, dropping its
`SUBJECT` constant); `failed_response.rs` (`into_error` and `read_json` take
`Missing`: `"a listing"` → `Listing`, `"a commit slot"` → `Endpoint`, the
app-folder create → `Location`, dropping `PARENT_SUBJECT`); and
`coffret-usecase/src/in_memory_store.rs` (`Object`). The tests that read the
`object` field back (`s3-store/tests/logging.rs`, `google-drive-store/src/logging_tests.rs`)
then expect the fixed words. Sites that destructure
`object` and need updating: `coffret-usecase/src/error.rs` (`Display`), the
tests in `check_bucket.rs`, `s3-store/tests/logging.rs`, and `app_folder.rs`;
matches on `Error::NotFound { .. }` are unaffected. Doc comments that name the
variant with a sentence: `coffret-usecase/src/object_store.rs`,
`check_bucket.rs`, `coffret-device/src/error.rs`.

One diagnostic still composes its text from the configured location:
`s3-store/src/s3/listed_object.rs` `describe` builds the
`MalformedResponse` detail for a listed object with no key from
`layout.live_prefix()`, and the prefix is the person's arrangement of their
Storage, not evidence (see the rule below). Drop the prefix from that
message — `Storage listed an object with no key` says everything the entry
has to say — and keep its existing test passing.

Last, say in the Storage concept what the two sibling concepts already say.
`docs/concepts/storage/README.md` records that where a Library sits is the
user's arrangement of their own Storage and never draws the consequence for
diagnostic events. Add a Domain Rule immediately after the app-folder rule:

- **Where in Storage a Library was configured is the person's own arrangement,
  not evidence.** The bucket or provider folder they chose, the base prefix
  under it, and the endpoint a device reaches it at are theirs rather than
  anything coffret or a provider minted, so no diagnostic event composes a
  field from one, and provider text is retained only after the bucket, the
  prefix and the chosen folder have been taken out of it. The app folder
  coffret creates inside that location is named after the **Library ID**, and
  that name — like a Container's opaque name and a control object's
  recognizable one — stays evidence an event may keep (spec: EL-1, EL-5).

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A missing subject that is not an object is reported by its type, and the
      opaque name of one that is still reaches the report unchanged
      (`coffret-usecase` `missing.rs`:
      `a_listing_that_is_not_there_is_reported_without_a_name`,
      `a_configured_location_that_is_not_there_is_reported_without_being_named`,
      `an_object_that_is_not_there_is_still_reported_by_its_opaque_name`), run
      by `make check`.
- [x] The typed subject exists, is exported, and no prose sentinel is left in
      the object-name field:
      `grep -q "pub enum Missing" backend/crates/domain/coffret-usecase/src/missing.rs`,
      `grep -q "pub use missing::Missing" backend/crates/domain/coffret-usecase/src/lib.rs`,
      `! grep -q "\"the listing\"" backend/crates/gateway/s3-store/src/error.rs`,
      `! grep -q "\"the bucket\"" backend/crates/gateway/s3-store/src/check_bucket.rs`,
      `! grep -q "\"a commit slot\"" backend/crates/gateway/google-drive-store/src/google_drive.rs`,
      `! grep -q "\"the configured folder\"" backend/crates/gateway/google-drive-store/src/app_folder.rs`.
- [x] The listing diagnostic no longer names the configured prefix:
      `! grep -q "live_prefix()" backend/crates/gateway/s3-store/src/s3/listed_object.rs`.
- [x] The Storage concept states that where a Library was configured is the
      person's own arrangement and carries the citation for it:
      `grep -q "spec: EL-1, EL-5" docs/concepts/storage/README.md`.
- [x] Existing backend, frontend, and interoperability checks continue to pass,
      the logging cases in both gateways included.

## Out of scope

Reclassifying a 404 from the identifier-minting endpoint as anything other than
not-found. Any change to which private values the gateways remove from provider
text. Renaming "log line" across the backend.
