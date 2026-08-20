---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make s3-store-it && cd backend && cargo fmt --all -- --check && cargo build && cargo test && cargo clippy --all-targets -- -D warnings"
assignee: null
branch: task/0819-1350-object-store-and-adapters
created_at: 2026-08-19T13:50:58Z
updated_at: 2026-08-19T16:00:30Z
---

# feat(backend): add the ObjectStore port with S3 and Google Drive gateway adapters

## Overview

The storage layer starts here. Containers and control objects are stored
on third-party object storage; the format layer is storage-agnostic, and
backends plug in behind a single port. This task defines that port on
the usecase side and lands **both** adapters together, so the
abstraction is not silently shaped by one backend's habits
(pre-generated IDs, MD5, folder concepts, …) — two implementations keep
the port honest for the same reason the TypeScript second
implementation keeps the format spec honest.

Three new crates (first crates in their categories; follow the
workspace layer rules — `domain/` has no I/O, `gateway/` implements
`domain` traits, gateways never depend on each other):

- **`backend/crates/domain/coffret-usecase`** — the port and its
  vocabulary. No use-case orchestration yet (the transfer flow is a
  follow-up task). Defines:

  ```rust
  #[async_trait]
  pub trait ObjectStore: Send + Sync {
      async fn put(&self, name: &str, len: u64, body: impl AsyncRead) -> Result<ObjectRef>;
      async fn reserve_create(&self) -> Result<CommitSlot>;
      async fn put_if_absent(&self, slot: &CommitSlot, name: &str, len: u64, body: impl AsyncRead) -> Result<ObjectRef>;
      async fn get(&self, r: &ObjectRef, range: Option<Range<u64>>) -> Result<ByteStream>;
      async fn list(&self, page: Option<PageToken>) -> Result<(Vec<ObjectInfo>, Option<PageToken>)>;
      async fn trash(&self, r: &ObjectRef) -> Result<()>;
      async fn purge(&self, r: &ObjectRef) -> Result<()>;
  }
  ```

  (Signature sketch — adapt idiomatically, e.g. how the body stream and
  async trait are expressed, without changing the operation set.)

  Semantics the port fixes:
  - `ObjectInfo` carries name / size / mtime / the provider's content
    hash (MD5 on Drive, ETag on S3). `list` is always paginated.
  - `reserve_create` + `put_if_absent` model successor-slot allocation
    and conditional create for the control head; a lost race surfaces
    as a typed `AlreadyExists`, discriminable from transport errors.
  - `trash` is a recoverable soft delete (normal Container removal);
    `purge` is irreversible (old-epoch control objects after Master Key
    rotation). A `purge` is only successful when a read-back confirms
    the object is gone.
  - Resumable / multipart upload differences stay inside each
    adapter's `put`.
  - Error design follows the workspace convention: each crate owns
    `Error` / `Result` at its root; gateways translate SDK/HTTP errors
    into the usecase vocabulary; retryable failures (429, rate-limit
    403, 5xx, timeouts) are distinguishable from permanent ones by
    type, not by string inspection.

- **`backend/crates/gateway/s3-store`** — struct `S3`. The object name
  is the key. `reserve_create` returns the name unchanged (identity);
  `put_if_absent` is a conditional PUT with `If-None-Match: *`.
  `trash` moves the object under a reserved `trash/` key prefix
  (copy + delete); `purge` deletes outright and read-backs. Integration
  tests run against MinIO via a self-contained `make s3-store-it`
  target (starts MinIO in Docker, runs the suite, tears down), and a
  new CI job runs that target on every push.

- **`backend/crates/gateway/google-drive-store`** — struct
  `GoogleDrive`, scope **`drive.file` only**. OAuth 2.0 authorization
  code + PKCE with a loopback redirect (`127.0.0.1`, ephemeral port);
  refresh-token auto-refresh with a single retry on 401.
  `reserve_create` = `files.generateIds`; `put_if_absent` =
  `files.create` with the pre-generated ID; `put` = resumable upload
  session, and the server-reported MD5 is compared against the locally
  computed one after completion; `trash` / `purge` map to Drive's
  trash and permanent delete; `list` uses `files.list` pagination.
  CI never touches the real API: unit tests inject a stubbed HTTP
  transport (constructor DI, no `cfg` switching) and exercise the
  error translation and retry classification, including injected 429 /
  5xx / timeout. Real-API behavior is verified manually (below).

- **Conformance suite**: one backend-agnostic test suite exercises the
  `ObjectStore` contract (the operation × state matrix in the criteria
  below). It runs against MinIO in CI, and the same suite is runnable
  against a real Drive account via an env-gated entry point for the
  manual checks.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The three crates exist in their categories (`domain/coffret-usecase`,
      `gateway/s3-store`, `gateway/google-drive-store`) and the
      workspace builds them; both gateways depend on `coffret-usecase`
      and not on each other (the dependency graph compiles only in the
      allowed direction).
- [x] `make s3-store-it` runs the conformance suite against MinIO and
      passes, covering at least:
      - `put` then `get` round-trips content, including a zero-length
        object and a ranged read.
      - `get` on a missing object returns the typed not-found error.
      - `put_if_absent` on a free slot succeeds; a second
        `put_if_absent` on the same slot/name returns the typed
        `AlreadyExists`.
      - `list` on an empty store returns an empty page and no token;
        with more objects than one page, pagination walks the full set
        exactly once.
      - `trash` removes the object from `list` output; `purge` after
        `trash` (and on a live object) leaves the object gone on
        read-back; `purge` of an already-gone object succeeds
        (idempotent, for rotation retries).
- [x] The S3 adapter's `put_if_absent` sends `If-None-Match: *` and a
      concurrent/duplicate conditional PUT surfaces as `AlreadyExists`
      (asserted against MinIO in the suite).
- [x] The Drive adapter's retry classification is unit-tested with an
      injected stub transport: 429 / rate-limit 403 / 5xx / timeout map
      to typed retryable errors, 4xx otherwise to permanent ones, and a
      401 triggers exactly one token refresh before failing.
- [x] The Drive adapter's `put` verifies the server-reported MD5
      against the locally computed digest and fails with a typed
      integrity error on mismatch (stub-transport test).
- [x] A new CI job runs `make s3-store-it` on every push, alongside the
      existing jobs.

### Manual / on-hardware (verified by a human before merge)

- [x] The OAuth loopback + PKCE flow completes against a real Google
      account, granting only the `drive.file` scope, and the refresh
      path works on a subsequent run.
- [x] The env-gated conformance suite passes against a real Drive
      account.
- [x] Two concurrent `files.create` calls with the same pre-generated
      file ID: exactly one succeeds and the loser surfaces as the typed
      `AlreadyExists` (the Journal-commit conflict primitive).

## Out of scope

- The transfer flow (upload pipeline, Keyring replication, Journal
  commit, download/verify) and any `Interactor` — the next task, built
  on this port.
- Encrypting the OAuth token cache at rest (needs a key-derivation
  register entry and master-key wiring; lands with the transfer flow).
  Until then the token cache is written with mode 0600.
- A wiring crate (`libs/`): nothing consumes the port yet; wiring
  arrives with the first bin that does.
- Trash retention / restore tooling and orphan cleanup.
- Pack size targets and per-provider hard object-limit surveys.
