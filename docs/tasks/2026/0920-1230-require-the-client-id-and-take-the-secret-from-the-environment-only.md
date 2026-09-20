---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && ! grep -q 'std::env::var(CLIENT_ID)' backend/crates/apps/coffret-cli/src/drive_client.rs && ! grep -rq 'client_secret: Option<String>' backend/crates/apps/coffret-cli/src/init.rs backend/crates/apps/coffret-cli/src/join.rs && grep -q 'requires = \"client_id\"' backend/crates/apps/coffret-cli/src/init.rs && grep -q 'requires = \"client_id\"' backend/crates/apps/coffret-cli/src/join.rs && grep -q -- '--client-id \"\\$COFFRET_DRIVE_CLIENT_ID\"' scripts/drive-round-trip-it.sh && grep -q -- '--client-id \"\\$COFFRET_DRIVE_CLIENT_ID\"' scripts/drive-index-layout-it.sh && bash -n scripts/drive-round-trip-it.sh && bash -n scripts/drive-index-layout-it.sh"
assignee: null
branch: task/0920-1230-require-the-client-id-and-take-the-secret-from-the-environment-only
created_at: 2026-09-20T12:30:00Z
updated_at: 2026-09-20T14:50:00Z
---

# feat(cli): require --client-id and take the client secret from the environment only

## Overview

`coffret init` and `coffret join` take the OAuth desktop client two ways:
`--client-id` / `--client-secret` as flags, or `COFFRET_DRIVE_CLIENT_ID` /
`COFFRET_DRIVE_CLIENT_SECRET` from the environment when a flag is not
given (`backend/crates/apps/coffret-cli/src/drive_client.rs`). The
fallback exists because the real-Drive test scripts export those
variables and call `init` and `join` without the flags; no user-facing
reason is recorded. It has two costs:

- The client id decides which application a Library is created as, and
  with the fallback that decision can be made silently by whatever
  `.envrc` happens to be loaded. Once test and everyday Libraries live
  under different Cloud projects, an `init` typed inside the repository
  without `--client-id` puts an everyday Library under the test project.
  Nothing in the output or the settings says where the id came from.
- The client secret can be typed as `--client-secret <value>`, which puts
  it in the shell history and the process table — the same exposure
  #170 closed for the Passphrase and the Recovery Code. It is not a
  secret a person holds: `init` stores it in the Library's settings
  and every later token refresh reads it from there, so it is
  configuration, and configuration is what the environment is for.

Make the two values arrive one way each:

1. **`--client-id` is required** on `init --drive` and `join --drive`:
   the `--drive` flag gains `requires = "client_id"` beside its existing
   `requires` (the field stays `Option<String>` because `--s3` has no
   client). `drive_client.rs` no longer resolves the id at all: its one
   helper reads the secret (see 2). Remove
   the `COFFRET_DRIVE_CLIENT_ID` fallback and its constant from
   `drive_client.rs`; the flag's help says the id is the OAuth desktop
   client registered in the account owner's own Cloud project, and that
   there is no built-in one (the reason `drive_client.rs` gives today).
2. **The client secret comes from `COFFRET_DRIVE_CLIENT_SECRET` only.**
   Remove the `--client-secret` flag from both commands. `drive_client.rs`
   keeps the constant and a single function, `client_secret()`, that reads
   the variable
   (absent means a client registered without a secret; an empty value is
   an error, since an empty secret is never what was meant and the
   token exchange would fail later with a worse message). The help for
   `--client-id` and the module doc say where the secret comes from and
   why it is not a flag.
3. **The scripts pass the id explicitly.** `scripts/drive-round-trip-it.sh`
   (`init` and `join`) and `scripts/drive-index-layout-it.sh` (`init`)
   add `--client-id "$COFFRET_DRIVE_CLIENT_ID"`; their existing
   `COFFRET_DRIVE_CLIENT_ID is not set` checks stay. Their header
   comments and the Makefile's `## drive-round-trip-it:` /
   `## drive-index-layout-it:` blocks say the variable is what the
   script passes as `--client-id`, and that the secret is read by the
   CLI itself from the environment. `scripts/drive-it-reset.sh` and the
   `authorize` / `app_folders` examples read the environment directly and
   are unaffected.
4. **Tests.** In `backend/crates/apps/coffret-cli/tests/`, a case that
   `init --drive --parent <id> --passphrase-stdin` without `--client-id`
   is refused by clap naming the flag, and one that `--client-secret` is
   no longer accepted. Existing cases that pass `--client-id` keep
   passing.

Nothing here needs a consent to verify: the argument handling is
offline, and the existing round-trip Libraries exercise the stored
credentials on the next run. A fresh `init` with the new interface is
verified naturally the next time a Library is created — the move to a
separate test project — and is listed as a manual item for that moment.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `init --drive` and `join --drive` require `--client-id`; the `COFFRET_DRIVE_CLIENT_ID` fallback is gone from the CLI
- [x] `--client-secret` is gone; the secret is read from `COFFRET_DRIVE_CLIENT_SECRET` only, with an empty value refused
- [x] Both real-Drive scripts pass `--client-id "$COFFRET_DRIVE_CLIENT_ID"` to `init` (and `join`), and their headers and the Makefile blocks describe the split
- [x] CLI tests cover the missing flag and the removed flag

### Manual / on-hardware (verified by a human before merge)

- [ ] `make drive-round-trip-it` on the existing Libraries runs green (stored credentials unaffected), and `coffret init --drive --parent x` without `--client-id` is refused offline
- [ ] The next fresh `init` / `join` against real Drive (the test-project move) creates its Library with the flag and the environment secret

## Out of scope

- Any change to how the settings store the id and secret, or to `authorize`
- The `authorize` and `app_folders` examples and `scripts/drive-it-reset.sh`
- A built-in default client id
