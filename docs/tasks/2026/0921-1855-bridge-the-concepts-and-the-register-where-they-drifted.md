---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [concept-alignment, completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && grep -q 'chunk run' docs/concepts/container/README.md && grep -q '^- choose' docs/concepts/passphrase/README.md && grep -Eq '^- fill' docs/concepts/library/README.md && grep -Eq '^- arm' docs/concepts/library/README.md && grep -q 'KD-11' docs/concepts/master-key/README.md && ! grep -rq 'upload batch' docs/concepts docs/spec"
assignee: null
branch: task/0921-1855-bridge-the-concepts-and-the-register-where-they-drifted
created_at: 2026-09-21T09:55:17Z
updated_at: 2026-09-21T10:39:04Z
---

# docs: bridge the concepts and the register where they drifted, and register the verbs the code already uses

## Overview

A documentation pass over `docs/concepts/**` and `docs/spec/**` only. No
code, no behaviour, no rule whose meaning changes. Each item below was
checked against the tree and is still open; the locations are where it
was last seen, so re-find each by its text before editing. Follow the
document skeleton and vocabulary policy in `docs/concepts/README.md`, cite
rules as the neighbouring text does (`(spec: EP-11)`), and keep each
addition to the sentence or bullet it needs.

Vocabulary to register or bridge:

1. **`chunk run`** has no bridge on the concept side: the Container
   concept's Streamable rule never uses the word FM-5 and PK-16 use.
   Add it to `docs/concepts/container/README.md`.
2. **"upload batch"** survives in `docs/concepts/README.md`,
   `docs/concepts/storage-object/README.md` and the header of
   `docs/spec/commit-protocol/README.md`, where the Journal concept
   defines **batch**. Use the defined word.
3. **"partial fetch"** has two meanings: the Container concept uses it for
   the flow that range-reads, while the Library concept and the
   entry-path register use it for a fetch that was interrupted. Keep the
   first and reword the other two as an interrupted fetch.
4. **"human-readable part" / "prefix"**: KD-11 and the Recovery Code
   concept name the same thing two ways. Say once, in KD-11, that they
   are one thing, without renaming anything in code.
5. **`choose`** (a Passphrase) is missing from the Passphrase concept's
   Collocations.
6. **`fill`, `arm`, `map`** are verbs the server and the CLI already use
   (`coffret-server/src/fill/`, "arms a sync", `coffret map`) and the
   Library concept's Collocations list only `serve` and `drop`. Register
   the three, each with its object, reading the code for what each means.

What the register already says and the concept does not:

7. The Master Key concept cites `(spec: KD-8)` for the Recovery Code
   where KD-11 now also applies; cite both.
8. The Library concept's FM-18 rule does not say that the Library ID is
   configuration and not key material, which FM-18 does. Add it, and put
   beside it the Domain Rule that a device's settings are device-local.
9. The Storage concept's Collocations lack the app folder's verbs
   (`create`, `discover`); the Recovery Code concept does not say how a
   Code alone leads back to the Library; the Storage concept does not say
   what renaming the folder breaks. FM-18 has all three.
10. The Library concept lacks the one line that size-independence is the
    Library's contract while an app that serves it may hold narrower
    bounds of its own (spec: LA-9, LA-10, LA-11).
11. The entry-path concept states EP-4 for the scanning side only; the
    register states both sides. Add the placing side.
12. The entry-path concept lacks the sub-bullet for the two kinds of
    reason a fetch declines a file (the file's business, the mapping's
    business), which EP-11 already spells out.
13. The entry-path concept does not say that a name a person chose never
    reaches a diagnostic event (spec: EL-1).
14. The entry-path concept does not register a derived **folder**; define
    it in one sentence, following how a derived Entry is already defined
    there.

Statements that have aged:

15. The Pack concept's "A browsing unit is simply a folder … fetching that
    set", its Collocation, and the matching lines of the Library concept
    predate per-Entry range reads (PK-16). Bring them up to date.
16. OC-2's sub-bullet ends "leaves nothing cleanup cannot reach", which
    claims more than the rule holds. Narrow the sentence to what OC-2
    establishes; do not change the rule.

The Index concept:

17. Add the sentence that browsing never touches Storage, and the bullet
    for device state that lives only as long as the process.
18. Add one Domain Rule that a catalog may be open in more than one
    process at once and what makes that safe (the write-ahead log and the
    busy timeout the SQLite adapter sets). A Domain Rule in the concept
    only — do not mint a register rule for it.

Register additions, each only if the code already does it and a test
already holds it — read the code first, and if either is missing leave the
item out and say so in your report:

19. EP-11 states when a placed file matches an Entry but no rule obliges a
    fetch to stamp the Entry's mtime on the file it places. Add the
    sub-bullet under EP-11.
20. PK-3 does not say that a Pack is closed before its entry table reaches
    the FM-2 ceiling. Add the sub-bullet under PK-3.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The Container concept uses `chunk run`; the Passphrase concept registers `choose`; the Library concept registers `fill` and `arm`; the Master Key concept cites KD-11
- [x] "upload batch" no longer appears under `docs/concepts` or `docs/spec`
- [x] `make check` passes

### Manual / on-hardware (verified by a human before merge)

- [ ] Each of the twenty items is either done as described or reported as left out with the reason, and no register rule changed meaning

## Out of scope

- Any code, comment in code, or CLI help text
- A concept document for mappings, the words `remote`, `explorer` / `reader` / `viewer`, `supersede`, and the `(spec: …)` citation convention, which need decisions first
- Guidance on how large a unit to freeze, which has no home in `docs/` yet
