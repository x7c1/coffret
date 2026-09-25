---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check"
assignee: null
branch: task/0925-0955-say-in-the-concepts-and-the-register-that-a-grant-belongs-to-a-device-and-an-account
created_at: 2026-09-25T09:55:31Z
updated_at: 2026-09-25T13:23:35Z
---

# docs: say in the concepts and the register that a grant belongs to a device and an account

## Overview

A Storage grant — the refresh token a consent leaves behind — is today kept
per Library, sealed under that Library's Master Key (spec: KD-10, SA-6). But
what the grant reaches is decided by the account and the OAuth client, not by
the Library: with one client per device, two Libraries of the same account
hold two tokens with identical reach, and SA-7's "a bearer credential for
every object the Library has" understates it. This change moves the grant's
home in the documents and the register only; the device code follows in a
separate change. Nothing here changes behaviour.

### The shape to describe

- A grant belongs to **a device and an account**. A device keeps one sealed
  cache per account, under a random *account-cache key*.
- A Library that uses an account holds that key wrapped under its own
  `coffret/v1/…` purpose key (spec: KD-4) — an *account-cache key envelope*,
  one per Library, in the Library's directory. Unlocking any one Library
  opens the account's cache; a Library's directory still holds everything
  that device keeps for it; the last envelope removed leaves the cache
  unreadable, and the device discards such a cache when it next opens the
  accounts directory.
- coffret cannot see which account granted (the scope is `drive.file`
  alone, SA-3), so the person names the account on this device — a
  **device-local account name**, like the device-local Library name: chosen by
  the person, never written to Storage, never in a diagnostic event. It is
  optional while the device holds one account and required when a second is
  added. One account name binds to one OAuth client id on a device.
- `join` tries each grant the device holds and takes the account whose Drive
  has the named app folder; consent runs only when none does. `init` with
  more than one account requires the name. Renewal is per account and serves
  every Library that references it.
- A consent's reach is unchanged (SA-3, SA-4); what changes is how many
  copies of the same credential a device keeps (one) and what a Library's
  Passphrase unlocks (the account cache, which reached the whole account's
  app files already).

### What to write

1. **Storage concept** (`docs/concepts/storage/README.md`): a subsection on
   the grant — its home (device and account), the account-cache key, the
   envelope, the device-local account name — and Collocations `renew (an
   account's grant on this device)`, `name (an account, on this device)`;
   revise the existing `renew` gloss and Domain Rule to this shape.
2. **Library concept**: a Library *references* an account by its device-local
   name and holds the envelope; "separate Libraries share nothing" gains the
   qualification that two Libraries of one account share that account's grant
   (they always did on the provider's side).
3. **Register.** SA-8: the grant's home and the one-cache-per-account rule
   (Form: test). SA-9: the account-cache key envelope — what it seals, under
   which purpose key, bound to the account name as associated data, one per
   Library (Form: test). SA-6 / SA-7 reworded for the new home. KD-4's
   registry gains the purpose string the envelope is sealed under. KD-12 (or
   the next free id): the envelope's self-describing byte form, in the shape
   KD-10 uses. EL-1's list of names that never reach a diagnostic event gains
   the device-local account name. DK-6 stays (a Passphrase is still per
   device and per Library).
4. **Migration, stated:** a Library found with the previous per-Library cache
   and no envelope has its cache promoted into an account named `default`
   the first time it is opened; say this in SA-8's sub-bullet as the one-time
   path, so the code change can cite it.

Read the neighbouring rules and the two concept documents before writing so
the voice matches; every new rule id is the next free number in its prefix.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes
- [x] the Storage concept describes the grant's home, the account-cache key,
      the envelope and the device-local account name, with the two
      Collocations, and the Library concept references an account
- [x] SA-8 and SA-9 exist with their forms; SA-6 and SA-7 speak of an
      account's cache; KD-4 lists the envelope's purpose string; a KD rule
      states the envelope's byte form; EL-1 names the device-local account
      name

## Out of scope

- The device layer, the CLI and the migration code: a separate change that
  cites the rules written here
- Any change to the consent flow, the scope asked for, or the verification
  of the grant (SA-1 … SA-5)
