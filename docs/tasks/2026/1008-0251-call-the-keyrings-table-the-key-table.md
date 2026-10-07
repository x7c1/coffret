---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! git grep -q -e KeyringMapping -e keyring_mapping -e keyringMapping -- backend frontend docs/spec docs/concepts && ! git grep -qiE "present but locked|reported as locked" -- docs/spec && ! git grep -qiE "Keyring(.s)? mapping|generation.s (complete )?mapping|canonical (complete )?mapping" -- backend frontend docs/spec docs/concepts'
assignee: null
branch: task/1008-0251-call-the-keyrings-table-the-key-table
created_at: 2026-10-07T17:51:46Z
updated_at: 2026-10-07T18:36:58Z
---

# refactor(keyring): call the Keyring's table the key table everywhere, and a key-lost Container unreadable

## Overview

The concept docs now name the Keyring's Container-to-envelope table the
**key table** (`docs/concepts/keyring/README.md`), because "mapping" is also
the name of the Mapping concept (a device's mapped folders). They also say a
key-lost Container stays **unreadable** rather than "locked", because
"locked" is the state of a running Library that does not hold its Master Key.
The spec and the code still use the old words, so a reader moving between
concept, spec and code meets one thing under two names and one name for two
things. This change brings the spec and the code to the concept docs'
vocabulary. It changes names and prose only: no behaviour, no wire format.

**1. Key table.** Rename the type that holds one Keyring generation's
Container-to-envelope table, `KeyringMapping` (Rust:
`coffret-model/src/keyring_mapping.rs` and every user; TypeScript:
`frontend/packages/domain/format/src/model/keyringMapping.ts` and every user,
including the package's exports), to `KeyTable` / `key_table` / `keyTable`,
moving the files to match. Update doc comments and test names that say
"Keyring mapping", "the generation's mapping", "canonical (complete) mapping"
to say key table. In the spec (`docs/spec/keyring-lifecycle/`,
`commit-protocol/`, `format/` and any other register file), call it the key
table too.

The serialized field stays `mapping`: it is part of the control-object format
(FM-17) and existing Libraries carry it. Say so once where FM-17 lists the
payload's fields — the key table is serialized as the field `mapping` — and
keep the CBOR key, the interop manifest's field names, and fixtures
byte-for-byte unchanged. Generic uses of "mapping" that are not this table
(for example a device's folder mappings, or "a mapping from X to Y" in prose
about something else) stay as they are.

**2. Unreadable, not locked.** `docs/spec/recovery/README.md` RV-2 says a
Container mapped to a key-lost marker "is reported as locked", and RV-7 says
such Containers are "present but locked". Say unreadable (or key-lost), in
line with `docs/concepts/recovery-code/README.md`. Check the code and tests
for the same use of "locked" about a key-lost Container (as opposed to a
locked Library or device) and align it.

**3. Page.** `docs/concepts/library/README.md` defines a **page** as one step
of the reader's sequence, while the spec and concepts also say "page" for the
explorer's browser page ("the explorer's page", LA-3, LA-6). Add one sentence
to the Library concept that tells the two apart, so neither use needs
renaming.

Out of scope: renaming the wire field, and any change to what is serialized.

## Acceptance criteria

### Automated (pipeline-verified)
- [x] No identifier `KeyringMapping` / `keyring_mapping` / `keyringMapping` remains in code, spec or concepts; the type is `KeyTable` in Rust and TypeScript
- [x] No doc comment, spec or concept text calls the table "Keyring mapping", "the generation's mapping" or "canonical mapping"
- [x] The spec no longer calls a key-lost Container "locked"
- [x] Interop fixtures and format round-trip tests pass unchanged, so the serialized form is byte-for-byte the same
- [x] `make check` passes

### Before merge (verified outside the check command)
- [ ] Reading the diff confirms FM-17 states that the key table is serialized as the field `mapping`, and the Library concept distinguishes the reader's page from the explorer's browser page
