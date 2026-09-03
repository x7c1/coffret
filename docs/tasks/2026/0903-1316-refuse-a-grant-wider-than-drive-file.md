---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && ! grep -q 'any(|granted|' backend/crates/gateway/google-drive-store/src/oauth/authorization/mod.rs && grep -rq 'refuses_a_grant_wider_than_drive_file' backend/crates/gateway/google-drive-store/src/oauth && grep -rq 'refuses_a_grant_without_drive_file' backend/crates/gateway/google-drive-store/src/oauth && grep -rq 'refuses_a_grant_that_names_no_scope' backend/crates/gateway/google-drive-store/src/oauth && grep -rq 'accepts_drive_file_however_it_is_spelled_out' backend/crates/gateway/google-drive-store/src/oauth"
assignee: null
branch: task/0903-1316-refuse-a-grant-wider-than-drive-file
created_at: 2026-09-03T04:16:23Z
updated_at: 2026-09-03T05:13:13Z
---

# fix(google-drive-store): refuse a grant wider than `drive.file`

## Overview

The Drive gateway asks Google for exactly one permission, `drive.file`,
and its crate, module, and constant docs all promise that the grant it
keeps reaches nothing else in the account. The check that is supposed
to hold that line does not. In
`backend/crates/gateway/google-drive-store/src/oauth/authorization/mod.rs`,
`Authorization::exchange` reads the token response's `scope` and only
asks whether `drive.file` is **among** the granted scopes
(`scope.split(' ').any(...)`). A response of `drive.file drive` — a
grant over the whole account — passes and its refresh token is sealed
into the token cache. A response with no `scope` field at all is not
examined. The comment beside the check ("a grant that reaches more of
the account than coffret needs is one to refuse rather than to cache")
states the invariant; the code beneath it does not enforce it.

Google's ordinary endpoint does not widen a grant the client did not
ask for, so this is not exploitable today. It matters because the
least-privilege promise is written down in three places and read by
the owner as a reason to trust the token cache: a refresh token is a
bearer credential for the whole Library, and the design keeps it from
being a bearer credential for the whole Google account by *asking* for
less. That only holds if what was *granted* is verified, not assumed.

Make the check decide the granted set exactly:

1. **Read the granted set the way the contract defines it.** RFC 6749
   §3.3 makes `scope` a space-delimited, case-sensitive list; §5.1
   makes it OPTIONAL only when identical to what was requested, and
   Google's token response documents it as always present. Parse the
   field into a set: split on whitespace (tolerate repeated spaces and
   surrounding whitespace, which are delimiters, not scopes; keep the
   comparison case-sensitive), and drop repeats — `drive.file
   drive.file` is the same set as `drive.file`.
2. **Accept only the exact set `{drive.file}`.** A granted set with any
   other member is refused and nothing is cached: an additional scope
   (`drive.file drive`, `drive.file openid`), a missing `drive.file`
   (`drive` alone, an empty string), and — the decision this task
   makes — an absent `scope` field. Absence is refused rather than read
   as "identical to the request": the invariant is that the granted
   set was *verified* to be exactly `drive.file`, and silence from the
   endpoint does not verify anything. Google always sends the field, so
   the only cost is that a provider which stops sending it makes
   `coffret authorize` fail closed with a refusal that says why. State
   this reasoning in the code where the decision lives.
3. **Put the decision in one place.** Give the granted set a home of its
   own — a small type or module beside `TokenResponse` under
   `oauth/` that parses the raw `scope` text and answers "is this
   exactly `drive.file`" — with its own unit tests, and have
   `exchange` call it. The existing `any(...)` check goes away (the
   check command pins its absence).
4. **Refuse with structure, not a formatted string.** The refusal is a
   `coffret-google-drive-store` error the owner sees from
   `coffret authorize`; it should name what was granted so the owner
   can see the account-wide scope on the consent screen they clicked
   through. Prefer a dedicated variant that carries the granted scopes
   as data (with `Display` composing the message and `NotAuthorized` /
   `Unauthenticated` mapping unchanged) over stuffing the list into
   `Error::Authorization { detail }`. Scopes are not secrets; the
   access and refresh tokens in the same response are, and must not
   appear in the message.
5. **Tests that fix the cases.** Beside the exact-set type, unit tests
   for each case named in 2; and at `exchange` level, drive the flow
   through `StubTransport`
   (`backend/crates/gateway/google-drive-store/src/http/stub_transport.rs`)
   with a scripted token response and assert that a wider grant, a
   grant without `drive.file`, and a response without `scope` each
   produce the refusal **and leave the token cache empty**, while an
   exact grant (also spelled with repeats and extra whitespace) is
   cached. `Authorization` keeps its `token_endpoint` private; add
   whatever test-only constructor the exchange tests need. Use these
   test names, which the check command greps for:
   `refuses_a_grant_wider_than_drive_file`,
   `refuses_a_grant_without_drive_file`,
   `refuses_a_grant_that_names_no_scope`,
   `accepts_drive_file_however_it_is_spelled_out`.
6. **Say what is now true.** The crate doc (`src/lib.rs`), `oauth/mod.rs`,
   the `DRIVE_FILE_SCOPE` doc in `oauth/token_endpoint.rs`, and the
   comment in `exchange` describe the request side ("asks for
   `drive.file` and nothing else"). Extend them to the grant side: what
   is granted is verified to be exactly `drive.file`, and a wider grant
   is refused and never cached. Do not overstate it — this is a bearer
   credential for every object in the Library, and the concept docs
   already say so; the claim this task adds is only that it is not a
   credential for the rest of the account.

The refresh path (`oauth/oauth_tokens.rs`) is unchanged: a refresh
token cannot widen the grant it came from, and the grant was verified
when the token was cached.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The granted scopes of a token response are parsed as a
      whitespace-delimited, case-sensitive set, and `exchange` caches
      a refresh token only when that set is exactly `{drive.file}`;
      the old `any(...)` containment check is gone from
      `authorization/mod.rs` (grep gate).
- [x] A response whose grant is wider than `drive.file`, one whose
      grant lacks `drive.file`, and one with no `scope` field are each
      refused with a structured error naming the granted scopes, and
      the token cache stays empty afterwards (tests
      `refuses_a_grant_wider_than_drive_file`,
      `refuses_a_grant_without_drive_file`,
      `refuses_a_grant_that_names_no_scope`; grep gates).
- [x] An exact grant is accepted whether spelled once, repeated, or
      padded with extra whitespace (test
      `accepts_drive_file_however_it_is_spelled_out`; grep gate).
- [x] No token from the response appears in the refusal's `Display`
      text, and `make check` passes.

### Manual / on-hardware (verified by a human before merge)

- [ ] `coffret authorize` against the real Google endpoint still
      completes and caches the grant (the exact-scope check accepts
      Google's actual `scope` answer for a `drive.file` consent).

## Out of scope

- Verifying scope on the refresh path — the refresh token is bound to
  the grant verified at exchange and cannot widen it.
- Querying Google's `tokeninfo` endpoint to cross-check the grant: the
  token response is the contract's own statement of what was granted,
  and a second round trip adds nothing the exact-set check does not
  already decide.
- Incremental authorization (`include_granted_scopes`) — coffret never
  sets it and has no second scope to add.
