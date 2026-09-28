---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 2
retries_remaining: 1
check_command: 'make check && grep -qE "^check:.*spec-rule-ids" Makefile && ! git grep -qE "^- \*\*(SA-8|SA-9|KD-12)\.\*\*" -- docs/spec && git grep -qE "^//! SA-8:" -- backend && git grep -qE "^//! SA-9:" -- backend && git grep -qE "^//! KD-12:" -- backend && git ls-files -z docs/concepts docs/spec backend | xargs -0 perl -0777 -ne "exit 1 if /(another|different)\s+reach/i"'
assignee: null
branch: task/0928-1933-say-a-grant-reaches-what-its-project-reaches-and-refuse-a-rule-id-with-no-home
created_at: 2026-09-28T19:33:36Z
updated_at: 2026-09-28T20:05:56Z
---

# docs: say a grant reaches what its client's Cloud project reaches, and refuse a rule ID with no home

## Overview

Two things, both found while reading SA-8.

**1. Why one account name stands for one OAuth client is stated wrongly.** Under `drive.file`, what a grant reaches is decided by the account and the Cloud project the OAuth client belongs to: another client of the same project reaches the same objects. That is how a second device is meant to be set up — a client of its own, in the same project — and a second device has joined a Library that way. What another client gives is another *grant*: the refresh token is bound to the client that obtained it and is refreshed only through that client's id and secret. Several places say instead that another client means another *reach*:

- SA-8's statement in the module doc of `backend/crates/apps/coffret-device/src/account_tests.rs` ("a token minted through another client is a different grant with a different reach")
- `docs/concepts/storage/README.md`, in the opening paragraph ("decided by the account that consented and the OAuth client it consented to") and in the Domain Rules ("another grant with another reach")
- doc comments in `backend/crates/apps/coffret-device/src/account_grant.rs` and `account_settings.rs`, the comment on the `drive-it-list` target in the `Makefile`, and `backend/crates/gateway/google-drive-store/examples/app_folders.rs` — find every instance by what it claims, not by one phrasing.

Keep the rule — one account name binds one client id on the device — and give the reason above wherever it is given.

**2. Refuse a cited rule ID that has no home.** Per `docs/spec/README.md` (around lines 15-20 and 55-61), a rule lives in exactly one place: in the register as `**XX-n.**` while it is prose, and — once a `Form: test` rule has migrated — in the test comment that holds its full statement, with the register entry deleted in the same commit. SA-8, SA-9 and KD-12 are the first rules migrated that way; each full statement opens a module doc, as `//! SA-8: …` (`account_tests.rs`) and `//! KD-12: …` (`coffret-format/src/account_cache_key_envelope/tests.rs`). No other line in `backend/` or `frontend/` opens a module doc with an ID; the `// KD-4: …` form that opens a test comment names a rule the case samples and is not a home. Nothing checks today that a cited ID resolves to either home, so a citation whose rule was never written, or was lost, passes `make check`.

Add a `spec-rule-ids` target to the `Makefile` and make it a prerequisite of `check` beside `spec-citations`. It collects the homes — `**XX-n.**` under `docs/spec/`, and `//! XX-n:` opening a line under `backend/` and `frontend/` — and every `XX-n` cited under `backend/`, `frontend/`, `docs/concepts/` and `docs/spec/`, for the prefixes the register defines. It fails, naming each ID and where it is cited, when a cited ID has no home; and it fails, naming both places, when an ID has two homes — a rule left in the register after its statement moved into a test is a migration done by half, which the README forbids. Leave `docs/tasks/` out: task files are records of past work. Give it a `## spec-rule-ids:` help line and a comment in the style of `spec-citations` (Makefile, around line 551) that says what it holds and what it cannot see (for example, a migrated home written in a form other than `//! XX-n:`). Make it fail rather than pass when `git grep` cannot search, as `spec-citations` does. Show both refusals by hand: cite an ID nothing defines, then put one migrated rule's `**XX-n.**` entry back in the register; see each named; restore.

Do not move SA-8, SA-9 or KD-12 back into the register; they are where the README says they belong.

Out of scope: the Japanese copies of the concepts are kept outside this repository and are synced separately.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] No concept, register or backend file says another client gives a grant another or a different reach (a `perl` gate across line breaks over `docs/concepts`, `docs/spec` and `backend`)
- [x] SA-8, SA-9 and KD-12 stay migrated: their `//! XX-n:` homes are present and the register has no `**XX-n.**` entry for them (grep gates)
- [x] `make check` runs a `spec-rule-ids` check (grep gate on `check`'s prerequisites, and `make check` passes with it)
- [x] `make check` passes

### Manual (verified by a human before merge)

- [ ] `spec-rule-ids` refuses both an ID with no home and an ID with two homes, naming the places (the refusals shown by hand in the work phase)
