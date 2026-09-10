---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, error-type-design, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "PrivateValues" backend/crates/libs/coffret-logging/src/redact/mod.rs && grep -q "PrivateValues" backend/crates/gateway/s3-store/src/s3/mod.rs && grep -q "PrivateValues" backend/crates/gateway/s3-store/src/check_bucket.rs && grep -q "PrivateValues" backend/crates/gateway/google-drive-store/src/api/failed_response.rs && ! grep -qF "error, \"\")" backend/crates/gateway/s3-store/src/error.rs && ! grep -q "into_error(parent)" backend/crates/gateway/google-drive-store/src/app_folder.rs'
assignee: null
branch: task/0910-0455-keep-the-configured-storage-location-out-of-every-gateway-diagnostic
created_at: 2026-09-10T04:55:55Z
updated_at: 2026-09-10T05:54:37Z
---

# fix: keep the configured Storage location out of every gateway diagnostic

## Overview

Only one path removes the configured location from what it records. The S3
adapter passes a real private value at exactly one call site —
`backend/crates/gateway/s3-store/src/s3/mod.rs` hands the configured prefix
to `translate_listing` — while `translate` hands an empty value on to
`translate_with` (`backend/crates/gateway/s3-store/src/error.rs`), so every
refusal of a `put`, `get`, `head_object`, `delete_object`, `copy_object`, a
conditional create, and the pre-store bucket check (`check_bucket.rs`) records
provider text with only the credential rules applied, in the `detail` a port
error carries and in the `reason` and `body` of the events
`ServiceFailure::record` emits. The bucket is passed nowhere at all, on any
path. The Drive adapter reads every refusal through `redact::body`
(`google-drive-store/src/api/failed_response.rs`), which knows no private
value, and its app-folder create composes an event field out of the folder the
person chose (`google-drive-store/src/app_folder.rs`, `into_error(parent)`).
A provider echoes what it was asked for, so all of these can carry a bucket, a
base prefix, or a chosen folder into a plaintext file on the same disk as the
Library. EL-5 (`docs/spec/event-logging/README.md`) already states the
obligation; this is an implementation gap, and EL-5's text and `Form: prose`
stay as they are.

Widen the redaction rule from one caller-owned value to a set, and thread it
through every site in both gateways that records provider text. Add
`PrivateValues` to `coffret-logging`'s `redact` module as a module of its
own (`redact/private_values.rs`; one module per rule, as `redact/mod.rs`
requires): a builder with `none()` and `with(value)` (an empty value adds
nothing), applied longest-first so a value containing another leaves no
fragment. `text_without` and `body_without` take `&PrivateValues`;
`without_private` applies each value in turn, keeping the existing
percent-encoding variants.

Decide the short-value question the single-value rule left open: **match on
whole tokens** — an occurrence is replaced only where the characters before and
after it are ones a bucket name or a path segment cannot contain (start or end
of text, whitespace, `/ \ " ' < > ( ) [ ] { } , ; : = ? & % + * | @ ! # $ ^ ~`
backtick, and the ASCII control range; `-`, `.`, `_` do not bound a match) —
at every length, with no minimum and no configuration refused. A length
threshold would leave the location in the log for exactly the people whose
bucket is short (S3 allows three characters), while replacing a three-character
value anywhere it appeared would turn a provider's own `NoSuchBucket` into
wreckage (`log` → `[redacted]ging`) and lose the evidence the event exists
for. The existing cases (`without_private.rs`, `text.rs`) all hold under this
rule; write the rule and its reason into `redact`'s own documentation, not
into the register.

Wiring. `S3` gains a `private: PrivateValues` field built in `S3::new` from
`settings.bucket()` and `settings.prefix()` (the un-normalised prefix suffices:
the trailing-slash form is a superstring of it). `translate`,
`translate_conditional_create`, `translate_listing`, `translate_with`, and
`ServiceFailure::record` take `&PrivateValues`; `translate_transport` needs no
separate wiring because its `detail` is the string `translate_with` built.
`check_bucket` passes `PrivateValues::none().with(bucket)`. On Drive,
`FailedResponse::read(response, operation, private)` uses `body_without` for
`body` and `text_without` for the envelope-derived `detail`; of its call
sites, only `create_app_folder` passes a real value — the parent folder the
person chose — and the others pass `PrivateValues::none()` as the declaration
that they address an opaque object. Nothing else on Drive is private: EL-5
states that the app folder's own `coffret-<library id>` name and the ids
Drive mints stay permitted evidence, so `app_folder.rs` recording the folder
name and id is correct as it stands. Route `google_drive.rs` and `upload.rs`'s
remaining `redact::body` calls through `body_without` with an empty set so a
new body-recording site cannot be added without naming its private values. Do
not silence an event or paraphrase a provider's answer into a category: status,
structured reason, and the body around the removed value all stay. Update the
EL-5 paragraph in `coffret-logging/src/lib.rs` to describe the set.

Tests to update to the new signatures and keep: `coffret-logging`'s
`redact/text.rs`, `redact/body.rs`, `redact/without_private.rs` cases; the
ten cases in `s3-store/tests/logging.rs`; `google-drive-store/src/logging_tests.rs`;
the `failed_response.rs` and `app_folder.rs` tests. New tests are named in the
acceptance criteria; they assert with `CapturedLogs::assert_free_of` and
`LoggedEvent::field` as the existing ones do.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A refusal of a non-listing S3 call whose body echoes both the configured
      bucket and the configured prefix records neither, in the event or in the
      returned error's diagnostic rendering, while status, structured reason and
      the rest of the body survive as evidence — for an ordinary write, for a
      conditional create refused for an unfamiliar reason, and for the pre-store
      bucket check (`s3-store/tests/logging.rs`:
      `a_put_refused_with_an_echo_of_the_configured_location_keeps_neither_bucket_nor_prefix`,
      `a_conditional_create_refused_for_an_unfamiliar_reason_keeps_the_location_out`,
      `a_bucket_check_that_is_refused_keeps_the_bucket_out_of_its_detail`), run
      by `make check`.
- [x] A three-character bucket name echoed by a provider is removed where it
      stands as a token and left alone inside the provider's own words, so the
      refusal stays readable (`s3-store/tests/logging.rs`:
      `a_short_bucket_name_goes_without_mangling_the_refusal_it_was_echoed_in`;
      `coffret-logging` `redact/without_private.rs`:
      `a_three_character_bucket_is_taken_out_where_it_stands_as_a_token`,
      `a_three_character_bucket_inside_a_providers_own_word_is_left_alone`).
- [x] Several private values are removed from one body, longest first, so a
      value containing another leaves no fragment, and no private value is the
      identity (`coffret-logging` `redact/private_values.rs`:
      `a_bucket_and_a_prefix_are_both_taken_out_of_one_body`,
      `the_longest_private_value_goes_first_so_no_fragment_is_left`,
      `no_private_value_is_the_identity`).
- [x] A refused Drive app-folder create keeps the parent folder the person chose
      out of every event it emits, the 404 debug event included, while the
      reason Drive gave stays (`google-drive-store/src/logging_tests.rs`:
      `a_refused_app_folder_create_keeps_the_chosen_parent_out_of_the_log`;
      `google-drive-store/src/api/failed_response.rs`:
      `a_body_that_echoes_a_private_location_is_read_without_it`).
- [x] The redaction API a gateway reaches for takes a set of private values, and
      both gateways pass one at every site that records provider text:
      `grep -q "PrivateValues"` in `coffret-logging/src/redact/mod.rs`,
      `s3-store/src/s3/mod.rs`, `s3-store/src/check_bucket.rs`, and
      `google-drive-store/src/api/failed_response.rs`.
- [x] No S3 translation hands an empty private value on, and the Drive
      app-folder create no longer reports the parent it was pointed at:
      `! grep -qF "error, \"\")" backend/crates/gateway/s3-store/src/error.rs`
      and `! grep -q "into_error(parent)" backend/crates/gateway/google-drive-store/src/app_folder.rs`.
- [x] Existing backend, frontend, and interoperability checks continue to pass,
      the ten existing cases in `s3-store/tests/logging.rs` and the Drive
      gateway's logging cases included. Every case here runs from a
      deterministic fixture (the S3 ones from a replayed HTTP response, the
      Drive ones from the scripted transport) in the CI `backend` job; none
      needs MinIO or a Google account.

## Out of scope

The endpoint a device was pointed at is not removed from provider text.
`S3Settings` deliberately holds no endpoint — region, credentials and endpoint
belong to the `aws_sdk_s3::Client` the caller builds — and `aws_sdk_s3::Config`
exposes no getter for one, so the gateway cannot remove a value it does not
hold. The OAuth token endpoint's own refusals stay on `redact::body`: that
request carries no Storage location. The prose sentinels in `Error::NotFound`
(`"the listing"`, `"the bucket"`, `"a listing"`, `"a commit slot"`) and the
Storage concept's statement about the configured location are a separate
change built on this one. No renaming of "log line" across the backend. No new
telemetry, no change to the never-list itself, no restructuring of error
modules beyond the signatures this touches, and no change to what a
person-facing refusal may name.
