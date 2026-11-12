---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, spec-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it'
assignee: null
branch: task/1111-1710-name-the-key-file-before-asking
created_at: 2026-11-11T08:10:00Z
updated_at: 2026-11-11T08:10:00Z
---

# fix: name the key file when the desktop app cannot read it, and before asking for an account's Passphrase

## Overview

KD-9 tells a refusal of the stored Master Key file (missing, unreadable,
malformed, or not derivable) apart from a wrong Passphrase. Two paths still
lose that distinction:

1. **The desktop app does not say "key file" for an unreadable one.**
   `backend/crates/apps/coffret-desktop-core/src/refusal_line.rs` turns
   `Error::Local` into the generic "The Library could not be opened.",
   while the missing, malformed and not-derivable cases say "This
   Library's key file …". Give an unreadable key file a line that names the
   key file in the same way, saying also that the Passphrase was not
   checked.
2. **An account's key is asked for a Passphrase before the key file is
   read.** `key_for` in `backend/crates/apps/coffret-device/src/accounts.rs`,
   when it may ask, asks for the Passphrase of the first Library it consults
   and only then returns `MasterKeyFileMissing`. Read that Library's stored
   Master Key file before asking, and give a refusal of it without asking,
   so nobody types a Passphrase that cannot be used.

Write the second behaviour into KD-9 for the account path only (SA-9), as:
"Where the Passphrase of a Library referencing an account would be asked
for to open the account's key (SA-9), that Library's stored Master Key file
is read first, and a refusal of it is given without asking: nobody types a
Passphrase the file could not be opened with." Opening a Library itself and
`authorize` keep taking the Passphrase first; do not change them, and do not
write a broader rule. Leave the promotion of an older Library's grant cache
(`account_grant.rs`, `promotion_obstacle.rs`, SA-8) untouched: it is being
removed separately.

Name the rule from the tests. Keep the key file's format, the CLI's exit
codes and the HTTP status and refusal codes that already exist as they are;
if a path cannot be fixed without changing one of those, stop and say so
instead of changing it. Do not change `.github/`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A test of `refusal_line` shows an unreadable key file's line naming
      the key file.
- [x] A test of `key_for` with a missing key file returns
      `MasterKeyFileMissing` without asking for a Passphrase.
- [x] KD-9 states the account-path behaviour above and the tests cite it;
      `make check` and `make e2e-it` pass.
