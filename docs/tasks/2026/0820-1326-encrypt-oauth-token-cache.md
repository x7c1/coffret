---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "cd backend && cargo fmt --all -- --check && cargo build && cargo test && cargo clippy --all-targets -- -D warnings && grep -q 'coffret/v1/token-cache' ../docs/spec/key-derivation/README.md"
assignee: null
branch: task/0820-1326-encrypt-oauth-token-cache
created_at: 2026-08-20T13:26:45Z
updated_at: 2026-08-21T00:05:00Z
---

# feat(backend): encrypt the OAuth token cache under a derived purpose key

## Overview

The Google Drive adapter keeps its OAuth refresh token in a cache file on
disk, currently written as plaintext at mode 0600 — the one thing coffret
writes that is not encrypted. That was a deliberate deferral when the adapter
landed, recorded in its task's "Out of scope"
(`docs/tasks/2026/0819-1350-object-store-and-adapters.md`), because encrypting
it needs a key-derivation register entry and Master Key wiring. This task
closes that deferral.

The exposure is not theoretical: a refresh token is a bearer credential for
every object the app created. Anyone who reads the file can mint access
tokens and fetch the whole Library's ciphertext without touching the device
again, and can keep doing so until the grant is revoked. That is a strictly
larger window than reading the ciphertext itself, which stays useless without
the Master Key.

Encrypt the cache under a purpose key derived from the Master Key, following
the existing purpose-key machinery rather than inventing a second one:

- **`docs/spec/key-derivation/README.md`** — add `coffret/v1/token-cache` to
  the KD-4 registry table. It is the first purpose whose derived key protects
  device-local state rather than a Storage Object, so say that in the table's
  "derived key encrypts" cell plainly ("the OAuth token cache on this
  device"). KD-8 is unaffected and stays true: the cache never reaches
  Storage.
- **`docs/concepts/purpose-key/README.md`** — the concept currently reads as
  if every purpose key opens a control Storage Object ("Purpose keys open
  control Storage Objects directly…"). Widen the Domain Rules so a purpose
  may also protect state that never leaves the device, without weakening the
  one-key-one-job rule. Keep the change minimal — this is a widening, not a
  rewrite.
- **`backend/crates/domain/coffret-format/src/purpose.rs`** — add
  `Purpose::TokenCache` with info string `coffret/v1/token-cache`, and extend
  the `ALL` array the registry tests iterate so the uniqueness test covers it.
  Note that `Purpose::of_control_object` must stay exhaustive over
  `ControlObjectKind` only — the new purpose has no control-object kind, and
  nothing should invent one for it.
- **`backend/crates/domain/coffret-format/src/token_cache/`** — the codec for
  the sealed file, as a sibling of `stored_master_key/`, which is the same
  thing one layer over: a self-describing, device-local, encrypted blob. The
  AEAD framing and the byte layout belong next to the rest of the format
  layer, not inside a gateway.
- **`backend/crates/gateway/google-drive-store/src/oauth/token_cache/`** — the
  gateway calls that codec and otherwise keeps doing file I/O, permissions,
  and OAuth. It gains a `coffret-format` dependency (gateway → domain is the
  allowed direction) and a Master Key on the path that builds the store, so
  the derivation happens where the key already is rather than by passing a raw
  derived key around.

Cache-file requirements:

- Encrypt with XChaCha20-Poly1305 under the derived purpose key and a fresh
  random nonce per write, matching how every other coffret AEAD message is
  built (spec: FM-1).
- Make the file self-describing: a magic and a format version ahead of the
  nonce, bound as associated data, so a file from another tool or a future
  version is rejected rather than misread. KD-9's stored-Master-Key layout is
  the precedent to follow for shape and for the rejection rules; this file
  needs no Argon2id parameters because its key comes from the Master Key, not
  from the Passphrase.
- Keep mode 0600 — encryption does not make the permission bits pointless.
- A cache that fails to decrypt or authenticate — tampered, truncated,
  written under a different Master Key, or a leftover plaintext file from
  before this change — is reported as `Error::MalformedTokenCache`, which the
  existing translation already turns into
  `coffret_usecase::Error::Unauthenticated`. The caller then runs the
  authorization flow again. Do not silently fall back to "no cached token",
  and do not attempt to migrate a plaintext file: an unreadable credential
  store is a fact worth reporting, and nothing has shipped that would need a
  migration path.

The local Index stays plaintext by design and is not in scope here: it is a
rebuildable cache of paths and digests whose confidentiality rests on disk
encryption, whereas the token cache is a credential that grants access to
Storage. Encrypting the one and not the other is the design's intent, not an
oversight.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `Purpose::TokenCache` derives under the info string
      `coffret/v1/token-cache`, and the existing KD-4 registry tests —
      `info_strings_match_the_registry` and `every_purpose_has_its_own_info_string`
      — cover it through the extended `ALL` array.
- [x] The KD-4 registry table in `docs/spec/key-derivation/README.md` lists
      `coffret/v1/token-cache` (the check command greps for it) and names what
      it encrypts.
- [x] Writing a cache holding a known refresh token produces a file whose
      bytes contain neither that token nor the access token — asserted by
      searching the written bytes for the token strings.
- [x] A cache written and then read back under the same Master Key yields the
      same stored tokens — the round trip is asserted over the whole stored
      value, so any field it carries is covered.
- [x] A cache read back under a *different* Master Key fails with
      `Error::MalformedTokenCache` rather than yielding tokens or panicking.
- [x] Flipping a byte anywhere in a written cache — in the header, the nonce,
      the ciphertext, or the tag — makes the read fail with
      `Error::MalformedTokenCache`. Cover each of those four regions, not just
      one.
- [x] A file with an unknown magic or an unknown format version — including a
      leftover plaintext JSON cache — is rejected as
      `Error::MalformedTokenCache` and is not parsed as tokens.
- [x] `Error::MalformedTokenCache` still converts to
      `coffret_usecase::Error::Unauthenticated`, so a caller of the port sees
      "re-authorize", not "the disk broke".
- [x] The cache file is created with mode 0600.

### Manual / on-hardware (verified by a human before merge)

- [ ] Against a real Google account: run the authorization flow once, confirm
      the written cache file is not readable as JSON, then run the adapter
      again and confirm it refreshes from the encrypted cache without
      prompting for authorization a second time.

## Out of scope

- The transfer flow itself (`Interactor`, upload pipeline, download path) —
  separate tasks.
- Retry and backoff policy, and the error-classification work that goes with
  it — the next task.
- Encrypting the local Index, or any other device-local state (see above).
- Passphrase entry, locking, and idle re-locking of the Master Key on a
  device (the `DK` mechanism). This task takes the Master Key as already
  unlocked and available to whoever constructs the store.
