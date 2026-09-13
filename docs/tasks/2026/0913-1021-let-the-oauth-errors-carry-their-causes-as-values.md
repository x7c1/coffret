---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [error-type-design, completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rq "Error::Authorization" backend/crates/gateway/google-drive-store/src && grep -rq "RedirectTimedOut" backend/crates/gateway/google-drive-store/src && grep -rq "GrantWithoutRefreshToken" backend/crates/gateway/google-drive-store/src && grep -rq "RedirectWithoutState" backend/crates/gateway/google-drive-store/src && grep -rq "WrongTokenCacheKey" backend/crates/gateway/google-drive-store/src && grep -rq "warn!" backend/crates/gateway/google-drive-store/src/oauth/authorization'
assignee: null
branch: task/0913-1021-let-the-oauth-errors-carry-their-causes-as-values
created_at: 2026-09-13T10:21:20Z
updated_at: 2026-09-13T11:23:11Z
---

# refactor(backend): let the OAuth errors carry their causes as values

## Overview

Four findings share one rule: **an error's type is
where its cause belongs.** A `String` a caller cannot match on, a variant whose
documentation does not cover what it carries, and a refusal that records
nothing are the same defect seen from three sides. All four live in
`backend/crates/gateway/google-drive-store/`.

### 1. `Error::Authorization { detail: String }` is a catch-all

Four unrelated failures share one variant and differ only by a sentence the
caller cannot branch on:

- `oauth/authorization/loopback_redirect.rs:86` — the provider answered the
  redirect with its own `error=` parameter.
- `oauth/authorization/loopback_redirect.rs:97` — the redirect did not carry
  the state this flow sent, which is the CSRF check failing.
- `oauth/authorization/mod.rs:107` — no redirect arrived inside
  `REDIRECT_TIMEOUT`.
- `oauth/authorization/mod.rs:176` — the grant carries no refresh token.

Replace the variant with four that each name what happened:
`ProviderRefusedAuthorization` carrying the provider's own refusal,
`RedirectWithoutState`, `RedirectTimedOut { after: Duration }`, and
`GrantWithoutRefreshToken`. The fourth is the one a person is most likely to
meet — a re-authorization where the provider withholds the refresh token
because one was already issued — and it reads as an authorization failure in
general today.

The three that already have their own variants next door
(`GrantNotDriveFileAlone`, `LoopbackRedirect`, `MalformedRedirect`) show the
shape to follow: the value carries the fact, `Display` composes the sentence.

Keep each new variant where `Authorization` sits today in the `is_retryable`
and port-mapping matches. None of the four becomes retryable.

### 2. `TokenCacheDefect::Sealed` does not cover what it can carry

`TokenCacheDefect::Sealed`'s documentation (`error.rs:222-224`) says the sealed
form could not be opened because "another Master Key wrote it, its bytes have
been edited, or it was never a sealed cache at all". But `decode_token_cache`
calls `token_cache_key(key)` first, and that returns
`coffret_format::Error::WrongPurposeKey` when the key it was handed was derived
for another purpose (`purpose_key.rs:54`, spec: DK-4). That is a fact about the
*caller*, not about the file, and it reaches `TokenCache::load` as
`MalformedTokenCache` — a verdict about a file that is very likely fine.

No in-repo caller can reach it today, which is why this is a classification
defect rather than a bug: the path exists and the next caller inherits it.
Separate the two before the defect is built — a wrong purpose key is not a
malformed cache — so the type says which of the two happened. A new
`TokenCacheDefect` variant or an earlier branch in `load.rs` both satisfy this;
choose whichever keeps `Display` honest for both.

### 3. Record what the port sees when a cache is malformed

`error.rs:438` maps `MalformedTokenCache` to the port's `Io`, which changes the
port-level `Display` prefix from "Storage rejected the credentials:" to "local
transfer failed:". That is the right answer — the file is this machine's own,
and the comment above it says so at length — and the HTTP surface and
`is_retryable` do not change. What is missing is a test that says so, so the
next reader of that comment does not have to take it on trust. Add one that
pins the port-level rendering and the retryability.

### 4. A refused grant leaves no trace

`oauth/authorization/mod.rs:157-174` refuses a grant that is not `drive.file`
alone, and returns without recording anything. The provider answered `200`, so
`token_endpoint.post`'s `warn!` never runs, and the `authorization` module has
no logging of its own. The crate's convention is that a failure originating at
the remote is recorded, and this is one — a person who authorized the wrong
thing gets a refusal at the terminal and nothing in the log to read afterwards.

Record it. The granted scope set is the provider's own public identifier and
carries no private location, so `GrantedScopes`' existing `Display` may be
named in the event (spec: EL-1, EL-5). Nothing else about the grant may be.

## Out of scope

`s3-store::error::translate` and `coffret-sqlite-index::error::translate`
collide with the verb `docs/spec/entry-path/` reserves for turning an Entry
Path into a local path. That is a real finding and a separate rule — the
register owns a word — and renaming the family reaches 10 files in two crates
that this task does not otherwise open. It is filed as its own task.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `Error::Authorization` no longer exists in `google-drive-store`; the four
      failures it carried are `ProviderRefusedAuthorization`,
      `RedirectWithoutState`, `RedirectTimedOut` and `GrantWithoutRefreshToken`,
      each matched on by name in the tests that exercise its path.
- [x] A test asserts that a redirect arriving without the state this flow sent
      is refused as `RedirectWithoutState` and caches nothing.
- [x] A wrong purpose key handed to `TokenCache::load` is reported as its own
      kind rather than as a malformed cache, and a test names both outcomes so
      the two cannot collapse again.
- [x] A test pins the port-level rendering and the retryability of a malformed
      token cache, so the `Io` mapping's stated intent is checked rather than
      only described.
- [x] Refusing a grant that is not `drive.file` alone records a diagnostic
      event naming the scope set that was granted, asserted by a test.
- [x] The event carries no part of the grant besides the scope set — no
      refresh token, no authorization code, no cache path.

### Manual / on-hardware (verified by a human before merge)

- [ ] Nothing here needs live Drive: every path above is driven by the crate's
      existing fakes. A real `coffret authorize` against Drive is covered
      elsewhere and is not this task's gate.
