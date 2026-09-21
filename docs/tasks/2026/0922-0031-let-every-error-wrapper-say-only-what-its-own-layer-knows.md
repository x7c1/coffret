---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [error-type-design, completeness, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && grep -qF 'Self::EntropyUnavailable { .. } => f.write_str(\"could not draw random bytes\")' backend/crates/domain/coffret-format/src/error.rs && ! grep -qE 'a call did not become a usable answer: \\{error\\}' backend/crates/gateway/google-drive-store/src/error.rs"
assignee: null
branch: task/0922-0031-let-every-error-wrapper-say-only-what-its-own-layer-knows
created_at: 2026-09-21T15:31:15Z
updated_at: 2026-09-21T18:23:14Z
---

# refactor: let every error wrapper say only what its own layer knows

## Overview

A wrapper that renders its cause's sentence inside its own `Display` line
*and* returns that cause from `source()` makes every caller printing the chain
say the same thing twice. `coffret-device`'s `src/error.rs` states the rule at
the top of the file — a wrapper says which layer the failure belongs to, and
the typed cause carries the lower layer's own answer — and a recent change
brought the three flow enums (`SyncError`, `FreezeError`, `FetchError`), their
`Io` variants and the shared `LocalIoError` to it. Every other error type in
the workspace still predates the rule.

Finish the sweep. This is one rule applied across one workspace; nothing about
what fails or what a caller can branch on changes.

### What to change

For each type below, find every `Display` arm that renders a value the same
type's `source()` also returns, and remove the rendering. Keep whatever the
wrapper contributes that the cause does not — which operation it was, which
step, which status code, which path-shaped fact the type already chose to
carry. Drop only the duplicate.

1. `backend/crates/gateway/google-drive-store/src/error.rs` — the largest, and
   the one with the most callers. Its `source()` hands back `cause` for
   `HttpClient`, `TokenCache`, `MalformedTokenCache`, `UnencodableTokens`,
   `UnsealableTokenCache`, `UnreadableTokenResponse`, `LoopbackRedirect`,
   `MalformedRedirect`, `Transport`, `EntropyUnavailable`,
   `AppFolderNotCreated` and `AppFolderUnreadable`. Variants whose content the
   code composed itself — `TokenEndpoint`'s and `CodeExchangeWithoutSecret`'s
   `detail`, and everything `source()` answers `None` for — are not
   duplicates; leave them rendering what they render.
2. `backend/crates/domain/coffret-usecase/src/commit/commit_error.rs`.
3. `backend/crates/apps/coffret-device/src/error.rs` — the crate that states
   the rule, so its own arms have to follow it.
4. `backend/crates/domain/coffret-format/src/error.rs`.
5. `backend/crates/libs/coffret-logging/src/error.rs`.
6. `backend/crates/domain/coffret-usecase/src/error.rs`,
   `index_error.rs`, `descent_error.rs`.
7. `backend/crates/domain/coffret-usecase/src/root_marker.rs` — this one
   carries `FetchError::RefusedRoot`'s double print with it. `RefusedRoot`
   renders the refusal transparently while `source()` returns the refusal's
   own `cause`, and `RootRefused`'s `Display` embeds that cause in
   parentheses. Fixing the embedding here fixes the chain there; do not reach
   into `fetch_error.rs` to compensate. Read the comment on `RefusedRoot`
   first: it says the transparent rendering is deliberate, so that a device
   placing one file bears the same words as the state it raises (spec: EP-13).
   That decision stands — only the parenthesised cause goes.

### What to be careful about

- **`redacted()` must be byte-for-byte unchanged in every type.** It is a
  separate rendering for diagnostic events, bound by its own rules. Only the
  human-facing `Display` changes.
- **Tests across layers assert composed strings.** `coffret-device`'s
  `src/error.rs` has at least one test (near line 1497 before your change)
  asserting a sentence that contains the format crate's rendering inside it.
  Search the workspace for each sentence you change and update every
  assertion; do not weaken an assertion to `contains` to avoid updating it.
- **Where a wrapper has nothing of its own to say** once the cause is removed,
  give it a sentence naming its layer rather than leaving it empty — the same
  shape the flow enums took. Match their voice; read
  `sync/sync_error.rs`'s `Display` first.
- The CLI and the server both print `{error:#}`, so walk one failure per crate
  through to the terminal and confirm the chain still carries everything a
  person needs, in an order that reads.

### What to add

A chain test per type, following the `chain()` helper the flow enums use:
render `{error:#}` for one wrapping variant and assert the lower layer's
sentence appears exactly once. Put each test beside the type it covers.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes
- [x] no `Display` arm in the seven types renders a value the same type's
      `source()` returns
- [x] `coffret-format`'s entropy failure and the Drive gateway's transport
      failure no longer render their cause in their own line
- [x] each of the seven types has a test asserting the rendered `{error:#}`
      chain says the lower layer's sentence once

### Manual / on-hardware (verified by a human before merge)

- [ ] `make e2e-it` is green
- [ ] one failure per crate was walked to the terminal and the chain reads

## Out of scope

- Any change to what a variant carries, what it is named, or what a caller can
  branch on. This is a rendering change
- `redacted()` in any type
- `From<google_drive_store::Error>`'s `detail = error.to_string()` in
  `coffret-usecase` — flattening a cause into a string at a boundary is a
  different defect with a different fix, and it gets its own change
- Re-exporting `ControlObjectKind`, `SourceChange` or `ControlObjectFault`
  from `coffret-device`
- The three places that raise `LengthMismatch`
