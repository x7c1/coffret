---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, error-type-design, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && (cd backend && RUSTDOCFLAGS=-Dwarnings cargo doc --workspace --no-deps --document-private-items) && { grep -m1 -A4 '    Io {' backend/crates/domain/coffret-usecase/src/error.rs | grep -q 'cause'; } && ! grep -q 'detail: error.to_string()' backend/crates/domain/coffret-usecase/src/error.rs && { grep -m1 -A8 '    TokenCache {' backend/crates/gateway/google-drive-store/src/error.rs | grep -q 'cause'; } && grep -q 'serde_json::Error' backend/crates/gateway/google-drive-store/src/error.rs && ! git grep -qE 'detail: *error\\.to_string\\(\\)' -- backend/crates/gateway/google-drive-store/src/oauth && { grep -m1 -A8 '    LoopbackRedirect {' backend/crates/gateway/google-drive-store/src/error.rs | grep -q cause; } && { grep -m1 -A6 '    MalformedRedirect {' backend/crates/gateway/google-drive-store/src/error.rs | grep -q cause; } && ! git grep -qE 'derive\\(.*PartialEq.*\\)' -- backend/crates/domain/coffret-usecase/src/error.rs backend/crates/gateway/google-drive-store/src/error.rs"
assignee: null
branch: task/0829-0145-carry-flattened-causes-in-store-errors
created_at: 2026-08-29T01:45:00Z
updated_at: 2026-08-29T03:15:00Z
---

# fix(backend): carry flattened Rust error causes in store errors as values

## Overview

The store layers still flatten several *Rust error values* into `detail:
String` at the moment they are wrapped, which loses the structured cause the
moment it exists. This task replaces exactly those — a `detail` whose value is
produced by stringifying a value that implements `std::error::Error` — with a
typed `cause`, following the precedent `Error::UnsealableTokenCache { cause:
coffret_format::Error }` set (google-drive-store `error.rs:44`). Free-text
`detail` fields whose content is *provider-reported text* (HTTP bodies,
statuses, SDK messages) or diagnostics composed from non-error data are NOT in
scope: for those, text is the honest representation of the source. No
control-flow or retry-classification change anywhere.

**1. The `ObjectStore` port's `Io` variant stringifies the OS.**
`Error::Io { detail: String }`
(`coffret-usecase/src/error.rs:121`–`:125`) is built by
`From<std::io::Error>` at `error.rs:262`–`:264` via `error.to_string()`. This
is the sink the ledger's "spool open `io::Error` collapses into
`SyncError::Storage`" complaint points at: the OS error's kind, errno, and
chain are gone before anyone can act on them. Change the variant to carry the
error itself:

- `Io { cause: std::sync::Arc<std::io::Error> }` — `Arc` because the port
  error derives `Clone` (`error.rs:26`) and `io::Error` is not `Clone`;
  sharing the one cause is exactly right for a value that may be cloned into
  retries and reports.
- `From<io::Error>` keeps its signature (`Arc::new` inside); `Display` renders
  the cause; `Error::source()` — add the impl if the port error has none, and
  return the cause here; `is_retryable` keeps its current answer for `Io`.
- Every construction and assertion site follows mechanically (`in_memory_store`,
  the gateways, conformance suites that plant `Io` values).

**2. The Drive gateway's token-cache variants flatten what they observed.**
In `google-drive-store/src/error.rs`:

- `TokenCache { path, detail: String }` (`error.rs:26`–`:31`) documents its
  `detail` as "What the operating system reported", and three of its four
  construction sites in `oauth/token_cache/store.rs` (`:13`, `:43`, `:71`)
  wrap an `std::io::Error`. Change it to
  `TokenCache { path: PathBuf, cause: std::io::Error }` (the gateway error is
  not `Clone`, so the bare error is fine), wire `Display` and `source()` like
  `UnsealableTokenCache` already does.
- The fourth site (`store.rs:19`) is the mismatch the roadmap named: a
  **`serde_json::Error`** from encoding the tokens is stuffed into
  `TokenCache`, whose doc claims an OS origin. Give that failure its own
  variant carrying `cause: serde_json::Error` (name it for what happened —
  the tokens could not be encoded for sealing; mirror `UnsealableTokenCache`'s
  naming and doc style), and raise it at that site.
- `MalformedTokenCache { path, detail: String }` — inspect its construction
  sites: where the `detail` is a stringified `serde_json::Error` (the cache
  file failed to parse), carry that error as `cause`; where it is genuinely
  free text about the file's shape, leave that site's text but route it
  through whatever shape the variant ends up with (a small enum cause or two
  variants — pick the smallest honest shape).
- `HttpClient { detail: String }` (`error.rs:20`–`:24`) — if its construction
  site wraps the HTTP client library's builder error, carry that error as
  `cause` instead.

**3. The boundary to the port stays provider-neutral.** The gateway→port
translations (`translate_transport`, the status matching in both gateways, the
S3 `SdkError` matching) flatten provider/SDK errors into the port's
provider-text variants (`Transport`, `Timeout`, `Rejected`, …). That is the
explicit boundary translation the layering demands — the domain port cannot
carry `reqwest` or AWS SDK types — and those sites are NOT in scope. The rule
of thumb for every site you touch or leave: within one crate, a Rust error
value travels as a value; only at the port boundary may it become the port's
own vocabulary.

**Out-of-scope sweep guard**: `coffret-format`'s `detail` fields (CBOR/argon2
decode diagnostics over untrusted input) are a separate decision and stay
untouched; `RetryPolicy` keeps its shape; no port variant other than `Io`
changes.

Conventions per `CLAUDE.md` and the repo's error-type rules: no `PartialEq`
on error types; a test per variant a caller matches on (the new/changed
variants each get one, in the existing unit-test style of the file they live
in); `make check` as the gate; English throughout; self-contained commit and
PR text. Logging rules per `coffret-logging` hold: log lines keep rendering
causes via `Display` — carrying the value does not change what is logged.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The port's `Io` variant carries the OS error as a value:
      `Error::Io { cause: Arc<io::Error> }` (check gates: the first `Io {`
      block in `error.rs` contains `cause` within 4 lines, and
      `detail: error.to_string()` is gone from that file), `From<io::Error>`
      still works, `source()` returns the cause, and `is_retryable`'s answer
      for `Io` is unchanged (existing retry tests pass unmodified in meaning).
- [x] The Drive token-cache errors carry their observed causes:
      `TokenCache { path, cause: io::Error }` (check gate: the `TokenCache {`
      block contains `cause` within 8 lines), the `serde_json::Error` from
      encoding tokens travels in its own variant (check gate:
      `serde_json::Error` appears in the gateway's `error.rs`), and no
      `oauth/` site stringifies an error into a `detail` any more (check gate:
      `detail: error.to_string()` absent under `oauth/`).
- [x] `Display` output for every changed variant still names the same facts
      (path, operation) plus the cause's own message, and `source()` chains
      are wired — `RUSTDOCFLAGS=-Dwarnings cargo doc` and `make check` are
      clean, with each new/changed variant covered by a unit test that
      matches on it.
- [x] No error type gained `PartialEq` (check gate on both error files), and
      the port's provider-text variants (`Transport`, `Timeout`, `Rejected`,
      `RateLimited`, `ServiceUnavailable`, `MalformedResponse`) kept their
      `detail: String` shape — the boundary translation sites are untouched.

## Out of scope

- **`coffret-format`'s `detail` fields** — decode diagnostics over untrusted
  input; whether those should carry library error values is a separate
  decision.
- **The gateway→port boundary translations** (`translate_transport`, status
  matching, `SdkError` matching) and every port variant except `Io`.
- **`RetryPolicy`'s shape** and all retry classification.
- **Any control-flow change**; any rename beyond the variants this task
  introduces.
