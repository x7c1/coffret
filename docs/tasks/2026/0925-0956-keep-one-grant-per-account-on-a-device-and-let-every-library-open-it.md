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
branch: task/0925-0956-keep-one-grant-per-account-on-a-device-and-let-every-library-open-it
created_at: 2026-09-25T09:55:31Z
updated_at: 2026-09-25T15:17:04Z
---

# feat(device): keep one grant per account on a device, and let every Library open it

## Overview

The register now says a grant belongs to a device and an account (spec: SA-8,
SA-9), that each Library holds the account-cache key in an envelope under its
own purpose key, and that the person names an account on this device. Make
the device layer and the CLI do that.

### 1. Layout and sealing

- `<state>/coffret/accounts/<account-name>/` holds `token-cache.cftc` (the
  KD-10 form, sealed under the account-cache key) and a small settings file
  naming the OAuth client id the grant was issued to.
- Each Library directory holds `account.cfke`: the account-cache key wrapped
  under the Library's purpose key for it (spec: KD-4, the new purpose
  string), in the byte form the register states, with the account name as
  associated data. The Library's settings record the account name.
- `drive::token_cache` opens the account's cache with the key unwrapped from
  the Library's envelope; the gateway's `TokenCache` is unchanged. Count every
  caller of `token_cache` / `grant` and change all of them.
- On opening the accounts directory, a cache no Library envelope can open is
  removed (say how the device knows: every Library's settings name their
  account, so an account no Library names is orphaned).

### 2. Commands

- `init` / `join` gain `--account <name>` — optional while the device holds
  one account (that one is used, or created under the name `default` when
  none exists), required when a second exists. A name that is not a plain
  directory name is refused with a sentence that says what a name may be.
- `join`: before consent, try each account the device holds and take the
  one whose Drive lists the named app folder (spec: SA-8's sub-bullet on
  join); consent only when none does, then name the account.
- `authorize --account <name>` (or `--library <name>`, which resolves to
  its account) renews that one cache; every Library referencing the account
  uses the new token from its next run.
- A Library whose recorded client id differs from the account's is refused
  with a sentence saying the two clients differ and which one each names.

### 3. Migration

A Library found with `token-cache.cftc` and no `account.cfke` is promoted
the first time it is opened (spec: SA-8's migration sub-bullet): the cache is
opened under the Library's token-cache purpose key, re-sealed under a fresh
account-cache key into the account `default` (or the account the Library
already names), the envelope written, the old file removed. Pin it with a
test that lays out the old form and opens the Library.

### 4. Tests and the register's forms

SA-8 and SA-9 are Form: test — add the tests that sample them: two Libraries
of one account hold one cache; either Library's Passphrase opens it; removing
one Library leaves the cache open to the other and removing both leaves it
unreadable and then removed; a second account requires a name; `join` finds
the account holding the folder without consent; the envelope does not open
under another Library's key or another account's name. The device-local
account name never reaches a diagnostic event: a test captures logs across
`init`, `join`, `authorize` and an open with two accounts and asserts the
names are absent (spec: EL-1).

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes
- [x] a device keeps one sealed cache per account under a random key, each
      Library holds that key in an envelope under its own purpose key, and
      the tests above pin sharing, removal and the envelope's binding
- [x] `init` / `join` / `authorize` take `--account`, with the optional /
      required rule and the sentences for a bad name and a client mismatch
      pinned
- [x] `join` takes the account that holds the folder without consent, pinned
      against a stub transport
- [x] the previous per-Library cache is promoted on first open, pinned
- [x] no device-local account name reaches a diagnostic event, pinned

### Manual / on-hardware (verified by a human before merge)

- [ ] `make drive-round-trip-it` is green with the two test Libraries sharing
      one consent

## Out of scope

- Renaming an account after it is named: the name is bound into every
  envelope of the account (spec: SA-9), so a rename is a command that
  re-seals them, not a directory rename by hand. Its own change; `--help`
  says a name cannot be changed yet
- Any change to the consent flow, the scope, or the grant verification
