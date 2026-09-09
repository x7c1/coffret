---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "\[Loopback Access\](loopback-access/)" docs/spec/README.md && grep -q "\[Storage Authorization\](storage-authorization/)" docs/spec/README.md && test "$(grep -c "^- \*\*LA-" docs/spec/loopback-access/README.md)" -ge 5 && test "$(grep -c "^- \*\*SA-" docs/spec/storage-authorization/README.md)" -ge 5 && grep -q "LA-" backend/crates/apps/coffret-server/src/authorize/tests.rs && grep -q "LA-" backend/crates/apps/coffret-server/tests/routes.rs && grep -q "LA-" backend/crates/apps/coffret-device/src/server_key.rs && grep -q "SA-" backend/crates/gateway/google-drive-store/src/oauth/authorization/tests.rs && grep -q "SA-" backend/crates/gateway/google-drive-store/src/oauth/granted_scopes.rs && grep -q "SA-" backend/crates/gateway/google-drive-store/src/oauth/authorization/loopback_redirect.rs && grep -q "SA-" backend/crates/gateway/google-drive-store/src/oauth/pkce.rs && grep -q "verified rather than assumed" docs/concepts/storage/README.md && grep -q "SA-4" docs/concepts/storage/README.md && grep -q "LA-" docs/concepts/library/README.md && grep -q "LA-" docs/concepts/passphrase/README.md && grep -q "SA-" backend/crates/gateway/google-drive-store/src/oauth/token_endpoint.rs && grep -q "SA-4" backend/crates/gateway/google-drive-store/examples/authorize.rs && ! grep -rq "(spec: " docs/spec'
assignee: null
branch: task/0909-1507-give-loopback-access-and-the-storage-grant-rule-ids
created_at: 2026-09-09T15:07:52Z
updated_at: 2026-09-09T15:48:41Z
---

# docs(spec): give loopback access and the storage grant rule IDs

## Overview

Two guarantees coffret already keeps have no address anyone can cite.

A server serving a Library listens on loopback alone and refuses every request
that does not carry the key it draws at start and publishes owner-only into the
Library's directory (`backend/crates/apps/coffret-server/src/main.rs`,
`backend/crates/apps/coffret-device/src/server_key.rs`,
`backend/crates/apps/coffret-server/src/authorize/`). Neither the concept
documents nor the specification register mentions it: nothing under
`docs/concepts/` or `docs/spec/` says "server key" or "loopback", and no test in
`authorize/tests.rs`, `tests/routes.rs`, or `server_key.rs` cites a rule ID.

The Google Drive flow verifies, before it caches anything, that the grant it was
given is exactly the one narrow permission it asked for
(`backend/crates/gateway/google-drive-store/src/oauth/authorization/mod.rs`,
`oauth/granted_scopes.rs`, `oauth/token_endpoint.rs`). The register has no rule
that owns this check: `KD` stops at the sealed token cache (KD-10) and the
Recovery Code (KD-11), so neither ID is available. The Storage concept describes
the cached credential without saying what it is not.

Add two mechanisms to the register and register both in the mechanism table of
`docs/spec/README.md`, immediately after the `Device Key Custody` row:

```
| [Loopback Access](loopback-access/) | `LA` | loopback-only listening, the per-run server key and the file it is published in, the admission fences every request passes |
| [Storage Authorization](storage-authorization/) | `SA` | the authorization flow and its PKCE and loopback redirect, the one permission asked for, the grant width verified before anything is cached, what later runs mint |
```

Inside the register a rule cites a sibling rule bare (`KD-4`), never with a
`spec:` prefix; the two new documents follow that. Outside the register a
citation reads `(spec: LA-2)`.

### `docs/spec/loopback-access/README.md` (new)

Follow the layout of `docs/spec/device-key-custody/README.md`. Rules:

- **LA-1.** A server serving a Library listens on the loopback interface alone,
  so the Library's plaintext is never offered to another machine on the network.
  *(Form: prose — a property of the address the binary binds, with nothing for
  a test over the router value to observe; honored by construction and review.)*
- **LA-2.** Reaching that socket is not authorization. Every request is
  authorized before any route sees it, reads exactly as mutations, because what
  a read answers with is the Library. *(Form: test)*
- **LA-3.** The server draws a key as it starts, from the operating system's
  CSPRNG, and writes it owner-only into the Library's own directory on this
  device. A caller shows it in a header. The boundary is therefore the
  operating system's file permissions: a caller that can read the file is a
  process of the account that owns the Library, and a page in a browser is not
  one. *(Form: test)*
- **LA-4.** The key is one running process's. It is drawn afresh at every start
  and nothing carries from one process to the next, so a key that leaked is
  spent when its server stops and a file a killed server left behind opens
  nothing. *(Form: test)*
- **LA-5.** A request is admitted only where it names the authority the server
  actually bound, carries that key, and does not admit to coming from another
  site. A key that is shown and is wrong is answered exactly as no key at all,
  and the refusal names neither the key shown nor the one expected.
  *(Form: test)*
- **LA-6.** The key is never on a URL and never in a cookie: a URL is written
  down in referrers, histories and access logs, and a cookie is attached by the
  browser to requests the page never made — which is what LA-5's third fence
  exists against. *(Form: test)*
- **LA-7.** The shown key is compared against the expected one without stopping
  at the first byte that differs. *(Form: prose — a timing property of one
  comparison, which no test over the verdict can observe; honored by
  construction.)*

Cite the `Form: test` rules from the tests that already verify them, the way
`tests/routes.rs` cites DK-3 today: the module doc of
`coffret-server/src/authorize/tests.rs` and its cases (LA-2, LA-5, LA-6), the
admission block of `coffret-server/tests/routes.rs` (LA-2, LA-5), and the
module doc and three test comments of `coffret-device/src/server_key.rs`
(LA-3, LA-4). The module doc of `coffret-server/src/authorize/mod.rs` names the
rules its three fences implement.

### `docs/spec/storage-authorization/README.md` (new)

Rules, with the test that backs each `Form: test` half named in that test's
comment:

- **SA-1.** Authorization is the authorization-code flow with PKCE: the
  challenge is the SHA-256 of a per-run verifier, the verifier is sent only with
  the token exchange, and the plain challenge method is never used — it would
  put the secret in the very request the challenge protects. *(Form: test —
  `oauth/pkce.rs` and the `code_challenge_method=S256` assertions in
  `oauth/authorization/tests.rs`.)*
- **SA-2.** The redirect comes back to a loopback address on a port the
  operating system hands out, and the flow accepts only a redirect carrying the
  opaque `state` value this run drew. Anything else that arrives on the port is
  answered and ignored rather than mistaken for the redirect, and a redirect
  naming a refusal is reported rather than waited out. *(Form: test —
  `oauth/authorization/loopback_redirect.rs`.)*
- **SA-3.** coffret asks for exactly one provider permission and asks for no
  other: on Google Drive, the one that reaches only files this application
  itself created. Every Storage Object in a Library was written by coffret, so a
  wider permission is access no part of the design uses. *(Form: test —
  `the_authorization_url_asks_for_drive_file_and_nothing_else`.)*
- **SA-4.** The grant is verified before anything is cached. What the token
  endpoint says it granted must be exactly the permission SA-3 asked for and
  nothing besides — not a set that merely contains it, which would wave through
  a grant reaching the whole account. A wider set, a different set, or a
  response naming no scope at all is refused, and nothing reaches the token
  cache. *(Form: test — the `refuses_a_grant_*` and
  `accepts_drive_file_however_it_is_spelled_out` cases in
  `oauth/authorization/tests.rs`, and the unit cases of
  `oauth/granted_scopes.rs`.)*
  - An endpoint that says nothing verifies nothing. RFC 6749 §5.1 permits the
    field to be omitted when the grant equals the request, and refusing that
    silence is deliberate: the cost is that a provider which stopped sending
    the field fails this flow closed, with a refusal that says exactly why.
- **SA-5.** A refusal names what was granted, so the person can go and look at
  the consent they gave, and never names a token. *(Form: test — the refusal
  assertions in `oauth/authorization/tests.rs` and
  `what_was_granted_is_named_in_full` in `oauth/granted_scopes.rs`.)*
- **SA-6.** What the flow leaves behind is one long-lived refresh token in the
  sealed device-local cache (KD-4, KD-10). Later runs mint short-lived access
  tokens from it without a person, keep them in memory only, and cache nothing
  further — so SA-4's check belongs to the one moment something is kept.
  *(Form: test for what is minted and kept — name the case in
  `google-drive-store/src/refresh_tests.rs` that shows it; prose for what the
  provider does with a grant afterwards, which is observable only as that
  provider refusing.)*
- **SA-7.** The credential the cache holds is a bearer credential for every
  object the Library has on that provider, which is why it is sealed under a
  purpose key of its own and never leaves the device (KD-4, KD-10). What SA-4
  keeps it from being is a credential for the rest of that account.
  *(Form: prose — a scoping statement about a credential's reach, honored by
  SA-3 and SA-4 together rather than by an observation of its own.)*

Do not promise more than the code does: the grant check runs on the one-time
authorization exchange only, and `oauth/oauth_tokens.rs::mint` (the refresh
path) reads back an access token without re-checking `scope`. That is sound
because `mint` caches nothing, and SA-6 says so; nothing may imply a check on
every request, and there is no re-authorization path to describe.

### Concept documents

- `docs/concepts/storage/README.md`: append two sub-bullets to the credential
  Domain Rule (the one ending `(spec: KD-4, KD-10)`):
  - The grant behind that credential is verified rather than assumed: the
    provider has to say it granted exactly the one narrow permission coffret
    asked for — on Google Drive, the one that reaches only files this
    application itself created — so the credential reaches the Library's own
    objects and nothing else in that account. A wider grant, a different one,
    or an answer naming no grant at all is refused, and nothing is cached
    (spec: SA-3, SA-4).
  - The check is on the authorization that produces the credential, which is
    the only moment anything is cached. Later runs mint short-lived access
    tokens from what was cached and add no permission to it, so a grant the
    person later narrows or withdraws surfaces as the provider refusing rather
    than as a check here (spec: SA-6).
- `docs/concepts/library/README.md`: a new Domain Rule after "Multiple enrolled
  devices may write to one Library" and its sub-bullets:
  - A Library served for browsing on this device is served to this device
    alone: the server listens on loopback only, and it answers nobody who
    cannot read a file of the owner's that it writes as it starts. Reaching the
    port is not being the owner — the owner's own browser runs other people's
    pages, and a page can aim a request at a loopback port without ever reading
    the answer (spec: LA-1, LA-2, LA-3).
    - The key is one running server's and is drawn again at every start, so
      nothing about it outlives the process that published it (spec: LA-4).
- `docs/concepts/passphrase/README.md`: a sub-bullet under the existing "An
  unlocked Master Key sits in memory in the clear" sub-bullet:
  - While a device serves that Library for browsing, "whoever reaches the
    device" stays what it says: the server answers only a caller that can read
    the owner's own files, not everything that can reach a loopback port
    (spec: LA-2, LA-3).

### Doc comments in the OAuth crate

- `backend/crates/gateway/google-drive-store/src/oauth/token_endpoint.rs`: the
  `DRIVE_FILE_SCOPE` doc comment already says it is what is asked for and
  equally what is accepted; end that paragraph with `(spec: SA-3, SA-4, SA-7)`.
  The constant stays where it is.
- `backend/crates/gateway/google-drive-store/examples/authorize.rs`: the crate
  doc describes only the request side ("asks for `drive.file` alone"). Extend
  it: what comes back is held to the same thing — the token response has to say
  it granted `drive.file` and nothing besides, or the flow refuses and caches
  nothing (spec: SA-3, SA-4); a consent widened at the screen, and an answer
  that names no grant at all, both fail here rather than quietly becoming a
  credential for the rest of the account.

This is a documentation change. No behaviour changes, no test is added, removed
or altered in what it asserts, and no module moves.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The mechanism table registers both new mechanisms, and each new mechanism
      document carries at least five rules under its own prefix.
- [x] The `Form: test` rules of both mechanisms are cited from the suites that
      verify them: the admission tests, the routes admission block, the
      server-key tests, and the PKCE, loopback-redirect, granted-scope and
      authorization-exchange tests.
- [x] The Storage concept says the grant is verified rather than assumed and
      cites the rule that owns the check.
- [x] The Library and Passphrase concepts carry the loopback boundary by rule
      ID.
- [x] The one-permission constant and the authorization example both name the
      grant-side check by rule ID.
- [x] Neither new mechanism document, nor any other file under `docs/spec/`,
      cites a rule with a `spec:` prefix.
- [x] Existing backend, frontend, and interoperability checks continue to pass.

## Out of scope

No behaviour change of any kind: the admission fences, the server key, and the
authorization flow keep their current code. No module moves — the
one-permission constant stays in `token_endpoint.rs`. No new test, and no change
to what an existing test asserts. No `server-key` concept document: the boundary
is stated on Library and Passphrase, where the terms a user reasons about live.
Key custody (DK-7, DK-4, a rule for how a secret is entered), the Passphrase's
in-memory lifetime, and the `Library name` vocabulary are a separate change.
