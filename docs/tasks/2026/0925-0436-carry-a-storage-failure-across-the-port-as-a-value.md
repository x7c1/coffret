---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, error-type-design]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check"
assignee: null
branch: task/0925-0436-carry-a-storage-failure-across-the-port-as-a-value
created_at: 2026-09-25T04:36:06Z
updated_at: 2026-09-25T06:07:03Z
---

# refactor(usecase): carry a Storage failure across the port as a value, not as its rendering

## Overview

`coffret_usecase::Error` is the port every Storage gateway reports through, and
ten of its variants carry nothing but `detail: String` — `PermissionDenied`,
`LimitReached`, `Unauthenticated`, `Unsupported`, `Rejected`,
`MalformedResponse`, `RateLimited`, `ServiceUnavailable`, `Timeout` and
`Transport`. The gateways fill that string by rendering a structured value they
had in hand and then dropping it: `google-drive-store`'s `From<Error>` renders
its whole chain with `chain_to_the_workspace_edge` into `detail`,
`From<TransportError>` uses `error.to_string()`, and
`http/reqwest_transport.rs::classify` stringifies the `reqwest::Error` at the
very place it arrives. So by the time a failure reaches the device or the
server, `source()` ends at the port (`error.rs` line 331: every `detail`
variant answers `None`), the errno and the provider's structured answer are
gone, and the only way to see what happened is to read the sentence.

Two things follow from that shape and are part of this change:

- **`detail` slips the redaction contract.** `Redacted for Error` (line 360)
  renders `Io` as its kind and `Model` through its own `redacted()`, then lets
  every other variant fall to `other.to_string()` — which prints `detail`
  verbatim into a diagnostic event. The provider's text can name an object, a
  path or a bucket. `Io` and `Model` say in their docs that their message must
  not be recorded; the ten `detail` variants make no such promise and keep
  none.
- **The port cannot say a listing did not end.** The Drive gateway's
  `AppFolderDefect::UnendingListing { pages }` is classified as
  `MalformedResponse` (`error.rs::classify_folder_defect`), although Storage
  answered every page it was asked for. The same concept exists three times
  above the port — `SyncError`, `FreezeError` and `UploadError` each have
  `ListingLimitReached { pages }` — so the port is the one layer that cannot
  name it.

### What to build

Give the port a way to carry the gateway's failure as a value without naming
any gateway type (the dependency direction forbids that): each of the ten
variants keeps `detail` for `Display` and gains a `source` the gateway hands
over whole — a boxed `std::error::Error + Send + Sync + 'static` is the
smallest shape that lets `source()` walk into the gateway's own chain and lets
the logging layer redact link by link, and it is what "carry the value, not a rendering of it" means
here. Decide whether that is a field on
each variant or one shared wrapper type the variants hold; write the reason
where the type is declared. Whichever it is:

- `source()` returns the gateway value for every variant that has one.
- `redacted()` never renders `detail`. It renders the variant's name and its
  structured, non-private fields (a status, a limit's name, a `retry_after`),
  the way `Io` renders only its kind. Read `coffret-logging`'s crate doc and
  the `EL` register under `docs/spec/event-logging/` for what a diagnostic
  event may carry before deciding what is structured and what is private.
- `is_retryable` is untouched in meaning; its match stays exhaustive.
- `classify` in `reqwest_transport.rs` stops stringifying: `TransportError`
  carries the `reqwest::Error` (it has no `source()` today; give it one), and
  `From<TransportError>` passes it on as the port's `source`.
- The Drive gateway's `From<Error>` passes the `Error` itself as `source` and
  keeps `detail` as the rendered chain it is now. Every other place a gateway
  constructs a `detail` variant — count them first: `s3-store` builds them in
  `single_request_limit.rs`, `key_layout.rs`, `s3/listed_object.rs` and
  `s3/mod.rs`, the Drive gateway in `upload.rs`, `google_drive.rs` and
  `check_object.rs`, and there may be more — either has a value to hand over or
  composes the sentence itself with nothing behind it. The second kind is not
a violation: leave those with `source: None` (or
  whatever the shape's "nothing behind it" is) and do not invent a value.
- Add the listing-cap variant to the port (name it for what happened — the
  listing outran the pages this device reads — and carry `pages`), classify
  `UnendingListing` into it, and give it its `is_retryable` / `source` /
  `redacted` arms. Then look at the three `ListingLimitReached` above the
  port: if any of them is raised only because the gateway could not say it,
  it can now come from the port instead; if each is raised by the usecase's
  own page loop, leave them and say so in a comment on the new port variant.
  `coffret-server`'s `api_error/from_error.rs` must classify the new port
  variant the way it already classifies the usecase ones (kind `storage`,
  "the listing ran past its cap").

While in `From<Error>`, revisit one classification worth questioning:
`UnreadableTokenResponse` and `LoopbackRedirect` fold into
`Unauthenticated` (`is_retryable` = false), so a token response whose body
broke off mid-transfer is treated as a grant the person must renew. Now that
the cause travels as a value, decide per variant whether that is right and
write the reason beside the arm; change the arm only where the reason says the
current answer is wrong.

### Tests

The chain is the contract, so pin it: for one gateway failure of each kind
(a transport break, a provider refusal with structured fields, a defect the
gateway composed itself), assert what `source()` walks to at the port and that
`redacted()` of the port error contains no substring of `detail`. Existing
tests that match on `detail` text keep passing; tests that compare error values
with `assert_eq!` must not be added (equality on an error value is
not something a test may rely on).

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes
- [x] every `detail` variant of `coffret_usecase::Error` can carry the
      gateway's failure as a value, `source()` returns it, and a test walks
      the chain from a Drive transport break and from an S3 refusal
- [x] `redacted()` of a port error never contains its `detail`, and a test
      pins it for a `detail` that names an object
- [x] `reqwest_transport.rs::classify` no longer stringifies the
      `reqwest::Error`; `TransportError` has a `source()`
- [x] the port has a listing-cap variant, `UnendingListing` classifies into it,
      and the server answers it as kind `storage` with the cap sentence
- [x] every arm of `is_retryable`, `source` and `redacted` is exhaustive over
      the new shape (no wildcard arm on `Error`)

### Manual / on-hardware (verified by a human before merge)

- [ ] `make s3-store-it` is green

## Out of scope

- `ApiError.cause: Option<String>` in `coffret-server` and the shape of
  `coffret_device::Error` — the device-side error change that follows this one
- Splitting any `error.rs` into a directory — the module-structure change
- The three usecase `ListingLimitReached` variants beyond what the new port
  variant makes redundant
